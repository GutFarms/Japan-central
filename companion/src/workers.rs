//! Discover CYD workers on USB / Bluetooth serial, Wi‑Fi (UDP beacon + TCP cmp),
//! and Companion peers on the LAN.

use serde::{Deserialize, Serialize};
use serialport::{SerialPort, SerialPortType};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
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
    // Brief settle; CH340/CP210x still may glitch lines on some hosts.
    std::thread::sleep(Duration::from_millis(120));
    let _ = port.clear(serialport::ClearBuffer::All);
    Ok(port)
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
pub fn probe_usb_port(name: &str) -> Option<DiscoveredWorker> {
    for baud in [460_800u32, 115_200] {
        if let Some(w) = probe_usb_port_at(name, baud) {
            return Some(w);
        }
    }
    None
}

fn probe_usb_port_at(name: &str, baud: u32) -> Option<DiscoveredWorker> {
    let mut port = open_usb_serial(name, baud, Duration::from_millis(80)).ok()?;
    let mut buf = String::new();
    // First try after soft open; if the UART bridge still reset the MCU, wait for boot.
    let ping_ms = if baud > 115_200 { 1_200 } else { 2_000 };
    let mut pong = wait_for_pong(port.as_mut(), &mut buf, ping_ms);
    if pong.is_none() {
        std::thread::sleep(Duration::from_millis(1_600));
        let _ = port.clear(serialport::ClearBuffer::All);
        buf.clear();
        pong = wait_for_pong(port.as_mut(), &mut buf, ping_ms + 800);
    }
    let pong_line = pong?;
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
        let mid = mac_worker_id(&mac);
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

    Some(DiscoveredWorker {
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

/// Ports worth probing for CYD boards (USB first, then BT / unknown / PCI).
fn scan_candidate_ports() -> Vec<(String, WorkerKind)> {
    let mut infos = serialport::available_ports().unwrap_or_default();
    infos.sort_by(|a, b| {
        let rank = |p: &serialport::SerialPortInfo| match p.port_type {
            SerialPortType::UsbPort(_) => 0,
            SerialPortType::BluetoothPort => 1,
            SerialPortType::Unknown => 2,
            SerialPortType::PciPort => 3,
        };
        rank(a).cmp(&rank(b)).then_with(|| a.port_name.cmp(&b.port_name))
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
pub fn scan_usb_workers(skip: &[String]) -> Vec<DiscoveredWorker> {
    scan_usb_workers_with_progress(skip, |_| {})
}

/// Parallel USB/BT/PCI probe with per-port progress callback.
pub fn scan_usb_workers_with_progress(
    skip: &[String],
    on_port: impl Fn(&str) + Send + Sync + 'static,
) -> Vec<DiscoveredWorker> {
    let candidates: Vec<(String, WorkerKind)> = scan_candidate_ports()
        .into_iter()
        .filter(|(name, _)| !skip.iter().any(|s| port_names_match(s, name)))
        .collect();
    let on_port = Arc::new(on_port);
    let out = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    // Bound concurrency so Windows COM opens don't stampede.
    let slots = Arc::new(Mutex::new(0usize));
    const MAX_PARALLEL: usize = 4;
    for (name, kind) in candidates {
        let out = Arc::clone(&out);
        let on_port = Arc::clone(&on_port);
        let slots = Arc::clone(&slots);
        handles.push(std::thread::spawn(move || {
            loop {
                let mut n = slots.lock().unwrap();
                if *n < MAX_PARALLEL {
                    *n += 1;
                    break;
                }
                drop(n);
                std::thread::sleep(Duration::from_millis(30));
            }
            on_port(&name);
            if let Some(mut w) = probe_usb_port(&name) {
                w.kind = kind;
                if kind == WorkerKind::Bluetooth {
                    w.detail = format!("Bluetooth serial · {}", w.detail);
                    if w.id.starts_with("usb:") {
                        w.id = w.id.replacen("usb:", "bt:", 1);
                    }
                }
                out.lock().unwrap().push(w);
            }
            *slots.lock().unwrap() -= 1;
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    let mut found = out.lock().unwrap().clone();
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
        let mid = mac_worker_id(&mac);
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
        let sock = UdpSocket::bind(format!("0.0.0.0:{BOARD_WIFI_PORT}"))
            .or_else(|_| UdpSocket::bind("0.0.0.0:0"))
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
        let mid = mac_worker_id(&mac);
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
