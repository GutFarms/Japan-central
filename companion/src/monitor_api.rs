//! Personal phone-monitor HTTP API — token-gated JSON + mobile web UI on port 19285.

use qrcode::{Color as QrColor, QrCode};
use rand::RngCore;
use serde::Serialize;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const MONITOR_PORT: u16 = 19285;

#[derive(Debug, Clone, Serialize, Default)]
pub struct MonitorBoard {
    pub endpoint: String,
    pub mac: String,
    pub fw: String,
    pub hashrate_hs: f64,
    pub hashes: u64,
    pub mining: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitorSnapshot {
    pub version: String,
    pub product: String,
    /// Public install id (safe to show); not the secret token.
    pub pair_id: String,
    pub mining: bool,
    pub usb_open: bool,
    pub pool_phase: String,
    pub pool_authorized: bool,
    pub hashrate_hs: f64,
    pub hashes: u64,
    pub accepted: u32,
    pub rejected: u32,
    pub boards: Vec<MonitorBoard>,
    pub host: String,
    pub updated_ms: u64,
}

impl Default for MonitorSnapshot {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").into(),
            product: "Njörðr Seas' CYD miner".into(),
            pair_id: String::new(),
            mining: false,
            usb_open: false,
            pool_phase: "off".into(),
            pool_authorized: false,
            hashrate_hs: 0.0,
            hashes: 0,
            accepted: 0,
            rejected: 0,
            boards: Vec::new(),
            host: String::new(),
            updated_ms: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitorCreds {
    pub install_id: String,
    pub token: String,
}

struct MonitorInner {
    snap: Mutex<MonitorSnapshot>,
    creds: Mutex<MonitorCreds>,
}

#[derive(Clone)]
pub struct MonitorHub {
    inner: Arc<MonitorInner>,
}

impl MonitorHub {
    pub fn new(creds: MonitorCreds) -> Self {
        Self {
            inner: Arc::new(MonitorInner {
                snap: Mutex::new(MonitorSnapshot::default()),
                creds: Mutex::new(creds),
            }),
        }
    }

    pub fn publish(&self, snap: MonitorSnapshot) {
        if let Ok(mut g) = self.inner.snap.lock() {
            *g = snap;
        }
    }

    pub fn creds(&self) -> MonitorCreds {
        self.inner
            .creds
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| MonitorCreds {
                install_id: String::new(),
                token: String::new(),
            })
    }

    pub fn set_creds(&self, creds: MonitorCreds) {
        if let Ok(mut g) = self.inner.creds.lock() {
            *g = creds;
        }
    }
}

/// Short public install id (8 hex chars).
pub fn generate_install_id() -> String {
    let mut b = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut b);
    hex_lower(&b)
}

/// Secret pairing token (32 hex chars / 128-bit).
pub fn generate_token() -> String {
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    hex_lower(&b)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

/// Best-effort primary LAN IPv4 (UDP connect trick).
pub fn primary_lan_ipv4() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(v) if !v.is_loopback() && !v.is_unspecified() => Some(v.to_string()),
        _ => None,
    }
}

/// Deep-link payload encoded in the personal QR (unique per Companion install).
pub fn pair_url(host: &str, port: u16, install_id: &str, token: &str) -> String {
    format!(
        "njordrseas://cyd-monitor/v1?host={}&port={}&id={}&token={}",
        url_encode(host.trim()),
        port,
        url_encode(install_id.trim()),
        url_encode(token.trim())
    )
}

/// Browser fallback URL that carries the personal token.
pub fn web_pair_url(host: &str, port: u16, install_id: &str, token: &str) -> String {
    format!(
        "http://{}:{}/?id={}&token={}",
        host.trim(),
        port,
        url_encode(install_id.trim()),
        url_encode(token.trim())
    )
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// QR module matrix (true = dark). Empty on encode failure.
pub fn qr_modules(data: &str) -> Option<(usize, Vec<bool>)> {
    let code = QrCode::new(data.as_bytes()).ok()?;
    let w = code.width();
    let mut cells = Vec::with_capacity(w * w);
    for y in 0..w {
        for x in 0..w {
            cells.push(code[(x, y)] == QrColor::Dark);
        }
    }
    Some((w, cells))
}

/// Bind `0.0.0.0:19285` and serve token-gated `/api/status` + phone web UI.
pub fn start(hub: MonitorHub) -> Result<SocketAddr, String> {
    let listener = TcpListener::bind(("0.0.0.0", MONITOR_PORT))
        .map_err(|e| format!("monitor bind :{MONITOR_PORT}: {e}"))?;
    listener
        .set_nonblocking(false)
        .map_err(|e| format!("monitor set blocking: {e}"))?;
    let addr = listener
        .local_addr()
        .map_err(|e| format!("monitor local_addr: {e}"))?;
    thread::Builder::new()
        .name("cyd-monitor-http".into())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        let hub = hub.clone();
                        thread::spawn(move || handle_client(s, hub));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(20)),
                }
            }
        })
        .map_err(|e| format!("monitor spawn: {e}"))?;
    Ok(addr)
}

fn handle_client(mut stream: TcpStream, hub: MonitorHub) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let line = req.lines().next().unwrap_or("");
    let path_q = line.split_whitespace().nth(1).unwrap_or("/");
    let (path, query) = split_path_query(path_q);

    let creds = hub.creds();
    let auth_ok = token_authorized(&req, query, &creds.token);

    if path.starts_with("/api/status") {
        if !auth_ok {
            reply(
                &mut stream,
                401,
                "application/json; charset=utf-8",
                br#"{"error":"unauthorized","hint":"Scan your personal Companion QR, or pass ?token= / Authorization: Bearer"}"#,
            );
            return;
        }
        let body = hub
            .inner
            .snap
            .lock()
            .ok()
            .and_then(|g| serde_json::to_string_pretty(&*g).ok())
            .unwrap_or_else(|| "{\"error\":\"busy\"}".into());
        reply(
            &mut stream,
            200,
            "application/json; charset=utf-8",
            body.as_bytes(),
        );
        return;
    }

    if path == "/" || path.starts_with("/index") || path.starts_with("/monitor") {
        // Web UI is public shell; live data fetch requires the personal token.
        reply(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            MONITOR_HTML.as_bytes(),
        );
        return;
    }

    reply(
        &mut stream,
        404,
        "text/plain; charset=utf-8",
        b"not found\n",
    );
}

fn split_path_query(path_q: &str) -> (&str, &str) {
    match path_q.split_once('?') {
        Some((p, q)) => (p, q),
        None => (path_q, ""),
    }
}

fn token_authorized(req: &str, query: &str, expected: &str) -> bool {
    if expected.is_empty() {
        return false;
    }
    if query_param(query, "token").as_deref() == Some(expected) {
        return true;
    }
    if query_param(query, "t").as_deref() == Some(expected) {
        return true;
    }
    for line in req.lines().skip(1) {
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("authorization") {
            let tok = value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
                .unwrap_or("")
                .trim();
            if tok == expected {
                return true;
            }
        }
        if name.eq_ignore_ascii_case("x-cyd-token") && value == expected {
            return true;
        }
    }
    false
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for part in query.split('&') {
        let mut it = part.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        if k == key {
            return Some(url_decode(v));
        }
    }
    None
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = || -> Option<u8> {
                    let a = hex_nibble(bytes[i + 1])?;
                    let b = hex_nibble(bytes[i + 2])?;
                    Some((a << 4) | b)
                };
                if let Some(b) = h() {
                    out.push(b);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn reply(stream: &mut TcpStream, code: u16, ctype: &str, body: &[u8]) {
    let reason = match code {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         Content-Type: {ctype}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Headers: Authorization, X-Cyd-Token, Content-Type\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

const MONITOR_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover"/>
<meta name="apple-mobile-web-app-capable" content="yes"/>
<meta name="mobile-web-app-capable" content="yes"/>
<meta name="theme-color" content="#020a16"/>
<title>Njörðr Seas' monitor</title>
<style>
  :root {
    --bg:#020a16; --panel:#08162a; --line:#244e76;
    --ice:#7edcff; --neon:#00e5ff; --neon-deep:#1478ff; --neon-hot:#b4f5ff;
    --text:#e6f2fc; --muted:#789ebA; --dim:#466c8a; --warn:#ffc45b; --err:#ff6c5b;
  }
  *{box-sizing:border-box}
  body{
    margin:0; min-height:100vh; color:var(--text);
    background:
      radial-gradient(900px 520px at 12% -8%, rgba(0,229,255,.22), transparent 55%),
      radial-gradient(700px 420px at 92% 8%, rgba(20,120,255,.28), transparent 52%),
      radial-gradient(500px 280px at 70% 0%, rgba(180,245,255,.10), transparent 50%),
      linear-gradient(180deg,#031428 0%, var(--bg) 55%, #01060e 100%);
    font-family:"Segoe UI",system-ui,sans-serif;
  }
  .wrap{max-width:520px;margin:0 auto;padding:28px 18px 48px;position:relative}
  .brand-row{display:flex;align-items:center;gap:10px}
  .brand{
    font-size:28px;font-weight:700;letter-spacing:.02em;color:var(--ice);margin:0;
    text-shadow:0 0 18px rgba(0,229,255,.55);
  }
  .bolt{
    width:16px;height:26px;position:relative;filter:drop-shadow(0 0 8px rgba(0,229,255,.85));
  }
  .bolt i{
    position:absolute;display:block;height:3px;border-radius:2px;background:var(--neon);
  }
  .bolt i:nth-child(1){width:13px;top:3px;left:2px;transform:rotate(28deg)}
  .bolt i:nth-child(2){width:15px;top:11px;left:0;background:var(--neon-hot);transform:rotate(-32deg)}
  .bolt i:nth-child(3){width:11px;top:19px;left:4px;transform:rotate(24deg)}
  .tag{margin:6px 0 22px;color:var(--muted);font-size:14px}
  .hero{
    position:relative;overflow:hidden;
    border:1px solid rgba(0,229,255,.38); border-radius:22px; padding:22px 18px;
    background:
      linear-gradient(135deg, rgba(0,229,255,.14), transparent 42%),
      rgba(6,20,38,.82);
    backdrop-filter:blur(8px);
    box-shadow:0 0 28px rgba(0,229,255,.18);
  }
  .hero::before,.hero::after{
    content:"";position:absolute;pointer-events:none;border-radius:2px;
    background:linear-gradient(180deg,var(--neon-hot),var(--neon),transparent);
    opacity:.55;animation:flash 2.8s ease-in-out infinite;
  }
  .hero::before{right:34px;top:6px;width:3px;height:96px;transform:rotate(12deg)}
  .hero::after{
    right:62px;top:28px;width:2px;height:56px;transform:rotate(-18deg);
    background:linear-gradient(180deg,var(--neon),var(--neon-deep),transparent);
    animation-delay:.9s;
  }
  @keyframes flash{0%,100%{opacity:.2}40%{opacity:.85}55%{opacity:.25}70%{opacity:.7}}
  .rate{
    font-size:56px;line-height:1;font-weight:700;letter-spacing:-.03em;
    text-shadow:0 0 22px rgba(0,229,255,.4);
  }
  .unit{color:var(--neon);font-size:18px;margin-top:8px}
  .row{display:flex;justify-content:space-between;gap:12px;margin-top:18px;flex-wrap:wrap}
  .chip{
    flex:1 1 120px; border-radius:14px; padding:12px 14px;
    background:rgba(8,28,48,.9); border:1px solid rgba(0,229,255,.28);
  }
  .chip b{display:block;color:var(--dim);font-size:11px;letter-spacing:.08em;text-transform:uppercase}
  .chip span{display:block;margin-top:6px;font-size:18px;font-weight:600}
  .boards{margin-top:18px}
  .board{
    margin-top:10px;padding:12px 14px;border-radius:14px;
    background:rgba(5,18,34,.88);border:1px solid rgba(20,120,255,.35);
    font-size:13px;color:var(--muted)
  }
  .board strong{color:var(--text)}
  .ok{color:var(--neon)} .warn{color:var(--warn)} .err{color:var(--err)}
  .foot{margin-top:22px;color:var(--dim);font-size:12px;line-height:1.45}
  input{
    width:100%;margin-top:10px;padding:12px 14px;border-radius:12px;border:1px solid var(--line);
    background:#061426;color:var(--text);font-size:16px
  }
  button{
    margin-top:10px;width:100%;padding:12px;border:0;border-radius:12px;
    background:linear-gradient(180deg,#7af0ff,#00c8e6);color:#031018;font-weight:700;font-size:15px;
    box-shadow:0 0 18px rgba(0,229,255,.35);
  }
</style>
</head>
<body>
  <div class="wrap">
    <div class="brand-row">
      <p class="brand">Njörðr Seas'</p>
      <span class="bolt" aria-hidden="true"><i></i><i></i><i></i></span>
    </div>
    <p class="tag">CYD miner · personal phone monitor</p>
    <div class="hero">
      <div class="rate" id="rate">—</div>
      <div class="unit" id="unit">board measured</div>
      <div class="row">
        <div class="chip"><b>Pool</b><span id="pool">—</span></div>
        <div class="chip"><b>Accept</b><span id="acc">0</span></div>
        <div class="chip"><b>Reject</b><span id="rej">0</span></div>
      </div>
      <div class="boards" id="boards"></div>
    </div>
    <label class="foot" for="host">Companion host (LAN IP / DDNS)</label>
    <input id="host" placeholder="192.168.x.x" autocomplete="off" autocapitalize="off"/>
    <label class="foot" for="token">Personal token (from your QR)</label>
    <input id="token" placeholder="scan Companion QR or paste token" autocomplete="off" autocapitalize="off"/>
    <button id="save">Save &amp; refresh</button>
    <p class="foot" id="meta">Each Companion install has a unique QR. Only phones that scanned yours can read this miner.</p>
  </div>
<script>
const $ = (id) => document.getElementById(id);
function fmtRate(hs){
  hs = Number(hs)||0;
  if (hs < 1000) return [hs.toFixed(0), 'H/s'];
  if (hs < 1e6) return [(hs/1000).toFixed(hs>=100000?0:hs>=10000?1:2), 'kH/s'];
  return [(hs/1e6).toFixed(2), 'MH/s'];
}
function qp(){
  const u = new URL(location.href);
  return { id: u.searchParams.get('id')||'', token: u.searchParams.get('token')||u.searchParams.get('t')||'' };
}
function loadCreds(){
  const q = qp();
  let host = localStorage.getItem('cyd_monitor_host') || '';
  let token = localStorage.getItem('cyd_monitor_token') || '';
  let id = localStorage.getItem('cyd_monitor_id') || '';
  if (q.token) { token = q.token; localStorage.setItem('cyd_monitor_token', token); }
  if (q.id) { id = q.id; localStorage.setItem('cyd_monitor_id', id); }
  if (!host && location.hostname && location.hostname !== 'localhost') host = location.hostname;
  return { host, token, id };
}
function baseUrl(host){
  if (!host) return '';
  return 'http://' + host.replace(/^https?:\/\//,'').replace(/\/$/,'').replace(/:\d+$/,'') + ':19285';
}
async function tick(){
  const { host, token, id } = loadCreds();
  $('host').value = host || '';
  $('token').value = token || '';
  if (!token){
    $('meta').textContent = 'Scan your personal QR in Companion Settings → Phone monitor (or paste the token).';
    return;
  }
  const base = baseUrl(host) || '';
  const url = (base || '') + '/api/status?token=' + encodeURIComponent(token);
  try{
    const r = await fetch(url, {
      cache:'no-store',
      headers: { 'Authorization': 'Bearer ' + token, 'X-Cyd-Token': token }
    });
    if (r.status === 401){
      $('meta').textContent = 'Unauthorized — this token is not for this Companion. Scan the QR from your PC app.';
      return;
    }
    const j = await r.json();
    if (id && j.pair_id && j.pair_id !== id){
      $('meta').textContent = 'Pair id mismatch — QR is for a different Companion install.';
      return;
    }
    const [n,u] = fmtRate(j.hashrate_hs);
    $('rate').textContent = n;
    $('unit').textContent = u + ' · ' + (j.mining ? 'mining' : (j.usb_open?'linked':'idle'));
    const pool = j.pool_authorized ? 'AUTHORIZED' : (j.pool_phase||'IDLE');
    $('pool').textContent = pool;
    $('pool').className = j.pool_authorized ? 'ok' : 'warn';
    $('acc').textContent = j.accepted ?? 0;
    $('rej').textContent = j.rejected ?? 0;
    const boards = Array.isArray(j.boards)?j.boards:[];
    $('boards').innerHTML = boards.length ? boards.map(b => {
      const [rn,ru]=fmtRate(b.hashrate_hs);
      return `<div class="board"><strong>${b.mac||b.endpoint}</strong><br/>${b.endpoint} · ${rn} ${ru}${b.mining?' · hashing':''}</div>`;
    }).join('') : '<div class="board">No boards linked</div>';
    $('meta').textContent = (j.product||'Companion') + ' ' + (j.version||'') + ' · pair ' + (j.pair_id||'?') + ' · ' + new Date(j.updated_ms||Date.now()).toLocaleTimeString();
  }catch(e){
    $('meta').textContent = 'Cannot reach Companion. Same Wi‑Fi / VPN / port-forward to :19285, and use your personal token.';
  }
}
$('save').onclick = () => {
  localStorage.setItem('cyd_monitor_host', $('host').value.trim());
  localStorage.setItem('cyd_monitor_token', $('token').value.trim());
  tick();
};
tick();
setInterval(tick, 2000);
</script>
</body>
</html>
"##;


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_url_roundtrip_fields() {
        let u = pair_url("10.0.0.5", MONITOR_PORT, "aabbccdd", "0123456789abcdef0123456789abcdef");
        assert!(u.starts_with("njordrseas://cyd-monitor/v1?"));
        assert!(u.contains("host=10.0.0.5"));
        assert!(u.contains("id=aabbccdd"));
        assert!(u.contains("token=0123456789abcdef0123456789abcdef"));
    }

    #[test]
    fn qr_encodes() {
        let u = pair_url("192.168.0.2", MONITOR_PORT, "11223344", "ffffffffffffffffffffffffffffffff");
        let (w, cells) = qr_modules(&u).expect("qr");
        assert!(w >= 21);
        assert_eq!(cells.len(), w * w);
        assert!(cells.iter().any(|&c| c));
    }

    #[test]
    fn token_gate_headers() {
        let tok = "abc123";
        let req = format!(
            "GET /api/status HTTP/1.1\r\nAuthorization: Bearer {tok}\r\n\r\n"
        );
        assert!(token_authorized(&req, "", tok));
        assert!(!token_authorized(&req, "", "other"));
        assert!(token_authorized("GET /x HTTP/1.1\r\n\r\n", "token=abc123", tok));
    }
}
