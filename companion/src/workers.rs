//! Discover CYD workers on USB (cmp ping/config) and Companion peers on the LAN.

use serde::{Deserialize, Serialize};
use serialport::{SerialPort, SerialPortType};
use std::io::Write;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

pub const LAN_DISCOVERY_PORT: u16 = 19283;
pub const LAN_MAGIC: &str = "CYDCOMPANION";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerKind {
    Usb,
    Lan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredWorker {
    pub id: String,
    pub kind: WorkerKind,
    /// COM port path, or `host:port` for LAN peers.
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

pub fn mac_worker_id(mac: &str) -> String {
    let m = normalize_mac(mac);
    if m.is_empty() || m == "unknown" {
        String::new()
    } else {
        format!("usb:mac:{m}")
    }
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
    let mut port = serialport::new(name, baud)
        .timeout(Duration::from_millis(40))
        .open()
        .ok()?;
    let _ = port.clear(serialport::ClearBuffer::All);
    let _ = port.write_all(b"\r\ncmp ping\r\n");
    let _ = port.flush();

    let mut buf = String::new();
    let deadline = Instant::now() + Duration::from_millis(if baud > 115_200 { 450 } else { 700 });
    let mut saw_pong = false;
    let mut mac = String::new();
    while Instant::now() < deadline {
        drain(&mut *port, &mut buf);
        if let Some(line) = buf.lines().find(|l| {
            let t = l.trim();
            t.starts_with("CMP ok") || t.eq_ignore_ascii_case("CMPACK ping")
        }) {
            saw_pong = true;
            if let Some(rest) = line.trim().split_once("mac=") {
                mac = normalize_mac(rest.1.trim());
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    if !saw_pong {
        return None;
    }

    let mut fw = String::new();
    let mut mode = String::new();
    let _ = port.write_all(b"cmp config\r\n");
    let _ = port.flush();
    let cfg_deadline = Instant::now() + Duration::from_millis(700);
    while Instant::now() < cfg_deadline {
        drain(&mut *port, &mut buf);
        if let Some(line) = buf.lines().rev().find(|l| l.trim().starts_with('{') || l.trim().starts_with("CMPCONFIG ")) {
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
        std::thread::sleep(Duration::from_millis(15));
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

/// Scan all serial ports for CYD boards. Skips ports listed in `skip`.
pub fn scan_usb_workers(skip: &[String]) -> Vec<DiscoveredWorker> {
    let ports = list_serial_ports();
    let mut out = Vec::new();
    for p in ports {
        if skip.iter().any(|s| s == &p.name) {
            continue;
        }
        if let Some(w) = probe_usb_port(&p.name) {
            out.push(w);
        }
    }
    out
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
