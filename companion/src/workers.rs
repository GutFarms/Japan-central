//! Discover CYD workers on USB (cmp ping/config) and Companion peers on the LAN.

use serde::{Deserialize, Serialize};
use serialport::SerialPort;
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
    pub fw: String,
    pub detail: String,
    pub host: String,
    #[serde(skip)]
    pub last_seen_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct WorkerLive {
    pub endpoint: String,
    pub fw: String,
    pub connected: bool,
    pub hashrate_hs: f64,
    pub hashes: u64,
    pub mining: bool,
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
    while Instant::now() < deadline {
        drain(&mut *port, &mut buf);
        if buf.lines().any(|l| {
            let t = l.trim();
            t.starts_with("CMP ok") || t.eq_ignore_ascii_case("CMPACK ping")
        }) {
            saw_pong = true;
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
        if let Some(line) = buf.lines().rev().find(|l| l.trim().starts_with('{')) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
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
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(15));
    }

    let detail = if fw.is_empty() {
        format!("CYD companion USB · pong @ {baud}")
    } else if mode.is_empty() {
        format!("fw {fw} @ {baud}")
    } else {
        format!("fw {fw} · {mode} @ {baud}")
    };

    Some(DiscoveredWorker {
        id: format!("usb:{name}"),
        kind: WorkerKind::Usb,
        endpoint: name.to_string(),
        fw,
        detail,
        host: "local".into(),
        last_seen_ms: now_ms(),
    })
}

/// Scan all serial ports for CYD boards. Skips ports listed in `skip`.
pub fn scan_usb_workers(skip: &[String]) -> Vec<DiscoveredWorker> {
    let ports = serialport::available_ports().unwrap_or_default();
    let mut out = Vec::new();
    for p in ports {
        if skip.iter().any(|s| s == &p.port_name) {
            continue;
        }
        if let Some(w) = probe_usb_port(&p.port_name) {
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

    pub fn maybe_beacon(&mut self, local_boards: &[(String, String)], host: &str) {
        if self.last_beacon.elapsed() < Duration::from_secs(4) {
            return;
        }
        self.last_beacon = Instant::now();
        let Some(sock) = self.sock.as_ref() else {
            return;
        };
        let boards = local_boards
            .iter()
            .map(|(p, fw)| {
                if fw.is_empty() {
                    p.clone()
                } else {
                    format!("{p}@{fw}")
                }
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
        // Also try subnet broadcast via connected interface default.
        let _ = sock.send_to(msg.as_bytes(), format!("224.0.0.1:{LAN_DISCOVERY_PORT}"));
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
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
    // Ignore empty beacons from ourselves with no useful data still OK to show peers.
    let fw = boards
        .split(',')
        .find_map(|b| b.split_once('@').map(|(_, fw)| fw.to_string()))
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
        fw,
        detail,
        host,
        last_seen_ms: now_ms(),
    })
}
