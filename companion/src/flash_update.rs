//! Push bundled (or nearby) firmware to the ESP32-2432S028 over USB serial.
//! Prefers bundled / auto-downloaded `espflash`; optional Python `esptool` fallback.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

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

/// Compare key for board vs kit: strip flavor suffixes so D0 boards match kit VERSION.txt.
/// `0.8.56-sha256-d0` and `0.8.56-sha256` are the same release line.
pub fn fw_version_key(raw: &str) -> String {
    let mut v = normalize_fw_version(raw);
    for suffix in ["-d0", "_d0"] {
        if let Some(stripped) = v.strip_suffix(suffix) {
            v = stripped.to_string();
            break;
        }
    }
    v
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
    let a = fw_version_key(board_fw);
    let b = fw_version_key(bundled);
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
    // Prefer commit-pinned raw for ~1MB bins — Contents API is flaky at the 1MB cap
    // and a long timeout on a stuck GET looks like “Fetching…” forever.
    for p in [
        "flash/downloads/esp32-2432s028-sha256-miner-merged.bin",
        "flash/downloads/esp32-2432s028-sha256-miner-d0-merged.bin",
        "flash/esp32-2432s028-sha256-miner-merged.bin",
    ] {
        out.extend(repo_bin_urls(p));
    }
    out
}

/// Large binaries: commit-pinned raw first, then branch raw (skip Contents API).
pub fn repo_bin_urls(path: &str) -> Vec<String> {
    let path = path.trim_start_matches('/');
    let mut out = Vec::new();
    for r in REPO_REFS {
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

pub const COMPANION_UA: &str = "Njordr-seas-CYD-miner/0.8.71";
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
            // jsDelivr is often fresher than raw branch CDN when the API is rate-limited.
            out.push(format!(
                "https://cdn.jsdelivr.net/gh/{REPO_OWNER}/{REPO_NAME}@{sha}/{path}"
            ));
        }
        out.push(format!(
            "https://raw.githubusercontent.com/{REPO_OWNER}/{REPO_NAME}/{r}/{path}"
        ));
        // Branch refs may contain `/` — do not percent-encode for jsDelivr.
        out.push(format!(
            "https://cdn.jsdelivr.net/gh/{REPO_OWNER}/{REPO_NAME}@{r}/{path}"
        ));
    }
    out
}

/// VERSION.txt only — skip branch-name raw CDN (can lag and falsely report "up to date").
pub fn repo_version_urls(path: &str) -> Vec<String> {
    let path = path.trim_start_matches('/');
    let mut out = Vec::new();
    for r in REPO_REFS {
        let enc = urlencode_ref(r);
        out.push(format!(
            "https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/contents/{path}?ref={enc}"
        ));
        if let Some(sha) = resolve_ref_commit(r) {
            out.push(format!(
                "https://raw.githubusercontent.com/{REPO_OWNER}/{REPO_NAME}/{sha}/{path}"
            ));
            out.push(format!(
                "https://cdn.jsdelivr.net/gh/{REPO_OWNER}/{REPO_NAME}@{sha}/{path}"
            ));
        }
        // Branch tip via jsDelivr (not GitHub branch raw CDN).
        out.push(format!(
            "https://cdn.jsdelivr.net/gh/{REPO_OWNER}/{REPO_NAME}@{r}/{path}"
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
    // Keep read timeout modest so a dead mirror fails fast and the next URL is tried.
    // Streaming downloads reset the idle timer on each chunk, so ~1MB bins still finish.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(45))
        .user_agent(COMPANION_UA)
        .build();
    let mut req = agent.get(url);
    // GitHub Contents API: ask for raw bytes (avoids base64 JSON + stale branch CDN).
    if url.contains("api.github.com/repos/") && url.contains("/contents/") {
        req = req.set("Accept", "application/vnd.github.raw");
    }
    progress("Connecting…".into());
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
    let mut last_report = 0u64;
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| format!("write: {e}"))?;
        total += n as u64;
        if total.saturating_sub(last_report) >= 128 * 1024 {
            last_report = total;
            progress(format!("Downloaded {} KB…", total / 1024));
        }
    }
    drop(file);
    if total < 1024 {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("download too small ({total} bytes) from {url}"));
    }
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

/// Per-attempt ceilings — espflash can hang forever waiting for serial sync / prompts.
const WRITE_TIMEOUT: Duration = Duration::from_secs(120);
const ERASE_TIMEOUT: Duration = Duration::from_secs(70);
const RESET_TIMEOUT: Duration = Duration::from_secs(12);
const ESPTOOL_TIMEOUT: Duration = Duration::from_secs(150);
/// Hard wall-clock budget for the whole Update board flash sequence.
const FLASH_BUDGET: Duration = Duration::from_secs(160);
/// No useful output at all → stuck before connect.
const IDLE_TIMEOUT: Duration = Duration::from_secs(28);
/// Chip MAC/connect seen but no write progress → stuck on prompt/sync.
const IDLE_AFTER_CONNECT: Duration = Duration::from_secs(18);
/// During active write (% lines), allow longer silence between ticks.
const IDLE_DURING_WRITE: Duration = Duration::from_secs(45);

fn flash_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.map(|c| c.load(Ordering::SeqCst)).unwrap_or(false)
}

/// Run an espflash subcommand, retrying skip-update-check flag forms for Windows clap quirks.
fn run_espflash_argv(
    espflash: &Path,
    sub_args: &[&str],
    progress: &dyn Fn(String),
    label: &str,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
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
        if flash_cancelled(cancel) {
            return Err("flash cancelled".into());
        }
        let mut cmd = Command::new(espflash);
        cmd.env_remove("ESPFLASH_SKIP_UPDATE_CHECK");
        if set_env_true {
            cmd.env("ESPFLASH_SKIP_UPDATE_CHECK", "true");
        }
        cmd.args(skip).args(sub_args);
        match run_streaming_timeout(&mut cmd, progress, label, timeout, cancel) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let lower = e.to_ascii_lowercase();
                last = e;
                // Only rotate skip-flag forms on CLI parse errors; port/flash/timeout stop here.
                if lower.contains("timed out")
                    || lower.contains("cancelled")
                    || lower.contains("idle")
                    || !(lower.contains("skip-update-check")
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

fn clamp_timeout(cap: Duration, budget: Duration) -> Duration {
    let t = cap.min(budget);
    if t < Duration::from_secs(5) {
        Duration::from_secs(5)
    } else {
        t
    }
}

fn run_espflash_erase(
    espflash: &Path,
    port: &str,
    baud: &str,
    progress: &dyn Fn(String),
    budget: Duration,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let timeout = clamp_timeout(ERASE_TIMEOUT, budget);
    progress(format!(
        "espflash erase-flash → {port} @ {baud} ({}) [timeout {}s]",
        espflash.display(),
        timeout.as_secs()
    ));
    // hard-reset after erase — do NOT use no-reset (soft_reset often fails on wiped flash;
    // write-bin is a new process so bootloader state cannot carry over).
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
            "hard-reset",
        ],
        progress,
        "espflash-erase",
        timeout,
        cancel,
    )
}

fn run_espflash_write(
    espflash: &Path,
    port: &str,
    baud: &str,
    before: &str,
    image: &Path,
    progress: &dyn Fn(String),
    budget: Duration,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let timeout = clamp_timeout(WRITE_TIMEOUT, budget);
    progress(format!(
        "espflash write-bin → {port} @ {baud} before={before} ({}) [timeout {}s]",
        espflash.display(),
        timeout.as_secs()
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
            "--before",
            before,
            "--after",
            "hard-reset",
            addr,
            img.as_ref(),
        ],
        progress,
        "espflash-write",
        timeout,
        cancel,
    )
}

fn run_espflash_reset(
    espflash: &Path,
    port: &str,
    progress: &dyn Fn(String),
    budget: Duration,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let timeout = clamp_timeout(RESET_TIMEOUT, budget);
    progress(format!("espflash reset → {port} [timeout {}s]", timeout.as_secs()));
    run_espflash_argv(
        espflash,
        &[
            "reset",
            "-p",
            port,
            "-c",
            "esp32",
            "--non-interactive",
            "--no-stub",
        ],
        progress,
        "espflash-reset",
        timeout,
        cancel,
    )
}

fn append_flash_log(line: &str) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let path = dir.join("last-flash.log");
    let stamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
    let entry = format!("[{stamp}] {line}\n");
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, entry.as_bytes()));
}

/// Flash merged image @ 0x0 (DIO / 4MB / 40MHz layout inside the merge).
///
/// Tries direct write with several reset/baud combos, then erase+write recovery.
pub fn flash_merged_bin(
    port: &str,
    image: &Path,
    progress: &dyn Fn(String),
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    use crate::workers::{flash_port_arg, is_usb_serial_port};

    if port.trim().is_empty() {
        return Err("Select a USB COM port before updating.".into());
    }
    if !is_usb_serial_port(port) {
        return Err(format!(
            "Cannot flash over '{port}' — pick a USB COM port (Wi‑Fi/TCP boards flash only via USB)."
        ));
    }
    if !image.is_file() {
        return Err(format!("Firmware missing: {}", image.display()));
    }

    let port_arg = flash_port_arg(port);
    // Fail fast with a clear list if Windows no longer sees the COM (common after unplug
    // or when Companion still held the handle a moment ago).
    {
        let listed: Vec<String> = serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.port_name)
            .collect();
        let want = crate::workers::normalize_port_name(&port_arg);
        let found = listed
            .iter()
            .any(|n| crate::workers::normalize_port_name(n) == want);
        if !found {
            let hint = if listed.is_empty() {
                "no serial ports listed".into()
            } else {
                listed.join(", ")
            };
            return Err(format!(
                "USB port '{port}' not found right now (looked for '{port_arg}'). \
Available: {hint}. Unplug/replug the CYD, pick the COM again, then Update board."
            ));
        }
    }
    progress(format!(
        "Flash {} ({} bytes) → {port} ({port_arg}) @ 0x0",
        image
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("firmware.bin"),
        std::fs::metadata(image).map(|m| m.len()).unwrap_or(0),
    ));
    progress(
        "Tip: if connect fails, hold BOOT, tap RESET, release BOOT, then retry (uses before=no-reset)."
            .into(),
    );
    append_flash_log(&format!(
        "begin port={port} arg={port_arg} image={}",
        image.display()
    ));

    let log_progress = |line: String| {
        append_flash_log(&line);
        progress(line);
    };
    let progress = &log_progress;

    let started = Instant::now();
    let budget_left = || FLASH_BUDGET.saturating_sub(started.elapsed());
    let ensure_budget = |progress: &dyn Fn(String)| -> Result<(), String> {
        if flash_cancelled(cancel) {
            return Err("flash cancelled".into());
        }
        let left = budget_left();
        if left.is_zero() {
            Err(format!(
                "Flash timed out after {}s overall — hold BOOT, tap RESET, release BOOT, then Update again.",
                FLASH_BUDGET.as_secs()
            ))
        } else {
            progress(format!(
                "Flash budget · {}s left",
                left.as_secs().max(1)
            ));
            Ok(())
        }
    };

    let espflash = ensure_espflash(progress)?;
    let mut esp_err = String::new();

    // Few short attempts — espflash often prints chip MAC then freezes on a prompt/sync.
    let attempts: &[(&str, &str)] = &[
        ("460800", "default-reset"),
        ("115200", "default-reset"),
        ("115200", "no-reset"),
    ];

    for &(baud, before) in attempts {
        ensure_budget(progress)?;
        progress(format!("Writing firmware @ {baud} (before={before})…"));
        match run_espflash_write(
            &espflash,
            &port_arg,
            baud,
            before,
            image,
            progress,
            budget_left(),
            cancel,
        ) {
            Ok(()) => {
                let _ = run_espflash_reset(&espflash, &port_arg, progress, budget_left(), cancel);
                append_flash_log("success write");
                return Ok(());
            }
            Err(e) => {
                esp_err = format!("write {baud}/{before}: {e}");
                progress(format!("espflash write failed: {esp_err}"));
                let low = e.to_ascii_lowercase();
                if low.contains("cancelled") {
                    return Err(e);
                }
                if low.contains("timed out") || low.contains("idle") {
                    progress(
                        "Stuck after chip connect — try BOOT+RESET, then no-reset…"
                            .into(),
                    );
                }
            }
        }
    }

    if budget_left() > Duration::from_secs(25) {
        ensure_budget(progress)?;
        progress("Direct write failed — trying full erase @ 115200, then write…".into());
        match run_espflash_erase(&espflash, &port_arg, "115200", progress, budget_left(), cancel) {
            Ok(()) => {
                progress("Erase done — waiting for COM port to settle…".into());
                std::thread::sleep(Duration::from_millis(800));
                ensure_budget(progress)?;
                progress("Writing after erase @ 115200 (before=default-reset)…".into());
                match run_espflash_write(
                    &espflash,
                    &port_arg,
                    "115200",
                    "default-reset",
                    image,
                    progress,
                    budget_left(),
                    cancel,
                ) {
                    Ok(()) => {
                        let _ =
                            run_espflash_reset(&espflash, &port_arg, progress, budget_left(), cancel);
                        append_flash_log("success erase+write");
                        return Ok(());
                    }
                    Err(e) => {
                        esp_err = format!("write after erase 115200/default-reset: {e}");
                        progress(format!("espflash write failed: {esp_err}"));
                        if e.to_ascii_lowercase().contains("cancelled") {
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) => {
                esp_err = format!("erase 115200: {e}");
                progress(format!("espflash erase failed: {esp_err}"));
                if e.to_ascii_lowercase().contains("cancelled") {
                    return Err(e);
                }
            }
        }
    }

    // Optional Python fallback — one timed attempt only.
    let mut py_err = String::new();
    let py_bins: Vec<PathBuf> = ["py", "python", "python3"]
        .iter()
        .filter_map(|n| which_on_path(n))
        .filter(|p| !is_windows_store_python_stub(p))
        .collect();
    if !py_bins.is_empty() && budget_left() > Duration::from_secs(20) {
        progress("espflash failed — trying Python esptool (one timed attempt)…".into());
        if let Some(py) = py_bins.first() {
            let py_launcher = py
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("py") || s.eq_ignore_ascii_case("py.exe"))
                .unwrap_or(false);

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
                port_arg.clone(),
                "--baud".into(),
                "115200".into(),
                "write_flash".into(),
                "--erase-all".into(),
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
            let py_timeout = clamp_timeout(ESPTOOL_TIMEOUT, budget_left());
            progress(format!(
                "esptool write_flash --erase-all via {} @ 115200 [timeout {}s]…",
                py.display(),
                py_timeout.as_secs()
            ));
            let mut cmd = Command::new(py);
            cmd.args(&args);
            match run_streaming_timeout(&mut cmd, progress, "esptool", py_timeout, cancel) {
                Ok(()) => {
                    append_flash_log("success esptool");
                    return Ok(());
                }
                Err(e) => {
                    if !(e.contains("9009") || e.to_ascii_lowercase().contains("microsoft store")) {
                        py_err = e.clone();
                        progress(format!("esptool failed: {e}"));
                    }
                }
            }
        }
    }

    let tip = format!(
        "Flash failed after {}s (espflash: {esp_err}{}). \
See last-flash.log next to the app. \
Hold BOOT, tap RESET, release BOOT, then Update again. \
Or run Flash-Firmware.bat and enter the COM number. \
Ensure Tools\\espflash.exe sits next to the app.",
        started.elapsed().as_secs(),
        if py_err.is_empty() {
            String::new()
        } else {
            format!("; esptool: {py_err}")
        }
    );
    append_flash_log(&tip);
    Err(tip)
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
        // CREATE_NO_WINDOW keeps the child killable and avoids a console flash.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

fn force_kill(child: &mut Child) {
    let pid = child.id();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // Kill the whole tree — espflash can leave helpers stuck on the COM port.
        let mut killer = Command::new("taskkill");
        killer
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let _ = killer.status();
    }
    #[cfg(unix)]
    {
        unsafe {
            extern "C" {
                fn killpg(pgrp: i32, sig: i32) -> i32;
            }
            const SIGKILL: i32 = 9;
            let _ = killpg(pid as i32, SIGKILL);
        }
    }
    let _ = child.kill();
    for _ in 0..30 {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => break,
        }
    }
    let _ = child.try_wait();
}

fn join_pumps_brief(
    h1: std::thread::JoinHandle<()>,
    h2: std::thread::JoinHandle<()>,
    max_wait: Duration,
) {
    let (tx, rx) = mpsc::channel();
    let tx2 = tx.clone();
    std::thread::spawn(move || {
        let _ = h1.join();
        let _ = tx.send(());
    });
    std::thread::spawn(move || {
        let _ = h2.join();
        let _ = tx2.send(());
    });
    let deadline = Instant::now() + max_wait;
    let mut got = 0u8;
    while got < 2 {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match rx.recv_timeout(left) {
            Ok(()) => got += 1,
            Err(_) => break,
        }
    }
}

fn line_looks_connected(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    l.contains("mac")
        || l.contains("connected")
        || l.contains("chip type")
        || l.contains("chip is")
        || l.contains("stub")
        || l.contains("uploading")
        || l.contains("writing at")
        || l.contains("flash size")
        || l.contains("crystal is")
}

/// Run a flash helper with wall-clock + idle timeouts. Kills the child on expiry
/// so Update board cannot hang forever after espflash prints the chip MAC.
fn run_streaming_timeout(
    cmd: &mut Command,
    progress: &dyn Fn(String),
    label: &str,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    hide_console_window(cmd);
    // Critical: close stdin. espflash can print MAC then wait forever on a prompt
    // when stdin is an inherited console / pipe that never answers.
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                extern "C" {
                    fn setpgid(pid: i32, pgid: i32) -> i32;
                }
                let _ = setpgid(0, 0);
                Ok(())
            });
        }
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("launch {label}: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let (tx, rx) = mpsc::channel::<String>();
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

    let deadline = Instant::now() + timeout;
    let mut tail = String::new();
    let mut last_beat = Instant::now();
    let mut last_status = Instant::now();
    let mut saw_connect = false;
    let mut saw_write = false;
    let abort = |reason: String,
                 child: &mut Child,
                 h1: std::thread::JoinHandle<()>,
                 h2: std::thread::JoinHandle<()>,
                 progress: &dyn Fn(String),
                 tail: &str|
     -> Result<(), String> {
        progress(reason.clone());
        force_kill(child);
        join_pumps_brief(h1, h2, Duration::from_secs(2));
        Err(format!(
            "{reason} — {}",
            if tail.is_empty() {
                "no output (board never entered download mode?)".into()
            } else {
                trunc_tail(tail)
            }
        ))
    };

    loop {
        if flash_cancelled(cancel) {
            return abort(
                format!("{label} cancelled"),
                &mut child,
                h1,
                h2,
                progress,
                &tail,
            );
        }
        let wait = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(350));
        if wait.is_zero() {
            return abort(
                format!("{label} timed out after {}s — killing stuck flash tool", timeout.as_secs()),
                &mut child,
                h1,
                h2,
                progress,
                &tail,
            );
        }
        let idle_limit = if saw_write {
            IDLE_DURING_WRITE
        } else if saw_connect {
            IDLE_AFTER_CONNECT
        } else {
            IDLE_TIMEOUT
        };
        if last_beat.elapsed() >= idle_limit {
            return abort(
                format!(
                    "{label} idle {}s after {} — killing stuck flash tool",
                    idle_limit.as_secs(),
                    if saw_write {
                        "write progress"
                    } else if saw_connect {
                        "chip connect/MAC"
                    } else {
                        "start"
                    }
                ),
                &mut child,
                h1,
                h2,
                progress,
                &tail,
            );
        }
        match rx.recv_timeout(wait) {
            Ok(line) => {
                last_beat = Instant::now();
                let lower = line.to_ascii_lowercase();
                if line_looks_connected(&line) {
                    if !saw_connect {
                        saw_connect = true;
                        progress(format!(
                            "{label}: chip seen — need write progress within {}s (or will abort)…",
                            IDLE_AFTER_CONNECT.as_secs()
                        ));
                    }
                }
                if lower.contains('%')
                    || lower.contains("writing")
                    || lower.contains("erasing")
                    || lower.contains("compressed")
                    || lower.contains("hash of data")
                {
                    saw_write = true;
                }
                // Tiny percent ticks still count as activity (already updated last_beat).
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
            Err(RecvTimeoutError::Timeout) => match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if last_status.elapsed() >= Duration::from_secs(5) {
                        progress(format!(
                            "{label} still running… {}s left{}",
                            deadline.saturating_duration_since(Instant::now()).as_secs(),
                            if saw_write {
                                " (writing)"
                            } else if saw_connect {
                                " (after MAC/connect)"
                            } else {
                                ""
                            }
                        ));
                        last_status = Instant::now();
                    }
                }
                Err(e) => {
                    return abort(
                        format!("{label} wait: {e}"),
                        &mut child,
                        h1,
                        h2,
                        progress,
                        &tail,
                    );
                }
            },
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    join_pumps_brief(h1, h2, Duration::from_secs(5));

    let status = match child.try_wait() {
        Ok(Some(s)) => s,
        Ok(None) => {
            force_kill(&mut child);
            return Err(format!(
                "{label} did not exit cleanly after output closed — killed"
            ));
        }
        Err(e) => return Err(format!("{label} wait: {e}")),
    };
    if status.success() {
        progress(format!("{label} complete."));
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
                trunc_tail(&tail)
            }
        ))
    }
}

fn trunc_tail(tail: &str) -> String {
    if tail.len() <= 400 {
        tail.to_string()
    } else {
        tail[tail.len() - 400..].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{fw_version_key, update_needed};

    #[test]
    fn d0_board_matches_kit_without_d0_suffix() {
        assert_eq!(
            fw_version_key("0.8.56-sha256-d0"),
            fw_version_key("0.8.56-sha256")
        );
        assert_eq!(
            update_needed("0.8.56-sha256-d0", "0.8.56-sha256"),
            Some(false)
        );
        assert_eq!(
            update_needed("0.8.56-sha256-d0", "0.8.71-sha256"),
            Some(true)
        );
        assert_eq!(update_needed("", "0.8.71-sha256"), None);
    }
}
