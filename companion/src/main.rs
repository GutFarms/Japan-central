//! CYD Companion — USB-only mining control.
//! PC owns stratum/WiFi; board only hashes work received over USB-C.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api_feeds;
mod flash_update;
mod live_bar;
mod stratum;

use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use api_feeds::{load_feeds, next_feed_id, pull_feed, save_feeds, ApiFeed, ApiPullOutcome};
use flash_update::{
    fetch_latest_firmware, find_firmware_image, flash_merged_bin, update_needed, FirmwareImage,
};
use live_bar::{format_change, format_usd, LiveFeed};
use stratum::{
    encode_job_cmd, encode_job_parts, expected_shares_per_hour, urlenc, ShareOutcome, StratumClient,
    WorkJob,
};

use eframe::egui::{
    self, Align, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Layout, Margin,
    Pos2, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, TextEdit, Vec2,
};
use eframe::{App, NativeOptions};
use serde::{Deserialize, Serialize};
use serialport::SerialPort;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 820.0])
            .with_min_inner_size([1020.0, 700.0])
            .with_title("CYD Companion · USB SHA-256 Miner"),
        multisampling: 8,
        depth_buffer: 0,
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        "CYD Companion",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            install_fonts(&cc.egui_ctx);
            apply_theme(&cc.egui_ctx);
            Box::new(CompanionApp::new(cc.storage))
        }),
    )
}

const C_BG: Color32 = Color32::from_rgb(5, 8, 10);
const C_BG_2: Color32 = Color32::from_rgb(10, 16, 18);
const C_PANEL: Color32 = Color32::from_rgb(14, 22, 24);
const C_PANEL_SOFT: Color32 = Color32::from_rgb(22, 34, 36);
const C_PANEL_QUIET: Color32 = Color32::from_rgb(11, 18, 20);
const C_STROKE: Color32 = Color32::from_rgb(43, 63, 62);
const C_BUBBLE: Color32 = Color32::from_rgb(30, 49, 48);
const C_BUBBLE_HI: Color32 = Color32::from_rgb(113, 153, 141);
const C_LIME: Color32 = Color32::from_rgb(198, 255, 64);
const C_LIME_SOFT: Color32 = Color32::from_rgb(130, 210, 52);
const C_TEXT: Color32 = Color32::from_rgb(235, 244, 238);
const C_MUTED: Color32 = Color32::from_rgb(134, 154, 148);
const C_DIM: Color32 = Color32::from_rgb(82, 103, 99);
const C_WARN: Color32 = Color32::from_rgb(255, 196, 91);
const C_ERR: Color32 = Color32::from_rgb(255, 108, 91);
const MAX_LOGS: usize = 500;
const HASH_HISTORY_SAMPLES: usize = 90;

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Outfit-Regular".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Outfit-Regular.ttf")),
    );
    fonts.font_data.insert(
        "Outfit-Bold".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Outfit-Bold.ttf")),
    );
    fonts.font_data.insert(
        "JetBrainsMono-Regular".into(),
        FontData::from_static(include_bytes!(
            "../assets/fonts/JetBrainsMono-Regular.ttf"
        )),
    );
    fonts.font_data.insert(
        "JetBrainsMono-SemiBold".into(),
        FontData::from_static(include_bytes!(
            "../assets/fonts/JetBrainsMono-SemiBold.ttf"
        )),
    );

    fonts
        .families
        .insert(FontFamily::Name("Outfit Display".into()), vec!["Outfit-Bold".into()]);
    fonts.families.insert(
        FontFamily::Name("JetBrains Mono UI".into()),
        vec!["JetBrainsMono-SemiBold".into()],
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .splice(0..0, ["Outfit-Regular".into(), "Outfit-Bold".into()]);
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .splice(
            0..0,
            [
                "JetBrainsMono-Regular".into(),
                "JetBrainsMono-SemiBold".into(),
            ],
        );
    ctx.set_fonts(fonts);
}

fn display_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Outfit Display".into()))
}

fn mono_ui_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("JetBrains Mono UI".into()))
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = C_BG;
    style.visuals.window_fill = C_PANEL;
    style.visuals.extreme_bg_color = Color32::from_rgb(8, 13, 14);
    style.visuals.override_text_color = Some(C_TEXT);
    style.visuals.widgets.inactive.bg_fill = C_BUBBLE;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(42, 70, 66);
    style.visuals.widgets.active.bg_fill = C_LIME;
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, C_TEXT);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, C_TEXT);
    style.visuals.widgets.active.fg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(8, 16, 10));
    style.visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(198, 255, 64, 72);
    style.visuals.widgets.inactive.rounding = Rounding::same(16.0);
    style.visuals.widgets.hovered.rounding = Rounding::same(16.0);
    style.visuals.widgets.active.rounding = Rounding::same(16.0);
    style.spacing.item_spacing = Vec2::new(14.0, 11.0);
    style.spacing.button_padding = Vec2::new(18.0, 11.0);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        display_font(30.0),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );
    ctx.set_style(style);
}

fn bubble(ui: &mut egui::Ui, label: &str, lime: bool) -> egui::Response {
    let fill = if lime { C_LIME } else { C_BUBBLE_HI };
    let text = if lime {
        Color32::from_rgb(16, 28, 12)
    } else {
        Color32::from_rgb(12, 20, 28)
    };
    ui.add(
        egui::Button::new(RichText::new(label).strong().color(text).size(14.0))
            .fill(fill)
            .rounding(Rounding::same(22.0))
            .min_size(Vec2::new(110.0, 34.0)),
    )
}

fn stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

/// Uptime: `45s` → `3m 12s` → `2h 5m` → `1d 4h`.
fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        if s == 0 {
            format!("{m}m")
        } else {
            format!("{m}m {s}s")
        }
    } else if secs < 86_400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            format!("{h}h")
        } else {
            format!("{h}h {m}m")
        }
    } else {
        let d = secs / 86_400;
        let h = (secs % 86_400) / 3600;
        if h == 0 {
            format!("{d}d")
        } else {
            format!("{d}d {h}h")
        }
    }
}

fn scale_si(value: f64, units: &[&str]) -> (f64, usize) {
    let mut v = value.max(0.0);
    let mut i = 0usize;
    while v >= 1000.0 && i + 1 < units.len() {
        v /= 1000.0;
        i += 1;
    }
    (v, i)
}

fn fmt_scaled(v: f64, i: usize) -> String {
    if i == 0 {
        format!("{:.0}", v)
    } else if v >= 100.0 {
        format!("{v:.0}")
    } else if v >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// Total hashes: `812 H` → `12.4 kH` → `1.05 MH` → …
fn format_hash_count(n: u64) -> String {
    const UNITS: &[&str] = &["H", "kH", "MH", "GH", "TH", "PH"];
    if n < 1000 {
        return format!("{n} H");
    }
    let (v, i) = scale_si(n as f64, UNITS);
    format!("{} {}", fmt_scaled(v, i), UNITS[i])
}

/// Rate string: `812 H/s` → `12.4 kH/s` → `1.05 MH/s`.
fn format_hashrate(hs: f64) -> String {
    let (num, unit) = format_hashrate_parts(hs);
    format!("{num} {unit}")
}

fn format_hashrate_parts(hs: f64) -> (String, &'static str) {
    const UNITS: &[&str] = &["H/s", "kH/s", "MH/s", "GH/s", "TH/s"];
    let hs = hs.max(0.0);
    if hs < 1000.0 {
        return (format!("{hs:.0}"), "H/s");
    }
    let (v, i) = scale_si(hs, UNITS);
    (fmt_scaled(v, i), UNITS[i])
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Mine,
    Settings,
    Debug,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogKind {
    Info,
    Usb,
    Stratum,
    Warn,
    Err,
}

#[derive(Clone)]
struct LogEntry {
    time: String,
    kind: LogKind,
    text: String,
}

#[derive(Clone, Default)]
struct StratumLive {
    endpoint: String,
    phase: String,
    connected: bool,
    authorized: bool,
    difficulty: f64,
    jobs: u32,
    accepted: u32,
    rejected: u32,
    lines_rx: u64,
    lines_tx: u64,
    last_job: String,
    last_rx: String,
    last_tx: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct StatusJson {
    #[serde(default)]
    hashrate_hs: f64,
    #[serde(default)]
    hashrate_khs: f64,
    #[serde(default)]
    shares: u64,
    #[serde(default)]
    hashes: u64,
    #[serde(default)]
    mining: bool,
    #[serde(default)]
    accepted: u32,
    #[serde(default)]
    rejected: u32,
    #[serde(default)]
    pool: String,
    #[serde(default)]
    connected: bool,
    #[serde(default)]
    cpu_mhz: u8,
    #[serde(default)]
    uptime_secs: u64,
    #[serde(default)]
    nonce: String,
    #[serde(default)]
    fw: String,
    #[serde(default)]
    link: String,
    #[serde(default)]
    job: String,
    #[serde(default)]
    sha_mode: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigJson {
    #[serde(default)]
    cpu_mhz: u8,
    #[serde(default)]
    hash_focus: bool,
    #[serde(default)]
    fw: String,
    #[serde(default)]
    mode: String,
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedMine {
    #[serde(default)]
    stratum: String,
    #[serde(default)]
    worker: String,
    #[serde(default)]
    password: String,
    #[serde(default = "default_mhz")]
    cpu_mhz: u8,
    #[serde(default)]
    com_port: String,
    #[serde(default)]
    auto_connect: bool,
    #[serde(default)]
    wizard_done: bool,
}

fn default_mhz() -> u8 {
    240
}

#[derive(Clone)]
struct ShareRow {
    time: String,
    accepted: bool,
    detail: String,
    latency_ms: Option<u64>,
}

const POOL_PRESETS: &[(&str, &str)] = &[
    ("Public Pool", "stratum+tcp://public-pool.io:21496"),
    ("Public Pool EU", "stratum+tcp://eu.public-pool.io:21496"),
    ("NerdMiner", "stratum+tcp://pool.nerdminers.org:3333"),
];

enum NetMsg {
    Ports(Vec<String>),
    Action(Result<String, String>),
    Status(Result<StatusJson, String>),
    Config(Result<ConfigJson, String>),
    MineStats {
        accepted: u32,
        rejected: u32,
        phase: String,
    },
    Stratum(StratumLive),
    Log {
        kind: LogKind,
        text: String,
    },
    Terminal(String),
    /// Firmware flash finished; `reopen` is the COM port to reclaim if flash succeeded.
    FlashDone {
        result: Result<String, String>,
        reopen: Option<String>,
    },
    Share(ShareOutcome),
    FirmwareFetched(Result<FirmwareImage, String>),
    ApiFeedResult(ApiPullOutcome),
}

enum NetCmd {
    ListPorts,
    OpenUsb(String),
    CloseUsb,
    StartMine {
        stratum: String,
        worker: String,
        password: String,
    },
    StopMine,
    SetClock(u8),
    PollStatus,
    Bench,
    UsbRaw(String),
    /// Stop mining, release USB, flash merged.bin @ 0x0, optionally reopen.
    UpdateFirmware {
        port: String,
        image: String,
        reopen: bool,
    },
    /// Push live ticker text to the ESP LCD.
    PushNet {
        text: String,
    },
    RebootBoard,
    FetchFirmware,
    /// Pull one user-configured API feed in the worker thread.
    PullApiFeed(ApiFeed),
}

struct CompanionApp {
    tab: Tab,
    com_port: String,
    ports: Vec<String>,
    usb_open: bool,
    mining: bool,
    edit_stratum: String,
    edit_worker: String,
    edit_password: String,
    target_mhz: u8,
    status: StatusJson,
    fw_label: String,
    pool_phase: String,
    accepted: u32,
    rejected: u32,
    last_error: String,
    last_ok: String,
    cmd_tx: Sender<NetCmd>,
    msg_rx: Receiver<NetMsg>,
    last_poll: Instant,
    pulse: f32,
    /// 0..1 scroll phase for background grid (wraps exactly one line spacing).
    grid_phase: f32,
    /// 0..1 progress toward the next hashrate sample (smooths sparkline scroll).
    history_phase: f32,
    displayed_khs: f32,
    hashrate_history: VecDeque<f32>,
    last_hash_sample: Instant,
    logs: VecDeque<LogEntry>,
    log_auto_scroll: bool,
    stratum_live: StratumLive,
    term_input: String,
    term_history: VecDeque<String>,
    term_out: VecDeque<String>,
    live: LiveFeed,
    firmware: Option<FirmwareImage>,
    update_confirm: bool,
    update_busy: bool,
    update_status: String,
    auto_connect: bool,
    auto_connect_attempted: bool,
    session_started: Option<Instant>,
    session_hash_start: u64,
    share_history: VecDeque<ShareRow>,
    last_net_push: Instant,
    last_ticker: String,
    session_accepted: u32,
    session_rejected: u32,
    last_share_latency_ms: Option<u64>,
    /// None = wizard dismissed; Some(0..3) = step.
    wizard_step: Option<u8>,
    fetch_busy: bool,
    /// User-configured HTTP APIs that pull external info into the app.
    api_feeds: Vec<ApiFeed>,
    api_draft_name: String,
    api_draft_url: String,
    api_draft_auth: String,
    api_draft_path: String,
    api_pulling_id: Option<u64>,
    last_api_auto_pull: Instant,
}

impl CompanionApp {
    fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<NetCmd>();
        let (msg_tx, msg_rx) = mpsc::channel::<NetMsg>();
        thread::spawn(move || mine_worker(cmd_rx, msg_tx));
        let _ = cmd_tx.send(NetCmd::ListPorts);

        let mut edit_stratum = "stratum+tcp://public-pool.io:21496".into();
        let mut edit_worker = String::new();
        let mut edit_password = "x".into();
        let mut target_mhz = 240u8;
        let mut com_port = String::new();
        let mut auto_connect = false;
        let mut wizard_done = false;
        let mut api_feeds: Vec<ApiFeed> = Vec::new();
        if let Some(storage) = storage {
            if let Some(raw) = storage.get_string("mine_prefs") {
                if let Ok(p) = serde_json::from_str::<PersistedMine>(&raw) {
                    if !p.stratum.is_empty() {
                        edit_stratum = p.stratum;
                    }
                    edit_worker = p.worker;
                    if !p.password.is_empty() {
                        edit_password = p.password;
                    }
                    target_mhz = match p.cpu_mhz {
                        80 | 160 | 240 => p.cpu_mhz,
                        _ => 240,
                    };
                    com_port = p.com_port;
                    auto_connect = p.auto_connect;
                    wizard_done = p.wizard_done;
                }
            }
            if let Some(raw) = storage.get_string("api_feeds") {
                api_feeds = load_feeds(&raw);
            }
        }

        let mut app = Self {
            tab: Tab::Mine,
            com_port,
            ports: Vec::new(),
            usb_open: false,
            mining: false,
            edit_stratum,
            edit_worker,
            edit_password,
            target_mhz,
            status: StatusJson::default(),
            fw_label: "—".into(),
            pool_phase: "off".into(),
            accepted: 0,
            rejected: 0,
            last_error: String::new(),
            last_ok: "Connect USB-C, enter BTC address, Start mining. SHA-256 on board · pool on PC."
                .into(),
            cmd_tx,
            msg_rx,
            last_poll: Instant::now() - Duration::from_secs(10),
            pulse: 0.0,
            grid_phase: 0.0,
            history_phase: 0.0,
            displayed_khs: 0.0,
            hashrate_history: VecDeque::from(vec![0.0; HASH_HISTORY_SAMPLES]),
            last_hash_sample: Instant::now(),
            logs: VecDeque::new(),
            log_auto_scroll: true,
            stratum_live: StratumLive::default(),
            term_input: "cmp ping".into(),
            term_history: VecDeque::new(),
            term_out: VecDeque::new(),
            live: LiveFeed::start(),
            firmware: find_firmware_image().ok(),
            update_confirm: false,
            update_busy: false,
            update_status: String::new(),
            auto_connect,
            auto_connect_attempted: false,
            session_started: None,
            session_hash_start: 0,
            share_history: VecDeque::new(),
            last_net_push: Instant::now() - Duration::from_secs(120),
            last_ticker: String::new(),
            session_accepted: 0,
            session_rejected: 0,
            last_share_latency_ms: None,
            wizard_step: if wizard_done { None } else { Some(0) },
            fetch_busy: false,
            api_feeds,
            api_draft_name: String::new(),
            api_draft_url: String::new(),
            api_draft_auth: String::new(),
            api_draft_path: String::new(),
            api_pulling_id: None,
            last_api_auto_pull: Instant::now() - Duration::from_secs(120),
        };
        app.push_log(LogKind::Info, "CYD Companion ready".into());
        if let Some(fw) = &app.firmware {
            app.push_log(
                LogKind::Info,
                format!(
                    "Bundled firmware ready · {} ({} KB)",
                    fw.path.display(),
                    fw.bytes / 1024
                ),
            );
        } else {
            app.push_log(
                LogKind::Warn,
                "No bundled firmware found — Update board needs Firmware\\ next to the app (Miner kit)."
                    .into(),
            );
        }
        app
    }

    fn board_khs(&self) -> f32 {
        if self.status.hashrate_khs > 0.0 {
            self.status.hashrate_khs as f32
        } else if self.status.hashrate_hs > 0.0 {
            (self.status.hashrate_hs / 1000.0) as f32
        } else {
            0.0
        }
    }

    fn board_hashing(&self) -> bool {
        self.status.mining
            || self.status.connected
            || self.status.hashrate_hs > 0.0
            || self.status.hashes > 0
            || (!self.status.nonce.is_empty() && self.status.nonce != "00000000")
    }

    fn pool_state(&self) -> (&'static str, Color32) {
        if self.stratum_live.authorized {
            ("AUTHORIZED", C_LIME)
        } else if self.stratum_live.phase == "err" {
            ("ERROR", C_ERR)
        } else if self.stratum_live.connected
            || (self.stratum_live.phase != "off" && !self.stratum_live.phase.is_empty())
        {
            ("LINKING", C_WARN)
        } else {
            ("IDLE", C_MUTED)
        }
    }

    fn update_motion(&mut self, ctx: &egui::Context) {
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.05);
        let pace = if self.mining || self.board_hashing() {
            2.15
        } else {
            0.75
        };
        // Keep pulse continuous for sin()-based motion. Wrapping at τ made
        // position drifts/sweeps hitch every loop.
        self.pulse = (self.pulse + dt * pace).rem_euclid(std::f32::consts::TAU * 64.0);
        // Grid wraps by exactly one spacing → seamless.
        let grid_speed = if self.mining || self.board_hashing() {
            0.22
        } else {
            0.08
        };
        self.grid_phase = (self.grid_phase + dt * grid_speed).rem_euclid(1.0);

        let target = self.board_khs();
        let alpha = 1.0 - (-dt * 7.5).exp();
        self.displayed_khs += (target - self.displayed_khs) * alpha;
        if self.displayed_khs.abs() < 0.001 {
            self.displayed_khs = 0.0;
        }

        const SAMPLE_MS: f32 = 560.0;
        let sample_elapsed = self.last_hash_sample.elapsed().as_secs_f32() * 1000.0;
        self.history_phase = (sample_elapsed / SAMPLE_MS).clamp(0.0, 1.0);
        if sample_elapsed >= SAMPLE_MS {
            self.hashrate_history.push_back(self.displayed_khs.max(0.0));
            while self.hashrate_history.len() > HASH_HISTORY_SAMPLES {
                self.hashrate_history.pop_front();
            }
            self.last_hash_sample = Instant::now();
            self.history_phase = 0.0;
        }
    }

    fn push_log(&mut self, kind: LogKind, text: String) {
        self.logs.push_back(LogEntry {
            time: stamp(),
            kind,
            text,
        });
        while self.logs.len() > MAX_LOGS {
            self.logs.pop_front();
        }
    }

    fn persist(&self, storage: &mut dyn eframe::Storage) {
        let p = PersistedMine {
            stratum: self.edit_stratum.clone(),
            worker: self.edit_worker.clone(),
            password: self.edit_password.clone(),
            cpu_mhz: self.target_mhz,
            com_port: self.com_port.clone(),
            auto_connect: self.auto_connect,
            wizard_done: self.wizard_step.is_none(),
        };
        if let Ok(raw) = serde_json::to_string(&p) {
            storage.set_string("mine_prefs", raw);
        }
        storage.set_string("api_feeds", save_feeds(&self.api_feeds));
    }

    fn request_api_pull(&mut self, id: u64) {
        let Some(feed) = self.api_feeds.iter().find(|f| f.id == id).cloned() else {
            return;
        };
        self.api_pulling_id = Some(id);
        let _ = self.cmd_tx.send(NetCmd::PullApiFeed(feed));
        self.push_log(LogKind::Info, format!("API pull → id={id}"));
    }

    fn add_api_feed_from_draft(&mut self) {
        let name = self.api_draft_name.trim().to_string();
        let url = self.api_draft_url.trim().to_string();
        if name.is_empty() || !(url.starts_with("http://") || url.starts_with("https://")) {
            self.last_error = "API needs a name and http(s) URL.".into();
            return;
        }
        let id = next_feed_id(&self.api_feeds);
        let mut feed = ApiFeed::new(id, name, url);
        feed.auth = self.api_draft_auth.trim().to_string();
        feed.json_path = self.api_draft_path.trim().to_string();
        self.api_feeds.push(feed);
        self.api_draft_name.clear();
        self.api_draft_url.clear();
        self.api_draft_auth.clear();
        self.api_draft_path.clear();
        self.last_ok = "API feed added — Pull now to load data into the app.".into();
        self.push_log(LogKind::Info, "API feed added".into());
        if let Some(last) = self.api_feeds.last() {
            self.request_api_pull(last.id);
        }
    }

    fn apply_api_pull(&mut self, outcome: ApiPullOutcome) {
        if self.api_pulling_id == Some(outcome.id) {
            self.api_pulling_id = None;
        }
        if let Some(feed) = self.api_feeds.iter_mut().find(|f| f.id == outcome.id) {
            feed.last_status = outcome.status.clone();
            feed.last_summary = outcome.summary.clone();
            feed.last_preview = outcome.preview;
            feed.last_pulled_ms = outcome.pulled_ms;
            let name = feed.name.clone();
            if outcome.ok {
                self.last_ok = format!(
                    "API {name}: {}{}",
                    outcome.status,
                    if outcome.summary.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", outcome.summary)
                    }
                );
                self.push_log(LogKind::Info, self.last_ok.clone());
            } else {
                self.last_error = format!("API {name}: {}", outcome.status);
                self.push_log(LogKind::Warn, self.last_error.clone());
            }
        }
    }

    fn connect_usb(&mut self) {
        self.last_error.clear();
        if self.com_port.is_empty() {
            self.last_error = "Select a COM / serial port.".into();
            return;
        }
        let _ = self.cmd_tx.send(NetCmd::OpenUsb(self.com_port.clone()));
        self.last_ok = format!("Opening {}…", self.com_port);
        self.push_log(LogKind::Usb, format!("Opening {}", self.com_port));
    }

    fn start_mine(&mut self) {
        self.last_error.clear();
        if !self.usb_open {
            self.last_error = "Connect USB first.".into();
            return;
        }
        if self.edit_worker.trim().is_empty() {
            self.last_error = "Enter worker / Bitcoin address.".into();
            return;
        }
        let _ = self.cmd_tx.send(NetCmd::StartMine {
            stratum: self.edit_stratum.trim().to_string(),
            worker: self.edit_worker.trim().to_string(),
            password: self.edit_password.clone(),
        });
        self.mining = true;
        self.session_started = Some(Instant::now());
        self.session_hash_start = self.status.hashes;
        self.reset_share_session_ui();
        self.last_ok = "Starting pool on PC → pushing work over USB…".into();
        self.push_log(
            LogKind::Info,
            format!(
                "Start mining → {} as {}",
                self.edit_stratum.trim(),
                self.edit_worker.trim()
            ),
        );
    }

    fn stop_mine(&mut self) {
        let _ = self.cmd_tx.send(NetCmd::StopMine);
        self.mining = false;
        self.session_started = None;
        self.last_ok = "Mining stopped.".into();
        self.push_log(LogKind::Info, "Mining stopped".into());
    }

    fn push_board_ticker(&mut self, force: bool) {
        if !self.usb_open || self.update_busy {
            return;
        }
        let tick = self.live.board_ticker();
        if !force && tick == self.last_ticker && self.last_net_push.elapsed() < Duration::from_secs(45)
        {
            return;
        }
        if !force && self.last_net_push.elapsed() < Duration::from_secs(40) {
            return;
        }
        self.last_ticker = tick.clone();
        self.last_net_push = Instant::now();
        let _ = self.cmd_tx.send(NetCmd::PushNet { text: tick });
    }

    fn session_elapsed_label(&self) -> String {
        match self.session_started {
            Some(t) => format_uptime(t.elapsed().as_secs()),
            None => "—".into(),
        }
    }

    fn accept_rate_label(&self) -> String {
        if !self.stratum_live.authorized {
            return "—".into();
        }
        let total = self.session_accepted + self.session_rejected;
        if total == 0 {
            "—".into()
        } else {
            format!(
                "{:.0}%",
                100.0 * self.session_accepted as f64 / total as f64
            )
        }
    }

    fn expected_shares_label(&self) -> String {
        let exp = expected_shares_per_hour(
            self.status.hashrate_hs,
            self.stratum_live.difficulty,
        );
        if exp <= 0.0 {
            "—".into()
        } else if exp >= 10.0 {
            format!("{exp:.1}/h")
        } else if exp >= 1.0 {
            format!("{exp:.2}/h")
        } else {
            format!("{exp:.3}/h")
        }
    }

    fn luck_label(&self) -> String {
        if !self.stratum_live.authorized {
            return "—".into();
        }
        let Some(started) = self.session_started else {
            return "—".into();
        };
        let hours = (started.elapsed().as_secs_f64() / 3600.0).max(1.0 / 3600.0);
        let expected = expected_shares_per_hour(
            self.status.hashrate_hs,
            self.stratum_live.difficulty,
        ) * hours;
        if expected < 0.01 && self.session_accepted == 0 {
            return "—".into();
        }
        format!(
            "{:.2} exp · {} ok",
            expected, self.session_accepted
        )
    }

    fn reset_share_session_ui(&mut self) {
        self.session_accepted = 0;
        self.session_rejected = 0;
        self.accepted = 0;
        self.rejected = 0;
        self.last_share_latency_ms = None;
        self.share_history.clear();
    }

    fn sha_mode_label(&self) -> &str {
        if !self.status.sha_mode.is_empty() {
            &self.status.sha_mode
        } else if self.status.pool.contains("HW") {
            // Fallback parse from pool string like SHA256-HW+
            if let Some(rest) = self.status.pool.strip_prefix("SHA256-") {
                return rest;
            }
            "—"
        } else {
            "—"
        }
    }

    fn firmware_status_label(&self) -> (String, Color32) {
        let board = if self.fw_label.is_empty() || self.fw_label == "—" {
            String::new()
        } else {
            self.fw_label.clone()
        };
        let bundled = self
            .firmware
            .as_ref()
            .map(|f| f.version.clone())
            .unwrap_or_default();
        match update_needed(&board, &bundled) {
            Some(true) => (
                format!("Update available · board {board} → kit {bundled}"),
                C_WARN,
            ),
            Some(false) => (format!("Firmware up to date · {board}"), C_LIME),
            None if !bundled.is_empty() => (
                format!("Bundled {bundled} · connect board to compare"),
                C_MUTED,
            ),
            None => ("Firmware status unknown".into(), C_MUTED),
        }
    }

    fn session_hashes_label(&self) -> String {
        if self.session_started.is_none() {
            return "—".into();
        }
        let delta = self.status.hashes.saturating_sub(self.session_hash_start);
        format_hash_count(delta)
    }

    fn copy_logs_to_clipboard(&self, ctx: &egui::Context) {
        let mut out = String::new();
        for e in &self.logs {
            let tag = match e.kind {
                LogKind::Info => "INFO",
                LogKind::Usb => "USB",
                LogKind::Stratum => "POOL",
                LogKind::Warn => "WARN",
                LogKind::Err => "ERR",
            };
            out.push_str(&format!("{} [{}] {}\n", e.time, tag, e.text));
        }
        ctx.output_mut(|o| o.copied_text = out);
    }

    fn request_board_update(&mut self) {
        self.firmware = find_firmware_image().ok().or_else(|| self.firmware.clone());
        if self.firmware.is_none() {
            self.last_error =
                "Firmware image not found. Use Fetch latest or install the Miner kit."
                    .into();
            self.push_log(LogKind::Err, self.last_error.clone());
            return;
        }
        if self.com_port.trim().is_empty() {
            self.last_error = "Select a COM / serial port before updating.".into();
            return;
        }
        // Soft-block when already matching — still allow force via confirm dialog.
        self.update_confirm = true;
    }

    fn start_firmware_fetch(&mut self) {
        if self.fetch_busy {
            return;
        }
        self.fetch_busy = true;
        self.update_status = "Fetching latest firmware…".into();
        self.push_log(LogKind::Info, "Fetching latest firmware…".into());
        let _ = self.cmd_tx.send(NetCmd::FetchFirmware);
    }

    fn begin_board_update(&mut self) {
        self.update_confirm = false;
        let Some(fw) = self.firmware.clone() else {
            self.last_error = "No firmware image available.".into();
            return;
        };
        if self.com_port.trim().is_empty() {
            self.last_error = "Select a COM / serial port before updating.".into();
            return;
        }
        let reopen = self.usb_open;
        if self.mining {
            self.stop_mine();
        }
        self.update_busy = true;
        self.update_status = format!("Updating board via {}…", self.com_port);
        self.last_ok = self.update_status.clone();
        self.last_error.clear();
        self.push_log(
            LogKind::Usb,
            format!(
                "Update board → {} ({} KB) on {}",
                fw.path.display(),
                fw.bytes / 1024,
                self.com_port
            ),
        );
        // Release USB in the worker before flash (port must be free).
        self.usb_open = false;
        self.mining = false;
        let _ = self.cmd_tx.send(NetCmd::UpdateFirmware {
            port: self.com_port.clone(),
            image: fw.path.to_string_lossy().into_owned(),
            reopen,
        });
    }

    fn send_term(&mut self) {
        let cmd = self.term_input.trim().to_string();
        if cmd.is_empty() {
            return;
        }
        if !self.usb_open {
            self.push_log(LogKind::Err, "USB not open — Connect first".into());
            return;
        }
        self.term_history.push_back(format!(">>> {cmd}"));
        while self.term_history.len() > 200 {
            self.term_history.pop_front();
        }
        self.push_log(LogKind::Usb, format!("TX {cmd}"));
        let _ = self.cmd_tx.send(NetCmd::UsbRaw(cmd));
    }

    fn ui_mine(&mut self, ui: &mut egui::Ui) {
        self.ui_mining_hero(ui);
        ui.add_space(18.0);

        ui.columns(2, |columns| {
            let (left, right) = columns.split_at_mut(1);
            self.ui_connection_controls(&mut left[0]);
            self.ui_telemetry_rail(&mut right[0]);
        });

        ui.add_space(16.0);
        self.ui_api_feeds_mine(ui);
        self.ui_stratum_panel(ui);
        ui.add_space(12.0);
        self.ui_logs_panel(ui);
    }

    fn ui_mining_hero(&mut self, ui: &mut egui::Ui) {
        let pulse = 0.5 + 0.5 * self.pulse.sin();
        let pulse_alpha = if self.mining || self.board_hashing() {
            (38.0 + pulse * 58.0) as u8
        } else {
            20
        };
        Frame::none()
            .fill(Color32::from_rgba_unmultiplied(10, 18, 17, 232))
            .rounding(Rounding::same(28.0))
            .stroke(Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(198, 255, 64, pulse_alpha),
            ))
            .inner_margin(Margin::symmetric(28.0, 24.0))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                paint_hero_wash(ui, rect, self.pulse, self.mining || self.board_hashing());
                ui.set_min_height(278.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width((ui.available_width() * 0.58).clamp(420.0, 720.0));
                        ui.label(
                            RichText::new("CYD")
                                .color(C_LIME)
                                .font(display_font(54.0)),
                        );
                        ui.label(
                            RichText::new("USB SHA-256 miner")
                                .color(C_TEXT)
                                .font(display_font(22.0)),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(if self.board_hashing() {
                                format!(
                                    "Board measured {} · path {} · nonce {} · {}",
                                    format_hashrate(self.status.hashrate_hs),
                                    self.sha_mode_label(),
                                    if self.status.nonce.is_empty() {
                                        "—"
                                    } else {
                                        &self.status.nonce
                                    },
                                    format_hash_count(self.status.hashes)
                                )
                            } else if self.mining {
                                "Jobs streaming — waiting for board hashrate…".into()
                            } else if self.usb_open {
                                "USB linked. Start mining to stream pool work to the board.".into()
                            } else {
                                "Connect the board, route a pool, then bring hashpower online."
                                    .into()
                            })
                            .color(C_MUTED)
                            .size(15.0),
                        );
                        ui.add_space(14.0);
                        let (rate_num, rate_unit) =
                            format_hashrate_parts(self.displayed_khs as f64 * 1000.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(rate_num)
                                    .color(C_TEXT)
                                    .font(display_font(72.0)),
                            );
                            ui.vertical(|ui| {
                                ui.add_space(22.0);
                                ui.label(
                                    RichText::new(rate_unit)
                                        .color(C_LIME)
                                        .font(display_font(26.0)),
                                );
                                ui.label(
                                    RichText::new("board measured")
                                        .color(C_DIM)
                                        .font(mono_ui_font(10.0)),
                                );
                            });
                        });
                        ui.label(
                            RichText::new(format!(
                                "SHA path {} · classic ESP32 ceiling is typically ~0.7–0.8 MH/s, not 1 MH/s marketing",
                                self.sha_mode_label()
                            ))
                            .color(C_DIM)
                            .font(mono_ui_font(11.0)),
                        );
                        ui.add_space(10.0);
                        sparkline(ui, &self.hashrate_history, self.pulse, self.history_phase);
                        ui.add_space(8.0);
                        hash_activity_bars(ui, self.displayed_khs, self.pulse, self.board_hashing());
                    });

                    ui.add_space(12.0);
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        let (pool_label, pool_color) = self.pool_state();
                        ui.label(
                            RichText::new(format!(
                                "USB {} · POOL {}",
                                if self.usb_open { "LINKED" } else { "IDLE" },
                                pool_label
                            ))
                            .color(pool_color)
                            .font(mono_ui_font(12.0)),
                        );
                        ui.add_space(10.0);
                        let usb_label = if self.usb_open {
                            "Disconnect USB"
                        } else {
                            "Connect USB"
                        };
                        if cta_button(ui, usb_label, !self.usb_open, 210.0).clicked() {
                            if self.usb_open {
                                let _ = self.cmd_tx.send(NetCmd::CloseUsb);
                                self.usb_open = false;
                                self.mining = false;
                                self.push_log(LogKind::Usb, "Disconnect requested".into());
                            } else {
                                self.connect_usb();
                            }
                        }
                        ui.add_space(8.0);
                        let mine_label = if self.mining {
                            "Stop mining"
                        } else {
                            "Start mining"
                        };
                        if cta_button(ui, mine_label, !self.mining, 210.0).clicked() {
                            if self.mining {
                                self.stop_mine();
                            } else {
                                self.start_mine();
                            }
                        }
                    });
                });
            });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Board tools", |ui| {
            ui.label(
                RichText::new("Bench, flash, and firmware fetch — kept off the Mine screen.")
                    .color(C_MUTED)
                    .size(13.0),
            );
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                if soft_button(ui, "Bench board", 160.0).clicked() {
                    let _ = self.cmd_tx.send(NetCmd::Bench);
                    self.push_log(LogKind::Usb, "Bench requested".into());
                }
                let update_label = if self.update_busy {
                    "Updating…"
                } else {
                    "Update board"
                };
                if soft_button(ui, update_label, 160.0).clicked() && !self.update_busy {
                    self.request_board_update();
                }
                let fetch_label = if self.fetch_busy {
                    "Fetching…"
                } else {
                    "Fetch latest FW"
                };
                if soft_button(ui, fetch_label, 160.0).clicked() && !self.fetch_busy {
                    self.start_firmware_fetch();
                }
            });
            ui.add_space(10.0);
            let (fw_status, fw_color) = self.firmware_status_label();
            ui.label(
                RichText::new(fw_status)
                    .color(fw_color)
                    .font(mono_ui_font(11.0)),
            );
            if !self.update_status.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(&self.update_status)
                        .color(if self.update_busy || self.fetch_busy {
                            C_WARN
                        } else {
                            C_MUTED
                        })
                        .font(mono_ui_font(11.0)),
                );
            }
            if let Some(fw) = &self.firmware {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "Bundled · {} · {} KB",
                        if fw.version.is_empty() {
                            "unknown"
                        } else {
                            &fw.version
                        },
                        fw.bytes / 1024
                    ))
                    .color(C_DIM)
                    .font(mono_ui_font(10.0)),
                );
                ui.label(
                    RichText::new(fw.path.display().to_string())
                        .color(C_DIM)
                        .font(mono_ui_font(10.0)),
                );
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "Port · {}  ·  board fw {}",
                    if self.com_port.is_empty() {
                        "—"
                    } else {
                        &self.com_port
                    },
                    if self.fw_label.is_empty() {
                        "—"
                    } else {
                        &self.fw_label
                    }
                ))
                .color(C_MUTED)
                .font(mono_ui_font(11.0)),
            );
        });

        ui.add_space(14.0);
        soft_panel(ui, "Preferences", |ui| {
            ui.checkbox(&mut self.auto_connect, "Auto-connect USB on launch");
            ui.add_space(8.0);
            ui.label(
                RichText::new("Tip: hold BOOT, tap RESET, release BOOT if Update board fails.")
                    .color(C_DIM)
                    .size(12.0),
            );
        });

        ui.add_space(14.0);
        self.ui_api_feeds(ui);
    }

    fn ui_api_feeds(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "API feeds", |ui| {
            ui.label(
                RichText::new(
                    "Add HTTPS APIs to pull info from other sites into Companion. Optional JSON path picks a field for the summary (e.g. data.price).",
                )
                .color(C_MUTED)
                .size(13.0),
            );
            ui.add_space(10.0);
            labeled_edit(ui, "Name", &mut self.api_draft_name, "Weather · Markets · Custom");
            labeled_edit(
                ui,
                "URL",
                &mut self.api_draft_url,
                "https://api.example.com/v1/info",
            );
            labeled_edit(
                ui,
                "Auth (optional)",
                &mut self.api_draft_auth,
                "Bearer …  or  X-Api-Key: …",
            );
            labeled_edit(
                ui,
                "JSON path (optional)",
                &mut self.api_draft_path,
                "data.price",
            );
            ui.add_space(8.0);
            if soft_button(ui, "Add API feed", 160.0).clicked() {
                self.add_api_feed_from_draft();
            }

            if self.api_feeds.is_empty() {
                ui.add_space(10.0);
                ui.label(
                    RichText::new("No API feeds yet — add a URL above to pull live data.")
                        .color(C_DIM)
                        .size(12.0),
                );
                return;
            }

            ui.add_space(12.0);
            let mut pull_id: Option<u64> = None;
            let mut remove_id: Option<u64> = None;
            let pulling = self.api_pulling_id;
            for feed in &mut self.api_feeds {
                Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(4, 8, 9, 160))
                    .rounding(Rounding::same(12.0))
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut feed.enabled, "");
                            ui.label(
                                RichText::new(&feed.name)
                                    .color(C_TEXT)
                                    .font(mono_ui_font(13.0)),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if soft_button(ui, "Remove", 88.0).clicked() {
                                    remove_id = Some(feed.id);
                                }
                                let pull_label = if pulling == Some(feed.id) {
                                    "Pulling…"
                                } else {
                                    "Pull now"
                                };
                                if soft_button(ui, pull_label, 100.0).clicked()
                                    && pulling.is_none()
                                    && feed.enabled
                                {
                                    pull_id = Some(feed.id);
                                }
                            });
                        });
                        ui.label(
                            RichText::new(&feed.url)
                                .color(C_DIM)
                                .font(mono_ui_font(10.0)),
                        );
                        if !feed.last_status.is_empty() {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!(
                                    "{}{}",
                                    feed.last_status,
                                    if feed.last_summary.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" · {}", feed.last_summary)
                                    }
                                ))
                                .color(C_LIME)
                                .font(mono_ui_font(11.0)),
                            );
                        }
                        if !feed.last_preview.is_empty() {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(trunc(&feed.last_preview, 220))
                                    .color(C_MUTED)
                                    .font(mono_ui_font(10.0)),
                            );
                        }
                    });
                ui.add_space(8.0);
            }
            if let Some(id) = pull_id {
                self.request_api_pull(id);
            }
            if let Some(id) = remove_id {
                self.api_feeds.retain(|f| f.id != id);
                self.push_log(LogKind::Info, format!("API feed removed id={id}"));
            }
        });
    }

    fn ui_api_feeds_mine(&self, ui: &mut egui::Ui) {
        let active: Vec<_> = self
            .api_feeds
            .iter()
            .filter(|f| f.enabled && (!f.last_summary.is_empty() || !f.last_status.is_empty()))
            .collect();
        if active.is_empty() {
            return;
        }
        soft_panel(ui, "API feeds", |ui| {
            for feed in active.iter().take(6) {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&feed.name)
                            .color(C_LIME)
                            .font(mono_ui_font(11.0)),
                    );
                    ui.label(
                        RichText::new(if feed.last_summary.is_empty() {
                            feed.last_status.clone()
                        } else {
                            feed.last_summary.clone()
                        })
                        .color(C_TEXT)
                        .font(mono_ui_font(11.0)),
                    );
                });
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new("Manage feeds in Settings → API feeds")
                    .color(C_DIM)
                    .font(mono_ui_font(10.0)),
            );
        });
        ui.add_space(12.0);
    }

    fn ui_connection_controls(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Control routing", |ui| {
            ui.label(
                RichText::new("USB-C")
                    .color(C_LIME)
                    .font(mono_ui_font(12.0)),
            );
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_source("com")
                    .width(210.0)
                    .selected_text(if self.com_port.is_empty() {
                        "Select port"
                    } else {
                        &self.com_port
                    })
                    .show_ui(ui, |ui| {
                        for p in &self.ports {
                            ui.selectable_value(&mut self.com_port, p.clone(), p);
                        }
                    });
                if soft_button(ui, "Refresh", 98.0).clicked() {
                    let _ = self.cmd_tx.send(NetCmd::ListPorts);
                }
                if soft_button(
                    ui,
                    if self.usb_open { "Disconnect" } else { "Connect" },
                    112.0,
                )
                .clicked()
                {
                    if self.usb_open {
                        let _ = self.cmd_tx.send(NetCmd::CloseUsb);
                        self.usb_open = false;
                        self.mining = false;
                        self.session_started = None;
                        self.push_log(LogKind::Usb, "Disconnect requested".into());
                    } else {
                        self.connect_usb();
                    }
                }
            });
            ui.add_space(16.0);
            ui.label(
                RichText::new("POOL")
                    .color(C_LIME)
                    .font(mono_ui_font(12.0)),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Presets").color(C_MUTED).size(12.0));
                for (name, url) in POOL_PRESETS {
                    if soft_button(ui, name, 118.0).clicked() {
                        self.edit_stratum = (*url).into();
                        self.push_log(LogKind::Info, format!("Pool preset → {name}"));
                    }
                }
            });
            ui.add_space(6.0);
            labeled_edit(ui, "Stratum URL", &mut self.edit_stratum, "stratum+tcp://host:port");
            labeled_edit(
                ui,
                "Worker",
                &mut self.edit_worker,
                "Bitcoin address (bc1... / 1... / 3...)",
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new("Password").color(C_MUTED).size(12.0));
                ui.add(
                    TextEdit::singleline(&mut self.edit_password)
                        .desired_width(130.0)
                        .font(FontId::new(13.0, FontFamily::Monospace)),
                );
                ui.separator();
                ui.label(RichText::new("Clock").color(C_MUTED).size(12.0));
                for mhz in [80_u8, 160, 240] {
                    let selected = self.target_mhz == mhz;
                    if clock_chip(ui, mhz, selected).clicked() {
                        self.target_mhz = mhz;
                        let _ = self.cmd_tx.send(NetCmd::SetClock(mhz));
                        self.push_log(LogKind::Usb, format!("Clock request {mhz} MHz"));
                    }
                }
            });
        });
    }

    fn ui_telemetry_rail(&self, ui: &mut egui::Ui) {
        soft_panel(ui, "Board telemetry", |ui| {
            let authed = self.stratum_live.authorized;
            // Never surface connect-handshake rejects — stay at 0/0 until authorize.
            let (acc, rej) = if !authed {
                (0, 0)
            } else if self.session_started.is_some() {
                (self.session_accepted, self.session_rejected)
            } else {
                (self.accepted, self.rejected)
            };
            // Compact chips — large metric tiles overflow short viewports.
            ui.horizontal_wrapped(|ui| {
                mini_stat(ui, "Accept", &acc.to_string());
                mini_stat(ui, "Reject", &rej.to_string());
                mini_stat(ui, "Accept%", &self.accept_rate_label());
                mini_stat(ui, "Session", &self.session_elapsed_label());
                mini_stat(ui, "Luck", &self.luck_label());
                mini_stat(ui, "Expect/h", &self.expected_shares_label());
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                mini_stat(ui, "Rate", &format_hashrate(self.status.hashrate_hs));
                mini_stat(ui, "Hashes", &format_hash_count(self.status.hashes));
                mini_stat(ui, "SHA", self.sha_mode_label());
                mini_stat(
                    ui,
                    "Reply",
                    &self
                        .last_share_latency_ms
                        .map(|ms| format!("{ms}ms"))
                        .unwrap_or_else(|| "—".into()),
                );
                mini_stat(ui, "Sess H", &self.session_hashes_label());
            });
            if !authed {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Share counts after authorize")
                        .color(C_DIM)
                        .font(mono_ui_font(10.0)),
                );
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "fw {} · {} · job {}",
                    if self.fw_label.is_empty() {
                        "—"
                    } else {
                        &self.fw_label
                    },
                    format!(
                        "{} @ {} MHz",
                        format_uptime(self.status.uptime_secs),
                        self.target_mhz
                    ),
                    if self.status.job.is_empty() {
                        "—"
                    } else {
                        &self.status.job
                    },
                ))
                .color(C_MUTED)
                .font(mono_ui_font(10.0)),
            );
            ui.label(
                RichText::new(format!(
                    "nonce {}",
                    if self.status.nonce.is_empty() {
                        "—"
                    } else {
                        &self.status.nonce
                    }
                ))
                .color(C_DIM)
                .font(FontId::new(11.0, FontFamily::Monospace)),
            );
            if !self.share_history.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Recent shares")
                        .color(C_MUTED)
                        .font(mono_ui_font(10.0)),
                );
                Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(4, 8, 9, 160))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(Margin::same(8.0))
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .id_source("share_hist_scroll")
                            .max_height(72.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for row in self.share_history.iter().rev().take(8) {
                                    let mark = if row.accepted { "OK" } else { "RJ" };
                                    let color = if row.accepted { C_LIME } else { C_ERR };
                                    let lat = row
                                        .latency_ms
                                        .map(|ms| format!(" {ms}ms"))
                                        .unwrap_or_default();
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {} {}{}",
                                            row.time,
                                            mark,
                                            trunc(&row.detail, 28),
                                            lat
                                        ))
                                        .color(color)
                                        .font(FontId::new(10.0, FontFamily::Monospace)),
                                    );
                                }
                            });
                    });
            }
            if !self.last_ok.is_empty() || !self.last_error.is_empty() {
                ui.add_space(6.0);
            }
            if !self.last_ok.is_empty() {
                ui.label(
                    RichText::new(trunc(&self.last_ok, 72))
                        .color(C_LIME)
                        .size(11.0),
                );
            }
            if !self.last_error.is_empty() {
                ui.label(
                    RichText::new(trunc(&self.last_error, 72))
                        .color(C_ERR)
                        .size(11.0),
                );
            }
        });
    }

    fn ui_stratum_panel(&mut self, ui: &mut egui::Ui) {
        let s = &self.stratum_live;
        soft_panel(ui, "Live stratum", |ui| {
            ui.horizontal_wrapped(|ui| {
                let (state, color) = self.pool_state();
                status_chip(ui, state, color);
                ui.label(
                    RichText::new(format!(
                        "{} · phase {} · diff {:.6}",
                        if s.endpoint.is_empty() { "—" } else { &s.endpoint },
                        if s.phase.is_empty() { "off" } else { &s.phase },
                        s.difficulty
                    ))
                    .color(C_MUTED)
                    .size(13.0),
                );
            });
            ui.add_space(8.0);
            let (acc, rej) = if s.authorized {
                (s.accepted, s.rejected)
            } else {
                (0, 0)
            };
            ui.horizontal_wrapped(|ui| {
                mini_stat(ui, "TX", &s.lines_tx.to_string());
                mini_stat(ui, "RX", &s.lines_rx.to_string());
                mini_stat(ui, "Jobs", &s.jobs.to_string());
                mini_stat(ui, "Accept", &acc.to_string());
                mini_stat(ui, "Reject", &rej.to_string());
                mini_stat(ui, "Expect/h", &self.expected_shares_label());
                mini_stat(
                    ui,
                    "Job id",
                    if s.last_job.is_empty() { "—" } else { &s.last_job },
                );
            });
            ui.add_space(8.0);
            stratum_line(ui, "Last TX → pool", &trunc(&s.last_tx, 150));
            stratum_line(ui, "Last RX ← pool", &trunc(&s.last_rx, 150));
            ui.add_space(6.0);
            if soft_button(ui, "Copy stratum snapshot", 180.0).clicked() {
                let snap = format!(
                    "endpoint={} phase={} diff={} acc={} rej={} job={}\nTX {}\nRX {}\n",
                    s.endpoint,
                    s.phase,
                    s.difficulty,
                    acc,
                    rej,
                    s.last_job,
                    s.last_tx,
                    s.last_rx
                );
                ui.ctx().output_mut(|o| o.copied_text = snap);
            }
        });
    }

    fn ui_logs_panel(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Event log", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.log_auto_scroll, "Auto-scroll");
                if soft_button(ui, "Clear logs", 110.0).clicked() {
                    self.logs.clear();
                }
                if soft_button(ui, "Copy logs", 110.0).clicked() {
                    self.copy_logs_to_clipboard(ui.ctx());
                    self.last_ok = "Event log copied to clipboard.".into();
                }
            });
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(4, 8, 9, 202))
                .rounding(Rounding::same(16.0))
                .stroke(Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_unmultiplied(198, 255, 64, 18),
                ))
                .inner_margin(Margin::same(10.0))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_source("logs_scroll")
                        .stick_to_bottom(self.log_auto_scroll)
                        .max_height(170.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for e in &self.logs {
                                let (tag, color) = match e.kind {
                                    LogKind::Info => ("INF", C_MUTED),
                                    LogKind::Usb => ("USB", C_BUBBLE_HI),
                                    LogKind::Stratum => ("STR", C_LIME),
                                    LogKind::Warn => ("WRN", C_WARN),
                                    LogKind::Err => ("ERR", C_ERR),
                                };
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{} [{tag}]", e.time))
                                            .color(color)
                                            .font(mono_ui_font(11.0)),
                                    );
                                    ui.label(
                                        RichText::new(&e.text)
                                            .color(C_TEXT)
                                            .font(FontId::new(11.0, FontFamily::Monospace)),
                                    );
                                });
                            }
                        });
                });
        });
    }

    fn ui_debug(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Debug / Terminal", |ui| {
            ui.label(
                RichText::new("Send raw cmp commands over USB-C without bypassing the worker path.")
                    .color(C_MUTED)
                    .size(13.0),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let resp = ui.add(
                    TextEdit::singleline(&mut self.term_input)
                        .desired_width((ui.available_width() - 360.0).max(240.0))
                        .font(FontId::new(14.0, FontFamily::Monospace))
                        .hint_text("cmp ping"),
                );
                if cta_button(ui, "Send", true, 96.0).clicked()
                    || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    self.send_term();
                }
                if soft_button(ui, "Ping", 82.0).clicked() {
                    self.term_input = "cmp ping".into();
                    self.send_term();
                }
                if soft_button(ui, "Status", 92.0).clicked() {
                    self.term_input = "cmp status".into();
                    self.send_term();
                }
                if soft_button(ui, "Config", 92.0).clicked() {
                    self.term_input = "cmp config".into();
                    self.send_term();
                }
                if soft_button(ui, "Stop", 82.0).clicked() {
                    self.term_input = "cmp stop".into();
                    self.send_term();
                }
                if soft_button(ui, "Bench", 82.0).clicked() {
                    let _ = self.cmd_tx.send(NetCmd::Bench);
                }
                if soft_button(ui, "Reboot", 92.0).clicked() {
                    let _ = self.cmd_tx.send(NetCmd::RebootBoard);
                    self.push_log(LogKind::Usb, "Board reboot requested".into());
                }
                if soft_button(ui, "Push ticker", 110.0).clicked() {
                    self.push_board_ticker(true);
                }
            });
            ui.add_space(10.0);
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(4, 8, 9, 224))
                .rounding(Rounding::same(18.0))
                .stroke(Stroke::new(1.0_f32, C_STROKE))
                .inner_margin(Margin::same(14.0))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_source("term_scroll")
                        .stick_to_bottom(true)
                        .max_height(310.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for line in self.term_history.iter().chain(self.term_out.iter()) {
                                let color = if line.starts_with(">>>") {
                                    C_BUBBLE_HI
                                } else if line.contains("CMPERR") || line.starts_with("ERR") {
                                    C_ERR
                                } else if line.contains("CMPACK") || line.contains("CMP ok") {
                                    C_LIME
                                } else {
                                    C_TEXT
                                };
                                ui.label(
                                    RichText::new(line)
                                        .color(color)
                                        .font(FontId::new(12.0, FontFamily::Monospace)),
                                );
                            }
                        });
                });
            ui.add_space(8.0);
            if soft_button(ui, "Clear terminal", 140.0).clicked() {
                self.term_history.clear();
                self.term_out.clear();
            }
        });

        ui.add_space(14.0);
        self.ui_stratum_panel(ui);
        ui.add_space(12.0);
        self.ui_logs_panel(ui);
    }
}

impl App for CompanionApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.persist(storage);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
                NetMsg::Ports(p) => {
                    self.ports = p;
                    if self.com_port.is_empty() {
                        if let Some(first) = self.ports.first() {
                            self.com_port = first.clone();
                        }
                    } else if !self.ports.iter().any(|x| x == &self.com_port) {
                        // Keep remembered port even if not listed yet (driver lag).
                    }
                    if self.auto_connect
                        && !self.auto_connect_attempted
                        && !self.usb_open
                        && !self.com_port.is_empty()
                        && self.ports.iter().any(|x| x == &self.com_port)
                    {
                        self.auto_connect_attempted = true;
                        self.connect_usb();
                    }
                }
                NetMsg::Action(Ok(s)) => {
                    self.last_ok = s.clone();
                    let low = s.to_lowercase();
                    if low.contains("usb open") {
                        self.usb_open = true;
                    }
                    if low.contains("closed") {
                        self.usb_open = false;
                        self.mining = false;
                    }
                    let kind = if low.contains("job") || low.contains("share") || low.contains("pool")
                    {
                        LogKind::Stratum
                    } else if low.contains("usb") || low.contains("cmp") || low.contains("board") {
                        LogKind::Usb
                    } else {
                        LogKind::Info
                    };
                    self.push_log(kind, s);
                }
                NetMsg::Action(Err(e)) => {
                    self.last_error = e.clone();
                    self.push_log(LogKind::Err, e);
                }
                NetMsg::Status(Ok(s)) => {
                    self.status = s;
                    self.last_error.clear();
                }
                NetMsg::Status(Err(e)) => {
                    self.last_error = e.clone();
                    self.push_log(LogKind::Warn, format!("status: {e}"));
                }
                NetMsg::Config(Ok(c)) => {
                    self.fw_label = if c.fw.is_empty() {
                        "—".into()
                    } else {
                        c.fw.clone()
                    };
                    if c.cpu_mhz == 80 || c.cpu_mhz == 160 || c.cpu_mhz == 240 {
                        self.target_mhz = c.cpu_mhz;
                    }
                    self.push_log(
                        LogKind::Usb,
                        format!("config fw={} mode={}", c.fw, c.mode),
                    );
                }
                NetMsg::Config(Err(e)) => self.push_log(LogKind::Warn, format!("config: {e}")),
                NetMsg::MineStats {
                    accepted,
                    rejected,
                    phase,
                } => {
                    if self.stratum_live.authorized {
                        self.accepted = accepted;
                        self.rejected = rejected;
                    } else {
                        self.accepted = 0;
                        self.rejected = 0;
                    }
                    self.pool_phase = phase;
                }
                NetMsg::Stratum(live) => {
                    let was_authed = self.stratum_live.authorized;
                    self.stratum_live = live;
                    if self.stratum_live.authorized && !was_authed {
                        // Fresh authorize: HUD stays at 0 until real post-grace outcomes arrive.
                        // Do not wipe session counters here — Share msgs can race ahead of this.
                        self.accepted = 0;
                        self.rejected = 0;
                        self.push_log(
                            LogKind::Stratum,
                            "Authorized — counting shares after warmup".into(),
                        );
                    }
                    if self.stratum_live.authorized {
                        self.accepted = self.stratum_live.accepted;
                        self.rejected = self.stratum_live.rejected;
                    } else {
                        self.accepted = 0;
                        self.rejected = 0;
                    }
                    self.pool_phase = self.stratum_live.phase.clone();
                }
                NetMsg::Log { kind, text } => self.push_log(kind, text),
                NetMsg::Terminal(line) => {
                    self.term_out.push_back(line.clone());
                    while self.term_out.len() > 300 {
                        self.term_out.pop_front();
                    }
                    self.push_log(LogKind::Usb, format!("RX {line}"));
                }
                NetMsg::FlashDone { result, reopen } => {
                    self.update_busy = false;
                    match result {
                        Ok(s) => {
                            self.update_status = s.clone();
                            self.last_ok = s.clone();
                            self.last_error.clear();
                            self.push_log(LogKind::Usb, s);
                            if let Some(port) = reopen {
                                self.com_port = port.clone();
                                self.push_log(
                                    LogKind::Usb,
                                    format!("Reconnecting USB on {port}…"),
                                );
                                let _ = self.cmd_tx.send(NetCmd::OpenUsb(port));
                            }
                        }
                        Err(e) => {
                            self.update_status = e.clone();
                            self.last_error = e.clone();
                            self.push_log(LogKind::Err, e);
                        }
                    }
                }
                NetMsg::Share(ev) => {
                    // Stratum only emits post-authorize, post-warmup outcomes.
                    if ev.accepted {
                        self.session_accepted = self.session_accepted.saturating_add(1);
                    } else {
                        self.session_rejected = self.session_rejected.saturating_add(1);
                    }
                    if let Some(ms) = ev.latency_ms {
                        self.last_share_latency_ms = Some(ms);
                    }
                    let detail = if ev.accepted {
                        match ev.latency_ms {
                            Some(ms) => format!(
                                "{} · #{id} · {ms} ms",
                                if ev.nonce.is_empty() { "ok" } else { &ev.nonce },
                                id = ev.id
                            ),
                            None => format!(
                                "{} · #{id}",
                                if ev.nonce.is_empty() { "ok" } else { &ev.nonce },
                                id = ev.id
                            ),
                        }
                    } else {
                        format!(
                            "{} · {}",
                            if ev.nonce.is_empty() {
                                format!("#{id}", id = ev.id)
                            } else {
                                ev.nonce.clone()
                            },
                            trunc(&ev.detail, 36)
                        )
                    };
                    self.share_history.push_back(ShareRow {
                        time: stamp(),
                        accepted: ev.accepted,
                        detail,
                        latency_ms: ev.latency_ms,
                    });
                    while self.share_history.len() > 40 {
                        self.share_history.pop_front();
                    }
                }
                NetMsg::FirmwareFetched(result) => {
                    self.fetch_busy = false;
                    match result {
                        Ok(fw) => {
                            self.update_status = format!(
                                "Fetched {} ({} KB)",
                                fw.version,
                                fw.bytes / 1024
                            );
                            self.last_ok = self.update_status.clone();
                            self.push_log(LogKind::Info, self.update_status.clone());
                            self.firmware = Some(fw);
                        }
                        Err(e) => {
                            self.update_status = e.clone();
                            self.last_error = e.clone();
                            self.push_log(LogKind::Err, e);
                        }
                    }
                }
                NetMsg::ApiFeedResult(outcome) => {
                    self.apply_api_pull(outcome);
                }
            }
        }

        if self.usb_open && self.last_poll.elapsed() > Duration::from_millis(800) {
            let _ = self.cmd_tx.send(NetCmd::PollStatus);
            self.last_poll = Instant::now();
        }

        // Periodically refresh enabled API feeds (every 5 minutes, round-robin).
        if self.api_pulling_id.is_none()
            && self.last_api_auto_pull.elapsed() > Duration::from_secs(300)
        {
            let enabled: Vec<u64> = self
                .api_feeds
                .iter()
                .filter(|f| f.enabled)
                .map(|f| f.id)
                .collect();
            if let Some(&id) = enabled.first() {
                // Prefer the feed with the oldest pull time.
                let id = enabled
                    .iter()
                    .min_by_key(|id| {
                        self.api_feeds
                            .iter()
                            .find(|f| f.id == **id)
                            .map(|f| f.last_pulled_ms)
                            .unwrap_or(0)
                    })
                    .copied()
                    .unwrap_or(id);
                self.last_api_auto_pull = Instant::now();
                self.request_api_pull(id);
            } else {
                self.last_api_auto_pull = Instant::now();
            }
        }

        self.update_motion(ctx);
        self.live.poll();
        if self.usb_open && !self.update_busy {
            self.push_board_ticker(false);
        }
        if self.update_busy || self.fetch_busy {
            ctx.request_repaint();
        }

        if let Some(step) = self.wizard_step {
            egui::Window::new("Welcome · CYD setup")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
                .show(ctx, |ui| {
                    ui.set_min_width(460.0);
                    let title = match step {
                        0 => "1 · USB driver",
                        1 => "2 · Select COM & connect",
                        2 => "3 · Firmware on the board",
                        _ => "4 · Pool worker",
                    };
                    ui.label(RichText::new(title).color(C_LIME).font(display_font(28.0)));
                    ui.add_space(8.0);
                    match step {
                        0 => {
                            ui.label(
                                RichText::new(
                                    "Most CYD boards use a CH340 USB-serial chip. If Windows shows an unknown device, install a CH340 driver, then plug the board with USB-C.",
                                )
                                .color(C_TEXT)
                                .size(14.0),
                            );
                        }
                        1 => {
                            ui.label(
                                RichText::new("Pick the COM port for your board, then Connect.")
                                    .color(C_TEXT)
                                    .size(14.0),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_source("wiz_com")
                                    .width(200.0)
                                    .selected_text(if self.com_port.is_empty() {
                                        "Select port"
                                    } else {
                                        &self.com_port
                                    })
                                    .show_ui(ui, |ui| {
                                        for p in &self.ports {
                                            ui.selectable_value(&mut self.com_port, p.clone(), p);
                                        }
                                    });
                                if soft_button(ui, "Refresh", 90.0).clicked() {
                                    let _ = self.cmd_tx.send(NetCmd::ListPorts);
                                }
                                if soft_button(
                                    ui,
                                    if self.usb_open { "Connected" } else { "Connect" },
                                    110.0,
                                )
                                .clicked()
                                    && !self.usb_open
                                {
                                    self.connect_usb();
                                }
                            });
                            ui.label(
                                RichText::new(if self.usb_open {
                                    "USB linked — continue."
                                } else {
                                    "Waiting for USB…"
                                })
                                .color(if self.usb_open { C_LIME } else { C_MUTED })
                                .size(13.0),
                            );
                        }
                        2 => {
                            let (st, col) = self.firmware_status_label();
                            ui.label(RichText::new(st).color(col).size(14.0));
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(
                                    "Flash the bundled image once (or when Update available). Hold BOOT + tap RESET if download mode fails.",
                                )
                                .color(C_MUTED)
                                .size(13.0),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if soft_button(ui, "Update board", 130.0).clicked() {
                                    self.request_board_update();
                                }
                                if soft_button(ui, "Fetch latest FW", 140.0).clicked() {
                                    self.start_firmware_fetch();
                                }
                            });
                        }
                        _ => {
                            ui.label(
                                RichText::new(
                                    "Paste your Bitcoin address as the worker name, then Start mining from the Mine tab.",
                                )
                                .color(C_TEXT)
                                .size(14.0),
                            );
                            ui.add_space(8.0);
                            labeled_edit(
                                ui,
                                "Worker / BTC address",
                                &mut self.edit_worker,
                                "bc1… / 1… / 3…",
                            );
                            labeled_edit(
                                ui,
                                "Stratum",
                                &mut self.edit_stratum,
                                "stratum+tcp://…",
                            );
                        }
                    }
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if step > 0 && soft_button(ui, "Back", 90.0).clicked() {
                            self.wizard_step = Some(step - 1);
                        }
                        let next_label = if step >= 3 { "Finish" } else { "Next" };
                        if cta_button(ui, next_label, true, 120.0).clicked() {
                            if step >= 3 {
                                self.wizard_step = None;
                            } else {
                                self.wizard_step = Some(step + 1);
                            }
                        }
                        if soft_button(ui, "Skip setup", 110.0).clicked() {
                            self.wizard_step = None;
                        }
                    });
                });
        }

        if self.update_confirm {
            egui::Window::new("Update board firmware")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(440.0);
                    let (st, col) = self.firmware_status_label();
                    ui.label(RichText::new(st).color(col).size(14.0));
                    ui.add_space(8.0);
                    if let Some(fw) = &self.firmware {
                        let ver = if fw.version.is_empty() {
                            "unknown".into()
                        } else {
                            fw.version.clone()
                        };
                        ui.label(
                            RichText::new(format!(
                                "Image · {ver} · {} KB",
                                fw.bytes / 1024
                            ))
                            .color(C_LIME)
                            .font(mono_ui_font(12.0)),
                        );
                        ui.label(
                            RichText::new(fw.path.display().to_string())
                                .color(C_DIM)
                                .font(mono_ui_font(10.0)),
                        );
                    }
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "Board fw · {}  ·  Port · {}  ·  @ 0x0",
                            if self.fw_label.is_empty() {
                                "—"
                            } else {
                                &self.fw_label
                            },
                            self.com_port
                        ))
                        .color(C_MUTED)
                        .font(mono_ui_font(11.0)),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Mining stops and USB disconnects for the flash. Hold BOOT, tap RESET, release BOOT if it fails.",
                        )
                        .color(C_MUTED)
                        .size(13.0),
                    );
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        let up_to_date = update_needed(
                            &self.fw_label,
                            &self
                                .firmware
                                .as_ref()
                                .map(|f| f.version.clone())
                                .unwrap_or_default(),
                        ) == Some(false);
                        let flash_label = if up_to_date {
                            "Flash anyway"
                        } else {
                            "Flash now"
                        };
                        if cta_button(ui, flash_label, true, 140.0).clicked() {
                            self.begin_board_update();
                        }
                        if soft_button(ui, "Cancel", 100.0).clicked() {
                            self.update_confirm = false;
                        }
                    });
                });
        }

        egui::TopBottomPanel::top("live_ticker_bar")
            .exact_height(36.0)
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(8, 14, 16))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(32, 48, 46)))
                    .inner_margin(Margin::symmetric(16.0, 0.0)),
            )
            .show(ctx, |ui| {
                ui_live_bar(ui, &self.live);
            });

        egui::CentralPanel::default()
            .frame(Frame::none().fill(C_BG).inner_margin(Margin::same(22.0)))
            .show(ctx, |ui| {
                paint_background(ui, ui.max_rect(), self.pulse, self.grid_phase, self.mining || self.board_hashing());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("CYD").color(C_LIME).font(display_font(64.0)));
                        ui.label(
                            RichText::new("Companion · SHA-256")
                                .color(C_MUTED)
                                .font(mono_ui_font(12.0)),
                        );
                    });
                    ui.add_space(24.0);
                    if nav_button(ui, "Mine", self.tab == Tab::Mine).clicked() {
                        self.tab = Tab::Mine;
                    }
                    if nav_button(ui, "Settings", self.tab == Tab::Settings).clicked() {
                        self.tab = Tab::Settings;
                    }
                    if nav_button(ui, "Debug / Terminal", self.tab == Tab::Debug).clicked() {
                        self.tab = Tab::Debug;
                    }
                    ui.label(
                        RichText::new(format!("fw {}", self.fw_label))
                            .color(C_DIM)
                            .font(mono_ui_font(11.0)),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (pool, color) = self.pool_state();
                        status_chip(ui, pool, color);
                        status_chip(
                            ui,
                            if self.usb_open { "USB LINKED" } else { "USB IDLE" },
                            if self.usb_open { C_LIME } else { C_MUTED },
                        );
                    });
                });
                ui.add_space(14.0);

                // Outer scroll so Mine/Debug content is fully reachable on short screens.
                ScrollArea::vertical()
                    .id_source("main_app_scroll")
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        match self.tab {
                            Tab::Mine => self.ui_mine(ui),
                            Tab::Settings => self.ui_settings(ui),
                            Tab::Debug => self.ui_debug(ui),
                        }
                        ui.add_space(28.0);
                    });
            });

        // Keep animation continuous (~60 fps). 40 ms made looping motion feel stepped.
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.is_empty() {
        "—".into()
    } else if s.len() > n {
        format!("{}…", &s[..n])
    } else {
        s.to_string()
    }
}

fn ui_live_bar(ui: &mut egui::Ui, live: &LiveFeed) {
    let snap = live.snap();
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), ui.available_height()),
        Layout::left_to_right(Align::Center),
        |ui| {
            // Local place / weather / clock (IP-derived)
            ui.label(
                RichText::new(live.place_label())
                    .color(C_TEXT)
                    .font(mono_ui_font(12.0)),
            );
            ui.add_space(10.0);
            if snap.ready && !snap.weather.is_empty() {
                ui.label(
                    RichText::new(format!("{:.0}°C {}", snap.temp_c, snap.weather))
                        .color(C_LIME_SOFT)
                        .font(mono_ui_font(12.0)),
                );
                ui.add_space(10.0);
            }
            ui.label(
                RichText::new(live.local_now_label())
                    .color(C_MUTED)
                    .font(mono_ui_font(12.0)),
            );

            ui.add_space(16.0);
            ui.label(RichText::new("│").color(C_DIM).size(14.0));
            ui.add_space(16.0);

            // Crypto strip
            if snap.quotes.is_empty() {
                ui.label(
                    RichText::new(if snap.error.is_empty() {
                        "prices…"
                    } else {
                        "prices offline"
                    })
                    .color(C_DIM)
                    .font(mono_ui_font(11.0)),
                );
            } else {
                for (i, q) in snap.quotes.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(12.0);
                    }
                    let ch_color = if q.change_24h >= 0.0 {
                        C_LIME
                    } else {
                        C_ERR
                    };
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(
                            RichText::new(&q.symbol)
                                .color(C_MUTED)
                                .font(mono_ui_font(11.0)),
                        );
                        ui.label(
                            RichText::new(format_usd(q.usd))
                                .color(C_TEXT)
                                .font(mono_ui_font(12.0)),
                        );
                        ui.label(
                            RichText::new(format_change(q.change_24h))
                                .color(ch_color)
                                .font(mono_ui_font(11.0)),
                        );
                    });
                }
            }
        },
    );
}

fn paint_background(ui: &mut egui::Ui, rect: Rect, pulse: f32, grid_phase: f32, mining: bool) {
    let painter = ui.painter();
    let bands = 84;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        let y0 = rect.top() + rect.height() * t;
        let y1 = rect.top() + rect.height() * ((i + 1) as f32 / bands as f32) + 1.0;
        let c = lerp_color(C_BG, C_BG_2, t);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(rect.left(), y0), Pos2::new(rect.right(), y1)),
            0.0,
            c,
        );
    }

    let glow = if mining {
        (42.0 + pulse.sin().max(0.0) * 58.0) as u8
    } else {
        28
    };
    painter.circle_filled(
        Pos2::new(rect.left() + rect.width() * 0.22, rect.top() + rect.height() * 0.18),
        360.0,
        rgba(C_LIME, glow / 3),
    );
    painter.circle_filled(
        Pos2::new(rect.right() - rect.width() * 0.12, rect.bottom() - rect.height() * 0.12),
        280.0,
        rgba(C_LIME_SOFT, 18),
    );

    // Drift wraps by exactly one line spacing — no hitch when phase resets.
    let step = 34.0;
    let drift = grid_phase * step;
    let mut x = rect.left() - rect.height() + drift;
    while x < rect.right() + rect.height() {
        painter.line_segment(
            [
                Pos2::new(x, rect.bottom()),
                Pos2::new(x + rect.height() * 0.66, rect.top()),
            ],
            Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(198, 255, 64, 10),
            ),
        );
        x += step;
    }
}

fn paint_hero_wash(ui: &mut egui::Ui, rect: Rect, pulse: f32, mining: bool) {
    let painter = ui.painter();
    let alpha = if mining {
        (26.0 + (0.5 + 0.5 * pulse.sin()) * 48.0) as u8
    } else {
        16
    };
    painter.circle_filled(
        Pos2::new(rect.left() + 110.0, rect.top() + 78.0),
        190.0,
        rgba(C_LIME, alpha),
    );
    painter.circle_filled(
        Pos2::new(rect.right() - 80.0, rect.bottom() - 40.0),
        120.0,
        rgba(C_LIME_SOFT, if mining { 22 } else { 12 }),
    );

    // Slow diagonal sweep — fade at loop edges so the reset is invisible.
    let sweep = (pulse * 0.12).rem_euclid(1.0);
    let edge = loop_edge_fade(sweep, 0.14);
    let x = rect.left() + rect.width() * sweep;
    let sweep_a = ((if mining { 28.0 } else { 12.0 }) * edge) as u8;
    if sweep_a > 0 {
        painter.line_segment(
            [
                Pos2::new(x, rect.bottom() - 18.0),
                Pos2::new(x + rect.height() * 0.55, rect.top() + 18.0),
            ],
            Stroke::new(
                2.0_f32,
                Color32::from_rgba_unmultiplied(198, 255, 64, sweep_a),
            ),
        );
    }
}

fn hash_activity_bars(ui: &mut egui::Ui, khs: f32, pulse: f32, hashing: bool) {
    let desired = Vec2::new((ui.available_width() * 0.72).clamp(320.0, 620.0), 28.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter_at(rect);
    let n = 28;
    let gap = 3.0;
    let bar_w = ((rect.width() - gap * (n as f32 - 1.0)) / n as f32).max(4.0);
    for i in 0..n {
        let phase = pulse * 2.4 + i as f32 * 0.37;
        let breathe = 0.35 + 0.65 * (0.5 + 0.5 * phase.sin());
        let level = if hashing {
            ((khs / 60.0).clamp(0.08, 1.0) * breathe).clamp(0.12, 1.0)
        } else {
            0.08 + 0.04 * (0.5 + 0.5 * (pulse + i as f32 * 0.2).sin())
        };
        let h = rect.height() * level;
        let x = rect.left() + i as f32 * (bar_w + gap);
        let y = rect.bottom() - h;
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x, y), Pos2::new(x + bar_w, rect.bottom())),
            Rounding::same(2.0),
            if hashing {
                rgba(C_LIME, (70.0 + level * 140.0) as u8)
            } else {
                rgba(C_BUBBLE_HI, 50)
            },
        );
    }
}

fn sparkline(ui: &mut egui::Ui, values: &VecDeque<f32>, pulse: f32, history_phase: f32) {
    let desired = Vec2::new((ui.available_width() * 0.72).clamp(320.0, 620.0), 86.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::same(18.0), Color32::from_rgba_unmultiplied(5, 10, 11, 170));
    painter.rect_stroke(
        rect,
        Rounding::same(18.0),
        Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(198, 255, 64, 24),
        ),
    );

    for i in 1..4 {
        let y = rect.top() + rect.height() * i as f32 / 4.0;
        painter.line_segment(
            [Pos2::new(rect.left() + 14.0, y), Pos2::new(rect.right() - 14.0, y)],
            Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(235, 244, 238, 12),
            ),
        );
    }

    let max = values
        .iter()
        .copied()
        .fold(1.0_f32, |acc, v| acc.max(v.max(0.0)));
    let inner = rect.shrink2(Vec2::new(16.0, 14.0));
    let len = values.len().max(2);
    // Scroll smoothly between samples so the chart doesn't jump when a point drops off.
    let dx = inner.width() / (len - 1) as f32;
    let scroll = if values.len() >= HASH_HISTORY_SAMPLES {
        history_phase.clamp(0.0, 1.0) * dx
    } else {
        0.0
    };
    let points: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = inner.left() + dx * i as f32 - scroll;
            let y = inner.bottom() - inner.height() * (v.max(0.0) / max).clamp(0.0, 1.0);
            Pos2::new(x, y)
        })
        .filter(|p| p.x >= inner.left() - 2.0 && p.x <= inner.right() + 2.0)
        .collect();
    for pair in points.windows(2) {
        painter.line_segment([pair[0], pair[1]], Stroke::new(6.0_f32, rgba(C_LIME, 22)));
        painter.line_segment([pair[0], pair[1]], Stroke::new(2.25_f32, C_LIME));
    }

    let scan_t = (pulse * 0.16).rem_euclid(1.0);
    let scan_fade = loop_edge_fade(scan_t, 0.12);
    let scan_x = inner.left() + inner.width() * scan_t;
    let scan_a = (80.0 * scan_fade) as u8;
    if scan_a > 0 {
        painter.line_segment(
            [Pos2::new(scan_x, inner.top()), Pos2::new(scan_x, inner.bottom())],
            Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(198, 255, 64, scan_a),
            ),
        );
    }
    if let Some(last) = points.last() {
        painter.circle_filled(*last, 4.5, C_LIME);
        painter.circle_filled(*last, 10.0 + pulse.sin().max(0.0) * 4.0, rgba(C_LIME, 28));
    }
}

/// Fade a 0..1 looping parameter near the wrap so teleports aren't visible.
fn loop_edge_fade(t: f32, edge: f32) -> f32 {
    let edge = edge.clamp(0.02, 0.45);
    if t < edge {
        (t / edge).clamp(0.0, 1.0)
    } else if t > 1.0 - edge {
        ((1.0 - t) / edge).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn soft_panel(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    Frame::none()
        .fill(C_PANEL_SOFT)
        .rounding(Rounding::same(24.0))
        .stroke(Stroke::new(1.0_f32, C_STROKE))
        .inner_margin(Margin::same(18.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title.to_uppercase())
                    .color(C_BUBBLE_HI)
                    .font(mono_ui_font(12.0)),
            );
            ui.add_space(10.0);
            add(ui);
        });
}

fn nav_button(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let fill = if selected {
        Color32::from_rgba_unmultiplied(198, 255, 64, 48)
    } else {
        Color32::from_rgba_unmultiplied(22, 34, 36, 128)
    };
    let stroke = if selected {
        Stroke::new(1.0_f32, rgba(C_LIME, 112))
    } else {
        Stroke::new(1.0_f32, C_STROKE)
    };
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .color(if selected { C_LIME } else { C_MUTED })
                .font(mono_ui_font(12.0)),
        )
        .fill(fill)
        .stroke(stroke)
        .rounding(Rounding::same(999.0))
        .min_size(Vec2::new(86.0, 34.0)),
    )
}

fn cta_button(ui: &mut egui::Ui, label: &str, lime: bool, width: f32) -> egui::Response {
    let fill = if lime { C_LIME } else { Color32::from_rgb(42, 61, 57) };
    let text = if lime {
        Color32::from_rgb(7, 14, 8)
    } else {
        C_TEXT
    };
    ui.add(
        egui::Button::new(RichText::new(label).color(text).font(display_font(17.0)))
            .fill(fill)
            .stroke(Stroke::new(
                1.0_f32,
                rgba(C_LIME, if lime { 100 } else { 38 }),
            ))
            .rounding(Rounding::same(20.0))
            .min_size(Vec2::new(width, 46.0)),
    )
}

fn soft_button(ui: &mut egui::Ui, label: &str, width: f32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(C_TEXT).size(13.0))
            .fill(C_BUBBLE)
            .stroke(Stroke::new(1.0_f32, C_STROKE))
            .rounding(Rounding::same(16.0))
            .min_size(Vec2::new(width, 36.0)),
    )
}

fn status_chip(ui: &mut egui::Ui, label: &str, color: Color32) {
    Frame::none()
        .fill(Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28))
        .rounding(Rounding::same(999.0))
        .stroke(Stroke::new(1.0_f32, rgba(color, 92)))
        .inner_margin(Margin::symmetric(10.0, 5.0))
        .show(ui, |ui| {
            ui.label(RichText::new(label).color(color).font(mono_ui_font(11.0)));
        });
}

fn labeled_edit(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(RichText::new(label).color(C_MUTED).size(12.0));
    ui.add(
        TextEdit::singleline(value)
            .desired_width(ui.available_width())
            .hint_text(hint)
            .font(FontId::new(13.0, FontFamily::Proportional)),
    );
}

fn clock_chip(ui: &mut egui::Ui, mhz: u8, selected: bool) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(format!("{mhz}"))
                .color(if selected { Color32::from_rgb(8, 16, 10) } else { C_TEXT })
                .font(mono_ui_font(12.0)),
        )
        .fill(if selected { C_LIME } else { C_BUBBLE })
        .stroke(Stroke::new(
            1.0_f32,
            if selected { rgba(C_LIME, 120) } else { C_STROKE },
        ))
        .rounding(Rounding::same(999.0))
        .min_size(Vec2::new(48.0, 30.0)),
    )
}

fn metric(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).color(C_MUTED).font(mono_ui_font(11.0)));
        ui.label(RichText::new(value).color(color).font(display_font(24.0)));
    });
}

fn telemetry_line(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(24.0);
        ui.label(RichText::new(label).color(C_DIM).font(mono_ui_font(11.0)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(trunc(value, 48))
                    .color(C_TEXT)
                    .font(FontId::new(12.0, FontFamily::Monospace)),
            );
        });
    });
}

fn stratum_line(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(RichText::new(label).color(C_BUBBLE_HI).font(mono_ui_font(11.0)));
    ui.label(
        RichText::new(value)
            .color(C_TEXT)
            .font(FontId::new(12.0, FontFamily::Monospace)),
    );
}

fn mini_stat(ui: &mut egui::Ui, label: &str, value: &str) {
    Frame::none()
        .fill(C_PANEL_QUIET)
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(8.0, 5.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new(label).color(C_DIM).font(mono_ui_font(9.0)));
                ui.label(
                    RichText::new(value)
                        .color(C_TEXT)
                        .font(FontId::new(12.0, FontFamily::Monospace)),
                );
            });
        });
}

fn rgba(base: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

fn log_msg(tx: &Sender<NetMsg>, kind: LogKind, text: impl Into<String>) {
    let _ = tx.send(NetMsg::Log {
        kind,
        text: text.into(),
    });
}

fn push_stratum_live(tx: &Sender<NetMsg>, client: &StratumClient) {
    let _ = tx.send(NetMsg::Stratum(StratumLive {
        endpoint: client.endpoint.clone(),
        phase: client.phase.clone(),
        connected: client.stream_connected(),
        authorized: client.authorized(),
        difficulty: client.difficulty(),
        jobs: client.jobs_seen,
        accepted: client.accepted,
        rejected: client.rejected,
        lines_rx: client.lines_rx,
        lines_tx: client.lines_tx,
        last_job: client.job_id().to_string(),
        last_rx: client.last_rx.clone(),
        last_tx: client.last_tx.clone(),
    }));
}

fn mine_worker(cmd_rx: Receiver<NetCmd>, msg_tx: Sender<NetMsg>) {
    let mut usb: Option<Box<dyn SerialPort>> = None;
    let mut usb_rx = String::new();
    let mut stratum: Option<StratumClient> = None;
    let mut mining = false;
    let mut legacy_job = false; // old firmware without jh/jt/ja
    let mut recent_jobs: VecDeque<WorkJob> = VecDeque::new();
    let mut last_stats_push = Instant::now() - Duration::from_secs(10);
    let mut last_stratum_ui = Instant::now() - Duration::from_secs(10);
    let mut mine_endpoint = String::new();
    let mut mine_worker_name = String::new();
    let mut mine_password = String::new();
    let mut reconnect_at: Option<Instant> = None;
    let mut reconnect_backoff = Duration::from_secs(2);

    loop {
        let cmd = if mining || usb.is_some() {
            cmd_rx.try_recv().ok()
        } else {
            match cmd_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(c) => Some(c),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };

        if let Some(cmd) = cmd {
            match cmd {
                NetCmd::ListPorts => {
                    let ports = serialport::available_ports()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|p| p.port_name)
                        .collect();
                    let _ = msg_tx.send(NetMsg::Ports(ports));
                }
                NetCmd::OpenUsb(name) => {
                    usb = None;
                    usb_rx.clear();
                    mining = false;
                    legacy_job = false;
                    if let Some(mut s) = stratum.take() {
                        s.disconnect();
                    }
                    log_msg(&msg_tx, LogKind::Usb, format!("Opening {name} @ 115200"));
                    match serialport::new(&name, 115_200)
                        .timeout(Duration::from_millis(40))
                        .open()
                    {
                        Ok(mut port) => {
                            let _ = port.clear(serialport::ClearBuffer::All);
                            let _ = port.write_all(b"\r\ncmp ping\r\n");
                            let _ = port.flush();
                            let deadline = Instant::now() + Duration::from_millis(3500);
                            let mut saw = false;
                            while Instant::now() < deadline {
                                drain_serial(port.as_mut(), &mut usb_rx);
                                if usb_rx.lines().any(|l| l.trim().starts_with("CMP ok")) {
                                    saw = true;
                                    break;
                                }
                                thread::sleep(Duration::from_millis(30));
                            }
                            usb = Some(port);
                            let _ = msg_tx.send(NetMsg::Action(Ok(if saw {
                                format!("USB open {name} @ 115200 (pong)")
                            } else {
                                format!("USB open {name} @ 115200 (no pong yet)")
                            })));
                            if let Some(p) = usb.as_mut() {
                                if let Ok(line) = usb_cmd(p.as_mut(), &mut usb_rx, "cmp config") {
                                    if let Ok(cfg) = parse_cmp_config(&line) {
                                        // Prefer split jobs on 0.6.2+; older boards stay legacy.
                                        legacy_job = !fw_supports_split_jobs(&cfg.fw);
                                        if legacy_job {
                                            log_msg(
                                                &msg_tx,
                                                LogKind::Warn,
                                                format!(
                                                    "Board fw {} lacks jh/jt/ja — using legacy job (flash 0.6.2+)",
                                                    cfg.fw
                                                ),
                                            );
                                        }
                                        let _ = msg_tx.send(NetMsg::Config(Ok(cfg)));
                                    } else {
                                        let _ = msg_tx.send(NetMsg::Config(parse_cmp_config(&line)));
                                    }
                                }
                                if let Ok(line) = usb_cmd(p.as_mut(), &mut usb_rx, "cmp status") {
                                    let _ = msg_tx.send(NetMsg::Status(parse_cmp_status(&line)));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = msg_tx
                                .send(NetMsg::Action(Err(format!("USB open failed: {e}"))));
                        }
                    }
                }
                NetCmd::CloseUsb => {
                    mining = false;
                    reconnect_at = None;
                    mine_endpoint.clear();
                    if let Some(mut s) = stratum.take() {
                        s.disconnect();
                    }
                    if let Some(p) = usb.as_mut() {
                        let _ = usb_cmd(p.as_mut(), &mut usb_rx, "cmp stop");
                    }
                    usb = None;
                    usb_rx.clear();
                    let _ = msg_tx.send(NetMsg::Action(Ok("USB closed".into())));
                }
                NetCmd::StartMine {
                    stratum: endpoint,
                    worker,
                    password,
                } => {
                    if usb.is_none() {
                        let _ = msg_tx.send(NetMsg::Action(Err("USB not open".into())));
                        continue;
                    }
                    mine_endpoint = endpoint.clone();
                    mine_worker_name = worker.clone();
                    mine_password = password.clone();
                    reconnect_backoff = Duration::from_secs(2);
                    reconnect_at = None;
                    if let Some(p) = usb.as_mut() {
                        // Clear any previous-session Accept/Reject digits on the LCD.
                        let _ = usb_cmd(
                            p.as_mut(),
                            &mut usb_rx,
                            "cmp stats accepted=0&rejected=0",
                        );
                        match usb_push_job(p.as_mut(), &mut usb_rx, &warmup_job(), &mut legacy_job, &msg_tx)
                        {
                            Ok(_) => {
                                let _ = msg_tx.send(NetMsg::Action(Ok(
                                    "Board hashing (warmup job)…".into(),
                                )));
                            }
                            Err(e) => {
                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                    "Warmup job failed: {e}"
                                ))));
                            }
                        }
                    }
                    log_msg(
                        &msg_tx,
                        LogKind::Stratum,
                        format!("Connecting pool {endpoint}"),
                    );
                    let mut client = StratumClient::new(worker, password);
                    match client.connect(&endpoint) {
                        Ok(()) => {
                            for line in client.take_recent() {
                                log_msg(&msg_tx, LogKind::Stratum, line);
                            }
                            push_stratum_live(&msg_tx, &client);
                            stratum = Some(client);
                            mining = true;
                            reconnect_backoff = Duration::from_secs(2);
                            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                "Pool connecting {endpoint} — board already hashing"
                            ))));
                        }
                        Err(e) => {
                            mining = true;
                            stratum = None;
                            reconnect_at = Some(Instant::now() + reconnect_backoff);
                            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                "Pool error (board still hashing; will retry): {e}"
                            ))));
                        }
                    }
                }
                NetCmd::StopMine => {
                    mining = false;
                    reconnect_at = None;
                    mine_endpoint.clear();
                    if let Some(mut s) = stratum.take() {
                        s.disconnect();
                        for line in s.take_recent() {
                            log_msg(&msg_tx, LogKind::Stratum, line);
                        }
                    }
                    if let Some(p) = usb.as_mut() {
                        let _ = usb_cmd(p.as_mut(), &mut usb_rx, "cmp stop");
                    }
                    let _ = msg_tx.send(NetMsg::Action(Ok("Mining stopped".into())));
                    let _ = msg_tx.send(NetMsg::MineStats {
                        accepted: 0,
                        rejected: 0,
                        phase: "off".into(),
                    });
                    let _ = msg_tx.send(NetMsg::Stratum(StratumLive {
                        phase: "off".into(),
                        ..Default::default()
                    }));
                }
                NetCmd::SetClock(mhz) => {
                    if let Some(p) = usb.as_mut() {
                        let cmd = format!("cmp clock cpu_mhz={mhz}");
                        let r = usb_cmd(p.as_mut(), &mut usb_rx, &cmd);
                        let _ = msg_tx
                            .send(NetMsg::Action(r.map(|_| format!("Clock {mhz} MHz queued"))));
                    }
                }
                NetCmd::PollStatus => {
                    if let Some(p) = usb.as_mut() {
                        match usb_cmd(p.as_mut(), &mut usb_rx, "cmp status") {
                            Ok(line) => {
                                harvest_shares(
                                    p.as_mut(),
                                    &mut usb_rx,
                                    stratum.as_mut(),
                                    &recent_jobs,
                                    &msg_tx,
                                );
                                let _ = msg_tx.send(NetMsg::Status(parse_cmp_status(&line)));
                            }
                            Err(e) => {
                                // Soft-fail: never stall the UI/stratum on a missed status.
                                // Mining may briefly delay USB; next poll usually recovers.
                                log_msg(&msg_tx, LogKind::Warn, format!("status soft-fail: {e}"));
                            }
                        }
                    }
                }
                NetCmd::Bench => {
                    if let Some(p) = usb.as_mut() {
                        match usb_cmd(p.as_mut(), &mut usb_rx, "cmp bench n=8") {
                            Ok(line) => {
                                let _ = msg_tx.send(NetMsg::Action(Ok(line)));
                            }
                            Err(e) => {
                                let _ = msg_tx.send(NetMsg::Action(Err(e)));
                            }
                        }
                    } else {
                        let _ = msg_tx.send(NetMsg::Action(Err("USB not open".into())));
                    }
                }
                NetCmd::UsbRaw(cmd) => {
                    if let Some(p) = usb.as_mut() {
                        match usb_cmd(p.as_mut(), &mut usb_rx, &cmd) {
                            Ok(line) => {
                                let _ = msg_tx.send(NetMsg::Terminal(line));
                            }
                            Err(e) => {
                                let _ = msg_tx.send(NetMsg::Terminal(format!("ERR {e}")));
                            }
                        }
                    } else {
                        let _ = msg_tx.send(NetMsg::Terminal("ERR USB not open".into()));
                    }
                }
                NetCmd::UpdateFirmware {
                    port,
                    image,
                    reopen,
                } => {
                    mining = false;
                    legacy_job = false;
                    if let Some(mut s) = stratum.take() {
                        s.disconnect();
                    }
                    if let Some(mut p) = usb.take() {
                        let _ = usb_cmd(p.as_mut(), &mut usb_rx, "cmp stop");
                        drop(p);
                    }
                    usb_rx.clear();
                    // Give Windows a moment to release the COM handle.
                    thread::sleep(Duration::from_millis(400));
                    let img = std::path::PathBuf::from(&image);
                    let tx = msg_tx.clone();
                    let progress = move |line: String| {
                        log_msg(&tx, LogKind::Usb, line);
                    };
                    let result = flash_merged_bin(&port, &img, &progress).map(|_| {
                        format!(
                            "Board update OK · flashed {} @ 0x0 on {port}",
                            img.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("firmware.bin")
                        )
                    });
                    let reopen_port = if result.is_ok() && reopen {
                        // Board reboots after flash — wait briefly before reclaiming USB.
                        thread::sleep(Duration::from_millis(1200));
                        Some(port)
                    } else {
                        None
                    };
                    let _ = msg_tx.send(NetMsg::FlashDone {
                        result,
                        reopen: reopen_port,
                    });
                }
                NetCmd::PushNet { text } => {
                    if let Some(p) = usb.as_mut() {
                        let cmd = format!("cmp netdata text={}", urlenc(&text));
                        match usb_cmd(p.as_mut(), &mut usb_rx, &cmd) {
                            Ok(_) => {
                                log_msg(
                                    &msg_tx,
                                    LogKind::Usb,
                                    format!("LCD ticker ← {}", trunc(&text, 64)),
                                );
                            }
                            Err(e) => {
                                log_msg(&msg_tx, LogKind::Warn, format!("netdata: {e}"));
                            }
                        }
                    }
                }
                NetCmd::RebootBoard => {
                    if let Some(p) = usb.as_mut() {
                        match usb_cmd(p.as_mut(), &mut usb_rx, "cmp reboot") {
                            Ok(line) => {
                                let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                    "Board reboot queued ({line})"
                                ))));
                            }
                            Err(e) => {
                                let _ = msg_tx.send(NetMsg::Action(Err(format!("reboot: {e}"))));
                            }
                        }
                    } else {
                        let _ = msg_tx.send(NetMsg::Action(Err("USB not open".into())));
                    }
                }
                NetCmd::FetchFirmware => {
                    let tx = msg_tx.clone();
                    let progress = move |line: String| {
                        log_msg(&tx, LogKind::Info, line);
                    };
                    let result = fetch_latest_firmware(&progress);
                    let _ = msg_tx.send(NetMsg::FirmwareFetched(result));
                }
                NetCmd::PullApiFeed(feed) => {
                    let outcome = pull_feed(&feed);
                    let _ = msg_tx.send(NetMsg::ApiFeedResult(outcome));
                }
            }
        }

        // Constant stratum communication updates while linked / mining.
        if let Some(client) = stratum.as_mut() {
            let was_authorized = client.authorized();
            match client.poll() {
                Ok(()) => {
                    if client.authorized() && !was_authorized {
                        // Publish authorize immediately so the UI does not linger on stale A/R.
                        push_stratum_live(&msg_tx, client);
                        let _ = msg_tx.send(NetMsg::MineStats {
                            accepted: 0,
                            rejected: 0,
                            phase: client.phase.clone(),
                        });
                        if let Some(p) = usb.as_mut() {
                            let _ = usb_cmd(
                                p.as_mut(),
                                &mut usb_rx,
                                "cmp stats accepted=0&rejected=0",
                            );
                        }
                    }
                    for line in client.take_recent() {
                        log_msg(&msg_tx, LogKind::Stratum, line);
                    }
                    for ev in client.take_share_events() {
                        let _ = msg_tx.send(NetMsg::Share(ev));
                    }
                    if let Some(job) = client.take_job() {
                        if let Some(p) = usb.as_mut() {
                            match usb_push_job(
                                p.as_mut(),
                                &mut usb_rx,
                                &job,
                                &mut legacy_job,
                                &msg_tx,
                            ) {
                                Ok(_) => {
                                    recent_jobs.push_back(job.clone());
                                    while recent_jobs.len() > 24 {
                                        recent_jobs.pop_front();
                                    }
                                    let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                        "USB ← job {}",
                                        job.job_id
                                    ))));
                                }
                                Err(e) => {
                                    let _ = msg_tx.send(NetMsg::Action(Err(e)));
                                }
                            }
                        }
                    }
                    if last_stats_push.elapsed() > Duration::from_secs(3) {
                        if let Some(p) = usb.as_mut() {
                            // Keep LCD at 0/0 through handshake and reject-grace warmup.
                            let (a, r) = if client.authorized() {
                                (client.accepted, client.rejected)
                            } else {
                                (0, 0)
                            };
                            let cmd = format!("cmp stats accepted={a}&rejected={r}");
                            let _ = usb_cmd(p.as_mut(), &mut usb_rx, &cmd);
                        }
                        last_stats_push = Instant::now();
                    }
                }
                Err(e) => {
                    log_msg(&msg_tx, LogKind::Err, format!("Pool: {e}"));
                    if let Some(mut s) = stratum.take() {
                        s.disconnect();
                    }
                    if mining && !mine_endpoint.is_empty() {
                        reconnect_at = Some(Instant::now() + reconnect_backoff);
                        log_msg(
                            &msg_tx,
                            LogKind::Warn,
                            format!(
                                "Pool reconnect in {}s…",
                                reconnect_backoff.as_secs().max(1)
                            ),
                        );
                        reconnect_backoff =
                            (reconnect_backoff * 2).min(Duration::from_secs(60));
                    }
                    thread::sleep(Duration::from_millis(200));
                }
            }
            if let Some(client) = stratum.as_mut() {
                if last_stratum_ui.elapsed() > Duration::from_millis(250) {
                    push_stratum_live(&msg_tx, client);
                    let _ = msg_tx.send(NetMsg::MineStats {
                        accepted: if client.authorized() {
                            client.accepted
                        } else {
                            0
                        },
                        rejected: if client.authorized() {
                            client.rejected
                        } else {
                            0
                        },
                        phase: client.phase.clone(),
                    });
                    last_stratum_ui = Instant::now();
                }
            }
        } else if mining && !mine_endpoint.is_empty() {
            let due = reconnect_at
                .map(|t| Instant::now() >= t)
                .unwrap_or(true);
            if due {
                reconnect_at = None;
                log_msg(
                    &msg_tx,
                    LogKind::Stratum,
                    format!("Reconnecting pool {mine_endpoint}…"),
                );
                let mut client =
                    StratumClient::new(mine_worker_name.clone(), mine_password.clone());
                match client.connect(&mine_endpoint) {
                    Ok(()) => {
                        for line in client.take_recent() {
                            log_msg(&msg_tx, LogKind::Stratum, line);
                        }
                        push_stratum_live(&msg_tx, &client);
                        stratum = Some(client);
                        reconnect_backoff = Duration::from_secs(2);
                        let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                            "Pool reconnected {mine_endpoint}"
                        ))));
                    }
                    Err(e) => {
                        reconnect_at = Some(Instant::now() + reconnect_backoff);
                        log_msg(
                            &msg_tx,
                            LogKind::Warn,
                            format!(
                                "Reconnect failed ({e}); retry in {}s",
                                reconnect_backoff.as_secs().max(1)
                            ),
                        );
                        reconnect_backoff =
                            (reconnect_backoff * 2).min(Duration::from_secs(60));
                    }
                }
            }
        }

        if let Some(p) = usb.as_mut() {
            harvest_shares(
                p.as_mut(),
                &mut usb_rx,
                stratum.as_mut(),
                &recent_jobs,
                &msg_tx,
            );
        }

        if mining || stratum.is_some() {
            thread::sleep(Duration::from_millis(15));
        }
    }
}

fn harvest_shares(
    port: &mut dyn SerialPort,
    buf: &mut String,
    stratum: Option<&mut StratumClient>,
    recent_jobs: &VecDeque<WorkJob>,
    msg_tx: &Sender<NetMsg>,
) {
    drain_serial(port, buf);
    let mut keep = String::new();
    let mut shares: Vec<(String, String, String, String)> = Vec::new();
    for line in buf.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("CMPSHARE ") {
            let mut nonce = String::new();
            let mut job = String::new();
            let mut en2 = String::new();
            let mut ntime = String::new();
            for pair in rest.split('&') {
                let mut it = pair.splitn(2, '=');
                let k = it.next().unwrap_or("");
                let v = it.next().unwrap_or("");
                match k {
                    "nonce" => nonce = v.to_string(),
                    "job" => job = v.to_string(),
                    "en2" => en2 = v.to_string(),
                    "ntime" => ntime = v.to_string(),
                    _ => {}
                }
            }
            shares.push((job, en2, ntime, nonce));
        } else if !t.is_empty() {
            keep.push_str(t);
            keep.push('\n');
        }
    }
    *buf = keep;
    if let Some(s) = stratum {
        for (job, en2, ntime, nonce) in shares {
            if job == "warmup" || job.is_empty() {
                log_msg(
                    msg_tx,
                    LogKind::Info,
                    format!("Ignoring local/warmup share nonce={nonce}"),
                );
                continue;
            }
            if let Some(wj) = recent_jobs
                .iter()
                .rev()
                .find(|j| j.job_id == job && j.extranonce2_hex == en2)
            {
                if let Err(e) = StratumClient::verify_share_against_job(wj, &nonce) {
                    log_msg(
                        msg_tx,
                        LogKind::Warn,
                        format!("Dropping bad board share nonce={nonce} job={job}: {e}"),
                    );
                    continue;
                }
            } else {
                log_msg(
                    msg_tx,
                    LogKind::Warn,
                    format!(
                        "Share job={job} en2={en2} not in recent job cache — submitting anyway"
                    ),
                );
            }
            match s.submit_share(&job, &en2, &ntime, &nonce) {
                Ok(()) => {
                    let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                        "Share submitted {nonce} job={job}"
                    ))));
                }
                Err(e) if e.contains("duplicate share") => {
                    log_msg(
                        msg_tx,
                        LogKind::Warn,
                        format!("Duplicate share skipped {nonce} job={job}"),
                    );
                }
                Err(e) if e.contains("not authorized") => {
                    // Board may still be hashing warmup / held work during pool handshake.
                    log_msg(
                        msg_tx,
                        LogKind::Info,
                        format!("Holding share {nonce} until authorize"),
                    );
                }
                Err(e) => {
                    let _ = msg_tx.send(NetMsg::Action(Err(format!("Share submit: {e}"))));
                }
            }
        }
    }
}

fn drain_serial(port: &mut dyn SerialPort, buf: &mut String) {
    let mut tmp = [0u8; 512];
    for _ in 0..40 {
        match port.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                buf.push_str(&String::from_utf8_lossy(&tmp[..n]));
                if buf.len() > 12288 {
                    *buf = buf[buf.len() - 4096..].to_string();
                }
            }
        }
    }
}

fn cmp_reply_line(buf: &str) -> Option<String> {
    for raw in buf.lines() {
        let t = raw.trim();
        for prefix in [
            "CMPSTATUS ",
            "CMPCONFIG ",
            "CMPBENCH ",
            "CMPACK",
            "CMP ok",
            "CMPERR",
            "CMPSHARE ",
        ] {
            if let Some(idx) = t.find(prefix) {
                if prefix == "CMPSHARE " {
                    continue;
                }
                return Some(t[idx..].to_string());
            }
        }
    }
    None
}

fn usb_cmd(port: &mut dyn SerialPort, buf: &mut String, cmd: &str) -> Result<String, String> {
    let mut last_err = String::new();
    let (wait_ms, retries, chunk, gap_ms) = if cmd.contains("bench") {
        (60_000u64, 2usize, 32usize, 3u64)
    } else if cmd.contains("status") {
        (1_800u64, 2usize, 64usize, 2u64)
    } else if cmd.contains(" jh") || cmd.contains(" jt") || cmd.contains(" ja") {
        (3_500u64, 3usize, 32usize, 4u64)
    } else if cmd.contains(" job ") {
        (6_000u64, 2usize, 32usize, 4u64)
    } else {
        (3_500u64, 3usize, 64usize, 2u64)
    };
    for _ in 0..retries {
        drain_serial(port, buf);
        let mut keep = String::new();
        for line in buf.lines() {
            if line.trim().starts_with("CMPSHARE ") {
                keep.push_str(line.trim());
                keep.push('\n');
            }
        }
        *buf = keep;
        let line = format!("\r\n{cmd}\r\n");
        for piece in line.as_bytes().chunks(chunk) {
            port.write_all(piece)
                .map_err(|e| format!("USB write: {e}"))?;
            let _ = port.flush();
            thread::sleep(Duration::from_millis(gap_ms));
        }
        let deadline = Instant::now() + Duration::from_millis(wait_ms);
        while Instant::now() < deadline {
            drain_serial(port, buf);
            if let Some(reply) = cmp_reply_line(buf) {
                if let Some(pos) = buf.find(&reply) {
                    let end = pos + reply.len();
                    let rest = format!("{}{}", &buf[..pos], &buf[end..]);
                    *buf = rest
                        .lines()
                        .filter(|l| l.trim().starts_with("CMPSHARE "))
                        .fold(String::new(), |mut acc, l| {
                            acc.push_str(l.trim());
                            acc.push('\n');
                            acc
                        });
                }
                if reply.starts_with("CMPERR") {
                    return Err(reply);
                }
                return Ok(reply);
            }
            thread::sleep(Duration::from_millis(10));
        }
        last_err = format!(
            "USB timeout waiting for reply to `{}`",
            cmd.chars().take(56).collect::<String>()
        );
        thread::sleep(Duration::from_millis(30));
    }
    Err(last_err)
}

fn fw_supports_split_jobs(fw: &str) -> bool {
    // Split jobs landed in firmware 0.6.2-sha256.
    let digits: String = fw
        .chars()
        .map(|c| if c.is_ascii_digit() || c == '.' { c } else { ' ' })
        .collect();
    let mut parts = digits.split_whitespace().next().unwrap_or("").split('.');
    let major: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch) >= (0, 6, 2)
}

fn usb_push_job(
    port: &mut dyn SerialPort,
    buf: &mut String,
    job: &stratum::WorkJob,
    legacy_job: &mut bool,
    msg_tx: &Sender<NetMsg>,
) -> Result<(), String> {
    if !*legacy_job {
        let mut split_ok = true;
        for part in encode_job_parts(job) {
            match usb_cmd(port, buf, &part) {
                Ok(reply) if reply.starts_with("CMPACK") || reply.starts_with("CMP ok") => {}
                Ok(reply) if reply.contains("unknown") || reply.starts_with("CMPERR") => {
                    split_ok = false;
                    log_msg(
                        msg_tx,
                        LogKind::Warn,
                        format!("Board rejected `{part}` ({reply}) — falling back to legacy job"),
                    );
                    break;
                }
                Ok(reply) => {
                    return Err(format!("unexpected job reply: {reply}"));
                }
                Err(e) if e.contains("CMPERR") && e.contains("unknown") => {
                    split_ok = false;
                    log_msg(
                        msg_tx,
                        LogKind::Warn,
                        format!("Board rejected split job ({e}) — falling back to legacy job"),
                    );
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        if split_ok {
            return Ok(());
        }
        *legacy_job = true;
        log_msg(
            msg_tx,
            LogKind::Warn,
            String::from("Flash firmware 0.6.2+ for reliable split jobs (jh/jt/ja)"),
        );
    }

    // Legacy one-shot — works on older boards; long line is less reliable.
    let cmd = encode_job_cmd(job);
    let reply = usb_cmd(port, buf, &cmd)?;
    if reply.starts_with("CMPACK") || reply.starts_with("CMP ok") {
        Ok(())
    } else {
        Err(format!("unexpected job reply: {reply}"))
    }
}

fn warmup_job() -> stratum::WorkJob {
    let mut header = [0u8; 80];
    header[0] = 0x01;
    header[68] = 0x5a;
    let mut target = [0xffu8; 32];
    target[31] = 0x0f;
    stratum::WorkJob {
        job_id: "warmup".into(),
        header,
        target,
        extranonce2_hex: "00000000".into(),
        ntime_hex: "00000000".into(),
    }
}

fn parse_cmp_status(line: &str) -> Result<StatusJson, String> {
    let json = line
        .strip_prefix("CMPSTATUS ")
        .ok_or_else(|| format!("bad status line: {line}"))?;
    serde_json::from_str(json).map_err(|e| e.to_string())
}

fn parse_cmp_config(line: &str) -> Result<ConfigJson, String> {
    let json = line
        .strip_prefix("CMPCONFIG ")
        .ok_or_else(|| format!("bad config line: {line}"))?;
    serde_json::from_str(json).map_err(|e| e.to_string())
}
