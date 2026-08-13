//! LAN phone-monitor HTTP API — JSON + mobile web UI on port 19285.

use serde::Serialize;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
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

pub type MonitorShared = Arc<Mutex<MonitorSnapshot>>;

pub fn new_shared() -> MonitorShared {
    Arc::new(Mutex::new(MonitorSnapshot::default()))
}

pub fn publish(shared: &MonitorShared, snap: MonitorSnapshot) {
    if let Ok(mut g) = shared.lock() {
        *g = snap;
    }
}

/// Bind `0.0.0.0:19285` and serve `/api/status` + a phone web UI on a background thread.
pub fn start(shared: MonitorShared) -> Result<SocketAddr, String> {
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
                        let shared = Arc::clone(&shared);
                        thread::spawn(move || handle_client(s, shared));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(20)),
                }
            }
        })
        .map_err(|e| format!("monitor spawn: {e}"))?;
    Ok(addr)
}

fn handle_client(mut stream: TcpStream, shared: MonitorShared) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut buf = [0u8; 2048];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let line = req.lines().next().unwrap_or("");
    let path = line.split_whitespace().nth(1).unwrap_or("/");

    if path.starts_with("/api/status") {
        let body = shared
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

fn reply(stream: &mut TcpStream, code: u16, ctype: &str, body: &[u8]) {
    let reason = match code {
        200 => "OK",
        404 => "Not Found",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         Content-Type: {ctype}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
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
    --ice:#7edcff; --text:#e6f2fc; --muted:#789ebA; --dim:#466c8a; --warn:#ffc45b; --err:#ff6c5b;
  }
  *{box-sizing:border-box}
  body{
    margin:0; min-height:100vh; color:var(--text);
    background:
      radial-gradient(1200px 600px at 20% -10%, rgba(126,220,255,.16), transparent 55%),
      radial-gradient(900px 500px at 100% 10%, rgba(36,78,118,.35), transparent 50%),
      linear-gradient(180deg,#031428 0%, var(--bg) 55%, #01060e 100%);
    font-family:"Segoe UI",system-ui,sans-serif;
  }
  .wrap{max-width:520px;margin:0 auto;padding:28px 18px 48px}
  .brand{font-size:28px;font-weight:700;letter-spacing:.02em;color:var(--ice);margin:0}
  .tag{margin:6px 0 22px;color:var(--muted);font-size:14px}
  .hero{
    border:1px solid rgba(126,220,255,.28); border-radius:22px; padding:22px 18px;
    background:rgba(6,20,38,.82); backdrop-filter:blur(8px);
  }
  .rate{font-size:56px;line-height:1;font-weight:700;letter-spacing:-.03em}
  .unit{color:var(--ice);font-size:18px;margin-top:8px}
  .row{display:flex;justify-content:space-between;gap:12px;margin-top:18px;flex-wrap:wrap}
  .chip{
    flex:1 1 120px; border-radius:14px; padding:12px 14px;
    background:rgba(8,28,48,.9); border:1px solid rgba(36,78,118,.7);
  }
  .chip b{display:block;color:var(--dim);font-size:11px;letter-spacing:.08em;text-transform:uppercase}
  .chip span{display:block;margin-top:6px;font-size:18px;font-weight:600}
  .boards{margin-top:18px}
  .board{
    margin-top:10px;padding:12px 14px;border-radius:14px;
    background:rgba(5,18,34,.88);border:1px solid rgba(36,78,118,.55);
    font-size:13px;color:var(--muted)
  }
  .board strong{color:var(--text)}
  .ok{color:var(--ice)} .warn{color:var(--warn)} .err{color:var(--err)}
  .foot{margin-top:22px;color:var(--dim);font-size:12px;line-height:1.45}
  input{
    width:100%;margin-top:10px;padding:12px 14px;border-radius:12px;border:1px solid var(--line);
    background:#061426;color:var(--text);font-size:16px
  }
  button{
    margin-top:10px;width:100%;padding:12px;border:0;border-radius:12px;
    background:linear-gradient(180deg,#9ae6ff,#5ec4ef);color:#031018;font-weight:700;font-size:15px
  }
</style>
</head>
<body>
  <div class="wrap">
    <p class="brand">Njörðr Seas'</p>
    <p class="tag">CYD miner · phone monitor</p>
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
    <label class="foot" for="host">Companion host (LAN IP)</label>
    <input id="host" placeholder="192.168.x.x" autocomplete="off" autocapitalize="off"/>
    <button id="save">Save &amp; refresh</button>
    <p class="foot" id="meta">Polling /api/status on this host when opened from Companion. For a remote PC, enter its LAN IP.</p>
  </div>
<script>
const $ = (id) => document.getElementById(id);
function fmtRate(hs){
  hs = Number(hs)||0;
  if (hs < 1000) return [hs.toFixed(0), 'H/s'];
  if (hs < 1e6) return [(hs/1000).toFixed(hs>=100000?0:hs>=10000?1:2), 'kH/s'];
  return [(hs/1e6).toFixed(2), 'MH/s'];
}
function baseUrl(){
  const saved = localStorage.getItem('cyd_monitor_host') || '';
  if (saved) return 'http://' + saved.replace(/^https?:\/\//,'').replace(/\/$/,'') + ':19285';
  return '';
}
async function tick(){
  const base = baseUrl();
  const url = (base || '') + '/api/status';
  try{
    const r = await fetch(url, {cache:'no-store'});
    const j = await r.json();
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
    $('meta').textContent = (j.product||'Companion') + ' ' + (j.version||'') + ' · updated ' + new Date(j.updated_ms||Date.now()).toLocaleTimeString();
  }catch(e){
    $('meta').textContent = 'Cannot reach Companion monitor API. Enter the PC LAN IP below (port 19285).';
  }
}
$('host').value = localStorage.getItem('cyd_monitor_host') || '';
$('save').onclick = () => {
  localStorage.setItem('cyd_monitor_host', $('host').value.trim());
  tick();
};
tick();
setInterval(tick, 2000);
</script>
</body>
</html>
"##;
