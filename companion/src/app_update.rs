//! Companion self-update — fetch latest Windows app build from the repo downloads.
//! Board firmware fetch stays in `flash_update`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::flash_update::{normalize_fw_version, repo_file_urls, COMPANION_UA};

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

fn version_urls() -> Vec<String> {
    let mut out = repo_file_urls("flash/downloads/VERSION.txt");
    out.extend(repo_file_urls("flash/VERSION.txt"));
    out
}

fn app_zip_urls() -> Vec<String> {
    let mut out = repo_file_urls("flash/downloads/CYD-Companion-App-Only.zip");
    out.extend(repo_file_urls("flash/downloads/CYD-Miner-Portable.zip"));
    out
}

fn app_exe_urls() -> Vec<String> {
    repo_file_urls("flash/downloads/cyd-companion.exe")
}

fn http_get_text(url: &str) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(20))
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
    resp.into_string().map_err(|e| format!("read: {e}"))
}

fn http_download(url: &str, dest: &Path, progress: &dyn Fn(String)) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(300))
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
    loop {
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
    let local = running_version();
    let mut last = String::new();
    progress(format!("Checking for Companion updates (running {local})…"));
    for url in version_urls() {
        match http_get_text(&url) {
            Ok(txt) => {
                if let Some(remote) = parse_version_text(&txt) {
                    let newer = is_newer(&remote, local);
                    return Ok(AppRemoteInfo {
                        version: remote.clone(),
                        newer,
                        detail: if newer {
                            format!("Update available · {local} → {remote}")
                        } else {
                            format!("App is up to date · {local}")
                        },
                    });
                }
                last = "VERSION.txt had no usable version".into();
            }
            Err(e) => last = e,
        }
    }
    Err(format!("Could not check for app updates ({last})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare_numeric() {
        assert!(is_newer("0.8.23", "0.8.21"));
        assert!(is_newer("0.8.22", "0.8.21"));
        assert!(!is_newer("0.8.21", "0.8.21"));
        assert!(!is_newer("0.8.21-sha256", "0.8.21"));
        assert!(!is_newer("0.8.3", "0.8.21"));
        assert!(is_newer("v0.9.0", "0.8.21"));
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
}

fn install_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "no install directory".into())
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
        let name = entry.name().replace('\\', "/");
        let file_name = name.rsplit('/').next().unwrap_or("");
        let lower = name.to_ascii_lowercase();

        // Map kit layouts → install dir (preserve original path casing after prefix).
        //   cyd-companion-app-only/cyd-companion.exe
        //   cyd-companion-app-only/Firmware/...
        //   cyd-miner-kit/cyd-companion.exe
        let rel = if let Some(idx) = lower.find("cyd-companion-app-only/") {
            name[idx + "cyd-companion-app-only/".len()..].to_string()
        } else if let Some(idx) = lower.find("cyd-companion-windows/") {
            name[idx + "cyd-companion-windows/".len()..].to_string()
        } else if let Some(idx) = lower.find("cyd-miner-kit/") {
            name[idx + "cyd-miner-kit/".len()..].to_string()
        } else if file_name.eq_ignore_ascii_case("cyd-companion.exe") {
            file_name.to_string()
        } else if lower.contains("/firmware/") {
            format!("Firmware/{file_name}")
        } else if lower.contains("/tools/") {
            format!("Tools/{file_name}")
        } else {
            continue;
        };

        let rel_path = rel.replace('\\', "/");
        let out_name = if rel_path.eq_ignore_ascii_case("cyd-companion.exe") {
            // Never overwrite the running exe in-place on Windows — stage as .new
            PathBuf::from("cyd-companion.exe.new")
        } else {
            PathBuf::from(&rel_path)
        };

        let out_path = dest_dir.join(&out_name);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
        progress(format!("Extracting {rel_path}…"));
        let mut out = std::fs::File::create(&out_path).map_err(|e| format!("create: {e}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("extract: {e}"))?;
        if out_name
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("cyd-companion.exe.new"))
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
    let new_name = "cyd-companion.exe.new";
    let bat_path = install.join("cyd-companion-update.bat");
    // Retry while .new still exists (move failed because the old exe is locked).
    // The previous "if not exist exe" check was wrong: a failed move leaves the
    // old exe in place, so the bat would start the stale build immediately.
    let bat = format!(
        "@echo off\r\n\
         setlocal\r\n\
         cd /d \"{dir}\"\r\n\
         timeout /t 2 /nobreak >nul\r\n\
         set /a tries=0\r\n\
         :retry\r\n\
         set /a tries+=1\r\n\
         if exist \"{exe}\" del /f /q \"{exe}\" >nul 2>nul\r\n\
         move /y \"{new}\" \"{exe}\" >nul 2>nul\r\n\
         if exist \"{new}\" (\r\n\
           if %tries% geq 40 exit /b 1\r\n\
           timeout /t 1 /nobreak >nul\r\n\
           goto retry\r\n\
         )\r\n\
         if not exist \"{exe}\" exit /b 1\r\n\
         start \"\" \"{exe}\"\r\n\
         del \"%~f0\"\r\n",
        dir = install.display(),
        new = new_name,
        exe = exe_name,
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
pub fn update_companion_app(progress: &dyn Fn(String)) -> Result<AppRemoteInfo, String> {
    let local = running_version();
    let info = check_app_update(progress)?;
    if !info.newer {
        progress(info.detail.clone());
        return Ok(info);
    }

    let install = install_dir()?;
    progress(format!(
        "Updating Companion {local} → {} in {}",
        info.version,
        install.display()
    ));

    // Prefer full App-Only zip (exe + Firmware + Tools).
    let zip_path = install.join("companion-update.zip.part");
    let mut zip_ok = false;
    let mut last = String::new();
    for url in app_zip_urls() {
        progress(format!("GET {url}"));
        match http_download(&url, &zip_path, progress) {
            Ok(()) => match extract_app_kit(&zip_path, &install, progress) {
                Ok(()) => {
                    zip_ok = true;
                    let _ = std::fs::remove_file(&zip_path);
                    break;
                }
                Err(e) => {
                    last = e;
                    let _ = std::fs::remove_file(&zip_path);
                }
            },
            Err(e) => last = e,
        }
    }

    if !zip_ok {
        progress("Zip update failed — trying bare cyd-companion.exe…".into());
        let new_exe = install.join("cyd-companion.exe.new");
        let mut exe_ok = false;
        for url in app_exe_urls() {
            progress(format!("GET {url}"));
            match http_download(&url, &new_exe, progress) {
                Ok(()) => {
                    let bytes = std::fs::metadata(&new_exe).map(|m| m.len()).unwrap_or(0);
                    if bytes > 1_000_000 {
                        exe_ok = true;
                        break;
                    }
                    last = format!("exe too small ({bytes} bytes)");
                    let _ = std::fs::remove_file(&new_exe);
                }
                Err(e) => last = e,
            }
        }
        if !exe_ok {
            return Err(format!("Could not download Companion update ({last})"));
        }
    }

    let _ = std::fs::write(
        install.join("VERSION.txt"),
        format!("{}-sha256\n", info.version),
    );

    #[cfg(windows)]
    {
        progress("Scheduling replace + restart…".into());
        schedule_windows_replace_and_restart(&install)?;
        progress(format!(
            "Companion {} downloaded — restarting…",
            info.version
        ));
        return Ok(AppRemoteInfo {
            version: info.version,
            newer: true,
            detail: "Restarting into the new Companion build…".into(),
        });
    }

    #[cfg(not(windows))]
    {
        let _ = last;
        Err(format!(
            "Downloaded update files into {} — replace the binary manually, then relaunch.",
            install.display()
        ))
    }
}
