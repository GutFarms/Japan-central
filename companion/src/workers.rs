//! Discover CYD workers on USB serial, Wi‑Fi (UDP beacon + TCP cmp),
//! and Companion peers on the LAN.

use serde::{Deserialize, Serialize};
use serialport::{SerialPort, SerialPortType};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

pub const LAN_DISCOVERY_PORT: u16 = 19283;
pub const LAN_MAGIC: &str = "CYDCOMPANION";
pub const BOARD_WIFI_PORT: u16 = 19284;
pub const BOARD_WIFI_MAGIC: &str = "CYDBOARD";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerKind {
    Usb,
    Wifi,
    Lan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredWorker {
    pub id: String,
    pub kind: WorkerKind,
    /// COM port path, or `host:port` for Wi‑Fi / LAN peers.
    pub endpoint: String,
    /// Board eFuse Wi‑Fi STA MAC (`aa:bb:…`) when known — stable board identity.
    #[serde(default)]
    pub mac: String,
    pub fw: String,
    pub detail: String,
    pub host: String,
    #[serde(skip)]
    pub last_seen_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct WorkerLive {
    pub endpoint: String,
    pub mac: String,
    pub fw: String,
    pub connected: bool,
    pub hashrate_hs: f64,
    pub hashes: u64,
    pub mining: bool,
    /// USB root currently bridging ESP-NOW mesh peers (hash rate intentionally reduced).
    pub mesh_bridging: bool,
    pub mesh_peers: u8,
}

/// One OS serial port with a human-readable label (USB chip / product).
#[derive(Debug, Clone)]
pub struct PortChoice {
    pub name: String,
    pub label: String,
}

/// True when the OS labeled this as a motherboard PCI UART (never a CYD).
///
/// Match the deliberate `"— PCI"` tag from `port_info_to_choice` — do **not**
/// use a bare `"PCI"` substring (USB product strings occasionally contain it).
pub fn port_choice_is_pci(p: &PortChoice) -> bool {
    let u = p.label.to_ascii_uppercase();
    u.contains("— PCI") || u.contains("- PCI") || u.contains("PCI (NOT A CYD)")
}

/// Motherboard / non-CYD serial — never use for Connect / Update / flash.
/// COM1 is almost always the PC's built-in UART on Windows.
pub fn port_choice_is_system_junk(p: &PortChoice) -> bool {
    if port_choice_is_pci(p) || cyd_port_score(p) < 0 {
        return true;
    }
    let n = normalize_port_name(&p.name);
    // Bare COM1 with no USB/CH340 hint — hide from flash/update.
    if n == "COM1" {
        let l = p.label.to_ascii_lowercase();
        if !(l.contains("ch340")
            || l.contains("cp210")
            || l.contains("ftdi")
            || l.contains("wch")
            || l.contains("silicon")
            || (l.contains("usb") && !l.contains("pci")))
        {
            return true;
        }
    }
    false
}

/// Ports safe to offer for CYD connect / Update board / flash.
pub fn flashable_ports(ports: &[PortChoice]) -> Vec<&PortChoice> {
    ports
        .iter()
        .filter(|p| is_usb_serial_port(&p.name) && !port_choice_is_system_junk(p))
        .collect()
}

/// Score higher for likely CYD USB-UART adapters (CH340 / CP210x / …).
pub fn cyd_port_score(p: &PortChoice) -> i32 {
    if !is_usb_serial_port(&p.name) || port_choice_is_pci(p) {
        return -100;
    }
    let l = p.label.to_ascii_lowercase();
    // Bluetooth serial never speaks `cmp` and hangs Windows opens — never offer it.
    if l.contains("bluetooth") {
        return -100;
    }
    let mut s = 10;
    if l.contains("ch340") || l.contains("wch") {
        s += 50;
    }
    if l.contains("cp210") || l.contains("silicon labs") {
        s += 45;
    }
    if l.contains("ftdi") || l.contains("usb serial") || l.contains("usb-enhanced") {
        s += 40;
    }
    if l.contains("usb") {
        s += 20;
    }
    // Registry / Unknown entries (common for CH340) — still prefer over PCI.
    if l.contains("registry") || l.contains("— serial") || l.ends_with(" serial") {
        s += 25;
    }
    // Higher COM numbers are often the plugged-in dongle vs COM1/COM3 system ports.
    if let Some(n) = normalize_port_name(&p.name)
        .strip_prefix("COM")
        .and_then(|x| x.parse::<u32>().ok())
    {
        if n >= 4 {
            s += 5;
        }
        if n >= 6 {
            s += 3;
        }
    }
    s
}

/// Best COM for a CYD: real USB-UART, never PCI / bare COM1.
pub fn prefer_cyd_port(ports: &[PortChoice]) -> Option<&PortChoice> {
    prefer_cyd_port_excluding(ports, &[])
}

/// Best CYD USB-UART excluding already-linked endpoints (for Add board / multi-USB).
pub fn prefer_cyd_port_excluding<'a>(
    ports: &'a [PortChoice],
    exclude: &[String],
) -> Option<&'a PortChoice> {
    ports
        .iter()
        .filter(|p| !port_choice_is_system_junk(p) && cyd_port_score(p) >= 0)
        .filter(|p| !exclude.iter().any(|e| port_names_match(e, &p.name)))
        .max_by_key(|p| cyd_port_score(p))
}

/// Count USB-UART style COMs (excludes motherboard PCI / bare COM1).
pub fn count_usb_uart_ports(ports: &[PortChoice]) -> usize {
    ports
        .iter()
        .filter(|p| !port_choice_is_system_junk(p) && cyd_port_score(p) >= 0)
        .count()
}

fn port_info_to_choice(p: serialport::SerialPortInfo) -> PortChoice {
    let label = match &p.port_type {
        SerialPortType::UsbPort(usb) => {
            let mut bits: Vec<String> = Vec::new();
            if let Some(m) = usb.manufacturer.as_ref() {
                let t = m.trim();
                if !t.is_empty() {
                    bits.push(t.to_string());
                }
            }
            if let Some(prod) = usb.product.as_ref() {
                let t = prod.trim();
                if !t.is_empty() {
                    bits.push(t.to_string());
                }
            }
            if let Some(sn) = usb.serial_number.as_ref() {
                let t = sn.trim();
                if !t.is_empty() {
                    bits.push(format!("SN {t}"));
                }
            }
            if bits.is_empty() {
                format!("{} — USB {:04X}:{:04X}", p.port_name, usb.vid, usb.pid)
            } else {
                format!("{} — {}", p.port_name, bits.join(" · "))
            }
        }
        SerialPortType::PciPort => format!("{} — PCI (not a CYD)", p.port_name),
        SerialPortType::BluetoothPort => format!("{} — Bluetooth", p.port_name),
        SerialPortType::Unknown => {
            // Many CH340s show as Unknown — still usable.
            format!("{} — serial", p.port_name)
        }
    };
    PortChoice {
        name: p.port_name,
        label,
    }
}

/// Windows live COM map (`HARDWARE\DEVICEMAP\SERIALCOMM`) — present devices only.
#[cfg(windows)]
fn windows_serialcomm_ports() -> Vec<(String, String)> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::types::FromRegValue;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let Ok(key) = hklm.open_subkey_with_flags(r"HARDWARE\DEVICEMAP\SERIALCOMM", KEY_READ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in key.enum_values() {
        let Ok((device, value)) = item else {
            continue;
        };
        let Ok(com) = String::from_reg_value(&value) else {
            continue;
        };
        let com = com.trim().to_ascii_uppercase();
        if com.starts_with("COM") && com.len() > 3 && com[3..].chars().all(|c| c.is_ascii_digit()) {
            let hint = if device.contains('\\') {
                format!("registry · {}", device.rsplit('\\').next().unwrap_or(&device))
            } else {
                format!("registry · {device}")
            };
            out.push((com, hint));
        }
    }
    out
}

/// Windows Enum PortName labels (USB/FTDI/…) for COMs that are already live.
///
/// Never used to *invent* COMs — Enum keeps PortName after unplug and caused
/// ghost rows like COM7 that fail with "system cannot find the file".
#[cfg(windows)]
fn windows_enum_com_labels() -> Vec<(String, String)> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    fn read_port_name(key: &RegKey) -> Option<String> {
        key.get_value::<String, _>("PortName")
            .ok()
            .or_else(|| {
                key.open_subkey("Device Parameters")
                    .ok()
                    .and_then(|dp| dp.get_value::<String, _>("PortName").ok())
            })
            .map(|s| s.trim().to_ascii_uppercase())
            .filter(|s| {
                s.starts_with("COM") && s.len() > 3 && s[3..].chars().all(|c| c.is_ascii_digit())
            })
    }

    fn walk_enum(key: &RegKey, depth: u8, hint: &str, out: &mut Vec<(String, String)>) {
        if depth > 5 {
            return;
        }
        if let Some(com) = read_port_name(key) {
            let u = hint.to_ascii_uppercase();
            let chip = if u.contains("VID_1A86") || u.contains("PID_7523") {
                "CH340/WCH"
            } else if u.contains("VID_10C4") || u.contains("CP210") {
                "CP210x"
            } else if u.contains("VID_0403") || u.contains("FTDI") {
                "FTDI"
            } else if u.contains("VID_067B") {
                "Prolific"
            } else {
                "USB"
            };
            out.push((com, format!("enum · {chip} · {hint}")));
        }
        let Ok(subs) = key.enum_keys().collect::<Result<Vec<_>, _>>() else {
            return;
        };
        for sub in subs {
            if let Ok(child) = key.open_subkey_with_flags(&sub, KEY_READ) {
                let child_hint = if hint.is_empty() {
                    sub.clone()
                } else {
                    format!("{hint}\\{sub}")
                };
                let short = if child_hint.len() > 48 {
                    child_hint
                        .rsplit('\\')
                        .take(2)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .join("\\")
                } else {
                    child_hint
                };
                walk_enum(&child, depth + 1, &short, out);
            }
        }
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut out = Vec::new();
    for path in [
        r"SYSTEM\CurrentControlSet\Enum\USB",
        r"SYSTEM\CurrentControlSet\Enum\FTDIBUS",
        r"SYSTEM\CurrentControlSet\Enum\USBSSER",
        r"SYSTEM\CurrentControlSet\Enum\PORTS",
    ] {
        if let Ok(root) = hklm.open_subkey_with_flags(path, KEY_READ) {
            let leaf = path.rsplit('\\').next().unwrap_or(path);
            walk_enum(&root, 0, leaf, &mut out);
        }
    }
    out
}

#[cfg(not(windows))]
fn windows_serialcomm_ports() -> Vec<(String, String)> {
    Vec::new()
}

#[cfg(not(windows))]
fn windows_enum_com_labels() -> Vec<(String, String)> {
    Vec::new()
}

/// Prefer `\\.\COMx` for Windows CreateFile (required for COM10+; helps COM1–9 too).
pub fn windows_com_path(name: &str) -> String {
    let n = normalize_port_name(name);
    if n.starts_with("COM") && n.len() > 3 && n[3..].chars().all(|c| c.is_ascii_digit()) {
        format!(r"\\.\{n}")
    } else {
        name.trim().to_string()
    }
}

/// List every *live* serial port the OS reports.
///
/// On Windows: SetupAPI + live SERIALCOMM. Enum PortName only enriches labels —
/// it must not add ghost COMs from unplugged devices.
pub fn list_serial_ports() -> Vec<PortChoice> {
    use std::collections::BTreeMap;

    let mut by_name: BTreeMap<String, PortChoice> = BTreeMap::new();
    for info in serialport::available_ports().unwrap_or_default() {
        let choice = port_info_to_choice(info);
        by_name.insert(normalize_port_name(&choice.name), choice);
    }
    // Live device map — add COMs SetupAPI missed (2nd CH340), never ghosts.
    for (com, hint) in windows_serialcomm_ports() {
        let key = normalize_port_name(&com);
        by_name.entry(key).or_insert_with(|| PortChoice {
            name: com.clone(),
            label: format!("{com} — {hint}"),
        });
    }
    // Enrich labels only for COMs already live.
    for (com, hint) in windows_enum_com_labels() {
        let key = normalize_port_name(&com);
        if let Some(existing) = by_name.get_mut(&key) {
            let weak = existing.label.to_ascii_lowercase().contains("— serial")
                || existing.label.to_ascii_lowercase().contains("registry ·");
            if weak || hint.contains("CH340") || hint.contains("CP210") || hint.contains("FTDI") {
                if !existing.label.to_ascii_lowercase().contains("ch340")
                    && !existing.label.to_ascii_lowercase().contains("cp210")
                    && !existing.label.to_ascii_lowercase().contains("ftdi")
                {
                    existing.label = format!("{com} — {hint}");
                }
            }
        }
    }
    by_name.into_values().collect()
}

/// Normalize MAC for ids / display (`aabbccddeeff` → `aa:bb:cc:dd:ee:ff`).
pub fn normalize_mac(raw: &str) -> String {
    let hex: String = raw
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if hex.len() != 12 {
        return raw.trim().to_ascii_lowercase();
    }
    format!(
        "{}:{}:{}:{}:{}:{}",
        &hex[0..2],
        &hex[2..4],
        &hex[4..6],
        &hex[6..8],
        &hex[8..10],
        &hex[10..12]
    )
}

/// Normalize COM / serial path for comparisons (`\\.\COM10` ↔ `COM10`).
pub fn normalize_port_name(name: &str) -> String {
    let t = name.trim();
    let stripped = t.strip_prefix(r"\\.\").unwrap_or(t);
    stripped.to_ascii_uppercase()
}

pub fn port_names_match(a: &str, b: &str) -> bool {
    normalize_port_name(a) == normalize_port_name(b)
}

/// True when `name` is a local USB/UART serial device (not Wi‑Fi / LAN TCP).
pub fn is_usb_serial_port(name: &str) -> bool {
    let t = name.trim();
    if t.is_empty() {
        return false;
    }
    // host:port → network worker
    if t.contains('.') && t.contains(':') {
        return false;
    }
    if t.matches(':').count() == 1 {
        let mut parts = t.splitn(2, ':');
        let host = parts.next().unwrap_or("");
        let port = parts.next().unwrap_or("");
        if !host.is_empty()
            && port.chars().all(|c| c.is_ascii_digit())
            && !host.eq_ignore_ascii_case("COM")
        {
            // "192.168.4.1:19284" or "hostname:19284"
            if host.contains('.') || host.chars().any(|c| c.is_ascii_alphabetic()) {
                return false;
            }
        }
    }
    let n = normalize_port_name(t);
    if n.starts_with("COM") && n.len() > 3 && n[3..].chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    lower.contains("ttyusb")
        || lower.contains("ttyacm")
        || lower.contains("usbserial")
        || lower.contains("wchusb")
        || lower.contains("usbmodem")
        || lower.contains("slab_uspto")
}

/// Port string for espflash/esptool.
///
/// espflash resolves ports with an *exact* match against `serialport::available_ports()`.
/// Passing `\\.\COM6` when the OS lists `COM6` yields `espflash::serial_not_found`.
/// Always prefer the enumerated name; fall back to plain `COMx`.
pub fn flash_port_arg(name: &str) -> String {
    let want = normalize_port_name(name);
    if let Ok(ports) = serialport::available_ports() {
        for p in ports {
            if normalize_port_name(&p.port_name) == want {
                return p.port_name;
            }
        }
    }
    if want.starts_with("COM") && want.len() > 3 && want[3..].chars().all(|c| c.is_ascii_digit()) {
        return want;
    }
    name.trim().to_string()
}

/// True when `mac` is a real 6-byte identity (not empty / `unknown`).
pub fn mac_is_stable(mac: &str) -> bool {
    let hex: String = normalize_mac(mac)
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    hex.len() == 12
}

pub fn mac_worker_id(mac: &str) -> String {
    if !mac_is_stable(mac) {
        String::new()
    } else {
        format!("usb:mac:{}", normalize_mac(mac))
    }
}

/// Stable discovery id that keeps USB and Wi‑Fi rows distinct for the same board.
pub fn transport_mac_id(kind: WorkerKind, mac: &str) -> String {
    if !mac_is_stable(mac) {
        return String::new();
    }
    let prefix = match kind {
        WorkerKind::Wifi => "wifi",
        WorkerKind::Lan => "lan",
        WorkerKind::Usb => "usb",
    };
    format!("{prefix}:mac:{}", normalize_mac(mac))
}

/// Open a USB-UART port without asserting DTR (ESP32 often resets on DTR/RTS).
pub fn open_usb_serial(
    name: &str,
    baud: u32,
    timeout: Duration,
) -> Result<Box<dyn SerialPort>, String> {
    let plain = normalize_port_name(name);
    let extended = windows_com_path(&plain);
    let candidates: Vec<String> = if plain.eq_ignore_ascii_case(&extended) {
        vec![plain.clone()]
    } else if plain.starts_with("COM") {
        // Try extended path first on Windows — plain COMx fails with
        // "The system cannot find the file specified" on some hosts / COM10+.
        vec![extended, plain.clone()]
    } else {
        vec![name.trim().to_string()]
    };

    let mut last_err = String::new();
    for path in &candidates {
        match serialport::new(path.as_str(), baud)
            .timeout(timeout)
            .dtr_on_open(false)
            .open()
        {
            Ok(mut port) => {
                let _ = port.write_data_terminal_ready(false);
                let _ = port.write_request_to_send(false);
                // Settle after open; CH340/CP210x may still glitch / reset the MCU.
                std::thread::sleep(Duration::from_millis(350));
                let _ = port.clear(serialport::ClearBuffer::All);
                return Ok(port);
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    let low = last_err.to_ascii_lowercase();
    let hint = if low.contains("cannot find the file")
        || low.contains("the system cannot find")
        || low.contains("os error 2")
    {
        format!(
            " — {plain} is not a live Windows COM (ghost registry entry, unplugged, or wrong port). \
Refresh the COM list and pick a port shown in Device Manager → Ports."
        )
    } else if low.contains("access is denied") || low.contains("os error 5") {
        format!(
            " — {plain} is busy (another app has it open). Close Arduino IDE / serial monitors, then retry."
        )
    } else {
        String::new()
    };
    Err(format!("USB open failed @ {baud} on {plain}: {last_err}{hint}"))
}

fn slip_encode(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 8);
    out.push(0xC0);
    for &b in payload {
        match b {
            0xC0 => {
                out.push(0xDB);
                out.push(0xDC);
            }
            0xDB => {
                out.push(0xDB);
                out.push(0xDD);
            }
            _ => out.push(b),
        }
    }
    out.push(0xC0);
    out
}

/// True when the UART answers ESP ROM SYNC (BOOT held / blank / download mode).
/// Safe to call after `cmp ping` failed — does not require running firmware.
pub fn probe_esp_download_mode(port: &mut dyn SerialPort) -> bool {
    let _ = port.clear(serialport::ClearBuffer::All);
    // ESP_SYNC (0x08): direction=0, cmd=8, size=36, checksum=0, data=07 07 12 20 + 32×55
    let mut data = vec![0x07u8, 0x07, 0x12, 0x20];
    data.extend(std::iter::repeat(0x55u8).take(32));
    let mut hdr = vec![0x00u8, 0x08, 36, 0, 0, 0, 0, 0];
    hdr.extend_from_slice(&data);
    let frame = slip_encode(&hdr);
    let mut buf = [0u8; 256];
    for _ in 0..6 {
        let _ = port.write_all(&frame);
        let _ = port.flush();
        let deadline = Instant::now() + Duration::from_millis(120);
        while Instant::now() < deadline {
            match port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    // ROM replies with SLIP (0xC0…) — presence is enough to claim download mode.
                    if buf[..n].contains(&0xC0) {
                        return true;
                    }
                }
                _ => std::thread::sleep(Duration::from_millis(8)),
            }
        }
    }
    false
}

/// Open with a hard deadline — motherboard PCI COM ports can hang forever in `open()`.
pub fn open_usb_serial_timed(
    name: &str,
    baud: u32,
    io_timeout: Duration,
    open_deadline: Duration,
) -> Result<Box<dyn SerialPort>, String> {
    let name_owned = name.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(open_usb_serial(&name_owned, baud, io_timeout));
    });
    match rx.recv_timeout(open_deadline) {
        Ok(r) => r,
        Err(_) => Err(format!(
            "USB open timed out after {}ms on {name} @ {baud} (port hung?)",
            open_deadline.as_millis()
        )),
    }
}

fn wait_for_pong(port: &mut dyn SerialPort, buf: &mut String, wait_ms: u64) -> Option<String> {
    let _ = port.write_all(b"\r\ncmp ping\r\n");
    let _ = port.flush();
    let deadline = Instant::now() + Duration::from_millis(wait_ms);
    while Instant::now() < deadline {
        drain(port, buf);
        if let Some(line) = buf.lines().find(|l| {
            let t = l.trim();
            t.starts_with("CMP ok") || t.eq_ignore_ascii_case("CMPACK ping")
        }) {
            return Some(line.trim().to_string());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// Probe a serial port for CYD companion firmware (`cmp ping` → `CMP ok`).
#[allow(dead_code)]
pub fn probe_usb_port(name: &str) -> Option<DiscoveredWorker> {
    probe_usb_port_detailed(name).ok()
}

/// Like [`probe_usb_port`], but returns why a port was skipped (for Event log).
#[allow(dead_code)]
pub fn probe_usb_port_detailed(name: &str) -> Result<DiscoveredWorker, String> {
    let mut last = String::new();
    for baud in [460_800u32, 115_200] {
        match probe_usb_port_at(name, baud) {
            Ok(w) => return Ok(w),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn probe_usb_port_at(name: &str, baud: u32) -> Result<DiscoveredWorker, String> {
    let mut port = open_usb_serial_timed(
        name,
        baud,
        Duration::from_millis(80),
        Duration::from_secs(3),
    )?;
    let mut buf = String::new();
    // SoftAP + splash can delay usbTask after a UART reset — retry with boot waits.
    let ping_ms = if baud > 115_200 { 1_500 } else { 2_500 };
    let mut pong = None;
    for attempt in 0..3u32 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(1_800));
            let _ = port.clear(serialport::ClearBuffer::All);
            buf.clear();
        }
        pong = wait_for_pong(port.as_mut(), &mut buf, ping_ms + attempt as u64 * 400);
        if pong.is_some() {
            break;
        }
    }
    let pong_line = pong.ok_or_else(|| format!("no CMP ok @ {baud} on {name}"))?;
    let mut mac = String::new();
    if let Some(rest) = pong_line.split_once("mac=") {
        mac = normalize_mac(rest.1.trim());
    }

    let mut fw = String::new();
    let mut mode = String::new();
    let _ = port.write_all(b"cmp config\r\n");
    let _ = port.flush();
    let cfg_deadline = Instant::now() + Duration::from_millis(1_500);
    while Instant::now() < cfg_deadline {
        drain(port.as_mut(), &mut buf);
        if let Some(line) = buf
            .lines()
            .rev()
            .find(|l| l.trim().starts_with('{') || l.trim().starts_with("CMPCONFIG "))
        {
            let json = line
                .trim()
                .strip_prefix("CMPCONFIG ")
                .unwrap_or(line.trim());
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
                fw = v
                    .get("fw")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                mode = v
                    .get("mode")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(m) = v.get("mac").and_then(|x| x.as_str()) {
                    if !m.is_empty() {
                        mac = normalize_mac(m);
                    }
                }
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    // Always key USB discovery by COM — cheap CYD clones can share an eFuse MAC;
    // MAC-based ids collapsed two boards into one Find-workers row.
    let id = format!("usb:{}", normalize_port_name(name));
    let detail = if fw.is_empty() {
        format!("CYD companion USB · pong @ {baud}")
    } else if mode.is_empty() {
        format!("fw {fw} @ {baud}")
    } else {
        format!("fw {fw} · {mode} @ {baud}")
    };

    Ok(DiscoveredWorker {
        id,
        kind: WorkerKind::Usb,
        endpoint: name.to_string(),
        mac,
        fw,
        detail,
        host: "local".into(),
        last_seen_ms: now_ms(),
    })
}

/// Ports worth probing for CYD boards.
///
/// Uses `list_serial_ports` (Windows SERIALCOMM merge) so a 2nd CH340 is not
/// missed when SetupAPI only reports one COM. Skip PCI / Bluetooth / bare COM1.
fn scan_candidate_ports() -> Vec<(String, WorkerKind)> {
    let mut out: Vec<(String, WorkerKind)> = Vec::new();
    for p in list_serial_ports() {
        if port_choice_is_system_junk(&p) {
            continue;
        }
        if !is_usb_serial_port(&p.name) {
            continue;
        }
        out.push((p.name.clone(), WorkerKind::Usb));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Scan serial ports for CYD boards. Skips ports listed in `skip`.
#[allow(dead_code)]
pub fn scan_usb_workers(skip: &[String]) -> Vec<DiscoveredWorker> {
    scan_usb_workers_with_progress(skip, |_, _| {})
}

/// Serial USB probe (one port at a time) with per-port progress + result callback.
///
/// `on_port(port, detail)` — detail is `"probing"`, `"ok · …"`, or `"miss · …"`.
#[allow(dead_code)]
pub fn scan_usb_workers_with_progress(
    skip: &[String],
    on_port: impl Fn(&str, &str),
) -> Vec<DiscoveredWorker> {
    let mut found = Vec::new();

    // USB only — never parallel: shared hubs brown out / reset both CYDs.
    // Bluetooth serial is excluded (hangs opens; does not speak `cmp`).
    let usb_candidates: Vec<(String, WorkerKind)> = scan_candidate_ports()
        .into_iter()
        .filter(|(name, _)| !skip.iter().any(|s| port_names_match(s, name)))
        .collect();
    for (name, kind) in usb_candidates {
        on_port(&name, "probing");
        match probe_usb_port_detailed(&name) {
            Ok(mut w) => {
                w.kind = kind;
                on_port(&name, &format!("ok · {}", w.detail));
                found.push(w);
            }
            Err(e) => {
                on_port(&name, &format!("miss · {e}"));
                // Still surface the COM so the UI shows every USB port Windows listed,
                // even when the board is blank / download-mode / not speaking cmp yet.
                let id = format!("usb-present:{}", normalize_port_name(&name));
                found.push(DiscoveredWorker {
                    id,
                    kind,
                    endpoint: name.clone(),
                    mac: String::new(),
                    fw: String::new(),
                    detail: format!("USB COM present · no cmp ({e})"),
                    host: "local".into(),
                    last_seen_ms: now_ms(),
                });
            }
        }
    }

    found.sort_by(|a, b| a.endpoint.cmp(&b.endpoint));
    found
}

/// Probe a Wi‑Fi board over TCP using the same `cmp` line protocol.
pub fn probe_wifi_endpoint(endpoint: &str) -> Option<DiscoveredWorker> {
    let mut stream = TcpStream::connect_timeout(
        &endpoint
            .to_socket_addrs()
            .ok()?
            .next()?,
        Duration::from_secs(2),
    )
    .ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(800)));
    let _ = stream.set_nodelay(true);
    let _ = stream.write_all(b"\r\ncmp ping\r\n");
    let _ = stream.flush();
    let mut buf = String::new();
    let deadline = Instant::now() + Duration::from_millis(2_000);
    let mut mac = String::new();
    let mut saw = false;
    while Instant::now() < deadline {
        let mut tmp = [0u8; 512];
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.push_str(&String::from_utf8_lossy(&tmp[..n])),
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
        if let Some(line) = buf.lines().find(|l| {
            let t = l.trim();
            t.starts_with("CMP ok") || t.eq_ignore_ascii_case("CMPACK ping")
        }) {
            saw = true;
            if let Some(rest) = line.trim().split_once("mac=") {
                mac = normalize_mac(rest.1.trim());
            }
            break;
        }
    }
    if !saw {
        return None;
    }
    let _ = stream.write_all(b"cmp config\r\n");
    let _ = stream.flush();
    let mut fw = String::new();
    let cfg_deadline = Instant::now() + Duration::from_millis(1_500);
    while Instant::now() < cfg_deadline {
        let mut tmp = [0u8; 512];
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.push_str(&String::from_utf8_lossy(&tmp[..n])),
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
        if let Some(line) = buf
            .lines()
            .rev()
            .find(|l| l.trim().starts_with('{') || l.trim().starts_with("CMPCONFIG "))
        {
            let json = line
                .trim()
                .strip_prefix("CMPCONFIG ")
                .unwrap_or(line.trim());
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
                fw = v
                    .get("fw")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(m) = v.get("mac").and_then(|x| x.as_str()) {
                    if !m.is_empty() {
                        mac = normalize_mac(m);
                    }
                }
                break;
            }
        }
    }
    let id = {
        let mid = transport_mac_id(WorkerKind::Wifi, &mac);
        if mid.is_empty() {
            format!("wifi:{endpoint}")
        } else {
            mid
        }
    };
    Some(DiscoveredWorker {
        id,
        kind: WorkerKind::Wifi,
        endpoint: endpoint.to_string(),
        mac,
        fw: fw.clone(),
        detail: if fw.is_empty() {
            format!("Wi‑Fi TCP :{BOARD_WIFI_PORT}")
        } else {
            format!("fw {fw} · Wi‑Fi TCP")
        },
        host: endpoint.split(':').next().unwrap_or(endpoint).into(),
        last_seen_ms: now_ms(),
    })
}

pub fn open_wifi_tcp(endpoint: &str) -> Result<TcpStream, String> {
    let addr = endpoint
        .to_socket_addrs()
        .map_err(|e| format!("resolve {endpoint}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address for {endpoint}"))?;
    let host = endpoint.split(':').next().unwrap_or(endpoint);
    // SoftAP 10.x and busy LAN boards may need a bit longer than 3s.
    let timeout = if host.starts_with("10.") {
        Duration::from_secs(5)
    } else {
        Duration::from_secs(4)
    };
    let stream = TcpStream::connect_timeout(&addr, timeout).map_err(|e| {
        let softap_hint = if host.starts_with("10.") {
            " — SoftAP IP: join open Njordr-XXXX in Windows Wi‑Fi (no password), or Link over USB instead"
        } else {
            " — board offline / wrong network / firewall"
        };
        format!("TCP connect {endpoint}: {e}{softap_hint}")
    })?;
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(2_000)));
    Ok(stream)
}

/// Listen for board Wi‑Fi UDP beacons (`CYDBOARD|…`).
pub struct BoardWifiDiscovery {
    sock: Option<UdpSocket>,
}

impl BoardWifiDiscovery {
    pub fn start() -> Self {
        // Single listener on BOARD_WIFI_PORT. ScanWorkers must not create a second
        // bind (ephemeral fallback never receives CYDBOARD beacons).
        let sock = UdpSocket::bind(format!("0.0.0.0:{BOARD_WIFI_PORT}"))
            .ok()
            .and_then(|s| {
                s.set_broadcast(true).ok()?;
                s.set_nonblocking(true).ok()?;
                Some(s)
            });
        Self { sock }
    }

    pub fn poll_boards(&mut self) -> Vec<DiscoveredWorker> {
        let Some(sock) = self.sock.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut buf = [0u8; 1500];
        loop {
            match sock.recv_from(&mut buf) {
                Ok((n, addr)) => {
                    if let Some(w) = parse_board_beacon(&String::from_utf8_lossy(&buf[..n]), addr) {
                        out.push(w);
                    }
                }
                Err(_) => break,
            }
        }
        out
    }
}

fn parse_board_beacon(raw: &str, addr: SocketAddr) -> Option<DiscoveredWorker> {
    let line = raw.trim();
    if !line.starts_with(BOARD_WIFI_MAGIC) {
        return None;
    }
    let mut mac = String::new();
    let mut fw = String::new();
    let mut tcp = BOARD_WIFI_PORT;
    let mut ip = addr.ip().to_string();
    let mut ap = String::new();
    let mut mode = String::new();
    for part in line.split('|').skip(1) {
        if let Some((k, v)) = part.split_once('=') {
            match k {
                "mac" => mac = normalize_mac(v),
                "fw" => fw = v.to_string(),
                "tcp" => tcp = v.parse().unwrap_or(BOARD_WIFI_PORT),
                "ip" => {
                    if !v.is_empty() {
                        ip = v.to_string();
                    }
                }
                "ap" => ap = v.to_string(),
                "mode" => mode = v.to_string(),
                _ => {}
            }
        }
    }
    let endpoint = format!("{ip}:{tcp}");
    let id = {
        let mid = transport_mac_id(WorkerKind::Wifi, &mac);
        if mid.is_empty() {
            format!("wifi:{endpoint}")
        } else {
            mid
        }
    };
    let detail = match mode.as_str() {
        "ap" if !ap.is_empty() => {
            if fw.is_empty() {
                format!("setup SoftAP {ap} · open (no password)")
            } else {
                format!("fw {fw} · setup SoftAP {ap} · open (no password)")
            }
        }
        _ => match (fw.is_empty(), ap.is_empty()) {
            (false, false) => format!("fw {fw} · Wi‑Fi {mode} · AP {ap}"),
            (false, true) => format!("fw {fw} · Wi‑Fi {mode}"),
            (true, false) => format!("Wi‑Fi board · AP {ap}"),
            _ => "Wi‑Fi CYD board".into(),
        },
    };
    Some(DiscoveredWorker {
        id,
        kind: WorkerKind::Wifi,
        endpoint,
        mac,
        fw,
        detail,
        host: ip,
        last_seen_ms: now_ms(),
    })
}

fn drain(port: &mut dyn SerialPort, buf: &mut String) {
    let mut tmp = [0u8; 512];
    for _ in 0..20 {
        match port.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.push_str(&String::from_utf8_lossy(&tmp[..n])),
        }
    }
    if buf.len() > 16_000 {
        let keep = buf[buf.len() - 8_000..].to_string();
        *buf = keep;
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Age of a discovery beacon/probe row in milliseconds (`0` = unknown / synthetic).
pub fn discovery_age_ms(w: &DiscoveredWorker) -> u64 {
    if w.last_seen_ms == 0 {
        return u64::MAX;
    }
    now_ms().saturating_sub(w.last_seen_ms)
}

/// SoftAP setup beacon — only reachable after the PC joins Njordr-XXXX.
pub fn wifi_is_softap_setup(w: &DiscoveredWorker) -> bool {
    w.detail.to_ascii_lowercase().contains("setup softap")
}

/// True when a Wi‑Fi beacon is recent enough to attempt TCP link.
pub fn wifi_beacon_fresh(w: &DiscoveredWorker, max_age_ms: u64) -> bool {
    w.kind == WorkerKind::Wifi && discovery_age_ms(w) <= max_age_ms
}

/// LAN beacon + listener for other Companion instances advertising CYD workers.
pub struct LanDiscovery {
    sock: Option<UdpSocket>,
    last_beacon: Instant,
}

impl LanDiscovery {
    pub fn start() -> Self {
        let sock = UdpSocket::bind(format!("0.0.0.0:{LAN_DISCOVERY_PORT}"))
            .or_else(|_| UdpSocket::bind("0.0.0.0:0"))
            .ok()
            .and_then(|s| {
                s.set_broadcast(true).ok()?;
                s.set_nonblocking(true).ok()?;
                s.set_read_timeout(Some(Duration::from_millis(1))).ok()?;
                Some(s)
            });
        Self {
            sock,
            last_beacon: Instant::now() - Duration::from_secs(30),
        }
    }

    pub fn poll_peers(&mut self) -> Vec<DiscoveredWorker> {
        let Some(sock) = self.sock.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut buf = [0u8; 1500];
        loop {
            match sock.recv_from(&mut buf) {
                Ok((n, addr)) => {
                    if let Some(w) = parse_beacon(&String::from_utf8_lossy(&buf[..n]), addr) {
                        out.push(w);
                    }
                }
                Err(_) => break,
            }
        }
        out
    }

    pub fn maybe_beacon(&mut self, local_boards: &[(String, String, String)], host: &str) {
        if self.last_beacon.elapsed() < Duration::from_secs(4) {
            return;
        }
        self.last_beacon = Instant::now();
        let Some(sock) = self.sock.as_ref() else {
            return;
        };
        // boards=COM3@fw@mac,COM4@fw@mac
        let boards = local_boards
            .iter()
            .map(|(p, fw, mac)| {
                let mut s = p.clone();
                if !fw.is_empty() {
                    s.push('@');
                    s.push_str(fw);
                }
                if !mac.is_empty() {
                    s.push('@');
                    s.push_str(mac);
                }
                s
            })
            .collect::<Vec<_>>()
            .join(",");
        let msg = format!(
            "{LAN_MAGIC}|v={}|host={}|boards={}",
            env!("CARGO_PKG_VERSION"),
            sanitize(host),
            boards
        );
        let _ = sock.send_to(msg.as_bytes(), format!("255.255.255.255:{LAN_DISCOVERY_PORT}"));
        let _ = sock.send_to(msg.as_bytes(), format!("224.0.0.1:{LAN_DISCOVERY_PORT}"));
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect()
}

fn parse_beacon(raw: &str, addr: SocketAddr) -> Option<DiscoveredWorker> {
    let line = raw.trim();
    if !line.starts_with(LAN_MAGIC) {
        return None;
    }
    let mut host = addr.ip().to_string();
    let mut boards = String::new();
    let mut ver = String::new();
    for part in line.split('|').skip(1) {
        if let Some((k, v)) = part.split_once('=') {
            match k {
                "host" => host = v.to_string(),
                "boards" => boards = v.to_string(),
                "v" => ver = v.to_string(),
                _ => {}
            }
        }
    }
    let mut mac = String::new();
    let fw = boards
        .split(',')
        .find_map(|b| {
            let parts: Vec<&str> = b.split('@').collect();
            if parts.len() >= 3 {
                mac = normalize_mac(parts[2]);
            }
            if parts.len() >= 2 {
                Some(parts[1].to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();
    let detail = if boards.is_empty() {
        format!("Companion {ver} · no USB boards advertised")
    } else {
        format!("Companion {ver} · boards {boards}")
    };
    Some(DiscoveredWorker {
        id: format!("lan:{}:{}", addr.ip(), host),
        kind: WorkerKind::Lan,
        endpoint: format!("{}:{}", addr.ip(), LAN_DISCOVERY_PORT),
        mac,
        fw,
        detail,
        host,
        last_seen_ms: now_ms(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_normalize_matches_windows_prefixes() {
        assert!(port_names_match(r"\\.\COM6", "COM6"));
        assert!(port_names_match("com10", r"\\.\COM10"));
        assert!(!port_names_match("COM6", "COM7"));
        assert!(is_usb_serial_port("COM6"));
        assert!(is_usb_serial_port(r"\\.\COM10"));
        assert!(!is_usb_serial_port("192.168.4.1:19284"));
        assert!(!is_usb_serial_port("cyd.local:19284"));
        assert_eq!(windows_com_path("COM7"), r"\\.\COM7");
        assert_eq!(windows_com_path(r"\\.\COM10"), r"\\.\COM10");
        // Prefer plain COMx — espflash exact-matches available_ports() names.
        let arg = flash_port_arg("COM6");
        assert!(
            arg.eq_ignore_ascii_case("COM6") || arg.eq_ignore_ascii_case(r"\\.\COM6"),
            "unexpected flash port arg {arg}"
        );
        assert_eq!(flash_port_arg(r"\\.\COM6").to_ascii_uppercase().replace(r"\\.\", ""), "COM6");
    }

    #[test]
    fn pci_label_requires_deliberate_tag() {
        let pci = PortChoice {
            name: "COM1".into(),
            label: "COM1 — PCI (not a CYD)".into(),
        };
        let usb_named_pci = PortChoice {
            name: "COM8".into(),
            label: "COM8 — Acme PCIE USB Serial".into(),
        };
        assert!(port_choice_is_pci(&pci));
        assert!(!port_choice_is_pci(&usb_named_pci));
        assert!(port_choice_is_system_junk(&pci));
        assert!(!port_choice_is_system_junk(&usb_named_pci));
    }

    #[test]
    fn prefer_excludes_linked_com() {
        let ports = vec![
            PortChoice {
                name: "COM6".into(),
                label: "COM6 — USB CH340".into(),
            },
            PortChoice {
                name: "COM7".into(),
                label: "COM7 — USB CH340".into(),
            },
        ];
        let best_all = prefer_cyd_port(&ports).map(|p| p.name.as_str());
        assert!(best_all == Some("COM6") || best_all == Some("COM7"));
        let next = prefer_cyd_port_excluding(&ports, &["COM6".into()]);
        assert_eq!(next.map(|p| p.name.as_str()), Some("COM7"));
        let none = prefer_cyd_port_excluding(&ports, &["COM6".into(), "COM7".into()]);
        assert!(none.is_none());
    }

    #[test]
    fn mac_stable_rejects_unknown() {
        assert!(!mac_is_stable(""));
        assert!(!mac_is_stable("unknown"));
        assert!(mac_is_stable("aa:bb:cc:dd:ee:01"));
        assert_eq!(mac_worker_id("aabbccddee01"), "usb:mac:aa:bb:cc:dd:ee:01");
        assert!(mac_worker_id("unknown").is_empty());
        assert_eq!(
            transport_mac_id(WorkerKind::Wifi, "aabbccddee01"),
            "wifi:mac:aa:bb:cc:dd:ee:01"
        );
        assert_ne!(
            transport_mac_id(WorkerKind::Usb, "aabbccddee01"),
            transport_mac_id(WorkerKind::Wifi, "aabbccddee01")
        );
    }

    #[test]
    fn prefer_cyd_skips_pci_and_ranks_ch340() {
        let ports = vec![
            PortChoice {
                name: "COM1".into(),
                label: "COM1 — PCI".into(),
            },
            PortChoice {
                name: "COM6".into(),
                label: "COM6 — USB CH340".into(),
            },
            PortChoice {
                name: "COM3".into(),
                label: "COM3 — Unknown".into(),
            },
        ];
        assert!(port_choice_is_pci(&ports[0]));
        assert!(!port_choice_is_pci(&ports[1]));
        let best = prefer_cyd_port(&ports).unwrap();
        assert_eq!(best.name, "COM6");
        assert!(cyd_port_score(&ports[1]) > cyd_port_score(&ports[2]));
    }

    #[test]
    fn system_junk_hides_bare_com1_and_flashable_list() {
        let pci = PortChoice {
            name: "COM1".into(),
            label: "COM1 — PCI".into(),
        };
        let bare = PortChoice {
            name: "COM1".into(),
            label: "COM1".into(),
        };
        let usb_com1 = PortChoice {
            name: "COM1".into(),
            label: "COM1 — USB CH340".into(),
        };
        let cyd = PortChoice {
            name: "COM6".into(),
            label: "COM6 — USB CH340".into(),
        };
        assert!(port_choice_is_system_junk(&pci));
        assert!(port_choice_is_system_junk(&bare));
        assert!(!port_choice_is_system_junk(&usb_com1));
        assert!(!port_choice_is_system_junk(&cyd));
        let ports = vec![pci, bare.clone(), cyd.clone()];
        let flashable = flashable_ports(&ports);
        assert_eq!(flashable.len(), 1);
        assert_eq!(flashable[0].name, "COM6");
        assert_eq!(
            prefer_cyd_port(&[bare, cyd.clone()]).map(|p| p.name.as_str()),
            Some("COM6")
        );
    }

    #[test]
    fn bluetooth_ports_are_junk_not_scanned() {
        let bt = PortChoice {
            name: "COM9".into(),
            label: "COM9 — Bluetooth".into(),
        };
        let usb = PortChoice {
            name: "COM6".into(),
            label: "COM6 — USB CH340".into(),
        };
        assert!(cyd_port_score(&bt) < 0);
        assert!(port_choice_is_system_junk(&bt));
        assert!(!port_choice_is_system_junk(&usb));
        let ports = vec![bt, usb];
        let flashable = flashable_ports(&ports);
        assert_eq!(flashable.len(), 1);
        assert_eq!(flashable[0].name, "COM6");
    }
}
