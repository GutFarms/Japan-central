//! Tiny HTTP status server — open `http://<board-ip>/` on the LAN.
//!
//! Runs only with WiFi + embassy-net. One connection at a time; read-only.
//! Discovery endpoints (`/probe`, `/alive`, `/api/system/info`) mirror the
//! NMMiner-style LAN monitor shape so the board is easy to find on the network.

use heapless::String;

use crate::radio::WifiPhase;
use crate::stratum::StratumPhase;

/// Live snapshot published by the miner loop for the web UI.
#[derive(Clone, Debug)]
pub struct WebStatus {
    pub hashrate_x100: u32,
    pub shares: u64,
    pub nonce: u32,
    pub address: String<96>,
    pub stratum: String<96>,
    pub wifi_ssid: String<32>,
    pub wifi: WifiPhase,
    pub ip: Option<[u8; 4]>,
    pub pool_phase: StratumPhase,
    pub accepted: u32,
    pub rejected: u32,
    pub dropped: u32,
    pub difficulty: u32,
    pub uptime_secs: u64,
    pub screen_on: bool,
}

impl Default for WebStatus {
    fn default() -> Self {
        Self {
            hashrate_x100: 0,
            shares: 0,
            nonce: 0,
            address: String::new(),
            stratum: String::new(),
            wifi_ssid: String::new(),
            wifi: WifiPhase::Disabled,
            ip: None,
            pool_phase: StratumPhase::Disabled,
            accepted: 0,
            rejected: 0,
            dropped: 0,
            difficulty: 1,
            uptime_secs: 0,
            screen_on: true,
        }
    }
}

#[cfg(feature = "esp")]
mod server {
    use alloc::format;

    use embassy_executor::Spawner;
    use embassy_net::tcp::TcpSocket;
    use embassy_net::Stack;
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
    use embassy_sync::mutex::Mutex;
    use embassy_time::{Duration, Timer};
    use log::info;

    use super::WebStatus;
    use crate::display::{DISPLAY_HEIGHT, DISPLAY_WIDTH};
    use crate::radio::WifiPhase;
    use crate::stratum::StratumPhase;

    const FW_VERSION: &str = env!("CARGO_PKG_VERSION");
    const MODEL: &str = "SCRYPT-CYD";
    const HOSTNAME: &str = "SCRYPT-CYD";

    static STATUS: Mutex<CriticalSectionRawMutex, WebStatus> = Mutex::new(WebStatus {
        hashrate_x100: 0,
        shares: 0,
        nonce: 0,
        address: heapless::String::new(),
        stratum: heapless::String::new(),
        wifi_ssid: heapless::String::new(),
        wifi: WifiPhase::Disabled,
        ip: None,
        pool_phase: StratumPhase::Disabled,
        accepted: 0,
        rejected: 0,
        dropped: 0,
        difficulty: 1,
        uptime_secs: 0,
        screen_on: true,
    });

    pub fn publish(s: WebStatus) {
        if let Ok(mut slot) = STATUS.try_lock() {
            *slot = s;
        }
    }

    pub fn start(spawner: &Spawner, stack: Stack<'static>) {
        match http_task(stack) {
            Ok(token) => {
                spawner.spawn(token);
                info!("web: listening on :80 (http://<dhcp-ip>/)");
            }
            Err(_) => info!("web: task token failed"),
        }
    }

    #[embassy_executor::task]
    async fn http_task(stack: Stack<'static>) {
        use static_cell::StaticCell;
        static RX_BUF: StaticCell<[u8; 512]> = StaticCell::new();
        static TX_BUF: StaticCell<[u8; 512]> = StaticCell::new();
        let rx_buf = RX_BUF.init([0; 512]);
        let tx_buf = TX_BUF.init([0; 512]);

        loop {
            stack.wait_config_up().await;

            let mut socket = TcpSocket::new(stack, rx_buf, tx_buf);
            socket.set_timeout(Some(Duration::from_secs(10)));

            if let Err(e) = socket.accept(80).await {
                info!("web: accept error {e:?}");
                Timer::after(Duration::from_millis(200)).await;
                continue;
            }

            let mut req = [0u8; 256];
            let mut got = 0usize;
            let deadline = embassy_time::Instant::now() + Duration::from_secs(3);
            while got < req.len() && embassy_time::Instant::now() < deadline {
                match socket.read(&mut req[got..]).await {
                    Ok(0) => break,
                    Ok(n) => {
                        got += n;
                        if req[..got].windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            let path = parse_path(&req[..got]);
            let snap = STATUS.lock().await.clone();

            let _ = match path {
                "/probe" => write_probe(&mut socket, &snap).await,
                "/alive" => write_alive(&mut socket, &snap).await,
                "/api/system/info" => write_system_info(&mut socket, &snap).await,
                "/api" | "/api/" | "/api/status" => write_json(&mut socket, &snap).await,
                "/api/reconnect" => {
                    crate::stratum::request_reconnect();
                    write_text(
                        &mut socket,
                        "200 OK",
                        "text/plain",
                        "stratum reconnect requested\n",
                    )
                    .await
                }
                _ => write_html(&mut socket, &snap).await,
            };

            let _ = socket.flush().await;
            socket.close();
            Timer::after(Duration::from_millis(20)).await;
        }
    }

    fn parse_path(req: &[u8]) -> &str {
        // "GET /path HTTP/1.x"
        let Ok(s) = core::str::from_utf8(req) else {
            return "/";
        };
        let mut parts = s.split_whitespace();
        let _method = parts.next();
        let path = parts.next().unwrap_or("/");
        path.split('?').next().unwrap_or("/")
    }

    async fn write_probe(socket: &mut TcpSocket<'_>, s: &WebStatus) -> Result<(), ()> {
        // NMMiner discovery shape: `hr` + `ver` required by their monitor.
        let hr = s.hashrate_x100 / 100;
        let body = format!(
            "{{\"model\":\"{MODEL}\",\"hostname\":\"{HOSTNAME}\",\"ver\":\"{FW_VERSION}\",\
\"sw\":{sw},\"sh\":{sh},\"hr\":{hr},\"ut\":{ut},\"algo\":\"scrypt\",\"board\":\"ESP32-2432S028\"}}",
            sw = DISPLAY_WIDTH,
            sh = DISPLAY_HEIGHT,
            hr = hr,
            ut = s.uptime_secs,
        );
        write_json_raw(socket, &body).await
    }

    async fn write_alive(socket: &mut TcpSocket<'_>, s: &WebStatus) -> Result<(), ()> {
        let self_ip = match s.ip {
            Some([a, b, c, d]) => format!("\"{a}.{b}.{c}.{d}\""),
            None => "null".into(),
        };
        let ips = match s.ip {
            Some([a, b, c, d]) => format!("[\"{a}.{b}.{c}.{d}\"]"),
            None => "[]".into(),
        };
        let body = format!("{{\"self\":{self_ip},\"ips\":{ips}}}");
        write_json_raw(socket, &body).await
    }

    async fn write_system_info(socket: &mut TcpSocket<'_>, s: &WebStatus) -> Result<(), ()> {
        let hr = format!("{}.{:02}", s.hashrate_x100 / 100, s.hashrate_x100 % 100);
        let body = format!(
            "{{\"identity\":{{\"hwModel\":\"{MODEL}\",\"hostName\":\"{HOSTNAME}\",\
\"fwVersion\":\"{FW_VERSION}\",\"board\":\"ESP32-2432S028\",\"algo\":\"scrypt\"}},\
\"miner\":{{\"hashRate\":{hr},\"sAccepted\":{acc},\"sRejected\":{rej},\"dropped\":{drop},\
\"uptimeSeconds\":{ut},\"poolDiff\":{diff},\"shares\":{shares},\"nonce\":\"{nonce:08x}\",\
\"screenOn\":{screen}}},\"stratum\":{{\"url\":{url},\"user\":{user},\"phase\":\"{phase}\",\
\"connected\":{conn}}},\"wifi\":{{\"ssid\":{ssid},\"state\":\"{wifi}\",\"ip\":{ip}}}}}",
            hr = hr,
            acc = s.accepted,
            rej = s.rejected,
            drop = s.dropped,
            ut = s.uptime_secs,
            diff = s.difficulty,
            shares = s.shares,
            nonce = s.nonce,
            screen = if s.screen_on { "true" } else { "false" },
            url = json_str(s.stratum.as_str()),
            user = json_str(s.address.as_str()),
            phase = if s.pool_phase.is_connected() {
                "CONNECTED"
            } else {
                s.pool_phase.label()
            },
            conn = if s.pool_phase.is_connected() {
                "true"
            } else {
                "false"
            },
            ssid = json_str(s.wifi_ssid.as_str()),
            wifi = s.wifi.label(),
            ip = match s.ip {
                Some([a, b, c, d]) => format!("\"{a}.{b}.{c}.{d}\""),
                None => "null".into(),
            },
        );
        write_json_raw(socket, &body).await
    }

    async fn write_html(socket: &mut TcpSocket<'_>, s: &WebStatus) -> Result<(), ()> {
        let ip = match s.ip {
            Some([a, b, c, d]) => format!("{a}.{b}.{c}.{d}"),
            None => "—".into(),
        };
        let rate = format!("{}.{:02}", s.hashrate_x100 / 100, s.hashrate_x100 % 100);
        let connected = s.pool_phase.is_connected();
        let phase = if connected {
            "CONNECTED"
        } else {
            s.pool_phase.label()
        };
        let phase_color = if connected { "#7dffa0" } else { "#e8f0e4" };
        let lcd = if s.screen_on { "on" } else { "off (mining)" };
        let body = format!(
            "<!doctype html><html><head><meta charset=utf-8>\
<meta name=viewport content=\"width=device-width,initial-scale=1\">\
<meta http-equiv=refresh content=3>\
<title>SCRYPT · CYD</title>\
<style>\
body{{margin:0;font:15px/1.45 Georgia,'Times New Roman',serif;background:#101612;color:#e8f0e4}}\
header{{padding:1.2rem 1.4rem;background:linear-gradient(120deg,#1a2a1c,#142018);border-bottom:3px solid #c45c26}}\
h1{{margin:0;font-size:1.6rem;letter-spacing:.04em;color:#ff8c1a}}\
.sub{{color:#8aa08c;margin-top:.35rem}}\
main{{padding:1.2rem 1.4rem;display:grid;gap:.9rem;max-width:520px}}\
.card{{background:#1a241c;border-radius:12px;padding:1rem 1.1rem;border:1px solid #2a3a2c}}\
.k{{color:#8aa08c;font-size:.75rem;text-transform:uppercase;letter-spacing:.06em}}\
.v{{font-size:1.35rem;margin-top:.2rem;font-variant-numeric:tabular-nums}}\
.rate{{font-size:2.2rem;color:#ff8c1a}}\
.row{{display:grid;grid-template-columns:1fr 1fr;gap:.8rem}}\
a{{color:#7dffa0}}\
</style></head><body>\
<header><h1>SCRYPT</h1>\
<div class=sub>ESP32-2432S028 · http://{ip}/ · LCD {lcd}</div></header>\
<main>\
<div class=card><div class=k>Active hashrate</div><div class=\"v rate\">{rate} H/s</div></div>\
<div class=row>\
<div class=card><div class=k>Pool</div><div class=v style=color:{phase_color}>{phase}</div></div>\
<div class=card><div class=k>Shares</div><div class=v>{shares}</div></div>\
</div>\
<div class=row>\
<div class=card><div class=k>Accepted</div><div class=v>{acc}</div></div>\
<div class=card><div class=k>Rejected</div><div class=v>{rej}</div></div>\
</div>\
<div class=card><div class=k>Worker</div><div class=v style=font-size:1rem>{addr}</div></div>\
<div class=card><div class=k>Stratum</div><div class=v style=font-size:1rem>{stratum}</div></div>\
<div class=card><div class=k>WiFi</div><div class=v>{wifi} · {ip}</div>\
<div class=k style=margin-top:.6rem>Diff {diff} · dropped {drop} · up {ut}s · nonce {nonce:08x}</div></div>\
<div class=card>\
<a href=/api/status>JSON</a> · \
<a href=/probe>probe</a> · \
<a href=/api/system/info>system</a> · \
<a href=/api/reconnect>Reconnect</a><br>\
<span style=color:#8aa08c;font-size:.85rem>LCD sleeps after 60s idle (BOOT/touch wake) — like NMMiner</span>\
</div>\
</main></body></html>",
            ip = ip,
            lcd = lcd,
            rate = rate,
            shares = s.shares,
            phase = phase,
            phase_color = phase_color,
            acc = s.accepted,
            rej = s.rejected,
            addr = html_escape(s.address.as_str()),
            stratum = html_escape(s.stratum.as_str()),
            wifi = s.wifi.label(),
            diff = s.difficulty,
            drop = s.dropped,
            ut = s.uptime_secs,
            nonce = s.nonce,
        );

        let header = format!(
            "HTTP/1.0 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        write_all(socket, header.as_bytes()).await?;
        write_all(socket, body.as_bytes()).await
    }

    async fn write_text(
        socket: &mut TcpSocket<'_>,
        status: &str,
        ctype: &str,
        body: &str,
    ) -> Result<(), ()> {
        let header = format!(
            "HTTP/1.0 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        write_all(socket, header.as_bytes()).await?;
        write_all(socket, body.as_bytes()).await
    }

    async fn write_json_raw(socket: &mut TcpSocket<'_>, body: &str) -> Result<(), ()> {
        let header = format!(
            "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        write_all(socket, header.as_bytes()).await?;
        write_all(socket, body.as_bytes()).await
    }

    async fn write_json(socket: &mut TcpSocket<'_>, s: &WebStatus) -> Result<(), ()> {
        let ip = match s.ip {
            Some([a, b, c, d]) => format!("\"{a}.{b}.{c}.{d}\""),
            None => "null".into(),
        };
        let pool = if s.pool_phase.is_connected() {
            "CONNECTED"
        } else {
            s.pool_phase.label()
        };
        let body = format!(
            "{{\"hashrate_hs\":{}.{:02},\"shares\":{},\"nonce\":\"{:08x}\",\
\"address\":{},\"stratum\":{},\"wifi\":\"{}\",\"ip\":{},\
\"pool\":\"{}\",\"connected\":{},\"accepted\":{},\"rejected\":{},\"dropped\":{},\
\"difficulty\":{},\"uptime_secs\":{},\"screen_on\":{}}}",
            s.hashrate_x100 / 100,
            s.hashrate_x100 % 100,
            s.shares,
            s.nonce,
            json_str(s.address.as_str()),
            json_str(s.stratum.as_str()),
            s.wifi.label(),
            ip,
            pool,
            if s.pool_phase.is_connected() {
                "true"
            } else {
                "false"
            },
            s.accepted,
            s.rejected,
            s.dropped,
            s.difficulty,
            s.uptime_secs,
            if s.screen_on { "true" } else { "false" },
        );
        write_json_raw(socket, &body).await
    }

    async fn write_all(socket: &mut TcpSocket<'_>, mut data: &[u8]) -> Result<(), ()> {
        while !data.is_empty() {
            match socket.write(data).await {
                Ok(0) => return Err(()),
                Ok(n) => data = &data[n..],
                Err(_) => return Err(()),
            }
        }
        Ok(())
    }

    fn html_escape(s: &str) -> alloc::string::String {
        let mut out = alloc::string::String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                _ => out.push(c),
            }
        }
        out
    }

    fn json_str(s: &str) -> alloc::string::String {
        let mut out = alloc::string::String::from("\"");
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                c if c < ' ' => {
                    let _ = core::fmt::Write::write_fmt(&mut out, format_args!("\\u{:04x}", c as u32));
                }
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
}

#[cfg(feature = "esp")]
pub use server::{publish, start};

#[cfg(not(feature = "esp"))]
pub fn publish(_s: WebStatus) {}
