//! Push bundled (or nearby) firmware to the ESP32-2432S028 over USB serial.
//! Prefers bundled / auto-downloaded `espflash`; optional Python `esptool` fallback.

use std::io::{BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MERGED_BIN_NAME: &str = "esp32-2432s028-sha256-miner-merged.bin";
/// App-only image for Wi‑Fi OTA (`cmp ota`) — not the merged @ 0x0 USB flash image.
pub const APP_BIN_NAME: &str = "esp32-2432s028-sha256-miner.bin";
pub const APP_BIN_D0_NAME: &str = "esp32-2432s028-sha256-miner-d0.bin";

/// Shared cancel + Terminator-style BOOT Ready handshake with the UI.
#[derive(Clone)]
pub struct FlashControl {
    pub cancel: Arc<AtomicBool>,
    /// Flash thread sets true while waiting for the user to confirm BOOT.
    pub need_boot: Arc<AtomicBool>,
    /// UI sets true when the user clicks Ready (chip should be in download mode).
    pub boot_ready: Arc<AtomicBool>,
}

impl FlashControl {
    pub fn new(cancel: Arc<AtomicBool>) -> Self {
        Self {
            cancel,
            need_boot: Arc::new(AtomicBool::new(false)),
            boot_ready: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FirmwareImage {
    pub path: PathBuf,
    pub bytes: u64,
    pub version: String,
}

/// True when the image is a user-dropped / browsed .bin (not the auto-resolved kit image).
pub fn firmware_is_custom(fw: &FirmwareImage) -> bool {
    fw.version.to_ascii_lowercase().contains("dropped")
}

/// Load any `.bin` (merged @ 0x0 or app image) for Update board / flash.
pub fn load_firmware_bin(path: &Path) -> Result<FirmwareImage, String> {
    let path = path.to_path_buf();
    if !path.is_file() {
        return Err(format!("File not found: {}", path.display()));
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !name.ends_with(".bin") {
        return Err("Drop a .bin firmware image (merged @ 0x0 preferred).".into());
    }
    let bytes = std::fs::metadata(&path)
        .map(|m| m.len())
        .map_err(|e| format!("stat {}: {e}", path.display()))?;
    if bytes < 64_000 {
        return Err(format!(
            "{} is only {bytes} bytes — need a full ESP32 image (>64 KB).",
            path.display()
        ));
    }
    // Peek magic: ESP image at 0x0 (merged) or raw payload.
    let mut hdr = [0u8; 1];
    if let Ok(mut f) = std::fs::File::open(&path) {
        let _ = std::io::Read::read(&mut f, &mut hdr);
    }
    let near = read_nearby_fw_version(&path);
    let version = if near.is_empty() {
        if hdr[0] == 0xE9 {
            "dropped".into()
        } else {
            "dropped-raw".into()
        }
    } else {
        format!("{near} · dropped")
    };
    Ok(FirmwareImage {
        path,
        bytes,
        version,
    })
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

fn collect_bin_candidates(file_name: &str) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("Firmware").join(file_name));
            candidates.push(dir.join(file_name));
            let mut walk = dir.to_path_buf();
            for _ in 0..6 {
                candidates.push(walk.join("flash").join("downloads").join(file_name));
                candidates.push(walk.join("flash").join(file_name));
                candidates.push(
                    walk.join("dist")
                        .join("cyd-miner-kit")
                        .join("Firmware")
                        .join(file_name),
                );
                if !walk.pop() {
                    break;
                }
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("Firmware").join(file_name));
        candidates.push(cwd.join("flash").join("downloads").join(file_name));
        candidates.push(cwd.join("flash").join(file_name));
        candidates.push(cwd.join(file_name));
    }
    candidates
}

/// True when a board fw tag / path names the CYD-D0 flavor.
pub fn board_fw_is_d0(fw_or_path: &str) -> bool {
    let f = fw_or_path.to_ascii_lowercase();
    f.contains("-d0") || f.contains("_d0") || f.contains("d0.bin") || f.contains("d0-merged")
}

fn app_bin_names(prefer_d0: bool) -> [&'static str; 2] {
    if prefer_d0 {
        [APP_BIN_D0_NAME, APP_BIN_NAME]
    } else {
        [APP_BIN_NAME, APP_BIN_D0_NAME]
    }
}

/// Resolve the app-only `.bin` used for wireless OTA (ESP Update / `cmp ota`).
pub fn find_app_firmware_image() -> Result<FirmwareImage, String> {
    find_app_firmware_image_ex(false)
}

pub fn find_app_firmware_image_ex(prefer_d0: bool) -> Result<FirmwareImage, String> {
    for name in app_bin_names(prefer_d0) {
        for path in collect_bin_candidates(name) {
            if path.is_file() {
                let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                if bytes > 64_000 {
                    let mut hdr = [0u8; 1];
                    if let Ok(mut f) = std::fs::File::open(&path) {
                        let _ = f.read(&mut hdr);
                    }
                    if hdr[0] != 0xE9 {
                        continue;
                    }
                    let version = read_nearby_fw_version(&path);
                    return Ok(FirmwareImage {
                        path,
                        bytes,
                        version,
                    });
                }
            }
        }
    }
    Err(format!(
        "App firmware not found ({APP_BIN_NAME}). Wi‑Fi push needs the app-only image next to the merged kit bin."
    ))
}

/// Prefer an app-only image for OTA. If `preferred` is already app-sized with ESP magic, use it;
/// if it is a merged @ 0x0 image, look for the sibling app bin in the same folder / kit paths.
pub fn resolve_ota_app_image(preferred: Option<&Path>) -> Result<FirmwareImage, String> {
    let prefer_d0 = preferred
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(board_fw_is_d0)
        .unwrap_or(false);
    resolve_ota_app_image_ex(preferred, prefer_d0)
}

/// Like [`resolve_ota_app_image`], but picks the D0 app bin first when `prefer_d0`.
pub fn resolve_ota_app_image_ex(
    preferred: Option<&Path>,
    prefer_d0: bool,
) -> Result<FirmwareImage, String> {
    if let Some(path) = preferred {
        if path.is_file() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            let mut hdr = [0u8; 1];
            if let Ok(mut f) = std::fs::File::open(path) {
                let _ = f.read(&mut hdr);
            }
            // Merged kits are named *-merged.bin and are larger than the app.
            let looks_merged = name.contains("merged") || bytes > 1_100_000;
            if !looks_merged && hdr[0] == 0xE9 && bytes > 64_000 {
                let version = read_nearby_fw_version(path);
                return Ok(FirmwareImage {
                    path: path.to_path_buf(),
                    bytes,
                    version,
                });
            }
            if let Some(dir) = path.parent() {
                for sib in app_bin_names(prefer_d0) {
                    let p = dir.join(sib);
                    if p.is_file() {
                        let b = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                        if b > 64_000 {
                            let version = read_nearby_fw_version(&p);
                            return Ok(FirmwareImage {
                                path: p,
                                bytes: b,
                                version,
                            });
                        }
                    }
                }
            }
        }
    }
    find_app_firmware_image_ex(prefer_d0)
}

/// Scan an ESP app image for an embedded `N.N.N-sha256` tag (from `kFwTag`).
pub fn scan_app_bin_fw_tag(bytes: &[u8]) -> Option<String> {
    // ASCII search for "0." … "-sha256" (optionally "-d0").
    let needle = b"-sha256";
    let mut i = 0usize;
    while i + 12 < bytes.len() {
        if let Some(rel) = bytes[i..].windows(needle.len()).position(|w| w == needle) {
            let end = i + rel + needle.len();
            let mut start = i + rel;
            while start > 0 && (bytes[start - 1].is_ascii_digit() || bytes[start - 1] == b'.') {
                start -= 1;
            }
            if start < i + rel && bytes[start].is_ascii_digit() {
                let mut tag = String::from_utf8_lossy(&bytes[start..end]).into_owned();
                if end + 3 <= bytes.len() && &bytes[end..end + 3] == b"-d0" {
                    tag.push_str("-d0");
                }
                if tag.contains('.') && tag.contains("-sha256") {
                    return Some(normalize_fw_version(&tag));
                }
            }
            i = end;
            continue;
        }
        break;
    }
    None
}

fn ota_wait_line<R: Read + ?Sized>(
    stream: &mut R,
    rx: &mut String,
    deadline: Instant,
    pred: &dyn Fn(&str) -> bool,
    label: &str,
) -> Result<String, String> {
    let mut tmp = [0u8; 2048];
    while Instant::now() < deadline {
        match stream.read(&mut tmp) {
            Ok(0) => {}
            Ok(n) => {
                rx.push_str(&String::from_utf8_lossy(&tmp[..n]));
                if rx.len() > 32768 {
                    crate::utf8_safe::keep_last(rx, 8192);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(format!("{label} read: {e}")),
        }
        for line in rx.lines() {
            let t = line.trim();
            if pred(t) {
                let out = t.to_string();
                // Drop consumed prefix up to this line.
                if let Some(pos) = rx.find(t) {
                    let end = pos + t.len();
                    *rx = rx[end..].trim_start_matches(['\r', '\n']).to_string();
                }
                return Ok(out);
            }
            if t.starts_with("CMPERR") {
                return Err(t.to_string());
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!("{label} timed out waiting for board ACK"))
}

fn push_firmware_ota_stream<S: Read + Write + ?Sized>(
    stream: &mut S,
    label: &str,
    endpoint: &str,
    img: &FirmwareImage,
    bytes: &[u8],
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> Result<(), String> {
    push_firmware_ota_stream_ex(stream, label, endpoint, img, bytes, progress, cancel, false)
}

/// `slow`: safer pace for boards still on older firmware (no miner suspend during OTA).
fn push_firmware_ota_stream_ex<S: Read + Write + ?Sized>(
    stream: &mut S,
    label: &str,
    endpoint: &str,
    img: &FirmwareImage,
    bytes: &[u8],
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
    slow: bool,
) -> Result<(), String> {
    if cancel.load(Ordering::SeqCst) {
        return Err(format!("{label} cancelled"));
    }
    progress(format!(
        "{label} → {endpoint} · {} · {} KB",
        img.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app.bin"),
        bytes.len() / 1024
    ));

    let mut rx = String::new();
    let _ = stream.write_all(b"\r\ncmp stop\r\n");
    let _ = stream.flush();
    std::thread::sleep(Duration::from_millis(200));
    // Drain any stop ACK noise.
    let _ = ota_wait_line(
        stream,
        &mut rx,
        Instant::now() + Duration::from_millis(800),
        &|t| t.starts_with("CMPACK"),
        label,
    );

    if cancel.load(Ordering::SeqCst) {
        return Err(format!("{label} cancelled"));
    }

    let cmd = format!("cmp ota size={}\r\n", bytes.len());
    stream
        .write_all(cmd.as_bytes())
        .map_err(|e| format!("ota cmd: {e}"))?;
    stream.flush().map_err(|e| format!("ota flush: {e}"))?;

    let ready = ota_wait_line(
        stream,
        &mut rx,
        Instant::now() + Duration::from_secs(8),
        &|t| t.starts_with("CMPACK ota ready") || t.eq_ignore_ascii_case("CMPACK ota ready"),
        label,
    )?;
    progress(format!("Board ready · {ready}"));
    // Let the board leave the command path, suspend miners, and enter binary
    // drain before the first chunk — racing Update.write caused CMPERR ota write.
    // Older boards (pre-0.8.191) need a longer settle because miners stay live.
    std::thread::sleep(Duration::from_millis(if slow { 450 } else { 220 }));
    let mut drain = [0u8; 512];
    while let Ok(n) = stream.read(&mut drain) {
        if n == 0 {
            break;
        }
        rx.push_str(&String::from_utf8_lossy(&drain[..n]));
        for line in rx.lines() {
            let t = line.trim();
            if t.to_ascii_lowercase().starts_with("cmperr ota") {
                return Err(format!("{label} board rejected before upload: {t}"));
            }
        }
        if rx.len() > 2048 {
            crate::utf8_safe::keep_last(&mut rx, 512);
        }
    }
    progress(format!(
        "{label} streaming app image{}…",
        if slow { " (safe pace)" } else { "" }
    ));

    // Match firmware pollOtaBinary buf[512]. Unpaced blasts fill the 16 KiB
    // USB RX buffer while flash sector erase stalls → CMPERR ota write.
    // 256 B + higher floor works against boards that still hash during OTA.
    let chunk: usize = if slow { 256 } else { 512 };
    let total = bytes.len();
    let mut sent = 0usize;
    let mut last_pct = 0u32;
    let mut chunks = 0u32;
    while sent < total {
        if cancel.load(Ordering::SeqCst) {
            return Err(format!("{label} cancelled"));
        }
        let end = (sent + chunk).min(total);
        stream
            .write_all(&bytes[sent..end])
            .map_err(|e| format!("ota write @{sent}: {e}"))?;
        sent = end;
        chunks = chunks.wrapping_add(1);
        // Pace so the board can drain flash writes; slower at start (board settling)
        // and near the end (commit phase). Keep a floor so sector erase cannot
        // overflow the CH340 RX FIFO.
        let pct_now = ((sent as u64 * 100) / total as u64) as u32;
        let pace_ms = if slow {
            if pct_now < 20 {
                28u64
            } else if pct_now >= 90 {
                40
            } else if pct_now >= 80 {
                32
            } else if pct_now >= 70 {
                26
            } else {
                20
            }
        } else if pct_now < 20 {
            14u64
        } else if pct_now >= 90 {
            22
        } else if pct_now >= 80 {
            16
        } else if pct_now >= 70 {
            12
        } else {
            8
        };
        let _ = stream.flush();
        // Abort early on CMPERR ota write instead of blasting the rest blind.
        let mut tmp = [0u8; 512];
        if let Ok(n) = stream.read(&mut tmp) {
            if n > 0 {
                rx.push_str(&String::from_utf8_lossy(&tmp[..n]));
                for line in rx.lines() {
                    let t = line.trim();
                    let low = t.to_ascii_lowercase();
                    if low.starts_with("cmperr ota") {
                        return Err(format!("{label} board rejected mid-upload: {t}"));
                    }
                }
                if rx.len() > 4096 {
                    crate::utf8_safe::keep_last(&mut rx, 1024);
                }
            }
        }
        // Extra breather every few chunks so erase windows catch up.
        let extra = if chunks % 8 == 0 {
            if slow {
                12
            } else {
                6
            }
        } else {
            0
        };
        std::thread::sleep(Duration::from_millis(pace_ms + extra));
        if pct_now >= last_pct + 5 || sent == total {
            last_pct = pct_now;
            progress(format!("{label} upload {pct_now}% ({sent}/{total})"));
        }
    }
    stream.flush().map_err(|e| format!("ota final flush: {e}"))?;

    // Drop leftover lines (esp. CMPACK ota ready) so we never treat ready/begin as success.
    rx.clear();
    progress(format!(
        "{label} upload complete — waiting for board ACK (flash commit / reboot)…"
    ));

    // Board must reply CMPACK ota ok then reboots. Do NOT match bare "CMPACK ota"
    // (that falsely accepted "CMPACK ota ready" left in the buffer).
    // Update.end + partition switch can take well over 45s on worn flash.
    match ota_wait_line(
        stream,
        &mut rx,
        Instant::now() + Duration::from_secs(90),
        &|t| {
            let low = t.to_ascii_lowercase();
            if low.starts_with("cmperr ota") {
                return false; // still wait; handled below via buffer scan
            }
            low.starts_with("cmpack ota ok")
        },
        label,
    ) {
        Ok(line) => progress(format!("Board · {line}")),
        Err(e) => {
            let low_rx = rx.to_ascii_lowercase();
            if let Some(err_line) = rx.lines().find(|l| {
                l.trim()
                    .to_ascii_lowercase()
                    .starts_with("cmperr ota")
            }) {
                return Err(format!(
                    "{label} board rejected after upload: {}",
                    err_line.trim()
                ));
            }
            // Full send without ok is a FAILED push — board may still be on the old image.
            // Callers may reopen + retry once; avoid jumping straight to BOOT.
            let _ = low_rx;
            return Err(format!(
                "{label} uploaded {sent}/{total} B but no CMPACK ota ok ({e}). Retry Push (no BOOT); use Flash (BOOT) only if Push keeps failing."
            ));
        }
    }
    progress(format!(
        "{label} pushed {} to {endpoint}",
        if img.version.is_empty() {
            "app image".into()
        } else {
            img.version.clone()
        }
    ));
    Ok(())
}

fn load_ota_app_bytes(image: &Path) -> Result<(FirmwareImage, Vec<u8>), String> {
    load_ota_app_bytes_ex(image, board_fw_is_d0(&image.to_string_lossy()))
}

fn load_ota_app_bytes_ex(image: &Path, prefer_d0: bool) -> Result<(FirmwareImage, Vec<u8>), String> {
    let mut img = resolve_ota_app_image_ex(Some(image), prefer_d0)?;
    let bytes = std::fs::read(&img.path).map_err(|e| format!("read {}: {e}", img.path.display()))?;
    if bytes.len() < 64_000 || bytes[0] != 0xE9 {
        return Err(format!(
            "{} is not an ESP app image (need 0xE9 magic, >64 KB)",
            img.path.display()
        ));
    }
    if let Some(embedded) = scan_app_bin_fw_tag(&bytes) {
        let near = img.version.clone();
        if !near.is_empty() {
            let a = fw_version_key(&near);
            let b = fw_version_key(&embedded);
            if a != b {
                return Err(format!(
                    "OTA image mismatch: VERSION.txt says {near} but {} embeds {embedded}. Re-fetch firmware / sync the Firmware folder.",
                    img.path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("app.bin")
                ));
            }
        }
        img.version = embedded;
    }
    Ok((img, bytes))
}

fn ota_line_is_ping_ack(t: &str) -> bool {
    t.starts_with("CMP ok") || t.eq_ignore_ascii_case("CMPACK ping")
}

fn ota_err_is_ack_miss(err: &str) -> bool {
    let low = err.to_ascii_lowercase();
    low.contains("no cmpack ota ok")
}

fn connect_wifi_ota_stream(endpoint: &str) -> Result<TcpStream, String> {
    let addr = endpoint
        .to_socket_addrs()
        .map_err(|e| format!("resolve {endpoint}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address for {endpoint}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("connect {endpoint}: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .ok();
    stream.set_nodelay(true).ok();
    Ok(stream)
}

/// Push app firmware over TCP `cmp ota size=N` (wireless Update board path).
pub fn push_firmware_ota(
    endpoint: &str,
    image: &Path,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> Result<(), String> {
    push_firmware_ota_ex(endpoint, image, false, progress, cancel)
}

pub fn push_firmware_ota_ex(
    endpoint: &str,
    image: &Path,
    prefer_d0: bool,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> Result<(), String> {
    let (img, bytes) = load_ota_app_bytes_ex(image, prefer_d0)?;
    progress(format!(
        "OTA image · {} · {} · {} KB",
        img.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app.bin"),
        if img.version.is_empty() {
            "?".into()
        } else {
            img.version.clone()
        },
        bytes.len() / 1024
    ));
    let mut stream = connect_wifi_ota_stream(endpoint)?;
    match push_firmware_ota_stream(
        &mut stream,
        "Wi‑Fi OTA",
        endpoint,
        &img,
        &bytes,
        progress,
        cancel,
    ) {
        Ok(()) => Ok(()),
        Err(e) if ota_err_is_ack_miss(&e) => {
            drop(stream);
            progress(
                "No Wi‑Fi OTA ACK — waiting for reboot, then soft-verify / one retry…".into(),
            );
            std::thread::sleep(Duration::from_secs(4));
            if cancel.load(Ordering::SeqCst) {
                return Err("Wi‑Fi OTA cancelled".into());
            }
            if wifi_ota_soft_verify(endpoint, &img.version, progress) {
                progress(format!(
                    "Wi‑Fi OTA soft-verified — board answers with {}",
                    if img.version.is_empty() {
                        "new firmware".into()
                    } else {
                        img.version.clone()
                    }
                ));
                return Ok(());
            }
            progress("Board still reachable / old tag — retrying full Wi‑Fi OTA once…".into());
            // Give stuck binary-mode boards time to idle-timeout before retry.
            std::thread::sleep(Duration::from_secs(9));
            if cancel.load(Ordering::SeqCst) {
                return Err("Wi‑Fi OTA cancelled".into());
            }
            let mut stream2 = connect_wifi_ota_stream(endpoint)?;
            let mut rx = String::new();
            let _ = stream2.write_all(b"\r\ncmp ping\r\n");
            let _ = stream2.flush();
            if ota_wait_line(
                &mut stream2,
                &mut rx,
                Instant::now() + Duration::from_secs(3),
                &ota_line_is_ping_ack,
                "Wi‑Fi OTA",
            )
            .is_err()
            {
                return Err(e);
            }
            match push_firmware_ota_stream(
                &mut stream2,
                "Wi‑Fi OTA",
                endpoint,
                &img,
                &bytes,
                progress,
                cancel,
            ) {
                Ok(()) => Ok(()),
                Err(e2) if ota_err_is_ack_miss(&e2) => {
                    drop(stream2);
                    std::thread::sleep(Duration::from_secs(4));
                    if wifi_ota_soft_verify(endpoint, &img.version, progress) {
                        Ok(())
                    } else {
                        Err(e2)
                    }
                }
                Err(e2) => Err(e2),
            }
        }
        Err(e) => Err(e),
    }
}

/// Push app firmware over an already-open serial/TCP stream (`cmp ota`).
/// Used when the mine-worker still holds the live link at the correct baud.
pub fn push_firmware_ota_on_port<S: Read + Write + ?Sized>(
    stream: &mut S,
    image: &Path,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> Result<(), String> {
    push_firmware_ota_on_port_ex(stream, image, false, progress, cancel)
}

pub fn push_firmware_ota_on_port_ex<S: Read + Write + ?Sized>(
    stream: &mut S,
    image: &Path,
    prefer_d0: bool,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> Result<(), String> {
    push_firmware_ota_on_port_recover_ex(stream, image, prefer_d0, progress, cancel, None)
}

/// Like [`push_firmware_ota_on_port_ex`], but after a lost `CMPACK ota ok` can
/// soft-verify / retry via USB reopen (`usb_reopen`) or Wi‑Fi TCP reconnect
/// (`tcp_reopen`).
pub fn push_firmware_ota_on_port_recover_ex<S: Read + Write + ?Sized>(
    stream: &mut S,
    image: &Path,
    prefer_d0: bool,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
    reopen: Option<OtaReopen<'_>>,
) -> Result<(), String> {
    let (img, bytes) = load_ota_app_bytes_ex(image, prefer_d0)?;
    progress(format!(
        "OTA image · {} · {} · {} KB",
        img.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app.bin"),
        if img.version.is_empty() {
            "?".into()
        } else {
            img.version.clone()
        },
        bytes.len() / 1024
    ));
    match push_firmware_ota_stream_ex(
        stream,
        "USB OTA",
        "open-link",
        &img,
        &bytes,
        progress,
        cancel,
        true,
    ) {
        Ok(()) => Ok(()),
        Err(e) if ota_err_is_ack_miss(&e) => {
            progress(
                "No OTA ACK on open link — waiting for reboot, then soft-verify / one retry…"
                    .into(),
            );
            std::thread::sleep(Duration::from_secs(3));
            if cancel.load(Ordering::SeqCst) {
                return Err("USB OTA cancelled".into());
            }
            // Prefer soft-verify on the still-open stream first (ACK lost but no reboot yet).
            if usb_ota_soft_verify_on(stream, &img.version, progress) {
                return Ok(());
            }
            match reopen {
                Some(OtaReopen::Usb(port)) => {
                    if usb_ota_soft_verify_any_baud(port, &img.version, progress) {
                        progress(format!(
                            "USB OTA soft-verified — board answers with {}",
                            if img.version.is_empty() {
                                "new firmware".into()
                            } else {
                                img.version.clone()
                            }
                        ));
                        return Ok(());
                    }
                    progress(
                        "Board still reachable / old tag — leaving open-link; reopen path will retry…"
                            .into(),
                    );
                    Err(e)
                }
                Some(OtaReopen::Tcp(endpoint)) => {
                    if wifi_ota_soft_verify(endpoint, &img.version, progress) {
                        return Ok(());
                    }
                    Err(e)
                }
                None => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

/// How to re-check the board after an open-link OTA ACK miss.
pub enum OtaReopen<'a> {
    Usb(&'a str),
    Tcp(&'a str),
}

/// Push app firmware over USB serial `cmp ota size=N` — live board, no BOOT hold.
/// Tries 460800 first (current companion default), then 115200 (older boards).
pub fn push_firmware_ota_usb(
    port: &str,
    image: &Path,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> Result<(), String> {
    push_firmware_ota_usb_ex(port, image, false, progress, cancel)
}

pub fn push_firmware_ota_usb_ex(
    port: &str,
    image: &Path,
    prefer_d0: bool,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> Result<(), String> {
    use crate::workers::{is_usb_serial_port, open_usb_serial};

    if !is_usb_serial_port(port) {
        return Err(format!(
            "USB OTA needs a COM port (got '{port}'). Use Push update (Wi‑Fi) for host:19284."
        ));
    }
    let (img, bytes) = load_ota_app_bytes_ex(image, prefer_d0)?;
    progress(format!(
        "OTA image · {} · {} · {} KB",
        img.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app.bin"),
        if img.version.is_empty() {
            "?".into()
        } else {
            img.version.clone()
        },
        bytes.len() / 1024
    ));
    let mut last = String::new();
    // One automatic full retry after a post-upload ACK miss (board often reboots mid-ACK).
    let mut ack_retry_left = 1u8;
    // Prefer 115200 for the binary stream — 460800 floods older boards that still
    // hash during OTA (CMPERR ota write). Ping may still answer at either baud.
    for baud in [115_200u32, 460_800] {
        if cancel.load(Ordering::SeqCst) {
            return Err("USB OTA cancelled".into());
        }
        progress(format!("Opening {port} @ {baud} for USB OTA (no BOOT)…"));
        // Longer I/O timeout during bulk OTA — 400ms was tight if Update.write stalls.
        let mut stream = match open_usb_serial(port, baud, Duration::from_millis(2_000)) {
            Ok(s) => s,
            Err(e) => {
                last = e;
                progress(format!("USB open @ {baud} failed — trying next baud…"));
                continue;
            }
        };
        // Confirm baud with ping before committing to binary OTA.
        let mut rx = String::new();
        let _ = stream.write_all(b"\r\ncmp ping\r\n");
        let _ = stream.flush();
        let ping_ok = ota_wait_line(
            &mut *stream,
            &mut rx,
            Instant::now() + Duration::from_millis(if baud > 115_200 { 1_200 } else { 2_000 }),
            &ota_line_is_ping_ack,
            "USB OTA",
        );
        match ping_ok {
            Ok(line) => progress(format!("USB OTA link @ {baud} · {line}")),
            Err(e) => {
                last = format!("no cmp ping @ {baud}: {e}");
                progress(format!("No ping @ {baud} — trying next baud…"));
                drop(stream);
                continue;
            }
        }
        // Always safe-pace on USB — first Push must succeed against pre-0.8.191 boards
        // that do not yet suspend miners during Update.write.
        match push_firmware_ota_stream_ex(
            &mut *stream,
            "USB OTA",
            port,
            &img,
            &bytes,
            progress,
            cancel,
            true,
        ) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let low = e.to_ascii_lowercase();
                last = e.clone();
                if low.contains("cancelled") {
                    return Err(e);
                }
                // Post-upload: board may have rebooted before we saw CMPACK ota ok.
                // Wait, re-ping, and either soft-verify the new tag or retry once.
                if ota_err_is_ack_miss(&e) {
                    drop(stream);
                    progress(
                        "No OTA ACK (common at ~82% after upload) — waiting for reboot, then re-check…"
                            .into(),
                    );
                    std::thread::sleep(Duration::from_secs(3));
                    if cancel.load(Ordering::SeqCst) {
                        return Err("USB OTA cancelled".into());
                    }
                    if usb_ota_soft_verify_any_baud(port, &img.version, progress) {
                        progress(format!(
                            "USB OTA soft-verified — board answers with {}",
                            if img.version.is_empty() {
                                "new firmware".into()
                            } else {
                                img.version.clone()
                            }
                        ));
                        return Ok(());
                    }
                    if ack_retry_left > 0 {
                        ack_retry_left -= 1;
                        progress(
                            "Board still reachable / old tag — reboot nudge, then one full USB OTA retry…"
                                .into(),
                        );
                        let _ = nudge_usb_reboot_for_push(port, progress, cancel);
                        // Wait for firmware OTA idle-timeout (8s) + reboot settle.
                        std::thread::sleep(Duration::from_secs(10));
                        if cancel.load(Ordering::SeqCst) {
                            return Err("USB OTA cancelled".into());
                        }
                        let mut stream2 =
                            match open_usb_serial(port, baud, Duration::from_millis(2_000)) {
                                Ok(s) => s,
                                Err(e2) => {
                                    last = e2;
                                    continue;
                                }
                            };
                        let mut rx2 = String::new();
                        let _ = stream2.write_all(b"\r\ncmp ping\r\n");
                        let _ = stream2.flush();
                        if ota_wait_line(
                            &mut *stream2,
                            &mut rx2,
                            Instant::now() + Duration::from_secs(3),
                            &ota_line_is_ping_ack,
                            "USB OTA",
                        )
                        .is_err()
                        {
                            // Baud may have changed after reboot — try next.
                            progress("No ping after ACK miss — trying next baud…".into());
                            drop(stream2);
                            continue;
                        }
                        match push_firmware_ota_stream_ex(
                            &mut *stream2,
                            "USB OTA",
                            port,
                            &img,
                            &bytes,
                            progress,
                            cancel,
                            true,
                        ) {
                            Ok(()) => return Ok(()),
                            Err(e2) => {
                                last = e2;
                                drop(stream2);
                                if ota_err_is_ack_miss(&last) {
                                    std::thread::sleep(Duration::from_secs(3));
                                    if usb_ota_soft_verify_any_baud(port, &img.version, progress)
                                    {
                                        return Ok(());
                                    }
                                }
                                // Don't thrash further bauds after two full binary attempts.
                                return Err(last);
                            }
                        }
                    }
                    return Err(e);
                }
                // Early mid-upload fail (UI often stuck at ~15% Board ready): clear binary
                // mode via reboot, wait for firmware idle-timeout, retry once on this baud.
                if low.contains("ota write")
                    || low.contains("mid-upload")
                    || low.contains("rejected before upload")
                    || low.contains("rejected mid-upload")
                {
                    drop(stream);
                    if ack_retry_left > 0 {
                        ack_retry_left -= 1;
                        progress(format!(
                            "USB OTA died early ({e}) — reboot nudge, wait for idle clear, retry…"
                        ));
                        let _ = nudge_usb_reboot_for_push(port, progress, cancel);
                        std::thread::sleep(Duration::from_secs(10));
                        if cancel.load(Ordering::SeqCst) {
                            return Err("USB OTA cancelled".into());
                        }
                        // Re-enter baud loop from the top by continuing after forcing
                        // another attempt at this baud via a nested open.
                        let mut stream2 =
                            match open_usb_serial(port, baud, Duration::from_millis(2_000)) {
                                Ok(s) => s,
                                Err(e2) => {
                                    last = e2;
                                    continue;
                                }
                            };
                        let mut rx2 = String::new();
                        let _ = stream2.write_all(b"\r\ncmp ping\r\n");
                        let _ = stream2.flush();
                        if ota_wait_line(
                            &mut *stream2,
                            &mut rx2,
                            Instant::now() + Duration::from_secs(3),
                            &ota_line_is_ping_ack,
                            "USB OTA",
                        )
                        .is_err()
                        {
                            progress("No ping after early OTA fail — trying next baud…".into());
                            drop(stream2);
                            continue;
                        }
                        match push_firmware_ota_stream_ex(
                            &mut *stream2,
                            "USB OTA",
                            port,
                            &img,
                            &bytes,
                            progress,
                            cancel,
                            true,
                        ) {
                            Ok(()) => return Ok(()),
                            Err(e2) => {
                                last = e2;
                                drop(stream2);
                                if ota_err_is_ack_miss(&last) {
                                    std::thread::sleep(Duration::from_secs(3));
                                    if usb_ota_soft_verify_any_baud(port, &img.version, progress) {
                                        return Ok(());
                                    }
                                }
                                return Err(last);
                            }
                        }
                    }
                    return Err(e);
                }
                // If ota ready never came, try the other baud (rare: ping false-positive).
                if low.contains("timed out") || low.contains("ota ready") || low.contains("unknown")
                {
                    progress(format!("USB OTA @ {baud} failed ({e}) — retrying…"));
                    drop(stream);
                    continue;
                }
                return Err(e);
            }
        }
    }
    // Open-link may have left the board stuck in binary OTA mode — no ping on either baud.
    // Reboot + wait for the 8s idle timeout, then one more full attempt.
    if last.to_ascii_lowercase().contains("no cmp ping") {
        progress(
            "No companion ping (possible stuck OTA binary mode) — reboot + clear, then retry once…"
                .into(),
        );
        let _ = nudge_usb_reboot_for_push(port, progress, cancel);
        std::thread::sleep(Duration::from_secs(10));
        if cancel.load(Ordering::SeqCst) {
            return Err("USB OTA cancelled".into());
        }
        for baud in [115_200u32, 460_800] {
            if cancel.load(Ordering::SeqCst) {
                return Err("USB OTA cancelled".into());
            }
            let mut stream = match open_usb_serial(port, baud, Duration::from_millis(2_000)) {
                Ok(s) => s,
                Err(e) => {
                    last = e;
                    continue;
                }
            };
            let mut rx = String::new();
            let _ = stream.write_all(b"\r\ncmp ping\r\n");
            let _ = stream.flush();
            if ota_wait_line(
                &mut *stream,
                &mut rx,
                Instant::now() + Duration::from_secs(2),
                &ota_line_is_ping_ack,
                "USB OTA",
            )
            .is_err()
            {
                drop(stream);
                continue;
            }
            match push_firmware_ota_stream_ex(
                &mut *stream,
                "USB OTA",
                port,
                &img,
                &bytes,
                progress,
                cancel,
                true,
            ) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last = e;
                    drop(stream);
                    if ota_err_is_ack_miss(&last)
                        && usb_ota_soft_verify_any_baud(port, &img.version, progress)
                    {
                        return Ok(());
                    }
                    break;
                }
            }
        }
    }
    Err(if last.is_empty() {
        "USB OTA failed at 460800 and 115200".into()
    } else {
        last
    })
}

/// Soft-verify after lost ACK — try both companion bauds (reboot may re-enumerate).
fn usb_ota_soft_verify_any_baud(
    port: &str,
    expected_ver: &str,
    progress: &dyn Fn(String),
) -> bool {
    for baud in [460_800u32, 115_200] {
        if usb_ota_soft_verify(port, baud, expected_ver, progress) {
            return true;
        }
    }
    false
}

/// After a lost OTA ACK, reopen and check whether the board already runs the target tag.
fn usb_ota_soft_verify(
    port: &str,
    baud: u32,
    expected_ver: &str,
    progress: &dyn Fn(String),
) -> bool {
    use crate::workers::open_usb_serial;
    let Ok(mut stream) = open_usb_serial(port, baud, Duration::from_millis(800)) else {
        // Port may still be re-enumerating — try once more shortly.
        std::thread::sleep(Duration::from_secs(2));
        let Ok(mut s) = open_usb_serial(port, baud, Duration::from_millis(800)) else {
            return false;
        };
        return usb_ota_soft_verify_on(&mut *s, expected_ver, progress);
    };
    usb_ota_soft_verify_on(&mut *stream, expected_ver, progress)
}

fn wifi_ota_soft_verify(
    endpoint: &str,
    expected_ver: &str,
    progress: &dyn Fn(String),
) -> bool {
    let Ok(mut stream) = connect_wifi_ota_stream(endpoint) else {
        std::thread::sleep(Duration::from_secs(2));
        let Ok(mut s) = connect_wifi_ota_stream(endpoint) else {
            return false;
        };
        return usb_ota_soft_verify_on(&mut s, expected_ver, progress);
    };
    usb_ota_soft_verify_on(&mut stream, expected_ver, progress)
}

fn usb_ota_soft_verify_on<S: Read + Write + ?Sized>(
    stream: &mut S,
    expected_ver: &str,
    progress: &dyn Fn(String),
) -> bool {
    let mut rx = String::new();
    let _ = stream.write_all(b"\r\ncmp config\r\n");
    let _ = stream.flush();
    let line = match ota_wait_line(
        stream,
        &mut rx,
        Instant::now() + Duration::from_secs(3),
        &|t| t.starts_with("CMPCONFIG ") || t.starts_with('{'),
        "USB OTA verify",
    ) {
        Ok(l) => l,
        Err(_) => {
            let _ = stream.write_all(b"\r\ncmp ping\r\n");
            let _ = stream.flush();
            match ota_wait_line(
                stream,
                &mut rx,
                Instant::now() + Duration::from_secs(2),
                &ota_line_is_ping_ack,
                "USB OTA verify",
            ) {
                Ok(l) => l,
                Err(_) => return false,
            }
        }
    };
    progress(format!("Post-OTA board says · {}", trunc_flash_line(&line, 120)));
    if expected_ver.is_empty() {
        // Any companion answer after reboot is enough when we have no tag to match.
        return line.starts_with("CMP") || line.starts_with('{');
    }
    let want = fw_version_key(expected_ver);
    let hay = line.to_ascii_lowercase();
    if hay.contains(&want) || hay.contains(&expected_ver.to_ascii_lowercase()) {
        return true;
    }
    // CMPCONFIG often embeds fw=0.8.185-sha256-d0
    if let Some(fw) = line
        .split([' ', '&', ',', '{', '}', '"', ':'])
        .find(|p| p.contains("-sha256"))
    {
        if fw_version_key(fw) == want {
            return true;
        }
    }
    false
}

fn trunc_flash_line(s: &str, max: usize) -> String {
    crate::utf8_safe::trunc(s, max)
}

/// Soft reboot a live USB board so a stuck `cmp ota` binary session clears
/// before the next Push OTA retry (no BOOT / no ROM).
pub fn nudge_usb_reboot_for_push(
    port: &str,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> bool {
    use crate::workers::{is_usb_serial_port, open_usb_serial};

    if !is_usb_serial_port(port) || cancel.load(Ordering::SeqCst) {
        return false;
    }
    for baud in [460_800u32, 115_200] {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        let Ok(mut stream) = open_usb_serial(port, baud, Duration::from_millis(350)) else {
            continue;
        };
        let mut rx = String::new();
        let _ = stream.write_all(b"\r\ncmp ping\r\n");
        let _ = stream.flush();
        if ota_wait_line(
            &mut *stream,
            &mut rx,
            Instant::now() + Duration::from_millis(1_000),
            &ota_line_is_ping_ack,
            "reboot nudge",
        )
        .is_err()
        {
            continue;
        }
        let _ = stream.write_all(b"cmp reboot\r\n");
        let _ = stream.flush();
        progress(format!(
            "Reboot nudged @ {baud} — settling before Push OTA retry (no BOOT)…"
        ));
        drop(stream);
        std::thread::sleep(Duration::from_millis(1800));
        return true;
    }
    false
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

/// Download latest merged.bin (and sibling app.bin for Wi‑Fi OTA).
pub fn fetch_latest_firmware(
    progress: &dyn Fn(String),
) -> Result<FirmwareImage, String> {
    let dest_dir = firmware_writable_dir()?;
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("mkdir Firmware: {e}"))?;
    let dest = dest_dir.join(MERGED_BIN_NAME);
    let app_dest = dest_dir.join(APP_BIN_NAME);
    let ver_path = dest_dir.join("VERSION.txt");

    progress("Checking GitHub releases for firmware…".into());
    if let Ok((url, ver)) = find_release_firmware_url() {
        progress(format!("Downloading release firmware {ver}…"));
        download_to(&url, &dest, progress)?;
        let _ = std::fs::write(&ver_path, format!("{ver}\n"));
        let _ = fetch_sibling_app_bin(&dest_dir, &url, progress);
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
        // Skip app-only URLs for the primary merged fetch.
        if url.contains("miner.bin") && !url.contains("merged") {
            continue;
        }
        progress(format!("GET {url}"));
        match download_to(&url, &dest, progress) {
            Ok(()) => {
                let ver = fetch_nearby_version_hint(&url).unwrap_or_else(|| "repo-flash".into());
                let _ = std::fs::write(&ver_path, format!("{ver}\n"));
                let _ = fetch_sibling_app_bin(&dest_dir, &url, progress);
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
        Ok(img) => {
            if !app_dest.is_file() {
                let _ = find_app_firmware_image().map(|a| {
                    let _ = std::fs::copy(&a.path, &app_dest);
                });
            }
            return Ok(img);
        }
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

/// Best-effort download of app-only bin beside merged (Wi‑Fi Push update).
fn fetch_sibling_app_bin(
    dest_dir: &Path,
    merged_url_hint: &str,
    progress: &dyn Fn(String),
) -> Result<(), String> {
    let dest = dest_dir.join(APP_BIN_NAME);
    if dest.is_file() {
        let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        if bytes > 64_000 {
            return Ok(());
        }
    }
    let mut urls = Vec::new();
    if merged_url_hint.contains("merged") {
        urls.push(merged_url_hint.replace("-merged.bin", ".bin").replace("merged.bin", "miner.bin"));
    }
    for p in [
        "flash/downloads/esp32-2432s028-sha256-miner.bin",
        "flash/esp32-2432s028-sha256-miner.bin",
    ] {
        urls.extend(repo_bin_urls(p));
    }
    for url in urls {
        progress(format!("GET app (Wi‑Fi OTA) {url}"));
        if download_to(&url, &dest, progress).is_ok() {
            let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
            if bytes > 64_000 {
                return Ok(());
            }
        }
    }
    Err("app.bin not fetched".into())
}

fn raw_firmware_candidate_urls() -> Vec<String> {
    let mut out = Vec::new();
    // Prefer commit-pinned raw for ~1MB bins — Contents API is flaky at the 1MB cap
    // and a long timeout on a stuck GET looks like “Fetching…” forever.
    for p in [
        "flash/downloads/esp32-2432s028-sha256-miner-merged.bin",
        "flash/downloads/esp32-2432s028-sha256-miner-d0-merged.bin",
        "flash/esp32-2432s028-sha256-miner-merged.bin",
        // App-only for Wi‑Fi OTA (sibling of merged in Firmware/).
        "flash/downloads/esp32-2432s028-sha256-miner.bin",
        "flash/downloads/esp32-2432s028-sha256-miner-d0.bin",
        "flash/esp32-2432s028-sha256-miner.bin",
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

pub const COMPANION_UA: &str = concat!("Njordr-seas-CYD-miner/", env!("CARGO_PKG_VERSION"));
const ESPFLASH_VERSION: &str = "4.5.0";
pub const REPO_OWNER: &str = "GutFarms";
pub const REPO_NAME: &str = "Japan-central";
/// Branches probed for Companion/firmware updates (newest VERSION wins).
/// Tip first — apps still on older builds may only hit the legacy CYD branch.
pub const REPO_REFS: &[&str] = &[
    // Tip agent branches first — newest VERSION wins for Fetch firmware / app update.
    "cursor/no-assist-no-mesh-e801",
    "cursor/ota-write-safe-e801",
    "cursor/ota-write-fail-e801",
    "cursor/share-counter-reset-e801",
    "cursor/project-cleanup-e801",
    "cursor/demote-fighting-systems-e801",
    "cursor/ota-fail-15pct-e801",
    "cursor/pool-hs-lag-e801",
    "cursor/ota-still-fails-e801",
    "master",
    "main",
];

/// URL-encode a git ref for GitHub API `?ref=` / path segments.
pub fn urlencode_ref(ref_name: &str) -> String {
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
const WRITE_TIMEOUT: Duration = Duration::from_secs(90);
const ERASE_TIMEOUT: Duration = Duration::from_secs(90);
const RESET_TIMEOUT: Duration = Duration::from_secs(12);
const ESPTOOL_TIMEOUT: Duration = Duration::from_secs(100);
/// Hard wall-clock budget for the whole Update board flash sequence.
/// Push + one BOOT Ready round — fail fast instead of thrashing the COM for minutes.
const FLASH_BUDGET: Duration = Duration::from_secs(150);
/// Abort a write round after this many connect/MAC stalls (stops terminal/COM thrash).
/// Silent Push is one attempt; Ready rounds allow two no-reset tries.
const MAX_CONNECT_STALLS_PER_ROUND: u8 = 2;
/// Abort immediately after this many Access Denied / port-busy errors.
const MAX_PORT_BUSY: u8 = 1;
/// Quiet gap between tool launches so Windows/CH340 can release the COM handle.
/// Too short (≤250ms) looks like connect↔disconnect thrash in Device Manager.
const ATTEMPT_GAP: Duration = Duration::from_millis(1200);
const FLASH_SAFE_BAUD: &str = "115200";
/// No useful output at all → stuck before connect (silent Push fail-fast).
const IDLE_TIMEOUT: Duration = Duration::from_secs(16);
/// Chip MAC/connect seen but no write progress yet — fail fast on silent Push.
const IDLE_AFTER_CONNECT: Duration = Duration::from_secs(12);
/// After BOOT Ready, ROM/no-stub can take longer before the first Writing % tick.
const IDLE_AFTER_CONNECT_PATIENT: Duration = Duration::from_secs(40);
/// Stub upload after MAC is not real write progress — kill fast and try no-stub.
const IDLE_STUB_UPLOAD: Duration = Duration::from_secs(10);
/// During active write/erase (% / `\r` ticks), allow longer silence between ticks.
const IDLE_DURING_WRITE: Duration = Duration::from_secs(50);
const IDLE_DURING_WRITE_PATIENT: Duration = Duration::from_secs(90);
/// After stdout/stderr EOF, wait for the tool to exit (hard-reset / flush) before kill.
/// Matches esptool-js / ESP Terminator: never yank the serial mid-operation.
const EXIT_GRACE: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
enum FlashToolKind {
    Write,
    Erase,
    Other,
}

fn flash_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.map(|c| c.load(Ordering::SeqCst)).unwrap_or(false)
}

fn flash_cancelled_ctrl(ctrl: &FlashControl) -> bool {
    ctrl.cancel.load(Ordering::SeqCst)
}

/// Run an espflash subcommand, retrying skip-update-check flag forms for Windows clap quirks.
fn run_espflash_argv(
    espflash: &Path,
    sub_args: &[&str],
    progress: &dyn Fn(String),
    label: &str,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
    kind: FlashToolKind,
    patient: bool,
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
        // Avoid config/env monitor settings spamming:
        // "Monitor options were provided, but `--monitor/-M` flag isn't set…"
        cmd.env_remove("ESPFLASH_SKIP_UPDATE_CHECK");
        cmd.env_remove("ESPFLASH_MONITOR");
        cmd.env_remove("ESPFLASH_BAUD");
        for key in ["ESPFLASH_CONFIG", "ESPFLASH_PORT", "ESPFLASH_BEFORE", "ESPFLASH_AFTER"] {
            cmd.env_remove(key);
        }
        if set_env_true {
            cmd.env("ESPFLASH_SKIP_UPDATE_CHECK", "true");
        }
        cmd.args(skip).args(sub_args);
        match run_streaming_timeout(&mut cmd, progress, label, timeout, cancel, kind, patient) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let lower = e.to_ascii_lowercase();
                last = e;
                // Only rotate skip-flag forms on CLI parse errors; port/flash/timeout stop here.
                if lower.contains("timed out")
                    || lower.contains("cancelled")
                    || lower.contains("idle")
                    || lower.contains("output closed")
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

/// Copy the firmware image into a local temp file so espflash/esptool never read
/// through OneDrive/cloud placeholders mid-write (common stall at "Chip connected" ~14%).
fn stage_flash_image_local(image: &Path, progress: &dyn Fn(String)) -> Result<PathBuf, String> {
    let meta = std::fs::metadata(image).map_err(|e| format!("firmware stat: {e}"))?;
    let bytes = meta.len();
    if bytes < 64_000 {
        return Err(format!(
            "Firmware too small ({} bytes) — re-fetch Update board firmware",
            bytes
        ));
    }
    let name = image
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(MERGED_BIN_NAME);
    let dir = std::env::temp_dir().join("cyd-flash");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir flash staging: {e}"))?;
    let dest = dir.join(name);
    if dest.is_file() {
        if let Ok(dm) = std::fs::metadata(&dest) {
            if dm.len() == bytes {
                // Same size already staged — still refresh if source is newer.
                let src_t = meta.modified().ok();
                let dst_t = dm.modified().ok();
                if let (Some(s), Some(d)) = (src_t, dst_t) {
                    if d >= s {
                        progress(format!(
                            "Flash staging ready · {} ({} KB local temp)",
                            dest.display(),
                            bytes / 1024
                        ));
                        return Ok(dest);
                    }
                }
            }
        }
    }
    progress(format!(
        "Staging firmware locally ({:.1} MB) — avoids OneDrive/cloud locks during write…",
        bytes as f64 / (1024.0 * 1024.0)
    ));
    // Chunked read forces cloud hydrate and shows progress (was silent → UI stuck at 2%).
    let mut file = std::fs::File::open(image).map_err(|e| {
        format!(
            "Could not open firmware {} ({e}) — if it is on OneDrive, right-click → Always keep on this device",
            image.display()
        )
    })?;
    let mut data = Vec::with_capacity(bytes as usize);
    let mut buf = [0u8; 256 * 1024];
    let mut last_report = 0u64;
    loop {
        let n = std::io::Read::read(&mut file, &mut buf).map_err(|e| format!("read firmware: {e}"))?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        let got = data.len() as u64;
        if got.saturating_sub(last_report) >= 256 * 1024 {
            last_report = got;
            progress(format!(
                "Staging firmware… {} / {} KB",
                got / 1024,
                bytes / 1024
            ));
        }
    }
    if (data.len() as u64) != bytes {
        return Err(format!(
            "Firmware read truncated ({} of {} bytes) — OneDrive may still be syncing; retry",
            data.len(),
            bytes
        ));
    }
    let tmp = dir.join(format!("{name}.part"));
    std::fs::write(&tmp, &data).map_err(|e| format!("write flash staging: {e}"))?;
    std::fs::rename(&tmp, &dest).map_err(|e| format!("promote flash staging: {e}"))?;
    progress(format!(
        "Flash staging ready · {} ({} KB)",
        dest.display(),
        data.len() / 1024
    ));
    Ok(dest)
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
        FlashToolKind::Erase,
        false,
    )
}

fn run_espflash_write(
    espflash: &Path,
    port: &str,
    baud: &str,
    before: &str,
    no_stub: bool,
    patient: bool,
    image: &Path,
    progress: &dyn Fn(String),
    budget: Duration,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let timeout = clamp_timeout(
        if patient {
            Duration::from_secs(140)
        } else {
            WRITE_TIMEOUT
        },
        budget,
    );
    let stub = if no_stub { "no-stub" } else { "stub" };
    progress(format!(
        "espflash write-bin → {port} @ {baud} before={before} {stub}{} ({}) [timeout {}s]",
        if patient { " · patient" } else { "" },
        espflash.display(),
        timeout.as_secs()
    ));
    progress(format!("Connecting to chip on {port}…"));
    let addr = "0x0";
    let img = image.to_string_lossy();
    // Build argv with optional --no-stub (ROM loader — blank/CH340 boards often stall
    // uploading the RAM stub after printing MAC).
    let mut args: Vec<&str> = vec![
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
    ];
    if no_stub {
        args.push("--no-stub");
    }
    args.push(addr);
    args.push(img.as_ref());
    run_espflash_argv(
        espflash,
        &args,
        progress,
        "espflash-write",
        timeout,
        cancel,
        FlashToolKind::Write,
        patient,
    )
}

/// Terminator-style: wait until the UI Ready button (or Cancel), not a blind countdown.
fn wait_for_boot_ready(
    ctrl: &FlashControl,
    progress: &dyn Fn(String),
    why: &str,
) -> Result<(), String> {
    ctrl.boot_ready.store(false, Ordering::SeqCst);
    ctrl.need_boot.store(true, Ordering::SeqCst);
    progress(format!(
        "Hold BOOT, tap RESET, keep BOOT held — then click Ready ({why}). \
Keep BOOT held until you see Writing %."
    ));
    let mut tick = 0u32;
    loop {
        if flash_cancelled_ctrl(ctrl) {
            ctrl.need_boot.store(false, Ordering::SeqCst);
            return Err("flash cancelled".into());
        }
        if ctrl.boot_ready.load(Ordering::SeqCst) {
            ctrl.need_boot.store(false, Ordering::SeqCst);
            ctrl.boot_ready.store(false, Ordering::SeqCst);
            progress(
                "Ready — writing now. Keep BOOT held until Writing % appears…"
                    .into(),
            );
            // Settle briefly — do NOT open the COM ourselves (CH340 DTR on
            // close kicks the chip out of download mode before espflash starts).
            // Slightly longer than 200ms so Windows finishes enumerating after RESET.
            std::thread::sleep(Duration::from_millis(600));
            return Ok(());
        }
        if tick % 20 == 0 {
            progress(format!(
                "Waiting for Ready… hold BOOT + RESET, keep BOOT, click Ready ({why})"
            ));
        }
        tick = tick.saturating_add(1);
        std::thread::sleep(Duration::from_millis(100));
    }
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
        FlashToolKind::Other,
        false,
    )
}

/// Python esptool write (same engine as ESP Terminator / esptool-js).
/// Prefer stub+compress first; for `--no-stub` use uncompressed (`--no-compress`) —
/// ROM FlashDeflate on ~1MB images often hangs right after MAC (UI stuck ~14%).
fn run_esptool_write(
    port: &str,
    image: &Path,
    before: &str,
    no_stub: bool,
    compress: bool,
    baud: &str,
    progress: &dyn Fn(String),
    budget: Duration,
    cancel: Option<&AtomicBool>,
    patient: bool,
) -> Result<(), String> {
    let (py, mut args, pythonpath) = ensure_python_esptool(progress)?;
    args.extend([
        "--chip".into(),
        "esp32".into(),
        "--port".into(),
        port.to_string(),
        "--baud".into(),
        baud.to_string(),
        "--before".into(),
        before.to_string(),
    ]);
    if no_stub {
        args.push("--no-stub".into());
    }
    args.push("write_flash".into());
    if compress {
        args.push("-z".into());
    } else {
        args.push("--no-compress".into());
    }
    args.extend([
        "--flash_mode".into(),
        "dio".into(),
        "--flash_freq".into(),
        "40m".into(),
        "--flash_size".into(),
        "4MB".into(),
        "0x0".into(),
        image.display().to_string(),
    ]);
    let py_timeout = clamp_timeout(
        if patient {
            Duration::from_secs(180)
        } else {
            ESPTOOL_TIMEOUT
        },
        budget,
    );
    let stub = if no_stub { "no-stub" } else { "stub" };
    let comp = if compress { "compress" } else { "no-compress" };
    progress(format!(
        "esptool write_flash {stub} {comp} before={before} @ {baud} [timeout {}s]…",
        py_timeout.as_secs()
    ));
    progress(format!("Connecting to chip on {port}…"));
    let mut cmd = Command::new(&py);
    if let Some(pp) = pythonpath {
        #[cfg(windows)]
        {
            let prev = std::env::var_os("PYTHONPATH").unwrap_or_default();
            let joined = if prev.is_empty() {
                pp.clone()
            } else {
                let mut s = pp;
                s.push(";");
                s.push(prev);
                s
            };
            cmd.env("PYTHONPATH", joined);
        }
        #[cfg(not(windows))]
        {
            let prev = std::env::var_os("PYTHONPATH").unwrap_or_default();
            let joined = if prev.is_empty() {
                pp.clone()
            } else {
                let mut s = pp;
                s.push(":");
                s.push(prev);
                s
            };
            cmd.env("PYTHONPATH", joined);
        }
    }
    cmd.args(&args);
    run_streaming_timeout(
        &mut cmd,
        progress,
        "esptool",
        py_timeout,
        cancel,
        FlashToolKind::Write,
        patient,
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
/// `live_push`: board was answering cmp — try auto-reset write first (no BOOT Ready).
/// Blank / download-mode boards keep the Ready + BOOT path.
/// Flash merged firmware @ 0x0 via USB.
///
/// Strategy (one smooth flow):
/// 1) Live Push: **one** fail-fast `no-stub @ 115200` with `default-reset` (no BOOT).
/// 2) If that stalls → Ready CTA quickly, then **`no-reset` only** writes (patient).
/// 3) Flash (BOOT): Ready first, then the same no-reset write path.
/// Never stub@460800. Never default-reset after Ready (CH340 leaves download mode).
pub fn flash_merged_bin(
    port: &str,
    image: &Path,
    progress: &dyn Fn(String),
    ctrl: &FlashControl,
    live_push: bool,
) -> Result<(), String> {
    use crate::workers::{flash_port_arg, is_usb_serial_port};

    let cancel = Some(ctrl.cancel.as_ref());

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
    let staged = stage_flash_image_local(image, progress)?;
    let image = staged.as_path();

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
    // Live Push must never enter ROM auto-reset — that path is Flash (BOOT) only.
    if live_push {
        return Err(
            "Push update uses USB app OTA only (no silent ROM). Retry Push, or use Flash (BOOT) for blank boards."
                .into(),
        );
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
        "Blank-board flash: Ready with BOOT held, then keep BOOT until Writing %."
            .into(),
    );
    append_flash_log(&format!(
        "begin port={port} arg={port_arg} live_push={live_push} baud={FLASH_SAFE_BAUD} image={}",
        image.display()
    ));

    let started = Instant::now();
    let log_progress = |line: String| {
        append_flash_log(&format!("+{}ms {line}", started.elapsed().as_millis()));
        progress(line);
    };
    let progress = &log_progress;

    let budget_left = || FLASH_BUDGET.saturating_sub(started.elapsed());
    let ensure_budget = |progress: &dyn Fn(String)| -> Result<(), String> {
        if flash_cancelled(cancel) {
            return Err("flash cancelled".into());
        }
        let left = budget_left();
        if left.is_zero() {
            Err(format!(
                "Flash timed out after {}s overall — hold BOOT, tap RESET, keep BOOT held, click Ready, then Update again.",
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

    let note_fail = |e: &str,
                     esp_err: &mut String,
                     saw_chip_connect: &mut bool,
                     connect_stall_only: &mut bool,
                     stall_hits: &mut u8,
                     busy_hits: &mut u8,
                     progress: &dyn Fn(String),
                     label: &str| {
        *esp_err = format!("{label}: {e}");
        progress(format!("write failed: {esp_err}"));
        let low = e.to_ascii_lowercase();
        let stall = (low.contains("idle") && low.contains("chip connect"))
            || (low.contains("idle") && low.contains("mac"))
            || (low.contains("idle") && low.contains("stub"))
            || low.contains("failed to connect")
            || low.contains("timed out waiting for packet")
            || (low.contains("timeout") && low.contains("connect"));
        let port_busy = low.contains("access is denied")
            || low.contains("access denied")
            || low.contains("sharing violation")
            || low.contains("resource busy")
            || low.contains("serial_not_found")
            || low.contains("not found right now")
            || (low.contains("port") && low.contains("busy"));
        if stall || low.contains("chip seen") || low.contains("stub upload") {
            *saw_chip_connect = true;
        }
        if stall {
            *stall_hits = stall_hits.saturating_add(1);
        }
        if port_busy {
            *busy_hits = busy_hits.saturating_add(1);
            progress(
                "Port busy/missing — close other apps using the COM, then retry…"
                    .into(),
            );
        }
        if !stall && !port_busy {
            *connect_stall_only = false;
        }
    };

    let espflash = ensure_espflash(progress)?;
    let mut esp_err = String::new();
    let mut py_err = String::new();
    let mut chip_may_be_blank = false;
    let mut connect_stall_only = true;
    let mut saw_chip_connect = false;
    let mut stall_hits: u8 = 0;
    let mut busy_hits: u8 = 0;

    // Helper: run one write attempt matrix. `need_ready` gates BOOT Ready once per round.
    let run_attempt_matrix = |need_ready: bool,
                              round_label: &str,
                              prefer_default_reset: bool,
                              esp_err: &mut String,
                              py_err: &mut String,
                              saw_chip_connect: &mut bool,
                              connect_stall_only: &mut bool,
                              stall_hits: &mut u8,
                              busy_hits: &mut u8|
     -> Result<bool, String> {
        // Returns Ok(true) on success, Ok(false) if all attempts failed (continue), Err on cancel/abort.
        if need_ready {
            wait_for_boot_ready(ctrl, progress, round_label)?;
        }
        // Two live matrices only:
        // - prefer_default_reset: silent Push (auto-reset)
        // - !prefer_default_reset: after Ready / Flash BOOT (no-reset)
        // Tuple: (label, before, no_stub, compress, use_esptool, baud)
        let attempts: &[(&str, &str, bool, bool, bool, &str)] = if prefer_default_reset {
            &[
                (
                    "espflash no-stub default-reset @115200",
                    "default-reset",
                    true,
                    false,
                    false,
                    FLASH_SAFE_BAUD,
                ),
            ]
        } else {
            // Post-Ready: chip already in download mode — do not reset.
            &[
                (
                    "espflash no-stub no-reset @115200",
                    "no-reset",
                    true,
                    false,
                    false,
                    FLASH_SAFE_BAUD,
                ),
            ]
        };

        let mut esptool_usable = true;
        let round_stall_start = *stall_hits;
        for &(label, before, no_stub, compress, use_esptool, baud) in attempts {
            // Silent Push must fail-fast into Ready. Ready/BOOT writes stay patient.
            let patient = need_ready;
            if flash_cancelled(cancel) {
                ctrl.need_boot.store(false, Ordering::SeqCst);
                return Err("flash cancelled".into());
            }
            if *busy_hits >= MAX_PORT_BUSY {
                progress(
                    "Stopping flash retries — COM still busy/denied (close Terminal/Arduino/other flash tools)."
                        .into(),
                );
                return Ok(false);
            }
            if stall_hits.saturating_sub(round_stall_start) >= MAX_CONNECT_STALLS_PER_ROUND {
                progress(
                    "Stopping this round — chip connect stalled (hold BOOT + Ready, or pick the CYD COM)."
                        .into(),
                );
                return Ok(false);
            }
            if budget_left() < Duration::from_secs(12) {
                break;
            }
            if use_esptool && !esptool_usable {
                continue;
            }
            progress(format!("{round_label}: {label}…"));
            let result = if use_esptool {
                match run_esptool_write(
                    &port_arg,
                    image,
                    before,
                    no_stub,
                    compress,
                    baud,
                    progress,
                    budget_left(),
                    cancel,
                    patient,
                ) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        let low = e.to_ascii_lowercase();
                        if e.contains("9009")
                            || low.contains("microsoft store")
                            || low.contains("no python")
                        {
                            *py_err = e.clone();
                            esptool_usable = false;
                            progress(format!("esptool unavailable: {e}"));
                            continue;
                        }
                        Err(e)
                    }
                }
            } else {
                run_espflash_write(
                    &espflash,
                    &port_arg,
                    baud,
                    before,
                    no_stub,
                    patient,
                    image,
                    progress,
                    budget_left(),
                    cancel,
                )
            };
            match result {
                Ok(()) => {
                    // write-bin already uses --after hard-reset — skip a second reset
                    // that only reopens the COM and looks like connect/disconnect thrash.
                    append_flash_log(&format!("success {label}"));
                    ctrl.need_boot.store(false, Ordering::SeqCst);
                    return Ok(true);
                }
                Err(e) => {
                    if e.to_ascii_lowercase().contains("cancelled") {
                        ctrl.need_boot.store(false, Ordering::SeqCst);
                        return Err(e);
                    }
                    if use_esptool {
                        *py_err = e.clone();
                    }
                    note_fail(
                        &e,
                        esp_err,
                        saw_chip_connect,
                        connect_stall_only,
                        stall_hits,
                        busy_hits,
                        progress,
                        label,
                    );
                    std::thread::sleep(ATTEMPT_GAP);
                }
            }
        }
        Ok(false)
    };

    // ── Live push: silent auto-reset first (no BOOT Ready) ─────────────────
    if live_push {
        ensure_budget(progress)?;
        progress("Push update round — silent auto-reset, no-stub first (no BOOT)…".into());
        if run_attempt_matrix(
            false,
            "push",
            true,
            &mut esp_err,
            &mut py_err,
            &mut saw_chip_connect,
            &mut connect_stall_only,
            &mut stall_hits,
            &mut busy_hits,
        )? {
            return Ok(());
        }
        if busy_hits >= MAX_PORT_BUSY {
            ctrl.need_boot.store(false, Ordering::SeqCst);
            let tip = safety_fail_tip(started.elapsed(), &esp_err, &py_err, false);
            append_flash_log(&tip);
            return Err(format!(
                "Update failed — COM port busy (close other Terminal/Arduino/flash tools), then try again. {tip}"
            ));
        }
        // Silent auto-reset stalled — escalate to Ready without a long patient wait.
        progress(
            "Auto-reset push stalled — Hold BOOT, tap RESET, keep BOOT held, click Ready…"
                .into(),
        );
        // Fresh stall budget for the Ready round (don't inherit silent-push stalls).
        stall_hits = 0;
        connect_stall_only = true;
    }

    // ── Ready → write (blank boards, or live push single fallback) ─────────
    // After Ready the chip is in download mode — use **no-reset** only.
    // default-reset here toggles DTR and ejects download mode → Ready then fail.
    // Two Ready rounds so a missed BOOT hold can be retried without restarting Update.
    let ready_rounds: u32 = 2;
    for round in 1..=ready_rounds {
        ensure_budget(progress)?;
        if busy_hits >= MAX_PORT_BUSY {
            break;
        }
        stall_hits = 0;
        if run_attempt_matrix(
            true,
            &format!("ready {round}/{ready_rounds}"),
            false, // no-reset after BOOT Ready
            &mut esp_err,
            &mut py_err,
            &mut saw_chip_connect,
            &mut connect_stall_only,
            &mut stall_hits,
            &mut busy_hits,
        )? {
            return Ok(());
        }
        if round < ready_rounds {
            progress(
                "Write after Ready did not start — Hold BOOT again, tap RESET, keep BOOT, click Ready…"
                    .into(),
            );
        }
    }

    // Skip long baud fallback / erase when we only saw connect stalls or port busy —
    // those paths reopen the COM repeatedly and look like looping terminals on Windows.
    if connect_stall_only || busy_hits >= MAX_PORT_BUSY {
        ctrl.need_boot.store(false, Ordering::SeqCst);
        let tip = safety_fail_tip(started.elapsed(), &esp_err, &py_err, false);
        append_flash_log(&tip);
        return Err(format!(
            "Update failed at chip connect (~14%) — write never started. \
Hold BOOT, tap RESET, keep BOOT held, click Ready. Use a short data USB cable \
(not charge-only). If the .bin is on OneDrive, Always keep on this device. {tip}"
        ));
    }

    // Secondary: no-reset only after another Ready — never default-reset here.
    let fallback: &[(&str, &str, bool)] = &[(FLASH_SAFE_BAUD, "no-reset", true)];
    for &(baud, before, no_stub) in fallback {
        ensure_budget(progress)?;
        wait_for_boot_ready(ctrl, progress, &format!("fallback {baud}/{before}"))?;
        match run_espflash_write(
            &espflash,
            &port_arg,
            baud,
            before,
            no_stub,
            true, // patient — ROM write after Ready
            image,
            progress,
            budget_left(),
            cancel,
        ) {
            Ok(()) => {
                append_flash_log("success write");
                ctrl.need_boot.store(false, Ordering::SeqCst);
                return Ok(());
            }
            Err(e) => {
                let stub = if no_stub { "no-stub" } else { "stub" };
                if e.to_ascii_lowercase().contains("cancelled") {
                    ctrl.need_boot.store(false, Ordering::SeqCst);
                    return Err(e);
                }
                note_fail(
                    &e,
                    &mut esp_err,
                    &mut saw_chip_connect,
                    &mut connect_stall_only,
                    &mut stall_hits,
                    &mut busy_hits,
                    progress,
                    &format!("write {baud}/{before}/{stub}"),
                );
                if busy_hits >= MAX_PORT_BUSY || stall_hits >= 4 {
                    break;
                }
                std::thread::sleep(ATTEMPT_GAP);
            }
        }
    }

    // Recovery erase ONLY when writes failed for reasons other than connect-stall.
    let allow_erase = !connect_stall_only
        && busy_hits < MAX_PORT_BUSY
        && budget_left() > Duration::from_secs(45);
    if allow_erase {
        ensure_budget(progress)?;
        progress(
            "Direct write failed — recovery erase @ 115200 (then rewrite; chip may be blank until rewrite succeeds)…"
                .into(),
        );
        match run_espflash_erase(&espflash, &port_arg, "115200", progress, budget_left(), cancel)
        {
            Ok(()) => {
                chip_may_be_blank = true;
                append_flash_log("erase ok — chip wiped; rewrite required");
                progress(
                    "Erase OK — SAFETY: flash is wiped until rewrite finishes. Rewriting…"
                        .into(),
                );
                wait_for_boot_ready(ctrl, progress, "rewrite after erase")?;
                // After Ready: no-reset only (default-reset ejects download mode).
                for (before, tool) in [("no-reset", "espflash"), ("no_reset", "esptool")] {
                    if ensure_budget(progress).is_err() {
                        break;
                    }
                    let result = if tool == "esptool" {
                        run_esptool_write(
                            &port_arg,
                            image,
                            before,
                            true,
                            false,
                            FLASH_SAFE_BAUD,
                            progress,
                            budget_left(),
                            cancel,
                            true,
                        )
                    } else {
                        run_espflash_write(
                            &espflash,
                            &port_arg,
                            FLASH_SAFE_BAUD,
                            before,
                            true,
                            true,
                            image,
                            progress,
                            budget_left(),
                            cancel,
                        )
                    };
                    match result {
                        Ok(()) => {
                            append_flash_log("success erase+write");
                            ctrl.need_boot.store(false, Ordering::SeqCst);
                            return Ok(());
                        }
                        Err(e) => {
                            esp_err = format!("rewrite after erase {before}/{tool}: {e}");
                            progress(format!("rewrite failed: {esp_err}"));
                            if e.to_ascii_lowercase().contains("cancelled") {
                                best_effort_reset(&espflash, &port_arg, progress, cancel);
                                ctrl.need_boot.store(false, Ordering::SeqCst);
                                return Err(safety_fail_tip(
                                    started.elapsed(),
                                    &esp_err,
                                    &py_err,
                                    true,
                                ));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                esp_err = format!("erase 115200: {e}");
                progress(format!("espflash erase failed: {esp_err}"));
                if e.to_ascii_lowercase().contains("cancelled") {
                    best_effort_reset(&espflash, &port_arg, progress, cancel);
                    ctrl.need_boot.store(false, Ordering::SeqCst);
                    return Err(e);
                }
            }
        }
    } else if connect_stall_only {
        progress(
            "Skipping recovery erase and final retries (connect/MAC stall only) — stop COM thrash."
                .into(),
        );
    }

    ctrl.need_boot.store(false, Ordering::SeqCst);
    // Extra reset after failure reopens COM again — only when we actually wrote something.
    if !connect_stall_only && busy_hits < MAX_PORT_BUSY {
        best_effort_reset(&espflash, &port_arg, progress, cancel);
    }

    let tip = safety_fail_tip(started.elapsed(), &esp_err, &py_err, chip_may_be_blank);
    append_flash_log(&tip);
    Err(tip)
}

/// Locate Python + esptool; if `python -m esptool` is missing, pip-install into Tools/.
fn ensure_python_esptool(
    progress: &dyn Fn(String),
) -> Result<(PathBuf, Vec<String>, Option<std::ffi::OsString>), String> {
    let py_bins: Vec<PathBuf> = ["py", "python", "python3"]
        .iter()
        .filter_map(|n| which_on_path(n))
        .filter(|p| !is_windows_store_python_stub(p))
        .collect();
    if py_bins.is_empty() {
        return Err(
            "No Python found. Install Python 3, or use Flash-Firmware.bat / esptool-js."
                .into(),
        );
    }
    let py = py_bins[0].clone();
    let py_launcher = py
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("py") || s.eq_ignore_ascii_case("py.exe"))
        .unwrap_or(false);

    let mut base_args: Vec<String> = Vec::new();
    if py_launcher {
        base_args.extend(["-3".into(), "-m".into(), "esptool".into()]);
    } else {
        base_args.extend(["-m".into(), "esptool".into()]);
    }

    // Probe system esptool.
    {
        let mut probe = Command::new(&py);
        hide_console_window(&mut probe);
        let mut args = base_args.clone();
        args.push("version".into());
        probe.args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Ok(status) = probe.status() {
            if status.success() {
                progress(format!("Found system esptool via {}", py.display()));
                return Ok((py, base_args, None));
            }
        }
    }

    // Install into Tools/esptool-pkgs (no admin / no global pip).
    let tools = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("Tools")))
        .unwrap_or_else(|| PathBuf::from("Tools"));
    let pkg = tools.join("esptool-pkgs");
    let _ = std::fs::create_dir_all(&pkg);
    progress(format!(
        "Installing esptool into {} (one-time)…",
        pkg.display()
    ));
    let mut pip = Command::new(&py);
    hide_console_window(&mut pip);
    let mut pip_args: Vec<String> = Vec::new();
    if py_launcher {
        pip_args.extend(["-3".into(), "-m".into(), "pip".into()]);
    } else {
        pip_args.extend(["-m".into(), "pip".into()]);
    }
    pip_args.extend([
        "install".into(),
        "--upgrade".into(),
        "--disable-pip-version-check".into(),
        "--target".into(),
        pkg.display().to_string(),
        "esptool".into(),
    ]);
    pip.args(&pip_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match pip.output() {
        Ok(out) if out.status.success() => {
            progress("esptool installed for Companion fallback".into());
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "pip install esptool failed: {}",
                trunc(&err, 180)
            ));
        }
        Err(e) => return Err(format!("pip launch failed: {e}")),
    }

    // Verify import via PYTHONPATH.
    {
        let mut probe = Command::new(&py);
        hide_console_window(&mut probe);
        probe.env("PYTHONPATH", &pkg);
        let mut args = base_args.clone();
        args.push("version".into());
        probe.args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        match probe.status() {
            Ok(status) if status.success() => Ok((py, base_args, Some(pkg.into_os_string()))),
            _ => Err(
                "esptool installed but still not importable — try: py -3 -m pip install esptool"
                    .into(),
            ),
        }
    }
}

fn trunc(s: &str, max: usize) -> String {
    crate::utf8_safe::trunc(s.trim(), max)
}

fn best_effort_reset(
    espflash: &Path,
    port: &str,
    progress: &dyn Fn(String),
    cancel: Option<&AtomicBool>,
) {
    progress("Force reset after failed flash (leave download mode)…".into());
    let _ = run_espflash_reset(espflash, port, progress, RESET_TIMEOUT, cancel);
}

fn safety_fail_tip(elapsed: Duration, esp_err: &str, py_err: &str, chip_may_be_blank: bool) -> String {
    let py = if py_err.is_empty() {
        String::new()
    } else {
        format!("; esptool: {py_err}")
    };
    if chip_may_be_blank {
        format!(
            "SAFETY: flash erase/rewrite did not finish cleanly after {}s (espflash: {esp_err}{py}). \
The board flash may be BLANK until a rewrite succeeds — keep USB connected. \
Hold BOOT, tap RESET, keep BOOT held, then click Ready and Update board again (or Flash-Firmware.bat). \
Browser fallback: https://espressif.github.io/esptool-js/ — pick Firmware\\merged.bin @ 0x0. \
See last-flash.log next to the app.",
            elapsed.as_secs()
        )
    } else {
        format!(
            "Flash failed after {}s (espflash: {esp_err}{py}). \
See last-flash.log next to the app. \
Hold BOOT, tap RESET, keep BOOT held, click Ready, then Update again. \
Or run Flash-Firmware.bat / https://espressif.github.io/esptool-js/ (merged.bin @ 0x0). \
Ensure Tools\\espflash.exe sits next to the app.",
            elapsed.as_secs()
        )
    }
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
    if l.contains("disconnected") || l.contains("not connected") || l.contains("failed to connect")
    {
        return false;
    }
    l.contains("mac:")
        || l.contains("mac address")
        || l.contains("chip type")
        || l.contains("chip is")
        || (l.contains("connected") && !l.contains("reconnect"))
        || l.contains("stub running")
        || l.contains("uploading stub")
        || l.contains("writing at")
        || l.contains("flash size")
        || l.contains("crystal is")
}


/// Read flash-tool stdout/stderr splitting on `\n` or `\r` (esptool progress bars).
fn pump_crlf_stream(out: impl Read, tx: Sender<String>) {
    let mut reader = BufReader::new(out);
    let mut buf: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) => {
                if !buf.is_empty() {
                    if let Ok(s) = std::str::from_utf8(&buf) {
                        let t = s.trim();
                        if !t.is_empty() {
                            let _ = tx.send(t.to_string());
                        }
                    }
                }
                break;
            }
            Ok(_) => {
                if byte[0] == b'\n' || byte[0] == b'\r' {
                    if !buf.is_empty() {
                        if let Ok(s) = std::str::from_utf8(&buf) {
                            let t = s.trim();
                            if !t.is_empty() {
                                let _ = tx.send(t.to_string());
                            }
                        }
                        buf.clear();
                    }
                } else if byte[0] >= 0x20 || byte[0] == b'\t' {
                    buf.push(byte[0]);
                    if buf.len() > 240 {
                        if let Ok(s) = std::str::from_utf8(&buf) {
                            let t = s.trim();
                            if !t.is_empty() {
                                let _ = tx.send(t.to_string());
                            }
                        }
                        buf.clear();
                    }
                }
            }
            Err(_) => break,
        }
    }
}

/// Run a flash helper with wall-clock + idle timeouts. Kills the child on expiry
/// so Update board cannot hang forever after espflash prints the chip MAC.
///
/// Safety vs ESP Terminator / esptool-js:
/// - Treat `\r` progress ticks as activity (erase/write bars often omit newlines)
/// - After pipes close, wait EXIT_GRACE for hard-reset/exit before killing
/// - Erase ops get a longer post-connect idle (full-chip erase is quiet)
/// - `patient`: Terminator-style — long idle after MAC (do not kill ~55s after connect)
fn run_streaming_timeout(
    cmd: &mut Command,
    progress: &dyn Fn(String),
    label: &str,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
    kind: FlashToolKind,
    patient: bool,
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
    let tx1 = tx.clone();
    let h1 = std::thread::spawn(move || {
        if let Some(out) = stdout {
            pump_crlf_stream(out, tx1);
        }
    });
    let tx2 = tx;
    let h2 = std::thread::spawn(move || {
        if let Some(out) = stderr {
            pump_crlf_stream(out, tx2);
        }
    });

    let deadline = Instant::now() + timeout;
    let mut tail = String::new();
    let mut last_beat = Instant::now();
    let mut last_status = Instant::now();
    let mut saw_connect = false;
    let mut saw_stub = false;
    let mut saw_write = matches!(kind, FlashToolKind::Erase);
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
            if patient {
                IDLE_DURING_WRITE_PATIENT
            } else {
                IDLE_DURING_WRITE
            }
        } else if saw_stub {
            // "Uploading stub" is not flash progress — CH340 often hangs here.
            IDLE_STUB_UPLOAD
        } else if saw_connect {
            if patient {
                IDLE_AFTER_CONNECT_PATIENT
            } else {
                match kind {
                    FlashToolKind::Erase => Duration::from_secs(90),
                    _ => IDLE_AFTER_CONNECT,
                }
            }
        } else if patient {
            // User may still be holding BOOT / waiting for Ready — don't kill early.
            Duration::from_secs(90)
        } else {
            IDLE_TIMEOUT
        };
        if last_beat.elapsed() >= idle_limit {
            return abort(
                format!(
                    "{label} idle {}s after {} — killing stuck flash tool{}",
                    idle_limit.as_secs(),
                    if saw_write {
                        "write/erase progress"
                    } else if saw_stub {
                        "stub upload"
                    } else if saw_connect {
                        "chip connect/MAC"
                    } else {
                        "start"
                    },
                    if (saw_connect || saw_stub) && !saw_write {
                        ". Hold BOOT, tap RESET, keep BOOT held, then click Ready."
                    } else {
                        ""
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
                        // Full-chip erase is quiet after MAC — don't demand write ticks ASAP.
                        if matches!(kind, FlashToolKind::Erase) {
                            progress(format!(
                                "{label}: chip seen — erasing (allowing up to 90s quiet)…"
                            ));
                            saw_write = true;
                        } else {
                            let wait_s = if patient {
                                IDLE_AFTER_CONNECT_PATIENT.as_secs()
                            } else {
                                IDLE_AFTER_CONNECT.as_secs()
                            };
                            progress(format!(
                                "{label}: chip seen — need write progress within {wait_s}s (or will abort)…"
                            ));
                        }
                    }
                }
                if lower.contains('%')
                    || lower.contains("writing")
                    || lower.contains("erasing")
                    || lower.contains("compressed")
                    || lower.contains("hash of data")
                    || lower.contains("chip erase")
                    || lower.contains("hard resetting")
                    || lower.contains("hard_reset")
                    || lower.contains("configuring flash")
                    || lower.contains("flash will be erased")
                    || lower.contains("flash_defl")
                    || lower.contains("wrote ")
                {
                    saw_write = true;
                } else if lower.contains("uploading stub")
                    || lower.contains("running stub")
                    || lower.contains("stub running")
                    || lower.contains("using flash stub")
                    || lower.contains("flash stub")
                {
                    // Stub phase ≠ write progress. Keep a short idle so we escalate.
                    if !saw_stub {
                        saw_stub = true;
                        progress(format!(
                            "{label}: stub upload — need Writing % within {}s…",
                            IDLE_STUB_UPLOAD.as_secs()
                        ));
                    }
                }
                // Tiny percent ticks still count as activity — forward to UI progress bar.
                if line.contains('%') && line.len() < 12 {
                    progress(line.clone());
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
                    crate::utf8_safe::keep_last(&mut tail, 400);
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
                                " (writing/erasing)"
                            } else if saw_stub {
                                " (stub upload)"
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

    // Pipes closed ≠ process finished (hard-reset / flush). Wait like esptool-js does.
    let grace = EXIT_GRACE.min(
        deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_secs(8)),
    );
    let grace_deadline = Instant::now() + grace;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if flash_cancelled(cancel) {
                    force_kill(&mut child);
                    return Err(format!("{label} cancelled after output closed"));
                }
                if Instant::now() >= grace_deadline {
                    force_kill(&mut child);
                    return Err(format!(
                        "{label} did not exit cleanly after output closed — killed (waited {}s)",
                        grace.as_secs()
                    ));
                }
                if last_status.elapsed() >= Duration::from_secs(4) {
                    progress(format!(
                        "{label}: output closed — waiting up to {}s for exit/hard-reset…",
                        grace_deadline
                            .saturating_duration_since(Instant::now())
                            .as_secs()
                            .max(1)
                    ));
                    last_status = Instant::now();
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => return Err(format!("{label} wait: {e}")),
        }
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
        let mut t = tail.to_string();
        crate::utf8_safe::keep_last(&mut t, 400);
        t
    }
}

#[cfg(test)]
mod tests {
    use super::{board_fw_is_d0, fw_version_key, scan_app_bin_fw_tag, update_needed};

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
            update_needed("0.8.56-sha256-d0", "0.8.77-sha256"),
            Some(true)
        );
        assert_eq!(update_needed("", "0.8.77-sha256"), None);
    }

    #[test]
    fn d0_flavor_detect() {
        assert!(board_fw_is_d0("0.8.174-sha256-d0"));
        assert!(board_fw_is_d0("esp32-2432s028-sha256-miner-d0.bin"));
        assert!(!board_fw_is_d0("0.8.175-sha256"));
    }

    #[test]
    fn scan_embedded_fw_tag() {
        let mut blob = vec![0u8; 64];
        blob.extend_from_slice(b"noise 0.8.175-sha256-d0 more");
        assert_eq!(
            scan_app_bin_fw_tag(&blob).as_deref(),
            Some("0.8.175-sha256-d0")
        );
    }
}
