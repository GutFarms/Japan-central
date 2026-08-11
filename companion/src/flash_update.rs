//! Push bundled (or nearby) firmware to the ESP32-2432S028 over USB serial.
//! Uses `espflash` (preferred) or Python `esptool` — same path as Flash-Firmware.bat.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

pub const MERGED_BIN_NAME: &str = "esp32-2432s028-sha256-miner-merged.bin";

#[derive(Clone, Debug)]
pub struct FirmwareImage {
    pub path: PathBuf,
    pub bytes: u64,
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
                return Ok(FirmwareImage { path, bytes });
            }
        }
    }

    Err(format!(
        "Firmware image not found ({MERGED_BIN_NAME}). Use CYD Miner Setup / Portable kit so Firmware\\ sits next to the app."
    ))
}

fn find_tool(names: &[&str]) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            dirs.push(dir.join("Tools"));
            dirs.push(dir.join("tools"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.clone());
        dirs.push(cwd.join("Tools"));
    }

    for dir in &dirs {
        for name in names {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    // PATH lookup
    for name in names {
        if which_on_path(name).is_some() {
            return Some(PathBuf::from(name));
        }
    }
    None
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

/// Flash merged image @ 0x0 (DIO / 4MB / 40MHz layout already inside the merge).
pub fn flash_merged_bin(port: &str, image: &Path, progress: &dyn Fn(String)) -> Result<(), String> {
    if port.trim().is_empty() {
        return Err("Select a COM / serial port before updating.".into());
    }
    if !image.is_file() {
        return Err(format!("Firmware missing: {}", image.display()));
    }

    progress(format!(
        "Flashing {} ({} bytes) → {} @ 0x0",
        image
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("firmware.bin"),
        std::fs::metadata(image).map(|m| m.len()).unwrap_or(0),
        port
    ));
    progress("Hold BOOT + tap RESET if the board does not enter download mode.".into());

    if let Some(espflash) = find_tool(&[
        "espflash.exe",
        "espflash",
        #[cfg(windows)]
        "espflash.cmd",
    ]) {
        progress(format!("Using espflash ({})", espflash.display()));
        let mut cmd = Command::new(&espflash);
        cmd.args([
            "write-bin",
            "-p",
            port,
            "-B",
            "460800",
            "-c",
            "esp32",
            "--non-interactive",
            "--skip-update-check",
            "0x0",
        ])
        .arg(image)
        .env("ESPFLASH_SKIP_UPDATE_CHECK", "1");
        return run_streaming(&mut cmd, progress, "espflash");
    }

    // Python esptool — same flags as packaging/Flash-Firmware.bat
    let esptool_cmds: Vec<(&str, Vec<&str>)> = vec![
        (
            "py",
            vec![
                "-3",
                "-m",
                "esptool",
                "--chip",
                "esp32",
                "--port",
                port,
                "--baud",
                "460800",
                "write_flash",
                "-z",
                "--flash_mode",
                "dio",
                "--flash_freq",
                "40m",
                "--flash_size",
                "4MB",
                "0x0",
            ],
        ),
        (
            "python",
            vec![
                "-m",
                "esptool",
                "--chip",
                "esp32",
                "--port",
                port,
                "--baud",
                "460800",
                "write_flash",
                "-z",
                "--flash_mode",
                "dio",
                "--flash_freq",
                "40m",
                "--flash_size",
                "4MB",
                "0x0",
            ],
        ),
        (
            "python3",
            vec![
                "-m",
                "esptool",
                "--chip",
                "esp32",
                "--port",
                port,
                "--baud",
                "460800",
                "write_flash",
                "-z",
                "--flash_mode",
                "dio",
                "--flash_freq",
                "40m",
                "--flash_size",
                "4MB",
                "0x0",
            ],
        ),
        (
            "esptool.py",
            vec![
                "--chip",
                "esp32",
                "--port",
                port,
                "--baud",
                "460800",
                "write_flash",
                "-z",
                "--flash_mode",
                "dio",
                "--flash_freq",
                "40m",
                "--flash_size",
                "4MB",
                "0x0",
            ],
        ),
        (
            "esptool",
            vec![
                "--chip",
                "esp32",
                "--port",
                port,
                "--baud",
                "460800",
                "write_flash",
                "-z",
                "--flash_mode",
                "dio",
                "--flash_freq",
                "40m",
                "--flash_size",
                "4MB",
                "0x0",
            ],
        ),
    ];

    let mut last_err = String::new();
    for (bin, args) in esptool_cmds {
        if which_on_path(bin).is_none() && bin != "esptool.py" {
            // Still try — Windows py launcher may exist without being a normal PATH file probe.
            if bin != "py" && bin != "python" && bin != "python3" {
                continue;
            }
        }
        progress(format!("Trying {bin} -m esptool / esptool…"));
        let mut cmd = Command::new(bin);
        cmd.args(&args).arg(image);
        match run_streaming(&mut cmd, progress, bin) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e;
                progress(format!("{bin} failed — trying next flasher…"));
            }
        }
    }

    Err(if last_err.is_empty() {
        "No flasher found. Install espflash (Tools\\espflash.exe in the kit) or: py -3 -m pip install esptool".into()
    } else {
        format!(
            "Flash failed: {last_err}. Tip: hold BOOT, tap RESET, release BOOT, then Update again."
        )
    })
}

fn run_streaming(
    cmd: &mut Command,
    progress: &dyn Fn(String),
    label: &str,
) -> Result<(), String> {
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
        // Keep logs readable — skip ultra-noisy percent spam duplicates lightly.
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
