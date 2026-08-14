//! Njörðr seas CYD miner — USB / Wi‑Fi / Bluetooth worker control.
//! PC owns stratum; boards hash work received over USB-C or Wi‑Fi TCP.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api_feeds;
mod app_update;
mod desktop_icon;
mod flash_update;
mod live_bar;
mod monitor_api;
mod stratum;
mod workers;

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use api_feeds::{
    load_feeds, next_feed_id, pull_feed, save_feeds, ApiContentKind, ApiFeed, ApiPullOutcome,
    ApiSource,
};
use app_update::{check_app_update, running_version, update_companion_app, AppRemoteInfo};
use flash_update::{
    ensure_firmware_image, fetch_latest_firmware, find_firmware_image, flash_merged_bin,
    update_needed, FirmwareImage, FlashControl,
};
use live_bar::{
    default_header_coins, format_change, format_usd, COIN_CATALOG, LiveFeed,
};
use monitor_api::{
    generate_install_id, generate_token, pair_url, primary_lan_ipv4, qr_modules,
    start as start_monitor_api, web_pair_url, MonitorBoard, MonitorCreds, MonitorHub,
    MonitorSnapshot, MONITOR_PORT,
};
use stratum::{
    encode_job_cmd, encode_job_parts, expected_shares_per_hour, urlenc, ShareOutcome, StratumClient,
    WorkJob,
};
use workers::{
    count_usb_uart_ports, cyd_port_score, is_usb_serial_port, list_serial_ports, mac_is_stable,
    mac_worker_id, normalize_mac, open_usb_serial_timed, open_wifi_tcp, port_choice_is_pci,
    port_names_match, prefer_cyd_port, prefer_cyd_port_excluding, probe_esp_download_mode,
    scan_usb_workers_with_progress, transport_mac_id, BoardWifiDiscovery, DiscoveredWorker,
    LanDiscovery, PortChoice, WorkerKind, WorkerLive,
};

use eframe::egui::{
    self, Align, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, IconData, Layout,
    Margin, Pos2, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, TextEdit, Vec2,
};
use eframe::{App, NativeOptions};
use serde::{Deserialize, Serialize};
use serialport::SerialPort;

fn load_app_icon() -> Option<IconData> {
    // Prefer multi-size logo; taskbar/title-bar use this runtime icon (PE ICO is separate).
    let bytes = include_bytes!("../assets/cyd-logo.png");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    // Windows taskbar looks best with a square mid/large icon.
    let resized = if w != 256 || h != 256 {
        image::imageops::resize(&img, 256, 256, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    Some(IconData {
        rgba: resized.into_raw(),
        width: 256,
        height: 256,
    })
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1200.0, 820.0])
        .with_min_inner_size([640.0, 480.0])
        .with_title(format!(
            "Njörðr seas CYD miner {}",
            env!("CARGO_PKG_VERSION")
        ));
    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = NativeOptions {
        viewport,
        multisampling: 8,
        depth_buffer: 0,
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        "Njörðr seas CYD miner",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            install_fonts(&cc.egui_ctx);
            apply_theme(&cc.egui_ctx);
            // Auto-place branded Desktop shortcut (Windows) using the logo ICO.
            desktop_icon::spawn_auto_desktop_icon();
            Box::new(CompanionApp::new(cc.storage))
        }),
    )
}

const C_BG: Color32 = Color32::from_rgb(2, 10, 22); // deep Njörðr sea
const C_BG_2: Color32 = Color32::from_rgb(6, 28, 48);
const C_PANEL: Color32 = Color32::from_rgb(8, 22, 40);
const C_PANEL_SOFT: Color32 = Color32::from_rgb(12, 34, 58);
const C_PANEL_QUIET: Color32 = Color32::from_rgb(5, 18, 34);
const C_STROKE: Color32 = Color32::from_rgb(36, 78, 118);
const C_BUBBLE: Color32 = Color32::from_rgb(14, 42, 72);
const C_BUBBLE_HI: Color32 = Color32::from_rgb(88, 168, 210);
// Accent kept as C_LIME for call sites — electric ice / lightning, not chartreuse.
const C_LIME: Color32 = Color32::from_rgb(126, 220, 255);
const C_LIME_SOFT: Color32 = Color32::from_rgb(64, 160, 220);
const C_BOLT: Color32 = Color32::from_rgb(232, 246, 255);
/// Hot neon cyan — splash bloom around lightning strikes.
const C_NEON: Color32 = Color32::from_rgb(0, 229, 255);
const C_NEON_DEEP: Color32 = Color32::from_rgb(20, 120, 255);
const C_NEON_HOT: Color32 = Color32::from_rgb(180, 245, 255);
const C_TEXT: Color32 = Color32::from_rgb(230, 242, 252);
const C_MUTED: Color32 = Color32::from_rgb(120, 158, 186);
const C_DIM: Color32 = Color32::from_rgb(70, 108, 138);
const C_WARN: Color32 = Color32::from_rgb(255, 196, 91);
const C_ERR: Color32 = Color32::from_rgb(255, 108, 91);
const MAX_LOGS: usize = 500;
const HASH_HISTORY_SAMPLES: usize = 90;
const LOGO_PNG: egui::ImageSource<'static> = egui::include_image!("../assets/cyd-logo.png");

fn brand_logo(ui: &mut egui::Ui, height: f32) {
    let size = Vec2::splat(height);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect.expand(height * 0.22));
    // Neon blue splash halo behind the logo.
    painter.circle_filled(rect.center(), height * 0.62, rgba(C_NEON_DEEP, 28));
    painter.circle_filled(rect.center(), height * 0.48, rgba(C_NEON, 18));
    painter.circle_stroke(
        rect.center(),
        height * 0.52,
        Stroke::new(1.4_f32, rgba(C_NEON, 55)),
    );
    egui::Image::new(LOGO_PNG)
        .fit_to_exact_size(size)
        .rounding(Rounding::same((height * 0.18).clamp(6.0, 14.0)))
        .paint_at(ui, rect);
}

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

fn blue_widget(fill: Color32, stroke: Color32, fg: Color32) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0_f32, stroke),
        rounding: Rounding::same(16.0),
        fg_stroke: Stroke::new(1.0_f32, fg),
        expansion: 0.0,
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = C_BG;
    style.visuals.window_fill = C_PANEL;
    style.visuals.extreme_bg_color = Color32::from_rgb(3, 14, 28);
    style.visuals.override_text_color = Some(C_TEXT);
    // ComboBox / menus use inactive + open; leave weak_bg_fill unset and egui
    // can paint a light/white tab — force sea-blue to match SoftBtn.
    let open_fill = Color32::from_rgb(24, 64, 98);
    style.visuals.widgets.noninteractive =
        blue_widget(C_PANEL_SOFT, C_STROKE, C_MUTED);
    style.visuals.widgets.inactive = blue_widget(C_BUBBLE, C_STROKE, C_TEXT);
    style.visuals.widgets.hovered = blue_widget(open_fill, C_STROKE, C_TEXT);
    style.visuals.widgets.active =
        blue_widget(C_LIME, C_STROKE, Color32::from_rgb(4, 18, 36));
    style.visuals.widgets.open = blue_widget(open_fill, C_STROKE, C_TEXT);
    style.visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(126, 220, 255, 72);
    style.visuals.window_stroke = Stroke::new(1.0_f32, C_STROKE);
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
        Color32::from_rgb(4, 18, 36)
    } else {
        Color32::from_rgb(8, 24, 44)
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
    /// Hidden from nav (0.8.31+); kept so older persisted state still loads.
    #[allow(dead_code)]
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
    #[serde(default)]
    mac: String,
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
    #[serde(default)]
    mac: String,
}

#[derive(Clone)]
struct PostFlashVerify {
    port: String,
    /// Remaining OpenUsb / config retries after flash.
    attempts_left: u8,
    deadline: Instant,
}

/// Bump to force the in-app welcome wizard for existing installs after a setup redesign.
const WIZARD_REV: u32 = 3;

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
    /// Which welcome-wizard revision the user last completed/skipped.
    #[serde(default)]
    wizard_rev: u32,
    /// Public install id shown in QR / phone UI (not secret).
    #[serde(default)]
    monitor_install_id: String,
    /// Secret pairing token — only phones that scanned this Companion’s QR may poll.
    #[serde(default)]
    monitor_token: String,
    /// Optional public host / DDNS / Tailscale IP for remote phone access; empty = LAN IP.
    #[serde(default)]
    monitor_public_host: String,
    /// Symbols shown in the top price header (e.g. BTC, LTC, ETH).
    #[serde(default = "default_header_coins")]
    header_coins: Vec<String>,
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
    ("HM Pool (ESP32)", "stratum+tcp://btc.hmpool.io:3337"),
    ("Public Pool", "stratum+tcp://public-pool.io:21496"),
    ("Public Pool EU", "stratum+tcp://eu.public-pool.io:21496"),
    ("NerdMiner", "stratum+tcp://pool.nerdminers.org:3333"),
];

enum NetMsg {
    Ports(Vec<PortChoice>),
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
    /// Live erase/flash progress line for the loading overlay (not the terminal).
    FlashProgress(String),
    /// Firmware flash finished; `reopen` is the COM port to reclaim if flash succeeded.
    FlashDone {
        result: Result<String, String>,
        reopen: Option<String>,
    },
    Share(ShareOutcome),
    FirmwareFetched(Result<FirmwareImage, String>),
    /// Result of Check / Update Companion app.
    AppUpdate(Result<AppRemoteInfo, String>),
    ApiFeedResult(ApiPullOutcome),
    WorkersFound(Vec<DiscoveredWorker>),
    WorkersLive(Vec<WorkerLive>),
}

enum NetCmd {
    ListPorts,
    OpenUsb {
        name: String,
    },
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
        /// Set by UI Cancel — flash runner aborts and kills espflash.
        cancel: Arc<AtomicBool>,
        /// Flash thread sets true while waiting for BOOT Ready.
        need_boot: Arc<AtomicBool>,
        /// UI sets true when the user clicks Ready.
        boot_ready: Arc<AtomicBool>,
        /// Board was answering cmp — try auto-reset push without BOOT Ready first.
        live_push: bool,
    },
    /// Push live ticker text to the ESP LCD.
    PushNet {
        text: String,
    },
    RebootBoard,
    /// Pull one user-configured API feed in the worker thread.
    PullApiFeed(ApiFeed),
    /// Probe USB / Bluetooth / Wi‑Fi for CYD companion firmwares.
    ScanWorkers,
    /// Open an additional CYD USB/BT worker without dropping existing ones.
    ConnectWorker(String),
    /// Open a Wi‑Fi CYD worker (`host:port` TCP cmp).
    ConnectWifi(String),
    /// Drop one connected USB worker by COM port name.
    DisconnectWorker(String),
}

struct CompanionApp {
    tab: Tab,
    com_port: String,
    ports: Vec<PortChoice>,
    usb_open: bool,
    mining: bool,
    edit_stratum: String,
    edit_worker: String,
    edit_password: String,
    target_mhz: u8,
    status: StatusJson,
    fw_label: String,
    board_mac: String,
    pool_phase: String,
    accepted: u32,
    rejected: u32,
    last_error: String,
    last_ok: String,
    cmd_tx: Sender<NetCmd>,
    /// Clone for UI-side background work (fetch/update) that must not wait on mine_worker.
    msg_tx: Sender<NetMsg>,
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
    /// After a successful board flash: wait until Instant, then OpenUsb (no Close bounce).
    pending_post_flash_reconnect: Option<(Instant, String)>,
    /// After reconnect: confirm board answers `cmp config` with kit firmware.
    post_flash_verify: Option<PostFlashVerify>,
    update_busy: bool,
    /// When Update board started — UI watchdog clears spinner if flash never finishes.
    update_busy_since: Option<Instant>,
    /// Shared cancel flag for the in-flight flash tool.
    flash_cancel: Option<Arc<AtomicBool>>,
    /// Flash thread asks UI to show Ready (BOOT held).
    flash_need_boot: Option<Arc<AtomicBool>>,
    /// UI → flash thread: user clicked Ready.
    flash_boot_ready: Option<Arc<AtomicBool>>,
    /// 0..=1 overall Update board progress for the overlay bar.
    flash_progress: f32,
    /// Short phase label under the bar (Writing / Erasing / Verifying…).
    flash_phase: String,
    /// Manual / UI "Bench boards" in flight (mine-worker retune).
    bench_busy: bool,
    update_status: String,
    auto_connect: bool,
    auto_connect_attempted: bool,
    /// OpenUsb / ConnectWorker queued — block boot auto-connect from double-opening.
    usb_connect_pending: bool,
    /// App start — delayed COM re-lists (USB enum often lags first paint).
    boot_at: Instant,
    port_rescans_done: u8,
    session_started: Option<Instant>,
    session_hash_start: u64,
    share_history: VecDeque<ShareRow>,
    last_net_push: Instant,
    last_ticker: String,
    session_accepted: u32,
    session_rejected: u32,
    last_share_latency_ms: Option<u64>,
    /// Last time a pool job was pushed to boards (drives data-flow packets).
    last_job_flow_at: Instant,
    /// Last time a share was accepted/rejected (drives return-path packets).
    last_share_flow_at: Instant,
    /// None = wizard dismissed; Some(0..3) = step.
    wizard_step: Option<u8>,
    fetch_busy: bool,
    /// Companion self-update in progress.
    app_update_busy: bool,
    /// Last check/update result for the Companion app itself.
    app_remote: Option<AppRemoteInfo>,
    /// User-configured HTTP APIs that pull external info into the app.
    api_feeds: Vec<ApiFeed>,
    api_draft_name: String,
    api_draft_url: String,
    api_draft_auth: String,
    api_draft_path: String,
    api_draft_kind: ApiContentKind,
    /// Extra sources being edited before Add (url, kind, path).
    api_draft_extra_url: String,
    api_draft_extra_kind: ApiContentKind,
    api_draft_extra_path: String,
    api_draft_extras: Vec<ApiSource>,
    api_pulling_id: Option<u64>,
    last_api_auto_pull: Instant,
    /// USB/LAN CYD worker discovery results.
    discovered_workers: Vec<DiscoveredWorker>,
    connected_workers: Vec<WorkerLive>,
    worker_scan_busy: bool,
    lan: LanDiscovery,
    board_wifi: BoardWifiDiscovery,
    /// Shared snapshot + personal pairing creds for the phone monitor HTTP API (:19285).
    monitor: MonitorHub,
    monitor_addr: String,
    monitor_install_id: String,
    monitor_token: String,
    /// Optional public host / DDNS for QR (empty → auto LAN IPv4).
    monitor_public_host: String,
    /// Cached LAN IPv4 for QR / phone API (refreshed every few seconds — not every frame).
    monitor_lan_ip: String,
    monitor_lan_ip_at: Instant,
    /// Coins visible in the top live-price header (order matters).
    header_coins: Vec<String>,
}

impl CompanionApp {
    fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<NetCmd>();
        let (msg_tx, msg_rx) = mpsc::channel::<NetMsg>();
        let msg_tx_ui = msg_tx.clone();
        thread::spawn(move || mine_worker(cmd_rx, msg_tx));
        let _ = cmd_tx.send(NetCmd::ListPorts);

        // Port 3337 = HM Pool IoT/ESP32 (start diff ~0.01). Port 3335 is CPU/GPU (diff 128)
        // and produces mass Low-difficulty rejects on CYD hashrates.
        let mut edit_stratum = "stratum+tcp://btc.hmpool.io:3337".into();
        let mut edit_worker = String::new();
        let mut edit_password = "x".into();
        let mut target_mhz = 240u8;
        let mut com_port = String::new();
        let mut auto_connect = true; // new installs: link the CYD USB port on launch
        let mut wizard_done = false;
        let mut monitor_install_id = String::new();
        let mut monitor_token = String::new();
        let mut monitor_public_host = String::new();
        let mut header_coins = default_header_coins();
        let mut api_feeds: Vec<ApiFeed> = Vec::new();
        if let Some(storage) = storage {
            if let Some(raw) = storage.get_string("mine_prefs") {
                if let Ok(p) = serde_json::from_str::<PersistedMine>(&raw) {
                    if !p.stratum.is_empty() {
                        edit_stratum = p.stratum;
                    }
                    // Migrate old ESP32-hostile default without clobbering custom ports.
                    if edit_stratum.trim() == "stratum+tcp://btc.hmpool.io:3335"
                        || edit_stratum.trim() == "stratum+tcp://hmpool.io:3335"
                    {
                        edit_stratum = "stratum+tcp://btc.hmpool.io:3337".into();
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
                    // Force redesigned wizard once for existing installs.
                    wizard_done = p.wizard_done && p.wizard_rev >= WIZARD_REV;
                    monitor_install_id = p.monitor_install_id;
                    monitor_token = p.monitor_token;
                    monitor_public_host = p.monitor_public_host;
                    if !p.header_coins.is_empty() {
                        header_coins = p
                            .header_coins
                            .into_iter()
                            .map(|s| s.trim().to_ascii_uppercase())
                            .filter(|s| COIN_CATALOG.iter().any(|(_, sym)| *sym == s.as_str()))
                            .collect();
                        if header_coins.is_empty() {
                            header_coins = default_header_coins();
                        }
                        // Ensure LTC is present for upgrades that only had BTC/ETH/…
                        if !header_coins.iter().any(|s| s == "LTC")
                            && header_coins.len() < 5
                            && COIN_CATALOG.iter().any(|(_, s)| *s == "LTC")
                        {
                            // Insert LTC after BTC when upgrading older prefs.
                            if let Some(i) = header_coins.iter().position(|s| s == "BTC") {
                                header_coins.insert(i + 1, "LTC".into());
                            } else {
                                header_coins.insert(0, "LTC".into());
                            }
                        }
                    }
                }
            }
            if let Some(raw) = storage.get_string("api_feeds") {
                api_feeds = load_feeds(&raw);
            }
        }
        if monitor_install_id.trim().is_empty() {
            monitor_install_id = generate_install_id();
        }
        if monitor_token.trim().is_empty() {
            monitor_token = generate_token();
        }

        let monitor = MonitorHub::new(MonitorCreds {
            install_id: monitor_install_id.clone(),
            token: monitor_token.clone(),
        });

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
            board_mac: String::new(),
            pool_phase: "off".into(),
            accepted: 0,
            rejected: 0,
            last_error: String::new(),
            last_ok: "Connect USB-C, enter BTC address, Start mining. SHA-256 on board · pool on PC."
                .into(),
            cmd_tx,
            msg_tx: msg_tx_ui,
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
            pending_post_flash_reconnect: None,
            post_flash_verify: None,
            update_busy: false,
            update_busy_since: None,
            flash_cancel: None,
            flash_need_boot: None,
            flash_boot_ready: None,
            flash_progress: 0.0,
            flash_phase: String::new(),
            bench_busy: false,
            update_status: String::new(),
            auto_connect,
            auto_connect_attempted: false,
            usb_connect_pending: false,
            boot_at: Instant::now(),
            port_rescans_done: 0,
            session_started: None,
            session_hash_start: 0,
            share_history: VecDeque::new(),
            last_net_push: Instant::now() - Duration::from_secs(120),
            last_ticker: String::new(),
            session_accepted: 0,
            session_rejected: 0,
            last_share_latency_ms: None,
            last_job_flow_at: Instant::now() - Duration::from_secs(30),
            last_share_flow_at: Instant::now() - Duration::from_secs(30),
            wizard_step: if wizard_done { None } else { Some(0) },
            fetch_busy: false,
            app_update_busy: false,
            app_remote: None,
            api_feeds,
            api_draft_name: String::new(),
            api_draft_url: String::new(),
            api_draft_auth: String::new(),
            api_draft_path: String::new(),
            api_draft_kind: ApiContentKind::Auto,
            api_draft_extra_url: String::new(),
            api_draft_extra_kind: ApiContentKind::Auto,
            api_draft_extra_path: String::new(),
            api_draft_extras: Vec::new(),
            api_pulling_id: None,
            last_api_auto_pull: Instant::now() - Duration::from_secs(120),
            discovered_workers: Vec::new(),
            connected_workers: Vec::new(),
            worker_scan_busy: false,
            lan: LanDiscovery::start(),
            board_wifi: BoardWifiDiscovery::start(),
            monitor,
            monitor_addr: format!("0.0.0.0:{MONITOR_PORT}"),
            monitor_install_id,
            monitor_token,
            monitor_public_host,
            monitor_lan_ip: primary_lan_ipv4().unwrap_or_default(),
            monitor_lan_ip_at: Instant::now(),
            header_coins,
        };
        match start_monitor_api(app.monitor.clone()) {
            Ok(addr) => {
                app.monitor_addr = addr.to_string();
                app.push_log(
                    LogKind::Info,
                    format!(
                        "Personal phone monitor on :{MONITOR_PORT} · pair id {} · scan QR in Settings",
                        app.monitor_install_id
                    ),
                );
            }
            Err(e) => {
                app.push_log(LogKind::Warn, format!("Phone monitor API: {e}"));
            }
        }
        app.push_log(
            LogKind::Info,
            format!("Njörðr seas CYD miner {} ready", running_version()),
        );
        // Soft check for a newer Companion build — never queue behind USB/bench.
        app.spawn_app_update_check();
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

    fn publish_phone_monitor(&self) {
        let (acc, rej) = if self.stratum_live.authorized {
            if self.session_started.is_some() {
                (self.session_accepted, self.session_rejected)
            } else {
                (self.accepted, self.rejected)
            }
        } else {
            (0, 0)
        };
        let mut boards: Vec<MonitorBoard> = self
            .connected_workers
            .iter()
            .map(|w| MonitorBoard {
                endpoint: w.endpoint.clone(),
                mac: w.mac.clone(),
                fw: w.fw.clone(),
                hashrate_hs: w.hashrate_hs,
                hashes: w.hashes,
                mining: w.mining,
            })
            .collect();
        // Legacy single-USB path may not be in connected_workers — still publish it.
        if boards.is_empty() && self.usb_open {
            boards.push(MonitorBoard {
                endpoint: self.com_port.clone(),
                mac: self.board_mac.clone(),
                fw: self.fw_label.clone(),
                hashrate_hs: if self.displayed_khs > 0.5 {
                    self.displayed_khs as f64 * 1000.0
                } else {
                    self.status.hashrate_hs
                },
                hashes: self.status.hashes,
                mining: self.mining && self.status.mining,
            });
        }
        let snap = MonitorSnapshot {
            version: running_version().into(),
            product: "Njörðr Seas' CYD miner".into(),
            pair_id: self.monitor_install_id.clone(),
            mining: self.mining,
            usb_open: self.usb_open,
            pool_phase: if self.stratum_live.phase.is_empty() {
                self.pool_phase.clone()
            } else {
                self.stratum_live.phase.clone()
            },
            pool_authorized: self.stratum_live.authorized,
            hashrate_hs: if self.displayed_khs > 0.5 {
                self.displayed_khs as f64 * 1000.0
            } else {
                self.status.hashrate_hs
            },
            hashes: self.status.hashes,
            accepted: acc,
            rejected: rej,
            boards,
            host: self.monitor_connect_host(),
            updated_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        };
        self.monitor.publish(snap);
    }

    fn refresh_monitor_lan_ip(&mut self) {
        if self.monitor_lan_ip_at.elapsed() < Duration::from_secs(5)
            && !self.monitor_lan_ip.is_empty()
        {
            return;
        }
        self.monitor_lan_ip_at = Instant::now();
        if let Some(ip) = primary_lan_ipv4() {
            self.monitor_lan_ip = ip;
        }
    }

    fn monitor_connect_host(&self) -> String {
        let override_host = self.monitor_public_host.trim();
        if !override_host.is_empty() {
            return override_host
                .trim_start_matches("http://")
                .trim_start_matches("https://")
                .split('/')
                .next()
                .unwrap_or(override_host)
                .split(':')
                .next()
                .unwrap_or(override_host)
                .to_string();
        }
        if !self.monitor_lan_ip.is_empty() {
            return self.monitor_lan_ip.clone();
        }
        // Avoid hostname fallback when possible — phones rarely resolve Windows COMPUTERNAME.
        primary_lan_ipv4().unwrap_or_else(local_host_hint)
    }

    fn monitor_pair_url(&self) -> String {
        pair_url(
            &self.monitor_connect_host(),
            MONITOR_PORT,
            &self.monitor_install_id,
            &self.monitor_token,
        )
    }

    fn monitor_web_url(&self) -> String {
        web_pair_url(
            &self.monitor_connect_host(),
            MONITOR_PORT,
            &self.monitor_install_id,
            &self.monitor_token,
        )
    }

    fn regenerate_monitor_pairing(&mut self) {
        self.monitor_install_id = generate_install_id();
        self.monitor_token = generate_token();
        self.monitor.set_creds(MonitorCreds {
            install_id: self.monitor_install_id.clone(),
            token: self.monitor_token.clone(),
        });
        self.push_log(
            LogKind::Warn,
            format!(
                "Phone pairing reset · new pair id {} — old QR codes no longer work",
                self.monitor_install_id
            ),
        );
    }

    fn board_khs(&self) -> f32 {
        // Prefer hashrate_hs (authoritative). Clamp absurd spikes — ESP32 SHA
        // mining cannot sustain multi‑MH/s; those readings were counter bugs.
        const MAX_KHS: f32 = 2000.0; // 2 MH/s hard ceiling for display
        let khs = if self.status.hashrate_hs > 0.0 {
            (self.status.hashrate_hs / 1000.0) as f32
        } else if self.status.hashrate_khs > 0.0 {
            self.status.hashrate_khs as f32
        } else {
            0.0
        };
        khs.clamp(0.0, MAX_KHS)
    }

    fn board_hashing(&self) -> bool {
        // Live hashing — hold true across brief status soft-fails so the hero
        // subtitle / activity bars don't blink "waiting for hashrate".
        self.usb_open
            && self.mining
            && (self.status.mining
                || self.status.hashrate_hs > 0.0
                || self.displayed_khs > 1.0
                || self.status.hashes > self.session_hash_start)
    }

    /// Merge a fresh board status into the UI without blinking zeros on soft polls.
    fn absorb_status(&mut self, mut s: StatusJson) {
        if !s.mac.is_empty() {
            self.board_mac = normalize_mac(&s.mac);
        }
        let prev = &self.status;
        let hold = self.usb_open && self.mining;
        if hold {
            if s.hashrate_hs <= 0.0 && prev.hashrate_hs > 0.0 {
                s.hashrate_hs = prev.hashrate_hs;
                s.hashrate_khs = prev.hashrate_khs;
            }
            // Hash counters only rise while mining — never flash back to 0.
            if s.hashes < prev.hashes {
                s.hashes = prev.hashes;
            }
            if !s.mining && (prev.mining || s.hashrate_hs > 0.0 || prev.hashrate_hs > 0.0) {
                s.mining = true;
            }
            if s.job.is_empty() && !prev.job.is_empty() {
                s.job = prev.job.clone();
            }
            if s.nonce.is_empty() && !prev.nonce.is_empty() {
                s.nonce = prev.nonce.clone();
            }
            if s.sha_mode.is_empty() && !prev.sha_mode.is_empty() {
                s.sha_mode = prev.sha_mode.clone();
            }
            if s.pool.is_empty() && !prev.pool.is_empty() {
                s.pool = prev.pool.clone();
            }
            if s.mac.is_empty() && !prev.mac.is_empty() {
                s.mac = prev.mac.clone();
            }
            if s.uptime_secs == 0 && prev.uptime_secs > 0 {
                s.uptime_secs = prev.uptime_secs;
            }
            if s.cpu_mhz == 0 && prev.cpu_mhz > 0 {
                s.cpu_mhz = prev.cpu_mhz;
            }
            s.connected = true;
        }
        self.status = s;
        self.last_error.clear();
    }

    fn clear_hash_display(&mut self) {
        self.status.hashrate_hs = 0.0;
        self.status.hashrate_khs = 0.0;
        self.status.mining = false;
        self.status.connected = false;
        self.status.nonce.clear();
        self.status.job.clear();
        self.status.mac.clear();
        self.board_mac.clear();
        self.displayed_khs = 0.0;
        self.hashrate_history = VecDeque::from(vec![0.0; HASH_HISTORY_SAMPLES]);
        self.history_phase = 0.0;
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
        // While mining, never slow-bleed the display toward 0 on a missed poll —
        // that looked like "raises, slow drops, raises again". Hold flat on 0;
        // ease only when the board reports a sustained lower rate.
        let target = if !self.usb_open || !self.mining {
            0.0
        } else if target <= 0.0 && self.displayed_khs > 1.0 {
            self.displayed_khs
        } else if target > 0.0
            && self.displayed_khs > 80.0
            && target < self.displayed_khs * 0.55
        {
            // Single weak sample (one of N boards timed out) — ease, don't cliff.
            self.displayed_khs * 0.94 + target * 0.06
        } else {
            target
        };
        let alpha = 1.0 - (-dt * 1.8).exp();
        self.displayed_khs += (target - self.displayed_khs) * alpha;
        if self.displayed_khs.abs() < 0.05 {
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
            wizard_rev: if self.wizard_step.is_none() {
                WIZARD_REV
            } else {
                0
            },
            monitor_install_id: self.monitor_install_id.clone(),
            monitor_token: self.monitor_token.clone(),
            monitor_public_host: self.monitor_public_host.clone(),
            header_coins: self.header_coins.clone(),
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
        feed.kind = self.api_draft_kind;
        feed.sources = std::mem::take(&mut self.api_draft_extras);
        self.api_feeds.push(feed);
        self.api_draft_name.clear();
        self.api_draft_url.clear();
        self.api_draft_auth.clear();
        self.api_draft_path.clear();
        self.api_draft_kind = ApiContentKind::Auto;
        self.api_draft_extra_url.clear();
        self.api_draft_extra_path.clear();
        self.api_draft_extra_kind = ApiContentKind::Auto;
        let n_src = self.api_feeds.last().map(|f| f.all_sources().len()).unwrap_or(1);
        self.last_ok = format!(
            "API feed added ({n_src} source{}) — Pull now to load data.",
            if n_src == 1 { "" } else { "s" }
        );
        self.push_log(LogKind::Info, self.last_ok.clone());
        if let Some(last) = self.api_feeds.last() {
            self.request_api_pull(last.id);
        }
    }

    fn add_api_draft_extra_source(&mut self) {
        let url = self.api_draft_extra_url.trim().to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.last_error = "Extra source needs an http(s) URL.".into();
            return;
        }
        self.api_draft_extras.push(ApiSource::new(
            url,
            self.api_draft_extra_kind,
            self.api_draft_extra_path.trim().to_string(),
        ));
        self.api_draft_extra_url.clear();
        self.api_draft_extra_path.clear();
        self.api_draft_extra_kind = ApiContentKind::Auto;
        self.last_ok = format!(
            "Extra source queued ({} total extras).",
            self.api_draft_extras.len()
        );
    }

    fn apply_api_pull(&mut self, outcome: ApiPullOutcome) {
        if self.api_pulling_id == Some(outcome.id) {
            self.api_pulling_id = None;
        }
        if let Some(feed) = self.api_feeds.iter_mut().find(|f| f.id == outcome.id) {
            feed.last_status = outcome.status.clone();
            feed.last_summary = outcome.summary.clone();
            feed.last_preview = outcome.preview;
            feed.last_saved = outcome.saved.clone();
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

    fn merge_discovered(&mut self, worker: DiscoveredWorker) {
        // Prefer stable MAC identity within the same transport. Never merge USB↔Wi‑Fi
        // by MAC (that hid the second path and collapsed multi-board SoftAP rows).
        // Never merge on mac=unknown — that collapsed distinct boards into one.
        if let Some(existing) = self.discovered_workers.iter_mut().find(|w| {
            w.id == worker.id
                || (w.kind == worker.kind
                    && mac_is_stable(&worker.mac)
                    && mac_is_stable(&w.mac)
                    && normalize_mac(&w.mac) == normalize_mac(&worker.mac))
                || (w.kind == WorkerKind::Usb
                    && worker.kind == WorkerKind::Usb
                    && port_names_match(&w.endpoint, &worker.endpoint))
                || (w.kind == WorkerKind::Wifi
                    && worker.kind == WorkerKind::Wifi
                    && w.endpoint == worker.endpoint)
        }) {
            *existing = worker;
        } else {
            self.discovered_workers.push(worker);
        }
        self.discovered_workers
            .sort_by(|a, b| a.mac.cmp(&b.mac).then(a.endpoint.cmp(&b.endpoint)));
    }

    fn worker_already_linked(&self, endpoint: &str) -> bool {
        self.connected_workers
            .iter()
            .any(|c| port_names_match(&c.endpoint, endpoint) || c.endpoint == endpoint)
    }

    fn worker_mac_already_linked(&self, mac: &str) -> bool {
        mac_is_stable(mac)
            && self.connected_workers.iter().any(|c| {
                mac_is_stable(&c.mac) && normalize_mac(&c.mac) == normalize_mac(mac)
            })
    }

    /// Prefer the next USB COM that is not already linked (for Add board).
    /// Never pick motherboard PCI — that made the UI “lose” the ESP after Connect.
    fn select_next_unlinked_usb(&mut self) -> bool {
        let linked: Vec<String> = self
            .connected_workers
            .iter()
            .map(|c| c.endpoint.clone())
            .collect();
        if let Some(p) = prefer_cyd_port_excluding(&self.ports, &linked) {
            if !port_names_match(&p.name, &self.com_port) {
                self.com_port = p.name.clone();
                self.push_log(
                    LogKind::Usb,
                    format!("COM selection → {} (next unlinked USB for Add board)", p.name),
                );
            } else {
                self.com_port = p.name.clone();
            }
            true
        } else {
            false
        }
    }

    fn linked_endpoints(&self) -> Vec<String> {
        self.connected_workers
            .iter()
            .map(|c| c.endpoint.clone())
            .collect()
    }

    /// Queue ConnectWorker for every unlinked USB-UART that looks like a CYD.
    fn link_all_unlinked_usb(&mut self) {
        if self.update_busy {
            self.last_error = "Wait for Update board / flash to finish.".into();
            return;
        }
        let linked = self.linked_endpoints();
        let candidates: Vec<String> = self
            .ports
            .iter()
            .filter(|p| !port_choice_is_pci(p) && cyd_port_score(p) >= 0)
            .filter(|p| !linked.iter().any(|e| port_names_match(e, &p.name)))
            .map(|p| p.name.clone())
            .collect();
        if candidates.is_empty() {
            let usb_n = count_usb_uart_ports(&self.ports);
            self.last_ok = if usb_n <= 1 {
                "No 2nd USB COM yet — Windows only lists one USB-UART. Use a data cable on another PC USB port, check Device Manager → Ports, then Refresh.".into()
            } else {
                "No other USB COM to link — Refresh, or Find CYD workers.".into()
            };
            self.push_log(LogKind::Warn, self.last_ok.clone());
            // Still scan — probe may find a board the score skipped.
            self.worker_scan_busy = true;
            let _ = self.cmd_tx.send(NetCmd::ScanWorkers);
            return;
        }
        for name in &candidates {
            self.push_log(LogKind::Usb, format!("Linking USB worker {name}…"));
            let _ = self.cmd_tx.send(NetCmd::ConnectWorker(name.clone()));
        }
        self.usb_connect_pending = true;
        self.last_ok = format!("Linking {} USB COM(s)…", candidates.len());
        // Point combo at the first candidate we just queued.
        if let Some(first) = candidates.first() {
            self.com_port = first.clone();
        }
    }

    fn apply_best_com_port(&mut self, force: bool) {
        let linked = self.linked_endpoints();
        // When boards are already linked, prefer an *unlinked* USB so Add board
        // does not keep snapping back to the first CYD after Refresh.
        let best = if !linked.is_empty() {
            prefer_cyd_port_excluding(&self.ports, &linked).or_else(|| prefer_cyd_port(&self.ports))
        } else {
            prefer_cyd_port(&self.ports)
        };
        let selected = self
            .ports
            .iter()
            .find(|p| port_names_match(&p.name, &self.com_port));
        // Persisted PCI / gone selection with no usable CYD — clear so UI shows Select port.
        if best.is_none() {
            if self.com_port.is_empty() {
                return;
            }
            let bad = selected
                .map(|p| port_choice_is_pci(p) || cyd_port_score(p) < 0)
                .unwrap_or(true);
            if bad {
                self.com_port.clear();
            }
            return;
        }
        let best = best.unwrap();
        if force || self.com_port.is_empty() {
            self.com_port = best.name.clone();
            return;
        }
        // Never "upgrade" onto a COM that is already linked while another USB is free.
        if !linked.is_empty()
            && linked.iter().any(|e| port_names_match(e, &best.name))
            && prefer_cyd_port_excluding(&self.ports, &linked).is_some()
        {
            // Keep current if it is a free USB; else snap to next unlinked.
            let selected_free = selected
                .map(|p| {
                    !port_choice_is_pci(p)
                        && cyd_port_score(p) >= 0
                        && !linked.iter().any(|e| port_names_match(e, &p.name))
                })
                .unwrap_or(false);
            if !selected_free {
                let _ = self.select_next_unlinked_usb();
            }
            return;
        }
        let selected_score = selected.map(cyd_port_score).unwrap_or(-999);
        let selected_bad = selected
            .map(|p| port_choice_is_pci(p) || cyd_port_score(p) < 0)
            .unwrap_or(true);
        let missing = selected.is_none();
        let upgrade = cyd_port_score(best) > selected_score + 15;
        if missing || selected_bad || upgrade {
            self.com_port = best.name.clone();
            self.push_log(
                LogKind::Usb,
                format!("COM selection → {} (preferred USB-UART for CYD)", best.name),
            );
        }
    }

    /// Rescan OS serial ports on the UI thread (do not wait on the USB worker).
    ///
    /// `allow_auto_connect` is only for boot/delayed discovery — the Board & pool
    /// **Refresh** button must never open/reconnect USB; it only refreshes the COM list.
    fn refresh_com_ports(&mut self, force_best: bool, allow_auto_connect: bool) {
        let ports = list_serial_ports();
        let n = ports.len();
        let usb_n = count_usb_uart_ports(&ports);
        let pci_n = ports.iter().filter(|p| port_choice_is_pci(p)).count();
        self.ports = ports;
        self.apply_best_com_port(force_best);
        let selected = if self.com_port.is_empty() {
            "—".into()
        } else {
            self.com_port.clone()
        };
        self.last_ok = format!(
            "COM list · {n} port(s) · {usb_n} USB-UART · {pci_n} PCI · selected {selected}"
        );
        self.push_log(LogKind::Usb, self.last_ok.clone());
        let port_labels: Vec<String> = self.ports.iter().map(|p| p.label.clone()).collect();
        for label in port_labels {
            self.push_log(LogKind::Usb, format!("  · {label}"));
        }
        if self.usb_open && usb_n <= 1 {
            self.push_log(
                LogKind::Warn,
                "Only one USB-UART COM is visible to Windows. A 2nd CYD needs its own data cable \
+ its own COM in Device Manager → Ports (COM & LPT). COM1 PCI is not a board."
                    .into(),
            );
        }
        if allow_auto_connect {
            // Do not clear attempted while a connect is already in flight.
            if !self.usb_connect_pending {
                self.maybe_auto_connect_usb();
            }
        }
    }

    fn maybe_auto_connect_usb(&mut self) {
        if self.auto_connect
            && !self.auto_connect_attempted
            && !self.usb_connect_pending
            && !self.usb_open
            && !self.update_busy
            && !self.com_port.is_empty()
            && self.ports.iter().any(|x| x.name == self.com_port)
            && self
                .ports
                .iter()
                .find(|x| x.name == self.com_port)
                .map(|p| !port_choice_is_pci(p))
                .unwrap_or(false)
        {
            self.auto_connect_attempted = true;
            self.connect_or_add_usb();
        }
    }

    fn connect_or_add_usb(&mut self) {
        self.last_error.clear();
        if self.update_busy {
            self.last_error = "Wait for Update board / flash to finish.".into();
            return;
        }
        if self.com_port.is_empty() {
            self.last_error = "Select a COM / serial port.".into();
            return;
        }
        if self.worker_already_linked(&self.com_port) {
            self.last_ok = format!("{} already linked", self.com_port);
            return;
        }
        if self.usb_open {
            let _ = self
                .cmd_tx
                .send(NetCmd::ConnectWorker(self.com_port.clone()));
            self.usb_connect_pending = true;
            self.last_ok = format!("Adding board {}…", self.com_port);
            self.push_log(LogKind::Usb, format!("Adding worker {}", self.com_port));
        } else {
            let _ = self.cmd_tx.send(NetCmd::OpenUsb {
                name: self.com_port.clone(),
            });
            self.usb_connect_pending = true;
            self.last_ok = format!("Opening {}…", self.com_port);
            self.push_log(LogKind::Usb, format!("Opening {}", self.com_port));
        }
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
        self.clear_hash_display();
        self.last_ok = "Mining stopped.".into();
        self.push_log(LogKind::Info, "Mining stopped".into());
    }

    fn request_bench(&mut self) {
        if self.bench_busy {
            return;
        }
        if !self.usb_open && self.connected_workers.is_empty() {
            self.last_error = "Link a board before Bench.".into();
            self.push_log(LogKind::Warn, self.last_error.clone());
            return;
        }
        self.bench_busy = true;
        self.last_error.clear();
        self.last_ok = "Bench: tuning HW / HW+ / HW/SW on each linked board…".into();
        self.push_log(
            LogKind::Usb,
            "D0 auto-tune: each board times HW / HW+ / HW/SW and locks the best path…".into(),
        );
        let _ = self.cmd_tx.send(NetCmd::Bench);
    }

    fn push_board_ticker(&mut self, force: bool) {
        if !self.usb_open || self.update_busy {
            return;
        }
        let tick = self.live.board_ticker_for(&self.header_coins);
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

    /// Human label for the active SHA hash path (HW = full ESP SHA silicon).
    fn sha_path_display(&self) -> String {
        match self.sha_mode_label() {
            "HW" => "Full HW".into(),
            "HW+" => "HW+ mid".into(),
            "HW/SW" => "HW/SW".into(),
            "SW" => "Software".into(),
            "—" => "—".into(),
            other => other.to_string(),
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

    fn resolve_flash_usb_port(&self) -> Result<String, String> {
        if is_usb_serial_port(&self.com_port) {
            return Ok(self.com_port.clone());
        }
        // Prefer a linked USB board when the UI selection is Wi‑Fi / LAN.
        for w in &self.connected_workers {
            if is_usb_serial_port(&w.endpoint) {
                return Ok(w.endpoint.clone());
            }
        }
        for p in &self.ports {
            if is_usb_serial_port(&p.name) {
                return Ok(p.name.clone());
            }
        }
        Err(
            "Select a USB COM port to flash (Wi‑Fi boards cannot be flashed over TCP)."
                .into(),
        )
    }

    fn request_board_update(&mut self) {
        self.firmware = find_firmware_image().ok().or_else(|| self.firmware.clone());
        match self.resolve_flash_usb_port() {
            Ok(port) => {
                if port != self.com_port {
                    self.push_log(
                        LogKind::Info,
                        format!("Flash will use USB port {port} (selection was {})", self.com_port),
                    );
                    self.com_port = port;
                }
            }
            Err(e) => {
                self.last_error = e;
                return;
            }
        }
        // Missing local image is OK — worker will fetch Firmware\\ + Tools\\espflash.
        if self.firmware.is_none() {
            self.push_log(
                LogKind::Info,
                "No local firmware yet — Update will download image + espflash.".into(),
            );
        }
        self.update_confirm = true;
    }

    fn start_firmware_fetch(&mut self) {
        if self.fetch_busy {
            return;
        }
        self.fetch_busy = true;
        self.update_status = "Fetching latest board firmware…".into();
        self.push_log(LogKind::Info, "Fetching latest board firmware…".into());
        let tx = self.msg_tx.clone();
        thread::spawn(move || {
            let progress = {
                let tx = tx.clone();
                move |line: String| {
                    let _ = tx.send(NetMsg::FlashProgress(line.clone()));
                    log_msg(&tx, LogKind::Info, line);
                }
            };
            let result = fetch_latest_firmware(&progress);
            let _ = tx.send(NetMsg::FirmwareFetched(result));
        });
    }

    fn spawn_app_update_check(&mut self) {
        let tx = self.msg_tx.clone();
        thread::spawn(move || {
            let progress = {
                let tx = tx.clone();
                move |line: String| log_msg(&tx, LogKind::Info, line)
            };
            let result = check_app_update(&progress);
            let _ = tx.send(NetMsg::AppUpdate(result));
        });
    }

    fn start_app_update_check(&mut self) {
        if self.app_update_busy {
            return;
        }
        self.app_update_busy = true;
        self.update_status = "Checking for Companion updates…".into();
        self.push_log(LogKind::Info, "Checking for Companion updates…".into());
        self.spawn_app_update_check();
    }

    fn start_app_update(&mut self) {
        if self.app_update_busy || self.update_busy {
            return;
        }
        if self.mining {
            self.stop_mine();
        }
        self.app_update_busy = true;
        self.update_status = format!("Updating Companion {}…", running_version());
        self.last_ok = self.update_status.clone();
        self.push_log(LogKind::Info, self.update_status.clone());
        let tx = self.msg_tx.clone();
        thread::spawn(move || {
            let progress = {
                let tx = tx.clone();
                move |line: String| {
                    let _ = tx.send(NetMsg::FlashProgress(line.clone()));
                    log_msg(&tx, LogKind::Info, line);
                }
            };
            let result = update_companion_app(&progress);
            let _ = tx.send(NetMsg::AppUpdate(result));
        });
    }

    /// True when the target COM is a live companion board (cmp answering) — can
    /// push update via auto-reset without holding BOOT.
    fn board_supports_live_push(&self, port: &str) -> bool {
        self.connected_workers.iter().any(|c| {
            port_names_match(&c.endpoint, port)
                && !c.fw.is_empty()
                && !c.fw.eq_ignore_ascii_case("download-mode")
                && !c.fw.to_ascii_lowercase().contains("download")
        })
    }

    fn begin_board_update(&mut self) {
        self.update_confirm = false;
        let port = match self.resolve_flash_usb_port() {
            Ok(p) => p,
            Err(e) => {
                self.last_error = e;
                return;
            }
        };
        self.com_port = port.clone();
        let live_push = self.board_supports_live_push(&port);
        if self.mining {
            self.stop_mine();
        }
        let image = self
            .firmware
            .as_ref()
            .map(|fw| fw.path.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.update_busy = true;
        self.update_busy_since = Some(Instant::now());
        let cancel = Arc::new(AtomicBool::new(false));
        let need_boot = Arc::new(AtomicBool::new(false));
        let boot_ready = Arc::new(AtomicBool::new(false));
        self.flash_cancel = Some(cancel.clone());
        self.flash_need_boot = Some(need_boot.clone());
        self.flash_boot_ready = Some(boot_ready.clone());
        self.flash_progress = 0.02;
        self.flash_phase = if live_push {
            "Pushing update".into()
        } else {
            "Starting flash".into()
        };
        self.pending_post_flash_reconnect = None;
        self.post_flash_verify = None;
        self.update_status = if live_push {
            format!("Pushing firmware update to {port} (auto-reset)…")
        } else {
            format!("Flashing board via {port}…")
        };
        self.last_ok = self.update_status.clone();
        self.last_error.clear();
        self.push_log(
            LogKind::Usb,
            if live_push {
                format!("Push update → {port} (no BOOT; auto-reset)")
            } else if image.is_empty() {
                format!("Update board → fetch firmware + flash on {port}")
            } else {
                format!("Update board → {image} on {port}")
            },
        );
        // Release USB in the worker before flash (port must be free).
        self.usb_open = false;
        self.mining = false;
        let _ = self.cmd_tx.send(NetCmd::UpdateFirmware {
            port,
            image,
            // Always reconnect after a successful flash.
            reopen: true,
            cancel,
            need_boot,
            boot_ready,
            live_push,
        });
    }

    fn clear_flash_overlay(&mut self) {
        self.update_busy = false;
        self.update_busy_since = None;
        self.pending_post_flash_reconnect = None;
        self.post_flash_verify = None;
        self.flash_progress = 0.0;
        self.flash_phase.clear();
        if let Some(c) = self.flash_cancel.take() {
            c.store(true, Ordering::SeqCst);
        }
        if let Some(n) = self.flash_need_boot.take() {
            n.store(false, Ordering::SeqCst);
        }
        if let Some(r) = self.flash_boot_ready.take() {
            r.store(false, Ordering::SeqCst);
        }
    }

    /// Parse espflash / phase lines into overall 0..=1 progress for the overlay bar.
    fn absorb_flash_progress_line(&mut self, line: &str) {
        self.update_status = trunc(line, 140);
        let lower = line.to_ascii_lowercase();

        // Prefer explicit percent from espflash ("12%", "12.5 %", "[====] 45%").
        if let Some(pct) = parse_flash_percent(line) {
            // Map tool-local 0..=100 into the write band of the overall bar.
            let mapped = 0.12 + (pct / 100.0) * 0.70;
            if mapped > self.flash_progress {
                self.flash_progress = mapped.clamp(0.0, 0.92);
            }
            if self.flash_phase.is_empty() || self.flash_phase.starts_with("Writing") {
                self.flash_phase = format!("Writing firmware · {pct:.0}%");
            } else if lower.contains("eras") {
                self.flash_phase = format!("Erasing · {pct:.0}%");
            }
            return;
        }

        let (phase, floor) = if lower.contains("cancelled") {
            ("Cancelled", self.flash_progress)
        } else if lower.contains("waiting for ready")
            || lower.contains("hold boot")
            || lower.contains("click ready")
        {
            ("Hold BOOT — click Ready", 0.08)
        } else if lower.contains("rom sync") || lower.contains("syncing esp rom") {
            ("Syncing download mode", 0.10)
        } else if lower.contains("verif")
            || lower.contains("reading board config")
            || lower.contains("usb linked after flash")
        {
            ("Verifying board", 0.92)
        } else if lower.contains("flash ok")
            || lower.contains("booting board")
            || lower.contains("reconnecting")
            || lower.contains("reconnect")
        {
            ("Reconnecting", 0.88)
        } else if lower.contains("complete") && lower.contains("espflash") {
            ("Write complete", 0.86)
        } else if lower.contains("erase") {
            ("Erasing flash", 0.08)
        } else if lower.contains("chip seen")
            || (lower.contains("mac") && lower.contains("connect"))
        {
            ("Chip connected", 0.14)
        } else if lower.contains("write-bin")
            || lower.contains("writing firmware")
            || lower.contains("writing after")
            || lower.contains("patient write")
        {
            ("Writing firmware", 0.12)
        } else if lower.contains("usb released") || lower.contains("waiting for com") {
            ("Releasing USB", 0.04)
        } else if lower.contains("flash budget") || lower.contains("starting") {
            ("Starting flash", 0.02)
        } else if lower.contains("download") && lower.contains("firmware") {
            ("Downloading firmware", 0.05)
        } else if lower.contains("espflash") && lower.contains("found") {
            ("Preparing flash tool", 0.06)
        } else {
            return;
        };
        self.flash_phase = phase.into();
        if floor > self.flash_progress {
            self.flash_progress = floor;
        }
    }

    fn schedule_post_flash_reconnect(&mut self, port: String, delay: Duration) {
        self.update_busy = true;
        if self.update_busy_since.is_none() {
            self.update_busy_since = Some(Instant::now());
        }
        self.com_port = port.clone();
        self.pending_post_flash_reconnect = Some((Instant::now() + delay, port));
    }

    fn begin_post_flash_verify(&mut self, port: String) {
        self.post_flash_verify = Some(PostFlashVerify {
            port: port.clone(),
            attempts_left: 2,
            deadline: Instant::now() + Duration::from_secs(75),
        });
        self.update_status = format!("Flash OK — booting board, then verifying on {port}…");
        self.flash_phase = "Reconnecting".into();
        self.flash_progress = self.flash_progress.max(0.88);
        // Single quiet reopen after boot settle — no CloseUsb churn.
        self.schedule_post_flash_reconnect(port, Duration::from_secs(4));
    }

    fn finish_post_flash_ok(&mut self, board_fw: &str) {
        let msg = if board_fw.is_empty() {
            "Flash verified — board responded over USB.".to_string()
        } else {
            format!("Flash verified · board fw {board_fw}")
        };
        self.last_ok = msg.clone();
        self.last_error.clear();
        self.update_status = msg.clone();
        self.flash_phase = "Verified".into();
        self.flash_progress = 1.0;
        self.push_log(LogKind::Usb, msg);
        self.clear_flash_overlay();
    }

    fn fail_post_flash_verify(&mut self, reason: String) {
        let tip = format!(
            "{reason} Hold BOOT, tap RESET, keep BOOT held, click Ready, then Update board again."
        );
        self.update_status = tip.clone();
        self.last_error = tip.clone();
        self.push_log(LogKind::Err, tip);
        self.clear_flash_overlay();
    }

    fn retry_post_flash_verify(&mut self, why: &str) {
        let Some(mut v) = self.post_flash_verify.take() else {
            return;
        };
        if v.attempts_left == 0 || Instant::now() >= v.deadline {
            self.fail_post_flash_verify(format!(
                "Flash wrote OK but verify failed ({why})."
            ));
            return;
        }
        v.attempts_left = v.attempts_left.saturating_sub(1);
        let port = v.port.clone();
        self.post_flash_verify = Some(v);
        if self.usb_open {
            // Already linked — wait for the next config/status tick; do NOT bounce the port.
            self.update_status = format!(
                "Verify wait ({why}) — USB stays linked, waiting for board config…"
            );
            self.push_log(LogKind::Usb, self.update_status.clone());
        } else {
            self.update_status = format!(
                "Verify retry: {why} — opening {port} (no disconnect bounce)…"
            );
            self.push_log(LogKind::Usb, self.update_status.clone());
            self.schedule_post_flash_reconnect(port, Duration::from_secs(3));
        }
    }

    fn on_post_flash_config(&mut self, board_fw: &str) {
        if self.post_flash_verify.is_none() {
            return;
        }
        let kit = self
            .firmware
            .as_ref()
            .map(|f| f.version.clone())
            .unwrap_or_default();
        if board_fw.is_empty() {
            self.retry_post_flash_verify("board returned empty fw tag");
            return;
        }
        match update_needed(board_fw, &kit) {
            Some(true) => {
                // Flash write already succeeded and the board answers USB — don't
                // thrash COM over a tag mismatch (kit VERSION vs board kFwTag).
                self.push_log(
                    LogKind::Warn,
                    format!("Post-flash tag differs · board {board_fw} · kit {kit}"),
                );
                self.finish_post_flash_ok(&format!("{board_fw} (kit expected {kit})"));
            }
            _ => {
                self.finish_post_flash_ok(board_fw);
            }
        }
    }

    #[allow(dead_code)]
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
        // Brand-first Mine: hero → link board/pool → telemetry → events.
        // Updates, best-path tips, phone QR, and API feeds live in Settings.
        self.ui_mining_hero(ui);
        ui.add_space(20.0);

        let wide = ui.available_width() >= 880.0;
        if wide {
            ui.columns(2, |columns| {
                let (left, right) = columns.split_at_mut(1);
                self.ui_connection_controls(&mut left[0]);
                self.ui_telemetry_rail(&mut right[0]);
            });
        } else {
            self.ui_connection_controls(ui);
            ui.add_space(14.0);
            self.ui_telemetry_rail(ui);
        }

        ui.add_space(16.0);
        self.ui_logs_panel(ui);
    }

    #[allow(dead_code)]
    fn ui_data_flow(&self, ui: &mut egui::Ui) {
        let (pool_label, pool_color) = self.pool_state();
        let boards = self.connected_workers.len().max(if self.usb_open { 1 } else { 0 });
        let jobs_live = self.mining && self.stratum_live.authorized;
        let hash_live = self.board_hashing();
        let job_burst = self.last_job_flow_at.elapsed() < Duration::from_millis(2_400);
        let share_burst = self.last_share_flow_at.elapsed() < Duration::from_millis(2_800);
        soft_panel(ui, "Data flow", |ui| {
            ui.label(
                RichText::new("Pool jobs down · board hashes · shares back up")
                    .color(C_DIM)
                    .font(mono_ui_font(11.0)),
            );
            ui.add_space(8.0);
            paint_data_flow(
                ui,
                DataFlowView {
                    pulse: self.pulse,
                    pool_label,
                    pool_color,
                    usb_open: self.usb_open,
                    mining: self.mining,
                    jobs_live: jobs_live || job_burst,
                    hash_live,
                    share_burst: share_burst || (hash_live && jobs_live),
                    board_count: boards,
                    rate_label: format_hashrate(self.displayed_khs as f64 * 1000.0),
                    accepted: if self.stratum_live.authorized {
                        self.session_accepted
                    } else {
                        0
                    },
                },
            );
        });
    }

    fn ui_mining_hero(&mut self, ui: &mut egui::Ui) {
        let hashing = self.mining || self.board_hashing();
        let pulse = 0.5 + 0.5 * self.pulse.sin();
        let pulse_alpha = if hashing {
            (28.0 + pulse * 48.0) as u8
        } else {
            16
        };
        Frame::none()
            .fill(Color32::from_rgba_unmultiplied(4, 16, 32, 200))
            .rounding(Rounding::same(28.0))
            .stroke(Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(126, 220, 255, pulse_alpha),
            ))
            .inner_margin(Margin::symmetric(26.0, 22.0))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                paint_hero_wash(ui, rect, self.pulse, hashing);
                ui.set_min_height(if ui.available_width() < 700.0 {
                    200.0
                } else {
                    236.0
                });

                let narrow = ui.available_width() < 700.0;
                ui.horizontal(|ui| {
                    // Brand column — must survive even if nav were removed.
                    ui.vertical(|ui| {
                        let col_w = if narrow {
                            ui.available_width()
                        } else {
                            (ui.available_width() * 0.62).clamp(260.0, 760.0)
                        };
                        ui.set_min_width(col_w);
                        ui.horizontal(|ui| {
                            brand_logo(ui, if narrow { 88.0 } else { 132.0 });
                            ui.add_space(12.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Njörðr Seas'")
                                        .color(C_LIME)
                                        .font(display_font(if narrow { 30.0 } else { 40.0 })),
                                );
                                ui.label(
                                    RichText::new("CYD SHA-256d miner · USB first")
                                        .color(C_TEXT)
                                        .font(display_font(if narrow { 14.0 } else { 17.0 })),
                                );
                            });
                        });
                        ui.add_space(14.0);
                        let (rate_num, rate_unit) =
                            format_hashrate_parts(self.displayed_khs as f64 * 1000.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(rate_num)
                                    .color(C_TEXT)
                                    .font(display_font(if narrow { 56.0 } else { 68.0 })),
                            );
                            ui.vertical(|ui| {
                                ui.add_space(18.0);
                                ui.label(
                                    RichText::new(rate_unit)
                                        .color(C_LIME)
                                        .font(display_font(22.0)),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        self.sha_mode_label(),
                                        if self.usb_open { "USB linked" } else { "USB idle" }
                                    ))
                                    .color(C_DIM)
                                    .font(mono_ui_font(11.0)),
                                );
                            });
                        });
                        if narrow {
                            ui.add_space(12.0);
                            self.ui_hero_ctas(ui, true);
                        }
                    });

                    if !narrow {
                        ui.add_space(16.0);
                        ui.with_layout(Layout::top_down(Align::Center), |ui| {
                            let (pool_label, pool_color) = self.pool_state();
                            ui.label(
                                RichText::new(format!("Pool · {pool_label}"))
                                    .color(pool_color)
                                    .font(mono_ui_font(12.0)),
                            );
                            ui.add_space(12.0);
                            self.ui_hero_ctas(ui, false);
                        });
                    }
                });
            });
    }

    fn ui_hero_ctas(&mut self, ui: &mut egui::Ui, wrap: bool) {
        let mut row = |ui: &mut egui::Ui| {
            let usb_label = if self.usb_open {
                "Disconnect"
            } else {
                "Connect"
            };
            let w = if wrap { 150.0 } else { 200.0 };
            if cta_button(ui, usb_label, !self.usb_open, w).clicked() {
                if self.update_busy {
                    self.last_error = "Wait for Update board / flash to finish.".into();
                } else if self.usb_open {
                    let _ = self.cmd_tx.send(NetCmd::CloseUsb);
                    self.usb_open = false;
                    self.usb_connect_pending = false;
                    self.mining = false;
                    self.session_started = None;
                    self.connected_workers.clear();
                    self.clear_hash_display();
                    self.push_log(LogKind::Usb, "Disconnect requested".into());
                } else {
                    self.connect_or_add_usb();
                }
            }
            if self.usb_open {
                if wrap {
                } else {
                    ui.add_space(8.0);
                }
                if cta_button(ui, "Add board", true, w).clicked() {
                    if self.update_busy {
                        self.last_error = "Wait for Update board / flash to finish.".into();
                    } else {
                        // Refresh COM list first so a newly plugged CYD is visible.
                        self.refresh_com_ports(false, false);
                        if self.worker_already_linked(&self.com_port)
                            || self.com_port.is_empty()
                        {
                            if self.select_next_unlinked_usb() {
                                self.connect_or_add_usb();
                            } else {
                                // No free COM in the list — try linking any USB + scan.
                                self.link_all_unlinked_usb();
                            }
                        } else {
                            self.connect_or_add_usb();
                        }
                    }
                }
            }
            if wrap {
                // stay in same wrap row
            } else {
                ui.add_space(8.0);
            }
            let mine_label = if self.mining { "Stop mining" } else { "Start mining" };
            if cta_button(ui, mine_label, !self.mining, w).clicked() {
                if self.mining {
                    self.stop_mine();
                } else {
                    self.start_mine();
                }
            }
        };
        if wrap {
            ui.horizontal_wrapped(row);
        } else {
            ui.vertical(row);
        }
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Companion app", |ui| {
            ui.label(
                RichText::new(format!(
                    "Running Companion {} — check/download the latest Windows build.",
                    running_version()
                ))
                .color(C_MUTED)
                .size(13.0),
            );
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                let check_label = if self.app_update_busy {
                    "Working…"
                } else {
                    "Check for app update"
                };
                if soft_button(ui, check_label, 180.0).clicked() && !self.app_update_busy {
                    self.start_app_update_check();
                }
                let update_app_label = if self.app_update_busy {
                    "Updating app…"
                } else {
                    "Update app"
                };
                if soft_button(ui, update_app_label, 140.0).clicked() && !self.app_update_busy {
                    self.start_app_update();
                }
                if soft_button(ui, "Show setup wizard", 150.0).clicked() {
                    self.wizard_step = Some(0);
                    self.tab = Tab::Mine;
                }
                if soft_button(ui, "Add Desktop icon", 150.0).clicked() {
                    match desktop_icon::ensure_desktop_shortcut() {
                        Ok(msg) => {
                            self.last_ok = msg.clone();
                            self.update_status = msg.clone();
                            self.push_log(LogKind::Info, msg);
                        }
                        Err(e) => {
                            self.last_error = e.clone();
                            self.push_log(LogKind::Err, e);
                        }
                    }
                }
            });
            if let Some(info) = &self.app_remote {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&info.detail)
                        .color(if info.newer { C_LIME } else { C_MUTED })
                        .font(mono_ui_font(11.0)),
                );
            }
        });

        ui.add_space(14.0);
        soft_panel(ui, "Phone monitor", |ui| {
            ui.label(
                RichText::new(
                    "Each Companion install has a personal QR. Phones that scan it connect only to this PC — other users’ miners stay private.",
                )
                .color(C_MUTED)
                .size(13.0),
            );
            ui.add_space(10.0);

            let pair = self.monitor_pair_url();
            let web = self.monitor_web_url();
            let host = self.monitor_connect_host();

            ui.horizontal(|ui| {
                // Personal QR
                let qr_size = 168.0;
                if let Some((w, cells)) = qr_modules(&pair) {
                    let (resp, painter) =
                        ui.allocate_painter(Vec2::splat(qr_size), egui::Sense::hover());
                    let rect = resp.rect;
                    painter.rect_filled(rect, Rounding::same(8.0), Color32::WHITE);
                    let cell = (qr_size - 12.0) / w as f32;
                    let origin = rect.min + Vec2::splat(6.0);
                    for y in 0..w {
                        for x in 0..w {
                            if cells[y * w + x] {
                                let r = Rect::from_min_size(
                                    origin + Vec2::new(x as f32 * cell, y as f32 * cell),
                                    Vec2::splat(cell + 0.2),
                                );
                                painter.rect_filled(r, Rounding::ZERO, Color32::BLACK);
                            }
                        }
                    }
                } else {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("QR unavailable — copy link below").color(C_WARN));
                        ui.label(
                            RichText::new(&pair)
                                .color(C_DIM)
                                .font(mono_ui_font(9.0)),
                        );
                    });
                }

                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!("Pair id · {}", self.monitor_install_id))
                            .color(C_LIME)
                            .font(mono_ui_font(12.0)),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!("Connect host · {host}:{MONITOR_PORT}"))
                            .color(C_TEXT)
                            .font(mono_ui_font(11.0)),
                    );
                    if !host.chars().any(|c| c == '.') {
                        ui.label(
                            RichText::new(
                                "Host is not an IP — set Remote host to your LAN IP (ipconfig) so the phone can reach this PC.",
                            )
                            .color(C_WARN)
                            .size(11.0),
                        );
                    }
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("API · {}", self.monitor_addr))
                            .color(C_DIM)
                            .font(mono_ui_font(10.0)),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Scan with the Njörðr Seas' Monitor phone app (iPhone / Android).")
                            .color(C_MUTED)
                            .size(12.0),
                    );
                    ui.add_space(8.0);
                    if ui
                        .add(egui::Button::new(
                            RichText::new("Copy pair link").color(C_BG).size(13.0),
                        ))
                        .clicked()
                    {
                        ui.output_mut(|o| o.copied_text = pair.clone());
                        self.push_log(LogKind::Info, "Copied personal phone pair link".into());
                    }
                    if ui
                        .add(egui::Button::new(
                            RichText::new("Copy web link").color(C_BG).size(13.0),
                        ))
                        .clicked()
                    {
                        ui.output_mut(|o| o.copied_text = web.clone());
                        self.push_log(LogKind::Info, "Copied personal web monitor link".into());
                    }
                    ui.add_space(6.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Regenerate QR (revoke phones)")
                                    .color(C_WARN)
                                    .size(12.0),
                            )
                            .fill(Color32::from_rgb(40, 28, 12)),
                        )
                        .on_hover_text("Creates a new personal token. Old QR codes stop working.")
                        .clicked()
                    {
                        self.regenerate_monitor_pairing();
                    }
                });
            });

            ui.add_space(12.0);
            ui.label(
                RichText::new("Remote host (optional — DDNS / Tailscale / public IP for travel)")
                    .color(C_DIM)
                    .size(11.0),
            );
            ui.add(
                TextEdit::singleline(&mut self.monitor_public_host)
                    .desired_width(f32::INFINITY)
                    .hint_text("leave empty to use this PC’s LAN IP in the QR")
                    .font(mono_ui_font(12.0)),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Same Wi‑Fi works with LAN IP. Around the globe: set your reachable host above and forward TCP 19285 (or use Tailscale/VPN). Token still binds the phone to only this Companion.",
                )
                .color(C_DIM)
                .font(mono_ui_font(10.0)),
            );
        });

        ui.add_space(14.0);
        soft_panel(ui, "Board firmware", |ui| {
            ui.label(
                RichText::new(
                    "Fetch the latest board image, then Push update on a linked board (auto-reset). Blank boards still use Update board + BOOT Ready.",
                )
                .color(C_MUTED)
                .size(13.0),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("What’s left for the best path on this board (not a different alg)")
                    .color(C_LIME)
                    .font(mono_ui_font(12.0)),
            );
            ui.label(
                RichText::new(
                    "· Bench boards (D0) → lock Full HW  ·  Clock 240 MHz  ·  More CYDs for more rate
· Algorithm stays Bitcoin SHA-256d only
· Do NOT raise board voltage — CYD is fixed ~3.3V; overvolting can kill flash/USB/ESP
· SAFETY: blank / BOOT — hold BOOT, Update board, click Ready while BOOT held",
                )
                .color(C_MUTED)
                .font(mono_ui_font(11.0)),
            );
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                let bench_label = if self.bench_busy {
                    "Benching…"
                } else {
                    "Bench boards (D0)"
                };
                if soft_button(ui, bench_label, 168.0).clicked() && !self.bench_busy {
                    self.request_bench();
                }
                let fetch_label = if self.fetch_busy {
                    "Fetching FW…"
                } else {
                    "Fetch latest FW"
                };
                if soft_button(ui, fetch_label, 150.0).clicked() && !self.fetch_busy {
                    self.start_firmware_fetch();
                }
                let update_label = if self.update_busy {
                    "Updating…"
                } else if self.board_supports_live_push(&self.com_port) {
                    "Push update"
                } else {
                    "Update board"
                };
                if soft_button(ui, update_label, 140.0).clicked() && !self.update_busy {
                    self.request_board_update();
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
                        .color(if self.update_busy || self.fetch_busy || self.app_update_busy {
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
                        "Board image · {} · {} KB",
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
            ui.add_space(8.0);
            ui.label(
                RichText::new("Tip: hold BOOT, tap RESET, keep BOOT held, then click Ready if Update board asks.")
                    .color(C_DIM)
                    .size(12.0),
            );
        });

        ui.add_space(14.0);
        soft_panel(ui, "Preferences", |ui| {
            ui.checkbox(&mut self.auto_connect, "Auto-connect USB on launch");
        });

        ui.add_space(14.0);
        self.ui_api_feeds(ui);
    }

    fn ui_api_feeds(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "API feeds", |ui| {
            ui.label(
                RichText::new(
                    "Pull JSON, text, CSV, or files from one or more HTTPS sources. File saves land in ApiDownloads\\ next to the app.",
                )
                .color(C_MUTED)
                .size(13.0),
            );
            ui.add_space(10.0);
            labeled_edit(ui, "Name", &mut self.api_draft_name, "Weather · Markets · Firmware mirror");
            labeled_edit(
                ui,
                "Primary URL",
                &mut self.api_draft_url,
                "https://api.example.com/v1/info.json",
            );
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Type")
                        .color(C_MUTED)
                        .font(mono_ui_font(12.0)),
                );
                egui::ComboBox::from_id_source("api_draft_kind")
                    .width(120.0)
                    .selected_text(self.api_draft_kind.label())
                    .show_ui(ui, |ui| {
                        for k in ApiContentKind::all() {
                            ui.selectable_value(&mut self.api_draft_kind, *k, k.label());
                        }
                    });
                ui.add_space(12.0);
                ui.label(
                    RichText::new(match self.api_draft_kind {
                        ApiContentKind::File => "Save as (optional)",
                        ApiContentKind::Csv => "CSV column (optional)",
                        _ => "JSON path (optional)",
                    })
                    .color(C_MUTED)
                    .font(mono_ui_font(12.0)),
                );
            });
            labeled_edit(
                ui,
                match self.api_draft_kind {
                    ApiContentKind::File => "Save as",
                    ApiContentKind::Csv => "Column",
                    _ => "Path",
                },
                &mut self.api_draft_path,
                match self.api_draft_kind {
                    ApiContentKind::File => "merged.bin",
                    ApiContentKind::Csv => "price",
                    _ => "data.price",
                },
            );
            labeled_edit(
                ui,
                "Auth (optional)",
                &mut self.api_draft_auth,
                "Bearer …  or  X-Api-Key: …",
            );

            ui.add_space(8.0);
            ui.label(
                RichText::new("Extra sources (optional — other file types / URLs on the same feed)")
                    .color(C_DIM)
                    .font(mono_ui_font(11.0)),
            );
            labeled_edit(
                ui,
                "Extra URL",
                &mut self.api_draft_extra_url,
                "https://cdn.example.com/data.csv",
            );
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Extra type")
                        .color(C_MUTED)
                        .font(mono_ui_font(12.0)),
                );
                egui::ComboBox::from_id_source("api_draft_extra_kind")
                    .width(120.0)
                    .selected_text(self.api_draft_extra_kind.label())
                    .show_ui(ui, |ui| {
                        for k in ApiContentKind::all() {
                            ui.selectable_value(&mut self.api_draft_extra_kind, *k, k.label());
                        }
                    });
            });
            labeled_edit(
                ui,
                "Extra path / save-as",
                &mut self.api_draft_extra_path,
                "optional",
            );
            ui.horizontal(|ui| {
                if soft_button(ui, "Add extra source", 150.0).clicked() {
                    self.add_api_draft_extra_source();
                }
                if soft_button(ui, "Add API feed", 140.0).clicked() {
                    self.add_api_feed_from_draft();
                }
            });
            if !self.api_draft_extras.is_empty() {
                ui.add_space(6.0);
                for (i, src) in self.api_draft_extras.iter().enumerate() {
                    ui.label(
                        RichText::new(format!(
                            "· extra {}: {} · {}",
                            i + 1,
                            src.kind.label(),
                            src.url
                        ))
                        .color(C_LIME)
                        .font(mono_ui_font(10.0)),
                    );
                }
            }

            if self.api_feeds.is_empty() {
                ui.add_space(10.0);
                ui.label(
                    RichText::new("No API feeds yet — add a URL above to pull live data or files.")
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
                let src_n = 1 + feed.sources.len();
                Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(3, 12, 26, 170))
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
                            ui.label(
                                RichText::new(format!(
                                    "· {} · {src_n} source{}",
                                    feed.kind.label(),
                                    if src_n == 1 { "" } else { "s" }
                                ))
                                .color(C_DIM)
                                .font(mono_ui_font(10.0)),
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
                        for (i, src) in feed.sources.iter().enumerate() {
                            ui.label(
                                RichText::new(format!(
                                    "+{} {} · {}",
                                    i + 1,
                                    src.kind.label(),
                                    src.url
                                ))
                                .color(C_DIM)
                                .font(mono_ui_font(10.0)),
                            );
                        }
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
                        if !feed.last_saved.is_empty() {
                            ui.label(
                                RichText::new(format!("saved {}", feed.last_saved))
                                    .color(C_WARN)
                                    .font(mono_ui_font(10.0)),
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

    #[allow(dead_code)]
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
        soft_panel(ui, "Board & pool", |ui| {
            ui.label(
                RichText::new("USB-C")
                    .color(C_LIME)
                    .font(mono_ui_font(12.0)),
            );
            // Wrap so Refresh stays clickable in the half-width Mine column
            // (fixed 320px combo + buttons used to clip past the column edge).
            ui.horizontal_wrapped(|ui| {
                let reserve = if self.usb_open { 340.0 } else { 110.0 };
                let combo_w = (ui.available_width() - reserve).clamp(140.0, 320.0);
                let com_label = if self.com_port.is_empty() {
                    "Select port".to_string()
                } else {
                    self.ports
                        .iter()
                        .find(|p| port_names_match(&p.name, &self.com_port))
                        .map(|p| {
                            if self.worker_already_linked(&p.name) {
                                format!("{} · linked", p.label)
                            } else {
                                p.label.clone()
                            }
                        })
                        .unwrap_or_else(|| self.com_port.clone())
                };
                egui::ComboBox::from_id_source("com")
                    .width(combo_w)
                    .selected_text(RichText::new(com_label).color(C_TEXT).size(13.0))
                    .show_ui(ui, |ui| {
                        if self.ports.is_empty() {
                            ui.label(
                                RichText::new("No serial ports reported by the OS")
                                    .color(C_WARN)
                                    .size(12.0),
                            );
                        }
                        for p in self.ports.clone() {
                            let label = if self.worker_already_linked(&p.name) {
                                format!("{} · linked", p.label)
                            } else {
                                p.label.clone()
                            };
                            ui.selectable_value(&mut self.com_port, p.name.clone(), label);
                        }
                    });
                if soft_button(ui, "Refresh", 98.0).clicked() {
                    // List only — never OpenUsb / auto-reconnect.
                    self.refresh_com_ports(false, false);
                }
                // Always offer Add board once at least one board is linked.
                if self.usb_open {
                    let selected_linked = self.worker_already_linked(&self.com_port);
                    let add_label = if selected_linked {
                        "Add other COM"
                    } else {
                        "Add board"
                    };
                    if soft_button(ui, add_label, 120.0).clicked() {
                        self.refresh_com_ports(false, false);
                        if selected_linked || self.worker_already_linked(&self.com_port) {
                            if self.select_next_unlinked_usb() {
                                self.connect_or_add_usb();
                            } else {
                                self.link_all_unlinked_usb();
                            }
                        } else if !self.com_port.is_empty() {
                            self.connect_or_add_usb();
                        } else {
                            self.link_all_unlinked_usb();
                        }
                    }
                    if soft_button(ui, "Link all USB", 110.0).clicked() {
                        self.refresh_com_ports(false, false);
                        self.link_all_unlinked_usb();
                    }
                }
            });
            if self.ports.is_empty() {
                ui.label(
                    RichText::new(
                        "No COM ports yet — plug the CYD USB-C data cable, then Refresh.",
                    )
                    .color(C_WARN)
                    .size(12.0),
                );
            } else if self
                .ports
                .iter()
                .find(|p| port_names_match(&p.name, &self.com_port))
                .map(port_choice_is_pci)
                .unwrap_or(false)
            {
                ui.label(
                    RichText::new(
                        "Selected port is motherboard PCI — pick the USB CH340/CP210x COM for the CYD.",
                    )
                    .color(C_WARN)
                    .size(12.0),
                );
            } else if self.usb_open && count_usb_uart_ports(&self.ports) <= 1 {
                ui.label(
                    RichText::new(
                        "Windows only sees 1 USB board COM (plus PCI junk like COM1). Plug the 2nd CYD with a data USB-C cable into another port, open Device Manager → Ports (COM & LPT), confirm a new COMx appears, then Refresh.",
                    )
                    .color(C_WARN)
                    .size(12.0),
                );
            }
            ui.add_space(12.0);
            self.ui_worker_discovery(ui);
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
            if self.edit_stratum.contains(":3335") && self.edit_stratum.contains("hmpool") {
                ui.label(
                    RichText::new(
                        "HM Pool :3335 is CPU/GPU (diff≈128) — CYD boards get mass rejects. Use :3337.",
                    )
                    .color(C_WARN)
                    .size(11.0),
                );
            }
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

    fn ui_worker_discovery(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("CYD WORKERS")
                .color(C_LIME)
                .font(mono_ui_font(12.0)),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            let scan_label = if self.worker_scan_busy {
                "Scanning…"
            } else {
                "Find CYD workers"
            };
            if soft_button(ui, scan_label, 160.0).clicked() && !self.worker_scan_busy {
                if self.update_busy {
                    self.last_error = "Wait for Update board / flash to finish.".into();
                } else {
                    self.worker_scan_busy = true;
                    self.push_log(LogKind::Usb, "Scanning USB + LAN for CYD workers…".into());
                    let _ = self.cmd_tx.send(NetCmd::ScanWorkers);
                }
            }
            ui.label(
                RichText::new(format!(
                    "{} linked · {} found",
                    self.connected_workers.len(),
                    self.discovered_workers.len()
                ))
                .color(C_DIM)
                .font(mono_ui_font(11.0)),
            );
        });
        if self.connected_workers.len() <= 1 {
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Tip: plug the 2nd CYD on its own USB cable → Refresh → Add other COM (or Link all USB). SoftAP Wi‑Fi is one PC↔board link — use USB for multi-board.",
                )
                .color(C_MUTED)
                .size(11.0),
            );
        }

        if !self.connected_workers.is_empty() {
            ui.add_space(8.0);
            for w in self.connected_workers.clone() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new({
                            let mac = if w.mac.is_empty() {
                                "mac?".to_string()
                            } else {
                                w.mac.clone()
                            };
                            format!(
                                "● {} · {} · {} · {}",
                                mac,
                                w.endpoint,
                                if w.fw.is_empty() { "fw?" } else { &w.fw },
                                format_hashrate(w.hashrate_hs)
                            )
                        })
                        .color(C_LIME)
                        .font(mono_ui_font(11.0)),
                    );
                    if soft_button(ui, "Drop", 64.0).clicked() {
                        let _ = self
                            .cmd_tx
                            .send(NetCmd::DisconnectWorker(w.endpoint.clone()));
                        self.push_log(LogKind::Usb, format!("Drop worker {}", w.endpoint));
                    }
                });
            }
        }

        let usb_found: Vec<_> = self
            .discovered_workers
            .iter()
            .filter(|w| w.kind == WorkerKind::Usb || w.kind == WorkerKind::Bluetooth)
            .cloned()
            .collect();
        let wifi_found: Vec<_> = self
            .discovered_workers
            .iter()
            .filter(|w| w.kind == WorkerKind::Wifi)
            .cloned()
            .collect();
        let lan_found: Vec<_> = self
            .discovered_workers
            .iter()
            .filter(|w| w.kind == WorkerKind::Lan)
            .cloned()
            .collect();

        if !usb_found.is_empty() {
            ui.add_space(8.0);
            ui.label(
                RichText::new("USB / Bluetooth CYD boards")
                    .color(C_MUTED)
                    .size(12.0),
            );
            for w in usb_found {
                let already = self.worker_already_linked(&w.endpoint);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new({
                            let mac = if w.mac.is_empty() { "mac?" } else { &w.mac };
                            let tag = if w.kind == WorkerKind::Bluetooth {
                                "BT"
                            } else {
                                "USB"
                            };
                            format!("{tag} · {mac} · {} · {}", w.endpoint, w.detail)
                        })
                        .color(C_TEXT)
                        .font(mono_ui_font(11.0)),
                    );
                    if already {
                        ui.label(RichText::new("linked").color(C_LIME).size(11.0));
                    } else if soft_button(ui, "Connect", 88.0).clicked() {
                        self.com_port = w.endpoint.clone();
                        let _ = self
                            .cmd_tx
                            .send(NetCmd::ConnectWorker(w.endpoint.clone()));
                        self.push_log(
                            LogKind::Usb,
                            format!("Connecting worker {}", w.endpoint),
                        );
                    }
                });
            }
        }

        if !wifi_found.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Wi‑Fi CYD boards").color(C_MUTED).size(12.0));
            for w in wifi_found {
                let already = self.worker_already_linked(&w.endpoint)
                    || self.worker_mac_already_linked(&w.mac);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new({
                            let mac = if w.mac.is_empty() { "mac?" } else { &w.mac };
                            format!("WiFi · {mac} · {} · {}", w.endpoint, w.detail)
                        })
                        .color(C_TEXT)
                        .font(mono_ui_font(11.0)),
                    );
                    if already {
                        ui.label(RichText::new("linked").color(C_LIME).size(11.0));
                    } else if soft_button(ui, "Connect", 88.0).clicked() {
                        let _ = self.cmd_tx.send(NetCmd::ConnectWifi(w.endpoint.clone()));
                        self.push_log(
                            LogKind::Usb,
                            format!("Connecting Wi‑Fi worker {}", w.endpoint),
                        );
                    }
                });
            }
        }

        if !lan_found.is_empty() {
            ui.add_space(8.0);
            ui.label(
                RichText::new("LAN Companion peers")
                    .color(C_MUTED)
                    .size(12.0),
            );
            for w in lan_found.iter().take(8) {
                ui.label(
                    RichText::new(format!("{} · {}", w.host, w.detail))
                        .color(C_DIM)
                        .font(mono_ui_font(10.0)),
                );
            }
            ui.label(
                RichText::new("LAN peers advertise local USB CYDs — connect those boards on that PC.")
                    .color(C_DIM)
                    .size(11.0),
            );
        }
    }

    fn ui_telemetry_rail(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Board telemetry", |ui| {
            let authed = self.stratum_live.authorized;
            // Never surface connect-handshake rejects — stay at 0/0 until authorize.
            let (acc, rej) = if !authed {
                (0, 0)
            } else if self.session_started.is_some() {
                (self.session_accepted, self.session_rejected)
            } else {
                // Authorized but session not stamped — still show live pool counters.
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
                // Use smoothed display rate so soft-fails don't blink the chip.
                let rate_hs = if self.mining && self.displayed_khs > 0.5 {
                    self.displayed_khs as f64 * 1000.0
                } else {
                    self.status.hashrate_hs
                };
                mini_stat(ui, "Rate", &format_hashrate(rate_hs));
                mini_stat(ui, "Total Hash", &format_hash_count(self.status.hashes));
                let sha = self.sha_path_display();
                mini_stat(ui, "SHA", &sha);
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
            ui.label(
                RichText::new(
                    "SHA path · Full HW = silicon · HW+ = midstate · HW/SW = hybrid",
                )
                .color(C_DIM)
                .font(mono_ui_font(10.0)),
            );
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
                    .fill(Color32::from_rgba_unmultiplied(3, 12, 26, 170))
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
        ui.add_space(12.0);
        self.ui_stratum_panel(ui);
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
                .fill(Color32::from_rgba_unmultiplied(3, 12, 24, 210))
                .rounding(Rounding::same(16.0))
                .stroke(Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_unmultiplied(126, 220, 255, 22),
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

    #[allow(dead_code)]
    fn ui_debug(&mut self, ui: &mut egui::Ui) {
        if self.update_busy {
            soft_panel(ui, "Board update", |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    ui.add(egui::Spinner::new().size(48.0).color(C_LIME));
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new("Updating board…")
                            .color(C_LIME)
                            .font(display_font(24.0)),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(&self.update_status)
                            .color(C_MUTED)
                            .font(mono_ui_font(12.0)),
                    );
                    ui.add_space(18.0);
                    ui.label(
                        RichText::new("Terminal is paused while flash runs.")
                            .color(C_DIM)
                            .size(12.0),
                    );
                    ui.add_space(12.0);
                });
            });
            return;
        }
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
                let bench_label = if self.bench_busy { "Benching…" } else { "Bench D0" };
                if soft_button(ui, bench_label, 96.0).clicked() && !self.bench_busy {
                    self.request_bench();
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
                .fill(Color32::from_rgba_unmultiplied(3, 12, 26, 230))
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

    /// Top chrome: brand + tabs + status. Wraps on narrow windows so tabs never overlap.
    fn ui_app_nav(&mut self, ui: &mut egui::Ui) {
        let wide = ui.available_width() >= 760.0;
        let (pool, pool_color) = self.pool_state();
        let usb_label = if self.usb_open { "USB LINKED" } else { "USB IDLE" };
        let usb_color = if self.usb_open { C_LIME } else { C_MUTED };

        if wide {
            ui.horizontal(|ui| {
                brand_logo(ui, 40.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Njörðr Seas'")
                            .color(C_LIME)
                            .font(display_font(18.0)),
                    );
                    ui.label(
                        RichText::new("CYD miner")
                            .color(C_MUTED)
                            .font(mono_ui_font(11.0)),
                    );
                    if !self.board_mac.is_empty() {
                        ui.label(
                            RichText::new(format!("board {}", self.board_mac))
                                .color(C_DIM)
                                .font(mono_ui_font(10.0)),
                        );
                    }
                });
                ui.add_space(18.0);
                if nav_button(ui, "Mine", self.tab == Tab::Mine).clicked() {
                    self.tab = Tab::Mine;
                }
                ui.add_space(6.0);
                if nav_button(ui, "Settings", self.tab == Tab::Settings).clicked() {
                    self.tab = Tab::Settings;
                }
                ui.add_space(10.0);
                if !self.fw_label.is_empty() {
                    ui.label(
                        RichText::new(format!("fw {}", self.fw_label))
                            .color(C_DIM)
                            .font(mono_ui_font(11.0)),
                    );
                }
                let rest = ui.available_width();
                if rest > 8.0 {
                    ui.allocate_ui_with_layout(
                        Vec2::new(rest, 40.0),
                        Layout::right_to_left(Align::Center),
                        |ui| {
                            status_chip(ui, pool, pool_color);
                            ui.add_space(6.0);
                            status_chip(ui, usb_label, usb_color);
                        },
                    );
                }
            });
        } else {
            // Compact: brand + chips on first row, tabs on second — never overlap.
            ui.horizontal(|ui| {
                brand_logo(ui, 32.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Njörðr Seas'")
                            .color(C_LIME)
                            .font(display_font(15.0)),
                    );
                    if !self.fw_label.is_empty() {
                        ui.label(
                            RichText::new(format!("fw {}", trunc(&self.fw_label, 18)))
                                .color(C_DIM)
                                .font(mono_ui_font(10.0)),
                        );
                    }
                });
                let rest = ui.available_width();
                if rest > 8.0 {
                    ui.allocate_ui_with_layout(
                        Vec2::new(rest, 36.0),
                        Layout::right_to_left(Align::Center),
                        |ui| {
                            status_chip(ui, pool, pool_color);
                            ui.add_space(4.0);
                            status_chip(ui, usb_label, usb_color);
                        },
                    );
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if nav_button(ui, "Mine", self.tab == Tab::Mine).clicked() {
                    self.tab = Tab::Mine;
                }
                if nav_button(ui, "Settings", self.tab == Tab::Settings).clicked() {
                    self.tab = Tab::Settings;
                }
            });
        }
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
                    let n = p.len();
                    self.ports = p;
                    self.apply_best_com_port(false);
                    self.push_log(
                        LogKind::Usb,
                        format!(
                            "Serial ports: {n} reported · selected {}",
                            if self.com_port.is_empty() {
                                "—".into()
                            } else {
                                self.com_port.clone()
                            }
                        ),
                    );
                    self.maybe_auto_connect_usb();
                }
                NetMsg::Action(Ok(s)) => {
                    self.last_ok = s.clone();
                    let low = s.to_lowercase();
                    if low.contains("usb open") || low.contains("worker linked") {
                        self.usb_open = true;
                        self.usb_connect_pending = false;
                        // Keep the COM that just linked selected — do NOT jump to PCI COM1.
                        if self.post_flash_verify.is_some() && low.contains("usb open") {
                            self.absorb_flash_progress_line(
                                "USB linked after flash — reading board config…",
                            );
                        }
                    }
                    if low.contains("closed") {
                        self.usb_open = false;
                        self.usb_connect_pending = false;
                        self.mining = false;
                    }
                    if low.contains("already linked") || low.contains("skip wi") {
                        self.usb_connect_pending = false;
                    }
                    if low.contains("usb ← job") || low.contains("usb <- job") {
                        self.last_job_flow_at = Instant::now();
                    }
                    // Manual Bench completion only (not the "Bench running…" ack).
                    if self.bench_busy
                        && (low.contains(" → ")
                            || low.contains(" -> ")
                            || low.contains("bench done")
                            || low.contains("no boards"))
                    {
                        self.bench_busy = false;
                    }
                    let kind = if low.contains("job") || low.contains("share") || low.contains("pool")
                    {
                        LogKind::Stratum
                    } else if low.contains("usb")
                        || low.contains("cmp")
                        || low.contains("board")
                        || low.contains("bench")
                    {
                        LogKind::Usb
                    } else {
                        LogKind::Info
                    };
                    self.push_log(kind, s);
                }
                NetMsg::Action(Err(e)) => {
                    self.usb_connect_pending = false;
                    if self.bench_busy {
                        self.bench_busy = false;
                    }
                    if self.post_flash_verify.is_some() {
                        self.retry_post_flash_verify(&e);
                    } else {
                        self.last_error = e.clone();
                        self.push_log(LogKind::Err, e);
                    }
                }
                NetMsg::Status(Ok(s)) => {
                    self.absorb_status(s);
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
                    if !c.mac.is_empty() {
                        self.board_mac = normalize_mac(&c.mac);
                    }
                    if c.cpu_mhz == 80 || c.cpu_mhz == 160 || c.cpu_mhz == 240 {
                        self.target_mhz = c.cpu_mhz;
                    }
                    self.push_log(
                        LogKind::Usb,
                        format!(
                            "config fw={} mode={} mac={}",
                            c.fw,
                            c.mode,
                            if c.mac.is_empty() { "—" } else { &c.mac }
                        ),
                    );
                    if self.post_flash_verify.is_some() {
                        self.on_post_flash_config(&c.fw);
                    }
                }
                NetMsg::Config(Err(e)) => {
                    if self.post_flash_verify.is_some() {
                        self.retry_post_flash_verify(&format!("config: {e}"));
                    } else {
                        self.push_log(LogKind::Warn, format!("config: {e}"));
                    }
                }
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
                    // During board update, keep the UI on the loading overlay — don't
                    // flood Debug/Terminal with flash chatter.
                    if self.update_busy {
                        self.update_status = trunc(&line, 120);
                    } else {
                        self.term_out.push_back(line.clone());
                        while self.term_out.len() > 300 {
                            self.term_out.pop_front();
                        }
                        self.push_log(LogKind::Usb, format!("RX {line}"));
                    }
                }
                NetMsg::FlashProgress(line) => {
                    self.absorb_flash_progress_line(&line);
                }
                NetMsg::FlashDone { result, reopen } => {
                    match result {
                        Ok(s) => {
                            self.last_ok = s.clone();
                            self.last_error.clear();
                            self.push_log(LogKind::Usb, s.clone());
                            let port = reopen
                                .filter(|p| !p.trim().is_empty())
                                .unwrap_or_else(|| self.com_port.clone());
                            if port.trim().is_empty() {
                                self.clear_flash_overlay();
                                self.update_status = s;
                            } else {
                                // Keep spinner: reconnect, then verify via cmp ping/config.
                                self.begin_post_flash_verify(port);
                            }
                        }
                        Err(e) => {
                            self.clear_flash_overlay();
                            self.update_status = e.clone();
                            self.last_error = e.clone();
                            self.push_log(LogKind::Err, e);
                        }
                    }
                }
                NetMsg::Share(ev) => {
                    self.last_share_flow_at = Instant::now();
                    if self.session_started.is_none() && self.mining {
                        self.session_started = Some(Instant::now());
                    }
                    if ev.accepted {
                        self.session_accepted = self.session_accepted.saturating_add(1);
                        self.accepted = self.accepted.saturating_add(1);
                    } else {
                        self.session_rejected = self.session_rejected.saturating_add(1);
                        self.rejected = self.rejected.saturating_add(1);
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
                                "Board FW fetched {} ({} KB)",
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
                NetMsg::AppUpdate(result) => {
                    let user_initiated = self.app_update_busy;
                    self.app_update_busy = false;
                    match result {
                        Ok(info) => {
                            self.update_status = info.detail.clone();
                            if info.newer
                                && info.detail.to_ascii_lowercase().contains("restarting")
                            {
                                self.last_ok = info.detail.clone();
                                self.push_log(LogKind::Info, info.detail.clone());
                                self.app_remote = Some(info);
                                // Give the updater bat a moment, then release the .exe lock.
                                thread::sleep(Duration::from_millis(400));
                                std::process::exit(0);
                            }
                            if user_initiated || info.newer {
                                self.last_ok = info.detail.clone();
                                self.push_log(LogKind::Info, info.detail.clone());
                            }
                            self.app_remote = Some(info);
                        }
                        Err(e) => {
                            if user_initiated {
                                self.update_status = e.clone();
                                self.last_error = e.clone();
                                self.push_log(LogKind::Err, e);
                            } else {
                                self.push_log(LogKind::Warn, format!("App update check: {e}"));
                            }
                        }
                    }
                }
                NetMsg::ApiFeedResult(outcome) => {
                    self.apply_api_pull(outcome);
                }
                NetMsg::WorkersFound(found) => {
                    self.worker_scan_busy = false;
                    let mut auto_usb: Vec<String> = Vec::new();
                    let mut auto_wifi: Vec<String> = Vec::new();
                    for w in found {
                        if !self.worker_already_linked(&w.endpoint)
                            && !self.worker_mac_already_linked(&w.mac)
                        {
                            match w.kind {
                                WorkerKind::Usb | WorkerKind::Bluetooth => {
                                    auto_usb.push(w.endpoint.clone());
                                }
                                WorkerKind::Wifi => auto_wifi.push(w.endpoint.clone()),
                                WorkerKind::Lan => {}
                            }
                        }
                        self.merge_discovered(w);
                    }
                    // Wi‑Fi beacons are owned by the UI UDP listener — fold recent
                    // SoftAP/STA finds into auto-link (scan thread no longer rebinds :19284).
                    for w in self.discovered_workers.clone() {
                        if w.kind != WorkerKind::Wifi {
                            continue;
                        }
                        if self.worker_already_linked(&w.endpoint)
                            || self.worker_mac_already_linked(&w.mac)
                        {
                            continue;
                        }
                        if !auto_wifi.iter().any(|e| e == &w.endpoint) {
                            auto_wifi.push(w.endpoint.clone());
                        }
                    }
                    // Always surface already-linked boards in the found list too.
                    for live in self.connected_workers.clone() {
                        let kind = if live.endpoint.contains(':') && !live.endpoint.starts_with("COM")
                        {
                            WorkerKind::Wifi
                        } else {
                            WorkerKind::Usb
                        };
                        self.merge_discovered(DiscoveredWorker {
                            id: {
                                let mid = transport_mac_id(kind, &live.mac);
                                if mid.is_empty() {
                                    format!(
                                        "{}:{}",
                                        if kind == WorkerKind::Wifi { "wifi" } else { "usb" },
                                        live.endpoint
                                    )
                                } else {
                                    mid
                                }
                            },
                            kind,
                            endpoint: live.endpoint.clone(),
                            mac: live.mac.clone(),
                            fw: live.fw.clone(),
                            detail: format!("linked · {}", format_hashrate(live.hashrate_hs)),
                            host: "local".into(),
                            last_seen_ms: 0,
                        });
                    }
                    for endpoint in auto_usb {
                        self.push_log(
                            LogKind::Usb,
                            format!("Auto-linking serial CYD {endpoint}"),
                        );
                        let _ = self.cmd_tx.send(NetCmd::ConnectWorker(endpoint));
                    }
                    // Prefer USB for multi-board farms — only auto-link Wi‑Fi when that
                    // MAC is not already on USB (SoftAP is one-at-a-time to the PC).
                    for endpoint in auto_wifi {
                        if let Some(w) = self
                            .discovered_workers
                            .iter()
                            .find(|d| d.endpoint == endpoint)
                        {
                            if self.worker_mac_already_linked(&w.mac) {
                                continue;
                            }
                        }
                        self.push_log(
                            LogKind::Usb,
                            format!("Auto-linking Wi‑Fi CYD {endpoint}"),
                        );
                        let _ = self.cmd_tx.send(NetCmd::ConnectWifi(endpoint));
                    }
                    self.push_log(
                        LogKind::Usb,
                        format!(
                            "Worker scan done · {} entries · {} linked",
                            self.discovered_workers.len(),
                            self.connected_workers.len()
                        ),
                    );
                    self.select_next_unlinked_usb();
                }
                NetMsg::WorkersLive(live) => {
                    let prev_n = self.connected_workers.len();
                    self.connected_workers = live;
                    let was_open = self.usb_open;
                    self.usb_open = !self.connected_workers.is_empty();
                    if self.usb_open {
                        self.usb_connect_pending = false;
                    }
                    if !self.usb_open {
                        if was_open {
                            self.mining = false;
                            self.session_started = None;
                        }
                        self.clear_hash_display();
                    } else {
                        // After a new board links, point the combo at the next free USB.
                        if self.connected_workers.len() > prev_n {
                            let _ = self.select_next_unlinked_usb();
                        }
                        // Keep user's COM pick for Add board even when that COM is not
                        // linked yet. Only snap selection when empty or the COM vanished.
                        let selected_still_listed = self
                            .ports
                            .iter()
                            .any(|p| port_names_match(&p.name, &self.com_port));
                        let selected_live_idx = self
                            .connected_workers
                            .iter()
                            .position(|c| port_names_match(&c.endpoint, &self.com_port));
                        if self.com_port.is_empty()
                            || (!selected_still_listed && selected_live_idx.is_none())
                        {
                            if let Some(b) = self.connected_workers.first() {
                                self.com_port = b.endpoint.clone();
                            }
                        }
                        let live_idx = selected_live_idx.or_else(|| {
                            if self.connected_workers.is_empty() {
                                None
                            } else {
                                Some(0)
                            }
                        });
                        if let Some(i) = live_idx {
                            if let Some(b) = self.connected_workers.get(i) {
                                if !b.fw.is_empty() {
                                    self.fw_label = b.fw.clone();
                                }
                                if !b.mac.is_empty() {
                                    self.board_mac = b.mac.clone();
                                }
                            }
                        }
                    }
                }
            }
        }

        if self.usb_open && self.last_poll.elapsed() > Duration::from_millis(1_100) {
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
        // USB serial enum often finishes after the first ListPorts — re-scan briefly.
        if self.port_rescans_done < 2 {
            let due = if self.port_rescans_done == 0 {
                Duration::from_millis(900)
            } else {
                Duration::from_millis(2_400)
            };
            if self.boot_at.elapsed() >= due {
                self.port_rescans_done = self.port_rescans_done.saturating_add(1);
                // UI-thread enum — same path as Refresh list scan; auto-connect only here.
                let force = !self.usb_open && prefer_cyd_port(&self.ports).is_none();
                self.refresh_com_ports(force, true);
            }
        }
        // LAN peer discovery / advertise local CYD USB fleet.
        for peer in self.lan.poll_peers() {
            self.merge_discovered(peer);
        }
        for board in self.board_wifi.poll_boards() {
            self.merge_discovered(board);
        }
        let board_ads: Vec<(String, String, String)> = self
            .connected_workers
            .iter()
            .map(|w| (w.endpoint.clone(), w.fw.clone(), w.mac.clone()))
            .collect();
        let host = hostname_fallback();
        self.lan.maybe_beacon(&board_ads, &host);
        if self.update_busy || self.fetch_busy || self.app_update_busy || self.bench_busy {
            ctx.request_repaint();
        }
        // UI watchdog: flash (~160s) + verify. Unlock if FlashDone/verify never finishes.
        if self.update_busy {
            let flash_cap = Duration::from_secs(400);
            let verify_overdue = self
                .post_flash_verify
                .as_ref()
                .map(|v| Instant::now() >= v.deadline)
                .unwrap_or(false);
            let flash_overdue = self
                .update_busy_since
                .map(|since| since.elapsed() > flash_cap)
                .unwrap_or(false);
            if verify_overdue {
                self.fail_post_flash_verify(
                    "Flash wrote OK but verify timed out (board never answered)."
                        .into(),
                );
            } else if flash_overdue && self.post_flash_verify.is_none() {
                self.clear_flash_overlay();
                let msg = "Board update timed out — hold BOOT, tap RESET, click Ready, then Update board again."
                    .to_string();
                self.update_status = msg.clone();
                self.last_error = msg.clone();
                self.push_log(LogKind::Err, msg);
            }
        }
        if let Some((when, port)) = self.pending_post_flash_reconnect.clone() {
            if Instant::now() >= when {
                self.pending_post_flash_reconnect = None;
                // Keep overlay up while verifying; only drop busy after verify finishes.
                self.update_busy = true;
                self.com_port = port.clone();
                let verifying = self.post_flash_verify.is_some();
                if verifying {
                    self.flash_phase = "Verifying board".into();
                    self.flash_progress = self.flash_progress.max(0.92);
                }
                if self.usb_open {
                    // Already linked — do not bounce COM; wait for config/status.
                    self.update_status = if verifying {
                        format!("Verifying firmware on {port} (USB already open)…")
                    } else {
                        format!("USB already open on {port}")
                    };
                    self.push_log(LogKind::Usb, self.update_status.clone());
                } else {
                    self.update_status = if verifying {
                        format!("Verifying firmware on {port}…")
                    } else {
                        format!("Opening {port} after flash…")
                    };
                    self.push_log(
                        LogKind::Usb,
                        if verifying {
                            format!("Post-flash verify: open {port} (no disconnect)")
                        } else {
                            format!("Post-flash: open {port}")
                        },
                    );
                    let _ = self.cmd_tx.send(NetCmd::OpenUsb {
                        name: port,
                    });
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }

        if let Some(step) = self.wizard_step {
            egui::Window::new("Njörðr seas · Setup")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
                .show(ctx, |ui| {
                    ui.set_min_width(500.0);
                    ui.vertical_centered(|ui| {
                        brand_logo(ui, 72.0);
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Njörðr seas CYD miner")
                                .color(C_LIME)
                                .font(display_font(26.0)),
                        );
                        ui.label(
                            RichText::new("USB SHA-256 Bitcoin miner · PC owns the pool")
                                .color(C_MUTED)
                                .font(mono_ui_font(12.0)),
                        );
                    });
                    ui.add_space(14.0);
                    let title = match step {
                        0 => "1 · Plug in & driver",
                        1 => "2 · Select COM & connect",
                        2 => "3 · Flash & verify firmware",
                        _ => "4 · Pool worker",
                    };
                    ui.label(RichText::new(title).color(C_TEXT).font(display_font(22.0)));
                    ui.add_space(8.0);
                    match step {
                        0 => {
                            ui.label(
                                RichText::new(
                                    "Use a USB-C data cable. Most CYD boards need a CH340 driver — if Windows shows an unknown device, install CH340, then replug.",
                                )
                                .color(C_TEXT)
                                .size(14.0),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(
                                    "Tip: after flashing, Companion verifies the board over USB (cmp ping + config).",
                                )
                                .color(C_DIM)
                                .size(13.0),
                            );
                        }
                        1 => {
                            ui.label(
                                RichText::new("Pick the COM port for your board, then Connect.")
                                    .color(C_TEXT)
                                    .size(14.0),
                            );
                            ui.add_space(8.0);
                            ui.horizontal_wrapped(|ui| {
                                let combo_w = (ui.available_width() - 220.0).clamp(140.0, 320.0);
                                let wiz_com_label = if self.com_port.is_empty() {
                                    "Select port".to_string()
                                } else {
                                    self.ports
                                        .iter()
                                        .find(|p| p.name == self.com_port)
                                        .map(|p| p.label.clone())
                                        .unwrap_or_else(|| self.com_port.clone())
                                };
                                egui::ComboBox::from_id_source("wiz_com")
                                    .width(combo_w)
                                    .selected_text(
                                        RichText::new(wiz_com_label).color(C_TEXT).size(13.0),
                                    )
                                    .show_ui(ui, |ui| {
                                        for p in self.ports.clone() {
                                            ui.selectable_value(
                                                &mut self.com_port,
                                                p.name.clone(),
                                                &p.label,
                                            );
                                        }
                                    });
                                if soft_button(ui, "Refresh", 90.0).clicked() {
                                    self.refresh_com_ports(false, false);
                                }
                                if soft_button(
                                    ui,
                                    if self.usb_open { "Connected" } else { "Connect" },
                                    110.0,
                                )
                                .clicked()
                                    && !self.usb_open
                                {
                                    self.connect_or_add_usb();
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
                                    "Update board writes the kit image, then reconnects and verifies fw over USB. When asked: hold BOOT, tap RESET, keep BOOT, click Ready.",
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
                                if soft_button(ui, "Update app", 120.0).clicked() {
                                    self.start_app_update();
                                }
                            });
                            if !self.update_status.is_empty() {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(&self.update_status)
                                        .color(if self.update_busy { C_WARN } else { C_DIM })
                                        .font(mono_ui_font(11.0)),
                                );
                            }
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
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(
                                    "Best path on this board (not a different alg): Bench boards → Full HW · 240 MHz · more CYDs for more rate. Algorithm stays SHA-256d.",
                                )
                                .color(C_DIM)
                                .size(12.0),
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
            let live_push = self.board_supports_live_push(&self.com_port);
            egui::Window::new(if live_push {
                "Push firmware update"
            } else {
                "Update board firmware"
            })
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
                        RichText::new(if live_push {
                            "Linked board will get a silent push update (auto-reset). No BOOT button unless auto-reset fails. Mining stops briefly."
                        } else {
                            "Blank / download-mode board: mining stops, then hold BOOT → tap RESET → keep BOOT → Ready when asked."
                        })
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
                        let flash_label = if live_push {
                            if up_to_date {
                                "Push anyway"
                            } else {
                                "Push update"
                            }
                        } else if up_to_date {
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

        // Board update: loading overlay with live progress bar.
        if self.update_busy {
            let awaiting_boot = self
                .flash_need_boot
                .as_ref()
                .map(|n| n.load(Ordering::SeqCst))
                .unwrap_or(false);
            if awaiting_boot {
                // Keep the Ready CTA responsive while the flash thread waits.
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            egui::Window::new("Updating board")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(420.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(12.0);
                        ui.add(egui::Spinner::new().size(44.0).color(C_LIME));
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(if self.post_flash_verify.is_some() {
                                "Verifying board firmware"
                            } else if self.pending_post_flash_reconnect.is_some() {
                                "Flash complete — reconnecting"
                            } else if awaiting_boot {
                                "Download mode — click Ready"
                            } else {
                                "Flashing board firmware"
                            })
                            .color(C_LIME)
                            .font(display_font(22.0)),
                        );
                        ui.add_space(12.0);
                        let pct = (self.flash_progress.clamp(0.0, 1.0) * 100.0).round() as u32;
                        let phase = if self.flash_phase.is_empty() {
                            "Working…".to_string()
                        } else {
                            self.flash_phase.clone()
                        };
                        ui.label(
                            RichText::new(format!("{phase} · {pct}%"))
                                .color(C_TEXT)
                                .font(mono_ui_font(13.0)),
                        );
                        ui.add_space(8.0);
                        let bar_w = ui.available_width().min(360.0);
                        ui.add(
                            egui::ProgressBar::new(self.flash_progress.clamp(0.0, 1.0))
                                .desired_width(bar_w)
                                .animate(true)
                                .fill(C_LIME),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(if self.update_status.is_empty() {
                                "Starting…".to_string()
                            } else {
                                self.update_status.clone()
                            })
                            .color(C_MUTED)
                            .font(mono_ui_font(11.0)),
                        );
                        ui.add_space(10.0);
                        if awaiting_boot {
                            ui.label(
                                RichText::new(
                                    "1) Hold BOOT · 2) Tap RESET · 3) Keep BOOT held · 4) Ready · keep BOOT until Writing %",
                                )
                                .color(C_TEXT)
                                .size(13.0),
                            );
                            ui.add_space(12.0);
                            if cta_button(ui, "Ready", true, 160.0).clicked() {
                                if let Some(r) = &self.flash_boot_ready {
                                    r.store(true, Ordering::SeqCst);
                                }
                                self.flash_phase = "Syncing download mode".into();
                                self.update_status =
                                    "Ready — syncing ROM, then writing…".into();
                            }
                            ui.add_space(8.0);
                        } else {
                            ui.label(
                                RichText::new(
                                    "Keep USB connected · hold BOOT + tap RESET if needed",
                                )
                                .color(C_DIM)
                                .size(12.0),
                            );
                            ui.add_space(8.0);
                        }
                        if soft_button(ui, "Cancel", 120.0).clicked() {
                            if let Some(c) = &self.flash_cancel {
                                c.store(true, Ordering::SeqCst);
                            }
                            if let Some(n) = &self.flash_need_boot {
                                n.store(false, Ordering::SeqCst);
                            }
                            self.clear_flash_overlay();
                            self.update_status =
                                "Board update cancelled — killing flash tool if stuck…"
                                    .into();
                            self.push_log(LogKind::Usb, self.update_status.clone());
                        }
                        ui.add_space(8.0);
                    });
                });
        }

        egui::TopBottomPanel::top("live_ticker_bar")
            .exact_height(if ctx.available_rect().width() < 820.0 {
                64.0
            } else {
                36.0
            })
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(3, 14, 28))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(28, 64, 98)))
                    .inner_margin(Margin::symmetric(12.0, 4.0)),
            )
            .show(ctx, |ui| {
                ui_live_bar(ui, &self.live, &mut self.header_coins);
            });

        egui::TopBottomPanel::top("app_nav_bar")
            .resizable(false)
            .show_separator_line(false)
            .frame(
                Frame::none()
                    .fill(C_BG)
                    .inner_margin(Margin::symmetric(16.0, 10.0)),
            )
            .show(ctx, |ui| {
                self.ui_app_nav(ui);
            });

        egui::CentralPanel::default()
            .frame(Frame::none().fill(C_BG).inner_margin(Margin::symmetric(16.0, 8.0)))
            .show(ctx, |ui| {
                paint_background(ui, ui.max_rect(), self.pulse, self.grid_phase, self.mining || self.board_hashing());
                // After sea backdrop, before widgets — faded so it never fights controls.
                paint_under_development_watermarks(ui);

                // Outer scroll so Mine/Settings content is fully reachable on short screens.
                ScrollArea::vertical()
                    .id_source("main_app_scroll")
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        match self.tab {
                            Tab::Mine | Tab::Debug => self.ui_mine(ui),
                            Tab::Settings => self.ui_settings(ui),
                        }
                        ui.add_space(28.0);
                    });
            });

        // Keep animation continuous (~60 fps). 40 ms made looping motion feel stepped.
        ctx.request_repaint_after(Duration::from_millis(16));
        self.refresh_monitor_lan_ip();
        self.publish_phone_monitor();
    }
}

fn hostname_fallback() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "cyd-pc".into())
}

fn local_host_hint() -> String {
    hostname_fallback()
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

/// Pull a 0..=100 percent from espflash-style progress text.
fn parse_flash_percent(line: &str) -> Option<f32> {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            // Walk back over digits / decimal.
            let mut j = i;
            while j > 0 && (bytes[j - 1] == b'.' || bytes[j - 1].is_ascii_digit()) {
                j -= 1;
            }
            if j < i {
                if let Some(v) = std::str::from_utf8(&bytes[j..i])
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                {
                    if (0.0..=100.0).contains(&v) {
                        return Some(v);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn ui_live_bar(ui: &mut egui::Ui, live: &LiveFeed, header_coins: &mut Vec<String>) {
    let snap = live.snap();
    // Wrap on narrow windows so the ticker never overlaps the nav tabs below.
    ui.horizontal_wrapped(|ui| {
            // Local place / weather / clock (IP-derived)
            ui.label(
                RichText::new(live.place_label())
                    .color(C_TEXT)
                    .font(mono_ui_font(12.0)),
            );
            ui.add_space(10.0);
            if snap.ready && !snap.weather.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{:.0}°F {}",
                        snap.temp_c * 9.0 / 5.0 + 32.0,
                        snap.weather
                    ))
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
            ui.add_space(12.0);

            // Coin picker — choose which symbols appear in the strip.
            egui::ComboBox::from_id_source("header_coin_picker")
                .selected_text(
                    RichText::new("Coin selection")
                        .color(C_LIME)
                        .font(mono_ui_font(11.0)),
                )
                .width(128.0)
                .show_ui(ui, |ui| {
                    ui.set_min_width(160.0);
                    ui.label(
                        RichText::new("Header coins")
                            .color(C_MUTED)
                            .font(mono_ui_font(11.0)),
                    );
                    ui.add_space(4.0);
                    for (_, sym) in COIN_CATALOG {
                        let mut on = header_coins.iter().any(|s| s.eq_ignore_ascii_case(sym));
                        if ui
                            .checkbox(
                                &mut on,
                                RichText::new(*sym)
                                    .color(C_TEXT)
                                    .font(mono_ui_font(12.0)),
                            )
                            .changed()
                        {
                            if on {
                                if !header_coins.iter().any(|s| s.eq_ignore_ascii_case(sym)) {
                                    header_coins.push((*sym).into());
                                }
                            } else if header_coins.len() > 1 {
                                header_coins.retain(|s| !s.eq_ignore_ascii_case(sym));
                            }
                        }
                    }
                    ui.add_space(6.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Reset BTC · LTC · ETH")
                                    .color(C_MUTED)
                                    .font(mono_ui_font(11.0)),
                            )
                            .fill(Color32::TRANSPARENT),
                        )
                        .clicked()
                    {
                        *header_coins = default_header_coins();
                    }
                });

            ui.add_space(12.0);

            // Crypto strip (selected coins only)
            let quotes = live.quotes_for(header_coins);
            if quotes.is_empty() {
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
                for (i, q) in quotes.iter().enumerate() {
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
    });
}

fn paint_under_development_watermarks(ui: &egui::Ui) {
    // Drawn on the panel layer after the sea backdrop and before widgets.
    let rect = ui.max_rect();
    if rect.width() < 8.0 || rect.height() < 8.0 {
        return;
    }
    let painter = ui.painter();

    let phrase = "UNDER DEVELOPMENT";
    let chars: Vec<char> = phrase.chars().collect();
    let angle = rect.height().atan2(rect.width());
    let dir = Vec2::angled(angle);
    let perp = Vec2::new(-dir.y, dir.x);
    let color = Color32::from_rgba_unmultiplied(186, 214, 232, 11);
    let font = FontId::monospace(15.0);
    let char_step = 13.5;
    let phrase_span = chars.len() as f32 * char_step;
    let phrase_gap = 72.0;
    let lane_spacing = 96.0;
    let diag = (rect.width() * rect.width() + rect.height() * rect.height()).sqrt();
    let origin = rect.left_top() - dir * 40.0 - perp * 60.0;
    let lanes = ((diag / lane_spacing) as i32) + 6;

    for lane in -3..lanes {
        let lane_origin = origin + perp * (lane as f32 * lane_spacing);
        let mut along = -phrase_span;
        while along < diag + phrase_span {
            for (i, ch) in chars.iter().enumerate() {
                if *ch == ' ' {
                    continue;
                }
                let p = lane_origin + dir * (along + i as f32 * char_step);
                if rect.expand(48.0).contains(p) {
                    painter.text(
                        p,
                        egui::Align2::CENTER_CENTER,
                        ch.to_string(),
                        font.clone(),
                        color,
                    );
                }
            }
            along += phrase_span + phrase_gap;
        }
    }
}

fn paint_background(ui: &mut egui::Ui, rect: Rect, pulse: f32, grid_phase: f32, mining: bool) {
    let painter = ui.painter();
    let bands = 84;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        let y0 = rect.top() + rect.height() * t;
        let y1 = rect.top() + rect.height() * ((i + 1) as f32 / bands as f32) + 1.0;
        // Deep sea → shallower teal horizon.
        let c = lerp_color(C_BG, C_BG_2, t * 0.85 + 0.08 * (pulse * 0.35).sin());
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(rect.left(), y0), Pos2::new(rect.right(), y1)),
            0.0,
            c,
        );
    }

    let glow = if mining {
        (48.0 + pulse.sin().max(0.0) * 72.0) as u8
    } else {
        28
    };
    // Soft aurora / neon storm glow orbs.
    painter.circle_filled(
        Pos2::new(rect.left() + rect.width() * 0.18, rect.top() + rect.height() * 0.16),
        380.0,
        rgba(C_NEON_DEEP, glow / 4),
    );
    painter.circle_filled(
        Pos2::new(rect.left() + rect.width() * 0.22, rect.top() + rect.height() * 0.20),
        220.0,
        rgba(C_NEON, glow / 5),
    );
    painter.circle_filled(
        Pos2::new(rect.right() - rect.width() * 0.10, rect.bottom() - rect.height() * 0.14),
        300.0,
        rgba(C_LIME_SOFT, 18),
    );
    painter.circle_filled(
        Pos2::new(rect.left() + rect.width() * 0.72, rect.top() + rect.height() * 0.08),
        180.0,
        rgba(C_NEON, glow / 4),
    );
    painter.circle_filled(
        Pos2::new(rect.left() + rect.width() * 0.48, rect.top() + rect.height() * 0.02),
        90.0,
        rgba(C_NEON_HOT, glow / 6),
    );

    // Slow current lines (wave diagonals) — neon-tinted.
    let step = 38.0;
    let drift = grid_phase * step;
    let mut x = rect.left() - rect.height() + drift;
    while x < rect.right() + rect.height() {
        painter.line_segment(
            [
                Pos2::new(x, rect.bottom()),
                Pos2::new(x + rect.height() * 0.66, rect.top()),
            ],
            Stroke::new(1.0_f32, rgba(C_NEON, 14)),
        );
        x += step;
    }

    paint_lightning_storm(&painter, rect, pulse, mining);
}

/// Njörðr storm — rare sharp neon-blue lightning bolts across the deep.
fn paint_lightning_storm(painter: &egui::Painter, rect: Rect, pulse: f32, mining: bool) {
    let base = if mining { 1.15 } else { 0.65 };
    let flashes = [
        ((pulse * 1.15).sin().max(0.0).powf(9.0) * base, 0.18, 0.05, 1.05),
        (((pulse * 0.82) + 1.7).sin().max(0.0).powf(11.0) * base, 0.62, 0.12, 0.92),
        (((pulse * 1.55) + 3.1).sin().max(0.0).powf(13.0) * base * 0.9, 0.42, 0.0, 0.78),
        (((pulse * 0.95) + 4.4).sin().max(0.0).powf(15.0) * base * 0.7, 0.78, 0.08, 0.7),
    ];
    for (intensity, x_frac, y_frac, scale) in flashes {
        if intensity < 0.07 {
            continue;
        }
        let origin = Pos2::new(
            rect.left() + rect.width() * x_frac,
            rect.top() + rect.height() * y_frac,
        );
        paint_lightning_bolt(
            painter,
            origin,
            rect.height() * 0.55 * scale,
            intensity,
            1.6 + intensity * 2.0,
        );
    }
}

fn paint_lightning_bolt(
    painter: &egui::Painter,
    origin: Pos2,
    length: f32,
    intensity: f32,
    width: f32,
) {
    // Jagged relative polyline (x,y) in unit space down the bolt.
    let segs: [(f32, f32); 7] = [
        (0.00, 0.00),
        (0.14, 0.16),
        (-0.08, 0.28),
        (0.18, 0.44),
        (-0.06, 0.58),
        (0.12, 0.76),
        (0.02, 1.00),
    ];
    let mut pts: Vec<Pos2> = Vec::with_capacity(segs.len());
    for (dx, dy) in segs {
        pts.push(Pos2::new(origin.x + dx * length * 0.55, origin.y + dy * length));
    }

    let i = intensity.clamp(0.0, 1.0);
    let bloom_a = (i * 55.0) as u8;
    let neon_a = (i * 160.0) as u8;
    let core_a = (i * 245.0) as u8;
    let hot_a = (i * 220.0) as u8;

    // Neon splash bloom at strike origin.
    painter.circle_filled(origin, length * 0.14 * i + 10.0, rgba(C_NEON_DEEP, bloom_a));
    painter.circle_filled(origin, length * 0.07 * i + 5.0, rgba(C_NEON, neon_a / 2));

    for pair in pts.windows(2) {
        // Wide deep-blue halo → neon cyan → ice-hot core.
        painter.line_segment(
            [pair[0], pair[1]],
            Stroke::new(width * 7.5, rgba(C_NEON_DEEP, bloom_a)),
        );
        painter.line_segment(
            [pair[0], pair[1]],
            Stroke::new(width * 4.2, rgba(C_NEON, neon_a)),
        );
        painter.line_segment(
            [pair[0], pair[1]],
            Stroke::new(width * 2.0, rgba(C_LIME, neon_a.saturating_add(20))),
        );
        painter.line_segment(
            [pair[0], pair[1]],
            Stroke::new(width * 0.85, rgba(C_NEON_HOT, hot_a)),
        );
        painter.line_segment(
            [pair[0], pair[1]],
            Stroke::new(width * 0.35, rgba(C_BOLT, core_a)),
        );
    }
    // Small fork near mid — neon splash.
    if pts.len() >= 4 {
        let mid = pts[3];
        let fork = Pos2::new(mid.x + length * 0.12, mid.y + length * 0.14);
        painter.line_segment([mid, fork], Stroke::new(width * 5.0, rgba(C_NEON_DEEP, bloom_a)));
        painter.line_segment([mid, fork], Stroke::new(width * 2.6, rgba(C_NEON, neon_a)));
        painter.line_segment([mid, fork], Stroke::new(width * 0.7, rgba(C_NEON_HOT, hot_a)));
        painter.circle_filled(fork, 4.0 + 6.0 * i, rgba(C_NEON, neon_a / 2));
    }
    // Tip splash.
    if let Some(tip) = pts.last() {
        painter.circle_filled(*tip, 8.0 + 10.0 * i, rgba(C_NEON, neon_a / 3));
        painter.circle_filled(*tip, 3.0 + 4.0 * i, rgba(C_NEON_HOT, hot_a / 2));
    }
}

fn paint_hero_wash(ui: &mut egui::Ui, rect: Rect, pulse: f32, mining: bool) {
    let painter = ui.painter();
    let alpha = if mining {
        (28.0 + (0.5 + 0.5 * pulse.sin()) * 52.0) as u8
    } else {
        16
    };
    painter.circle_filled(
        Pos2::new(rect.left() + 110.0, rect.top() + 78.0),
        200.0,
        rgba(C_NEON_DEEP, alpha / 2),
    );
    painter.circle_filled(
        Pos2::new(rect.left() + 130.0, rect.top() + 90.0),
        120.0,
        rgba(C_NEON, alpha / 3),
    );
    painter.circle_filled(
        Pos2::new(rect.right() - 80.0, rect.bottom() - 40.0),
        130.0,
        rgba(C_LIME_SOFT, if mining { 22 } else { 12 }),
    );
    painter.circle_filled(
        Pos2::new(rect.right() - 160.0, rect.top() + 36.0),
        70.0,
        rgba(C_NEON, if mining { alpha / 3 } else { 10 }),
    );

    // Storm sweep — neon electric arc across the hero.
    let sweep = (pulse * 0.12).rem_euclid(1.0);
    let edge = loop_edge_fade(sweep, 0.14);
    let x = rect.left() + rect.width() * sweep;
    let sweep_a = ((if mining { 48.0 } else { 18.0 }) * edge) as u8;
    if sweep_a > 0 {
        painter.line_segment(
            [
                Pos2::new(x, rect.bottom() - 18.0),
                Pos2::new(x + rect.height() * 0.55, rect.top() + 18.0),
            ],
            Stroke::new(5.0_f32, rgba(C_NEON_DEEP, sweep_a / 2)),
        );
        painter.line_segment(
            [
                Pos2::new(x, rect.bottom() - 18.0),
                Pos2::new(x + rect.height() * 0.55, rect.top() + 18.0),
            ],
            Stroke::new(2.4_f32, rgba(C_NEON, sweep_a)),
        );
        painter.line_segment(
            [
                Pos2::new(x + 8.0, rect.bottom() - 28.0),
                Pos2::new(x + rect.height() * 0.42, rect.top() + 36.0),
            ],
            Stroke::new(1.1_f32, rgba(C_NEON_HOT, sweep_a / 2)),
        );
    }

    // Occasional hero bolt when hashing — neon splash.
    if mining {
        let flash = (pulse * 1.4 + 0.6).sin().max(0.0).powf(10.0);
        if flash > 0.10 {
            paint_lightning_bolt(
                &painter,
                Pos2::new(rect.right() - 140.0, rect.top() + 12.0),
                rect.height() * 0.85,
                flash,
                1.5 + flash * 1.2,
            );
        }
        let flash2 = (pulse * 1.05 + 2.3).sin().max(0.0).powf(12.0);
        if flash2 > 0.14 {
            paint_lightning_bolt(
                &painter,
                Pos2::new(rect.left() + 90.0, rect.top() + 8.0),
                rect.height() * 0.7,
                flash2 * 0.85,
                1.2 + flash2,
            );
        }
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
        let color = if hashing {
            // Alternate neon cyan / ice blue for electric hash bars.
            if i % 3 == 0 {
                rgba(C_NEON, (90.0 + level * 150.0) as u8)
            } else if i % 3 == 1 {
                rgba(C_LIME, (70.0 + level * 140.0) as u8)
            } else {
                rgba(C_NEON_HOT, (60.0 + level * 130.0) as u8)
            }
        } else {
            rgba(C_BUBBLE_HI, 50)
        };
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x, y), Pos2::new(x + bar_w, rect.bottom())),
            Rounding::same(2.0),
            color,
        );
        if hashing && level > 0.55 {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x, y),
                    Pos2::new(x + bar_w, (y + 3.0).min(rect.bottom())),
                ),
                Rounding::same(1.0),
                rgba(C_NEON_HOT, 180),
            );
        }
    }
}

struct DataFlowView {
    pulse: f32,
    pool_label: &'static str,
    pool_color: Color32,
    usb_open: bool,
    mining: bool,
    jobs_live: bool,
    hash_live: bool,
    share_burst: bool,
    board_count: usize,
    rate_label: String,
    accepted: u32,
}

fn paint_data_flow(ui: &mut egui::Ui, v: DataFlowView) {
    let desired = Vec2::new(ui.available_width().clamp(280.0, 920.0), 118.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        Rounding::same(16.0),
        Color32::from_rgba_unmultiplied(3, 14, 28, 200),
    );
    painter.rect_stroke(rect, Rounding::same(16.0), Stroke::new(1.0_f32, rgba(C_STROKE, 160)));

    let pad = 18.0;
    let node_w = 118.0;
    let node_h = 64.0;
    let y = rect.center().y;
    let left = rect.left() + pad + node_w * 0.5;
    let right = rect.right() - pad - node_w * 0.5;
    let mid = rect.center().x;
    let pool_c = Pos2::new(left, y);
    let app_c = Pos2::new(mid, y);
    let board_c = Pos2::new(right, y);

    let link = |a: Pos2, b: Pos2, active: bool, reverse: bool, color: Color32| {
        let stroke = Stroke::new(
            if active { 2.4_f32 } else { 1.2_f32 },
            rgba(color, if active { 160 } else { 55 }),
        );
        painter.line_segment([a, b], stroke);
        // Traveling packets along the link.
        let n = if active { 3 } else { 1 };
        for i in 0..n {
            let base = (v.pulse * (if reverse { -0.55 } else { 0.55 })
                + i as f32 * (1.0 / n as f32))
                .rem_euclid(1.0);
            let t = if active {
                base
            } else {
                0.15 + 0.1 * (v.pulse + i as f32).sin()
            };
            let p = Pos2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            let r = if active { 4.2 } else { 2.4 };
            painter.circle_filled(p, r + 2.0, rgba(color, if active { 40 } else { 18 }));
            painter.circle_filled(p, r, if active { color } else { rgba(color, 90) });
        }
    };

    // Upper path: jobs pool → app → board. Lower path offset for shares back.
    let job_y = y - 10.0;
    let share_y = y + 10.0;
    link(
        Pos2::new(pool_c.x + node_w * 0.42, job_y),
        Pos2::new(app_c.x - node_w * 0.42, job_y),
        v.jobs_live,
        false,
        C_LIME,
    );
    link(
        Pos2::new(app_c.x + node_w * 0.42, job_y),
        Pos2::new(board_c.x - node_w * 0.42, job_y),
        v.jobs_live && v.usb_open,
        false,
        C_LIME,
    );
    link(
        Pos2::new(board_c.x - node_w * 0.42, share_y),
        Pos2::new(app_c.x + node_w * 0.42, share_y),
        v.share_burst || v.hash_live,
        true,
        C_WARN,
    );
    link(
        Pos2::new(app_c.x - node_w * 0.42, share_y),
        Pos2::new(pool_c.x + node_w * 0.42, share_y),
        v.share_burst && v.jobs_live,
        true,
        C_WARN,
    );

    let glow = 0.55 + 0.45 * (0.5 + 0.5 * v.pulse.sin());
    let draw_node = |center: Pos2, title: &str, detail: &str, live: bool, accent: Color32| {
        let r = Rect::from_center_size(center, Vec2::new(node_w, node_h));
        painter.rect_filled(
            r,
            Rounding::same(14.0),
            Color32::from_rgba_unmultiplied(8, 26, 46, 235),
        );
        painter.rect_stroke(
            r,
            Rounding::same(14.0),
            Stroke::new(
                if live { 1.8_f32 } else { 1.0_f32 },
                rgba(accent, if live { (90.0 + glow * 120.0) as u8 } else { 70 }),
            ),
        );
        if live {
            painter.circle_filled(
                Pos2::new(r.right() - 12.0, r.top() + 12.0),
                3.6,
                accent,
            );
        }
        painter.text(
            Pos2::new(center.x, center.y - 12.0),
            egui::Align2::CENTER_CENTER,
            title,
            mono_ui_font(12.0),
            C_TEXT,
        );
        painter.text(
            Pos2::new(center.x, center.y + 12.0),
            egui::Align2::CENTER_CENTER,
            detail,
            mono_ui_font(10.0),
            if live { accent } else { C_MUTED },
        );
    };

    let board_detail = if v.usb_open {
        if v.board_count > 1 {
            format!("{} boards · {}", v.board_count, v.rate_label)
        } else {
            format!("USB · {}", v.rate_label)
        }
    } else {
        "idle".into()
    };
    let app_detail = if v.mining {
        format!("mine · {} ok", v.accepted)
    } else if v.usb_open {
        "linked".into()
    } else {
        "standby".into()
    };

    draw_node(pool_c, "POOL", v.pool_label, v.jobs_live, v.pool_color);
    draw_node(app_c, "COMPANION", &app_detail, v.mining || v.usb_open, C_LIME);
    draw_node(
        board_c,
        "BOARD",
        &board_detail,
        v.hash_live || v.usb_open,
        if v.hash_live { C_LIME } else { C_BUBBLE_HI },
    );

    // Direction captions
    painter.text(
        Pos2::new(mid, rect.top() + 14.0),
        egui::Align2::CENTER_CENTER,
        if v.jobs_live { "jobs →" } else { "jobs idle" },
        mono_ui_font(10.0),
        if v.jobs_live { rgba(C_LIME, 200) } else { C_DIM },
    );
    painter.text(
        Pos2::new(mid, rect.bottom() - 14.0),
        egui::Align2::CENTER_CENTER,
        if v.share_burst || v.hash_live {
            "← shares / hash"
        } else {
            "← shares idle"
        },
        mono_ui_font(10.0),
        if v.share_burst || v.hash_live {
            rgba(C_WARN, 200)
        } else {
            C_DIM
        },
    );
}

fn sparkline(ui: &mut egui::Ui, values: &VecDeque<f32>, pulse: f32, history_phase: f32) {
    let desired = Vec2::new((ui.available_width() * 0.72).clamp(320.0, 620.0), 86.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::same(18.0), Color32::from_rgba_unmultiplied(3, 12, 26, 180));
    painter.rect_stroke(
        rect,
        Rounding::same(18.0),
        Stroke::new(1.0_f32, rgba(C_LIME, 28)),
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
            Stroke::new(1.0_f32, rgba(C_BOLT, scan_a)),
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
    // Light atmospheric panels — not opaque dashboard cards.
    Frame::none()
        .fill(Color32::from_rgba_unmultiplied(10, 30, 52, 148))
        .rounding(Rounding::same(20.0))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(36, 78, 118, 140)))
        .inner_margin(Margin::same(16.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .color(C_LIME)
                    .font(display_font(15.0)),
            );
            ui.add_space(8.0);
            add(ui);
        });
}

fn nav_button(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let fill = if selected {
        Color32::from_rgba_unmultiplied(126, 220, 255, 42)
    } else {
        Color32::from_rgba_unmultiplied(12, 34, 58, 128)
    };
    let stroke = if selected {
        Stroke::new(1.0_f32, rgba(C_LIME, 120))
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
        .rounding(Rounding::same(12.0))
        .min_size(Vec2::new(86.0, 34.0)),
    )
}

fn cta_button(ui: &mut egui::Ui, label: &str, lime: bool, width: f32) -> egui::Response {
    let fill = if lime { C_LIME } else { Color32::from_rgb(18, 48, 78) };
    let text = if lime {
        Color32::from_rgb(4, 18, 36)
    } else {
        C_TEXT
    };
    ui.add(
        egui::Button::new(RichText::new(label).color(text).font(display_font(17.0)))
            .fill(fill)
            .stroke(Stroke::new(
                1.0_f32,
                rgba(C_LIME, if lime { 110 } else { 42 }),
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
                .color(if selected { Color32::from_rgb(4, 18, 36) } else { C_TEXT })
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

enum BoardIo {
    Serial(Box<dyn SerialPort>),
    Tcp(TcpStream),
}

impl BoardIo {
    fn drain(&mut self, buf: &mut String) {
        let mut tmp = [0u8; 2048];
        for _ in 0..64 {
            let n = match self {
                BoardIo::Serial(p) => p.read(&mut tmp),
                BoardIo::Tcp(p) => p.read(&mut tmp),
            };
            match n {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    buf.push_str(&String::from_utf8_lossy(&tmp[..n]));
                    if buf.len() > 24576 {
                        *buf = buf[buf.len() - 8192..].to_string();
                    }
                }
            }
        }
    }

    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        match self {
            BoardIo::Serial(p) => p.write_all(data).map_err(|e| format!("write: {e}")),
            BoardIo::Tcp(p) => p.write_all(data).map_err(|e| format!("write: {e}")),
        }
    }

    fn flush(&mut self) -> Result<(), String> {
        match self {
            BoardIo::Serial(p) => p.flush().map_err(|e| format!("flush: {e}")),
            BoardIo::Tcp(p) => p.flush().map_err(|e| format!("flush: {e}")),
        }
    }

    fn clear(&mut self) {
        if let BoardIo::Serial(p) = self {
            let _ = p.clear(serialport::ClearBuffer::All);
        }
    }
}


fn mine_worker(cmd_rx: Receiver<NetCmd>, msg_tx: Sender<NetMsg>) {
    struct UsbBoard {
        name: String,
        port: BoardIo,
        rx: String,
        legacy_job: bool,
        fw: String,
        mac: String,
        hashrate_hs: f64,
        hashes: u64,
        mining: bool,
        /// Consecutive cmp status soft-fails — used to drop dead USB boards.
        status_fails: u8,
        /// ROM download mode (BOOT held / blank) — no cmp; flash via Update board.
        download_mode: bool,
    }

    fn live_from(boards: &[UsbBoard]) -> Vec<WorkerLive> {
        boards
            .iter()
            .map(|b| WorkerLive {
                endpoint: b.name.clone(),
                mac: b.mac.clone(),
                fw: if b.download_mode {
                    "download-mode".into()
                } else {
                    b.fw.clone()
                },
                connected: true,
                hashrate_hs: b.hashrate_hs,
                hashes: b.hashes,
                mining: b.mining,
            })
            .collect()
    }

    fn publish_live(tx: &Sender<NetMsg>, boards: &[UsbBoard]) {
        let _ = tx.send(NetMsg::WorkersLive(live_from(boards)));
    }

    fn open_board(name: &str) -> Result<(UsbBoard, bool), String> {
        // Prefer 460800 for faster job/share traffic; fall back to 115200 for older FW.
        let mut last_err = String::new();
        for baud in [460_800u32, 115_200] {
            match open_board_at(name, baud) {
                Ok(v) => return Ok(v),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    fn open_wifi_board(endpoint: &str) -> Result<(UsbBoard, bool), String> {
        let stream = open_wifi_tcp(endpoint)?;
        let mut rx = String::new();
        let mut port = BoardIo::Tcp(stream);
        let _ = port.write_all(b"\r\ncmp ping\r\n");
        let _ = port.flush();
        let deadline = Instant::now() + Duration::from_millis(2_500);
        let mut saw = false;
        while Instant::now() < deadline {
            port.drain(&mut rx);
            if rx.lines().any(|l| {
                let t = l.trim();
                t.starts_with("CMP ok") || t.eq_ignore_ascii_case("CMPACK ping")
            }) {
                saw = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        if !saw {
            return Err(format!("no pong from Wi‑Fi {endpoint}"));
        }
        Ok((
            UsbBoard {
                name: endpoint.to_string(),
                port,
                rx,
                legacy_job: false,
                fw: String::new(),
                mac: String::new(),
                hashrate_hs: 0.0,
                hashes: 0,
                mining: false,
                status_fails: 0,
                download_mode: false,
            },
            true,
        ))
    }

    fn open_board_at(name: &str, baud: u32) -> Result<(UsbBoard, bool), String> {
        // Timed open so a hung motherboard COM cannot stall Add board / auto-link.
        let mut port = BoardIo::Serial(open_usb_serial_timed(
            name,
            baud,
            Duration::from_millis(8),
            Duration::from_secs(3),
        )?);
        let mut rx = String::new();
        let wait_ms = if baud > 115_200 { 1_600 } else { 2_800 };
        let mut saw = false;
        for attempt in 0..3u32 {
            if attempt > 0 {
                // UART bridge may still have reset the MCU on open — wait for boot + SoftAP.
                thread::sleep(Duration::from_millis(1_800));
                port.clear();
                rx.clear();
            }
            let _ = port.write_all(b"\r\ncmp ping\r\n");
            let _ = port.flush();
            let deadline = Instant::now() + Duration::from_millis(wait_ms + attempt as u64 * 400);
            while Instant::now() < deadline {
                port.drain(&mut rx);
                if rx.lines().any(|l| {
                    let t = l.trim();
                    t.starts_with("CMP ok") || t.eq_ignore_ascii_case("CMPACK ping")
                }) {
                    saw = true;
                    break;
                }
                thread::sleep(Duration::from_millis(15));
            }
            if saw {
                break;
            }
        }
        // Prefer firmware pong; if silent, accept ESP ROM download mode (BOOT held / blank).
        if !saw {
            if let BoardIo::Serial(ref mut p) = port {
                if probe_esp_download_mode(p.as_mut()) {
                    return Ok((
                        UsbBoard {
                            name: name.to_string(),
                            port,
                            rx,
                            legacy_job: false,
                            fw: "download-mode".into(),
                            mac: String::new(),
                            hashrate_hs: 0.0,
                            hashes: 0,
                            mining: false,
                            status_fails: 0,
                            download_mode: true,
                        },
                        false,
                    ));
                }
            }
            return Err(format!(
                "no pong @ {baud} on {name} (hold BOOT + tap RESET if flashing a blank board)"
            ));
        }
        Ok((
            UsbBoard {
                name: name.to_string(),
                port,
                rx,
                legacy_job: false,
                fw: String::new(),
                mac: String::new(),
                hashrate_hs: 0.0,
                hashes: 0,
                mining: false,
                status_fails: 0,
                download_mode: false,
            },
            true,
        ))
    }

    fn configure_board(board: &mut UsbBoard, msg_tx: &Sender<NetMsg>) {
        if board.download_mode {
            log_msg(
                msg_tx,
                LogKind::Usb,
                format!(
                    "{} in download mode (BOOT held / blank) — use Update board to flash; mining needs firmware",
                    board.name
                ),
            );
            return;
        }
        if let Ok(line) = usb_cmd(&mut board.port, &mut board.rx, "cmp config") {
            if let Ok(cfg) = parse_cmp_config(&line) {
                board.legacy_job = !fw_supports_split_jobs(&cfg.fw);
                board.fw = cfg.fw.clone();
                if !cfg.mac.is_empty() {
                    board.mac = normalize_mac(&cfg.mac);
                }
                if board.legacy_job {
                    log_msg(
                        msg_tx,
                        LogKind::Warn,
                        format!(
                            "Board {} fw {} lacks jh/jt/ja — using legacy job",
                            board.name, cfg.fw
                        ),
                    );
                }
                let _ = msg_tx.send(NetMsg::Config(Ok(cfg)));
            } else {
                let _ = msg_tx.send(NetMsg::Config(parse_cmp_config(&line)));
            }
        }
        if let Ok(line) = usb_cmd(&mut board.port, &mut board.rx, "cmp status") {
            if let Ok(st) = parse_cmp_status(&line) {
                board.hashrate_hs = st.hashrate_hs;
                board.hashes = st.hashes;
                board.mining = st.mining;
                if !st.mac.is_empty() {
                    board.mac = normalize_mac(&st.mac);
                }
                let _ = msg_tx.send(NetMsg::Status(Ok(st)));
            }
        }
        if !board.mac.is_empty() {
            let id = mac_worker_id(&board.mac);
            log_msg(
                msg_tx,
                LogKind::Usb,
                format!(
                    "Board {} identity {} ({})",
                    board.name,
                    board.mac,
                    if id.is_empty() { "port" } else { &id }
                ),
            );
        }
    }

    /// Arm mining job on a newly linked board.
    fn arm_mining_if_needed(
        board: &mut UsbBoard,
        msg_tx: &Sender<NetMsg>,
        resume_mining: bool,
    ) {
        if board.download_mode || !resume_mining {
            return;
        }
        let mut legacy = board.legacy_job;
        let _ = usb_cmd(
            &mut board.port,
            &mut board.rx,
            "cmp stats accepted=0&rejected=0",
        );
        let _ = usb_push_job(
            &mut board.port,
            &mut board.rx,
            &warmup_job(),
            &mut legacy,
            msg_tx,
        );
        board.legacy_job = legacy;
        board.mining = true;
    }

    let mut boards: Vec<UsbBoard> = Vec::new();
    let mut stratum: Option<StratumClient> = None;
    let mut recent_jobs: VecDeque<WorkJob> = VecDeque::new();
    // Board shares held while the pool socket is down (submit on reconnect).
    let mut held_board_shares: VecDeque<(String, String, String, String)> = VecDeque::new();
    let mut mining = false;
    let mut last_stats_push = Instant::now() - Duration::from_secs(10);
    let mut last_stratum_ui = Instant::now() - Duration::from_secs(10);
    let mut last_fleet_status = StatusJson::default();
    let mut mine_endpoint = String::new();
    let mut mine_worker_name = String::new();
    let mut mine_password = String::new();
    let mut reconnect_at: Option<Instant> = None;
    let mut reconnect_backoff = Duration::from_secs(2);

    loop {
        let cmd = if mining || !boards.is_empty() {
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
                    let ports = list_serial_ports();
                    let _ = msg_tx.send(NetMsg::Ports(ports));
                }
                NetCmd::ScanWorkers => {
                    // Probe on a side thread so mining / UI stay responsive while
                    // each COM settles (ESP32 boot after open can take >1s).
                    let skip: Vec<String> = boards.iter().map(|b| b.name.clone()).collect();
                    let msg_tx_scan = msg_tx.clone();
                    thread::spawn(move || {
                        log_msg(
                            &msg_tx_scan,
                            LogKind::Usb,
                            format!(
                                "USB CYD scan (skip PCI; skipping {} open)…",
                                skip.len()
                            ),
                        );
                        let msg_probe = msg_tx_scan.clone();
                        let mut found =
                            scan_usb_workers_with_progress(&skip, move |port, detail| {
                                if detail == "probing" || detail.starts_with("probing ") {
                                    log_msg(
                                        &msg_probe,
                                        LogKind::Usb,
                                        format!("Probing {port}…"),
                                    );
                                } else if detail.starts_with("ok") {
                                    log_msg(
                                        &msg_probe,
                                        LogKind::Usb,
                                        format!("{port}: {detail}"),
                                    );
                                } else {
                                    log_msg(
                                        &msg_probe,
                                        LogKind::Warn,
                                        format!("{port}: {detail}"),
                                    );
                                }
                            });
                        log_msg(
                            &msg_tx_scan,
                            LogKind::Usb,
                            format!(
                                "USB probe finished · {} CYD worker(s) answering (Wi‑Fi from live beacons)",
                                found.len()
                            ),
                        );
                        let _ = msg_tx_scan.send(NetMsg::WorkersFound(found));
                    });
                }
                NetCmd::OpenUsb { name } => {
                    // Do not wipe the whole fleet — reconnect/replace this port only
                    // so multi-board setups survive a primary Connect click.
                    if let Some(idx) = boards
                        .iter()
                        .position(|b| port_names_match(&b.name, &name))
                    {
                        let mut old = boards.remove(idx);
                        let _ = usb_cmd(&mut old.port, &mut old.rx, "cmp stop");
                    }
                    log_msg(&msg_tx, LogKind::Usb, format!("Opening {name} (460800→115200)"));
                    match open_board(&name) {
                        Ok((mut board, saw)) => {
                            let dl = board.download_mode;
                            configure_board(&mut board, &msg_tx);
                            arm_mining_if_needed(&mut board, &msg_tx, mining);
                            boards.push(board);
                            publish_live(&msg_tx, &boards);
                            let _ = msg_tx.send(NetMsg::Action(Ok(if dl {
                                format!(
                                    "USB open {name} (download mode / BOOT) · flash with Update board · {} board(s)",
                                    boards.len()
                                )
                            } else {
                                format!(
                                    "USB open {name}{} · {} board(s)",
                                    if saw { " (pong)" } else { "" },
                                    boards.len()
                                )
                            })));
                        }
                        Err(e) => {
                            publish_live(&msg_tx, &boards);
                            let _ = msg_tx.send(NetMsg::Action(Err(e)));
                        }
                    }
                }
                NetCmd::ConnectWorker(name) => {
                    if boards.iter().any(|b| port_names_match(&b.name, &name)) {
                        let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                            "Worker {name} already linked"
                        ))));
                        publish_live(&msg_tx, &boards);
                        continue;
                    }
                    // Pause jobs on already-linked boards while we open another COM —
                    // shared USB hubs often brown out / reset both CYDs mid-mine.
                    let resume_mine = mining;
                    if !boards.is_empty() {
                        log_msg(
                            &msg_tx,
                            LogKind::Usb,
                            format!(
                                "Pausing {} board(s) while linking {name}…",
                                boards.len()
                            ),
                        );
                        for b in boards.iter_mut() {
                            let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                        }
                        thread::sleep(Duration::from_millis(400));
                    }
                    log_msg(&msg_tx, LogKind::Usb, format!("Connecting worker {name}"));
                    match open_board(&name) {
                        Ok((mut board, saw)) => {
                            let dl = board.download_mode;
                            configure_board(&mut board, &msg_tx);
                            // Same eFuse MAC on another COM: replace only if that COM
                            // vanished (re-enumeration). Never eject a live 2nd farm board.
                            if mac_is_stable(&board.mac) {
                                if let Some(idx) = boards.iter().position(|b| {
                                    mac_is_stable(&b.mac)
                                        && normalize_mac(&b.mac) == normalize_mac(&board.mac)
                                }) {
                                    let old_name = boards[idx].name.clone();
                                    let old_still_listed = serialport::available_ports()
                                        .unwrap_or_default()
                                        .iter()
                                        .any(|p| port_names_match(&p.port_name, &old_name));
                                    if old_still_listed && !port_names_match(&old_name, &name) {
                                        log_msg(
                                            &msg_tx,
                                            LogKind::Warn,
                                            format!(
                                                "Keep {} — same MAC {} as {name} but both COMs present (two boards?)",
                                                old_name, board.mac
                                            ),
                                        );
                                    } else {
                                        let old = boards.remove(idx);
                                        log_msg(
                                            &msg_tx,
                                            LogKind::Usb,
                                            format!(
                                                "Replaced {} with {} (same MAC {})",
                                                old.name, name, board.mac
                                            ),
                                        );
                                    }
                                }
                            }
                            arm_mining_if_needed(&mut board, &msg_tx, resume_mine);
                            boards.push(board);
                            if resume_mine {
                                for b in boards.iter_mut() {
                                    arm_mining_if_needed(b, &msg_tx, true);
                                }
                            }
                            publish_live(&msg_tx, &boards);
                            let _ = msg_tx.send(NetMsg::Action(Ok(if dl {
                                format!(
                                    "Worker linked {name} (download mode / BOOT) · Update board to flash · {} total",
                                    boards.len()
                                )
                            } else {
                                format!(
                                    "Worker linked {name}{} · {} total",
                                    if saw { " (pong)" } else { "" },
                                    boards.len()
                                )
                            })));
                        }
                        Err(e) => {
                            if resume_mine {
                                for b in boards.iter_mut() {
                                    arm_mining_if_needed(b, &msg_tx, true);
                                }
                            }
                            publish_live(&msg_tx, &boards);
                            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                "Link {name} failed: {e}"
                            ))));
                        }
                    }
                }
                NetCmd::ConnectWifi(endpoint) => {
                    if boards.iter().any(|b| b.name == endpoint) {
                        let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                            "Wi‑Fi worker {endpoint} already linked"
                        ))));
                        publish_live(&msg_tx, &boards);
                        continue;
                    }
                    log_msg(
                        &msg_tx,
                        LogKind::Usb,
                        format!("Connecting Wi‑Fi worker {endpoint}"),
                    );
                    match open_wifi_board(&endpoint) {
                        Ok((mut board, saw)) => {
                            configure_board(&mut board, &msg_tx);
                            // Prefer keeping USB when the same MAC is already linked —
                            // SoftAP Wi‑Fi must not eject a working serial board.
                            if mac_is_stable(&board.mac) {
                                if let Some(existing) = boards.iter().find(|b| {
                                    mac_is_stable(&b.mac)
                                        && normalize_mac(&b.mac) == normalize_mac(&board.mac)
                                }) {
                                    let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                        "Skip Wi‑Fi {endpoint} — already linked as {} (same MAC {})",
                                        existing.name, board.mac
                                    ))));
                                    publish_live(&msg_tx, &boards);
                                    continue;
                                }
                            }
                            arm_mining_if_needed(&mut board, &msg_tx, mining);
                            boards.push(board);
                            publish_live(&msg_tx, &boards);
                            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                "Wi‑Fi worker linked {endpoint}{} · {} total",
                                if saw { " (pong)" } else { "" },
                                boards.len()
                            ))));
                        }
                        Err(e) => {
                            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                "Wi‑Fi link {endpoint} failed: {e}"
                            ))));
                        }
                    }
                }
                NetCmd::DisconnectWorker(name) => {
                    if let Some(idx) = boards
                        .iter()
                        .position(|b| port_names_match(&b.name, &name))
                    {
                        let mut b = boards.remove(idx);
                        let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                        publish_live(&msg_tx, &boards);
                        let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                            "Worker dropped {name} · {} remain",
                            boards.len()
                        ))));
                        if boards.is_empty() {
                            mining = false;
                            if let Some(mut s) = stratum.take() {
                                s.disconnect();
                            }
                        }
                    } else {
                        let _ = msg_tx
                            .send(NetMsg::Action(Err(format!("Worker {name} not linked"))));
                    }
                }
                NetCmd::CloseUsb => {
                    mining = false;
                    reconnect_at = None;
                    mine_endpoint.clear();
                    if let Some(mut s) = stratum.take() {
                        s.disconnect();
                    }
                    for b in boards.iter_mut() {
                        let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                        b.mining = false;
                        b.hashrate_hs = 0.0;
                    }
                    boards.clear();
                    publish_live(&msg_tx, &boards);
                    let _ = msg_tx.send(NetMsg::Status(Ok(StatusJson::default())));
                    let _ = msg_tx.send(NetMsg::Action(Ok("USB closed".into())));
                }
                NetCmd::StartMine {
                    stratum: endpoint,
                    worker,
                    password,
                } => {
                    if boards.is_empty() {
                        let _ = msg_tx.send(NetMsg::Action(Err("USB not open".into())));
                        continue;
                    }
                    mine_endpoint = endpoint.clone();
                    mine_worker_name = worker.clone();
                    mine_password = password.clone();
                    reconnect_backoff = Duration::from_secs(2);
                    reconnect_at = None;
                    for b in boards.iter_mut() {
                        let _ = usb_cmd(
                            &mut b.port,
                            &mut b.rx,
                            "cmp stats accepted=0&rejected=0",
                        );
                        let mut legacy = b.legacy_job;
                        match usb_push_job(
                            &mut b.port,
                            &mut b.rx,
                            &warmup_job(),
                            &mut legacy,
                            &msg_tx,
                        ) {
                            Ok(_) => {
                                b.legacy_job = legacy;
                                let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                    "Board {} hashing (warmup)…",
                                    b.name
                                ))));
                            }
                            Err(e) => {
                                b.legacy_job = legacy;
                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                    "Warmup {} failed: {e}",
                                    b.name
                                ))));
                            }
                        }
                    }
                    log_msg(
                        &msg_tx,
                        LogKind::Stratum,
                        format!(
                            "Connecting pool {endpoint} · {} worker(s)",
                            boards.len()
                        ),
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
                                "Pool connecting {endpoint} — {} board(s) hashing",
                                boards.len()
                            ))));
                        }
                        Err(e) => {
                            mining = true;
                            stratum = None;
                            reconnect_at = Some(Instant::now() + reconnect_backoff);
                            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                "Pool error (boards still hashing; will retry): {e}"
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
                    for b in boards.iter_mut() {
                        let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                        b.mining = false;
                        b.hashrate_hs = 0.0;
                    }
                    publish_live(&msg_tx, &boards);
                    let _ = msg_tx.send(NetMsg::Status(Ok(StatusJson {
                        mining: false,
                        connected: false,
                        hashrate_hs: 0.0,
                        hashrate_khs: 0.0,
                        ..Default::default()
                    })));
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
                    let mut any = false;
                    for b in boards.iter_mut() {
                        let cmd = format!("cmp clock cpu_mhz={mhz}");
                        if usb_cmd(&mut b.port, &mut b.rx, &cmd).is_ok() {
                            any = true;
                        }
                    }
                    let _ = msg_tx.send(NetMsg::Action(if any {
                        Ok(format!("Clock {mhz} MHz queued on {} board(s)", boards.len()))
                    } else {
                        Err("USB not open".into())
                    }));
                }
                NetCmd::PollStatus => {
                    if boards.is_empty() {
                        last_fleet_status = StatusJson::default();
                        let _ = msg_tx.send(NetMsg::Status(Ok(StatusJson::default())));
                        continue;
                    }
                    let mut total_hs = 0.0;
                    let mut total_hashes = 0u64;
                    let mut any_mining = false;
                    let mut last_status: Option<StatusJson> = None;
                    let mut drop_names: Vec<String> = Vec::new();
                    for b in boards.iter_mut() {
                        if b.download_mode {
                            // ROM bootloader — no cmp; keep linked for Update board only.
                            continue;
                        }
                        harvest_shares(
                            &mut b.port,
                            &mut b.rx,
                            stratum.as_mut(),
                            &recent_jobs,
                            &mut held_board_shares,
                            &msg_tx,
                        );
                        match usb_cmd(&mut b.port, &mut b.rx, "cmp status") {
                            Ok(line) => match parse_cmp_status(&line) {
                                Ok(st) => {
                                    b.status_fails = 0;
                                    // Hold last rate across empty EMA windows / job edges.
                                    if st.hashrate_hs > 0.0 {
                                        b.hashrate_hs = st.hashrate_hs;
                                    } else if !(mining || b.mining) {
                                        b.hashrate_hs = 0.0;
                                    }
                                    // Counters only rise while the fleet is mining.
                                    if mining {
                                        b.hashes = b.hashes.max(st.hashes);
                                    } else if st.hashes > 0 || !b.mining {
                                        b.hashes = st.hashes;
                                    }
                                    b.mining = st.mining || (mining && b.hashrate_hs > 0.0);
                                    if !st.mac.is_empty() {
                                        b.mac = normalize_mac(&st.mac);
                                    }
                                    total_hs += b.hashrate_hs;
                                    total_hashes = total_hashes.saturating_add(b.hashes);
                                    any_mining |= b.mining;
                                    last_status = Some(st);
                                }
                                Err(e) => {
                                    b.status_fails = b.status_fails.saturating_add(1);
                                    // Hold last known rate so multi-board totals don't dip.
                                    total_hs += b.hashrate_hs;
                                    total_hashes = total_hashes.saturating_add(b.hashes);
                                    any_mining |= b.mining || mining;
                                    if b.status_fails == 1 || b.status_fails % 5 == 0 {
                                        log_msg(
                                            &msg_tx,
                                            LogKind::Warn,
                                            format!("status {}: {e}", b.name),
                                        );
                                    }
                                    if b.status_fails >= 12 {
                                        drop_names.push(b.name.clone());
                                    }
                                }
                            },
                            Err(e) => {
                                b.status_fails = b.status_fails.saturating_add(1);
                                total_hs += b.hashrate_hs;
                                total_hashes = total_hashes.saturating_add(b.hashes);
                                any_mining |= b.mining || mining;
                                // Soft-fails are common under hash load; only log sparsely.
                                if b.status_fails == 1 || b.status_fails % 5 == 0 {
                                    log_msg(
                                        &msg_tx,
                                        LogKind::Warn,
                                        format!(
                                            "status soft-fail {} (#{}) : {e}",
                                            b.name, b.status_fails
                                        ),
                                    );
                                }
                                // Need a longer streak before dropping — USB can stall briefly.
                                if b.status_fails >= 12 {
                                    drop_names.push(b.name.clone());
                                }
                            }
                        }
                    }
                    for name in drop_names {
                        if let Some(idx) = boards.iter().position(|b| b.name == name) {
                            let mut dead = boards.remove(idx);
                            let _ = usb_cmd(&mut dead.port, &mut dead.rx, "cmp stop");
                            log_msg(
                                &msg_tx,
                                LogKind::Err,
                                format!("Dropped dead board {name} after status failures"),
                            );
                        }
                    }
                    if boards.is_empty() {
                        mining = false;
                        reconnect_at = None;
                        last_fleet_status = StatusJson::default();
                        if let Some(mut s) = stratum.take() {
                            s.disconnect();
                        }
                        publish_live(&msg_tx, &boards);
                        let _ = msg_tx.send(NetMsg::Status(Ok(StatusJson::default())));
                        continue;
                    }
                    publish_live(&msg_tx, &boards);
                    let mut st = last_status.unwrap_or_else(|| last_fleet_status.clone());
                    st.hashrate_hs = total_hs;
                    st.hashrate_khs = total_hs / 1000.0;
                    st.hashes = total_hashes.max(last_fleet_status.hashes);
                    st.mining = any_mining || mining;
                    st.connected = true;
                    // Preserve identity fields when this round was all soft-fails.
                    if st.job.is_empty() {
                        st.job = last_fleet_status.job.clone();
                    }
                    if st.nonce.is_empty() {
                        st.nonce = last_fleet_status.nonce.clone();
                    }
                    if st.sha_mode.is_empty() {
                        st.sha_mode = last_fleet_status.sha_mode.clone();
                    }
                    if st.mac.is_empty() {
                        st.mac = last_fleet_status.mac.clone();
                    }
                    if st.pool.is_empty() {
                        st.pool = last_fleet_status.pool.clone();
                    }
                    if st.uptime_secs == 0 {
                        st.uptime_secs = last_fleet_status.uptime_secs;
                    }
                    if st.cpu_mhz == 0 {
                        st.cpu_mhz = last_fleet_status.cpu_mhz;
                    }
                    last_fleet_status = st.clone();
                    let _ = msg_tx.send(NetMsg::Status(Ok(st)));
                }
                NetCmd::Bench => {
                    if boards.is_empty() {
                        let _ = msg_tx.send(NetMsg::Action(Err("No boards linked".into())));
                        continue;
                    }
                    let was_mining = mining;
                    let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                        "Bench running on {} board(s)…",
                        boards.len()
                    ))));
                    // Pause pool hashing so each board can retune HW paths cleanly.
                    if was_mining {
                        for b in boards.iter_mut() {
                            let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                        }
                    }
                    let mut lines = Vec::new();
                    for b in boards.iter_mut() {
                        log_msg(
                            &msg_tx,
                            LogKind::Usb,
                            format!("Tuning {} for max hashrate…", b.name),
                        );
                        // n≈60k → ~20k hashes/path after firmware split (finishes well under USB wait).
                        match usb_cmd(&mut b.port, &mut b.rx, "cmp bench tune=1&n=60000") {
                            Ok(line) => {
                                lines.push(format!("{} → {line}", b.name));
                                log_msg(&msg_tx, LogKind::Usb, format!("{} bench OK: {line}", b.name));
                                if let Ok(st_line) =
                                    usb_cmd(&mut b.port, &mut b.rx, "cmp status")
                                {
                                    if let Ok(st) = parse_cmp_status(&st_line) {
                                        b.hashrate_hs = st.hashrate_hs;
                                        b.hashes = st.hashes;
                                        b.mining = st.mining;
                                        let _ = msg_tx.send(NetMsg::Status(Ok(st)));
                                    }
                                }
                            }
                            Err(e) => {
                                lines.push(format!("{} → ERR {e}", b.name));
                                log_msg(
                                    &msg_tx,
                                    LogKind::Warn,
                                    format!("{} bench failed: {e}", b.name),
                                );
                            }
                        }
                    }
                    if was_mining {
                        // Resume the last real pool job — warmup shares are ignored by
                        // harvest and starve accepts until the next mining.notify.
                        let resume = recent_jobs.back().cloned();
                        for b in boards.iter_mut() {
                            let mut legacy = b.legacy_job;
                            let _ = usb_cmd(
                                &mut b.port,
                                &mut b.rx,
                                "cmp stats accepted=0&rejected=0",
                            );
                            let job = resume.as_ref().cloned().unwrap_or_else(warmup_job);
                            match usb_push_job(
                                &mut b.port,
                                &mut b.rx,
                                &job,
                                &mut legacy,
                                &msg_tx,
                            ) {
                                Ok(_) => {
                                    b.legacy_job = legacy;
                                    if resume.is_some() {
                                        log_msg(
                                            &msg_tx,
                                            LogKind::Usb,
                                            format!(
                                                "{} resumed pool job {} after Bench",
                                                b.name, job.job_id
                                            ),
                                        );
                                    }
                                }
                                Err(e) => {
                                    b.legacy_job = legacy;
                                    log_msg(
                                        &msg_tx,
                                        LogKind::Warn,
                                        format!("{} resume after Bench failed: {e}", b.name),
                                    );
                                }
                            }
                        }
                    }
                    publish_live(&msg_tx, &boards);
                    let summary = if lines.is_empty() {
                        "Bench done".into()
                    } else {
                        lines.join(" · ")
                    };
                    let _ = msg_tx.send(NetMsg::Action(Ok(summary)));
                }
                NetCmd::UsbRaw(cmd) => {
                    if let Some(b) = boards.first_mut() {
                        match usb_cmd(&mut b.port, &mut b.rx, &cmd) {
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
                    cancel,
                    need_boot,
                    boot_ready,
                    live_push,
                } => {
                    // Only release the flash target COM — keep other linked boards.
                    if let Some(idx) = boards
                        .iter()
                        .position(|b| port_names_match(&b.name, &port))
                    {
                        let mut b = boards.remove(idx);
                        let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                    }
                    if boards.is_empty() {
                        mining = false;
                        if let Some(mut s) = stratum.take() {
                            s.disconnect();
                        }
                    } else {
                        // Pause pool jobs on remaining boards while flash runs.
                        for b in boards.iter_mut() {
                            let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                        }
                    }
                    publish_live(&msg_tx, &boards);
                    log_msg(
                        &msg_tx,
                        LogKind::Usb,
                        format!(
                            "Released {port} for {} · {} board(s) still linked",
                            if live_push { "push update" } else { "flash" },
                            boards.len()
                        ),
                    );
                    thread::sleep(Duration::from_millis(if live_push { 900 } else { 1800 }));

                    // Run flash off the mine-worker so Cancel / port list keep working.
                    let progress_tx = msg_tx.clone();
                    let done_tx = msg_tx.clone();
                    thread::spawn(move || {
                        let progress = move |line: String| {
                            let _ = progress_tx.send(NetMsg::FlashProgress(line));
                        };
                        let result = (|| {
                            if cancel.load(Ordering::SeqCst) {
                                return Err("flash cancelled".into());
                            }
                            let img = {
                                let local = std::path::PathBuf::from(&image);
                                if !image.is_empty() && local.is_file() {
                                    let bytes =
                                        std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
                                    let version = flash_update::read_nearby_fw_version(&local);
                                    FirmwareImage {
                                        path: local,
                                        bytes,
                                        version,
                                    }
                                } else {
                                    ensure_firmware_image(&progress)?
                                }
                            };
                            let _ = done_tx.send(NetMsg::FirmwareFetched(Ok(img.clone())));
                            let ctrl = FlashControl {
                                cancel,
                                need_boot,
                                boot_ready,
                            };
                            flash_merged_bin(&port, &img.path, &progress, &ctrl, live_push)?;
                            Ok(format!(
                                "Firmware {} {} on {port}",
                                if img.version.is_empty() {
                                    "image".into()
                                } else {
                                    img.version
                                },
                                if live_push { "pushed" } else { "flashed" }
                            ))
                        })();
                        let reopen_port = if reopen { Some(port) } else { None };
                        let _ = done_tx.send(NetMsg::FlashDone {
                            result,
                            reopen: reopen_port,
                        });
                    });
                }
                NetCmd::PushNet { text } => {
                    for b in boards.iter_mut() {
                        let cmd = format!("cmp netdata text={}", urlenc(&text));
                        let _ = usb_cmd(&mut b.port, &mut b.rx, &cmd);
                    }
                }
                NetCmd::RebootBoard => {
                    if let Some(b) = boards.first_mut() {
                        match usb_cmd(&mut b.port, &mut b.rx, "cmp reboot") {
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
                NetCmd::PullApiFeed(feed) => {
                    let outcome = pull_feed(&feed);
                    let _ = msg_tx.send(NetMsg::ApiFeedResult(outcome));
                }
            }
        }

        // Pull board shares BEFORE polling the pool so submits leave ASAP
        // (cuts measured share→accept latency).
        if !boards.is_empty() {
            for b in boards.iter_mut() {
                harvest_shares(
                    &mut b.port,
                    &mut b.rx,
                    stratum.as_mut(),
                    &recent_jobs,
                    &mut held_board_shares,
                    &msg_tx,
                );
            }
        }

        if let Some(client) = stratum.as_mut() {
            let was_authorized = client.authorized();
            // Drain pool socket aggressively — short read timeout, multiple passes.
            let mut poll_err: Option<String> = None;
            for _ in 0..4 {
                match client.poll() {
                    Ok(()) => {}
                    Err(e) => {
                        poll_err = Some(e);
                        break;
                    }
                }
            }
            match poll_err {
                None => {
                    if client.authorized() && !was_authorized {
                        push_stratum_live(&msg_tx, client);
                        let _ = msg_tx.send(NetMsg::MineStats {
                            accepted: 0,
                            rejected: 0,
                            phase: client.phase.clone(),
                        });
                        for b in boards.iter_mut() {
                            let _ = usb_cmd(
                                &mut b.port,
                                &mut b.rx,
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
                        let mut pushed = 0usize;
                        for b in boards.iter_mut() {
                            let mut legacy = b.legacy_job;
                            match usb_push_job(
                                &mut b.port,
                                &mut b.rx,
                                &job,
                                &mut legacy,
                                &msg_tx,
                            ) {
                                Ok(_) => {
                                    b.legacy_job = legacy;
                                    pushed += 1;
                                }
                                Err(e) => {
                                    b.legacy_job = legacy;
                                    let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                        "{} job push: {e}",
                                        b.name
                                    ))));
                                }
                            }
                        }
                        if pushed > 0 {
                            recent_jobs.push_back(job.clone());
                            while recent_jobs.len() > 32 {
                                recent_jobs.pop_front();
                            }
                            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                "USB ← job {} → {pushed} board(s)",
                                job.job_id
                            ))));
                        } else {
                            // Don't drop the only copy — retry next loop tick.
                            client.restore_job(job);
                        }
                    }
                    if last_stats_push.elapsed() > Duration::from_secs(2) {
                        let (a, r) = if client.authorized() {
                            (client.accepted, client.rejected)
                        } else {
                            (0, 0)
                        };
                        let cmd = format!("cmp stats accepted={a}&rejected={r}");
                        for b in boards.iter_mut() {
                            // Skip LCD stats push while status is soft-failing — frees USB.
                            if b.status_fails > 0 {
                                continue;
                            }
                            let _ = usb_cmd(&mut b.port, &mut b.rx, &cmd);
                        }
                        last_stats_push = Instant::now();
                    }
                }
                Some(e) => {
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
                    thread::sleep(Duration::from_millis(100));
                }
            }
            if let Some(client) = stratum.as_mut() {
                if last_stratum_ui.elapsed() > Duration::from_millis(150) {
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
                    format!("Reconnecting pool {mine_endpoint}"),
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
                                "Reconnect failed: {e} — retry in {}s",
                                reconnect_backoff.as_secs().max(1)
                            ),
                        );
                        reconnect_backoff =
                            (reconnect_backoff * 2).min(Duration::from_secs(60));
                    }
                }
            }
        }

        // Idle pause only — keep the hot path tight while mining / boards linked.
        if mining || stratum.is_some() || !boards.is_empty() {
            thread::sleep(Duration::from_millis(1));
        } else {
            thread::sleep(Duration::from_millis(8));
        }
    }
}

fn harvest_shares(
    port: &mut BoardIo,
    buf: &mut String,
    stratum: Option<&mut StratumClient>,
    recent_jobs: &VecDeque<WorkJob>,
    held: &mut VecDeque<(String, String, String, String)>,
    msg_tx: &Sender<NetMsg>,
) {
    port.drain(buf);
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
        let mut pending: Vec<(String, String, String, String)> =
            std::mem::take(held).into_iter().collect();
        pending.extend(shares);
        for (job, en2, ntime, nonce) in pending {
            match try_submit_board_share(s, recent_jobs, &job, &en2, &ntime, &nonce, msg_tx) {
                ShareSubmitResult::Ok | ShareSubmitResult::Dropped => {}
                ShareSubmitResult::Hold => {
                    held.push_back((job, en2, ntime, nonce));
                    while held.len() > 64 {
                        held.pop_front();
                    }
                }
            }
        }
    } else {
        let before = held.len();
        for (job, en2, ntime, nonce) in shares {
            if job == "warmup" || job.is_empty() {
                continue;
            }
            held.push_back((job, en2, ntime, nonce));
            while held.len() > 64 {
                held.pop_front();
            }
        }
        if held.len() > before {
            log_msg(
                msg_tx,
                LogKind::Info,
                format!(
                    "Holding {} board share(s) until pool reconnects",
                    held.len()
                ),
            );
        }
    }
}

enum ShareSubmitResult {
    Ok,
    Dropped,
    Hold,
}

fn try_submit_board_share(
    s: &mut StratumClient,
    recent_jobs: &VecDeque<WorkJob>,
    job: &str,
    en2: &str,
    ntime: &str,
    nonce: &str,
    msg_tx: &Sender<NetMsg>,
) -> ShareSubmitResult {
    if job == "warmup" || job.is_empty() {
        log_msg(
            msg_tx,
            LogKind::Info,
            format!("Ignoring local/warmup share nonce={nonce}"),
        );
        return ShareSubmitResult::Dropped;
    }
    if let Some(wj) = recent_jobs
        .iter()
        .rev()
        .find(|j| j.job_id == job && j.extranonce2_hex == en2)
    {
        if let Err(e) = StratumClient::verify_share_against_job(wj, nonce) {
            log_msg(
                msg_tx,
                LogKind::Warn,
                format!("Dropping bad board share nonce={nonce} job={job}: {e}"),
            );
            return ShareSubmitResult::Dropped;
        }
    } else {
        log_msg(
            msg_tx,
            LogKind::Warn,
            format!(
                "Dropping stale board share job={job} en2={en2} (not in recent job cache)"
            ),
        );
        return ShareSubmitResult::Dropped;
    }
    match s.submit_share(job, en2, ntime, nonce) {
        Ok(()) => {
            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                "Share submitted {nonce} job={job}"
            ))));
            ShareSubmitResult::Ok
        }
        Err(e) if e.contains("duplicate share") => {
            log_msg(
                msg_tx,
                LogKind::Warn,
                format!("Duplicate share skipped {nonce} job={job}"),
            );
            ShareSubmitResult::Dropped
        }
        Err(e) if e.contains("not authorized") => {
            log_msg(
                msg_tx,
                LogKind::Info,
                format!("Holding share {nonce} until authorize"),
            );
            ShareSubmitResult::Hold
        }
        Err(e) => {
            let _ = msg_tx.send(NetMsg::Action(Err(format!("Share submit: {e}"))));
            ShareSubmitResult::Dropped
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

fn usb_cmd(port: &mut BoardIo, buf: &mut String, cmd: &str) -> Result<String, String> {
    let mut last_err = String::new();
    // Bigger writes + single flush cut job-push latency a lot vs per-chunk sleeps.
    let (wait_ms, retries, chunk, gap_ms) = if cmd.contains("bench") {
        // One long wait — retrying restarts a board that may still be mid-tune.
        (180_000u64, 1usize, 128usize, 1u64)
    } else if cmd.contains("status") {
        // Board may be mid mineB batch; firmware yields on RX, but allow headroom.
        (2_800u64, 3usize, 256usize, 0u64)
    } else if cmd.contains("stats") {
        // Never stall the mine loop waiting on LCD stats ACKs.
        (450u64, 1usize, 256usize, 0u64)
    } else if cmd.contains(" jh") || cmd.contains(" jt") || cmd.contains(" ja") {
        (3_000u64, 3usize, 256usize, 0u64)
    } else if cmd.contains(" job ") {
        (4_000u64, 2usize, 128usize, 1u64)
    } else {
        (2_500u64, 3usize, 256usize, 0u64)
    };
    for _ in 0..retries {
        port.drain(buf);
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
            port.write_all(piece)?;
            if gap_ms > 0 {
                thread::sleep(Duration::from_millis(gap_ms));
            }
        }
        let _ = port.flush();
        let deadline = Instant::now() + Duration::from_millis(wait_ms);
        while Instant::now() < deadline {
            port.drain(buf);
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
            thread::sleep(Duration::from_millis(2));
        }
        last_err = format!(
            "USB timeout waiting for reply to `{}`",
            cmd.chars().take(56).collect::<String>()
        );
        thread::sleep(Duration::from_millis(15));
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
    port: &mut BoardIo,
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
