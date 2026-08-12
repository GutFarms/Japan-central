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
                    // Prefer a line that looks like a fw tag.
                    if t.contains("sha256") || t.contains('.') {
                        // "Firmware: 0.8.5-sha256" or bare version
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
    // Prefer flash/downloads (tracked on GitHub). Prefer feature branch until merge to master.
    const REFS: &[&str] = &[
        "cursor/esp32-cyd-cpp-firmware-e801",
        "master",
        "main",
    ];
    const PATHS: &[&str] = &[
        "flash/downloads/esp32-2432s028-sha256-miner-merged.bin",
        "flash/esp32-2432s028-sha256-miner-merged.bin",
    ];
    let mut out = Vec::new();
    for r in REFS {
        for p in PATHS {
            out.push(format!(
                "https://raw.githubusercontent.com/GutFarms/Japan-central/{r}/{p}"
            ));
        }
    }
    out
}

fn fetch_nearby_version_hint(bin_url: &str) -> Option<String> {
    let ver_url = bin_url.rsplit_once('/')?.0.to_string() + "/VERSION.txt";
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(6))
        .timeout_read(std::time::Duration::from_secs(10))
        .user_agent(COMPANION_UA)
        .build();
    let txt = agent.get(&ver_url).call().ok()?.into_string().ok()?;
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
    None
}

fn portable_zip_candidate_urls() -> Vec<String> {
    const REFS: &[&str] = &[
        "cursor/esp32-cyd-cpp-firmware-e801",
        "master",
        "main",
    ];
    const NAMES: &[&str] = &[
        "CYD-Miner-Portable.zip",
        "CYD-Companion-Portable.zip",
    ];
    let mut out = Vec::new();
    for r in REFS {
        for n in NAMES {
            out.push(format!(
                "https://raw.githubusercontent.com/GutFarms/Japan-central/{r}/flash/downloads/{n}"
            ));
        }
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

const COMPANION_UA: &str = "CYD-Companion/0.8.11";

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
            // Ignore unrelated repo releases (e.g. Native Pure).
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
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(180))
        .user_agent(COMPANION_UA)
        .build();
    let resp = agent.get(url).call().map_err(|e| format!("http: {e}"))?;
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
