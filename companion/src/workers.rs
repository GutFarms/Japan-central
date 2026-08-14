//! Discover CYD workers on USB / Bluetooth serial, Wi‑Fi (UDP beacon + TCP cmp),
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
    Bluetooth,
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
}

/// One OS serial port with a human-readable label (USB chip / product).
#[derive(Debug, Clone)]
pub struct PortChoice {
    pub name: String,
    pub label: String,
}

/// List every serial port the OS reports (no filtering) with USB details when available.
pub fn list_serial_ports() -> Vec<PortChoice> {
    let mut infos = serialport::available_ports().unwrap_or_default();
    infos.sort_by(|a, b| a.port_name.cmp(&b.port_name));
    infos
        .into_iter()
        .map(|p| {
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
                SerialPortType::PciPort => format!("{} — PCI", p.port_name),
                SerialPortType::BluetoothPort => format!("{} — Bluetooth", p.port_name),
                SerialPortType::Unknown => p.port_name.clone(),
            };
            PortChoice {
                name: p.port_name,
                label,
            }
        })
        .collect()
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
        WorkerKind::Bluetooth => "bt",
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
    let mut port = serialport::new(name, baud)
        .timeout(timeout)
        .dtr_on_open(false)
        .open()
        .map_err(|e| format!("USB open failed @ {baud}: {e}"))?;
    let _ = port.write_data_terminal_ready(false);
    let _ = port.write_request_to_send(false);
    // Settle after open; CH340/CP210x may still glitch / reset the MCU.
    std::thread::sleep(Duration::from_millis(350));
    let _ = port.clear(serialport::ClearBuffer::All);
    Ok(port)
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

    let id = {
        let mid = transport_mac_id(WorkerKind::Usb, &mac);
        if mid.is_empty() {
            format!("usb:{name}")
        } else {
            mid
        }
    };
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
/// Skip PCI (motherboard COM1 often hangs forever and is never a CYD).
/// Prefer real USB; include Unknown (some CH340 stacks mis-report).
/// Bluetooth is optional and probed only after USB finishes.
fn scan_candidate_ports(include_bluetooth: bool) -> Vec<(String, WorkerKind)> {
    let mut infos = serialport::available_ports().unwrap_or_default();
    infos.retain(|p| match p.port_type {
        SerialPortType::PciPort => false,
        SerialPortType::BluetoothPort => include_bluetooth,
        SerialPortType::UsbPort(_) | SerialPortType::Unknown => true,
    });
    infos.sort_by(|a, b| {
        let rank = |p: &serialport::SerialPortInfo| match p.port_type {
            SerialPortType::UsbPort(_) => 0,
            SerialPortType::Unknown => 1,
            SerialPortType::BluetoothPort => 2,
            SerialPortType::PciPort => 3,
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.port_name.cmp(&b.port_name))
    });
    infos
        .into_iter()
        .map(|p| {
            let kind = match p.port_type {
                SerialPortType::BluetoothPort => WorkerKind::Bluetooth,
                _ => WorkerKind::Usb,
            };
            (p.port_name, kind)
        })
        .collect()
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

    // USB / Unknown first — never parallel: shared hubs brown out / reset both CYDs.
    let usb_candidates: Vec<(String, WorkerKind)> = scan_candidate_ports(false)
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
            Err(e) => on_port(&name, &format!("miss · {e}")),
        }
    }

    // Bluetooth last, still serial, short open deadline (often hangs on Windows).
    let bt_candidates: Vec<(String, WorkerKind)> = scan_candidate_ports(true)
        .into_iter()
        .filter(|(_, k)| *k == WorkerKind::Bluetooth)
        .filter(|(name, _)| !skip.iter().any(|s| port_names_match(s, name)))
        .collect();
    for (name, _) in bt_candidates {
        on_port(&name, "probing (bluetooth)");
        match probe_usb_port_detailed(&name) {
            Ok(mut w) => {
                w.kind = WorkerKind::Bluetooth;
                w.detail = format!("Bluetooth serial · {}", w.detail);
                if w.id.starts_with("usb:") {
                    w.id = w.id.replacen("usb:", "bt:", 1);
                }
                on_port(&name, &format!("ok · {}", w.detail));
                found.push(w);
            }
            Err(e) => on_port(&name, &format!("miss · {e}")),
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
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map_err(|e| format!("TCP connect {endpoint}: {e}"))?;
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
    let detail = match (fw.is_empty(), ap.is_empty()) {
        (false, false) => format!("fw {fw} · Wi‑Fi {mode} · AP {ap}"),
        (false, true) => format!("fw {fw} · Wi‑Fi {mode}"),
        (true, false) => format!("Wi‑Fi board · AP {ap}"),
        _ => "Wi‑Fi CYD board".into(),
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
        // Prefer plain COMx — espflash exact-matches available_ports() names.
        let arg = flash_port_arg("COM6");
        assert!(
            arg.eq_ignore_ascii_case("COM6") || arg.eq_ignore_ascii_case(r"\\.\COM6"),
            "unexpected flash port arg {arg}"
        );
        assert_eq!(flash_port_arg(r"\\.\COM6").to_ascii_uppercase().replace(r"\\.\", ""), "COM6");
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
}
