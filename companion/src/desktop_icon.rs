//! Create a branded Desktop shortcut on Windows (logo ICO next to the exe).

use std::path::PathBuf;
#[cfg(windows)]
use std::process::Command;

const SHORTCUT_NAME: &str = "Njordr Seas CYD miner.lnk";
const ICO_NAME: &str = "cyd-miner.ico";
const ICO_BYTES: &[u8] = include_bytes!("../assets/cyd-miner.ico");

/// Ensure `cyd-miner.ico` sits beside the Companion exe (for shortcuts / Explorer).
pub fn ensure_logo_ico_beside_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let ico = dir.join(ICO_NAME);
    if !ico.is_file() {
        let _ = std::fs::write(&ico, ICO_BYTES);
    }
    if ico.is_file() {
        Some(ico)
    } else {
        None
    }
}

fn ps_escape(s: &str) -> String {
    s.replace('\'', "''")
}

fn desktop_dir() -> Option<PathBuf> {
    // Prefer the real Desktop folder (handles OneDrive Desktop redirection).
    #[cfg(windows)]
    {
        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "[Environment]::GetFolderPath('Desktop')",
            ])
            .output()
            .ok()?;
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                let path = PathBuf::from(p);
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let desk = PathBuf::from(home).join("Desktop");
    if desk.is_dir() {
        Some(desk)
    } else {
        None
    }
}

/// Create (or refresh) a Desktop .lnk that uses the brand logo icon.
/// Returns a short status string for logs / UI.
pub fn ensure_desktop_shortcut() -> Result<String, String> {
    #[cfg(not(windows))]
    {
        return Err("Desktop shortcut is Windows-only".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let exe = std::env::current_exe().map_err(|e| format!("exe path: {e}"))?;
        let exe_dir = exe
            .parent()
            .ok_or_else(|| "exe has no parent dir".to_string())?
            .to_path_buf();
        let ico = ensure_logo_ico_beside_exe().unwrap_or_else(|| exe.clone());
        let desktop = desktop_dir().ok_or_else(|| "Desktop folder not found".to_string())?;
        let lnk = desktop.join(SHORTCUT_NAME);

        let target = exe.display().to_string();
        let work = exe_dir.display().to_string();
        let icon_loc = if ico.extension().and_then(|e| e.to_str()) == Some("ico") {
            format!("{}", ico.display())
        } else {
            format!("{},0", exe.display())
        };
        let lnk_s = lnk.display().to_string();

        let script = format!(
            "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); \
             $s.TargetPath = '{}'; \
             $s.WorkingDirectory = '{}'; \
             $s.IconLocation = '{}'; \
             $s.Description = 'Njordr Seas CYD miner'; \
             $s.Save(); \
             if (-not (Test-Path -LiteralPath '{}')) {{ throw 'shortcut missing after save' }}",
            ps_escape(&lnk_s),
            ps_escape(&target),
            ps_escape(&work),
            ps_escape(&icon_loc),
            ps_escape(&lnk_s),
        );

        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("powershell: {e}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "desktop shortcut failed: {}",
                err.trim().chars().take(200).collect::<String>()
            ));
        }
        Ok(format!("Desktop icon ready · {}", lnk.display()))
    }
}

/// Fire-and-forget Desktop icon install on a background thread (Windows).
pub fn spawn_auto_desktop_icon() {
    #[cfg(windows)]
    {
        std::thread::spawn(|| {
            let _ = ensure_logo_ico_beside_exe();
            let _ = ensure_desktop_shortcut();
        });
    }
}
