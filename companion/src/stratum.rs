//! Bitcoin SHA-256 stratum client (TCP). Builds 80-byte headers for the board.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct WorkJob {
    pub job_id: String,
    pub header: [u8; 80],
    pub target: [u8; 32],
    pub extranonce2_hex: String,
    pub ntime_hex: String,
}

pub struct StratumClient {
    stream: Option<TcpStream>,
    reader: Option<BufReader<TcpStream>>,
    worker: String,
    password: String,
    msg_id: u64,
    subscribe_id: u64,
    authorize_id: u64,
    subscribed: bool,
    authorized: bool,
    extranonce1: Vec<u8>,
    extranonce2_size: usize,
    en2_counter: u64,
    difficulty: f64,
    job_id: String,
    prevhash_hex: String,
    coinb1_hex: String,
    coinb2_hex: String,
    merkle_hex: Vec<String>,
    version_hex: String,
    nbits_hex: String,
    ntime_hex: String,
    active_en2_hex: String,
    active_ntime_hex: String,
    pending_job: Option<WorkJob>,
    pub accepted: u32,
    pub rejected: u32,
    pub phase: String,
    pub endpoint: String,
    pub jobs_seen: u32,
    pub lines_rx: u64,
    pub lines_tx: u64,
    pub last_rx: String,
    pub last_tx: String,
    /// Recent stratum lines for the UI (newest last).
    pub recent: Vec<String>,
}

impl StratumClient {
    pub fn new(worker: String, password: String) -> Self {
        Self {
            stream: None,
            reader: None,
            worker,
            password,
            msg_id: 1,
            subscribe_id: 0,
            authorize_id: 0,
            subscribed: false,
            authorized: false,
            extranonce1: Vec::new(),
            extranonce2_size: 4,
            en2_counter: 1,
            difficulty: 1.0,
            job_id: String::new(),
            prevhash_hex: String::new(),
            coinb1_hex: String::new(),
            coinb2_hex: String::new(),
            merkle_hex: Vec::new(),
            version_hex: String::new(),
            nbits_hex: String::new(),
            ntime_hex: String::new(),
            active_en2_hex: String::new(),
            active_ntime_hex: String::new(),
            pending_job: None,
            accepted: 0,
            rejected: 0,
            phase: "off".into(),
            endpoint: String::new(),
            jobs_seen: 0,
            lines_rx: 0,
            lines_tx: 0,
            last_rx: String::new(),
            last_tx: String::new(),
            recent: Vec::new(),
        }
    }

    pub fn connect(&mut self, endpoint: &str) -> Result<(), String> {
        let (host, port) = parse_endpoint(endpoint)?;
        self.endpoint = format!("{host}:{port}");
        let addr = format!("{host}:{port}")
            .to_socket_addrs()
            .map_err(|e| format!("resolve: {e}"))?
            .next()
            .ok_or_else(|| "resolve empty".to_string())?;
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(8))
            .map_err(|e| format!("stratum connect: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .ok();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .ok();
        stream.set_nodelay(true).ok();
        let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        self.stream = Some(stream);
        self.reader = Some(reader);
        self.subscribed = false;
        self.authorized = false;
        self.pending_job = None;
        self.phase = "tcp".into();
        self.push_recent(format!("← TCP connected {}", self.endpoint));
        self.send_subscribe()
    }

    pub fn disconnect(&mut self) {
        self.stream = None;
        self.reader = None;
        self.subscribed = false;
        self.authorized = false;
        self.pending_job = None;
        self.phase = "off".into();
        self.push_recent("← disconnected".into());
    }

    pub fn connected(&self) -> bool {
        self.stream.is_some() && self.authorized
    }

    pub fn stream_connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn authorized(&self) -> bool {
        self.authorized
    }

    pub fn difficulty(&self) -> f64 {
        self.difficulty
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    fn push_recent(&mut self, line: String) {
        self.recent.push(line);
        if self.recent.len() > 40 {
            let n = self.recent.len() - 40;
            self.recent.drain(0..n);
        }
    }

    pub fn take_recent(&mut self) -> Vec<String> {
        std::mem::take(&mut self.recent)
    }

    pub fn poll(&mut self) -> Result<(), String> {
        let mut lines = Vec::new();
        if let Some(reader) = self.reader.as_mut() {
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        self.disconnect();
                        return Err("stratum closed".into());
                    }
                    Ok(_) => {
                        let t = line.trim().to_string();
                        if !t.is_empty() {
                            lines.push(t);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        break;
                    }
                    Err(e) => {
                        self.disconnect();
                        return Err(format!("stratum read: {e}"));
                    }
                }
            }
        }
        for line in lines {
            self.lines_rx += 1;
            self.last_rx = line.clone();
            let preview = if line.len() > 160 {
                format!("{}…", &line[..160])
            } else {
                line.clone()
            };
            self.push_recent(format!("← {preview}"));
            self.handle_line(&line)?;
        }
        Ok(())
    }

    pub fn take_job(&mut self) -> Option<WorkJob> {
        self.pending_job.take()
    }

    pub fn submit_share(
        &mut self,
        job_id: &str,
        en2: &str,
        ntime: &str,
        nonce_hex: &str,
    ) -> Result<(), String> {
        if !self.authorized {
            return Err("not authorized".into());
        }
        let id = self.msg_id;
        self.msg_id += 1;
        let msg = json!({
            "id": id,
            "method": "mining.submit",
            "params": [self.worker, job_id, en2, ntime, nonce_hex]
        });
        self.send_json(&msg)
    }

    fn send_subscribe(&mut self) -> Result<(), String> {
        self.subscribe_id = self.msg_id;
        self.msg_id += 1;
        let msg = json!({
            "id": self.subscribe_id,
            "method": "mining.subscribe",
            "params": ["cyd-companion/0.4.0-sha256"]
        });
        self.phase = "sub".into();
        self.send_json(&msg)
    }

    fn send_authorize(&mut self) -> Result<(), String> {
        self.authorize_id = self.msg_id;
        self.msg_id += 1;
        let msg = json!({
            "id": self.authorize_id,
            "method": "mining.authorize",
            "params": [self.worker, self.password]
        });
        self.phase = "auth".into();
        self.send_json(&msg)
    }

    fn send_json(&mut self, v: &Value) -> Result<(), String> {
        let mut s = serde_json::to_string(v).map_err(|e| e.to_string())?;
        s.push('\n');
        let stream = self.stream.as_mut().ok_or("not connected")?;
        stream
            .write_all(s.as_bytes())
            .map_err(|e| format!("stratum write: {e}"))?;
        self.lines_tx += 1;
        let trimmed = s.trim().to_string();
        self.last_tx = trimmed.clone();
        let preview = if trimmed.len() > 160 {
            format!("{}…", &trimmed[..160])
        } else {
            trimmed
        };
        self.push_recent(format!("→ {preview}"));
        Ok(())
    }

    fn handle_line(&mut self, line: &str) -> Result<(), String> {
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if let Some(method) = v.get("method").and_then(|m| m.as_str()) {
            let params = v.get("params").cloned().unwrap_or(Value::Null);
            if method == "mining.set_difficulty" {
                if let Some(d) = params.as_array().and_then(|a| a.first()).and_then(|x| x.as_f64())
                {
                    // Public-pool style solo pools use fractional difficulty (e.g. 0.001).
                    self.difficulty = if d > 0.0 { d } else { 1e-12 };
                }
                return Ok(());
            }
            if method == "mining.notify" {
                if let Some(arr) = params.as_array() {
                    if arr.len() >= 8 {
                        self.job_id = arr[0].as_str().unwrap_or("").to_string();
                        self.prevhash_hex = arr[1].as_str().unwrap_or("").to_string();
                        self.coinb1_hex = arr[2].as_str().unwrap_or("").to_string();
                        self.coinb2_hex = arr[3].as_str().unwrap_or("").to_string();
                        self.merkle_hex.clear();
                        if let Some(m) = arr[4].as_array() {
                            for b in m {
                                if let Some(s) = b.as_str() {
                                    self.merkle_hex.push(s.to_string());
                                }
                            }
                        }
                        self.version_hex = arr[5].as_str().unwrap_or("").to_string();
                        self.nbits_hex = arr[6].as_str().unwrap_or("").to_string();
                        self.ntime_hex = arr[7].as_str().unwrap_or("").to_string();
                        if let Some(job) = self.build_job() {
                            self.jobs_seen += 1;
                            self.push_recent(format!(
                                "← mining.notify job={} diff={}",
                                job.job_id, self.difficulty
                            ));
                            self.pending_job = Some(job);
                            self.phase = "mine".into();
                        }
                    }
                }
                return Ok(());
            }
            return Ok(());
        }

        let id = v.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
        let has_error = !v.get("error").map(|e| e.is_null()).unwrap_or(true);
        if id == self.subscribe_id && !self.subscribed {
            if has_error {
                self.phase = "err".into();
                return Err("subscribe failed".into());
            }
            if let Some(res) = v.get("result").and_then(|r| r.as_array()) {
                if res.len() >= 3 {
                    let en1 = res[1].as_str().unwrap_or("");
                    self.extranonce1 = hex::decode(en1).unwrap_or_default();
                    self.extranonce2_size = res[2].as_u64().unwrap_or(4) as usize;
                    if self.extranonce2_size == 0 {
                        self.extranonce2_size = 4;
                    }
                    if self.extranonce2_size > 16 {
                        self.extranonce2_size = 16;
                    }
                    self.subscribed = true;
                    return self.send_authorize();
                }
            }
            return Ok(());
        }

        if let Some(ok) = v.get("result").and_then(|r| r.as_bool()) {
            let ok = ok && !has_error;
            if id == self.authorize_id {
                self.authorized = ok;
                self.phase = if ok { "idle".into() } else { "err".into() };
                if !ok {
                    return Err("authorize failed".into());
                }
                return Ok(());
            }
            if ok {
                self.accepted += 1;
            } else {
                self.rejected += 1;
            }
            return Ok(());
        }

        if has_error && id != 0 && id != self.subscribe_id && id != self.authorize_id {
            self.rejected += 1;
        } else if id == self.authorize_id && !self.authorized && !has_error {
            self.authorized = true;
            self.phase = "idle".into();
        }
        Ok(())
    }

    fn build_job(&mut self) -> Option<WorkJob> {
        let coinb1 = hex::decode(&self.coinb1_hex).ok()?;
        let coinb2 = hex::decode(&self.coinb2_hex).ok()?;
        let en2_size = self.extranonce2_size.clamp(1, 16);
        let mut en2 = vec![0u8; en2_size];
        for i in 0..en2_size {
            let shift = (en2_size - 1 - i) * 8;
            en2[i] = ((self.en2_counter >> shift) & 0xff) as u8;
        }
        self.en2_counter = self.en2_counter.wrapping_add(1);
        self.active_en2_hex = hex::encode(&en2);
        self.active_ntime_hex = self.ntime_hex.clone();

        let mut coinbase = Vec::with_capacity(
            coinb1.len() + self.extranonce1.len() + en2_size + coinb2.len(),
        );
        coinbase.extend_from_slice(&coinb1);
        coinbase.extend_from_slice(&self.extranonce1);
        coinbase.extend_from_slice(&en2);
        coinbase.extend_from_slice(&coinb2);

        let mut merkle = dsha256(&coinbase);
        for branch_hex in &self.merkle_hex {
            let branch = hex::decode(branch_hex).ok()?;
            if branch.len() != 32 {
                return None;
            }
            let mut cat = [0u8; 64];
            cat[..32].copy_from_slice(&merkle);
            cat[32..].copy_from_slice(&branch);
            merkle = dsha256(&cat);
        }

        let version = hex_fixed(&self.version_hex, 4)?;
        let mut prev = hex_fixed(&self.prevhash_hex, 32)?;
        let nbits = hex_fixed(&self.nbits_hex, 4)?;
        let ntime = hex_fixed(&self.ntime_hex, 4)?;
        swab256(&mut prev);

        let mut header = [0u8; 80];
        header[0..4].copy_from_slice(&version);
        header[4..36].copy_from_slice(&prev);
        header[36..68].copy_from_slice(&merkle);
        header[68..72].copy_from_slice(&ntime);
        header[72..76].copy_from_slice(&nbits);

        let target = target_from_difficulty(self.difficulty);
        Some(WorkJob {
            job_id: self.job_id.clone(),
            header,
            target,
            extranonce2_hex: self.active_en2_hex.clone(),
            ntime_hex: self.active_ntime_hex.clone(),
        })
    }
}

fn parse_endpoint(raw: &str) -> Result<(String, u16), String> {
    let mut s = raw.trim().to_string();
    for pref in ["stratum+ssl://", "stratum+tcp://", "stratum://", "tcp://"] {
        if s.to_lowercase().starts_with(pref) {
            if pref.contains("ssl") {
                return Err("SSL stratum not supported".into());
            }
            s = s[pref.len()..].to_string();
            break;
        }
    }
    if let Some(i) = s.find('/') {
        s.truncate(i);
    }
    if let Some(i) = s.rfind(':') {
        let host = s[..i].to_string();
        let port: u16 = s[i + 1..].parse().map_err(|_| "bad port".to_string())?;
        if host.is_empty() {
            return Err("empty host".into());
        }
        Ok((host, port))
    } else {
        Ok((s, 3333))
    }
}

fn hex_fixed(hex: &str, n: usize) -> Option<Vec<u8>> {
    let v = hex::decode(hex).ok()?;
    if v.len() != n {
        return None;
    }
    Some(v)
}

fn dsha256(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(first);
    let mut out = [0u8; 32];
    out.copy_from_slice(&second);
    out
}

fn swab256(hash: &mut [u8]) {
    for i in 0..8 {
        let c = &mut hash[i * 4..i * 4 + 4];
        c.swap(0, 3);
        c.swap(1, 2);
    }
}

/// Stratum difficulty → 32-byte LE target (supports fractional diff for ESP32 solo pools).
fn target_from_difficulty(mut diff: f64) -> [u8; 32] {
    if diff <= 0.0 {
        diff = 1e-12;
    }
    // cgminer-style placement of 0xffff0000 / diff into LE words.
    let mut k: i32 = 6;
    while k > 0 && diff > 1.0 {
        diff /= 4294967296.0;
        k -= 1;
    }
    let m = (4294901760.0 / diff) as u64;
    let mut target = [0u8; 32];
    let idx = (k as usize).saturating_mul(4);
    if idx + 4 <= 32 {
        target[idx..idx + 4].copy_from_slice(&(m as u32).to_le_bytes());
    }
    if idx + 8 <= 32 {
        target[idx + 4..idx + 8].copy_from_slice(&((m >> 32) as u32).to_le_bytes());
    }
    if target.iter().all(|&b| b == 0) {
        target[0] = 1;
    }
    target
}

pub fn encode_job_cmd(job: &WorkJob) -> String {
    format!(
        "cmp job header={}&target={}&job={}&en2={}&ntime={}",
        hex::encode(job.header),
        hex::encode(job.target),
        urlenc(&job.job_id),
        urlenc(&job.extranonce2_hex),
        urlenc(&job.ntime_hex)
    )
}

fn urlenc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
