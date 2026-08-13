//! Live ticker bar: IP geolocation → local weather/time + selectable crypto prices.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{FixedOffset, Utc};
use serde::Deserialize;

/// Catalog of coins available for the header ticker (CoinGecko id → symbol).
pub const COIN_CATALOG: &[(&str, &str)] = &[
    ("bitcoin", "BTC"),
    ("litecoin", "LTC"),
    ("ethereum", "ETH"),
    ("solana", "SOL"),
    ("binancecoin", "BNB"),
    ("ripple", "XRP"),
    ("dogecoin", "DOGE"),
    ("cardano", "ADA"),
    ("monero", "XMR"),
    ("bitcoin-cash", "BCH"),
];

/// Default coins shown in the header strip.
pub fn default_header_coins() -> Vec<String> {
    vec!["BTC".into(), "LTC".into(), "ETH".into()]
}

pub fn coin_symbol_valid(sym: &str) -> bool {
    let u = sym.trim().to_ascii_uppercase();
    COIN_CATALOG.iter().any(|(_, s)| *s == u)
}

#[derive(Debug, Clone, Default)]
pub struct CryptoQuote {
    pub symbol: String,
    pub usd: f64,
    pub change_24h: f64,
}

#[derive(Debug, Clone, Default)]
pub struct LiveSnapshot {
    pub ready: bool,
    pub city: String,
    pub region: String,
    pub country: String,
    pub temp_c: f32,
    pub weather: String,
    pub utc_offset_secs: i32,
    pub quotes: Vec<CryptoQuote>,
    pub error: String,
    #[allow(dead_code)]
    pub fetched_at: Option<Instant>,
}

pub enum LiveMsg {
    Update(LiveSnapshot),
}

pub struct LiveFeed {
    rx: Receiver<LiveMsg>,
    snap: LiveSnapshot,
}

impl LiveFeed {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || live_worker(tx));
        Self {
            rx,
            snap: LiveSnapshot::default(),
        }
    }

    pub fn poll(&mut self) {
        while let Ok(LiveMsg::Update(s)) = self.rx.try_recv() {
            self.snap = s;
        }
    }

    pub fn snap(&self) -> &LiveSnapshot {
        &self.snap
    }

    /// Quotes filtered + ordered by the user's header coin selection.
    pub fn quotes_for(&self, selected: &[String]) -> Vec<&CryptoQuote> {
        let mut out = Vec::new();
        for sel in selected {
            let want = sel.trim().to_ascii_uppercase();
            if let Some(q) = self
                .snap
                .quotes
                .iter()
                .find(|q| q.symbol.eq_ignore_ascii_case(&want))
            {
                out.push(q);
            }
        }
        out
    }

    pub fn local_now_label(&self) -> String {
        let offset = FixedOffset::east_opt(self.snap.utc_offset_secs)
            .unwrap_or(FixedOffset::east_opt(0).unwrap());
        let now = Utc::now().with_timezone(&offset);
        now.format("%a %d %b  ·  %H:%M:%S").to_string()
    }

    pub fn place_label(&self) -> String {
        if self.snap.city.is_empty() {
            "Locating…".into()
        } else if !self.snap.region.is_empty() {
            format!("{}, {}", self.snap.city, self.snap.country)
        } else {
            format!("{}, {}", self.snap.city, self.snap.country)
        }
    }

    /// Short ticker for the ESP LCD (`cmp netdata text=…`).
    pub fn board_ticker(&self) -> String {
        self.board_ticker_for(&default_header_coins())
    }

    pub fn board_ticker_for(&self, selected: &[String]) -> String {
        let mut parts: Vec<String> = Vec::new();
        let quotes = self.quotes_for(selected);
        for q in quotes.iter().take(3) {
            parts.push(format!("{} {}", q.symbol, format_usd(q.usd)));
        }
        if parts.is_empty() {
            for q in self.snap.quotes.iter().take(3) {
                parts.push(format!("{} {}", q.symbol, format_usd(q.usd)));
            }
        }
        if self.snap.ready && !self.snap.city.is_empty() {
            parts.push(format!(
                "{} {:.0}F {}",
                self.snap.city,
                self.snap.temp_c * 9.0 / 5.0 + 32.0,
                self.snap.weather
            ));
        }
        let t = self.local_now_label();
        if !t.is_empty() {
            parts.push(t);
        }
        if parts.is_empty() {
            "usb · companion linked".into()
        } else {
            parts.join(" · ")
        }
    }
}

fn live_worker(tx: Sender<LiveMsg>) {
    // First fetch quickly, then refresh on a calm cadence.
    let mut first = true;
    loop {
        let snap = fetch_all();
        let _ = tx.send(LiveMsg::Update(snap));
        thread::sleep(Duration::from_secs(if first { 45 } else { 60 }));
        first = false;
    }
}

fn fetch_all() -> LiveSnapshot {
    let mut snap = LiveSnapshot {
        fetched_at: Some(Instant::now()),
        ..Default::default()
    };

    match fetch_geo() {
        Ok(geo) => {
            snap.city = geo.city;
            snap.region = geo.region;
            snap.country = geo.country;
            if let Ok(wx) = fetch_weather(geo.lat, geo.lon) {
                snap.temp_c = wx.temp_c;
                snap.weather = wx.label;
                snap.utc_offset_secs = wx.utc_offset_secs;
            } else {
                snap.weather = "—".into();
            }
        }
        Err(e) => {
            snap.error = e;
        }
    }

    match fetch_prices() {
        Ok(q) => snap.quotes = q,
        Err(e) => {
            if snap.error.is_empty() {
                snap.error = e;
            }
        }
    }

    snap.ready = !snap.quotes.is_empty() || !snap.city.is_empty();
    snap
}

struct Geo {
    city: String,
    region: String,
    country: String,
    lat: f64,
    lon: f64,
}

#[derive(Deserialize)]
struct IpApiJson {
    status: Option<String>,
    message: Option<String>,
    city: Option<String>,
    #[serde(rename = "regionName")]
    region_name: Option<String>,
    #[serde(rename = "countryCode")]
    country_code: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
}

fn fetch_geo() -> Result<Geo, String> {
    let url = "http://ip-api.com/json/?fields=status,message,city,regionName,countryCode,lat,lon";
    let resp: IpApiJson = http_get_json(url)?;
    if resp.status.as_deref() != Some("success") {
        return Err(resp.message.unwrap_or_else(|| "geo lookup failed".into()));
    }
    Ok(Geo {
        city: resp.city.unwrap_or_else(|| "Unknown".into()),
        region: resp.region_name.unwrap_or_default(),
        country: resp.country_code.unwrap_or_default(),
        lat: resp.lat.unwrap_or(0.0),
        lon: resp.lon.unwrap_or(0.0),
    })
}

struct Weather {
    temp_c: f32,
    label: String,
    utc_offset_secs: i32,
}

#[derive(Deserialize)]
struct OpenMeteoJson {
    utc_offset_seconds: Option<i32>,
    current: Option<OpenMeteoCurrent>,
}

#[derive(Deserialize)]
struct OpenMeteoCurrent {
    temperature_2m: Option<f64>,
    weather_code: Option<i32>,
}

fn fetch_weather(lat: f64, lon: f64) -> Result<Weather, String> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}&current=temperature_2m,weather_code&timezone=auto"
    );
    let resp: OpenMeteoJson = http_get_json(&url)?;
    let cur = resp.current.ok_or_else(|| "no weather".to_string())?;
    let code = cur.weather_code.unwrap_or(0);
    Ok(Weather {
        temp_c: cur.temperature_2m.unwrap_or(0.0) as f32,
        label: weather_label(code).into(),
        utc_offset_secs: resp.utc_offset_seconds.unwrap_or(0),
    })
}

fn weather_label(code: i32) -> &'static str {
    match code {
        0 => "Clear",
        1 | 2 => "Fair",
        3 => "Cloudy",
        45 | 48 => "Fog",
        51 | 53 | 55 | 56 | 57 => "Drizzle",
        61 | 63 | 65 | 66 | 67 => "Rain",
        71 | 73 | 75 | 77 => "Snow",
        80 | 81 | 82 => "Showers",
        85 | 86 => "Snow showers",
        95 | 96 | 99 => "Thunder",
        _ => "Weather",
    }
}

#[derive(Deserialize)]
struct PriceRow {
    usd: Option<f64>,
    usd_24h_change: Option<f64>,
}

fn fetch_prices() -> Result<Vec<CryptoQuote>, String> {
    // CoinGecko free simple price — no API key.
    let ids: String = COIN_CATALOG
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>()
        .join(",");
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={ids}&vs_currencies=usd&include_24hr_change=true"
    );
    let map: serde_json::Map<String, serde_json::Value> = http_get_json(&url)?;
    let mut out = Vec::new();
    for (id, sym) in COIN_CATALOG {
        if let Some(v) = map.get(*id) {
            if let Ok(row) = serde_json::from_value::<PriceRow>(v.clone()) {
                out.push(CryptoQuote {
                    symbol: (*sym).into(),
                    usd: row.usd.unwrap_or(0.0),
                    change_24h: row.usd_24h_change.unwrap_or(0.0),
                });
            }
        }
    }
    if out.is_empty() {
        Err("no prices".into())
    } else {
        Ok(out)
    }
}

fn http_get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout_read(Duration::from_secs(12))
        .user_agent("Njordr-seas-CYD-miner/0.8.73")
        .build();
    let resp = agent.get(url).call().map_err(|e| format!("http: {e}"))?;
    resp.into_json::<T>().map_err(|e| format!("json: {e}"))
}

pub fn format_usd(v: f64) -> String {
    if v >= 1000.0 {
        format!("${v:.0}")
    } else if v >= 100.0 {
        format!("${v:.1}")
    } else if v >= 1.0 {
        format!("${v:.2}")
    } else {
        format!("${v:.4}")
    }
}

pub fn format_change(v: f64) -> String {
    format!("{v:+.1}%")
}
