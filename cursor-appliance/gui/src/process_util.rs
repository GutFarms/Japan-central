use std::path::{Path, PathBuf};
use std::process::Command;

pub fn exe_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

pub fn find_binary(appliance_root: &Path, base: &str) -> Option<PathBuf> {
    let name = exe_name(base);
    let mut candidates = vec![
        appliance_root.join(&name),
        appliance_root.join("bin").join(&name),
        appliance_root.join("gui").join("target").join("release").join(&name),
        appliance_root.join("gui").join("target").join("debug").join(&name),
    ];

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.insert(0, dir.join(&name));
        }
    }

    // Friendly packaged name for the GUI.
    if base == "cursor-appliance-gui" {
        candidates.insert(0, appliance_root.join(exe_name("CursorAppliance")));
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.insert(0, dir.join(exe_name("CursorAppliance")));
            }
        }
    }

    candidates.into_iter().find(|p| p.is_file())
}

pub fn run_script(script: &Path) -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script.to_string_lossy(),
        ]);
        cmd
    } else {
        Command::new(script)
    }
}

pub fn kill_image(image: &str) {
    if cfg!(windows) {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", image, "/T"])
            .output();
    } else {
        let _ = Command::new("pkill").args(["-f", image]).output();
    }
}

pub fn kill_cloud_helpers() {
    if cfg!(windows) {
        // Best-effort: agent.exe hosts the worker bridge on Windows installs.
        kill_image("agent.exe");
    } else {
        let _ = Command::new("pkill")
            .args(["-f", "scripts/start-worker.sh"])
            .output();
        let _ = Command::new("pkill")
            .args(["-f", "agent worker"])
            .output();
    }
}

pub fn kill_local_helpers() {
    if cfg!(windows) {
        kill_image("cursor-local-worker.exe");
    } else {
        let _ = Command::new("pkill")
            .args(["-f", "scripts/start-local-worker.sh"])
            .output();
        let _ = Command::new("pkill")
            .args(["-f", "cursor-local-worker"])
            .output();
    }
}
