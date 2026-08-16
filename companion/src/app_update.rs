//! Companion self-update — fetch latest Windows app build from the repo downloads.
//! Board firmware fetch stays in `flash_update`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::flash_update::{
    normalize_fw_version, urlencode_ref, COMPANION_UA, REPO_NAME, REPO_OWNER, REPO_REFS, TIP_REF,
};

#[derive(Clone, Debug)]
pub struct AppRemoteInfo {
    pub version: String,
    /// True when remote version is newer than this running build.
    pub newer: bool,
    pub detail: String,
}

pub fn running_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn version_tuple(raw: &str) -> (u32, u32, u32) {
    let v = normalize_fw_version(raw);
    let v = v.trim_end_matches("-sha256");
    let mut parts = v.split('.');
    let major = parts
        .next()
        .and_then(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
        .unwrap_or(0);
    let minor = parts
        .next()
        .and_then(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
        .unwrap_or(0);
    let patch = parts
        .next()
        .and_then(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
        .unwrap_or(0);
    (major, minor, patch)
}

pub fn is_newer(remote: &str, local: &str) -> bool {
    version_tuple(remote) > version_tuple(local)
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.map(|c| c.load(Ordering::SeqCst)).unwrap_or(false)
}

fn ensure_not_cancelled(cancel: Option<&AtomicBool>) -> Result<(), String> {
    if cancelled(cancel) {
        Err("App update cancelled".into())
    } else {
        Ok(())
    }
}

fn version_urls() -> Vec<String> {
    // Prefer Contents API (always tip). Probe every REPO_REFS tip so a lagging
    // legacy branch (e.g. stuck at 0.8.102) cannot hide a newer release.
    let mut out = Vec::new();
    for r in REPO_REFS {
        let enc = urlencode_ref(r);
        out.push(format!(
            "https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/contents/flash/downloads/VERSION.txt?ref={enc}"
        ));
        out.push(format!(
            "https://raw.githubusercontent.com/{REPO_OWNER}/{REPO_NAME}/{r}/flash/downloads/VERSION.txt"
        ));
        out.push(format!(
            "https://cdn.jsdelivr.net/gh/{REPO_OWNER}/{REPO_NAME}@{r}/flash/downloads/VERSION.txt"
        ));
    }
    out
}

fn app_zip_urls() -> Vec<String> {
    // Large packages: commit-pinned / branch raw only (skip jsDelivr branch tip).
    let mut out = crate::flash_update::repo_bin_urls("flash/downloads/CYD-Companion-App-Only.zip");
    out.extend(crate::flash_update::repo_bin_urls(
        "flash/downloads/CYD-Miner-Portable.zip",
    ));
    // Fall back to the broader list (API / pinned CDN) if raw mirrors fail.
    out.extend(prefer_fresh_download_urls(crate::flash_update::repo_file_urls(
        "flash/downloads/CYD-Companion-App-Only.zip",
    )));
    out.extend(prefer_fresh_download_urls(crate::flash_update::repo_file_urls(
        "flash/downloads/CYD-Miner-Portable.zip",
    )));
    // Cap attempts — each failed mirror used to burn up to 300s and never stop.
    let mut out = dedupe_urls(out);
    out.truncate(6);
    out
}

fn app_exe_urls() -> Vec<String> {
    let mut out = crate::flash_update::repo_bin_urls("flash/downloads/cyd-companion.exe");
    out.extend(prefer_fresh_download_urls(crate::flash_update::repo_file_urls(
        "flash/downloads/cyd-companion.exe",
    )));
    let mut out = dedupe_urls(out);
    out.truncate(4);
    out
}

/// Push unpinned branch-tip jsDelivr URLs last — that CDN often lags tip by several releases.
fn prefer_fresh_download_urls(urls: Vec<String>) -> Vec<String> {
    let mut fresh = Vec::with_capacity(urls.len());
    let mut stale = Vec::new();
    for u in urls {
        let branch_jsdelivr = u.contains("cdn.jsdelivr.net/gh/") && u.contains("@cursor/");
        if branch_jsdelivr {
            stale.push(u);
        } else {
            fresh.push(u);
        }
    }
    fresh.extend(stale);
    fresh
}

fn dedupe_urls(urls: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(urls.len());
    for u in urls {
        if seen.insert(u.clone()) {
            out.push(u);
        }
    }
    out
}

fn sha256sums_urls() -> Vec<String> {
    // Contents API first so verify hashes match tip packages, not a cached branch raw.
    let mut out = Vec::new();
    for r in REPO_REFS {
        let enc = urlencode_ref(r);
        out.push(format!(
            "https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/contents/flash/downloads/SHA256SUMS.txt?ref={enc}"
        ));
        out.push(format!(
            "https://raw.githubusercontent.com/{REPO_OWNER}/{REPO_NAME}/{r}/flash/downloads/SHA256SUMS.txt"
        ));
        out.push(format!(
            "https://cdn.jsdelivr.net/gh/{REPO_OWNER}/{REPO_NAME}@{r}/flash/downloads/SHA256SUMS.txt"
        ));
    }
    let mut out = dedupe_urls(out);
    out.truncate(6);
    out
}

/// Best-effort version from a SHA256SUMS header comment (`# … 0.8.78 — …`).
fn sums_header_version(txt: &str) -> Option<String> {
    for line in txt.lines().take(4) {
        if let Some(v) = parse_version_text(line) {
            return Some(v);
        }
        // Header often embeds the version mid-line without a clean lone token.
        for token in line.split(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-') {
            if token.matches('.').count() >= 2 {
                if let Some(v) = parse_version_text(token) {
                    return Some(v);
                }
            }
        }
    }
    None
}

fn parse_sha256sums(txt: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in txt.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        // "hex  filename" or "hex *filename"
        let mut parts = t.split_whitespace();
        let Some(hex) = parts.next() else { continue };
        let Some(name) = parts.next() else { continue };
        if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let name = name.trim_start_matches('*');
        let base = name.rsplit('/').next().unwrap_or(name);
        map.insert(base.to_ascii_lowercase(), hex.to_ascii_lowercase());
    }
    map
}

fn file_sha256_hex(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn fetch_download_checksums(
    progress: &dyn Fn(String),
    cancel: Option<&AtomicBool>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let mut last = String::new();
    let mut best: Option<(String, std::collections::HashMap<String, String>)> = None;
    for url in sha256sums_urls() {
        ensure_not_cancelled(cancel)?;
        progress(format!("GET checksums {url}"));
        match http_get_text(&url) {
            Ok(txt) => {
                let map = parse_sha256sums(&txt);
                if map.is_empty() {
                    last = "SHA256SUMS.txt had no usable entries".into();
                    continue;
                }
                let ver = sums_header_version(&txt).unwrap_or_else(|| "0.0.0".into());
                best = match best.take() {
                    None => Some((ver, map)),
                    Some((prev_ver, _)) if is_newer(&ver, &prev_ver) => Some((ver, map)),
                    Some(prev) => Some(prev),
                };
                // Tip Contents API is authoritative — stop after first good tip hit.
                if url.contains("api.github.com")
                    && url.contains(TIP_REF)
                    && best.is_some()
                {
                    break;
                }
            }
            Err(e) => last = e,
        }
    }
    best.map(|(_, map)| map)
        .ok_or_else(|| format!("Could not fetch SHA256SUMS ({last})"))
}

fn verify_named_file(
    path: &Path,
    logical_name: &str,
    sums: &std::collections::HashMap<String, String>,
    progress: &dyn Fn(String),
    required: bool,
) -> Result<(), String> {
    let key = logical_name.to_ascii_lowercase();
    let Some(expected) = sums.get(&key) else {
        if required {
            return Err(format!("No SHA-256 entry for {logical_name} in SUMS"));
        }
        progress(format!(
            "No SHA-256 entry for {logical_name} in SUMS — skipping hash check for this file"
        ));
        return Ok(());
    };
    let actual = file_sha256_hex(path)?;
    if actual != *expected {
        return Err(format!(
            "SHA-256 mismatch for {logical_name}: expected {expected}, got {actual}"
        ));
    }
    progress(format!("Verified {logical_name} SHA-256"));
    Ok(())
}

fn http_get_text(url: &str) -> Result<String, String> {
    http_get_text_timeout(url, 4, 8)
}

fn http_get_text_timeout(url: &str, connect_s: u64, read_s: u64) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(connect_s))
        .timeout_read(std::time::Duration::from_secs(read_s))
        .user_agent(COMPANION_UA)
        .build();
    let mut req = agent.get(url);
    if url.contains("api.github.com/repos/") && url.contains("/contents/") {
        req = req.set("Accept", "application/vnd.github.raw");
    }
    // Discourage intermediary caches from serving a days-old VERSION.txt.
    req = req.set("Cache-Control", "no-cache");
    let resp = req.call().map_err(|e| format!("http: {e}"))?;
    if !(200..300).contains(&resp.status()) {
        return Err(format!("http {} for {url}", resp.status()));
    }
    resp.into_string().map_err(|e| format!("read: {e}"))
}

fn http_download(
    url: &str,
    dest: &Path,
    progress: &dyn Fn(String),
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    ensure_not_cancelled(cancel)?;
    // Fail fast — do not sit 5 minutes per dead mirror.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(90))
        .user_agent(COMPANION_UA)
        .build();
    let mut req = agent.get(url);
    if url.contains("api.github.com/repos/") && url.contains("/contents/") {
        req = req.set("Accept", "application/vnd.github.raw");
    }
    let resp = req.call().map_err(|e| format!("http: {e}"))?;
    if !(200..300).contains(&resp.status()) {
        return Err(format!("http {} for {url}", resp.status()));
    }
    let mut reader = resp.into_reader();
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("create: {e}"))?;
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    let started = Instant::now();
    loop {
        ensure_not_cancelled(cancel)?;
        if started.elapsed() > Duration::from_secs(120) {
            let _ = std::fs::remove_file(&tmp);
            return Err("download timed out after 120s — try Update app again".into());
        }
        let n = std::io::Read::read(&mut reader, &mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| format!("write: {e}"))?;
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

fn parse_version_text(txt: &str) -> Option<String> {
    for line in txt.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let raw = if let Some(rest) = t.split(':').nth(1) {
            rest.trim()
        } else {
            t
        };
        let v = normalize_fw_version(raw)
            .trim_end_matches("-sha256")
            .to_string();
        if version_tuple(&v) != (0, 0, 0) || v.contains('.') {
            return Some(v);
        }
    }
    None
}

/// Check repo VERSION.txt against this running Companion build.
pub fn check_app_update(progress: &dyn Fn(String)) -> Result<AppRemoteInfo, String> {
    check_app_update_ex(progress, None)
}

pub fn check_app_update_ex(
    progress: &dyn Fn(String),
    cancel: Option<&AtomicBool>,
) -> Result<AppRemoteInfo, String> {
    let local = running_version();
    let mut last = String::new();
    let mut best_remote: Option<String> = None;
    let mut sources_ok = 0u32;
    progress(format!("Checking for Companion updates (running {local})…"));
    // Probe tip first; keep newest, but stop once tip API + one raw agree.
    let mut tip_hits = 0u32;
    for url in version_urls() {
        ensure_not_cancelled(cancel)?;
        match http_get_text(&url) {
            Ok(txt) => {
                if let Some(remote) = parse_version_text(&txt) {
                    sources_ok += 1;
                    progress(format!("Remote VERSION {remote}"));
                    best_remote = match best_remote.take() {
                        None => Some(remote.clone()),
                        Some(prev) if is_newer(&remote, &prev) => Some(remote.clone()),
                        Some(prev) => Some(prev),
                    };
                    if url.contains(TIP_REF) {
                        tip_hits += 1;
                        if tip_hits >= 2 {
                            break;
                        }
                    }
                } else {
                    last = "VERSION.txt had no usable version".into();
                }
            }
            Err(e) => last = e,
        }
        // Hard cap so Check never walks every mirror for minutes.
        if sources_ok >= 4 {
            break;
        }
    }
    let Some(remote) = best_remote else {
        return Err(format!("Could not check for app updates ({last})"));
    };
    let newer = is_newer(&remote, local);
    Ok(AppRemoteInfo {
        version: remote.clone(),
        newer,
        detail: if newer {
            format!("Update available · {local} → {remote} ({sources_ok} source(s))")
        } else {
            format!("App is up to date · {local} (remote {remote})")
        },
    })
}

/// Staging folder under the install dir — updater bat promotes this after a clean sweep.
const STAGING_DIR_NAME: &str = "_update_staging";

fn install_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "no install directory".into())
}

fn name_eq_ignore(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Remove everything in `dir` except basenames listed in `keep` (case-insensitive).
fn clean_dir_contents(dir: &Path, keep: &[&str]) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if keep.iter().any(|k| name_eq_ignore(&name, k)) {
            continue;
        }
        let path = entry.path();
        let meta = entry.metadata().map_err(|e| format!("metadata {}: {e}", path.display()))?;
        if meta.is_dir() {
            std::fs::remove_dir_all(&path)
                .map_err(|e| format!("remove_dir {}: {e}", path.display()))?;
        } else {
            std::fs::remove_file(&path)
                .map_err(|e| format!("remove_file {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

fn prepare_staging(install: &Path, progress: &dyn Fn(String)) -> Result<PathBuf, String> {
    let staging = install.join(STAGING_DIR_NAME);
    if staging.exists() {
        progress("Clearing previous update staging…".into());
        std::fs::remove_dir_all(&staging)
            .map_err(|e| format!("remove staging: {e}"))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("mkdir staging: {e}"))?;
    Ok(staging)
}

fn kit_relative_path(name: &str) -> Option<String> {
    let name = name.replace('\\', "/");
    let lower = name.to_ascii_lowercase();
    let file_name = name.rsplit('/').next().unwrap_or("");

    let rel = if let Some(idx) = lower.find("cyd-companion-app-only/") {
        name[idx + "cyd-companion-app-only/".len()..].to_string()
    } else if let Some(idx) = lower.find("cyd-companion-windows/") {
        name[idx + "cyd-companion-windows/".len()..].to_string()
    } else if let Some(idx) = lower.find("cyd-miner-kit/") {
        name[idx + "cyd-miner-kit/".len()..].to_string()
    } else if file_name.eq_ignore_ascii_case("cyd-companion.exe") {
        file_name.to_string()
    } else {
        return None;
    };

    let rel = rel.trim_start_matches('/').to_string();
    if rel.is_empty() || rel.ends_with('/') {
        return None;
    }
    // Block path traversal from malicious zips.
    if rel.split('/').any(|p| p == ".." || p.is_empty()) {
        return None;
    }
    Some(rel)
}

fn extract_app_kit(zip_path: &Path, dest_dir: &Path, progress: &dyn Fn(String)) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip: {e}"))?;
    let mut wrote_exe = false;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel_path) = kit_relative_path(entry.name()) else {
            continue;
        };

        // Staging dest is empty — write the real exe name (bat promotes after clean sweep).
        let out_path = dest_dir.join(&rel_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
        progress(format!("Extracting {rel_path}…"));
        let mut out = std::fs::File::create(&out_path).map_err(|e| format!("create: {e}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("extract: {e}"))?;
        if rel_path.eq_ignore_ascii_case("cyd-companion.exe")
            || Path::new(&rel_path)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("cyd-companion.exe"))
                .unwrap_or(false)
        {
            wrote_exe = true;
        }
    }

    if !wrote_exe {
        return Err("cyd-companion.exe not found inside update zip".into());
    }
    Ok(())
}

#[cfg(windows)]
fn schedule_windows_replace_and_restart(install: &Path) -> Result<(), String> {
    let exe_name = "cyd-companion.exe";
    let staging = STAGING_DIR_NAME;
    let bat_path = install.join("cyd-companion-update.bat");
    // No `timeout /nobreak` — that flashes a console and loops when stdin is redirected.
    // Short ping delays only; fail after ~12s if the old exe stays locked.
    let bat = format!(
        "@echo off\r\n\
         setlocal EnableExtensions\r\n\
         cd /d \"{dir}\"\r\n\
         ping -n 2 127.0.0.1 >nul\r\n\
         set /a tries=0\r\n\
         :wait_unlock\r\n\
         set /a tries+=1\r\n\
         if exist \"{exe}\" del /f /q \"{exe}\" >nul 2>nul\r\n\
         if exist \"{exe}\" (\r\n\
           if %tries% geq 12 exit /b 1\r\n\
           ping -n 2 127.0.0.1 >nul\r\n\
           goto wait_unlock\r\n\
         )\r\n\
         REM Clean sweep — remove stale files/dirs left by older builds\r\n\
         for /d %%D in (*) do (\r\n\
           if /i not \"%%~nxD\"==\"{staging}\" rd /s /q \"%%D\" 2>nul\r\n\
         )\r\n\
         for %%F in (*) do (\r\n\
           if /i not \"%%~nxF\"==\"cyd-companion-update.bat\" if /i not \"%%~nxF\"==\"Uninstall.exe\" if /i not \"%%~nxF\"==\"{staging}\" del /f /q \"%%F\" 2>nul\r\n\
         )\r\n\
         if not exist \"{staging}\\{exe}\" exit /b 1\r\n\
         xcopy /e /y /i \"{staging}\\*\" \".\\\" >nul\r\n\
         if errorlevel 1 (\r\n\
           robocopy \"{staging}\" \".\" /E /NFL /NDL /NJH /NJS /NC /NS >nul\r\n\
           if errorlevel 8 exit /b 1\r\n\
         )\r\n\
         rd /s /q \"{staging}\" 2>nul\r\n\
         if not exist \"{exe}\" exit /b 1\r\n\
         start \"\" \"{exe}\"\r\n\
         del \"%~f0\"\r\n",
        dir = install.display(),
        exe = exe_name,
        staging = staging,
    );
    std::fs::write(&bat_path, bat).map_err(|e| format!("write updater bat: {e}"))?;

    // Detached + no console so the updater bat never flashes a terminal window.
    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/C", &bat_path.to_string_lossy()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
    cmd.spawn()
        .map_err(|e| format!("launch updater: {e}"))?;
    Ok(())
}

#[cfg(not(windows))]
fn schedule_windows_replace_and_restart(_install: &Path) -> Result<(), String> {
    Err("App self-update replace is only automated on Windows".into())
}

/// Download latest Companion build into the install folder and restart (Windows).
///
/// Updates stage into `_update_staging/`, then a helper bat clean-sweeps the install
/// directory (drops stale files from older kits) before promoting the new tree.
///
/// Pass `cancel` so the UI Cancel button can stop mirror retries immediately.
pub fn update_companion_app(progress: &dyn Fn(String)) -> Result<AppRemoteInfo, String> {
    update_companion_app_ex(progress, None)
}

pub fn update_companion_app_ex(
    progress: &dyn Fn(String),
    cancel: Option<&AtomicBool>,
) -> Result<AppRemoteInfo, String> {
    let local = running_version();
    let info = check_app_update_ex(progress, cancel)?;
    if !info.newer {
        progress(info.detail.clone());
        return Ok(info);
    }

    let install = install_dir()?;
    progress(format!(
        "Updating Companion {local} → {} (clean sweep) in {}",
        info.version,
        install.display()
    ));

    let staging = prepare_staging(&install, progress)?;

    let sums = fetch_download_checksums(progress, cancel)?;
    let has_companion_sum = ["cyd-companion.exe", "cyd-companion-app-only.zip", "cyd-miner-portable.zip"]
        .iter()
        .any(|n| sums.contains_key(*n));
    if !has_companion_sum {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(
            "SHA256SUMS.txt has no Companion package hashes — refusing unverified update".into(),
        );
    }

    // Prefer full App-Only zip (exe + Firmware + Tools).
    let zip_path = install.join("companion-update.zip");
    let mut zip_ok = false;
    let mut last = String::new();
    let mut used_zip_name = String::new();
    let mut hard_fails = 0u32;
    for url in app_zip_urls() {
        ensure_not_cancelled(cancel)?;
        progress(format!("GET {url}"));
        let zip_name = if url.to_ascii_lowercase().contains("portable") {
            "CYD-Miner-Portable.zip"
        } else {
            "CYD-Companion-App-Only.zip"
        };
        match http_download(&url, &zip_path, progress, cancel) {
            Ok(()) => {
                if let Err(e) = verify_named_file(&zip_path, zip_name, &sums, progress, true) {
                    last = e;
                    let _ = std::fs::remove_file(&zip_path);
                    hard_fails += 1;
                    if hard_fails >= 3 {
                        break;
                    }
                    continue;
                }
                match extract_app_kit(&zip_path, &staging, progress) {
                    Ok(()) => {
                        zip_ok = true;
                        used_zip_name = zip_name.into();
                        let _ = std::fs::remove_file(&zip_path);
                        break;
                    }
                    Err(e) => {
                        last = e;
                        let _ = std::fs::remove_file(&zip_path);
                        let _ = clean_dir_contents(&staging, &[]);
                        hard_fails += 1;
                        if hard_fails >= 3 {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                last = e;
                hard_fails += 1;
                if hard_fails >= 4 {
                    progress("Stopping zip mirrors after repeated timeouts…".into());
                    break;
                }
            }
        }
    }

    if !zip_ok {
        ensure_not_cancelled(cancel)?;
        progress("Zip update failed — trying bare cyd-companion.exe…".into());
        let _ = clean_dir_contents(&staging, &[]);
        let new_exe = staging.join("cyd-companion.exe");
        let mut exe_ok = false;
        hard_fails = 0;
        for url in app_exe_urls() {
            ensure_not_cancelled(cancel)?;
            progress(format!("GET {url}"));
            match http_download(&url, &new_exe, progress, cancel) {
                Ok(()) => {
                    let bytes = std::fs::metadata(&new_exe).map(|m| m.len()).unwrap_or(0);
                    if bytes <= 1_000_000 {
                        last = format!("exe too small ({bytes} bytes)");
                        let _ = std::fs::remove_file(&new_exe);
                        continue;
                    }
                    if let Err(e) =
                        verify_named_file(&new_exe, "cyd-companion.exe", &sums, progress, true)
                    {
                        last = e;
                        let _ = std::fs::remove_file(&new_exe);
                        hard_fails += 1;
                        if hard_fails >= 3 {
                            break;
                        }
                        continue;
                    }
                    exe_ok = true;
                    break;
                }
                Err(e) => {
                    last = e;
                    hard_fails += 1;
                    if hard_fails >= 3 {
                        break;
                    }
                }
            }
        }
        if !exe_ok {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("Could not download Companion update ({last})"));
        }
    } else {
        progress(format!("Extracted {used_zip_name} into staging"));
    }

    // Write a VERSION marker next to the staged exe for future checks.
    let _ = std::fs::write(
        staging.join("VERSION.txt"),
        format!("{}-sha256\n", info.version),
    );

    ensure_not_cancelled(cancel)?;
    progress(format!(
        "Companion {} staged — clean-sweeping install dir and restarting…",
        info.version
    ));

    #[cfg(windows)]
    {
        progress("Clean-sweeping install directory…".into());
        schedule_windows_replace_and_restart(&install)?;
        Ok(AppRemoteInfo {
            version: info.version.clone(),
            newer: true,
            detail: format!(
                "Companion {} staged — restarting…",
                info.version
            ),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = staging;
        Err("App self-update is only automated on Windows — copy the new build manually".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare_numeric() {
        assert!(is_newer("0.8.116", "0.8.115"));
        assert!(!is_newer("0.8.115", "0.8.116"));
        assert!(is_newer("0.8.116-sha256", "0.8.115"));
    }

    #[test]
    fn sums_header_parses_embedded_version() {
        let txt = "# Njörðr Seas' CYD miner 0.8.78 — verify with: sha256sum -c SHA256SUMS.txt\n";
        assert_eq!(sums_header_version(txt).as_deref(), Some("0.8.78"));
    }

    #[test]
    fn parse_version_lines() {
        assert_eq!(
            parse_version_text("0.8.23-sha256\n").as_deref(),
            Some("0.8.23")
        );
        assert_eq!(
            parse_version_text("# comment\nfw: 0.8.23-sha256\n").as_deref(),
            Some("0.8.23")
        );
    }

    #[test]
    fn kit_paths() {
        assert_eq!(
            kit_relative_path("cyd-companion-app-only/cyd-companion.exe").as_deref(),
            Some("cyd-companion.exe")
        );
        assert_eq!(
            kit_relative_path("cyd-miner-kit/Tools/espflash.exe").as_deref(),
            Some("Tools/espflash.exe")
        );
    }

    #[test]
    fn prefer_fresh_orders_jsdelivr_last() {
        let urls = prefer_fresh_download_urls(vec![
            "https://cdn.jsdelivr.net/gh/GutFarms/Japan-central@cursor/esp32-mesh-connectivity-e801/flash/downloads/VERSION.txt".into(),
            "https://raw.githubusercontent.com/GutFarms/Japan-central/cursor/esp32-mesh-connectivity-e801/flash/downloads/VERSION.txt".into(),
            "https://cdn.jsdelivr.net/gh/GutFarms/Japan-central@deadbeef/flash/downloads/VERSION.txt".into(),
        ]);
        assert!(urls[0].contains("raw.githubusercontent.com"));
    }
}
