//! Push bundled (or nearby) firmware to the ESP32-2432S028 over USB serial.
//! Prefers bundled / auto-downloaded `espflash`; optional Python `esptool` fallback.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

pub const MERGED_BIN_NAME: &str = "esp32-2432s028-sha256-miner-merged.bin";

#[derive(Clone, Debug)]
pub struct FirmwareImage {
    pub path: PathBuf,
    pub bytes: u64,
    pub version: String,
}

/// Resolve the merged firmware image shipped next to the Companion exe / kit.
pub fn find_firmware_image() -> Result<FirmwareImage, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("Firmware").join(MERGED_BIN_NAME));
            candidates.push(dir.join(MERGED_BIN_NAME));
            // Dev: run from companion/target/... → walk up toward repo flash/
            let mut walk = dir.to_path_buf();
            for _ in 0..6 {
                candidates.push(walk.join("flash").join(MERGED_BIN_NAME));
                candidates.push(
                    walk.join("dist")
                        .join("cyd-miner-kit")
                        .join("Firmware")
                        .join(MERGED_BIN_NAME),
                );
                if !walk.pop() {
                    break;
                }
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("Firmware").join(MERGED_BIN_NAME));
        candidates.push(cwd.join("flash").join(MERGED_BIN_NAME));
        candidates.push(cwd.join(MERGED_BIN_NAME));
    }

    for path in candidates {
        if path.is_file() {
            let bytes = std::fs::metadata(&path)
                .map(|m| m.len())
                .unwrap_or(0);
            if bytes > 64_000 {
                let version = read_nearby_fw_version(&path);
                return Ok(FirmwareImage {
                    path,
                    bytes,
                    version,
                });
            }
        }
    }

    Err(format!(
        "Firmware image not found ({MERGED_BIN_NAME}). Use CYD Miner Setup / Portable kit so Firmware\\ sits next to the app."
    ))
}

pub fn normalize_fw_version(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_ascii_lowercase()
}

pub fn read_nearby_fw_version(bin_path: &Path) -> String {
    if let Some(dir) = bin_path.parent() {
        for name in ["VERSION.txt", "version.txt", "FW_VERSION.txt"] {
            let p = dir.join(name);
            if let Ok(txt) = std::fs::read_to_string(&p) {
                for line in txt.lines() {
                    let t = line.trim();
                    if t.is_empty() || t.starts_with('#') {
                        continue;
                    }
                    if t.contains("sha256") || t.contains('.') {
                        if let Some(rest) = t.split(':').nth(1) {
                            return normalize_fw_version(rest);
                        }
                        return normalize_fw_version(t);
                    }
                }
            }
        }
    }
    String::new()
}

/// Compare board `cmp config` fw string vs bundled image version.
pub fn update_needed(board_fw: &str, bundled: &str) -> Option<bool> {
    let a = normalize_fw_version(board_fw);
    let b = normalize_fw_version(bundled);
    if a.is_empty() || b.is_empty() {
        return None;
    }
    Some(a != b)
}

/// Download latest merged.bin (GitHub release asset, else raw repo flash/downloads/).
pub fn fetch_latest_firmware(
    progress: &dyn Fn(String),
) -> Result<FirmwareImage, String> {
    let dest_dir = firmware_writable_dir()?;
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("mkdir Firmware: {e}"))?;
    let dest = dest_dir.join(MERGED_BIN_NAME);
    let ver_path = dest_dir.join("VERSION.txt");

    progress("Checking GitHub releases for firmware…".into());
    if let Ok((url, ver)) = find_release_firmware_url() {
        progress(format!("Downloading release firmware {ver}…"));
        download_to(&url, &dest, progress)?;
        let _ = std::fs::write(&ver_path, format!("{ver}\n"));
        let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        if bytes > 64_000 {
            return Ok(FirmwareImage {
                path: dest,
                bytes,
                version: normalize_fw_version(&ver),
            });
        }
        progress(format!("Release asset too small ({bytes} B) — trying repo…"));
    } else {
        progress("No CYD release asset — trying repository flash/downloads…".into());
    }

    let mut last = String::new();
    for url in raw_firmware_candidate_urls() {
        progress(format!("GET {url}"));
        match download_to(&url, &dest, progress) {
            Ok(()) => {
                let ver = fetch_nearby_version_hint(&url).unwrap_or_else(|| "repo-flash".into());
                let _ = std::fs::write(&ver_path, format!("{ver}\n"));
                let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                if bytes > 64_000 {
                    return Ok(FirmwareImage {
                        path: dest,
                        bytes,
                        version: normalize_fw_version(&ver),
                    });
                }
                last = format!("download too small ({bytes} bytes)");
            }
            Err(e) => last = e,
        }
    }

    progress("Raw .bin missing — extracting from portable kit zip…".into());
    match fetch_firmware_from_portable_zip(&dest_dir, progress) {
        Ok(img) => return Ok(img),
        Err(e) => {
            if last.is_empty() {
                last = e;
            } else {
                last = format!("{last}; zip fallback: {e}");
            }
        }
    }

    Err(format!(
        "Could not fetch firmware ({last}). Place {MERGED_BIN_NAME} in Firmware\\ manually."
    ))
}

fn raw_firmware_candidate_urls() -> Vec<String> {
    let mut out = Vec::new();
    for p in [
        "flash/downloads/esp32-2432s028-sha256-miner-merged.bin",
        "flash/esp32-2432s028-sha256-miner-merged.bin",
    ] {
        out.extend(repo_file_urls(p));
    }
    out
}

fn fetch_nearby_version_hint(bin_url: &str) -> Option<String> {
    // Prefer the same fresh URL set used for app update checks.
    let mut candidates = repo_file_urls("flash/downloads/VERSION.txt");
    if let Some((base, _)) = bin_url.rsplit_once('/') {
        candidates.insert(0, format!("{base}/VERSION.txt"));
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(6))
        .timeout_read(std::time::Duration::from_secs(10))
        .user_agent(COMPANION_UA)
        .build();
    for ver_url in candidates {
        let mut req = agent.get(&ver_url);
        if ver_url.contains("api.github.com/repos/") && ver_url.contains("/contents/") {
            req = req.set("Accept", "application/vnd.github.raw");
        }
        let Ok(resp) = req.call() else { continue };
        let Ok(txt) = resp.into_string() else { continue };
        for line in txt.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if let Some(rest) = t.split(':').nth(1) {
                let v = normalize_fw_version(rest);
                if !v.is_empty() {
                    return Some(v);
                }
            }
            let v = normalize_fw_version(t);
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn portable_zip_candidate_urls() -> Vec<String> {
    let mut out = Vec::new();
    for n in [
        "CYD-Miner-Portable.zip",
        "CYD-Companion-Portable.zip",
        "CYD-Companion-App-Only.zip",
    ] {
        out.extend(repo_file_urls(&format!("flash/downloads/{n}")));
    }
    out
}

fn fetch_firmware_from_portable_zip(
    dest_dir: &Path,
    progress: &dyn Fn(String),
) -> Result<FirmwareImage, String> {
    let zip_path = dest_dir.join("fetch-kit.zip.part");
    let dest = dest_dir.join(MERGED_BIN_NAME);
    let ver_path = dest_dir.join("VERSION.txt");
    let mut last = String::new();

    for url in portable_zip_candidate_urls() {
        progress(format!("GET {url}"));
        match download_to(&url, &zip_path, progress) {
            Ok(()) => match extract_merged_from_zip(&zip_path, &dest, &ver_path, progress) {
                Ok(img) => {
                    let _ = std::fs::remove_file(&zip_path);
                    return Ok(img);
                }
                Err(e) => {
                    last = e;
                    let _ = std::fs::remove_file(&zip_path);
                }
            },
            Err(e) => last = e,
        }
    }
    Err(if last.is_empty() {
        "no portable kit zip".into()
    } else {
        last
    })
}

fn extract_merged_from_zip(
    zip_path: &Path,
    dest: &Path,
    ver_path: &Path,
    progress: &dyn Fn(String),
) -> Result<FirmwareImage, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip: {e}"))?;

    let mut version = String::new();
    let mut found_bin = false;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry: {e}"))?;
        let name = entry.name().replace('\\', "/");
        let lower = name.to_ascii_lowercase();
        let file_name = name.rsplit('/').next().unwrap_or("");

        if file_name.eq_ignore_ascii_case("VERSION.txt")
            && (lower.contains("firmware") || lower.ends_with("/version.txt"))
        {
            let mut txt = String::new();
            std::io::Read::read_to_string(&mut entry, &mut txt)
                .map_err(|e| format!("read VERSION: {e}"))?;
            for line in txt.lines() {
                let t = line.trim();
                if t.is_empty() || t.starts_with('#') {
                    continue;
                }
                version = if let Some(rest) = t.split(':').nth(1) {
                    normalize_fw_version(rest)
                } else {
                    normalize_fw_version(t)
                };
                if !version.is_empty() {
                    break;
                }
            }
        }

        if file_name.eq_ignore_ascii_case(MERGED_BIN_NAME) {
            progress(format!("Extracting {name}…"));
            let mut out = std::fs::File::create(dest).map_err(|e| format!("create bin: {e}"))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("extract bin: {e}"))?;
            found_bin = true;
        }
    }

    if !found_bin {
        return Err(format!("{MERGED_BIN_NAME} not inside kit zip"));
    }
    if version.is_empty() {
        version = "kit-zip".into();
    }
    let _ = std::fs::write(ver_path, format!("{version}\n"));
    let bytes = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    if bytes <= 64_000 {
        return Err(format!("extracted bin too small ({bytes} bytes)"));
    }
    progress(format!(
        "Saved {} ({} KB) from portable kit",
        dest.display(),
        bytes / 1024
    ));
    Ok(FirmwareImage {
        path: dest.to_path_buf(),
        bytes,
        version,
    })
}

pub const COMPANION_UA: &str = "Njordr-seas-CYD-miner/0.8.46";
const ESPFLASH_VERSION: &str = "4.5.0";
pub const REPO_OWNER: &str = "GutFarms";
pub const REPO_NAME: &str = "Japan-central";
pub const REPO_REFS: &[&str] = &[
    "cursor/esp32-cyd-cpp-firmware-e801",
    "master",
    "main",
];

fn urlencode_ref(ref_name: &str) -> String {
    ref_name
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Resolve a branch/tag to a commit SHA (avoids stale raw.githubusercontent.com branch CDN).
pub fn resolve_ref_commit(ref_name: &str) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

    if let Ok(guard) = CACHE.lock() {
        if let Some(map) = guard.as_ref() {
            if let Some(sha) = map.get(ref_name) {
                return Some(sha.clone());
            }
        }
    }

    let url = format!(
        "https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/commits/{}",
        urlencode_ref(ref_name)
    );
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(15))
        .user_agent(COMPANION_UA)
        .build();
    let resp = agent
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    #[derive(serde::Deserialize)]
    struct Commit {
        sha: String,
    }
    let c: Commit = resp.into_json().ok()?;
    if c.sha.len() < 7 {
        return None;
    }
    if let Ok(mut guard) = CACHE.lock() {
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(ref_name.to_string(), c.sha.clone());
    }
    Some(c.sha)
}

/// Candidate URLs for a repo path: GitHub Contents API (fresh) → commit-pinned raw → branch raw.
pub fn repo_file_urls(path: &str) -> Vec<String> {
    let path = path.trim_start_matches('/');
    let mut out = Vec::new();
    for r in REPO_REFS {
        let enc = urlencode_ref(r);
        // Contents API with Accept: raw bypasses the branch CDN (see download_to).
        // Works for files ≤1MB; larger files fall through to commit-pinned raw.
        out.push(format!(
            "https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/contents/{path}?ref={enc}"
        ));
        if let Some(sha) = resolve_ref_commit(r) {
            out.push(format!(
                "https://raw.githubusercontent.com/{REPO_OWNER}/{REPO_NAME}/{sha}/{path}"
            ));
        }
        out.push(format!(
            "https://raw.githubusercontent.com/{REPO_OWNER}/{REPO_NAME}/{r}/{path}"
        ));
    }
    out
}

fn firmware_writable_dir() -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return Ok(dir.join("Firmware"));
        }
    }
    std::env::current_dir()
        .map(|d| d.join("Firmware"))
        .map_err(|e| e.to_string())
}

fn tools_writable_dir() -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return Ok(dir.join("Tools"));
        }
    }
    std::env::current_dir()
        .map(|d| d.join("Tools"))
        .map_err(|e| e.to_string())
}

fn espflash_download_urls() -> Vec<String> {
    // Prefer our tracked copy (API / commit-pinned), then upstream zip.
    let mut out = repo_file_urls("flash/downloads/espflash.exe");
    out.push(format!(
        "https://github.com/esp-rs/espflash/releases/download/v{ESPFLASH_VERSION}/espflash-x86_64-pc-windows-msvc.zip"
    ));
    out
}

fn looks_like_espflash(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(m) => m.is_file() && m.len() > 1_000_000,
        Err(_) => false,
    }
}

/// Ensure `Tools/espflash.exe` exists (local kit, then download).
pub fn ensure_espflash(progress: &dyn Fn(String)) -> Result<PathBuf, String> {
    // Always prefer a real file next to the app (never a bare PATH name).
    for name in ["espflash.exe", "espflash"] {
        if let Some(p) = find_tool_file(name) {
            if looks_like_espflash(&p) {
                progress(format!("Found espflash · {}", p.display()));
                return Ok(p);
            }
        }
    }

    let tools = tools_writable_dir()?;
    std::fs::create_dir_all(&tools).map_err(|e| format!("mkdir Tools: {e}"))?;
    let exe_path = tools.join("espflash.exe");

    let mut last = String::new();
    for url in espflash_download_urls() {
        progress(format!("Downloading espflash ({ESPFLASH_VERSION})…"));
        progress(format!("GET {url}"));
        let lower = url.to_ascii_lowercase();
        if lower.ends_with(".exe") {
            match download_to(&url, &exe_path, progress) {
                Ok(()) if looks_like_espflash(&exe_path) => {
                    progress(format!("espflash ready · {}", exe_path.display()));
                    return Ok(exe_path);
                }
                Ok(()) => {
                    last = format!("download too small: {}", exe_path.display());
                    let _ = std::fs::remove_file(&exe_path);
                }
                Err(e) => last = e,
            }
        } else {
            let zip_path = tools.join("espflash-fetch.zip.part");
            match download_to(&url, &zip_path, progress) {
                Ok(()) => match extract_named_from_zip(&zip_path, "espflash.exe", &exe_path, progress)
                {
                    Ok(()) if looks_like_espflash(&exe_path) => {
                        let _ = std::fs::remove_file(&zip_path);
                        progress(format!("espflash ready · {}", exe_path.display()));
                        return Ok(exe_path);
                    }
                    Ok(()) => {
                        last = "zip extract produced tiny espflash.exe".into();
                        let _ = std::fs::remove_file(&exe_path);
                        let _ = std::fs::remove_file(&zip_path);
                    }
                    Err(e) => {
                        last = e;
                        let _ = std::fs::remove_file(&zip_path);
                    }
                },
                Err(e) => last = e,
            }
        }
    }

    Err(format!(
        "Could not get espflash.exe ({last}). Re-install CYD-Miner-Setup / App-Only zip (includes Tools\\espflash.exe), or copy espflash.exe into Tools\\ next to the app."
    ))
}

fn extract_named_from_zip(
    zip_path: &Path,
    want_name: &str,
    dest: &Path,
    progress: &dyn Fn(String),
) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry: {e}"))?;
        let name = entry.name().replace('\\', "/");
        let file_name = name.rsplit('/').next().unwrap_or("");
        if file_name.eq_ignore_ascii_case(want_name) {
            progress(format!("Extracting {name}…"));
            let mut out = std::fs::File::create(dest).map_err(|e| format!("create: {e}"))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("extract: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("{want_name} not inside zip"))
}

/// Resolve a flashable image: local kit first, otherwise fetch into Firmware\\.
pub fn ensure_firmware_image(progress: &dyn Fn(String)) -> Result<FirmwareImage, String> {
    if let Ok(img) = find_firmware_image() {
        progress(format!(
            "Using local firmware {} ({} KB)",
            if img.version.is_empty() {
                "bundled"
            } else {
                &img.version
            },
            img.bytes / 1024
        ));
        return Ok(img);
    }
    progress("No local Firmware\\ image — fetching…".into());
    fetch_latest_firmware(progress)
}

fn find_release_firmware_url() -> Result<(String, String), String> {
    #[derive(serde::Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
    }
    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
        assets: Vec<Asset>,
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(20))
        .user_agent(COMPANION_UA)
        .build();
    let releases: Vec<Release> = agent
        .get("https://api.github.com/repos/GutFarms/Japan-central/releases?per_page=20")
        .call()
        .map_err(|e| format!("releases api: {e}"))?
        .into_json()
        .map_err(|e| format!("releases json: {e}"))?;
    for rel in releases {
        for a in rel.assets {
            let n = a.name.to_ascii_lowercase();
            if n.contains("merged")
                && n.ends_with(".bin")
                && (n.contains("2432") || n.contains("cyd") || n.contains("sha256"))
            {
                let ver = if rel.tag_name.is_empty() {
                    a.name.clone()
                } else {
                    rel.tag_name.clone()
                };
                return Ok((a.browser_download_url, ver));
            }
        }
    }
    Err("no matching release asset".into())
}

fn download_to(url: &str, dest: &Path, progress: &dyn Fn(String)) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(240))
        .user_agent(COMPANION_UA)
        .build();
    let mut req = agent.get(url);
    // GitHub Contents API: ask for raw bytes (avoids base64 JSON + stale branch CDN).
    if url.contains("api.github.com/repos/") && url.contains("/contents/") {
        req = req.set("Accept", "application/vnd.github.raw");
    }
    let resp = req.call().map_err(|e| format!("http: {e}"))?;
    let status = resp.status();
    if !(200..300).contains(&status) {
        return Err(format!("http {status} for {url}"));
    }
    let mut reader = resp.into_reader();
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("create: {e}"))?;
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| format!("write: {e}"))?;
        total += n as u64;
        if total % (512 * 1024) < n as u64 {
            progress(format!("Downloaded {} KB…", total / 1024));
        }
    }
    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| format!("rename: {e}"))?;
    progress(format!("Saved {} ({} KB)", dest.display(), total / 1024));
    Ok(())
}

/// Find a real on-disk tool file (not a bare PATH name).
fn find_tool_file(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            dirs.push(dir.join("Tools"));
            dirs.push(dir.join("tools"));
            // If user left the zip folder layout: ../Tools next to a nested exe
            if let Some(parent) = dir.parent() {
                dirs.push(parent.join("Tools"));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.clone());
        dirs.push(cwd.join("Tools"));
        dirs.push(cwd.join("tools"));
    }

    for dir in &dirs {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }

    which_on_path(name)
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
        #[cfg(windows)]
        {
            let p_exe = dir.join(format!("{name}.exe"));
            if p_exe.is_file() {
                return Some(p_exe);
            }
            let p_cmd = dir.join(format!("{name}.cmd"));
            if p_cmd.is_file() {
                return Some(p_cmd);
            }
            let p_bat = dir.join(format!("{name}.bat"));
            if p_bat.is_file() {
                return Some(p_bat);
            }
        }
    }
    None
}

/// Run an espflash subcommand, retrying skip-update-check flag forms for Windows clap quirks.
fn run_espflash_argv(
    espflash: &Path,
    sub_args: &[&str],
    progress: &dyn Fn(String),
    label: &str,
) -> Result<(), String> {
    // Global skip-update-check must come *before* the subcommand (espflash 4.x).
    // Never set ESPFLASH_SKIP_UPDATE_CHECK=1 — some Windows clap builds only accept
    // true/false and fail with: invalid value '1' for '--skip-update-check'.
    let mut last = String::new();
    for (skip, set_env_true) in [
        (&["--skip-update-check"][..], false),
        (&["--skip-update-check=true"][..], true),
        (&[][..], true),
    ] {
        let mut cmd = Command::new(espflash);
        cmd.env_remove("ESPFLASH_SKIP_UPDATE_CHECK");
        if set_env_true {
            cmd.env("ESPFLASH_SKIP_UPDATE_CHECK", "true");
        }
        cmd.args(skip).args(sub_args);
        match run_streaming(&mut cmd, progress, label) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let lower = e.to_ascii_lowercase();
                last = e;
                // Only rotate skip-flag forms on CLI parse errors; port/flash errors stop here.
                if !(lower.contains("skip-update-check")
                    || lower.contains("skip_update_check")
                    || lower.contains("unexpected argument")
                    || lower.contains("invalid value"))
                {
                    return Err(last);
                }
                progress("espflash CLI variant failed — trying next…".into());
            }
        }
    }
    Err(last)
}

fn run_espflash_erase(
    espflash: &Path,
    port: &str,
    baud: &str,
    progress: &dyn Fn(String),
) -> Result<(), String> {
    progress(format!(
        "espflash erase-flash → {port} @ {baud} ({})",
        espflash.display()
    ));
    // Stay in the bootloader so write-bin can follow without another reset dance.
    run_espflash_argv(
        espflash,
        &[
            "erase-flash",
            "-p",
            port,
            "-B",
            baud,
            "-c",
            "esp32",
            "--non-interactive",
            "--after",
            "no-reset",
        ],
        progress,
        "espflash-erase",
    )
}

fn run_espflash_write(
    espflash: &Path,
    port: &str,
    baud: &str,
    image: &Path,
    progress: &dyn Fn(String),
) -> Result<(), String> {
    progress(format!(
        "espflash write-bin → {port} @ {baud} ({})",
        espflash.display()
    ));
    let addr = "0x0";
    let img = image.to_string_lossy();
    run_espflash_argv(
        espflash,
        &[
            "write-bin",
            "-p",
            port,
            "-B",
            baud,
            "-c",
            "esp32",
            "--non-interactive",
            addr,
            img.as_ref(),
        ],
        progress,
        "espflash",
    )
}

/// Full erase, then flash merged image @ 0x0 (DIO / 4MB / 40MHz layout inside the merge).
pub fn flash_merged_bin(port: &str, image: &Path, progress: &dyn Fn(String)) -> Result<(), String> {
    if port.trim().is_empty() {
        return Err("Select a COM / serial port before updating.".into());
    }
    if !image.is_file() {
        return Err(format!("Firmware missing: {}", image.display()));
    }

    progress(format!(
        "Erase + flash {} ({} bytes) → {} @ 0x0",
        image
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("firmware.bin"),
        std::fs::metadata(image).map(|m| m.len()).unwrap_or(0),
        port
    ));
    progress("Hold BOOT + tap RESET if the board does not enter download mode.".into());

    // espflash is required — do not fall through to a misleading "esptool.py not found".
    let espflash = ensure_espflash(progress)?;
    let mut esp_err = String::new();
    for baud in ["460800", "115200"] {
        progress(format!("Erasing entire flash @ {baud}…"));
        match run_espflash_erase(&espflash, port, baud, progress) {
            Ok(()) => {
                progress("Erase done — writing firmware…".into());
                match run_espflash_write(&espflash, port, baud, image, progress) {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        esp_err = format!("write after erase: {e}");
                        progress(format!("espflash write @ {baud} failed: {esp_err}"));
                    }
                }
            }
            Err(e) => {
                esp_err = format!("erase: {e}");
                progress(format!("espflash erase @ {baud} failed: {esp_err}"));
            }
        }
        if baud == "460800" {
            progress("Retrying erase+flash at 115200…".into());
        }
    }

    // Optional Python fallback — skip Windows Store python stubs (exit 9009).
    let mut py_err = String::new();
    let py_bins: Vec<PathBuf> = ["py", "python", "python3"]
        .iter()
        .filter_map(|n| which_on_path(n))
        .filter(|p| !is_windows_store_python_stub(p))
        .collect();
    if !py_bins.is_empty() {
        progress("espflash failed — trying Python esptool (erase_flash + write_flash)…".into());
        for baud in ["460800", "115200"] {
            for py in &py_bins {
                let py_launcher = py
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("py") || s.eq_ignore_ascii_case("py.exe"))
                    .unwrap_or(false);

                // 1) erase_flash
                let mut erase_args: Vec<String> = Vec::new();
                if py_launcher {
                    erase_args.extend(["-3".into(), "-m".into(), "esptool".into()]);
                } else {
                    erase_args.extend(["-m".into(), "esptool".into()]);
                }
                erase_args.extend([
                    "--chip".into(),
                    "esp32".into(),
                    "--port".into(),
                    port.into(),
                    "--baud".into(),
                    baud.into(),
                    "erase_flash".into(),
                ]);
                progress(format!("esptool erase_flash via {} @ {baud}…", py.display()));
                let mut erase_cmd = Command::new(py);
                erase_cmd.args(&erase_args);
                if let Err(e) = run_streaming(&mut erase_cmd, progress, "esptool-erase") {
                    if e.contains("9009") || e.to_ascii_lowercase().contains("microsoft store") {
                        progress("Skipping Windows Store Python stub…".into());
                        continue;
                    }
                    py_err = e;
                    progress("esptool erase failed — trying next…".into());
                    continue;
                }

                // 2) write_flash
                let mut args: Vec<String> = Vec::new();
                if py_launcher {
                    args.extend(["-3".into(), "-m".into(), "esptool".into()]);
                } else {
                    args.extend(["-m".into(), "esptool".into()]);
                }
                args.extend([
                    "--chip".into(),
                    "esp32".into(),
                    "--port".into(),
                    port.into(),
                    "--baud".into(),
                    baud.into(),
                    "write_flash".into(),
                    "-z".into(),
                    "--flash_mode".into(),
                    "dio".into(),
                    "--flash_freq".into(),
                    "40m".into(),
                    "--flash_size".into(),
                    "4MB".into(),
                    "0x0".into(),
                    image.display().to_string(),
                ]);
                progress(format!("esptool write_flash via {} @ {baud}…", py.display()));
                let mut cmd = Command::new(py);
                cmd.args(&args);
                match run_streaming(&mut cmd, progress, "esptool") {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        if e.contains("9009") || e.to_ascii_lowercase().contains("microsoft store")
                        {
                            progress("Skipping Windows Store Python stub…".into());
                            continue;
                        }
                        py_err = e;
                        progress("esptool write failed — trying next…".into());
                    }
                }
            }
        }
    }

    Err(format!(
        "Flash failed (espflash: {esp_err}{}). Tip: hold BOOT, tap RESET, release BOOT, then Update again. Ensure Tools\\espflash.exe sits next to the app.",
        if py_err.is_empty() {
            String::new()
        } else {
            format!("; esptool: {py_err}")
        }
    ))
}

fn is_windows_store_python_stub(path: &Path) -> bool {
    let s = path.to_string_lossy().to_ascii_lowercase();
    s.contains("windowsapps") || s.contains("\\windowsapps\\")
}

/// Keep flash/update helper processes from popping console windows on Windows.
fn hide_console_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

fn run_streaming(
    cmd: &mut Command,
    progress: &dyn Fn(String),
    label: &str,
) -> Result<(), String> {
    hide_console_window(cmd);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("launch {label}: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let pump = |stream: Option<std::process::ChildStdout>, tx: Sender<String>| {
        if let Some(out) = stream {
            let reader = BufReader::new(out);
            for line in reader.lines().flatten() {
                let t = line.trim();
                if !t.is_empty() {
                    let _ = tx.send(t.to_string());
                }
            }
        }
    };
    let pump_err = |stream: Option<std::process::ChildStderr>, tx: Sender<String>| {
        if let Some(out) = stream {
            let reader = BufReader::new(out);
            for line in reader.lines().flatten() {
                let t = line.trim();
                if !t.is_empty() {
                    let _ = tx.send(t.to_string());
                }
            }
        }
    };

    let tx1 = tx.clone();
    let h1 = std::thread::spawn(move || pump(stdout, tx1));
    let tx2 = tx;
    let h2 = std::thread::spawn(move || pump_err(stderr, tx2));

    let mut tail = String::new();
    while let Ok(line) = rx.recv() {
        if line.contains('%') && line.len() < 8 {
            continue;
        }
        progress(line.clone());
        if tail.len() > 1200 {
            tail.clear();
        }
        if !tail.is_empty() {
            tail.push(' ');
        }
        tail.push_str(&line);
        if tail.len() > 400 {
            let keep = tail[tail.len() - 400..].to_string();
            tail = keep;
        }
    }
    let _ = h1.join();
    let _ = h2.join();

    let status = child
        .wait()
        .map_err(|e| format!("{label} wait: {e}"))?;
    if status.success() {
        progress("Flash write complete.".into());
        Ok(())
    } else {
        Err(format!(
            "{label} exited {} — {}",
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into()),
            if tail.is_empty() {
                "no output".into()
            } else {
                tail
            }
        ))
    }
}
