//! Njörðr seas CYD miner — USB / Wi‑Fi worker control.
//! Boards mine independently to the pool when STA+pool are set; Companion monitors
//! H/s and can feed USB/Wi‑Fi jobs only while a board is not yet indep-authorized.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api_feeds;
mod app_update;
mod desktop_icon;
mod flash_update;
mod live_bar;
mod monitor_api;
mod stratum;
mod utf8_safe;
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
use app_update::{
    check_app_update_ex, running_version, update_companion_app_ex, AppRemoteInfo,
};
use flash_update::{
    board_fw_is_d0, ensure_firmware_image, erase_flash_bin, fetch_latest_firmware, find_firmware_image,
    firmware_is_custom, flash_merged_bin, load_firmware_bin, nudge_usb_reboot_for_push,
    push_firmware_ota_ex, push_firmware_ota_on_port_recover_ex, push_firmware_ota_usb_ex,
    resolve_ota_app_image_ex, send_cmp_reboot, update_needed,
    FirmwareImage, FlashControl, OtaReopen,
};
use live_bar::{
    default_header_coins, format_change, format_usd, COIN_CATALOG, LiveFeed,
};
use monitor_api::{
    generate_install_id, generate_token, pair_url, primary_lan_ipv4, softap_setup_client_ipv4, qr_modules,
    start as start_monitor_api, web_pair_url, MonitorBoard, MonitorCreds, MonitorHub,
    MonitorSnapshot, MONITOR_PORT,
};
use stratum::{
    encode_job_cmd, encode_job_parts, expected_shares_per_hour, target_from_difficulty, urlenc,
    ShareOutcome, StratumClient, WorkJob,
};
use workers::{
    count_usb_uart_ports, cyd_port_score, flashable_ports, is_usb_serial_port, list_serial_ports,
    mac_is_stable, mac_worker_id, normalize_mac, normalize_port_name, open_usb_serial_timed,
    open_wifi_tcp, port_choice_is_pci, port_choice_is_system_junk, port_names_match,
    prefer_cyd_port, prefer_cyd_port_excluding, probe_esp_download_mode,
    scan_usb_workers_with_progress, transport_mac_id, wifi_beacon_fresh, wifi_is_softap_setup,
    BoardWifiDiscovery, DiscoveredWorker, LanDiscovery, PortChoice, WorkerKind, WorkerLive,
};

use eframe::egui::{
    self, Align, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, IconData, Layout,
    Margin, Pos2, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, TextEdit, Vec2,
};
use eframe::{App, NativeOptions};
use serde::{Deserialize, Serialize};
use serialport::SerialPort;

/// Set when Update board / Push is clicked — mine-worker aborts blocking USB/`via`
/// waits so flash can take the COM immediately (avoids stuck "auto-reset…" UI).
static USB_FLASH_PREEMPT: AtomicBool = AtomicBool::new(false);
/// UI Cancel / watchdog — abort a hung `cmp bench` wait without killing the process.
static USB_BENCH_CANCEL: AtomicBool = AtomicBool::new(false);

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
    install_crash_log_hook();
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
        // MSAA 8 crashes OpenGL startup on some Intel/AMD Windows drivers.
        multisampling: 0,
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

/// Write panics next to the exe so OneDrive/update crashes are diagnosable.
fn install_crash_log_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("{info}");
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".into());
        let body = format!(
            "Njörðr seas CYD miner {} panic\n{loc}\n{msg}\n",
            env!("CARGO_PKG_VERSION")
        );
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let _ = std::fs::write(dir.join("cyd-companion-crash.log"), &body);
            }
        }
        eprintln!("{body}");
    }));
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
const MAX_LOGS: usize = 800;
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
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let ms = dur.subsec_millis();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
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
}

/// Sub-pages under Settings (board tools + general).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    General,
    Setup,
    Flash,
    Erase,
    Reset,
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
    /// mining.submit lines sent (Pool TX also counts subscribe/auth/suggest).
    /// Session-scoped — resets with Accept/Reject on authorize.
    submits: u64,
    /// mining.submit still waiting for a pool reply.
    pending: u32,
    last_job: String,
    last_rx: String,
    last_tx: String,
    /// Authorize / subscribe failure detail for the Live stratum panel.
    last_error: String,
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
    #[serde(default)]
    mine_indep: bool,
    #[serde(default)]
    pool_ep: String,
    #[serde(default)]
    pool_phase: String,
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
    #[serde(default)]
    wifi_en: Option<bool>,
    #[serde(default)]
    wifi_ssid: String,
    #[serde(default)]
    wifi_mode: String,
    #[serde(default)]
    wifi_ip: String,
    #[serde(default)]
    wifi_ap: String,
    #[serde(default)]
    wifi_tcp: u16,
}

#[derive(Debug, Clone)]
enum WifiSetupPhase {
    Idle,
    Pushing,
    /// Waiting for board to join home Wi‑Fi (beacon/mode flip).
    WaitingSta {
        since: Instant,
        endpoint: String,
        mac: String,
        home_ssid: String,
        /// Throttle progress lines in the event log.
        last_progress_log: Instant,
    },
    Done {
        ip: String,
        ssid: String,
    },
    Failed(String),
}

impl Default for WifiSetupPhase {
    fn default() -> Self {
        Self::Idle
    }
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

/// ESP32 `setCpuFrequencyMhz` lock points (Arduino-ESP32).
const CPU_MHZ_STEPS: &[u8] = &[10, 20, 40, 80, 160, 240];

fn normalize_cpu_mhz(mhz: u8) -> u8 {
    if mhz == 0 {
        return 240;
    }
    CPU_MHZ_STEPS
        .iter()
        .copied()
        .min_by_key(|&s| (s as i16 - mhz as i16).unsigned_abs())
        .unwrap_or(240)
}

fn cpu_mhz_nudge(current: u8, up: bool) -> u8 {
    let cur = normalize_cpu_mhz(current);
    if let Some(i) = CPU_MHZ_STEPS.iter().position(|&s| s == cur) {
        if up {
            CPU_MHZ_STEPS[(i + 1).min(CPU_MHZ_STEPS.len() - 1)]
        } else {
            CPU_MHZ_STEPS[i.saturating_sub(1)]
        }
    } else if up {
        240
    } else {
        160
    }
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
    /// Live bench climb progress from firmware (`CMPBENCHPROG`).
    BenchProgress(String),
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
    /// Live status line while Companion self-update runs (not board flash).
    AppUpdateProgress(String),
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
        /// True while flash tools own the COM — UI Cancel clears this too.
        hold: Arc<AtomicBool>,
        /// Board was answering cmp — USB app OTA only (no BOOT / no silent ROM).
        live_push: bool,
        /// Stream app.bin over TCP `cmp ota` (Wi‑Fi-linked board).
        wifi_ota: bool,
    },
    /// Pull one user-configured API feed in the worker thread.
    PullApiFeed(ApiFeed),
    /// Probe USB / Wi‑Fi for CYD companion firmwares.
    ScanWorkers,
    /// Open an additional CYD USB worker without dropping existing ones.
    ConnectWorker(String),
    /// Open a Wi‑Fi CYD worker (`host:port` TCP cmp).
    ConnectWifi(String),
    /// Push home Wi‑Fi credentials to a linked board (NVS SoftAP→STA).
    SetBoardWifi {
        endpoint: String,
        ssid: String,
        pass: String,
        enable: bool,
    },
    /// Clear STA credentials (board returns to SoftAP-only setup).
    ClearBoardWifi {
        endpoint: String,
    },
    /// Drop one connected USB worker by COM port name.
    DisconnectWorker(String),
    /// Full-chip erase via espflash (USB only; BOOT Ready handshake).
    EraseFlash {
        port: String,
        cancel: Arc<AtomicBool>,
        need_boot: Arc<AtomicBool>,
        boot_ready: Arc<AtomicBool>,
        hold: Arc<AtomicBool>,
    },
    /// Soft reboot companion firmware (`cmp reboot`).
    RebootBoard {
        endpoint: String,
    },
}

struct CompanionApp {
    tab: Tab,
    /// When `tab == Settings` — which Settings sub-page is shown.
    settings_section: SettingsSection,
    com_port: String,
    ports: Vec<PortChoice>,
    usb_open: bool,
    mining: bool,
    edit_stratum: String,
    edit_worker: String,
    edit_password: String,
    /// Pool username of the active session (for Start debounce — keep A/R).
    active_mine_worker: String,
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
    /// Keep showing AUTHORIZED / RECONNECTING for a few seconds across soft pool drops.
    pool_auth_hold_until: Option<Instant>,
    live: LiveFeed,
    firmware: Option<FirmwareImage>,
    /// After successful flash verify: send `cmp reboot` once the board is linked again.
    pending_post_flash_reboot: Option<(Instant, String)>,
    /// After a successful board flash: wait until Instant, then OpenUsb (no Close bounce).
    pending_post_flash_reconnect: Option<(Instant, String)>,
    /// After reconnect: confirm board answers `cmp config` with kit firmware.
    post_flash_verify: Option<PostFlashVerify>,
    update_busy: bool,
    /// When Update board started — UI watchdog clears spinner if flash never finishes.
    update_busy_since: Option<Instant>,
    /// After a failed flash: block auto-link/Scan briefly so COM does not thrash.
    flash_cooldown_until: Option<Instant>,
    /// Shared cancel flag for the in-flight flash tool.
    flash_cancel: Option<Arc<AtomicBool>>,
    /// Shared with mine-worker — false when Cancel/watchdog/FlashDone releases the COM.
    flash_hold: Option<Arc<AtomicBool>>,
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
    /// When the current bench started (for elapsed + watchdog).
    bench_busy_since: Option<Instant>,
    /// Live status line shown on the bench notifier ("Mid · HW · 180 kH/s").
    bench_status: String,
    /// Last CMPBENCHPROG / heartbeat — stall detection.
    last_bench_prog_at: Option<Instant>,
    /// Last bench notifier ping.
    last_bench_notify_at: Option<Instant>,
    update_status: String,
    auto_connect: bool,
    auto_connect_attempted: bool,
    /// OpenUsb / ConnectWorker queued — block boot auto-connect from double-opening.
    usb_connect_pending: bool,
    /// App start — delayed COM re-lists (USB enum often lags first paint).
    boot_at: Instant,
    port_rescans_done: u8,
    /// Periodic COM re-enum so newly plugged USB adapters appear without Refresh.
    last_port_refresh: Instant,
    /// Last Ports log fingerprint (count|selected|names) — suppress identical spam.
    last_ports_log_sig: Option<String>,
    session_started: Option<Instant>,
    session_hash_start: u64,
    share_history: VecDeque<ShareRow>,
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
    /// When Update app started — watchdog clears if download never finishes.
    app_update_busy_since: Option<Instant>,
    /// UI Cancel → update thread aborts mirror retries.
    app_update_cancel: Option<Arc<AtomicBool>>,
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
    /// SoftAP → home Wi‑Fi provisioning UI.
    wifi_setup_ssid: String,
    wifi_setup_pass: String,
    wifi_setup_target: String,
    wifi_setup_phase: WifiSetupPhase,
    /// SoftAP endpoint we already auto-routed to Setup for (cleared when SoftAP gone).
    softap_setup_routed_ep: String,
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
        thread::spawn(move || {
            // A panic in the mine-worker must not tear down the GUI process.
            let err_tx = msg_tx.clone();
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                mine_worker(cmd_rx, msg_tx)
            }))
            .is_err()
            {
                let _ = err_tx.send(NetMsg::Action(Err(
                    "Mine worker stopped after an internal panic (see cyd-companion-crash.log next to the exe). Restart Companion, then re-link."
                        .into(),
                )));
            }
        });
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
                        m if CPU_MHZ_STEPS.contains(&m) => m,
                        other => normalize_cpu_mhz(other),
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
            settings_section: SettingsSection::General,
            com_port,
            ports: Vec::new(),
            usb_open: false,
            mining: false,
            edit_stratum,
            edit_worker,
            edit_password,
            active_mine_worker: String::new(),
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
            pool_auth_hold_until: None,
            live: LiveFeed::start(),
            firmware: find_firmware_image().ok(),
            pending_post_flash_reboot: None,
            pending_post_flash_reconnect: None,
            post_flash_verify: None,
            update_busy: false,
            update_busy_since: None,
            flash_cooldown_until: None,
            flash_cancel: None,
            flash_hold: None,
            flash_need_boot: None,
            flash_boot_ready: None,
            flash_progress: 0.0,
            flash_phase: String::new(),
            bench_busy: false,
            bench_busy_since: None,
            bench_status: String::new(),
            last_bench_prog_at: None,
            last_bench_notify_at: None,
            update_status: String::new(),
            auto_connect,
            auto_connect_attempted: false,
            usb_connect_pending: false,
            boot_at: Instant::now(),
            port_rescans_done: 0,
            last_port_refresh: Instant::now(),
            last_ports_log_sig: None,
            session_started: None,
            session_hash_start: 0,
            share_history: VecDeque::new(),
            session_accepted: 0,
            session_rejected: 0,
            last_share_latency_ms: None,
            last_job_flow_at: Instant::now() - Duration::from_secs(30),
            last_share_flow_at: Instant::now() - Duration::from_secs(30),
            wizard_step: if wizard_done { None } else { Some(0) },
            fetch_busy: false,
            app_update_busy: false,
            app_update_busy_since: None,
            app_update_cancel: None,
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
            wifi_setup_ssid: String::new(),
            wifi_setup_pass: String::new(),
            wifi_setup_target: String::new(),
            wifi_setup_phase: WifiSetupPhase::Idle,
            softap_setup_routed_ep: String::new(),
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
        if self.bench_busy && self.displayed_khs > 0.5 {
            return true;
        }
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
        } else if self.stratum_live.phase == "auth-fail"
            || self.stratum_live.phase == "err"
            || !self.stratum_live.last_error.is_empty()
        {
            ("AUTH FAILED", C_ERR)
        } else if self
            .pool_auth_hold_until
            .map(|t| Instant::now() < t)
            .unwrap_or(false)
            && self.mining
        {
            // Soft reconnect — do not flash SUBSCRIBE over a live session.
            match self.stratum_live.phase.as_str() {
                "sub" | "auth" | "tcp" => ("RECONNECTING…", C_WARN),
                _ => ("AUTHORIZED", C_LIME),
            }
        } else if self.stratum_live.connected
            || matches!(
                self.stratum_live.phase.as_str(),
                "tcp" | "sub" | "auth"
            )
        {
            let label = match self.stratum_live.phase.as_str() {
                "tcp" => "TCP…",
                "sub" => "SUBSCRIBE…",
                "auth" => "AUTHORIZE…",
                _ => "CONNECTING…",
            };
            (label, C_WARN)
        } else if self.mining {
            // SoftAP setup has no internet — pool cannot stay up while the PC is on Njordr.
            if self.pc_on_softap_setup_net() {
                ("SOFTAP — REJOIN HOME", C_WARN)
            } else {
                ("POOL DOWN", C_WARN)
            }
        } else {
            ("IDLE", C_MUTED)
        }
    }

    /// True when this PC is on Njordr SoftAP (no internet → pool cannot stay up).
    fn pc_on_softap_setup_net(&self) -> bool {
        if let Some(ip) = softap_setup_client_ipv4() {
            return ip.starts_with("10.88.88.");
        }
        if self.monitor_lan_ip.starts_with("10.88.88.") {
            return true;
        }
        // Fresh SoftAP beacon usually means this PC joined Njordr (no uplink).
        self.fresh_softap_board().is_some() && self.mining && !self.stratum_live.connected
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
        let target = if self.bench_busy {
            // Bench climbs paths — show measured kH, never force 0.
            if target > 0.5 {
                target
            } else if self.displayed_khs > 0.5 {
                self.displayed_khs
            } else {
                target
            }
        } else if !self.usb_open || !self.mining {
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

    /// Indented detail line under a parent event-log entry.
    fn push_log_detail(&mut self, kind: LogKind, text: impl Into<String>) {
        self.push_log(kind, format!("  · {}", text.into()));
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
        // Prefer stable MAC identity within the same *non-USB* transport.
        // USB is always keyed by COM endpoint — eFuse MAC collisions on cheap
        // boards must not collapse two Find-workers rows into one.
        // Never merge USB↔Wi‑Fi by MAC. Never merge on mac=unknown.
        if let Some(existing) = self.discovered_workers.iter_mut().find(|w| {
            w.id == worker.id
                || (w.kind == WorkerKind::Usb
                    && worker.kind == WorkerKind::Usb
                    && port_names_match(&w.endpoint, &worker.endpoint))
                || (w.kind == WorkerKind::Wifi
                    && worker.kind == WorkerKind::Wifi
                    && (w.endpoint == worker.endpoint
                        || (mac_is_stable(&worker.mac)
                            && mac_is_stable(&w.mac)
                            && normalize_mac(&w.mac) == normalize_mac(&worker.mac))))
                || (w.kind == worker.kind
                    && !matches!(w.kind, WorkerKind::Usb | WorkerKind::Wifi)
                    && mac_is_stable(&worker.mac)
                    && mac_is_stable(&w.mac)
                    && normalize_mac(&w.mac) == normalize_mac(&worker.mac))
        }) {
            *existing = worker;
        } else {
            self.discovered_workers.push(worker);
        }
        self.discovered_workers
            .sort_by(|a, b| a.endpoint.cmp(&b.endpoint).then(a.mac.cmp(&b.mac)));
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

    /// True when two+ linked boards report the same stable eFuse MAC.
    fn connected_mac_collision(&self) -> bool {
        let mut seen: Vec<String> = Vec::new();
        for w in &self.connected_workers {
            if !mac_is_stable(&w.mac) {
                continue;
            }
            let m = normalize_mac(&w.mac);
            if seen.iter().any(|s| s == &m) {
                return true;
            }
            seen.push(m);
        }
        false
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
        if self.flash_busy() {
            self.last_error = "Wait for Update board / flash to finish.".into();
            return;
        }
        let linked = self.linked_endpoints();
        let candidates: Vec<String> = self
            .ports
            .iter()
            .filter(|p| !port_choice_is_system_junk(p) && cyd_port_score(p) >= 0)
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
                .map(|p| port_choice_is_system_junk(p) || cyd_port_score(p) < 0)
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
                    !port_choice_is_system_junk(p)
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
            .map(|p| port_choice_is_system_junk(p) || cyd_port_score(p) < 0)
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
        let changed = ports.len() != self.ports.len()
            || ports.iter().zip(self.ports.iter()).any(|(a, b)| {
                normalize_port_name(&a.name) != normalize_port_name(&b.name) || a.label != b.label
            });
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
        // Periodic polls stay quiet when nothing changed; boot / first paint / changes log fully.
        if changed || allow_auto_connect || force_best {
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
        }
        if allow_auto_connect {
            // Do not clear attempted while a connect is already in flight.
            if !self.usb_connect_pending {
                self.maybe_auto_connect_usb();
            }
        }
    }

    /// True only while Update board overlay is active (blocks Connect/Scan).
    fn flash_busy(&self) -> bool {
        self.update_busy
    }

    /// Cooldown after a failed flash — blocks auto-link/Scan, not manual Connect.
    fn flash_cooldown_active(&self) -> bool {
        self.flash_cooldown_until
            .map(|t| Instant::now() < t)
            .unwrap_or(false)
    }

    fn arm_flash_cooldown(&mut self, secs: u64) {
        self.flash_cooldown_until = Some(Instant::now() + Duration::from_secs(secs));
        self.pending_post_flash_reconnect = None;
        self.post_flash_verify = None;
        self.usb_connect_pending = false;
    }

    fn maybe_auto_connect_usb(&mut self) {
        if self.auto_connect
            && !self.auto_connect_attempted
            && !self.usb_connect_pending
            && !self.usb_open
            && !self.flash_busy()
            && !self.flash_cooldown_active()
            && !self.com_port.is_empty()
            && self.ports.iter().any(|x| x.name == self.com_port)
            && self
                .ports
                .iter()
                .find(|x| x.name == self.com_port)
                .map(|p| !port_choice_is_system_junk(p))
                .unwrap_or(false)
        {
            self.auto_connect_attempted = true;
            self.connect_or_add_usb();
        }
    }

    fn queue_connect_usb(&mut self, endpoint: String, why: &str) {
        if self.worker_already_linked(&endpoint) {
            self.last_ok = format!("{endpoint} already linked");
            return;
        }
        if self.flash_busy() || self.flash_cooldown_active() {
            self.last_error =
                format!("Wait for Update board / flash cooldown before linking {endpoint}");
            return;
        }
        if self.usb_connect_pending {
            self.push_log(
                LogKind::Warn,
                format!("Link already in progress — queued note for {endpoint} ({why})"),
            );
        }
        self.usb_connect_pending = true;
        self.com_port = endpoint.clone();
        self.push_log(LogKind::Usb, format!("{why} → ConnectWorker {endpoint}"));
        let _ = self.cmd_tx.send(NetCmd::ConnectWorker(endpoint));
    }

    fn queue_connect_wifi(&mut self, endpoint: String, why: &str) {
        if self.worker_already_linked(&endpoint) {
            self.last_ok = format!("{endpoint} already linked");
            return;
        }
        if let Some(w) = self
            .discovered_workers
            .iter()
            .find(|d| d.endpoint == endpoint)
        {
            if self.worker_mac_already_linked(&w.mac) {
                self.last_ok = format!(
                    "Skip Wi‑Fi {endpoint} — same board already linked over USB ({})",
                    w.mac
                );
                self.push_log(LogKind::Usb, self.last_ok.clone());
                return;
            }
            if wifi_is_softap_setup(w) {
                self.last_error = format!(
                    "SoftAP {endpoint} — join open Njordr-XXXX in Windows Wi‑Fi (no password), then Connect. Prefer Link over USB when the board is plugged in."
                );
                self.push_log(LogKind::Warn, self.last_error.clone());
                // Still attempt — PC may already be on SoftAP.
            }
        }
        if self.flash_busy() || self.flash_cooldown_active() {
            self.last_error = format!("Wait for Update board before Wi‑Fi link {endpoint}");
            return;
        }
        self.usb_connect_pending = true;
        self.push_log(LogKind::Usb, format!("{why} → ConnectWifi {endpoint}"));
        let _ = self.cmd_tx.send(NetCmd::ConnectWifi(endpoint));
    }

    fn connect_or_add_usb(&mut self) {
        self.last_error.clear();
        if self.flash_busy() {
            self.last_error = "Wait for Update board / flash to finish.".into();
            return;
        }
        if self.com_port.is_empty() {
            self.last_error = "Select a COM / serial port.".into();
            return;
        }
        if self
            .port_lookup(&self.com_port)
            .map(port_choice_is_system_junk)
            .unwrap_or_else(|| {
                let n = normalize_port_name(&self.com_port);
                n == "COM1"
            })
        {
            self.last_error =
                "COM1 / PCI motherboard ports are not CYD boards — pick the USB-UART COM."
                    .into();
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
        let pool_user = stratum_worker_for_companion(self.edit_worker.trim());
        let pool_pass = stratum_password(&self.edit_password);
        let stratum = self.edit_stratum.trim().to_string();
        // Same pool session already running — do not reconnect / wipe Accept·Reject.
        // Duplicate StartMine was wiping counters every few seconds.
        if self.mining
            && self.stratum_live.authorized
            && stratum_endpoints_match(&self.stratum_live.endpoint, &stratum)
            && self
                .active_mine_worker
                .eq_ignore_ascii_case(&pool_user)
        {
            self.last_ok = format!(
                "Already mining {stratum} as {pool_user} — counters kept (A={} R={})",
                self.session_accepted, self.session_rejected
            );
            self.push_log(LogKind::Info, self.last_ok.clone());
            return;
        }
        let _ = self.cmd_tx.send(NetCmd::StartMine {
            stratum: stratum.clone(),
            worker: self.edit_worker.trim().to_string(),
            password: pool_pass,
        });
        self.mining = true;
        self.active_mine_worker = pool_user.clone();
        self.session_started = Some(Instant::now());
        self.session_hash_start = self.status.hashes;
        self.reset_share_session_ui();
        self.last_ok = format!("Starting pool as {pool_user}…");
        self.push_log(
            LogKind::Info,
            format!(
                "Start mining → {stratum} as {pool_user} · linked USB/Wi‑Fi={}",
                self.connected_workers.len().max(usize::from(self.usb_open)),
            ),
        );
        if pool_user != self.edit_worker.trim() {
            self.push_log(
                LogKind::Info,
                format!(
                    "Pool username {pool_user} — on HMPool paste the BTC address (worker suffix after the dot)"
                ),
            );
        }
        self.push_log_detail(
            LogKind::Info,
            format!(
                "session counters reset · target clock {} MHz · auto-connect={}",
                self.target_mhz, self.auto_connect
            ),
        );
    }

    fn stop_mine(&mut self) {
        let _ = self.cmd_tx.send(NetCmd::StopMine);
        let up = self
            .session_started
            .map(|t| format_uptime(t.elapsed().as_secs()))
            .unwrap_or_else(|| "—".into());
        let acc = self.session_accepted;
        let rej = self.session_rejected;
        let pool = self.edit_stratum.trim().to_string();
        self.mining = false;
        self.active_mine_worker.clear();
        self.session_started = None;
        self.clear_hash_display();
        self.last_ok = "Mining stopped.".into();
        self.push_log(
            LogKind::Info,
            format!("Mining stopped · session {up} · accepted={acc} rejected={rej} · pool {pool}"),
        );
    }

    fn clear_bench_busy(&mut self, reason: &str) {
        if !self.bench_busy && self.bench_busy_since.is_none() {
            return;
        }
        self.bench_busy = false;
        self.bench_busy_since = None;
        USB_BENCH_CANCEL.store(false, Ordering::SeqCst);
        if !reason.is_empty() {
            self.bench_status = reason.to_string();
            self.last_ok = reason.to_string();
            self.push_log(LogKind::Usb, reason.to_string());
        }
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
        USB_BENCH_CANCEL.store(false, Ordering::SeqCst);
        self.bench_busy = true;
        self.bench_busy_since = Some(Instant::now());
        self.last_bench_prog_at = Some(Instant::now());
        self.last_bench_notify_at = Some(Instant::now());
        self.bench_status = "Starting Mid→HW→HW/SW climb…".into();
        self.last_error.clear();
        self.last_ok = "Bench: climbing Mid→HW→HW/SW — live kH updates while tuning…".into();
        self.push_log(
            LogKind::Usb,
            "D0 auto-tune: each board climbs SHA paths with stable dual-pass timing…".into(),
        );
        let _ = self.cmd_tx.send(NetCmd::Bench);
    }

    /// Live banner + stuck watchdog while Bench owns the boards.
    fn tick_bench_notifier(&mut self) {
        if !self.bench_busy {
            return;
        }
        let since = self.bench_busy_since.unwrap_or_else(Instant::now);
        let elapsed = since.elapsed().as_secs();
        let last_prog = self
            .last_bench_prog_at
            .unwrap_or(since)
            .elapsed()
            .as_secs();

        // Hard watchdog — never leave the UI on "Benching…" forever.
        if elapsed >= 210 {
            self.clear_bench_busy(&format!(
                "Bench watchdog: still busy after {elapsed}s — cleared. Retry Bench or Push firmware if it hangs again."
            ));
            self.last_error = self.last_ok.clone();
            return;
        }
        // No CMPBENCHPROG for a long stretch after start → treat as hung.
        if elapsed >= 50 && last_prog >= 45 {
            USB_BENCH_CANCEL.store(true, Ordering::SeqCst);
            self.clear_bench_busy(&format!(
                "Bench stuck — no progress for {last_prog}s (elapsed {elapsed}s). Cancelled; update board FW if this repeats."
            ));
            self.last_error = self.last_ok.clone();
            return;
        }

        // Periodic "still benching" notifier so it never looks frozen.
        let due = self
            .last_bench_notify_at
            .map(|t| t.elapsed() >= Duration::from_secs(8))
            .unwrap_or(true);
        if due {
            self.last_bench_notify_at = Some(Instant::now());
            let status = if self.bench_status.is_empty() {
                "tuning SHA paths".into()
            } else {
                self.bench_status.clone()
            };
            let msg = format!("Still benching… {status} · {elapsed}s");
            self.last_ok = msg.clone();
            self.push_log(LogKind::Usb, msg.clone());
        }
    }

    fn cancel_bench(&mut self) {
        if !self.bench_busy {
            return;
        }
        USB_BENCH_CANCEL.store(true, Ordering::SeqCst);
        self.clear_bench_busy("Bench cancelled — waiting for USB wait to abort…");
    }

    fn absorb_bench_progress(&mut self, line: &str) {
        // CMPBENCHPROG step=… path=… mhz=… hs=… khs=…
        let mut khs = None;
        let mut path = String::new();
        let mut step = String::new();
        for part in line.split_whitespace() {
            if let Some(v) = part.strip_prefix("khs=") {
                khs = v.parse::<f64>().ok();
            } else if let Some(v) = part.strip_prefix("path=") {
                path = v.to_string();
            } else if let Some(v) = part.strip_prefix("step=") {
                step = v.to_string();
            }
        }
        if let Some(k) = khs {
            if k > 0.5 {
                self.status.hashrate_hs = k * 1000.0;
                self.status.hashrate_khs = k;
                self.status.mining = true;
                let kf = k as f32;
                self.displayed_khs = if self.displayed_khs < 1.0 {
                    kf
                } else {
                    self.displayed_khs * 0.4 + kf * 0.6
                };
            }
        }
        let msg = if step == "wait" {
            let elapsed = line
                .split_whitespace()
                .find_map(|p| p.strip_prefix("elapsed="))
                .unwrap_or("…");
            format!("Bench waiting on USB… {elapsed}")
        } else if !path.is_empty() && khs.unwrap_or(0.0) > 0.5 {
            format!(
                "Bench {step} · {path} · {:.0} kH/s",
                khs.unwrap_or(0.0)
            )
        } else if !step.is_empty() {
            format!("Bench {step}…")
        } else {
            trunc(line, 120)
        };
        // Heartbeats keep the notifier alive but don't reset stall clock unless real prog.
        if step != "wait" {
            self.last_bench_prog_at = Some(Instant::now());
        } else if self.last_bench_prog_at.is_none() {
            self.last_bench_prog_at = Some(Instant::now());
        }
        self.bench_status = msg.clone();
        self.last_ok = msg.clone();
        self.push_log(LogKind::Usb, msg);
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

    fn fleet_hashrate_hs(&self) -> f64 {
        if self.mining && self.displayed_khs > 0.5 {
            self.displayed_khs as f64 * 1000.0
        } else if self.status.hashrate_hs > 0.0 {
            self.status.hashrate_hs
        } else {
            self.connected_workers
                .iter()
                .map(|w| w.hashrate_hs)
                .sum()
        }
    }

    fn expected_shares_label(&self) -> String {
        let hs = self.fleet_hashrate_hs();
        let exp = expected_shares_per_hour(hs, self.stratum_live.difficulty);
        if exp <= 0.0 {
            "—".into()
        } else if exp >= 10.0 {
            format!("{exp:.1}/h")
        } else if exp >= 1.0 {
            format!("{exp:.2}/h")
        } else if exp >= 0.01 {
            format!("{exp:.3}/h")
        } else {
            format!("{exp:.4}/h")
        }
    }

    /// Pool-inferred hashrate from accepted shares (same formula pools use).
    fn pool_estimated_hs(&self) -> f64 {
        if !self.stratum_live.authorized {
            return 0.0;
        }
        let Some(started) = self.session_started else {
            return 0.0;
        };
        let diff = self.stratum_live.difficulty;
        if !(diff > 0.0) || self.session_accepted == 0 {
            return 0.0;
        }
        let secs = started.elapsed().as_secs_f64().max(15.0);
        self.session_accepted as f64 * diff * 4_294_967_296.0 / secs
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
            self.fleet_hashrate_hs(),
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

    fn port_lookup(&self, name: &str) -> Option<&PortChoice> {
        self.ports
            .iter()
            .find(|p| port_names_match(&p.name, name))
    }

    fn port_is_flashable_name(&self, name: &str) -> bool {
        if !is_usb_serial_port(name) {
            return false;
        }
        match self.port_lookup(name) {
            Some(p) => !port_choice_is_system_junk(p),
            // Unknown label — allow COM3+; still reject bare COM1.
            None => {
                let n = normalize_port_name(name);
                n != "COM1" && n.starts_with("COM")
            }
        }
    }

    /// USB COM / Wi‑Fi linked worker choices for Update board.
    fn flash_target_choices(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for w in &self.connected_workers {
            let usb = self.port_is_flashable_name(&w.endpoint);
            let wifi = Self::is_wifi_ota_endpoint(&w.endpoint);
            if !usb && !wifi {
                continue;
            }
            let mac = if w.mac.is_empty() {
                "mac?".to_string()
            } else {
                w.mac.clone()
            };
            let fw = if w.fw.is_empty() {
                "fw?".to_string()
            } else {
                w.fw.clone()
            };
            let tag = if Self::fw_looks_download_mode(&w.fw) {
                "download-mode"
            } else if wifi {
                "Wi‑Fi"
            } else {
                "USB"
            };
            out.push((
                w.endpoint.clone(),
                format!("{} · {mac} · {fw} · {tag}", w.endpoint),
            ));
        }
        // Recent Wi‑Fi discoveries (not linked yet) — still offer wireless push.
        for w in &self.discovered_workers {
            if w.kind != WorkerKind::Wifi {
                continue;
            }
            if out.iter().any(|(e, _)| port_names_match(e, &w.endpoint)) {
                continue;
            }
            if !Self::is_wifi_ota_endpoint(&w.endpoint) {
                continue;
            }
            let mac = if w.mac.is_empty() {
                "mac?".to_string()
            } else {
                w.mac.clone()
            };
            let fw = if w.fw.is_empty() {
                "fw?".to_string()
            } else {
                w.fw.clone()
            };
            out.push((
                w.endpoint.clone(),
                format!("{} · {mac} · {fw} · Wi‑Fi (scan)", w.endpoint),
            ));
        }
        for p in flashable_ports(&self.ports) {
            if out.iter().any(|(e, _)| port_names_match(e, &p.name)) {
                continue;
            }
            out.push((p.name.clone(), format!("{} · not linked", p.label)));
        }
        out
    }

    /// `host:19284` Wi‑Fi TCP endpoint suitable for `cmp ota` push.
    fn is_wifi_ota_endpoint(name: &str) -> bool {
        let n = name.trim();
        if n.is_empty() || is_usb_serial_port(n) {
            return false;
        }
        let upper = n.to_ascii_uppercase();
        if upper.starts_with("COM") || upper.starts_with("/DEV/") {
            return false;
        }
        // host:port
        n.contains(':')
    }

    /// USB COM or Wi‑Fi endpoint for Update board — respect the user’s selected worker first.
    fn resolve_flash_target(&self) -> Result<String, String> {
        // 1) Explicit selection (Mine / Update picker).
        if self.port_is_flashable_name(&self.com_port) || Self::is_wifi_ota_endpoint(&self.com_port)
        {
            return Ok(self.com_port.clone());
        }
        // 2) Linked Wi‑Fi boards (wireless push).
        for w in &self.connected_workers {
            if Self::is_wifi_ota_endpoint(&w.endpoint) && !Self::fw_looks_download_mode(&w.fw) {
                return Ok(w.endpoint.clone());
            }
        }
        // 3) Linked USB boards (prefer live companion over download-mode).
        for w in &self.connected_workers {
            if self.port_is_flashable_name(&w.endpoint) && !Self::fw_looks_download_mode(&w.fw) {
                return Ok(w.endpoint.clone());
            }
        }
        for w in &self.connected_workers {
            if self.port_is_flashable_name(&w.endpoint) {
                return Ok(w.endpoint.clone());
            }
        }
        // 4) Scanned Wi‑Fi boards.
        for w in &self.discovered_workers {
            if w.kind == WorkerKind::Wifi
                && Self::is_wifi_ota_endpoint(&w.endpoint)
                && !Self::fw_looks_download_mode(&w.fw)
            {
                return Ok(w.endpoint.clone());
            }
        }
        // 5) Best scored CYD from the port list.
        if let Some(p) = prefer_cyd_port(&self.ports) {
            return Ok(p.name.clone());
        }
        if let Some(p) = flashable_ports(&self.ports).into_iter().next() {
            return Ok(p.name.clone());
        }
        Err(
            "Select which board to update (USB COM / Wi‑Fi worker). COM1 / PCI motherboard ports are not flashable."
                .into(),
        )
    }

    fn go_flash_tab(&mut self) {
        if !self
            .firmware
            .as_ref()
            .map(firmware_is_custom)
            .unwrap_or(false)
        {
            self.firmware = find_firmware_image().ok().or_else(|| self.firmware.clone());
        }
        if let Ok(port) = self.resolve_flash_target() {
            if !port_names_match(&port, &self.com_port) {
                self.com_port = port;
            }
        }
        self.open_settings(SettingsSection::Flash);
    }

    fn request_board_update(&mut self) {
        self.go_flash_tab();
        if self.firmware.is_none() {
            self.push_log(
                LogKind::Info,
                "No local firmware yet — Fetch latest FW under Settings → Flash, or drop a .bin.".into(),
            );
        }
    }

    fn absorb_firmware_path(&mut self, path: std::path::PathBuf) {
        match load_firmware_bin(&path) {
            Ok(img) => {
                let label = format!(
                    "Custom .bin · {} · {} KB",
                    img.path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("bin"),
                    img.bytes / 1024
                );
                self.last_ok = label.clone();
                self.push_log(LogKind::Info, format!("Flash image ← {}", img.path.display()));
                self.firmware = Some(img);
                self.update_status = "Custom .bin ready — Update board to flash it.".into();
            }
            Err(e) => {
                self.last_error = e.clone();
                self.push_log(LogKind::Err, e);
            }
        }
    }

    fn absorb_dropped_files(&mut self, files: Vec<egui::DroppedFile>) {
        for f in files {
            let path = if let Some(p) = f.path {
                p
            } else if let Some(bytes) = f.bytes {
                let name = if f.name.is_empty() {
                    "dropped.bin".to_string()
                } else {
                    f.name.clone()
                };
                if !name.to_ascii_lowercase().ends_with(".bin") {
                    self.last_error = format!("Ignored drop “{name}” — need a .bin");
                    continue;
                }
                let dir = std::env::temp_dir().join("cyd-companion-drop");
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    self.last_error = format!("drop temp dir: {e}");
                    continue;
                }
                let dest = dir.join(&name);
                if let Err(e) = std::fs::write(&dest, bytes.as_ref()) {
                    self.last_error = format!("save dropped bin: {e}");
                    continue;
                }
                dest
            } else {
                continue;
            };
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !name.ends_with(".bin") {
                self.last_error = format!(
                    "Ignored {} — drop a .bin firmware image",
                    path.display()
                );
                continue;
            }
            self.absorb_firmware_path(path);
        }
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
        let cancel = self
            .app_update_cancel
            .clone()
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
        thread::spawn(move || {
            let progress = {
                let tx = tx.clone();
                move |line: String| log_msg(&tx, LogKind::Info, line)
            };
            let result = check_app_update_ex(&progress, Some(cancel.as_ref()));
            let _ = tx.send(NetMsg::AppUpdate(result));
        });
    }

    fn start_app_update_check(&mut self) {
        if self.app_update_busy {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.app_update_cancel = Some(cancel);
        self.app_update_busy = true;
        self.app_update_busy_since = Some(Instant::now());
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
        let cancel = Arc::new(AtomicBool::new(false));
        self.app_update_cancel = Some(cancel.clone());
        self.app_update_busy = true;
        self.app_update_busy_since = Some(Instant::now());
        self.update_status = format!("Updating Companion {}…", running_version());
        self.last_ok = self.update_status.clone();
        self.push_log(LogKind::Info, self.update_status.clone());
        let tx = self.msg_tx.clone();
        thread::spawn(move || {
            let progress = {
                let tx = tx.clone();
                move |line: String| {
                    // Do NOT send FlashProgress — that drives the board-flash overlay.
                    log_msg(&tx, LogKind::Info, line.clone());
                    let _ = tx.send(NetMsg::AppUpdateProgress(line));
                }
            };
            let result = update_companion_app_ex(&progress, Some(cancel.as_ref()));
            let _ = tx.send(NetMsg::AppUpdate(result));
        });
    }

    fn cancel_app_update(&mut self) {
        if let Some(c) = self.app_update_cancel.take() {
            c.store(true, Ordering::SeqCst);
        }
        self.app_update_busy = false;
        self.app_update_busy_since = None;
        self.update_status = "Companion update cancelled".into();
        self.push_log(LogKind::Warn, self.update_status.clone());
    }

    /// ROM/bootloader download mode (BOOT held / blank chip) — not companion firmware.
    fn fw_looks_download_mode(fw: &str) -> bool {
        let l = fw.trim().to_ascii_lowercase();
        l == "download-mode"
            || l.contains("download")
            || l.contains("bootloader")
            || l == "rom"
    }

    fn board_is_download_mode(&self, port: &str) -> bool {
        self.connected_workers.iter().any(|c| {
            port_names_match(&c.endpoint, port) && Self::fw_looks_download_mode(&c.fw)
        })
    }

    /// True when the target COM likely has companion firmware answering cmp —
    /// including older builds with empty/odd fw tags. Used to prefer Push update.
    fn board_supports_live_push(&self, port: &str) -> bool {
        if Self::is_wifi_ota_endpoint(port) {
            return self.board_supports_wifi_ota(port);
        }
        if self.board_is_download_mode(port) {
            return false;
        }
        // Linked worker on this COM (old firmware may leave fw blank — still pushable).
        if self.connected_workers.iter().any(|c| {
            port_names_match(&c.endpoint, port) && !Self::fw_looks_download_mode(&c.fw)
        }) {
            return true;
        }
        // Primary USB session on this COM — cmp already succeeded to open.
        if self.usb_open
            && port_names_match(&self.com_port, port)
            && !Self::fw_looks_download_mode(&self.fw_label)
        {
            return true;
        }
        // Recent Find-workers / scan hit that answered CMP ok.
        self.discovered_workers.iter().any(|w| {
            matches!(w.kind, WorkerKind::Usb)
                && port_names_match(&w.endpoint, port)
                && !w.fw.is_empty()
                && !Self::fw_looks_download_mode(&w.fw)
        })
    }

    /// Wi‑Fi board answering cmp — can receive `cmp ota` app push.
    fn board_supports_wifi_ota(&self, endpoint: &str) -> bool {
        if !Self::is_wifi_ota_endpoint(endpoint) {
            return false;
        }
        if self.connected_workers.iter().any(|c| {
            port_names_match(&c.endpoint, endpoint) && !Self::fw_looks_download_mode(&c.fw)
        }) {
            return true;
        }
        self.discovered_workers.iter().any(|w| {
            w.kind == WorkerKind::Wifi
                && port_names_match(&w.endpoint, endpoint)
                && !Self::fw_looks_download_mode(&w.fw)
        })
    }

    fn begin_board_update(&mut self, prefer_live_push: bool) {
        self.begin_board_update_ex(prefer_live_push, false);
    }

    fn begin_board_update_wifi(&mut self) {
        self.begin_board_update_ex(false, true);
    }

    fn begin_board_update_ex(&mut self, prefer_live_push: bool, wifi_ota: bool) {
        let port = match self.resolve_flash_target() {
            Ok(p) => p,
            Err(e) => {
                self.last_error = e;
                return;
            }
        };
        self.com_port = port.clone();
        let wifi_target = Self::is_wifi_ota_endpoint(&port);
        if wifi_ota && !wifi_target {
            self.last_error =
                "Push update (Wi‑Fi) needs a Wi‑Fi board (host:19284). Link/scan one on Mine, or pick it above."
                    .into();
            return;
        }
        if !wifi_ota && wifi_target {
            self.last_error =
                "This target is Wi‑Fi — use Push update (Wi‑Fi). USB Flash (BOOT) needs a COM port."
                    .into();
            return;
        }
        // Only refuse Push when we *know* this COM is ROM download-mode.
        if prefer_live_push && !wifi_ota && self.board_is_download_mode(&port) {
            self.last_error =
                "This COM is in download mode (BOOT held / blank) — use Flash (BOOT), then Ready."
                    .into();
            return;
        }
        // Honor the user's Push choice even if detection is uncertain (old fw / not linked).
        // USB OTA first (no hold); flash_update falls back to ROM auto-reset, then Ready.
        let live_push = prefer_live_push && !wifi_ota;
        // Abort any in-flight USB wait BEFORE queuing UpdateFirmware —
        // otherwise Push sits on "auto-reset…" until via timeouts finish (tens of seconds).
        USB_FLASH_PREEMPT.store(true, Ordering::SeqCst);
        if self.mining {
            self.stop_mine();
        }
        let prefer_d0 = board_fw_is_d0(&self.fw_label)
            || self
                .firmware
                .as_ref()
                .map(|f| board_fw_is_d0(&f.path.to_string_lossy()))
                .unwrap_or(false);
        let image = if wifi_ota {
            resolve_ota_app_image_ex(
                self.firmware.as_ref().map(|f| f.path.as_path()),
                prefer_d0,
            )
            .ok()
            .map(|fw| fw.path.to_string_lossy().into_owned())
            .or_else(|| {
                self.firmware
                    .as_ref()
                    .map(|fw| fw.path.to_string_lossy().into_owned())
            })
            .unwrap_or_default()
        } else {
            self.firmware
                .as_ref()
                .map(|fw| fw.path.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        self.update_busy = true;
        self.update_busy_since = Some(Instant::now());
        self.flash_cooldown_until = None;
        let cancel = Arc::new(AtomicBool::new(false));
        let need_boot = Arc::new(AtomicBool::new(false));
        let boot_ready = Arc::new(AtomicBool::new(false));
        let hold = Arc::new(AtomicBool::new(true));
        self.flash_cancel = Some(cancel.clone());
        self.flash_hold = Some(hold.clone());
        self.flash_need_boot = Some(need_boot.clone());
        self.flash_boot_ready = Some(boot_ready.clone());
        self.flash_progress = 0.02;
        self.flash_phase = if wifi_ota {
            "Wi‑Fi push".into()
        } else if live_push {
            "Pushing update".into()
        } else {
            "Preparing — Ready next".into()
        };
        self.pending_post_flash_reconnect = None;
        self.post_flash_verify = None;
        self.update_status = if wifi_ota {
            format!("Pushing firmware over Wi‑Fi to {port}…")
        } else if live_push {
            format!("Releasing {port} for push (USB OTA, no BOOT)…")
        } else {
            format!("Flashing board via {port}…")
        };
        self.last_ok = self.update_status.clone();
        self.last_error.clear();
        let fw_ver = self
            .firmware
            .as_ref()
            .map(|f| f.version.as_str())
            .filter(|v| !v.is_empty())
            .unwrap_or("?");
        let fw_kb = self.firmware.as_ref().map(|f| f.bytes / 1024).unwrap_or(0);
        let img_note = if image.is_empty() {
            format!("fetch+flash · kit {fw_ver}")
        } else {
            format!("{image} · {fw_ver} ({fw_kb} KB)")
        };
        self.push_log(
            LogKind::Usb,
            if wifi_ota {
                format!("Push update (Wi‑Fi) → {port} · {img_note}")
            } else if live_push {
                format!("Push update → {port} · {img_note} · no BOOT (USB OTA)")
            } else if image.is_empty() {
                format!("Flash (BOOT) → {port} · {img_note}")
            } else {
                format!("Flash (BOOT) → {port} · {img_note}")
            },
        );
        // Release USB/TCP in the worker before flash so the update path owns the board.
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
            hold,
            live_push,
            wifi_ota,
        });
    }

    fn begin_board_erase(&mut self) {
        let port = match self.resolve_usb_erase_target() {
            Ok(p) => p,
            Err(e) => {
                self.last_error = e;
                return;
            }
        };
        self.com_port = port.clone();
        if self.mining {
            self.stop_mine();
        }
        USB_FLASH_PREEMPT.store(true, Ordering::SeqCst);
        self.update_busy = true;
        self.update_busy_since = Some(Instant::now());
        self.flash_cooldown_until = None;
        let cancel = Arc::new(AtomicBool::new(false));
        let need_boot = Arc::new(AtomicBool::new(false));
        let boot_ready = Arc::new(AtomicBool::new(false));
        let hold = Arc::new(AtomicBool::new(true));
        self.flash_cancel = Some(cancel.clone());
        self.flash_hold = Some(hold.clone());
        self.flash_need_boot = Some(need_boot.clone());
        self.flash_boot_ready = Some(boot_ready.clone());
        self.flash_progress = 0.02;
        self.flash_phase = "Erasing flash".into();
        self.pending_post_flash_reconnect = None;
        self.post_flash_verify = None;
        self.update_status = format!("Erasing flash on {port}…");
        self.last_ok = self.update_status.clone();
        self.last_error.clear();
        self.push_log(LogKind::Usb, format!("Erase flash → {port} (full chip)"));
        self.usb_open = false;
        self.mining = false;
        let _ = self.cmd_tx.send(NetCmd::EraseFlash {
            port,
            cancel,
            need_boot,
            boot_ready,
            hold,
        });
    }

    fn begin_board_reset(&mut self) {
        let endpoint = match self.resolve_reset_target() {
            Ok(e) => e,
            Err(e) => {
                self.last_error = e;
                return;
            }
        };
        self.com_port = endpoint.clone();
        self.last_error.clear();
        self.last_ok = format!("Reset command queued for {endpoint}");
        self.push_log(LogKind::Usb, format!("Reset board → {endpoint} (cmp reboot)"));
        let _ = self.cmd_tx.send(NetCmd::RebootBoard { endpoint });
    }

    /// USB COM only — erase needs espflash / ROM, not Wi‑Fi.
    fn resolve_usb_erase_target(&self) -> Result<String, String> {
        if self.port_is_flashable_name(&self.com_port) {
            return Ok(self.com_port.clone());
        }
        for w in &self.connected_workers {
            if self.port_is_flashable_name(&w.endpoint) {
                return Ok(w.endpoint.clone());
            }
        }
        for p in flashable_ports(&self.ports) {
            return Ok(p.name.clone());
        }
        Err("Select a USB COM port for erase (Wi‑Fi cannot erase flash).".into())
    }

    /// USB or Wi‑Fi board that should answer `cmp`.
    fn resolve_reset_target(&self) -> Result<String, String> {
        if self.port_is_flashable_name(&self.com_port) || Self::is_wifi_ota_endpoint(&self.com_port)
        {
            return Ok(self.com_port.clone());
        }
        for w in &self.connected_workers {
            let usb = self.port_is_flashable_name(&w.endpoint);
            let wifi = Self::is_wifi_ota_endpoint(&w.endpoint);
            if (usb || wifi) && !Self::fw_looks_download_mode(&w.fw) {
                return Ok(w.endpoint.clone());
            }
        }
        for w in &self.connected_workers {
            if self.port_is_flashable_name(&w.endpoint) || Self::is_wifi_ota_endpoint(&w.endpoint)
            {
                return Ok(w.endpoint.clone());
            }
        }
        Err("Select a linked USB or Wi‑Fi board for reset.".into())
    }

    fn usb_erase_target_choices(&self) -> Vec<(String, String)> {
        self.flash_target_choices()
            .into_iter()
            .filter(|(ep, _)| self.port_is_flashable_name(ep))
            .collect()
    }

    fn reset_target_choices(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for w in &self.connected_workers {
            let usb = self.port_is_flashable_name(&w.endpoint);
            let wifi = Self::is_wifi_ota_endpoint(&w.endpoint);
            if !usb && !wifi {
                continue;
            }
            let mac = if w.mac.is_empty() {
                "mac?".to_string()
            } else {
                w.mac.clone()
            };
            let fw = if w.fw.is_empty() {
                "fw?".to_string()
            } else {
                w.fw.clone()
            };
            let tag = if Self::fw_looks_download_mode(&w.fw) {
                "download-mode"
            } else if wifi {
                "Wi‑Fi"
            } else {
                "USB"
            };
            out.push((
                w.endpoint.clone(),
                format!("{} · {mac} · {fw} · {tag}", w.endpoint),
            ));
        }
        for p in flashable_ports(&self.ports) {
            if out.iter().any(|(e, _)| port_names_match(e, &p.name)) {
                continue;
            }
            out.push((p.name.clone(), format!("{} · not linked", p.label)));
        }
        for w in &self.discovered_workers {
            if w.kind != WorkerKind::Wifi {
                continue;
            }
            if out.iter().any(|(e, _)| port_names_match(e, &w.endpoint)) {
                continue;
            }
            if !Self::is_wifi_ota_endpoint(&w.endpoint) {
                continue;
            }
            let mac = if w.mac.is_empty() {
                "mac?".to_string()
            } else {
                w.mac.clone()
            };
            let fw = if w.fw.is_empty() {
                "fw?".to_string()
            } else {
                w.fw.clone()
            };
            out.push((
                w.endpoint.clone(),
                format!("{} · {mac} · {fw} · Wi‑Fi (scan)", w.endpoint),
            ));
        }
        out
    }

    fn ui_flash_target_combo(&mut self, ui: &mut egui::Ui, id: &str) {
        let choices = self.flash_target_choices();
        ui.label(
            RichText::new("Board to update")
                .color(C_MUTED)
                .font(mono_ui_font(11.0)),
        );
        let combo_w = (ui.available_width() - 8.0).clamp(160.0, 420.0);
        let selected = if self.com_port.is_empty() {
            "— select linked worker / COM / Wi‑Fi —".to_string()
        } else {
            choices
                .iter()
                .find(|(e, _)| port_names_match(e, &self.com_port))
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| self.com_port.clone())
        };
        egui::ComboBox::from_id_source(id)
            .width(combo_w)
            .selected_text(RichText::new(selected).color(C_TEXT).size(12.0))
            .show_ui(ui, |ui| {
                if choices.is_empty() {
                    ui.label(
                        RichText::new("No targets — Link USB or Wi‑Fi on Mine, or pick a COM.")
                            .color(C_WARN)
                            .size(12.0),
                    );
                }
                for (ep, label) in &choices {
                    if ui
                        .selectable_label(port_names_match(ep, &self.com_port), label)
                        .clicked()
                    {
                        self.com_port = ep.clone();
                        if let Some(w) = self
                            .connected_workers
                            .iter()
                            .find(|w| port_names_match(&w.endpoint, ep))
                        {
                            if !w.fw.is_empty() {
                                self.fw_label = w.fw.clone();
                            }
                            if !w.mac.is_empty() {
                                self.board_mac = w.mac.clone();
                            }
                        }
                    }
                }
            });
    }

    fn ui_firmware_drop_zone(&mut self, ui: &mut egui::Ui) {
        let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        let fill = if hovering {
            Color32::from_rgb(20, 60, 90)
        } else {
            C_PANEL_SOFT
        };
        Frame::none()
            .fill(fill)
            .stroke(Stroke::new(
                1.0,
                if hovering { C_LIME } else { C_STROKE },
            ))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    RichText::new(if hovering {
                        "Release to use this .bin for flash"
                    } else {
                        "Drop any .bin here to flash (merged @ 0x0 preferred)"
                    })
                    .color(if hovering { C_LIME } else { C_MUTED })
                    .size(12.0),
                );
                ui.label(
                    RichText::new(
                        "Kit images or any ESP32 .bin — then pick target and Push or Flash (BOOT)",
                    )
                    .color(C_DIM)
                    .size(11.0),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if soft_button(ui, "Use bundled image", 150.0).clicked() {
                        match find_firmware_image() {
                            Ok(img) => {
                                self.firmware = Some(img);
                                self.update_status = "Bundled merged.bin selected.".into();
                                self.last_ok = self.update_status.clone();
                            }
                            Err(e) => {
                                self.last_error = e;
                            }
                        }
                    }
                    if self.firmware.as_ref().map(firmware_is_custom).unwrap_or(false)
                        && soft_button(ui, "Clear custom", 120.0).clicked()
                    {
                        self.firmware = find_firmware_image().ok();
                        self.update_status = "Custom .bin cleared.".into();
                    }
                });
            });
    }

    fn ui_flash(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Flash board firmware")
                .color(C_LIME)
                .font(display_font(24.0)),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Push (USB) — app OTA, no BOOT. Push (Wi‑Fi) — TCP OTA. Flash (BOOT) — full rewrite for blank chips.",
            )
            .color(C_MUTED)
            .size(13.0),
        );
        ui.add_space(14.0);
        soft_panel(ui, "Firmware", |ui| {
            let (fw_status, fw_color) = self.firmware_status_label();
            ui.label(
                RichText::new(fw_status)
                    .color(fw_color)
                    .font(mono_ui_font(12.0)),
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                let fetch_label = if self.fetch_busy {
                    "Fetching FW…"
                } else {
                    "Fetch latest FW"
                };
                if soft_button(ui, fetch_label, 150.0).clicked() && !self.fetch_busy {
                    self.start_firmware_fetch();
                }
            });
            if let Some(fw) = &self.firmware {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "Board image · {} · {} KB{}",
                        if fw.version.is_empty() {
                            "unknown"
                        } else {
                            &fw.version
                        },
                        fw.bytes / 1024,
                        if firmware_is_custom(fw) {
                            " · custom"
                        } else {
                            ""
                        }
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
            self.ui_firmware_drop_zone(ui);
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
        });
        ui.add_space(14.0);
        soft_panel(ui, "Target & actions", |ui| {
            ui.horizontal(|ui| {
                self.ui_flash_target_combo(ui, "flash_tab_target");
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "Board fw · {}  ·  Target · {}",
                    if self.fw_label.is_empty() {
                        "—"
                    } else {
                        &self.fw_label
                    },
                    if self.com_port.is_empty() {
                        "—"
                    } else {
                        &self.com_port
                    },
                ))
                .color(C_MUTED)
                .font(mono_ui_font(11.0)),
            );
            ui.add_space(10.0);
            let wifi_target = Self::is_wifi_ota_endpoint(&self.com_port);
            let download_only =
                !wifi_target && self.board_is_download_mode(&self.com_port);
            let can_wifi = self.board_supports_wifi_ota(&self.com_port) || wifi_target;
            let show_push = !download_only && !wifi_target;
            let show_wifi = wifi_target || can_wifi;
            if download_only {
                ui.label(
                    RichText::new(
                        "Download mode — use Flash (BOOT): hold BOOT, tap RESET, click Ready when asked.",
                    )
                    .color(C_WARN)
                    .size(12.0),
                );
            } else if show_push {
                ui.label(
                    RichText::new(
                        "Push (USB) is the usual path on live boards. Flash (BOOT) for blank / full rewrite.",
                    )
                    .color(C_DIM)
                    .size(12.0),
                );
            }
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if show_push {
                    let push_label = if self.update_busy {
                        "Pushing…"
                    } else {
                        "Push (USB)"
                    };
                    if soft_button(ui, push_label, 120.0).clicked() && !self.update_busy {
                        self.begin_board_update(true);
                    }
                }
                if show_wifi {
                    let wifi_label = if self.update_busy {
                        "Pushing…"
                    } else {
                        "Push (Wi‑Fi)"
                    };
                    if soft_button(ui, wifi_label, 120.0).clicked() && !self.update_busy {
                        self.begin_board_update_wifi();
                    }
                }
                if !wifi_target {
                    let flash_label = if self.update_busy {
                        "Flashing…"
                    } else {
                        "Flash (BOOT)"
                    };
                    if soft_button(ui, flash_label, 130.0).clicked() && !self.update_busy {
                        self.begin_board_update(false);
                    }
                }
            });
        });
    }

    fn ui_erase(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Erase board flash")
                .color(C_LIME)
                .font(display_font(24.0)),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Full-chip erase wipes all firmware. After erase you must Flash (BOOT) to install again. USB only — needs espflash / ROM.",
            )
            .color(C_MUTED)
            .size(13.0),
        );
        ui.add_space(14.0);
        soft_panel(ui, "USB target", |ui| {
            let choices = self.usb_erase_target_choices();
            ui.label(
                RichText::new("COM port")
                    .color(C_MUTED)
                    .font(mono_ui_font(11.0)),
            );
            let combo_w = (ui.available_width() - 8.0).clamp(160.0, 420.0);
            let selected = if self.com_port.is_empty() {
                "— select USB COM —".to_string()
            } else {
                choices
                    .iter()
                    .find(|(e, _)| port_names_match(e, &self.com_port))
                    .map(|(_, l)| l.clone())
                    .unwrap_or_else(|| self.com_port.clone())
            };
            egui::ComboBox::from_id_source("erase_tab_target")
                .width(combo_w)
                .selected_text(RichText::new(selected).color(C_TEXT).size(12.0))
                .show_ui(ui, |ui| {
                    if choices.is_empty() {
                        ui.label(
                            RichText::new("No USB COM — plug in the CYD and Refresh on Mine.")
                                .color(C_WARN)
                                .size(12.0),
                        );
                    }
                    for (ep, label) in &choices {
                        if ui
                            .selectable_label(port_names_match(ep, &self.com_port), label)
                            .clicked()
                        {
                            self.com_port = ep.clone();
                        }
                    }
                });
            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "SAFETY: erase is irreversible until you re-flash. Hold BOOT → RESET → Ready when prompted.",
                )
                .color(C_WARN)
                .size(12.0),
            );
            ui.add_space(10.0);
            let erase_label = if self.update_busy {
                "Erasing…"
            } else {
                "Erase flash"
            };
            if soft_button(ui, erase_label, 140.0).clicked() && !self.update_busy {
                self.begin_board_erase();
            }
        });
    }

    fn ui_reset(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Reset board")
                .color(C_LIME)
                .font(display_font(24.0)),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Soft reboot of companion firmware (`cmp reboot`). Does not enter download mode.",
            )
            .color(C_MUTED)
            .size(13.0),
        );
        ui.add_space(14.0);
        soft_panel(ui, "Target", |ui| {
            let choices = self.reset_target_choices();
            ui.label(
                RichText::new("USB or Wi‑Fi board")
                    .color(C_MUTED)
                    .font(mono_ui_font(11.0)),
            );
            let combo_w = (ui.available_width() - 8.0).clamp(160.0, 420.0);
            let selected = if self.com_port.is_empty() {
                "— select board —".to_string()
            } else {
                choices
                    .iter()
                    .find(|(e, _)| port_names_match(e, &self.com_port))
                    .map(|(_, l)| l.clone())
                    .unwrap_or_else(|| self.com_port.clone())
            };
            egui::ComboBox::from_id_source("reset_tab_target")
                .width(combo_w)
                .selected_text(RichText::new(selected).color(C_TEXT).size(12.0))
                .show_ui(ui, |ui| {
                    if choices.is_empty() {
                        ui.label(
                            RichText::new("No boards — Link USB or Wi‑Fi on Mine first.")
                                .color(C_WARN)
                                .size(12.0),
                        );
                    }
                    for (ep, label) in &choices {
                        if ui
                            .selectable_label(port_names_match(ep, &self.com_port), label)
                            .clicked()
                        {
                            self.com_port = ep.clone();
                            if let Some(w) = self
                                .connected_workers
                                .iter()
                                .find(|w| port_names_match(&w.endpoint, ep))
                            {
                                if !w.fw.is_empty() {
                                    self.fw_label = w.fw.clone();
                                }
                            }
                        }
                    }
                });
            ui.add_space(10.0);
            if soft_button(ui, "Reset board", 140.0).clicked() {
                self.begin_board_reset();
            }
        });
    }

    fn clear_flash_overlay(&mut self) {
        self.update_busy = false;
        self.update_busy_since = None;
        self.pending_post_flash_reconnect = None;
        self.post_flash_verify = None;
        self.flash_progress = 0.0;
        self.flash_phase.clear();
        USB_FLASH_PREEMPT.store(false, Ordering::SeqCst);
        if let Some(c) = self.flash_cancel.take() {
            c.store(true, Ordering::SeqCst);
        }
        // Release COM hold immediately so Connect/Scan work even if the flash
        // tool thread is still winding down.
        if let Some(h) = self.flash_hold.take() {
            h.store(false, Ordering::SeqCst);
        }
        if let Some(n) = self.flash_need_boot.take() {
            n.store(false, Ordering::SeqCst);
        }
        if let Some(r) = self.flash_boot_ready.take() {
            r.store(false, Ordering::SeqCst);
        }
        // Restore USB open from whatever boards are still linked.
        self.usb_open = !self.connected_workers.is_empty();
        if self.usb_open {
            self.usb_connect_pending = false;
        }
    }

    /// Parse espflash / phase lines into overall 0..=1 progress for the overlay bar.
    fn absorb_flash_progress_line(&mut self, line: &str) {
        self.update_status = trunc(line, 140);
        let lower = line.to_ascii_lowercase();

        // Prefer explicit percent from OTA upload / espflash.
        if let Some(pct) = parse_flash_percent(line) {
            if lower.contains("upload") && (lower.contains("usb ota") || lower.contains("wi‑fi ota") || lower.contains("wi-fi ota") || lower.contains("ota")) {
                // OTA upload band: 15% (ready) → 85% (complete).
                let mapped = 0.15 + (pct / 100.0) * 0.70;
                if mapped > self.flash_progress {
                    self.flash_progress = mapped.clamp(0.0, 0.88);
                }
                self.flash_phase = format!("USB OTA upload · {pct:.0}%");
                return;
            }
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

        let (phase, floor) = if lower.contains("waiting for ready")
            || lower.contains("hold boot")
            || lower.contains("click ready")
            || lower.contains("auto-reset push stalled")
            || lower.contains("silent push stalled")
        {
            ("Hold BOOT — click Ready", 0.08)
        } else if lower.contains("ready — writing")
            || lower.contains("writing now")
            || lower.contains("keep boot held until writing")
        {
            ("Writing after Ready", 0.12)
        } else if lower.contains("chip seen")
            || (lower.contains("mac") && lower.contains("connect"))
            || lower.contains("chip connected")
        {
            ("Chip connected", 0.14)
        } else if lower.contains("write-bin")
            || lower.contains("write_flash")
            || lower.contains("writing firmware")
            || lower.contains("writing after")
            || lower.contains("patient write")
            || lower.contains("auto-reset")
            || lower.contains("push update round")
        {
            ("Pushing / writing", 0.12)
        } else if lower.contains("connecting to chip")
            || lower.contains("espflash write")
            || lower.contains("esptool write")
        {
            ("Connecting to chip", 0.10)
        } else if lower.contains("released")
            || lower.contains("releasing")
            || lower.contains("waiting for com")
        {
            ("Releasing USB", 0.05)
        } else if lower.contains("staging firmware") || lower.contains("flash staging")
        {
            ("Staging firmware", 0.07)
        } else if lower.contains("espflash")
            && (lower.contains("found") || lower.contains("download"))
        {
            ("Preparing flash tool", 0.08)
        } else if lower.contains("download") && (lower.contains("firmware") || lower.contains("kb"))
        {
            ("Downloading firmware", 0.06)
        } else if lower.contains("flash budget") {
            ("Preparing flash", 0.09)
        } else if lower.contains("wifi ota") || lower.contains("wi‑fi ota") || lower.contains("wi-fi ota")
        {
            ("Wi‑Fi OTA", 0.12)
        } else if lower.contains("usb ota")
            || lower.contains("live push — usb")
            || lower.contains("no boot, no hold")
        {
            ("USB OTA (no hold)", 0.12)
        } else if lower.contains("board ready") {
            ("Board ready for OTA", 0.15)
        } else if lower.contains("waiting for board ack")
            || (lower.contains("upload complete") && lower.contains("waiting"))
            || (lower.contains("no ota ack") && lower.contains("82%"))
        {
            ("Waiting for board ACK", 0.85)
        } else if lower.contains("pushed over wi")
            || lower.contains("pushed over usb")
            || lower.contains("usb ota pushed")
            || lower.contains("ota push complete")
            || lower.contains("soft-verified")
        {
            ("OTA push complete", 0.88)
        } else if lower.contains("cancelled") {
            ("Cancelled", self.flash_progress)
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
        } else {
            // Keep status text moving even when phase is unknown — avoids frozen 2%.
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
            attempts_left: 5,
            deadline: Instant::now() + Duration::from_secs(120),
        });
        let via = if Self::is_wifi_ota_endpoint(&port) {
            "Wi‑Fi"
        } else {
            "USB"
        };
        self.update_status = format!("Update OK — booting board, then verifying on {port} ({via})…");
        self.flash_phase = "Reconnecting".into();
        self.flash_progress = self.flash_progress.max(0.88);
        // USB OTA reboots shortly after ACK; give CH340 + companion listen time before verify.
        let delay = if Self::is_wifi_ota_endpoint(&port) {
            Duration::from_secs(10)
        } else {
            Duration::from_secs(10)
        };
        self.schedule_post_flash_reconnect(port, delay);
    }

    fn finish_post_flash_ok(&mut self, board_fw: &str) {
        let reboot_port = self
            .post_flash_verify
            .as_ref()
            .map(|v| v.port.clone())
            .filter(|p| !p.trim().is_empty())
            .unwrap_or_else(|| self.com_port.clone());
        let msg = if board_fw.is_empty() {
            "Update verified — board responded.".to_string()
        } else {
            format!("Update verified · board fw {board_fw}")
        };
        self.last_ok = msg.clone();
        self.last_error.clear();
        self.update_status = msg.clone();
        self.flash_phase = "Verified".into();
        self.flash_progress = 1.0;
        self.push_log(LogKind::Usb, msg);
        self.clear_flash_overlay();
        if !reboot_port.trim().is_empty() {
            self.pending_post_flash_reboot =
                Some((Instant::now() + Duration::from_secs(1), reboot_port));
        }
    }

    fn fail_post_flash_verify(&mut self, reason: String) {
        let tip = if self
            .post_flash_verify
            .as_ref()
            .map(|v| Self::is_wifi_ota_endpoint(&v.port))
            .unwrap_or_else(|| Self::is_wifi_ota_endpoint(&self.com_port))
        {
            format!(
                "{reason} Wait for the board to rejoin Wi‑Fi, then Find workers / Connect and retry Push update (Wi‑Fi)."
            )
        } else {
            format!(
                "{reason} Flash image is on the board — wait a few seconds, unplug/replug USB if needed, then Connect. Do not re-flash unless the board stays silent."
            )
        };
        self.update_status = tip.clone();
        self.last_error = tip.clone();
        self.push_log(LogKind::Err, tip);
        self.clear_flash_overlay();
        self.arm_flash_cooldown(8);
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
                // Board still on the old tag — Push/OTA did not stick. Fail verify so
                // the UI does not claim success (e.g. 0.8.174 after a reported 0.8.175 push).
                self.push_log(
                    LogKind::Warn,
                    format!("Post-flash tag differs · board {board_fw} · kit {kit}"),
                );
                let attempts = self
                    .post_flash_verify
                    .as_ref()
                    .map(|v| v.attempts_left)
                    .unwrap_or(0);
                if attempts > 0 {
                    self.retry_post_flash_verify(&format!(
                        "fw still {board_fw}, expected {kit}"
                    ));
                } else {
                    self.fail_post_flash_verify(format!(
                        "Update did not stick — board still {board_fw}, kit is {kit}. Retry Push update; if it fails again use Flash (BOOT)."
                    ));
                }
            }
            _ => {
                self.finish_post_flash_ok(board_fw);
            }
        }
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
                                    RichText::new({
                                        let n = self.connected_workers.len().max(if self.usb_open {
                                            1
                                        } else {
                                            0
                                        });
                                        if n > 1 {
                                            format!(
                                                "{} · fleet {} board{}",
                                                self.sha_mode_label(),
                                                n,
                                                if n == 1 { "" } else { "s" }
                                            )
                                        } else if self.usb_open {
                                            format!("{} · USB linked", self.sha_mode_label())
                                        } else {
                                            format!("{} · USB idle", self.sha_mode_label())
                                        }
                                    })
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
                    self.push_log_detail(
                        LogKind::Usb,
                        format!(
                            "closing linked workers={} · mining was {}",
                            self.connected_workers.len(),
                            if self.mining { "on" } else { "off" }
                        ),
                    );
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

    fn open_settings(&mut self, section: SettingsSection) {
        self.tab = Tab::Settings;
        self.settings_section = section;
    }

    fn ui_settings_shell(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (sec, label) in [
                (SettingsSection::General, "General"),
                (SettingsSection::Setup, "Setup"),
                (SettingsSection::Flash, "Flash"),
                (SettingsSection::Erase, "Erase"),
                (SettingsSection::Reset, "Reset"),
            ] {
                let selected = self.settings_section == sec;
                if ui
                    .add(egui::SelectableLabel::new(selected, label))
                    .clicked()
                {
                    self.settings_section = sec;
                }
            }
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        match self.settings_section {
            SettingsSection::General => self.ui_settings(ui),
            SettingsSection::Setup => self.ui_setup(ui),
            SettingsSection::Flash => self.ui_flash(ui),
            SettingsSection::Erase => self.ui_erase(ui),
            SettingsSection::Reset => self.ui_reset(ui),
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
                if self.app_update_busy
                    && soft_button(ui, "Cancel update", 120.0).clicked()
                {
                    self.cancel_app_update();
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
                    "Flash, erase, and reset live on their own tabs. Bench and fetch shortcuts stay here.",
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
                    "· Bench boards (D0) → lock fastest SHA path (often HW/SW)  ·  Clock 240 MHz  ·  More CYDs for more rate
· Algorithm stays Bitcoin SHA-256d only
· Do NOT raise board voltage — CYD is fixed ~3.3V; overvolting can kill flash/USB/ESP",
                )
                .color(C_MUTED)
                .font(mono_ui_font(11.0)),
            );
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                let bench_label = if self.bench_busy {
                    let secs = self
                        .bench_busy_since
                        .map(|t| t.elapsed().as_secs())
                        .unwrap_or(0);
                    format!("Benching… {secs}s")
                } else {
                    "Bench boards (D0)".into()
                };
                if soft_button(ui, &bench_label, 168.0).clicked() && !self.bench_busy {
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
                if soft_button(ui, "Open Flash", 140.0).clicked() {
                    self.go_flash_tab();
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
                        fw.bytes / 1024,
                    ))
                    .color(C_DIM)
                    .font(mono_ui_font(10.0)),
                );
            }
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

    fn ui_connection_controls(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Board & pool", |ui| {
            ui.label(
                RichText::new("PC USB link")
                    .color(C_LIME)
                    .font(mono_ui_font(12.0)),
            );
            ui.label(
                RichText::new(
                    "One data cable per board to the PC (USB‑C typical). Plug each CYD into its own USB port — they appear in Find workers within a few seconds.",
                )
                .color(C_MUTED)
                .size(11.0),
            );
            ui.add_space(4.0);
            // Wrap so Refresh stays clickable in the half-width Mine column
            // (fixed 320px combo + buttons used to clip past the column edge).
            ui.horizontal_wrapped(|ui| {
                let reserve = if self.usb_open { 340.0 } else { 220.0 };
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
                            } else if port_choice_is_system_junk(p) {
                                format!("{} · skip", p.label)
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
                        // Show every COM Windows reported — junk rows visible but not selectable.
                        let serial: Vec<PortChoice> = self
                            .ports
                            .iter()
                            .filter(|p| is_usb_serial_port(&p.name))
                            .cloned()
                            .collect();
                        if serial.is_empty() {
                            ui.label(
                                RichText::new(
                                    "No COM ports from Windows — plug a data cable, then Refresh.",
                                )
                                .color(C_WARN)
                                .size(12.0),
                            );
                        }
                        for p in serial {
                            let junk = port_choice_is_system_junk(&p);
                            let label = if self.worker_already_linked(&p.name) {
                                format!("{} · linked", p.label)
                            } else if junk {
                                format!("{} · motherboard / skip", p.label)
                            } else {
                                p.label.clone()
                            };
                            if junk {
                                ui.add_enabled(false, egui::Button::new(label));
                            } else {
                                ui.selectable_value(&mut self.com_port, p.name.clone(), label);
                            }
                        }
                    });
                if soft_button(ui, "Refresh", 98.0).clicked() {
                    // List only — never OpenUsb / auto-reconnect.
                    self.refresh_com_ports(false, false);
                    self.push_log(LogKind::Usb, self.last_ok.clone());
                    for p in self.ports.clone() {
                        self.push_log(LogKind::Usb, format!("  · {}", p.label));
                    }
                }
                // Offer multi-USB link whenever any board COM exists (not only after first link).
                if soft_button(ui, "Link all USB", 110.0).clicked() {
                    self.refresh_com_ports(false, false);
                    self.link_all_unlinked_usb();
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
                }
            });
            // Always-visible COM inventory — dropdown alone hid secondary USB adapters.
            {
                let serial: Vec<PortChoice> = self
                    .ports
                    .iter()
                    .filter(|p| is_usb_serial_port(&p.name))
                    .cloned()
                    .collect();
                let usb_n = count_usb_uart_ports(&self.ports);
                let junk_n = serial.len().saturating_sub(usb_n);
                ui.label(
                    RichText::new(format!(
                        "PC COMs · {} total · {usb_n} USB-UART · {junk_n} motherboard/BT skipped",
                        serial.len()
                    ))
                    .color(C_DIM)
                    .size(11.0),
                );
                for p in serial.iter().take(12) {
                    let linked = self.worker_already_linked(&p.name);
                    let junk = port_choice_is_system_junk(p);
                    let tag = if linked {
                        "linked"
                    } else if junk {
                        "skip"
                    } else {
                        "ready"
                    };
                    let color = if linked {
                        C_LIME
                    } else if junk {
                        C_DIM
                    } else {
                        C_TEXT
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("  · {} · {tag}", p.label))
                                .color(color)
                                .font(mono_ui_font(11.0)),
                        );
                        if !junk && !linked {
                            if soft_button(ui, "Use", 48.0).clicked() {
                                self.com_port = p.name.clone();
                            }
                            if soft_button(ui, "Link", 48.0).clicked() {
                                self.com_port = p.name.clone();
                                self.connect_or_add_usb();
                            }
                        }
                    });
                }
                if serial.len() > 12 {
                    ui.label(
                        RichText::new(format!(
                            "  · …and {} more — open the COM menu",
                            serial.len() - 12
                        ))
                        .color(C_DIM)
                        .size(11.0),
                    );
                }
            }
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
            } else if count_usb_uart_ports(&self.ports) <= 1 {
                if self.usb_open {
                    ui.label(
                        RichText::new(
                            "Only one PC USB COM so far. Plug a 2nd CYD data cable into another USB port — it should appear above within a few seconds (or click Refresh).",
                        )
                        .color(C_WARN)
                        .size(12.0),
                    );
                }
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
                "Bitcoin address or address.workername",
            );
            let resolved = stratum_worker_for_companion(self.edit_worker.trim());
            if !resolved.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "Pool registers as {resolved} — HMPool dashboard: paste the BTC address before the dot"
                    ))
                    .color(C_MUTED)
                    .size(11.0),
                );
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("Password").color(C_MUTED).size(12.0));
                ui.add(
                    TextEdit::singleline(&mut self.edit_password)
                        .desired_width(130.0)
                        .font(FontId::new(13.0, FontFamily::Monospace)),
                );
                ui.separator();
                ui.label(RichText::new("Clock").color(C_MUTED).size(12.0));
                if soft_button(ui, "−", 28.0).clicked() {
                    let mhz = cpu_mhz_nudge(self.target_mhz, false);
                    self.target_mhz = mhz;
                    let _ = self.cmd_tx.send(NetCmd::SetClock(mhz));
                    self.push_log(LogKind::Usb, format!("Clock request {mhz} MHz"));
                }
                for &mhz in CPU_MHZ_STEPS {
                    if mhz < 40 {
                        continue;
                    }
                    let selected = self.target_mhz == mhz;
                    if clock_chip(ui, mhz, selected).clicked() {
                        self.target_mhz = mhz;
                        let _ = self.cmd_tx.send(NetCmd::SetClock(mhz));
                        self.push_log(LogKind::Usb, format!("Clock request {mhz} MHz"));
                    }
                }
                if soft_button(ui, "+", 28.0).clicked() {
                    let mhz = cpu_mhz_nudge(self.target_mhz, true);
                    self.target_mhz = mhz;
                    let _ = self.cmd_tx.send(NetCmd::SetClock(mhz));
                    self.push_log(LogKind::Usb, format!("Clock request {mhz} MHz"));
                }
                ui.label(
                    RichText::new(format!("{} MHz", self.target_mhz))
                        .color(C_DIM)
                        .font(mono_ui_font(11.0)),
                );
            });
            ui.label(
                RichText::new(
                    "ESP32 clock steps: 10·20·40·80·160·240 MHz (hardware lock points — not continuous).",
                )
                .color(C_MUTED)
                .size(11.0),
            );
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
                if self.flash_busy() {
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
                    "Link each CYD with its own USB data cable (or Wi‑Fi after Setup). SoftAP on a board is for phone Setup — not a second worker.",
                )
                .color(C_MUTED)
                .size(11.0),
            );
        }

        if !self.connected_workers.is_empty() {
            ui.add_space(8.0);
            let dup_mac = self.connected_mac_collision();
            for w in self.connected_workers.clone() {
                ui.horizontal(|ui| {
                    let rate = format_hashrate(w.hashrate_hs);
                    let mac = if w.mac.is_empty() {
                        "mac?".to_string()
                    } else {
                        w.mac.clone()
                    };
                    ui.label(
                        RichText::new(format!(
                            "● {}  {rate}  · {mac} · {}",
                            w.endpoint,
                            if w.fw.is_empty() { "fw?" } else { &w.fw },
                        ))
                        .color(C_LIME)
                        .font(mono_ui_font(12.0)),
                    );
                    if soft_button(ui, "Drop", 64.0).clicked() {
                        let _ = self
                            .cmd_tx
                            .send(NetCmd::DisconnectWorker(w.endpoint.clone()));
                        self.push_log(LogKind::Usb, format!("Drop worker {}", w.endpoint));
                    }
                });
            }
            if dup_mac {
                ui.label(
                    RichText::new(
                        "Shared eFuse MAC on two links — boards are tracked by COM endpoint; rates above are per board.",
                    )
                    .color(C_WARN)
                    .size(11.0),
                );
            }
        }

        let usb_found: Vec<_> = self
            .discovered_workers
            .iter()
            .filter(|w| w.kind == WorkerKind::Usb)
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
                RichText::new("USB CYD boards")
                    .color(C_MUTED)
                    .size(12.0),
            );
            for w in usb_found {
                let already = self.worker_already_linked(&w.endpoint);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new({
                            let mac = if w.mac.is_empty() { "mac?" } else { &w.mac };
                            format!("USB · {mac} · {} · {}", w.endpoint, w.detail)
                        })
                        .color(C_TEXT)
                        .font(mono_ui_font(11.0)),
                    );
                    if already {
                        ui.label(RichText::new("linked").color(C_LIME).size(11.0));
                    } else {
                        let no_cmp = w.detail.to_ascii_lowercase().contains("no cmp");
                        let btn = if no_cmp { "Link anyway" } else { "Connect" };
                        if soft_button(ui, btn, 100.0).clicked() {
                            if no_cmp {
                                self.push_log(
                                    LogKind::Warn,
                                    format!(
                                        "{} has no cmp reply yet — linking may enter download-mode / need Update board",
                                        w.endpoint
                                    ),
                                );
                            }
                            self.queue_connect_usb(w.endpoint.clone(), "Find workers");
                        }
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
                    } else {
                        let softap = wifi_is_softap_setup(&w);
                        let btn = if softap { "Set up Wi‑Fi" } else { "Connect" };
                        if soft_button(ui, btn, 120.0)
                            .on_hover_text(if softap {
                                "Open Setup to push home Wi‑Fi (PC must be on open Njordr-XXXX SoftAP)."
                            } else {
                                "TCP cmp to board on your LAN / SoftAP."
                            })
                            .clicked()
                        {
                            if softap {
                                // SoftAP is for provisioning — route to Setup, don't mine-link.
                                self.route_to_board_wifi_setup(
                                    Some(w.endpoint.clone()),
                                    true,
                                );
                            } else {
                                // Prefer USB for the same MAC when available.
                                let prefer_usb = self
                                    .discovered_workers
                                    .iter()
                                    .find(|d| {
                                        d.kind == WorkerKind::Usb
                                            && mac_is_stable(&d.mac)
                                            && mac_is_stable(&w.mac)
                                            && normalize_mac(&d.mac) == normalize_mac(&w.mac)
                                            && !self.worker_already_linked(&d.endpoint)
                                    })
                                    .map(|d| d.endpoint.clone());
                                if let Some(usb_ep) = prefer_usb {
                                    self.push_log(
                                        LogKind::Usb,
                                        format!(
                                            "Prefer USB {usb_ep} over Wi‑Fi {} (same MAC {})",
                                            w.endpoint, w.mac
                                        ),
                                    );
                                    self.queue_connect_usb(usb_ep, "Wi‑Fi row → USB");
                                } else {
                                    self.queue_connect_wifi(w.endpoint.clone(), "Find workers");
                                }
                            }
                        }
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

        ui.add_space(12.0);
        self.ui_wifi_setup_mine_callout(ui);
    }

    fn fresh_softap_board(&self) -> Option<&DiscoveredWorker> {
        self.discovered_workers.iter().find(|w| {
            w.kind == WorkerKind::Wifi
                && wifi_is_softap_setup(w)
                && wifi_beacon_fresh(w, 20_000)
        })
    }

    /// Switch to Setup and preselect a SoftAP/USB target for home Wi‑Fi push.
    fn route_to_board_wifi_setup(&mut self, endpoint: Option<String>, announce: bool) {
        if matches!(
            self.wifi_setup_phase,
            WifiSetupPhase::Pushing | WifiSetupPhase::WaitingSta { .. }
        ) {
            // Don't yank the user mid-push.
            self.open_settings(SettingsSection::Setup);
            return;
        }
        if let Some(ep) = endpoint {
            self.wifi_setup_target = ep.clone();
            self.softap_setup_routed_ep = ep;
        }
        self.open_settings(SettingsSection::Setup);
        if announce {
            self.last_ok =
                "Board SoftAP connected — enter home Wi‑Fi SSID/password, then Push & save."
                    .into();
            self.push_log(
                LogKind::Info,
                "Routed to Setup — push home Wi‑Fi to the board (SoftAP is open / no password)."
                    .into(),
            );
        }
    }

    /// When the PC is on Njordr SoftAP, SoftAP beacons arrive — open Setup once.
    fn maybe_route_softap_wifi_setup(&mut self) {
        let Some(ep) = self.fresh_softap_board().map(|w| w.endpoint.clone()) else {
            if !self.softap_setup_routed_ep.is_empty() {
                self.softap_setup_routed_ep.clear();
            }
            return;
        };
        if self.softap_setup_routed_ep == ep {
            return;
        }
        self.route_to_board_wifi_setup(Some(ep), true);
    }

    fn ui_wifi_setup_mine_callout(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Board Wi‑Fi", |ui| {
            if let Some(w) = self.fresh_softap_board().cloned() {
                ui.label(
                    RichText::new(format!(
                        "SoftAP {} — PC is on the board network. Open Setup to push home Wi‑Fi \
(SoftAP has no password).",
                        w.endpoint
                    ))
                    .color(C_WARN)
                    .size(12.0),
                );
                ui.add_space(6.0);
                if soft_button(ui, "Open Setup", 120.0).clicked() {
                    self.route_to_board_wifi_setup(Some(w.endpoint), true);
                }
            } else {
                ui.label(
                    RichText::new(
                        "Join open SoftAP Njordr-XXXX (no password) on phone or PC — \
phone opens Board Setup automatically (captive Sign-in), or use Setup / USB here.",
                    )
                    .color(C_DIM)
                    .size(12.0),
                );
                ui.add_space(6.0);
                if soft_button(ui, "Open Setup", 120.0).clicked() {
                    self.open_settings(SettingsSection::Setup);
                }
            }
        });
    }

    fn ui_setup(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Njörðr")
                .color(C_LIME)
                .font(display_font(28.0)),
        );
        ui.label(
            RichText::new("Board → home Wi‑Fi")
                .color(C_TEXT)
                .font(display_font(20.0)),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Connect the board to your router so it can mine on the LAN.",
            )
            .color(C_MUTED)
            .size(13.0),
        );
        ui.add_space(14.0);

        soft_panel(ui, "How to connect", |ui| {
            let softap = self.fresh_softap_board();
            let step1 = if softap.is_some() {
                "1. SoftAP is up — phone should auto-open Board Setup (Sign-in). \
Or open http://10.88.88.1/ (SoftAP). After save, board prefers 192.168.1.88 on home LAN else DHCP. PC: form below."
            } else {
                "1. Join open SoftAP Njordr-XXXX (no password). Phone auto-opens Board Setup \
(captive Sign-in → http://10.88.88.1/). PC: form below or USB Link."
            };
            ui.label(
                RichText::new(step1)
                    .color(if softap.is_some() { C_LIME } else { C_TEXT })
                    .size(12.0),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("2. Enter your home router SSID and password below.")
                    .color(C_TEXT)
                    .size(12.0),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "3. Push & save — board stores credentials in NVS and joins home Wi‑Fi.",
                )
                .color(C_TEXT)
                .size(12.0),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "4. Switch this PC back to home Wi‑Fi, then Find CYD workers on Mine.",
                )
                .color(C_TEXT)
                .size(12.0),
            );
            if let Some(w) = softap {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "SoftAP board · {} · {}",
                        w.endpoint,
                        if w.mac.is_empty() { "mac?" } else { &w.mac }
                    ))
                    .color(C_LIME)
                    .font(mono_ui_font(11.0)),
                );
            }
        });

        ui.add_space(14.0);
        self.ui_wifi_setup_panel(ui);
    }

    fn wifi_setup_target_choices(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for w in &self.connected_workers {
            let label = if w.mac.is_empty() {
                format!("{} · linked", w.endpoint)
            } else {
                format!("{} · {} · linked", w.endpoint, w.mac)
            };
            out.push((w.endpoint.clone(), label));
        }
        for w in &self.discovered_workers {
            if w.kind != WorkerKind::Wifi && w.kind != WorkerKind::Usb {
                continue;
            }
            if out.iter().any(|(e, _)| e == &w.endpoint) {
                continue;
            }
            if self.worker_already_linked(&w.endpoint) || self.worker_mac_already_linked(&w.mac) {
                continue;
            }
            let label = format!("{} · {}", w.endpoint, w.detail);
            out.push((w.endpoint.clone(), label));
        }
        // SoftAP beacons stay visible while a USB board is linked. Prefer linked
        // USB/TCP for Push — unless we just routed here from SoftAP join.
        let prefer_softap = !self.softap_setup_routed_ep.is_empty();
        out.sort_by(|a, b| {
            let rank = |ep: &str, label: &str| -> u8 {
                let l = label.to_ascii_lowercase();
                if prefer_softap && ep == self.softap_setup_routed_ep {
                    0
                } else if l.contains("· linked") || l.ends_with(" linked") {
                    1
                } else if l.contains("setup softap") {
                    3
                } else {
                    2
                }
            };
            rank(&a.0, &a.1)
                .cmp(&rank(&b.0, &b.1))
                .then(a.0.cmp(&b.0))
        });
        out
    }

    fn begin_wifi_setup_push(&mut self) {
        let ssid = self.wifi_setup_ssid.trim().to_string();
        if ssid.is_empty() {
            self.last_error = "Enter the home Wi‑Fi SSID.".into();
            return;
        }
        let mut endpoint = self.wifi_setup_target.trim().to_string();
        if endpoint.is_empty() {
            // Prefer a linked USB board when auto-picking.
            if let Some((e, _)) = self.wifi_setup_target_choices().into_iter().next() {
                endpoint = e;
                self.wifi_setup_target = endpoint.clone();
            }
        }
        if endpoint.is_empty() {
            self.last_error =
                "Link a USB board or join open SoftAP Njordr-XXXX (no password), then Find workers."
                    .into();
            return;
        }
        self.wifi_setup_phase = WifiSetupPhase::Pushing;
        self.last_ok = format!("Pushing Wi‑Fi “{ssid}” to {endpoint}…");
        self.last_error.clear();
        self.push_log(
            LogKind::Usb,
            format!("Wi‑Fi setup → save SSID “{ssid}” on {endpoint} (password not logged)"),
        );
        let _ = self.cmd_tx.send(NetCmd::SetBoardWifi {
            endpoint,
            ssid,
            pass: self.wifi_setup_pass.clone(),
            enable: true,
        });
    }

    fn on_wifi_setup_pushed(&mut self, endpoint: String, ok: Result<String, String>) {
        match ok {
            Ok(msg) => {
                let home_ssid = self.wifi_setup_ssid.trim().to_string();
                // USB link already has cmp — credentials are in NVS; no SoftAP STA wait.
                if is_usb_serial_port(&endpoint) {
                    self.wifi_setup_phase = WifiSetupPhase::Done {
                        ip: endpoint.clone(),
                        ssid: home_ssid.clone(),
                    };
                    self.last_ok = format!(
                        "{msg} — Wi‑Fi “{home_ssid}” saved over USB ({endpoint}). Board joins home Wi‑Fi; SoftAP may stay for Setup revisit."
                    );
                    self.push_log(LogKind::Usb, self.last_ok.clone());
                    self.push_log(
                        LogKind::Usb,
                        format!(
                            "  · NVS write confirmed · path=USB · endpoint={endpoint} · SSID “{home_ssid}” · SoftAP may stay for Setup"
                        ),
                    );
                    return;
                }
                let mac = self
                    .connected_workers
                    .iter()
                    .find(|w| w.endpoint == endpoint)
                    .map(|w| w.mac.clone())
                    .or_else(|| {
                        self.discovered_workers
                            .iter()
                            .find(|w| w.endpoint == endpoint)
                            .map(|w| w.mac.clone())
                    })
                    .unwrap_or_default();
                self.wifi_setup_phase = WifiSetupPhase::WaitingSta {
                    since: Instant::now(),
                    endpoint: endpoint.clone(),
                    mac: mac.clone(),
                    home_ssid: home_ssid.clone(),
                    last_progress_log: Instant::now(),
                };
                self.last_ok = format!(
                    "{msg} — waiting for board on “{home_ssid}” (switch PC back to home Wi‑Fi)"
                );
                self.push_log(LogKind::Usb, self.last_ok.clone());
                self.push_log(
                    LogKind::Usb,
                    format!(
                        "  · STA wait started · SoftAP was {endpoint} · mac={} · home SSID “{home_ssid}” · timeout 90s",
                        if mac.is_empty() { "unknown" } else { mac.as_str() }
                    ),
                );
            }
            Err(e) => {
                self.wifi_setup_phase = WifiSetupPhase::Failed(e.clone());
                self.last_error = e.clone();
                let ep = if endpoint.is_empty() {
                    self.wifi_setup_target.trim().to_string()
                } else {
                    endpoint
                };
                self.push_log(
                    LogKind::Err,
                    format!(
                        "Wi‑Fi Push & save FAILED{}: {e}",
                        if ep.is_empty() {
                            String::new()
                        } else {
                            format!(" on {ep}")
                        }
                    ),
                );
            }
        }
    }

    fn on_wifi_setup_cleared(&mut self, endpoint: String, ok: Result<String, String>) {
        match ok {
            Ok(msg) => {
                self.wifi_setup_phase = WifiSetupPhase::Idle;
                self.last_ok = format!("{msg} — board SoftAP Njordr-XXXX (open / no password)");
                self.last_error.clear();
                self.push_log(LogKind::Usb, format!("Wi‑Fi cleared on {endpoint}: {msg}"));
            }
            Err(e) => {
                self.wifi_setup_phase = WifiSetupPhase::Failed(e.clone());
                self.last_error = e.clone();
                self.push_log(
                    LogKind::Err,
                    format!("Wi‑Fi clear FAILED on {endpoint}: {e}"),
                );
            }
        }
    }

    fn tick_wifi_setup_wait(&mut self) {
        let (since, endpoint, mac, home_ssid, last_progress_log) = match &self.wifi_setup_phase {
            WifiSetupPhase::WaitingSta {
                since,
                endpoint,
                mac,
                home_ssid,
                last_progress_log,
            } => (
                *since,
                endpoint.clone(),
                mac.clone(),
                home_ssid.clone(),
                *last_progress_log,
            ),
            _ => return,
        };
        if since.elapsed() > Duration::from_secs(90) {
            let msg = format!(
                "Timed out waiting for home Wi‑Fi “{home_ssid}” after SoftAP push ({endpoint}). Re-join SoftAP or check SSID/password."
            );
            self.wifi_setup_phase = WifiSetupPhase::Failed(msg.clone());
            self.last_error = msg.clone();
            self.push_log(LogKind::Err, msg);
            return;
        }
        if last_progress_log.elapsed() >= Duration::from_secs(15) {
            if let WifiSetupPhase::WaitingSta {
                last_progress_log, ..
            } = &mut self.wifi_setup_phase
            {
                *last_progress_log = Instant::now();
            }
            let secs = since.elapsed().as_secs();
            self.push_log(
                LogKind::Usb,
                format!(
                    "  · still waiting for STA on “{home_ssid}”… {secs}s / 90s · SoftAP was {endpoint}"
                ),
            );
        }
        let softap_host = endpoint.split(':').next().unwrap_or("").to_string();
        // Success: beacon shows STA/apsta and a different IP than the SoftAP push endpoint.
        for w in &self.discovered_workers {
            let mac_match = mac_is_stable(&mac)
                && mac_is_stable(&w.mac)
                && normalize_mac(&mac) == normalize_mac(&w.mac);
            if !mac_match && w.endpoint != endpoint {
                continue;
            }
            let detail = w.detail.to_ascii_lowercase();
            if detail.contains("setup softap") {
                continue;
            }
            let on_lan = detail.contains("apsta")
                || detail.contains("wi-fi sta")
                || detail.contains("wi‑fi sta")
                || (detail.contains("sta") && !detail.contains("softap"));
            let ip = w.host.clone();
            if on_lan && !ip.is_empty() && ip != softap_host {
                let beacon = w.endpoint.clone();
                let detail_snip: String = w.detail.chars().take(120).collect();
                self.wifi_setup_phase = WifiSetupPhase::Done {
                    ip: ip.clone(),
                    ssid: home_ssid.clone(),
                };
                self.last_ok = format!("Board on “{home_ssid}” at {ip}");
                self.push_log(LogKind::Usb, self.last_ok.clone());
                self.push_log(
                    LogKind::Usb,
                    format!("  · STA joined · beacon {beacon} · detail={detail_snip}"),
                );
                return;
            }
        }
        for w in &self.connected_workers {
            let mac_match = mac_is_stable(&mac)
                && mac_is_stable(&w.mac)
                && normalize_mac(&mac) == normalize_mac(&w.mac);
            if !mac_match && w.endpoint != endpoint {
                continue;
            }
            if w.endpoint.contains(':') && !w.endpoint.to_ascii_uppercase().starts_with("COM") {
                let ip = w.endpoint.split(':').next().unwrap_or("").to_string();
                if !ip.is_empty() && ip != softap_host {
                    let mac = w.mac.clone();
                    self.wifi_setup_phase = WifiSetupPhase::Done {
                        ip: ip.clone(),
                        ssid: home_ssid.clone(),
                    };
                    self.last_ok = format!("Board on “{home_ssid}” at {ip}");
                    self.push_log(LogKind::Usb, self.last_ok.clone());
                    self.push_log(
                        LogKind::Usb,
                        format!("  · STA joined via linked worker {ip} · mac={mac}"),
                    );
                    return;
                }
            }
        }
    }

    fn ui_wifi_setup_panel(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Home Wi‑Fi credentials", |ui| {
            ui.label(
                RichText::new(
                    "Select the SoftAP or linked USB board, enter home Wi‑Fi, then Push & save — \
credentials write to board NVS over cmp. SoftAP Njordr-XXXX is open (no password). \
Phone: join SoftAP — Board Setup opens automatically to set home Wi‑Fi.",
                )
                .color(C_DIM)
                .size(12.0),
            );
            ui.add_space(8.0);
            let choices = self.wifi_setup_target_choices();
            if self.wifi_setup_target.is_empty() {
                if let Some((e, _)) = choices.first() {
                    self.wifi_setup_target = e.clone();
                }
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("Board").color(C_MUTED).size(12.0));
                let combo_w = (ui.available_width() - 80.0).clamp(160.0, 420.0);
                egui::ComboBox::from_id_source("wifi_setup_target")
                    .width(combo_w)
                    .selected_text(if self.wifi_setup_target.is_empty() {
                        if choices.is_empty() {
                            "— join SoftAP or link USB —".into()
                        } else {
                            choices[0].1.clone()
                        }
                    } else {
                        choices
                            .iter()
                            .find(|(e, _)| e == &self.wifi_setup_target)
                            .map(|(_, l)| l.clone())
                            .unwrap_or_else(|| self.wifi_setup_target.clone())
                    })
                    .show_ui(ui, |ui| {
                        for (ep, label) in &choices {
                            ui.selectable_value(&mut self.wifi_setup_target, ep.clone(), label);
                        }
                    });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Home SSID").color(C_MUTED).size(12.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.wifi_setup_ssid)
                        .desired_width(220.0)
                        .hint_text("Your router Wi‑Fi name"),
                );
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Password").color(C_MUTED).size(12.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.wifi_setup_pass)
                        .desired_width(220.0)
                        .password(true)
                        .hint_text("Wi‑Fi password"),
                );
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let busy = matches!(
                    self.wifi_setup_phase,
                    WifiSetupPhase::Pushing | WifiSetupPhase::WaitingSta { .. }
                );
                if soft_button(ui, if busy { "Working…" } else { "Push & save" }, 120.0).clicked()
                    && !busy
                {
                    self.begin_wifi_setup_push();
                }
                if soft_button(ui, "Clear STA", 100.0).clicked() && !busy {
                    let endpoint = if self.wifi_setup_target.is_empty() {
                        self.wifi_setup_target_choices()
                            .into_iter()
                            .next()
                            .map(|(e, _)| e)
                            .unwrap_or_default()
                    } else {
                        self.wifi_setup_target.clone()
                    };
                    if endpoint.is_empty() {
                        self.last_error = "Select a board first.".into();
                    } else {
                        self.wifi_setup_phase = WifiSetupPhase::Pushing;
                        let _ = self.cmd_tx.send(NetCmd::ClearBoardWifi { endpoint });
                        self.push_log(LogKind::Usb, "Clearing board STA Wi‑Fi credentials…".into());
                    }
                }
            });
            ui.add_space(4.0);
            let status = match &self.wifi_setup_phase {
                WifiSetupPhase::Idle => "Idle — SoftAP stays on after save for Setup revisit.".to_string(),
                WifiSetupPhase::Pushing => "Saving to board NVS…".into(),
                WifiSetupPhase::WaitingSta { home_ssid, .. } => format!(
                    "Waiting for board on “{home_ssid}”… switch this PC back to home Wi‑Fi, then Find workers."
                ),
                WifiSetupPhase::Done { ip, ssid } => {
                    format!("Saved — board on “{ssid}” at {ip}")
                }
                WifiSetupPhase::Failed(e) => format!("Failed — {e}"),
            };
            ui.label(
                RichText::new(status)
                    .color(match &self.wifi_setup_phase {
                        WifiSetupPhase::Failed(_) => C_ERR,
                        WifiSetupPhase::Done { .. } => C_LIME,
                        WifiSetupPhase::WaitingSta { .. } | WifiSetupPhase::Pushing => C_WARN,
                        WifiSetupPhase::Idle => C_DIM,
                    })
                    .size(12.0),
            );
        });
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
                // Connection-local pool counters (reset on re-authorize) — often match
                // the pool website window better than session chips after reconnects.
                if self.stratum_live.authorized {
                    mini_stat(
                        ui,
                        "Pool ok",
                        &self.stratum_live.accepted.to_string(),
                    );
                    mini_stat(
                        ui,
                        "Pool rj",
                        &self.stratum_live.rejected.to_string(),
                    );
                }
                mini_stat(ui, "Accept%", &self.accept_rate_label());
                mini_stat(ui, "Session", &self.session_elapsed_label());
                mini_stat(ui, "Luck", &self.luck_label());
                mini_stat(ui, "Expect/h", &self.expected_shares_label());
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                // Use smoothed display rate so soft-fails don't blink the chip.
                let rate_hs = self.fleet_hashrate_hs();
                mini_stat(ui, "Fleet", &format_hashrate(rate_hs));
                let pool_hs = self.pool_estimated_hs();
                mini_stat(
                    ui,
                    "Pool≈",
                    &if pool_hs > 0.0 {
                        format_hashrate(pool_hs)
                    } else {
                        "—".into()
                    },
                );
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
                mini_stat(
                    ui,
                    "Boards",
                    &self.connected_workers.len().max(if self.usb_open { 1 } else { 0 }).to_string(),
                );
            });
            if !self.connected_workers.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Per-board rate")
                        .color(C_MUTED)
                        .size(11.0),
                );
                for w in &self.connected_workers {
                    ui.label(
                        RichText::new(format!(
                            "  {}  {}{}{}",
                            w.endpoint,
                            format_hashrate(w.hashrate_hs),
                            if w.mine_indep { " · indep" } else { "" },
                            if w.mining { "" } else { " · idle" }
                        ))
                        .color(if w.hashrate_hs > 0.0 { C_LIME } else { C_DIM })
                        .font(mono_ui_font(11.0)),
                    );
                }
            }
            ui.label(
                RichText::new(
                    "SHA path · HW = full silicon · HW+ = midstate · HW/SW = hybrid (often peak on D0)",
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
                mini_stat(ui, "Msgs↑", &s.lines_tx.to_string());
                mini_stat(ui, "Msgs↓", &s.lines_rx.to_string());
                mini_stat(ui, "Jobs", &s.jobs.to_string());
                mini_stat(ui, "Submits", &s.submits.to_string());
                mini_stat(ui, "Pending", &s.pending.to_string());
                mini_stat(ui, "Accept", &acc.to_string());
                mini_stat(ui, "Reject", &rej.to_string());
                mini_stat(ui, "Expect/h", &self.expected_shares_label());
                mini_stat(
                    ui,
                    "Job id",
                    if s.last_job.is_empty() { "—" } else { &s.last_job },
                );
            });
            ui.label(
                RichText::new(
                    "Submits = mining.submit this session. Pending = awaiting pool reply (timeout 90s → Reject). Accept+Reject+Pending ≈ Submits.",
                )
                .color(C_DIM)
                .font(mono_ui_font(10.0)),
            );
            let hs = self.fleet_hashrate_hs();
            let exp = expected_shares_per_hour(hs, s.difficulty);
            if s.authorized && s.difficulty >= 0.05 && hs > 50_000.0 && exp < 5.0
            {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "Share diff {:.3} is hard for ~{:.0} kH/s (≈{:.1} accepts/h). Use an ESP pool port or wait for vardiff after suggest 0.01.",
                        s.difficulty,
                        hs / 1000.0,
                        exp
                    ))
                    .color(C_ERR)
                    .size(12.0),
                );
            }
            ui.add_space(8.0);
            stratum_line(ui, "Last TX → pool", &trunc(&s.last_tx, 150));
            stratum_line(ui, "Last RX ← pool", &trunc(&s.last_rx, 150));
            if !s.last_error.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Pool error · {}", trunc(&s.last_error, 160)))
                        .color(C_ERR)
                        .size(12.0),
                );
                ui.label(
                    RichText::new(
                        "Fix the worker / Bitcoin address on Mine, then Start mining again.",
                    )
                    .color(C_MUTED)
                    .size(11.0),
                );
            } else if self.mining && !s.authorized {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "Waiting for pool authorize (phase {})… Accept/Reject stay 0 until AUTHORIZED.",
                        if s.phase.is_empty() { "…" } else { &s.phase }
                    ))
                    .color(C_WARN)
                    .size(11.0),
                );
            }
            ui.add_space(6.0);
            if soft_button(ui, "Copy stratum snapshot", 180.0).clicked() {
                let snap = format!(
                    "endpoint={} phase={} diff={} submits={} pending={} acc={} rej={} job={}\nTX {}\nRX {}\n",
                    s.endpoint,
                    s.phase,
                    s.difficulty,
                    s.submits,
                    s.pending,
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
                        .max_height(240.0)
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
                ui.add_space(6.0);
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
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            self.absorb_dropped_files(dropped);
        }
        while let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
                NetMsg::Ports(p) => {
                    let n = p.len();
                    self.ports = p;
                    self.apply_best_com_port(false);
                    let sel = if self.com_port.is_empty() {
                        "—"
                    } else {
                        self.com_port.as_str()
                    };
                    // Mine-worker re-enums COM often — only log when list/selection changes.
                    let sig = format!(
                        "{n}|{sel}|{}",
                        self.ports
                            .iter()
                            .map(|x| x.name.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                    if self.last_ports_log_sig.as_deref() != Some(sig.as_str()) {
                        self.last_ports_log_sig = Some(sig);
                        let listed: Vec<&str> = self
                            .ports
                            .iter()
                            .map(|x| x.name.as_str())
                            .take(10)
                            .collect();
                        let more = if n > listed.len() {
                            format!(" +{} more", n - listed.len())
                        } else {
                            String::new()
                        };
                        self.push_log(
                            LogKind::Usb,
                            format!(
                                "Serial ports: {n} reported · selected {sel} · [{}{more}]",
                                listed.join(", ")
                            ),
                        );
                    }
                    self.maybe_auto_connect_usb();
                }
                NetMsg::BenchProgress(line) => {
                    self.absorb_bench_progress(&line);
                }
                NetMsg::Action(Ok(s)) => {
                    if matches!(self.wifi_setup_phase, WifiSetupPhase::Pushing) {
                        if let Some(rest) = s.strip_prefix("BOARD_WIFI_SAVED|") {
                            let mut parts = rest.splitn(2, '|');
                            let ep = parts.next().unwrap_or("").to_string();
                            let msg = parts.next().unwrap_or("saved").to_string();
                            self.on_wifi_setup_pushed(ep, Ok(msg));
                            continue;
                        }
                        if let Some(rest) = s.strip_prefix("BOARD_WIFI_CLEARED|") {
                            let mut parts = rest.splitn(2, '|');
                            let ep = parts.next().unwrap_or("").to_string();
                            let msg = parts.next().unwrap_or("cleared").to_string();
                            self.on_wifi_setup_cleared(ep, Ok(msg));
                            continue;
                        }
                    }
                    self.last_ok = s.clone();
                    let low = s.to_lowercase();
                    if low.contains("usb open") || low.contains("worker linked") {
                        self.usb_open = true;
                        self.usb_connect_pending = false;
                        // Keep the COM that just linked selected — do NOT jump to PCI COM1.
                        if self.post_flash_verify.is_some() && low.contains("usb open") {
                            if low.contains("download mode") {
                                self.fail_post_flash_verify(
                                    "Flash wrote but board stayed in download mode (BOOT)."
                                        .into(),
                                );
                            } else {
                                self.absorb_flash_progress_line(
                                    "USB linked after flash — reading board config…",
                                );
                            }
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
                        if low.contains("cmpbench") || low.contains("khs=") {
                            self.absorb_bench_progress(&s);
                        }
                        self.clear_bench_busy(&format!("Bench finished · {}", trunc(&s, 100)));
                    }
                    if low.contains("bench failed") && low.contains("timeout") {
                        self.clear_bench_busy(&format!("Bench timed out · {}", trunc(&s, 100)));
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
                    if matches!(self.wifi_setup_phase, WifiSetupPhase::Pushing) {
                        if let Some(rest) = e.strip_prefix("BOARD_WIFI_ERR|") {
                            let mut parts = rest.splitn(2, '|');
                            let ep = parts.next().unwrap_or("").to_string();
                            let msg = parts.next().unwrap_or(rest).to_string();
                            self.on_wifi_setup_pushed(ep, Err(msg));
                            continue;
                        }
                        if e.to_lowercase().contains("wi-fi") || e.to_lowercase().contains("wifi") {
                            self.on_wifi_setup_pushed(String::new(), Err(e.clone()));
                            continue;
                        }
                    }
                    self.usb_connect_pending = false;
                    if self.bench_busy {
                        self.clear_bench_busy(&format!("Bench aborted · {}", trunc(&e, 100)));
                    }
                    let low = e.to_lowercase();
                    if low.contains("authorize failed") || low.contains("auth failed") {
                        self.mining = false;
                        self.session_started = None;
                    }
                    // Ghost COM / unplugged — drop dead selection and re-enum live ports.
                    if low.contains("usb open failed")
                        || low.contains("cannot find the file")
                        || low.contains("ghost registry")
                        || low.contains("link ") && low.contains("failed")
                    {
                        let dead = self.com_port.clone();
                        self.refresh_com_ports(false, false);
                        if !dead.is_empty()
                            && !self
                                .ports
                                .iter()
                                .any(|p| port_names_match(&p.name, &dead) && !port_choice_is_system_junk(p))
                        {
                            self.com_port.clear();
                            self.apply_best_com_port(true);
                        }
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
                    if CPU_MHZ_STEPS.contains(&c.cpu_mhz) {
                        self.target_mhz = c.cpu_mhz;
                    } else if c.cpu_mhz > 0 {
                        self.target_mhz = normalize_cpu_mhz(c.cpu_mhz);
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
                    let was_connected = self.stratum_live.connected;
                    let prev_jobs = self.stratum_live.jobs;
                    self.stratum_live = live;
                    if self.stratum_live.jobs > prev_jobs {
                    }
                    if self.stratum_live.authorized {
                        self.pool_auth_hold_until =
                            Some(Instant::now() + Duration::from_secs(15));
                        if !was_authed {
                            // Fresh authorize: HUD stays at 0 until real post-grace outcomes arrive.
                            // Do not wipe session counters here — Share msgs can race ahead of this.
                            self.accepted = 0;
                            self.rejected = 0;
                            self.push_log(
                                LogKind::Stratum,
                                "Authorized — counting shares after warmup".into(),
                            );
                        }
                    }
                    if self.stratum_live.phase == "auth-fail"
                        || !self.stratum_live.last_error.is_empty()
                    {
                        self.pool_auth_hold_until = None;
                        if self.mining && !self.stratum_live.authorized {
                            self.mining = false;
                            self.session_started = None;
                        }
                        if !self.stratum_live.last_error.is_empty() {
                            self.last_error = format!(
                                "Pool: {}",
                                trunc(&self.stratum_live.last_error, 120)
                            );
                        }
                    }
                    if self.stratum_live.authorized {
                        self.accepted = self.stratum_live.accepted;
                        self.rejected = self.stratum_live.rejected;
                    } else if self
                        .pool_auth_hold_until
                        .map(|t| Instant::now() < t)
                        .unwrap_or(false)
                    {
                        // Keep last Accept/Reject visible during soft reconnect.
                    } else {
                        self.accepted = 0;
                        self.rejected = 0;
                    }
                    self.pool_phase = self.stratum_live.phase.clone();
                    if was_authed && !self.stratum_live.authorized {
                    }
                    if was_connected && !self.stratum_live.connected && self.mining {
                    }
                }
                NetMsg::Log { kind, text } => self.push_log(kind, text),
                NetMsg::FlashProgress(line) => {
                    self.absorb_flash_progress_line(&line);
                }
                NetMsg::FlashDone { result, reopen } => {
                    match result {
                        Ok(s) => {
                            self.last_ok = s.clone();
                            self.last_error.clear();
                            self.push_log(LogKind::Usb, s.clone());
                            if let Some(port) = reopen.filter(|p| !p.trim().is_empty()) {
                                // Keep spinner: reconnect, then verify via cmp ping/config.
                                self.begin_post_flash_verify(port);
                            } else {
                                self.clear_flash_overlay();
                                self.update_status = s;
                            }
                        }
                        Err(e) => {
                            self.clear_flash_overlay();
                            self.arm_flash_cooldown(8);
                            self.update_status = e.clone();
                            self.last_error = e.clone();
                            self.push_log(LogKind::Err, e);
                            self.push_log(
                                LogKind::Warn,
                                "Flash failed — brief COM cooldown (auto-link paused; Connect still works)"
                                    .into(),
                            );
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
                    let lat = match ev.latency_ms {
                        Some(ms) => format!(" · {ms} ms"),
                        None => String::new(),
                    };
                    let why = if ev.accepted {
                        String::new()
                    } else if ev.detail.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", trunc(&ev.detail, 64))
                    };
                    self.push_log(
                        if ev.accepted {
                            LogKind::Stratum
                        } else {
                            LogKind::Warn
                        },
                        format!(
                            "Share {} · job={} · nonce={} · id={}{lat}{why} · session A={} R={}",
                            if ev.accepted { "ACCEPTED" } else { "REJECTED" },
                            if ev.job_id.is_empty() {
                                "—"
                            } else {
                                ev.job_id.as_str()
                            },
                            if ev.nonce.is_empty() {
                                "—"
                            } else {
                                ev.nonce.as_str()
                            },
                            ev.id,
                            self.session_accepted,
                            self.session_rejected,
                        ),
                    );
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
                    self.app_update_busy_since = None;
                    self.app_update_cancel = None;
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
                            if e.to_ascii_lowercase().contains("cancelled") {
                                self.update_status = e.clone();
                                self.push_log(LogKind::Warn, e);
                            } else if user_initiated {
                                self.update_status = e.clone();
                                self.last_error = e.clone();
                                self.push_log(LogKind::Err, e);
                            } else {
                                self.push_log(LogKind::Warn, format!("App update check: {e}"));
                            }
                        }
                    }
                }
                NetMsg::AppUpdateProgress(line) => {
                    if self.app_update_busy {
                        self.update_status = trunc(&line, 140);
                    }
                }
                NetMsg::ApiFeedResult(outcome) => {
                    self.apply_api_pull(outcome);
                }
                NetMsg::WorkersFound(found) => {
                    self.worker_scan_busy = false;
                    let mut auto_usb: Vec<String> = Vec::new();
                    let mut auto_wifi: Vec<String> = Vec::new();
                    let mut usb_macs: Vec<String> = Vec::new();
                    for w in found {
                        // USB: link every distinct COM even when eFuse MACs collide.
                        // Wi‑Fi: skip when that MAC is already on USB (same board).
                        let skip = self.worker_already_linked(&w.endpoint)
                            || (matches!(w.kind, WorkerKind::Wifi)
                                && self.worker_mac_already_linked(&w.mac));
                        if !skip {
                            match w.kind {
                                WorkerKind::Usb => {
                                    // Probe misses are listed for visibility only — do not auto-link.
                                    let present_only = w
                                        .detail
                                        .to_ascii_lowercase()
                                        .contains("no cmp");
                                    if !present_only {
                                        if mac_is_stable(&w.mac) {
                                            usb_macs.push(normalize_mac(&w.mac));
                                        }
                                        auto_usb.push(w.endpoint.clone());
                                    }
                                }
                                WorkerKind::Wifi => {
                                    // SoftAP setup is not auto-linked — PC must join Njordr first.
                                    // Stale beacons also skip (freshness checked after merge).
                                    if !wifi_is_softap_setup(&w) {
                                        auto_wifi.push(w.endpoint.clone());
                                    }
                                }
                                WorkerKind::Lan => {}
                            }
                        }
                        self.merge_discovered(w);
                    }
                    // Fold fresh LAN STA/apsta beacons into auto-link — never SoftAP setup,
                    // never stale rows, never same MAC as a USB candidate about to link.
                    for w in self.discovered_workers.clone() {
                        if w.kind != WorkerKind::Wifi {
                            continue;
                        }
                        if wifi_is_softap_setup(&w) || !wifi_beacon_fresh(&w, 15_000) {
                            continue;
                        }
                        if self.worker_already_linked(&w.endpoint)
                            || self.worker_mac_already_linked(&w.mac)
                        {
                            continue;
                        }
                        if mac_is_stable(&w.mac)
                            && usb_macs
                                .iter()
                                .any(|m| m == &normalize_mac(&w.mac))
                        {
                            continue;
                        }
                        if !auto_wifi.iter().any(|e| e == &w.endpoint) {
                            auto_wifi.push(w.endpoint.clone());
                        }
                    }
                    // Drop stale SoftAP noise from the list (keep fresh + USB + linked).
                    self.discovered_workers.retain(|w| {
                        if w.kind != WorkerKind::Wifi {
                            return true;
                        }
                        if w.detail.to_ascii_lowercase().contains("linked") {
                            return true;
                        }
                        wifi_beacon_fresh(w, 45_000)
                    });
                    // Always surface already-linked boards in the found list too.
                    for live in self.connected_workers.clone() {
                        let kind = if live.endpoint.contains(':')
                            && !live.endpoint.to_ascii_uppercase().starts_with("COM")
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
                    // USB first — settle after Find-workers probe closed the COM.
                    if !auto_usb.is_empty() && !self.usb_connect_pending {
                        for endpoint in auto_usb {
                            if self.flash_busy() || self.flash_cooldown_active() {
                                self.push_log(
                                    LogKind::Warn,
                                    format!(
                                        "Skip auto-link {endpoint} — Update board / flash cooldown active"
                                    ),
                                );
                                continue;
                            }
                            self.queue_connect_usb(endpoint, "Auto-link USB");
                        }
                    } else if !auto_usb.is_empty() && self.usb_connect_pending {
                        self.push_log(
                            LogKind::Warn,
                            "Skip USB auto-link — another Link/Open is already in progress".into(),
                        );
                    }
                    // Prefer USB for multi-board farms — only auto-link fresh STA Wi‑Fi
                    // when that MAC is not USB-linked (SoftAP never auto).
                    for endpoint in auto_wifi {
                        if let Some(w) = self
                            .discovered_workers
                            .iter()
                            .find(|d| d.endpoint == endpoint)
                        {
                            if wifi_is_softap_setup(w) || !wifi_beacon_fresh(w, 15_000) {
                                continue;
                            }
                            if self.worker_mac_already_linked(&w.mac) {
                                self.push_log(
                                    LogKind::Usb,
                                    format!(
                                        "Skip Wi‑Fi {endpoint} — same board as USB root (SoftAP is this device)"
                                    ),
                                );
                                continue;
                            }
                            if mac_is_stable(&w.mac)
                                && usb_macs.iter().any(|m| m == &normalize_mac(&w.mac))
                            {
                                self.push_log(
                                    LogKind::Usb,
                                    format!(
                                        "Skip Wi‑Fi {endpoint} — USB auto-link owns MAC {}",
                                        w.mac
                                    ),
                                );
                                continue;
                            }
                        }
                        if self.flash_busy() || self.flash_cooldown_active() {
                            continue;
                        }
                        self.queue_connect_wifi(endpoint, "Auto-link Wi‑Fi STA");
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
                    // Always reflect linked boards — PollStatus is separately gated
                    // while update_busy so we do not thrash the flash COM.
                    self.usb_open = !self.connected_workers.is_empty();
                    if self.usb_open {
                        self.usb_connect_pending = false;
                    }
                    if !self.usb_open {
                        if was_open && !self.update_busy {
                            self.mining = false;
                            self.session_started = None;
                        }
                        if !self.update_busy {
                            self.clear_hash_display();
                        }
                        // Worker gone — refresh COM list so the dead port disappears.
                        if was_open {
                            self.refresh_com_ports(false, false);
                        }
                    } else {
                        // After a new board links, point the combo at the next free USB.
                        if self.connected_workers.len() > prev_n {
                            let _ = self.select_next_unlinked_usb();
                        }
                        // Fleet shrank (unplug) — refresh ports and fix selection.
                        if self.connected_workers.len() < prev_n {
                            self.refresh_com_ports(false, false);
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

        if self.usb_open
            && !self.update_busy
            && self.last_poll.elapsed() > Duration::from_millis(1_100)
        {
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
                self.last_port_refresh = Instant::now();
            }
        } else if !self.update_busy
            && !self.flash_busy()
            && self.last_port_refresh.elapsed() > Duration::from_secs(3)
        {
            // Keep picking up boards plugged into other PC USB ports without Refresh.
            self.refresh_com_ports(false, false);
            self.last_port_refresh = Instant::now();
        }
        // LAN peer discovery / advertise local CYD USB fleet.
        for peer in self.lan.poll_peers() {
            self.merge_discovered(peer);
        }
        for board in self.board_wifi.poll_boards() {
            self.merge_discovered(board);
        }
        self.maybe_route_softap_wifi_setup();
        self.tick_wifi_setup_wait();
        if matches!(self.wifi_setup_phase, WifiSetupPhase::WaitingSta { .. }) {
            ctx.request_repaint_after(Duration::from_millis(400));
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
        // App update watchdog — downloads used to retry mirrors for many minutes.
        if self.app_update_busy {
            let overdue = self
                .app_update_busy_since
                .map(|since| since.elapsed() > Duration::from_secs(180))
                .unwrap_or(false);
            if overdue {
                self.cancel_app_update();
                let msg = "Companion update timed out after 3 minutes — click Update app to retry."
                    .to_string();
                self.update_status = msg.clone();
                self.last_error = msg.clone();
                self.push_log(LogKind::Err, msg);
            }
        }
        // UI watchdog: fail fast if flash never leaves the 2% "starting" band.
        if self.update_busy {
            let flash_cap = Duration::from_secs(400);
            let starting_stall = self
                .update_busy_since
                .map(|since| {
                    since.elapsed() > Duration::from_secs(75)
                        && self.flash_progress < 0.10
                        && self.post_flash_verify.is_none()
                        && !self
                            .flash_need_boot
                            .as_ref()
                            .map(|n| n.load(Ordering::SeqCst))
                            .unwrap_or(false)
                })
                .unwrap_or(false);
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
            } else if starting_stall {
                if let Some(c) = &self.flash_cancel {
                    c.store(true, Ordering::SeqCst);
                }
                self.clear_flash_overlay();
                let msg = "Flash stuck before chip connect (~2%) — close other COM apps, \
use a short data USB cable, Keep Firmware on this device (OneDrive), then Push again \
or Flash (BOOT) with BOOT held + Ready."
                    .to_string();
                self.update_status = msg.clone();
                self.last_error = msg.clone();
                self.push_log(LogKind::Err, msg);
                self.arm_flash_cooldown(8);
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
                        format!("Verifying firmware on {port} (already open)…")
                    } else {
                        format!("Already open on {port}")
                    };
                    self.push_log(LogKind::Usb, self.update_status.clone());
                } else {
                    self.update_status = if verifying {
                        format!("Verifying firmware on {port}…")
                    } else {
                        format!("Opening {port} after update…")
                    };
                    self.push_log(
                        LogKind::Usb,
                        if verifying {
                            format!("Post-update verify: open {port} (no disconnect)")
                        } else {
                            format!("Post-update: open {port}")
                        },
                    );
                    if Self::is_wifi_ota_endpoint(&port) {
                        let _ = self.cmd_tx.send(NetCmd::ConnectWifi(port));
                    } else {
                        let _ = self.cmd_tx.send(NetCmd::OpenUsb { name: port });
                    }
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }

        if let Some((when, port)) = self.pending_post_flash_reboot.clone() {
            if Instant::now() >= when {
                let linked = self
                    .connected_workers
                    .iter()
                    .any(|w| port_names_match(&w.endpoint, &port))
                    || (self.usb_open && port_names_match(&self.com_port, &port));
                if linked {
                    self.pending_post_flash_reboot = None;
                    let _ = self.cmd_tx.send(NetCmd::RebootBoard {
                        endpoint: port.clone(),
                    });
                    self.push_log(
                        LogKind::Usb,
                        format!("Flash OK — reset command sent to {port}"),
                    );
                } else if Instant::now() > when + Duration::from_secs(30) {
                    self.pending_post_flash_reboot = None;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(200));
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
                            RichText::new("USB SHA-256 Bitcoin miner · boards mine to the pool")
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
                                        let serial: Vec<PortChoice> = self
                                            .ports
                                            .iter()
                                            .filter(|p| is_usb_serial_port(&p.name))
                                            .cloned()
                                            .collect();
                                        if serial.is_empty() {
                                            ui.label(
                                                RichText::new(
                                                    "No COM ports from Windows yet — Refresh.",
                                                )
                                                .color(C_WARN)
                                                .size(12.0),
                                            );
                                        }
                                        for p in serial {
                                            let junk = port_choice_is_system_junk(&p);
                                            if junk {
                                                ui.add_enabled(
                                                    false,
                                                    egui::Button::new(format!(
                                                        "{} · skip",
                                                        p.label
                                                    )),
                                                );
                                            } else {
                                                ui.selectable_value(
                                                    &mut self.com_port,
                                                    p.name.clone(),
                                                    &p.label,
                                                );
                                            }
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
                                    "Open Settings → Flash for Push (USB), Push (Wi‑Fi), or Flash with BOOT (blank / full rewrite).",
                                )
                                .color(C_MUTED)
                                .size(13.0),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if soft_button(ui, "Open Flash", 130.0).clicked() {
                                    self.go_flash_tab();
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
                                "bc1… / 1… / 3… or address.workername",
                            );
                            let resolved = stratum_worker_for_companion(self.edit_worker.trim());
                            if !resolved.is_empty() {
                                ui.label(
                                    RichText::new(format!("Pool username → {resolved}"))
                                        .color(C_MUTED)
                                        .size(11.0),
                                );
                            }
                            labeled_edit(
                                ui,
                                "Stratum",
                                &mut self.edit_stratum,
                                "stratum+tcp://…",
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(
                                    "Best path on this board (not a different alg): Bench / first-job auto-tune locks HW, HW+, or HW/SW · 240 MHz · more CYDs for more rate. Algorithm stays SHA-256d.",
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

        // Bench notifier — always visible while retuning so "Benching…" never looks stuck.
        if self.bench_busy {
            ctx.request_repaint_after(Duration::from_millis(200));
            let elapsed = self
                .bench_busy_since
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);
            let status = if self.bench_status.is_empty() {
                "Climbing Mid→HW→HW/SW…".to_string()
            } else {
                self.bench_status.clone()
            };
            egui::Window::new("Benching boards")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 48.0])
                .show(ctx, |ui| {
                    ui.set_min_width(420.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        ui.add(egui::Spinner::new().size(28.0).color(C_LIME));
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Auto-tune in progress")
                                .color(C_LIME)
                                .strong()
                                .size(16.0),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(&status)
                                .color(C_TEXT)
                                .font(mono_ui_font(12.0)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!(
                                "Elapsed {elapsed}s · live CMPBENCHPROG updates · Cancel if hung"
                            ))
                            .color(C_MUTED)
                            .size(11.0),
                        );
                        ui.add_space(10.0);
                        if soft_button(ui, "Cancel bench", 140.0).clicked() {
                            self.cancel_bench();
                        }
                        ui.add_space(6.0);
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
            // Fixed size — phase/status lines change every tick; auto-fit made the
            // window pulse bigger/smaller as espflash codes scrolled by.
            const FLASH_PANEL_W: f32 = 540.0;
            const FLASH_PANEL_H: f32 = 360.0;
            egui::Window::new("Updating board")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_size([FLASH_PANEL_W, FLASH_PANEL_H])
                .min_size([FLASH_PANEL_W, FLASH_PANEL_H])
                .max_size([FLASH_PANEL_W, FLASH_PANEL_H])
                .show(ctx, |ui| {
                    ui.set_min_size(egui::vec2(FLASH_PANEL_W - 24.0, FLASH_PANEL_H - 40.0));
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.add(egui::Spinner::new().size(44.0).color(C_LIME));
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(if self.post_flash_verify.is_some() {
                                "Verifying board firmware"
                            } else if self.pending_post_flash_reconnect.is_some() {
                                "Flash complete — reconnecting"
                            } else if awaiting_boot {
                                "Download mode — click Ready"
                            } else if self.flash_phase.to_ascii_lowercase().contains("eras") {
                                "Erasing board flash"
                            } else if self.flash_phase.to_ascii_lowercase().contains("push") {
                                "Pushing firmware update"
                            } else {
                                "Flashing board firmware"
                            })
                            .color(C_LIME)
                            .font(display_font(22.0)),
                        );
                        ui.add_space(10.0);
                        let pct = (self.flash_progress.clamp(0.0, 1.0) * 100.0).round() as u32;
                        let phase = if self.flash_phase.is_empty() {
                            "Working…".to_string()
                        } else {
                            self.flash_phase.clone()
                        };
                        // Fixed-height phase row so % text length changes don't reflow.
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width().min(500.0), 22.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new(format!("{phase} · {pct}%"))
                                        .color(C_TEXT)
                                        .font(mono_ui_font(13.0)),
                                );
                            },
                        );
                        ui.add_space(8.0);
                        ui.add(
                            egui::ProgressBar::new(self.flash_progress.clamp(0.0, 1.0))
                                .desired_width(480.0)
                                .animate(true)
                                .fill(C_LIME),
                        );
                        ui.add_space(10.0);
                        // Fixed 2-line status well — long espflash lines used to grow the box.
                        let status = if self.update_status.is_empty() {
                            "Starting…".to_string()
                        } else {
                            self.update_status.clone()
                        };
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width().min(500.0), 40.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new(status)
                                        .color(C_MUTED)
                                        .font(mono_ui_font(11.0)),
                                );
                            },
                        );
                        ui.add_space(8.0);
                        // Always reserve BOOT instruction height so Ready ↔ Writing doesn't jump.
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width().min(500.0), 56.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                if awaiting_boot {
                                    ui.label(
                                        RichText::new(
                                            "1) Hold BOOT · 2) Tap RESET · 3) Keep BOOT held · 4) Ready · keep BOOT until Writing %",
                                        )
                                        .color(C_TEXT)
                                        .size(13.0),
                                    );
                                    ui.add_space(8.0);
                                    if cta_button(ui, "Ready", true, 160.0).clicked() {
                                        if let Some(r) = &self.flash_boot_ready {
                                            r.store(true, Ordering::SeqCst);
                                        }
                                        self.flash_phase = "Writing after Ready".into();
                                        self.update_status =
                                            "Ready — keep BOOT held until Writing % appears…".into();
                                    }
                                } else if self.flash_phase.to_ascii_lowercase().contains("push")
                                    || self.update_status.to_ascii_lowercase().contains("usb ota")
                                    || self.update_status.to_ascii_lowercase().contains("auto-reset")
                                    || self
                                        .update_status
                                        .to_ascii_lowercase()
                                        .contains("push update")
                                {
                                    ui.label(
                                        RichText::new(
                                            "Push — no buttons. Hands free unless Ready appears.",
                                        )
                                        .color(C_DIM)
                                        .size(12.0),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("Keep USB connected")
                                            .color(C_DIM)
                                            .size(12.0),
                                    );
                                }
                            },
                        );
                        ui.add_space(8.0);
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
                        ui.add_space(6.0);
                    });
                });
        }

        // Companion self-update overlay — Cancel must always be available.
        if self.app_update_busy && !self.update_busy {
            egui::Window::new("Updating Companion")
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
                            RichText::new("Updating Companion app")
                                .color(C_LIME)
                                .font(display_font(22.0)),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(if self.update_status.is_empty() {
                                "Downloading…"
                            } else {
                                &self.update_status
                            })
                            .color(C_TEXT)
                            .font(mono_ui_font(12.0)),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            RichText::new("Stops automatically on timeout · or Cancel now")
                                .color(C_DIM)
                                .size(12.0),
                        );
                        ui.add_space(8.0);
                        if soft_button(ui, "Cancel", 120.0).clicked() {
                            self.cancel_app_update();
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
                            Tab::Mine => self.ui_mine(ui),
                            Tab::Settings => self.ui_settings_shell(ui),
                        }
                        ui.add_space(28.0);
                    });
            });

        // Keep animation continuous (~60 fps). 40 ms made looping motion feel stepped.
        ctx.request_repaint_after(Duration::from_millis(16));
        self.refresh_monitor_lan_ip();
        self.publish_phone_monitor();
        self.tick_bench_notifier();
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
    } else {
        utf8_safe::trunc(s, n)
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
        .min_size(Vec2::new(42.0, 30.0)),
    )
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

/// Drain stratum chatter into the event log. Share accept/reject lines are skipped —
/// those are logged once from `NetMsg::Share` with session counters.
fn drain_stratum_log(tx: &Sender<NetMsg>, client: &mut StratumClient) {
    for line in client.take_recent() {
        let low = line.to_ascii_lowercase();
        if low.contains("← share accepted")
            || low.contains("← share rejected")
            || low.contains("<- share accepted")
            || low.contains("<- share rejected")
        {
            continue;
        }
        log_msg(tx, LogKind::Stratum, line);
    }
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
        submits: client.submits,
        pending: client.pending_share_count(),
        last_job: client.job_id().to_string(),
        last_rx: client.last_rx.clone(),
        last_tx: client.last_tx.clone(),
        last_error: client.last_error.clone(),
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
                        utf8_safe::keep_last(buf, 8192);
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
        /// Board owns its own stratum session — Companion monitors H/s only.
        mine_indep: bool,
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
                mine_indep: b.mine_indep,
            })
            .collect()
    }

    fn publish_live(tx: &Sender<NetMsg>, boards: &[UsbBoard]) {
        let _ = tx.send(NetMsg::WorkersLive(live_from(boards)));
    }

    fn usb_err_looks_unplugged(err: &str) -> bool {
        let e = err.to_ascii_lowercase();
        e.contains("write:")
            || e.contains("access is denied")
            || e.contains("no such file")
            || e.contains("broken pipe")
            || e.contains("not connected")
            || e.contains("connection reset")
            || e.contains("clearcomm")
            || e.contains("device")
            || e.contains("os error 5")
            || e.contains("os error 6")
            || e.contains("os error 21")
            || e.contains("os error 22")
            || e.contains("os error 1167")
            || e.contains("the device does not recognize")
            || e.contains("the semaphore timeout")
    }

    /// Drop USB boards whose COM vanished from the OS (unplug). Updates UI immediately.
    fn prune_unplugged_boards(
        boards: &mut Vec<UsbBoard>,
        msg_tx: &Sender<NetMsg>,
        mining: &mut bool,
        stratum: &mut Option<StratumClient>,
        reconnect_at: &mut Option<Instant>,
        last_fleet_status: &mut StatusJson,
        flash_hold: &Option<(String, Arc<AtomicBool>)>,
    ) -> bool {
        if boards.is_empty() {
            return false;
        }
        let listed = list_serial_ports();
        let mut removed: Vec<String> = Vec::new();
        boards.retain(|b| {
            if matches!(b.port, BoardIo::Tcp(_)) {
                return true;
            }
            if let Some((p, flag)) = flash_hold {
                if flag.load(Ordering::SeqCst)
                    && (port_names_match(p, &b.name) || *p == b.name)
                {
                    return true;
                }
            }
            let present = listed
                .iter()
                .any(|p| port_names_match(&p.name, &b.name) || p.name == b.name);
            if !present {
                removed.push(b.name.clone());
            }
            present
        });
        if removed.is_empty() {
            return false;
        }
        for name in &removed {
            log_msg(
                msg_tx,
                LogKind::Warn,
                format!("USB {name} unplugged — removed from fleet"),
            );
        }
        let _ = msg_tx.send(NetMsg::Ports(listed));
        publish_live(msg_tx, boards);
        if boards.is_empty() {
            *mining = false;
            *reconnect_at = None;
            *last_fleet_status = StatusJson::default();
            if let Some(mut s) = stratum.take() {
                s.disconnect();
            }
            let _ = msg_tx.send(NetMsg::Status(Ok(StatusJson::default())));
            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                "All workers unplugged ({})",
                removed.join(", ")
            ))));
        } else {
            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                "Unplugged {} · {} board(s) left",
                removed.join(", "),
                boards.len()
            ))));
        }
        true
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
                mine_indep: false,
                download_mode: false,
            },
            true,
        ))
    }

    /// Push/clear STA Wi‑Fi on a linked board, or one-shot SoftAP TCP / USB session.
    fn board_wifi_cmd(
        boards: &mut [UsbBoard],
        endpoint: &str,
        cmd: &str,
    ) -> Result<String, String> {
        if let Some(b) = boards
            .iter_mut()
            .find(|b| port_names_match(&b.name, endpoint) || b.name == endpoint)
        {
            return usb_cmd(&mut b.port, &mut b.rx, cmd);
        }
        let ep = endpoint.trim();
        // One-shot USB open when Find-workers listed a COM that is not linked yet.
        if is_usb_serial_port(ep) {
            let mut port = BoardIo::Serial(open_usb_serial_timed(
                ep,
                115_200,
                Duration::from_millis(8),
                Duration::from_secs(3),
            )?);
            let mut rx = String::new();
            match usb_cmd(&mut port, &mut rx, "cmp ping") {
                Ok(_) => {}
                Err(e) => {
                    return Err(format!(
                        "USB {ep} no pong: {e} — Link the board on Mine first, then Push Wi‑Fi"
                    ));
                }
            }
            return usb_cmd(&mut port, &mut rx, cmd);
        }
        let looks_tcp = ep.contains(':')
            && !ep.to_ascii_uppercase().starts_with("COM")
            ;
        if !looks_tcp {
            return Err(format!(
                "Board {endpoint} not linked — join open SoftAP Njordr-XXXX or Link USB first"
            ));
        }
        let stream = open_wifi_tcp(ep)?;
        let mut rx = String::new();
        let mut port = BoardIo::Tcp(stream);
        match usb_cmd(&mut port, &mut rx, "cmp ping") {
            Ok(_) => {}
            Err(e) => return Err(format!("SoftAP {ep} no pong: {e}")),
        }
        usb_cmd(&mut port, &mut rx, cmd)
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
                            mine_indep: false,
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
                mine_indep: false,
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
        match usb_cmd(&mut board.port, &mut board.rx, "cmp config") {
            Ok(line) => {
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
                    log_msg(
                        msg_tx,
                        LogKind::Warn,
                        format!(
                            "{} config parse failed: {}",
                            board.name,
                            trunc(&line, 96)
                        ),
                    );
                    let _ = msg_tx.send(NetMsg::Config(parse_cmp_config(&line)));
                }
            }
            Err(e) => {
                log_msg(
                    msg_tx,
                    LogKind::Warn,
                    format!("{} config probe failed: {e}", board.name),
                );
            }
        }
        match usb_cmd(&mut board.port, &mut board.rx, "cmp status") {
            Ok(line) => {
                if let Ok(st) = parse_cmp_status(&line) {
                    board.hashrate_hs = st.hashrate_hs;
                    board.hashes = st.hashes;
                    board.mining = st.mining;
                    if !st.mac.is_empty() {
                        board.mac = normalize_mac(&st.mac);
                    }
                    let _ = msg_tx.send(NetMsg::Status(Ok(st)));
                } else {
                    log_msg(
                        msg_tx,
                        LogKind::Warn,
                        format!(
                            "{} status parse failed: {}",
                            board.name,
                            trunc(&line, 96)
                        ),
                    );
                }
            }
            Err(e) => {
                log_msg(
                    msg_tx,
                    LogKind::Warn,
                    format!("{} status probe failed: {e}", board.name),
                );
            }
        }
        if !board.mac.is_empty() {
            let id = mac_worker_id(&board.mac);
            log_msg(
                msg_tx,
                LogKind::Usb,
                format!(
                    "Board {} identity {} ({}) · fw={}",
                    board.name,
                    board.mac,
                    if id.is_empty() { "port" } else { &id },
                    if board.fw.is_empty() {
                        "?"
                    } else {
                        board.fw.as_str()
                    }
                ),
            );
        }
    }

    /// Arm mining job on a newly linked board.
    /// Always uses warmup — never reuse another board's recent_jobs en2 (duplicate submits).
    fn arm_mining_if_needed(
        board: &mut UsbBoard,
        msg_tx: &Sender<NetMsg>,
        resume_mining: bool,
        _resume_job: Option<&WorkJob>,
    ) {
        if board.download_mode || !resume_mining {
            return;
        }
        if board_endpoint_is_softap_setup(&board.name) {
            log_msg(
                msg_tx,
                LogKind::Warn,
                format!(
                    "Skip mine arm on SoftAP setup endpoint {} — rejoin home Wi‑Fi for pool uplink",
                    board.name
                ),
            );
            return;
        }
        let mut legacy = board.legacy_job;
        let _ = usb_cmd(
            &mut board.port,
            &mut board.rx,
            "cmp stats accepted=0&rejected=0",
        );
        // Unique-en2 pool jobs come from the next take_job_batch — never share en2.
        let warmup = warmup_job();
        let _ = usb_push_job(
            &mut board.port,
            &mut board.rx,
            &warmup,
            &mut legacy,
            msg_tx,
        );
        board.legacy_job = legacy;
        board.mining = true;
        log_msg(
            msg_tx,
            LogKind::Usb,
            format!(
                "Board {} hashing (warmup — waiting unique pool en2)…",
                board.name
            ),
        );
    }

    /// SoftAP Board Setup portal endpoint (`10.88.88.x:19284`) — no pool uplink path.
    fn board_endpoint_is_softap_setup(endpoint: &str) -> bool {
        let ep = endpoint.trim();
        ep.starts_with("10.88.88.") || ep.contains("@10.88.88.")
    }

    /// Keep the pool socket drained during long USB work so we do not
    /// bounce AUTHORIZED → SUBSCRIBE from a starved TCP session.
    fn pump_stratum_keepalive(
        stratum: &mut Option<StratumClient>,
        msg_tx: &Sender<NetMsg>,
    ) -> Option<String> {
        let client = stratum.as_mut()?;
        let mut err = None;
        for _ in 0..8 {
            match client.poll() {
                Ok(()) => {}
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        drain_stratum_log(msg_tx, client);
        if err.is_none() {
            push_stratum_live(msg_tx, client);
        }
        err
    }

    /// Stratum always wins: handshake, pending jobs, and reconnect windows defer USB chrome.
    fn pool_has_presidency(
        stratum: &Option<StratumClient>,
        reconnect_at: Option<Instant>,
    ) -> bool {
        if reconnect_at.is_some() {
            return true;
        }
        match stratum {
            Some(s) => {
                (s.stream_connected() && !s.authorized() && !s.auth_give_up) || s.has_pending_job()
            }
            None => false,
        }
    }

    let mut boards: Vec<UsbBoard> = Vec::new();
    let mut stratum: Option<StratumClient> = None;
    let mut last_ports_enum = Instant::now() - Duration::from_secs(30);
    let mut recent_jobs: VecDeque<WorkJob> = VecDeque::new();
    // Board shares held while the pool socket is down (submit on reconnect).
    let mut held_board_shares: VecDeque<(String, String, String, String)> = VecDeque::new();
    let mut mining = false;
    let mut last_stats_push = Instant::now() - Duration::from_secs(10);
    let mut last_stratum_ui = Instant::now() - Duration::from_secs(10);
    let mut last_unplug_check = Instant::now() - Duration::from_secs(1);
    let mut last_fleet_status = StatusJson::default();
    let mut mine_endpoint = String::new();
    let mut mine_worker_name = String::new();
    let mut mine_password = String::new();
    let mut reconnect_at: Option<Instant> = None;
    let mut reconnect_backoff = Duration::from_secs(1);
    /// Flash thread owns this COM until the AtomicBool clears — block Open/Scan/Link.
    let mut flash_hold: Option<(String, Arc<AtomicBool>)> = None;

    let flash_port_held = |hold: &Option<(String, Arc<AtomicBool>)>, name: &str| -> bool {
        match hold {
            Some((p, flag)) if flag.load(Ordering::SeqCst) => {
                port_names_match(p, name) || p == name
            }
            _ => false,
        }
    };
    let flash_any_held = |hold: &Option<(String, Arc<AtomicBool>)>| -> bool {
        match hold {
            Some((_, flag)) => flag.load(Ordering::SeqCst),
            None => false,
        }
    };

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
                    if flash_any_held(&flash_hold) {
                        log_msg(
                            &msg_tx,
                            LogKind::Warn,
                            "Skip USB scan — Update board owns the COM (avoids terminal thrash)",
                        );
                        let _ = msg_tx.send(NetMsg::WorkersFound(Vec::new()));
                        continue;
                    }
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
                    if flash_port_held(&flash_hold, &name) {
                        let _ = msg_tx.send(NetMsg::Action(Err(format!(
                            "Wait — Update board still owns {name}"
                        ))));
                        continue;
                    }
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
                            if mac_is_stable(&board.mac) {
                                let collision = boards.iter().any(|b| {
                                    mac_is_stable(&b.mac)
                                        && normalize_mac(&b.mac) == normalize_mac(&board.mac)
                                        && !port_names_match(&b.name, &name)
                                });
                                if collision {
                                    log_msg(
                                        &msg_tx,
                                        LogKind::Warn,
                                        format!(
                                            "Board {name} shares eFuse MAC {} with another linked COM — tracked by COM",
                                            board.mac
                                        ),
                                    );
                                }
                            }
                            arm_mining_if_needed(
                                &mut board,
                                &msg_tx,
                                mining,
                                None,
                            );
                            let mac = if board.mac.is_empty() {
                                "—".into()
                            } else {
                                board.mac.clone()
                            };
                            let fw = if board.fw.is_empty() {
                                "?".into()
                            } else {
                                board.fw.clone()
                            };
                            boards.push(board);
                            publish_live(&msg_tx, &boards);
                            let _ = msg_tx.send(NetMsg::Action(Ok(if dl {
                                format!(
                                    "USB open {name} (download mode / BOOT) · mac={mac} · fw={fw} · flash with Update board · {} board(s)",
                                    boards.len()
                                )
                            } else {
                                format!(
                                    "USB open {name}{} · mac={mac} · fw={fw} · {} board(s)",
                                    if saw { " (pong)" } else { "" },
                                    boards.len()
                                )
                            })));
                        }
                        Err(e) => {
                            publish_live(&msg_tx, &boards);
                            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                "USB open {name} failed: {e}"
                            ))));
                        }
                    }
                }
                NetCmd::ConnectWorker(name) => {
                    // Only block the flash target COM — other boards stay linkable.
                    if flash_port_held(&flash_hold, &name) {
                        let _ = msg_tx.send(NetMsg::Action(Err(format!(
                            "Wait — Update board still owns {name}"
                        ))));
                        continue;
                    }
                    if boards.iter().any(|b| port_names_match(&b.name, &name)) {
                        let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                            "Worker {name} already linked"
                        ))));
                        publish_live(&msg_tx, &boards);
                        continue;
                    }
                    // Keep already-linked boards hashing. Stopping them for hub
                    // brownout avoidance zeros fleet H/s on every multi-COM link.
                    let resume_mine = mining;
                    log_msg(&msg_tx, LogKind::Usb, format!("Connecting worker {name}"));
                    // Find-workers probe just closed this COM — brief settle avoids
                    // Access Denied / no-pong races on CH340.
                    thread::sleep(Duration::from_millis(280));
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
                                    // Use registry-merged COM list — SetupAPI alone often
                                    // misses the 2nd CH340 and wrongly "replaces" a live board.
                                    let old_still_listed = list_serial_ports()
                                        .iter()
                                        .any(|p| port_names_match(&p.name, &old_name));
                                    if old_still_listed && !port_names_match(&old_name, &name) {
                                        log_msg(
                                            &msg_tx,
                                            LogKind::Warn,
                                            format!(
                                                "Keep {old_name} + {name} — same eFuse MAC {} (identity is COM, not MAC)",
                                                board.mac
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
                            arm_mining_if_needed(
                                &mut board,
                                &msg_tx,
                                resume_mine,
                                None,
                            );
                            let mac = if board.mac.is_empty() {
                                "—".into()
                            } else {
                                board.mac.clone()
                            };
                            let fw = if board.fw.is_empty() {
                                "?".into()
                            } else {
                                board.fw.clone()
                            };
                            boards.push(board);
                            publish_live(&msg_tx, &boards);
                            let _ = msg_tx.send(NetMsg::Action(Ok(if dl {
                                format!(
                                    "Worker linked {name} (download mode / BOOT) · mac={mac} · fw={fw} · Update board to flash · {} total",
                                    boards.len()
                                )
                            } else {
                                format!(
                                    "Worker linked {name}{} · mac={mac} · fw={fw} · {} total",
                                    if saw { " (pong)" } else { "" },
                                    boards.len()
                                )
                            })));
                        }
                        Err(e) => {
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
                            arm_mining_if_needed(
                                &mut board,
                                &msg_tx,
                                mining,
                                None,
                            );
                            let mac = if board.mac.is_empty() {
                                "—".into()
                            } else {
                                board.mac.clone()
                            };
                            let fw = if board.fw.is_empty() {
                                "?".into()
                            } else {
                                board.fw.clone()
                            };
                            boards.push(board);
                            publish_live(&msg_tx, &boards);
                            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                "Wi‑Fi worker linked {endpoint}{} · mac={mac} · fw={fw} · {} total",
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
                NetCmd::SetBoardWifi {
                    endpoint,
                    ssid,
                    pass,
                    enable,
                } => {
                    // Never log `pass` — credentials are written to board NVS only.
                    let cmd = if enable {
                        format!(
                            "cmp wifi ssid={}&pass={}&en=1",
                            urlenc(&ssid),
                            urlenc(&pass)
                        )
                    } else {
                        "cmp wifi clear".to_string()
                    };
                    log_msg(
                        &msg_tx,
                        LogKind::Usb,
                        format!(
                            "Board Wi‑Fi {} → {} (SSID “{}”, password not logged)",
                            if enable { "save" } else { "clear" },
                            endpoint,
                            if enable {
                                ssid.as_str()
                            } else {
                                "—"
                            }
                        ),
                    );
                    match board_wifi_cmd(&mut boards, &endpoint, &cmd) {
                        Ok(reply) => {
                            let low = reply.to_ascii_lowercase();
                            if low.contains("cmperr") {
                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                    "BOARD_WIFI_ERR|{endpoint}|{reply}"
                                ))));
                            } else if enable {
                                let ssid_l = ssid.to_ascii_lowercase();
                                let ack_has_ssid = low.contains(&format!("ssid={ssid_l}"))
                                    || reply.contains(&format!("ssid={ssid}"));
                                // USB: re-read cmp wifi so we know NVS actually holds the SSID.
                                // SoftAP TCP may drop after apply — trust ACK ssid= when present.
                                let verified = if is_usb_serial_port(&endpoint) {
                                    thread::sleep(Duration::from_millis(80));
                                    match board_wifi_cmd(&mut boards, &endpoint, "cmp wifi") {
                                        Ok(st) => {
                                            let st_l = st.to_ascii_lowercase();
                                            let ok = st_l.contains(&format!("ssid={ssid_l}"))
                                                || st.contains(&format!("ssid={ssid}"));
                                            if !ok {
                                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                                    "BOARD_WIFI_ERR|{endpoint}|saved ACK but board reports “{st}” (expected SSID “{ssid}”)"
                                                ))));
                                            }
                                            ok
                                        }
                                        Err(e) => {
                                            if ack_has_ssid {
                                                true
                                            } else {
                                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                                    "BOARD_WIFI_ERR|{endpoint}|save ACK “{reply}” but verify failed: {e}"
                                                ))));
                                                false
                                            }
                                        }
                                    }
                                } else {
                                    ack_has_ssid || low.contains("wifi saved")
                                };
                                if verified {
                                    let path = if is_usb_serial_port(&endpoint) {
                                        "USB"
                                    } else {
                                        "SoftAP/TCP"
                                    };
                                    let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                        "BOARD_WIFI_SAVED|{endpoint}|{reply}"
                                    ))));
                                    log_msg(
                                        &msg_tx,
                                        LogKind::Usb,
                                        format!(
                                            "Wi‑Fi credentials stored on {endpoint} · path={path} · SSID “{ssid}” · ACK {}",
                                            trunc(&reply, 96)
                                        ),
                                    );
                                }
                            } else {
                                let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                    "BOARD_WIFI_CLEARED|{endpoint}|{reply}"
                                ))));
                            }
                        }
                        Err(e) => {
                            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                "BOARD_WIFI_ERR|{endpoint}|{e}"
                            ))));
                        }
                    }
                }
                NetCmd::ClearBoardWifi { endpoint } => {
                    log_msg(
                        &msg_tx,
                        LogKind::Usb,
                        format!("Board Wi‑Fi clear → {endpoint}"),
                    );
                    match board_wifi_cmd(&mut boards, &endpoint, "cmp wifi clear") {
                        Ok(reply) => {
                            let low = reply.to_ascii_lowercase();
                            if low.contains("cmperr") {
                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                    "BOARD_WIFI_ERR|{endpoint}|{reply}"
                                ))));
                            } else {
                                let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                    "BOARD_WIFI_CLEARED|{endpoint}|{reply}"
                                ))));
                            }
                        }
                        Err(e) => {
                            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                "BOARD_WIFI_ERR|{endpoint}|{e}"
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
                    // SoftAP setup net has no internet — refuse Mine until PC rejoins home LAN.
                    if softap_setup_client_ipv4()
                        .map(|ip| ip.starts_with("10.88.88."))
                        .unwrap_or(false)
                    {
                        let _ = msg_tx.send(NetMsg::Action(Err(
                            "PC is on SoftAP (10.88.88.x) — rejoin home Wi‑Fi so the pool has uplink, then Mine"
                                .into(),
                        )));
                        continue;
                    }
                    // Drop SoftAP-only TCP links from the mining fleet (setup portal, no pool path).
                    let before = boards.len();
                    boards.retain(|b| {
                        if board_endpoint_is_softap_setup(&b.name) {
                            log_msg(
                                &msg_tx,
                                LogKind::Warn,
                                format!(
                                    "Skip SoftAP setup board {} for Mine — use Settings → Setup / home Wi‑Fi",
                                    b.name
                                ),
                            );
                            false
                        } else {
                            true
                        }
                    });
                    if boards.len() < before {
                        publish_live(&msg_tx, &boards);
                    }
                    if boards.is_empty() {
                        let _ = msg_tx.send(NetMsg::Action(Err(
                            "No mineable boards (SoftAP setup links only) — Connect USB/STA Wi‑Fi first"
                                .into(),
                        )));
                        continue;
                    }
                    mine_endpoint = endpoint.clone();
                    // Bare BTC address → address.companion so HMPool lists a named worker.
                    // Boards get the SAME username (not a MAC .cydXXXX) so the dashboard
                    // shows one worker whether Companion or the board submits.
                    let pool_worker = stratum_worker_for_companion(&worker);
                    // Already authorized on this pool — skip reconnect (keeps Accept/Reject).
                    // Duplicate StartMine was wiping counters every few seconds.
                    if mining
                        && stratum
                            .as_ref()
                            .map(|c| {
                                c.authorized()
                                    && stratum_endpoints_match(c.endpoint(), &endpoint)
                                    && c.worker().eq_ignore_ascii_case(&pool_worker)
                            })
                            .unwrap_or(false)
                    {
                        log_msg(
                            &msg_tx,
                            LogKind::Info,
                            format!(
                                "Start mining ignored — already authorized on {endpoint} as {pool_worker} (share counters kept)"
                            ),
                        );
                        continue;
                    }
                    mine_worker_name = pool_worker.clone();
                    mine_password = stratum_password(&password);
                    reconnect_backoff = Duration::from_secs(1);
                    reconnect_at = None;
                    // Connect the pool FIRST so authorize is not starved by USB
                    // USB warmup. Boards keep hashing once jobs arrive.
                    log_msg(
                        &msg_tx,
                        LogKind::Stratum,
                        format!(
                            "Connecting pool {endpoint} as {pool_worker} · {} USB/Wi‑Fi",
                            boards.len()
                        ),
                    );
                    let mut client = StratumClient::new(pool_worker.clone(), mine_password.clone());
                    let mut pool_ready = false;
                    match client.connect(&endpoint) {
                        Ok(()) => {
                            // Drain subscribe → authorize quickly (up to ~4s).
                            let deadline = Instant::now() + Duration::from_secs(4);
                            let mut handshake_err: Option<String> = None;
                            while Instant::now() < deadline
                                && !client.authorized()
                                && !client.auth_give_up
                            {
                                match client.poll() {
                                    Ok(()) => {}
                                    Err(e) => {
                                        handshake_err = Some(e);
                                        break;
                                    }
                                }
                                thread::sleep(Duration::from_millis(25));
                            }
                            drain_stratum_log(&msg_tx, &mut client);
                            push_stratum_live(&msg_tx, &client);
                            if let Some(e) = handshake_err {
                                if client.auth_give_up {
                                    let _ = msg_tx.send(NetMsg::Action(Err(e)));
                                    mining = false;
                                    reconnect_at = None;
                                } else {
                                    let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                        "Pool error (will retry): {e}"
                                    ))));
                                    mining = true;
                                    reconnect_at = Some(Instant::now() + reconnect_backoff);
                                }
                            } else if client.auth_give_up {
                                let why = if client.last_error.is_empty() {
                                    "authorize failed — check BTC address / worker name".into()
                                } else {
                                    client.last_error.clone()
                                };
                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                    "Pool authorize failed: {why}"
                                ))));
                                mining = false;
                                reconnect_at = None;
                            } else {
                                if client.authorized() {
                                    let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                        "Pool AUTHORIZED {endpoint} — arming {} board(s)",
                                        boards.len()
                                    ))));
                                } else {
                                    let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                        "Pool TCP up {endpoint} (waiting authorize) — arming boards"
                                    ))));
                                }
                                stratum = Some(client);
                                mining = true;
                                reconnect_backoff = Duration::from_secs(1);
                                pool_ready = true;
                                // Push pool credentials so STA boards mine independently
                                // (each board owns stratum). Companion keeps polling H/s.
                                for b in boards.iter_mut() {
                                    if b.download_mode || board_endpoint_is_softap_setup(&b.name) {
                                        continue;
                                    }
                                    let board_worker = stratum_worker_for_board(&worker, &b.mac);
                                    let cmd = format!(
                                        "cmp pool url={}&worker={}&pass={}&indep=1",
                                        urlenc(&endpoint),
                                        urlenc(&board_worker),
                                        urlenc(&mine_password)
                                    );
                                    match usb_cmd(&mut b.port, &mut b.rx, &cmd) {
                                        Ok(line) if line.to_ascii_lowercase().contains("cmpack") => {
                                            // Keep Companion job ownership until status shows
                                            // pool_phase=ok (STA + onboard stratum authorized).
                                            b.mine_indep = false;
                                            log_msg(
                                                &msg_tx,
                                                LogKind::Usb,
                                                format!(
                                                    "Board {} ← pool creds {endpoint} as {board_worker} (same pool user as Companion; keeps feeding jobs until board pool authorizes)",
                                                    b.name
                                                ),
                                            );
                                        }
                                        Ok(line) => {
                                            log_msg(
                                                &msg_tx,
                                                LogKind::Warn,
                                                format!(
                                                    "Board {} pool push: {}",
                                                    b.name,
                                                    trunc(&line, 96)
                                                ),
                                            );
                                        }
                                        Err(e) => {
                                            log_msg(
                                                &msg_tx,
                                                LogKind::Warn,
                                                format!("Board {} pool push failed: {e}", b.name),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            mining = true;
                            reconnect_at = Some(Instant::now() + reconnect_backoff);
                            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                "Pool connect failed (boards will retry): {e}"
                            ))));
                        }
                    }
                    if !pool_ready && !mining {
                        // Authorize failed — don't arm boards on a dead pool session.
                        continue;
                    }
                    // Prefer real pool work over warmup. After authorize, wait briefly for
                    // set_difficulty + notify so Msgs↑=3 / Jobs≥1 is backed by board headers
                    // (warmup shares are discarded and look like a dead pool session).
                    let mut armed_pool = 0usize;
                    if let Some(client) = stratum.as_mut() {
                        if client.authorized() && !client.has_pending_job() {
                            let wait = Instant::now() + Duration::from_millis(1800);
                            while Instant::now() < wait && !client.has_pending_job() {
                                if let Err(e) = client.poll() {
                                    log_msg(
                                        &msg_tx,
                                        LogKind::Warn,
                                        format!("Pool {endpoint} poll: {e}"),
                                    );
                                    break;
                                }
                                thread::sleep(Duration::from_millis(25));
                            }
                        }
                        drain_stratum_log(&msg_tx, client);
                        push_stratum_live(&msg_tx, client);
                        let fleet_n = boards
                            .iter()
                            .filter(|b| !b.mine_indep)
                            .count()
                            
                            .max(1);
                        let jobs = client.take_job_batch(fleet_n);
                        if !jobs.is_empty() {
                            let mut remaining: VecDeque<WorkJob> = jobs.into();
                            for b in boards.iter_mut() {
                                if b.mine_indep {
                                    continue;
                                }
                                let Some(job) = remaining.pop_front() else { break };
                                let mut legacy = b.legacy_job;
                                let _ = usb_cmd_ex(
                                    &mut b.port,
                                    &mut b.rx,
                                    "cmp stats accepted=0&rejected=0",
                                    &mut || {
                                        let _ = client.poll();
                                    },
                                    None,
                                );
                                let push_res = {
                                    let mut pump = || {
                                        let _ = client.poll();
                                    };
                                    usb_push_job_ex(
                                        &mut b.port,
                                        &mut b.rx,
                                        &job,
                                        &mut legacy,
                                        &msg_tx,
                                        &mut pump,
                                    )
                                };
                                match push_res {
                                    Ok(_) => {
                                        b.legacy_job = legacy;
                                        b.mining = true;
                                        armed_pool += 1;
                                        recent_jobs.push_back(job);
                                        let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                            "Board {} ← pool job (unique en2)",
                                            b.name
                                        ))));
                                    }
                                    Err(e) => {
                                        b.legacy_job = legacy;
                                        remaining.push_front(job);
                                        let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                            "Pool job {} failed: {e}",
                                            b.name
                                        ))));
                                        break;
                                    }
                                }
                                if client.has_pending_job() {
                                    remaining.clear();
                                    break;
                                }
                            }
                            while recent_jobs.len() > 96 {
                                recent_jobs.pop_front();
                            }
                            if armed_pool == 0 {
                                if let Some(job) = remaining.pop_front() {
                                    client.restore_job(job);
                                }
                            }
                        }
                    }
                    // Fallback: no pool header yet — keep boards warm until notify lands.
                    // Skip boards already on independent pool (they must not hash easy warmup).
                    if armed_pool == 0 {
                        for b in boards.iter_mut() {
                            if b.mine_indep {
                                continue;
                            }
                            let stats_cmd = "cmp stats accepted=0&rejected=0";
                            {
                                let mut pump = || {
                                    let _ = pump_stratum_keepalive(&mut stratum, &msg_tx);
                                };
                                let _ = usb_cmd_ex(
                                    &mut b.port,
                                    &mut b.rx,
                                    stats_cmd,
                                    &mut pump,
                                    None,
                                );
                            }
                            let mut legacy = b.legacy_job;
                            let push_res = {
                                let mut pump = || {
                                    let _ = pump_stratum_keepalive(&mut stratum, &msg_tx);
                                };
                                usb_push_job_ex(
                                    &mut b.port,
                                    &mut b.rx,
                                    &warmup_job(),
                                    &mut legacy,
                                    &msg_tx,
                                    &mut pump,
                                )
                            };
                            match push_res {
                                Ok(_) => {
                                    b.legacy_job = legacy;
                                    let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                        "Board {} hashing (warmup — waiting pool job)…",
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
                    }
                }
                NetCmd::StopMine => {
                    mining = false;
                    reconnect_at = None;
                    mine_endpoint.clear();
                    if let Some(mut s) = stratum.take() {
                        s.disconnect();
                        drain_stratum_log(&msg_tx, &mut s);
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
                    let mhz = normalize_cpu_mhz(mhz);
                    let mut any = false;
                    for b in boards.iter_mut() {
                        let cmd = format!("cmp clock cpu_mhz={mhz}");
                        if usb_cmd(&mut b.port, &mut b.rx, &cmd).is_ok() {
                            any = true;
                        }
                    }
                    let _ = msg_tx.send(NetMsg::Action(if any {
                        Ok(format!(
                            "Clock {mhz} MHz queued on {} board(s)",
                            boards.len()
                        ))
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
                    // Unplug detection first — UI must update even if pool owns the thread.
                    if prune_unplugged_boards(
                        &mut boards,
                        &msg_tx,
                        &mut mining,
                        &mut stratum,
                        &mut reconnect_at,
                        &mut last_fleet_status,
                        &flash_hold,
                    ) && boards.is_empty()
                    {
                        continue;
                    }
                    // Drain pool first — USB work below can take seconds.
                    if let Some(e) = pump_stratum_keepalive(&mut stratum, &msg_tx) {
                        let give_up = stratum
                            .as_ref()
                            .map(|s| s.auth_give_up || s.phase == "auth-fail")
                            .unwrap_or(false);
                        if let Some(mut s) = stratum.take() {
                            s.disconnect();
                        }
                        log_msg(
                            &msg_tx,
                            LogKind::Err,
                            format!("Pool {mine_endpoint} as {mine_worker_name}: {e}"),
                        );
                        if give_up {
                            mining = false;
                            reconnect_at = None;
                        } else if mining && !mine_endpoint.is_empty() {
                            reconnect_at = Some(Instant::now() + reconnect_backoff);
                            log_msg(
                                &msg_tx,
                                LogKind::Warn,
                                format!(
                                    "Pool reconnect in {}s · {mine_endpoint} · backoff growing",
                                    reconnect_backoff.as_secs().max(1)
                                ),
                            );
                            reconnect_backoff =
                                (reconnect_backoff * 2).min(Duration::from_secs(60));
                        }
                    }
                    // If the pool is linking / has a job / reconnecting, skip USB status
                    // this tick so the hot loop can push work immediately.
                    if pool_has_presidency(&stratum, reconnect_at) {
                        publish_live(&msg_tx, &boards);
                        let _ = msg_tx.send(NetMsg::Status(Ok(last_fleet_status.clone())));
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
                            if b.mine_indep {
                                None
                            } else {
                                stratum.as_mut()
                            },
                            &recent_jobs,
                            &mut held_board_shares,
                            &msg_tx,
                            b.mine_indep,
                        );
                        let status_reply = {
                            let mut pump = || {
                                let _ = pump_stratum_keepalive(&mut stratum, &msg_tx);
                            };
                            usb_cmd_ex(
                                &mut b.port,
                                &mut b.rx,
                                "cmp status",
                                &mut pump,
                                None,
                            )
                        };
                        match status_reply {
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
                                    let indep_live = board_indep_live(&st);
                                    if indep_live && !b.mine_indep {
                                        log_msg(
                                            &msg_tx,
                                            LogKind::Usb,
                                            format!(
                                                "Board {} independent pool live (phase={}) — Companion stops job push; monitors H/s",
                                                b.name,
                                                if st.pool_phase.is_empty() {
                                                    "ok"
                                                } else {
                                                    st.pool_phase.as_str()
                                                }
                                            ),
                                        );
                                    } else if b.mine_indep && !indep_live {
                                        log_msg(
                                            &msg_tx,
                                            LogKind::Usb,
                                            format!(
                                                "Board {} indep pool down (phase={}) — Companion resumes job push",
                                                b.name,
                                                if st.pool_phase.is_empty() {
                                                    "-"
                                                } else {
                                                    st.pool_phase.as_str()
                                                }
                                            ),
                                        );
                                    }
                                    b.mine_indep = indep_live;
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
                                    if b.status_fails >= 4 {
                                        drop_names.push(b.name.clone());
                                    }
                                }
                            },
                            Err(e) => {
                                // Hard I/O (unplug mid-write) → drop immediately.
                                if usb_err_looks_unplugged(&e) {
                                    drop_names.push(b.name.clone());
                                    log_msg(
                                        &msg_tx,
                                        LogKind::Warn,
                                        format!("USB {} gone ({e})", b.name),
                                    );
                                    continue;
                                }
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
                                // Faster drop than before — 12×~2s left a ghost board in the UI.
                                if b.status_fails >= 4 {
                                    drop_names.push(b.name.clone());
                                }
                            }
                        }
                        let _ = pump_stratum_keepalive(&mut stratum, &msg_tx);
                    }
                    // Via status is chrome — never while mining (jobs/shares own the USB root).
                    for name in drop_names {
                        if let Some(idx) = boards.iter().position(|b| b.name == name) {
                            let dead = boards.remove(idx);
                            // Don't usb_cmd stop — port is often already gone (hangs the UI).
                            drop(dead);
                            log_msg(
                                &msg_tx,
                                LogKind::Err,
                                format!("Dropped dead board {name}"),
                            );
                        }
                    }
                    // Keep COM combo in sync when boards die from status fails.
                    // Do not re-enum every status tick — that flooded the UI log ~1 Hz.
                    if !boards.is_empty() && last_ports_enum.elapsed() >= Duration::from_secs(5) {
                        last_ports_enum = Instant::now();
                        let _ = msg_tx.send(NetMsg::Ports(list_serial_ports()));
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
                        // n≈60k → dual-pass per path with live CMPBENCHPROG kH updates.
                        // Keep stratum alive during the long USB wait (empty pump starved the pool).
                        match usb_bench_cmd(
                            &mut b.port,
                            &mut b.rx,
                            "cmp bench tune=1&n=60000",
                            &msg_tx,
                            &mut || {
                                if let Some(client) = stratum.as_mut() {
                                    let _ = client.poll();
                                }
                            },
                        ) {
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
                        // Resume with distinct en2 per board — never share one job across the fleet.
                        let mut seen_en2 = std::collections::HashSet::new();
                        let mut unique: Vec<WorkJob> = Vec::new();
                        for j in recent_jobs.iter().rev() {
                            if seen_en2.insert(j.extranonce2_hex.clone()) {
                                unique.push(j.clone());
                                if unique.len() >= boards.len() {
                                    break;
                                }
                            }
                        }
                        let mut remaining: VecDeque<WorkJob> = unique.into();
                        for b in boards.iter_mut() {
                            let mut legacy = b.legacy_job;
                            let _ = usb_cmd(
                                &mut b.port,
                                &mut b.rx,
                                "cmp stats accepted=0&rejected=0",
                            );
                            let Some(job) = remaining.pop_front() else {
                                // Do not arm warmup after bench — warmup shares are discarded
                                // and look like zero accepts while LCD hashrate recovers.
                                log_msg(
                                    &msg_tx,
                                    LogKind::Usb,
                                    format!(
                                        "{} waiting for unique pool en2 after Bench (no warmup)",
                                        b.name
                                    ),
                                );
                                continue;
                            };
                            let had_pool = !job.job_id.is_empty()
                                && !is_warmup_job_id(&job.job_id)
                                && !job.extranonce2_hex.is_empty();
                            if !had_pool {
                                log_msg(
                                    &msg_tx,
                                    LogKind::Usb,
                                    format!(
                                        "{} skipping non-pool job after Bench — waiting notify",
                                        b.name
                                    ),
                                );
                                continue;
                            }
                            match usb_push_job(
                                &mut b.port,
                                &mut b.rx,
                                &job,
                                &mut legacy,
                                &msg_tx,
                            ) {
                                Ok(_) => {
                                    b.legacy_job = legacy;
                                    log_msg(
                                        &msg_tx,
                                        LogKind::Usb,
                                        format!(
                                            "{} resumed pool job {} (en2 {}) after Bench",
                                            b.name, job.job_id, job.extranonce2_hex
                                        ),
                                    );
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
                NetCmd::UpdateFirmware {
                    port,
                    image,
                    reopen,
                    cancel,
                    need_boot,
                    boot_ready,
                    hold,
                    live_push,
                    wifi_ota,
                } => {
                    flash_hold = Some((port.clone(), hold.clone()));
                    // Preempt already aborted in-flight via/job waits. Clear it so our own
                    // cmp stop below is not immediately cancelled (that froze UI at 2%).
                    USB_FLASH_PREEMPT.store(false, Ordering::SeqCst);
                    // Only release the flash target — keep other linked boards.
                    // Prefer USB OTA on the *already-open* link (correct baud) before drop.
                    let mut live_ota_done: Option<Result<String, String>> = None;
                    if live_push && !wifi_ota {
                        if let Some(idx) = boards
                            .iter()
                            .position(|b| port_names_match(&b.name, &port))
                        {
                            let mut b = boards.remove(idx);
                            let prefer_d0 = board_fw_is_d0(&b.fw)
                                || board_fw_is_d0(&image);
                            let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                            b.port.clear();
                            b.rx.clear();
                            let app = {
                                let local = std::path::PathBuf::from(&image);
                                let preferred = if !image.is_empty() && local.is_file() {
                                    Some(local.as_path())
                                } else {
                                    None
                                };
                                resolve_ota_app_image_ex(preferred, prefer_d0).or_else(|_| {
                                    resolve_ota_app_image_ex(None, prefer_d0)
                                })
                            };
                            if let Ok(app) = app {
                                log_msg(
                                    &msg_tx,
                                    LogKind::Usb,
                                    format!(
                                        "Live Push — USB OTA on open link {port} (no BOOT)…"
                                    ),
                                );
                                let progress_tx = msg_tx.clone();
                                let progress = |line: String| {
                                    let _ = progress_tx.send(NetMsg::FlashProgress(line));
                                };
                                let ota = match &mut b.port {
                                    BoardIo::Serial(p) => push_firmware_ota_on_port_recover_ex(
                                        &mut **p,
                                        &app.path,
                                        prefer_d0,
                                        &progress,
                                        &cancel,
                                        Some(OtaReopen::Usb(&port)),
                                    ),
                                    BoardIo::Tcp(s) => push_firmware_ota_on_port_recover_ex(
                                        s,
                                        &app.path,
                                        prefer_d0,
                                        &progress,
                                        &cancel,
                                        Some(OtaReopen::Tcp(&port)),
                                    ),
                                };
                                match ota {
                                    Ok(()) => {
                                        live_ota_done = Some(Ok(format!(
                                            "Firmware {} pushed over USB OTA to {port}",
                                            if app.version.is_empty() {
                                                "image".into()
                                            } else {
                                                app.version
                                            }
                                        )));
                                    }
                                    Err(e) => {
                                        log_msg(
                                            &msg_tx,
                                            LogKind::Warn,
                                            format!(
                                                "Open-link USB OTA failed ({e}) — will reopen + retry / ROM"
                                            ),
                                        );
                                    }
                                }
                            }
                            // Drop the handle so reopen / ROM flash can own the COM.
                            drop(b);
                        }
                    } else if let Some(idx) = boards
                        .iter()
                        .position(|b| port_names_match(&b.name, &port))
                    {
                        let mut b = boards.remove(idx);
                        let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                    }
                    if let Some(result) = live_ota_done {
                        if boards.is_empty() {
                            mining = false;
                            if let Some(mut s) = stratum.take() {
                                s.disconnect();
                            }
                        }
                        publish_live(&msg_tx, &boards);
                        hold.store(false, Ordering::SeqCst);
                        USB_FLASH_PREEMPT.store(false, Ordering::SeqCst);
                        let reopen_port = if reopen && result.is_ok() {
                            Some(port.clone())
                        } else {
                            None
                        };
                        let _ = msg_tx.send(NetMsg::FlashDone {
                            result,
                            reopen: reopen_port,
                        });
                        continue;
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
                    let mode_label = if wifi_ota {
                        "Wi‑Fi OTA"
                    } else if live_push {
                        "USB OTA push"
                    } else {
                        "flash"
                    };
                    log_msg(
                        &msg_tx,
                        LogKind::Usb,
                        format!(
                            "Released {port} for {mode_label} · {} board(s) still linked",
                            boards.len()
                        ),
                    );
                    // Settle long enough for CH340/Windows to drop the old handle —
                    // too short and the next espflash open looks like connect/disconnect thrash.
                    thread::sleep(Duration::from_millis(if wifi_ota {
                        250
                    } else if live_push {
                        700
                    } else {
                        900
                    }));

                    // Run flash off the mine-worker so Cancel / port list keep working.
                    let progress_tx = msg_tx.clone();
                    let done_tx = msg_tx.clone();
                    let hold_clear = hold.clone();
                    thread::spawn(move || {
                        let progress = move |line: String| {
                            let _ = progress_tx.send(NetMsg::FlashProgress(line));
                        };
                        progress(format!(
                            "Released {port} — preparing flash tool…"
                        ));
                        // UTF-8 mid-char panics in OTA/flash must not kill the GUI.
                        let result: Result<String, String> = match std::panic::catch_unwind(
                            std::panic::AssertUnwindSafe(|| -> Result<String, String> {
                            if cancel.load(Ordering::SeqCst) {
                                return Err("flash cancelled".into());
                            }
                            if wifi_ota {
                                let prefer_d0 = board_fw_is_d0(&image);
                                let img = {
                                    let local = std::path::PathBuf::from(&image);
                                    let preferred = if !image.is_empty() && local.is_file() {
                                        Some(local.as_path())
                                    } else {
                                        None
                                    };
                                    resolve_ota_app_image_ex(preferred, prefer_d0).or_else(|_| {
                                        ensure_firmware_image(&progress).and_then(|merged| {
                                            resolve_ota_app_image_ex(
                                                Some(merged.path.as_path()),
                                                prefer_d0,
                                            )
                                        })
                                    })?
                                };
                                let _ = done_tx.send(NetMsg::FirmwareFetched(Ok(img.clone())));
                                push_firmware_ota_ex(
                                    &port,
                                    &img.path,
                                    prefer_d0,
                                    &progress,
                                    &cancel,
                                )?;
                                return Ok(format!(
                                    "Firmware {} pushed over Wi‑Fi to {port}",
                                    if img.version.is_empty() {
                                        "image".into()
                                    } else {
                                        img.version
                                    }
                                ));
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
                            // Live Push: USB app OTA only (no BOOT). Silent ROM fallthrough
                            // was demoted — it fought OTA and failed at Chip connected ~14%.
                            if live_push {
                                progress(
                                    "Live Push — USB app OTA (no BOOT, no hold)…".into(),
                                );
                                let prefer_d0 = board_fw_is_d0(&image)
                                    || board_fw_is_d0(&img.path.to_string_lossy());
                                match resolve_ota_app_image_ex(Some(img.path.as_path()), prefer_d0)
                                    .or_else(|_| resolve_ota_app_image_ex(None, prefer_d0))
                                {
                                    Ok(app) => {
                                        match push_firmware_ota_usb_ex(
                                            &port,
                                            &app.path,
                                            prefer_d0,
                                            &progress,
                                            &cancel,
                                        ) {
                                            Ok(()) => {
                                                return Ok(format!(
                                                    "Firmware {} pushed over USB OTA to {port}",
                                                    if app.version.is_empty() {
                                                        "image".into()
                                                    } else {
                                                        app.version
                                                    }
                                                ));
                                            }
                                            Err(e) => {
                                                let low = e.to_ascii_lowercase();
                                                // Reached binary mode / mid-upload: ROM auto-reset
                                                // almost always dies at Chip connected (~14/15%).
                                                // Clear the stuck OTA session and retry Push OTA once;
                                                // do not fall through to espflash.
                                                let ota_started = low.contains("board ready")
                                                    || low.contains("streaming")
                                                    || low.contains("upload")
                                                    || low.contains("mid-upload")
                                                    || low.contains("ota write")
                                                    || low.contains("no cmpack ota ok")
                                                    || low.contains("rejected");
                                                if ota_started {
                                                    progress(format!(
                                                        "USB OTA stalled after board ready ({e}) — reboot nudge + one more OTA (no ROM)…"
                                                    ));
                                                    let _ = nudge_usb_reboot_for_push(
                                                        &port, &progress, &cancel,
                                                    );
                                                    std::thread::sleep(Duration::from_secs(10));
                                                    if cancel.load(Ordering::SeqCst) {
                                                        return Err("flash cancelled".into());
                                                    }
                                                    match push_firmware_ota_usb_ex(
                                                        &port,
                                                        &app.path,
                                                        prefer_d0,
                                                        &progress,
                                                        &cancel,
                                                    ) {
                                                        Ok(()) => {
                                                            return Ok(format!(
                                                                "Firmware {} pushed over USB OTA to {port}",
                                                                if app.version.is_empty() {
                                                                    "image".into()
                                                                } else {
                                                                    app.version
                                                                }
                                                            ));
                                                        }
                                                        Err(e2) => {
                                                            return Err(format!(
                                                                "USB OTA failed after board ready ({e2}). \
Retry Push update — do not use Flash (BOOT) unless the board is blank. \
If this keeps failing, unplug/replug USB then Push again."
                                                            ));
                                                        }
                                                    }
                                                }
                                                // Live Push = app OTA only. Silent ROM was the
                                                // fighting double that failed at Chip connected ~14%.
                                                return Err(format!(
                                                    "USB OTA unavailable ({e}). Retry Push. \
Use Flash (BOOT) only for blank / download-mode boards — Push will not fall through to ROM."
                                                ));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        return Err(format!(
                                            "No app image for USB OTA ({e}). Fetch firmware, then Push again. \
Blank boards need Flash (BOOT)."
                                        ));
                                    }
                                }
                            }
                            // Flash (BOOT) path only — never reached for live_push after OTA.
                            let ctrl = FlashControl {
                                cancel,
                                need_boot,
                                boot_ready,
                            };
                            if live_push {
                                return Err(
                                    "Push update is USB OTA only (no silent ROM). Retry Push after unplug/replug."
                                        .into(),
                                );
                            }
                            flash_merged_bin(&port, &img.path, &progress, &ctrl, false)?;
                            Ok(format!(
                                "Firmware {} flashed on {port}",
                                if img.version.is_empty() {
                                    "image".into()
                                } else {
                                    img.version
                                },
                            ))
                            }),
                        ) {
                            Ok(r) => r,
                            Err(_) => Err(
                                "Flash/OTA thread panicked (see cyd-companion-crash.log next to the exe). Retry Push."
                                    .into(),
                            ),
                        };
                        hold_clear.store(false, Ordering::SeqCst);
                        // Allow mine-worker USB again after flash thread finishes.
                        USB_FLASH_PREEMPT.store(false, Ordering::SeqCst);
                        // On failure never ask the UI to reopen — that thrashing COM
                        // is what looks like looping terminals after a failed update.
                        let reopen_port = if reopen && result.is_ok() {
                            Some(port)
                        } else {
                            None
                        };
                        let _ = done_tx.send(NetMsg::FlashDone {
                            result,
                            reopen: reopen_port,
                        });
                    });
                }
                NetCmd::EraseFlash {
                    port,
                    cancel,
                    need_boot,
                    boot_ready,
                    hold,
                } => {
                    flash_hold = Some((port.clone(), hold.clone()));
                    USB_FLASH_PREEMPT.store(false, Ordering::SeqCst);
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
                        for b in boards.iter_mut() {
                            let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                        }
                    }
                    publish_live(&msg_tx, &boards);
                    log_msg(
                        &msg_tx,
                        LogKind::Usb,
                        format!("Released {port} for full-chip erase…"),
                    );
                    thread::sleep(Duration::from_millis(900));
                    let progress_tx = msg_tx.clone();
                    let done_tx = msg_tx.clone();
                    let hold_clear = hold.clone();
                    thread::spawn(move || {
                        let progress = move |line: String| {
                            let _ = progress_tx.send(NetMsg::FlashProgress(line));
                        };
                        let result: Result<String, String> = match std::panic::catch_unwind(
                            std::panic::AssertUnwindSafe(|| -> Result<String, String> {
                                if cancel.load(Ordering::SeqCst) {
                                    return Err("erase cancelled".into());
                                }
                                let ctrl = FlashControl {
                                    cancel,
                                    need_boot,
                                    boot_ready,
                                };
                                erase_flash_bin(&port, &progress, &ctrl)?;
                                Ok(format!("Flash erased on {port} — use Flash (BOOT) to reinstall"))
                            }),
                        ) {
                            Ok(r) => r,
                            Err(_) => Err(
                                "Erase thread panicked (see cyd-companion-crash.log). Retry Erase flash."
                                    .into(),
                            ),
                        };
                        hold_clear.store(false, Ordering::SeqCst);
                        USB_FLASH_PREEMPT.store(false, Ordering::SeqCst);
                        let _ = done_tx.send(NetMsg::FlashDone {
                            result,
                            reopen: None,
                        });
                    });
                }
                NetCmd::RebootBoard { endpoint } => {
                    let ep = endpoint.trim().to_string();
                    if ep.is_empty() {
                        let _ = msg_tx.send(NetMsg::Action(Err(
                            "Reset board: no endpoint.".into(),
                        )));
                        continue;
                    }
                    if let Some(idx) = boards.iter().position(|b| port_names_match(&b.name, &ep)) {
                        let b = &mut boards[idx];
                        match usb_cmd(&mut b.port, &mut b.rx, "cmp reboot") {
                            Ok(_) => {
                                log_msg(
                                    &msg_tx,
                                    LogKind::Usb,
                                    format!("cmp reboot sent to {ep}"),
                                );
                                let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                    "Board reset: {ep}"
                                ))));
                            }
                            Err(e) => {
                                log_msg(
                                    &msg_tx,
                                    LogKind::Warn,
                                    format!("cmp reboot failed on {ep}: {e}"),
                                );
                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                    "Reset failed on {ep}: {e}"
                                ))));
                            }
                        }
                    } else {
                        match send_cmp_reboot(&ep) {
                            Ok(()) => {
                                log_msg(
                                    &msg_tx,
                                    LogKind::Usb,
                                    format!("cmp reboot sent to {ep} (one-shot)"),
                                );
                                let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                    "Board reset: {ep}"
                                ))));
                            }
                            Err(e) => {
                                log_msg(
                                    &msg_tx,
                                    LogKind::Warn,
                                    format!("cmp reboot failed on {ep}: {e}"),
                                );
                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                    "Reset failed on {ep}: {e}"
                                ))));
                            }
                        }
                    }
                }
                NetCmd::PullApiFeed(feed) => {
                    let outcome = pull_feed(&feed);
                    let _ = msg_tx.send(NetMsg::ApiFeedResult(outcome));
                }
            }
        }

        // Update board clicked — skip jobs until flash owns the COM (preempt
        // also aborts in-flight usb_cmd_ex via waits).
        if USB_FLASH_PREEMPT.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(if flash_any_held(&flash_hold) {
                40
            } else {
                15
            }));
            continue;
        }

        // Detect unplug even while stratum has presidency (status polls may be deferred).
        if !boards.is_empty() && last_unplug_check.elapsed() >= Duration::from_millis(400) {
            last_unplug_check = Instant::now();
            let _ = prune_unplugged_boards(
                        &mut boards,
                &msg_tx,
                &mut mining,
                &mut stratum,
                &mut reconnect_at,
                &mut last_fleet_status,
                &flash_hold,
            );
        }

        // Prefer pool I/O before USB drain while authorizing — USB can starve the handshake.
        let pool_needs_handshake = stratum
            .as_ref()
            .map(|s| s.stream_connected() && !s.authorized() && !s.auth_give_up)
            .unwrap_or(false);
        if pool_needs_handshake {
            let mut fatal: Option<(String, bool)> = None;
            if let Some(client) = stratum.as_mut() {
                for _ in 0..8 {
                    if client.authorized() || client.auth_give_up {
                        break;
                    }
                    if let Err(e) = client.poll() {
                        drain_stratum_log(&msg_tx, client);
                        push_stratum_live(&msg_tx, client);
                        fatal = Some((e, client.auth_give_up));
                        break;
                    }
                }
                if fatal.is_none() {
                    drain_stratum_log(&msg_tx, client);
                    push_stratum_live(&msg_tx, client);
                }
            }
            if let Some((e, give_up)) = fatal {
                if let Some(mut s) = stratum.take() {
                    s.disconnect();
                }
                let _ = msg_tx.send(NetMsg::Action(Err(e)));
                if give_up {
                    mining = false;
                    reconnect_at = None;
                } else if mining && !mine_endpoint.is_empty() {
                    reconnect_at = Some(Instant::now() + reconnect_backoff);
                    reconnect_backoff =
                        (reconnect_backoff * 2).min(Duration::from_secs(60));
                }
            }
        }

        // STRATUM PRESIDENCY: drain/push pool work before board share harvest or USB chrome.
        if stratum.is_some() {
            let was_authorized = stratum.as_ref().map(|c| c.authorized()).unwrap_or(false);
            let mut poll_err: Option<String> = None;
            if let Some(client) = stratum.as_mut() {
                for _ in 0..4 {
                    match client.poll() {
                        Ok(()) => {}
                        Err(e) => {
                            poll_err = Some(e);
                            break;
                        }
                    }
                }
            }
            if let Some(e) = poll_err {
                let give_up = stratum
                    .as_ref()
                    .map(|s| s.auth_give_up || s.phase == "auth-fail")
                    .unwrap_or(false);
                if let Some(client) = stratum.as_mut() {
                    drain_stratum_log(&msg_tx, client);
                    push_stratum_live(&msg_tx, client);
                }
                log_msg(
                    &msg_tx,
                    LogKind::Err,
                    format!("Pool {mine_endpoint} as {mine_worker_name}: {e}"),
                );
                if let Some(mut s) = stratum.take() {
                    s.disconnect();
                }
                if give_up {
                    mining = false;
                    reconnect_at = None;
                    let _ = msg_tx.send(NetMsg::Action(Err(e)));
                    let _ = msg_tx.send(NetMsg::MineStats {
                        accepted: 0,
                        rejected: 0,
                        phase: "auth-fail".into(),
                    });
                } else if mining && !mine_endpoint.is_empty() {
                    reconnect_at = Some(Instant::now() + reconnect_backoff);
                    log_msg(
                        &msg_tx,
                        LogKind::Warn,
                        format!(
                            "Pool reconnect in {}s · {mine_endpoint} · last error above",
                            reconnect_backoff.as_secs().max(1)
                        ),
                    );
                    reconnect_backoff =
                        (reconnect_backoff * 2).min(Duration::from_secs(60));
                }
                thread::sleep(Duration::from_millis(100));
            } else if let Some(client) = stratum.as_mut() {
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
                drain_stratum_log(&msg_tx, client);
                for ev in client.take_share_events() {
                    let _ = msg_tx.send(NetMsg::Share(ev));
                }
                // NerdMiner-style clean_jobs: drop superseded job_ids; same-id clean clears
                // prior en2 *after* re-arming boards (purging before push caused unknown-en2
                // drops while boards still hashed the old header → pool ≪ LCD H/s).
                let cleaned = client.take_clean_jobs();
                let active_job = client.job_id().to_string();
                let before_jobs = recent_jobs.len();
                recent_jobs.retain(|j| !client.is_job_stale(&j.job_id));
                held_board_shares.retain(|(job, _, _, _)| !client.is_job_stale(job));
                if cleaned {
                    let dropped = before_jobs.saturating_sub(recent_jobs.len());
                    log_msg(
                        &msg_tx,
                        LogKind::Stratum,
                        format!(
                            "Pool clean_jobs — dropped {dropped} superseded job cache entr(y/ies); pausing boards before unique-en2 re-arm"
                        ),
                    );
                    // Stop companion-fed boards so they don't keep hashing abandoned work
                    // (LCD would stay high while pool rejects/ignores those shares).
                    for b in boards.iter_mut() {
                        if b.mine_indep {
                            continue;
                        }
                        let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                        b.mining = false;
                    }
                }
                // SoftAP setup net has no pool uplink — don't keep pushing work that can't submit.
                let softap_blocked = softap_setup_client_ipv4()
                    .map(|ip| ip.starts_with("10.88.88."))
                    .unwrap_or(false);
                if softap_blocked {
                    static LAST_SOFTAP_WARN: std::sync::Mutex<Option<Instant>> =
                        std::sync::Mutex::new(None);
                    let mut due = true;
                    if let Ok(mut g) = LAST_SOFTAP_WARN.lock() {
                        if let Some(t) = *g {
                            if t.elapsed() < Duration::from_secs(20) {
                                due = false;
                            }
                        }
                        if due {
                            *g = Some(Instant::now());
                        }
                    }
                    if due {
                        log_msg(
                            &msg_tx,
                            LogKind::Warn,
                            "PC on SoftAP (10.88.88.x) — pool has no uplink. Rejoin home Wi‑Fi; Companion-fed shares held. Indep boards keep mining."
                                .to_string(),
                        );
                        // SoftAP setup must not stop authorized indep miners (board pool wins).
                        for b in boards.iter_mut() {
                            if b.mine_indep {
                                continue;
                            }
                            let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                            b.mining = false;
                        }
                    }
                }
                // Pull any CMPSHARE already sitting in USB RX before we push a new
                // header (firmware also flushes its ring in onJob as of 0.8.127).
                if !softap_blocked && client.has_pending_job() {
                    for b in boards.iter_mut() {
                        if b.mine_indep {
                            // Board submits to its own pool — discard CMPSHARE (do not hold/submit).
                            harvest_shares(
                                &mut b.port,
                                &mut b.rx,
                                None,
                                &recent_jobs,
                                &mut held_board_shares,
                                &msg_tx,
                                true,
                            );
                            continue;
                        }
                        harvest_shares(
                            &mut b.port,
                            &mut b.rx,
                            Some(client),
                            &recent_jobs,
                            &mut held_board_shares,
                            &msg_tx,
                            false,
                        );
                    }
                }
                // Same-id clean: keep prior en2 in cache through harvest + push so mid-switch
                // CMPSHAREs still resolve. Purge only after new unique en2 are armed.
                let companion_fleet = boards
                    .iter()
                    .filter(|b| !b.mine_indep)
                    .count()
                    ;
                // When every board is indep, do not take_job_batch(.max(1)) — that consumed
                // pool notifies on the PC and dropped them (PC stratum fighting board pool).
                let jobs = if softap_blocked || companion_fleet == 0 {
                    Vec::new()
                } else {
                    client.take_job_batch(companion_fleet)
                };
                if !jobs.is_empty() {
                    // Pause each board immediately before its new en2 so we don't mine
                    // abandoned headers during set_difficulty / clean re-arm (LCD≫pool).
                    for b in boards.iter_mut() {
                        if b.mine_indep {
                            continue;
                        }
                        let _ = usb_cmd(&mut b.port, &mut b.rx, "cmp stop");
                        b.mining = false;
                    }
                    // Catch CMPSHARE flushed by stop before we swap headers.
                    for b in boards.iter_mut() {
                        if b.mine_indep {
                            continue;
                        }
                        harvest_shares(
                            &mut b.port,
                            &mut b.rx,
                            Some(client),
                            &recent_jobs,
                            &mut held_board_shares,
                            &msg_tx,
                            false,
                        );
                    }
                    let mut pushed = 0usize;
                    let mut pushed_en2: std::collections::HashSet<String> =
                        std::collections::HashSet::new();
                    let mut stale_abort = false;
                    let mut remaining: VecDeque<WorkJob> = jobs.into();
                    for b in boards.iter_mut() {
                        if b.mine_indep {
                            continue;
                        }
                        let Some(job) = remaining.pop_front() else { break };
                        let mut legacy = b.legacy_job;
                        let push_res = {
                            let mut pump = || {
                                let _ = client.poll();
                            };
                            usb_push_job_ex(
                                &mut b.port,
                                &mut b.rx,
                                &job,
                                &mut legacy,
                                &msg_tx,
                                &mut pump,
                            )
                        };
                        match push_res {
                            Ok(_) => {
                                b.legacy_job = legacy;
                                b.mining = true;
                                pushed += 1;
                                pushed_en2.insert(job.extranonce2_hex.to_ascii_lowercase());
                                recent_jobs.push_back(job);
                            }
                            Err(e) => {
                                b.legacy_job = legacy;
                                remaining.push_front(job);
                                let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                    "{} job push: {e}",
                                    b.name
                                ))));
                                break;
                            }
                        }
                        // Newer notify wins — abandon remaining of this batch.
                        if client.has_pending_job() {
                            stale_abort = true;
                            remaining.clear();
                            break;
                        }
                    }
                    // Now drop superseded same-id en2 (and held shares) that were not re-armed.
                    if cleaned && !active_job.is_empty() && !pushed_en2.is_empty() {
                        recent_jobs.retain(|j| {
                            j.job_id != active_job
                                || pushed_en2.contains(&j.extranonce2_hex.to_ascii_lowercase())
                        });
                        held_board_shares.retain(|(job, en2, _, _)| {
                            job != &active_job
                                || pushed_en2.contains(&en2.to_ascii_lowercase())
                        });
                    }
                    while recent_jobs.len() > 96 {
                        recent_jobs.pop_front();
                    }
                    // Keep cached targets aligned with live pool difficulty for any
                    // code paths still reading job.target (boards already re-armed on bump).
                    if client.authorized() {
                        let live_t = target_from_difficulty(client.difficulty());
                        for j in recent_jobs.iter_mut() {
                            j.target = live_t;
                        }
                    }
                    if stale_abort {
                        log_msg(
                            &msg_tx,
                            LogKind::Stratum,
                            format!(
                                "Job superseded mid-push — already pushed {pushed} · {} left in batch · pool stays first",
                                remaining.len()
                            ),
                        );
                    } else if pushed > 0 {
                        let job_id = recent_jobs
                            .back()
                            .map(|j| j.job_id.as_str())
                            .unwrap_or("?");
                        let en2 = recent_jobs
                            .back()
                            .map(|j| j.extranonce2_hex.as_str())
                            .unwrap_or("?");
                        let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                            "USB ← job → {pushed} board(s) · job={job_id} · en2={en2} · unique en2"
                        ))));
                    } else if let Some(job) = remaining.pop_front() {
                        // Total failure — restore so the next loop retries (pre-0.8.121 behavior).
                        client.restore_job(job);
                    }
                }
                if last_stats_push.elapsed() > Duration::from_secs(2)
                    && !client.has_pending_job()
                {
                    let (a, r) = if client.authorized() {
                        (client.accepted, client.rejected)
                    } else {
                        (0, 0)
                    };
                    let cmd = format!("cmp stats accepted={a}&rejected={r}");
                    for b in boards.iter_mut() {
                        if b.status_fails > 0 {
                            continue;
                        }
                        let _ = usb_cmd(&mut b.port, &mut b.rx, &cmd);
                    }
                    last_stats_push = Instant::now();
                }
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
                    format!(
                        "Reconnecting pool {mine_endpoint} as {mine_worker_name} · {} board(s)",
                        boards.len()
                    ),
                );
                let mut client =
                    StratumClient::new(mine_worker_name.clone(), mine_password.clone());
                match client.connect(&mine_endpoint) {
                    Ok(()) => {
                        // Pre-reconnect held shares are almost always stale after a new session.
                        held_board_shares.clear();
                        drain_stratum_log(&msg_tx, &mut client);
                        push_stratum_live(&msg_tx, &client);
                        stratum = Some(client);
                        reconnect_backoff = Duration::from_secs(1);
                        let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                            "Pool reconnected {mine_endpoint} as {mine_worker_name} · authorized next"
                        ))));
                        // Keep hashrate across the gap — re-push unique cached en2 jobs only
                        // (never broadcast one en2 to the whole fleet).
                        let fleet_n = boards.len();
                        let mut chosen: Vec<WorkJob> = Vec::new();
                        for j in recent_jobs.iter().rev() {
                            if chosen
                                .iter()
                                .any(|c| c.extranonce2_hex == j.extranonce2_hex)
                            {
                                continue;
                            }
                            chosen.push(j.clone());
                            if chosen.len() >= fleet_n {
                                break;
                            }
                        }
                        chosen.reverse();
                        let mut it = chosen.into_iter();
                        for b in boards.iter_mut() {
                            let Some(job) = it.next() else { break };
                            let mut legacy = b.legacy_job;
                            let push_res = {
                                let mut pump = || {
                                    if let Some(c) = stratum.as_mut() {
                                        let _ = c.poll();
                                    }
                                };
                                usb_push_job_ex(
                                    &mut b.port,
                                    &mut b.rx,
                                    &job,
                                    &mut legacy,
                                    &msg_tx,
                                    &mut pump,
                                )
                            };
                            if push_res.is_ok() {
                                b.legacy_job = legacy;
                                b.mining = true;
                            }
                        }
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

        // Board shares after pool drain — submit ASAP without delaying stratum poll/jobs.
        // SoftAP setup net has no uplink: hold in RX / held queue, do not submit into a dead TCP.
        let softap_no_uplink = softap_setup_client_ipv4()
            .map(|ip| ip.starts_with("10.88.88."))
            .unwrap_or(false);
        if !boards.is_empty() && !softap_no_uplink {
            for b in boards.iter_mut() {
                if b.mine_indep {
                    harvest_shares(
                        &mut b.port,
                        &mut b.rx,
                        None,
                        &recent_jobs,
                        &mut held_board_shares,
                        &msg_tx,
                        true,
                    );
                } else {
                    harvest_shares(
                        &mut b.port,
                        &mut b.rx,
                        stratum.as_mut(),
                        &recent_jobs,
                        &mut held_board_shares,
                        &msg_tx,
                        false,
                    );
                }
            }
        } else if !boards.is_empty() && softap_no_uplink {
            // Drain CMPSHARE into the hold queue without touching the pool socket.
            for b in boards.iter_mut() {
                harvest_shares(
                    &mut b.port,
                    &mut b.rx,
                    None,
                    &recent_jobs,
                    &mut held_board_shares,
                    &msg_tx,
                    b.mine_indep,
                );
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
    discard: bool,
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
            if discard || is_warmup_job_id(&job) {
                if !discard && is_warmup_job_id(&job) {
                    log_msg(
                        msg_tx,
                        LogKind::Info,
                        format!("Ignoring local/warmup share nonce={nonce}"),
                    );
                }
                continue;
            }
            shares.push((job, en2, ntime, nonce));
        } else if !t.is_empty() {
            keep.push_str(t);
            keep.push('\n');
        }
    }
    *buf = keep;

    if discard {
        return;
    }

    if let Some(s) = stratum {
        let mut pending: Vec<(String, String, String, String)> =
            std::mem::take(held).into_iter().collect();
        pending.extend(shares);
        for (job, en2, ntime, nonce) in pending {
            if is_warmup_job_id(&job) {
                log_msg(
                    msg_tx,
                    LogKind::Info,
                    format!("Ignoring local/warmup share nonce={nonce}"),
                );
                continue;
            }
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
            if is_warmup_job_id(&job) {
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
                    "Holding {} board share(s) until pool reconnects · newest job={} nonce={}",
                    held.len(),
                    held.back().map(|h| h.0.as_str()).unwrap_or("—"),
                    held.back().map(|h| h.3.as_str()).unwrap_or("—"),
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

/// Local easy-target arming / truncated USB job ids (`warm`, `warmu`, `warmup`).
fn is_warmup_job_id(job: &str) -> bool {
    let j = job.trim();
    if j.is_empty() {
        return true;
    }
    let lower = j.to_ascii_lowercase();
    lower == "warmup" || lower.starts_with("warm")
}

/// Board owns stratum only after onboard pool authorize (phase ok / mine).
fn board_indep_live(st: &StatusJson) -> bool {
    if !st.mine_indep {
        return false;
    }
    let p = st.pool_phase.as_str();
    p.eq_ignore_ascii_case("ok") || p.eq_ignore_ascii_case("mine")
}

/// HMPool-style `address.worker` label.
/// Bare addresses become `address.companion` so the dashboard always lists a named worker.
/// Companion PC and every board use this same string — splitting `.companion` vs `.cydXXXX`
/// made HMPool look like the worker never registered.
fn stratum_worker_for_companion(base: &str) -> String {
    let w = base.trim();
    if w.is_empty() {
        return String::new();
    }
    if w.contains('.') {
        return w.to_string();
    }
    format!("{w}.companion")
}

/// Compare stratum URLs ignoring scheme / trailing slash noise.
fn stratum_endpoints_match(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .trim_end_matches('/')
            .trim_start_matches("stratum+tcp://")
            .trim_start_matches("stratum+ssl://")
            .trim_start_matches("tcp://")
            .to_ascii_lowercase()
    };
    norm(a) == norm(b)
}

/// Board onboard stratum uses the same pool username as Companion (no MAC suffix).
fn stratum_worker_for_board(base: &str, _mac: &str) -> String {
    stratum_worker_for_companion(base)
}

fn stratum_password(raw: &str) -> String {
    let p = raw.trim();
    if p.is_empty() {
        "x".into()
    } else {
        p.to_string()
    }
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
    if is_warmup_job_id(job) {
        log_msg(
            msg_tx,
            LogKind::Info,
            format!("Ignoring local/warmup share nonce={nonce}"),
        );
        return ShareSubmitResult::Dropped;
    }
    if s.is_job_stale(job) {
        log_msg(
            msg_tx,
            LogKind::Warn,
            format!("Dropping share for superseded job={job} en2={en2}"),
        );
        return ShareSubmitResult::Dropped;
    }
    let en2_l = en2.to_ascii_lowercase();
    if let Some(wj) = recent_jobs.iter().rev().find(|j| {
        j.job_id == job && j.extranonce2_hex.eq_ignore_ascii_case(en2)
    }) {
        // Always check against *live* pool difficulty — cached job.target can be easier
        // after mining.set_difficulty climbed (Low-difficulty rejects).
        let live_diff = s.difficulty();
        if let Err(e) = StratumClient::verify_share_meets_difficulty(wj, nonce, live_diff) {
            // Board may still be on the previous easier target for a few ms after a
            // vardiff bump; if the share meets the *cached* job target, hold briefly
            // instead of dropping — re-arm will make live==job or the share ages out.
            if StratumClient::verify_share_against_job(wj, nonce).is_ok() {
                log_msg(
                    msg_tx,
                    LogKind::Info,
                    format!(
                        "Holding board share nonce={nonce} job={job} (meets job target, live diff {live_diff} pending re-arm): {e}"
                    ),
                );
                return ShareSubmitResult::Hold;
            }
            log_msg(
                msg_tx,
                LogKind::Warn,
                format!(
                    "Dropping board share nonce={nonce} job={job} (live diff {live_diff}): {e}"
                ),
            );
            return ShareSubmitResult::Dropped;
        }
    } else {
        // Unknown en2 (clean_jobs / cache eviction): do not speculative-submit —
        // that was a major source of stale/invalid rejects while LCD hashrate stayed high.
        log_msg(
            msg_tx,
            LogKind::Warn,
            format!(
                "Dropping share job={job} en2={en2_l} — not in job cache (stale/clean)"
            ),
        );
        return ShareSubmitResult::Dropped;
    }
    match s.submit_share(job, en2, ntime, nonce) {
        Ok(()) => {
            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                "Share submitted nonce={nonce} job={job} en2={en2_l} ntime={ntime}"
            ))));
            ShareSubmitResult::Ok
        }
        Err(e) if e.contains("duplicate share") => {
            log_msg(
                msg_tx,
                LogKind::Warn,
                format!("Duplicate share skipped nonce={nonce} job={job} en2={en2_l}"),
            );
            ShareSubmitResult::Dropped
        }
        Err(e) if e.contains("not authorized") => {
            log_msg(
                msg_tx,
                LogKind::Info,
                format!("Holding share nonce={nonce} job={job} en2={en2_l} until authorize"),
            );
            ShareSubmitResult::Hold
        }
        Err(e) => {
            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                "Share submit failed nonce={nonce} job={job}: {e}"
            ))));
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
            "CMPBENCHPROG ",
            "CMPBENCH ",
            "CMPMESH ",
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
    let mut noop = || {};
    usb_cmd_ex(port, buf, cmd, &mut noop, None)
}

fn usb_bench_cmd(
    port: &mut BoardIo,
    buf: &mut String,
    cmd: &str,
    msg_tx: &Sender<NetMsg>,
    pump: &mut dyn FnMut(),
) -> Result<String, String> {
    usb_cmd_ex(port, buf, cmd, pump, Some(msg_tx))
}

/// USB command with optional stratum pump so long waits do not starve the pool.
fn usb_cmd_ex(
    port: &mut BoardIo,
    buf: &mut String,
    cmd: &str,
    pump: &mut dyn FnMut(),
    progress: Option<&Sender<NetMsg>>,
) -> Result<String, String> {
    let mut last_err = String::new();
    // Bigger writes + single flush cut job-push latency a lot vs per-chunk sleeps.
    let (wait_ms, retries, chunk, gap_ms) = if cmd.contains("bench") {
        // One long wait — retrying restarts a board that may still be mid-tune.
        // Old firmware (no CMPBENCHPROG) can hang the full window; fail-fast below.
        (180_000u64, 1usize, 128usize, 1u64)
    } else if cmd.contains(" via ") {
        // Firmware via wait ≤4.5s + mid-resend; leave margin for USB drain.
        (10_000u64, 1usize, 256usize, 0u64)
    } else if cmd.contains("wifi") {
        // SoftAP/STA apply after save can briefly stall the USB task.
        (3_500u64, 2usize, 256usize, 0u64)
    } else if cmd.contains("status") {
        // Board may be mid mineB batch; firmware yields on RX, but allow headroom.
        (1_800u64, 3usize, 256usize, 0u64)
    } else if cmd.contains("stats") {
        // Never stall the mine loop waiting on LCD stats ACKs.
        (450u64, 1usize, 256usize, 0u64)
    } else if cmd.contains(" jh") || cmd.contains(" jt") || cmd.contains(" ja") {
        (2_400u64, 3usize, 256usize, 0u64)
    } else if cmd.contains(" job ") {
        (3_200u64, 2usize, 128usize, 1u64)
    } else {
        (2_000u64, 3usize, 256usize, 0u64)
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
        let wait_started = Instant::now();
        let mut last_pump = Instant::now() - Duration::from_millis(200);
        let mut last_bench_hb = Instant::now() - Duration::from_secs(5);
        let mut saw_bench_prog = false;
        let bench_fail_fast = Instant::now() + Duration::from_secs(35);
        while Instant::now() < deadline {
            // Abort USB waits for board update — including cmp stop/status that used
            // to hold the mine-worker so UpdateFirmware never started (UI frozen at 2%).
            if USB_FLASH_PREEMPT.load(Ordering::SeqCst)
                && !cmd.contains("wifi")
                && !cmd.contains("pool")
            {
                return Err("aborted for board update".into());
            }
            if cmd.contains("bench") && USB_BENCH_CANCEL.load(Ordering::SeqCst) {
                return Err("bench cancelled".into());
            }
            // Heartbeat so the UI notifier keeps moving even between CMPBENCHPROG lines.
            if cmd.contains("bench") && last_bench_hb.elapsed() >= Duration::from_secs(5) {
                last_bench_hb = Instant::now();
                if let Some(tx) = progress {
                    let waited = wait_started.elapsed().as_secs();
                    let _ = tx.send(NetMsg::BenchProgress(format!(
                        "CMPBENCHPROG step=wait path=usb elapsed={waited}s"
                    )));
                }
            }
            // Pump often — stratum presidency means the pool is polled during every USB wait.
            if last_pump.elapsed() >= Duration::from_millis(40) {
                pump();
                last_pump = Instant::now();
            }
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
                if reply.starts_with("CMPBENCHPROG") {
                    saw_bench_prog = true;
                    if let Some(tx) = progress {
                        let _ = tx.send(NetMsg::BenchProgress(reply));
                    }
                    continue;
                }
                return Ok(reply);
            }
            // Old FW: no live progress lines — don't sit 180s on a hung bench.
            if cmd.contains("bench")
                && !saw_bench_prog
                && Instant::now() >= bench_fail_fast
            {
                return Err(format!(
                    "USB timeout waiting for reply to `{}` (no CMPBENCHPROG — update board firmware)",
                    cmd.chars().take(56).collect::<String>()
                ));
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
    let mut noop = || {};
    usb_push_job_ex(port, buf, job, legacy_job, msg_tx, &mut noop)
}

fn usb_push_job_ex(
    port: &mut BoardIo,
    buf: &mut String,
    job: &stratum::WorkJob,
    legacy_job: &mut bool,
    msg_tx: &Sender<NetMsg>,
    pump: &mut dyn FnMut(),
) -> Result<(), String> {
    if !*legacy_job {
        let mut split_ok = true;
        for part in encode_job_parts(job) {
            match usb_cmd_ex(port, buf, &part, pump, None) {
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
    let reply = usb_cmd_ex(port, buf, &cmd, pump, None)?;
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
