//! CYD Companion — USB-only mining control.
//! PC owns stratum/WiFi; board only hashes work received over USB-C.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod live_bar;
mod stratum;

use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use live_bar::{format_change, format_usd, LiveFeed};

use eframe::egui::{
    self, Align, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Layout, Margin,
    Pos2, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, TextEdit, Vec2,
};
use eframe::{App, NativeOptions};
use serde::{Deserialize, Serialize};
use serialport::SerialPort;
use stratum::{encode_job_cmd, encode_job_parts, StratumClient, WorkJob};

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Mine,
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
}

fn default_mhz() -> u8 {
    240
}

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
                }
            }
        }

        let mut app = Self {
            tab: Tab::Mine,
            com_port: String::new(),
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
        };
        app.push_log(LogKind::Info, "CYD Companion ready".into());
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
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.12);
        let pace = if self.mining || self.board_hashing() {
            2.15
        } else {
            0.75
        };
        self.pulse = (self.pulse + dt * pace) % std::f32::consts::TAU;

        let target = self.board_khs();
        let alpha = 1.0 - (-dt * 7.5).exp();
        self.displayed_khs += (target - self.displayed_khs) * alpha;
        if self.displayed_khs.abs() < 0.001 {
            self.displayed_khs = 0.0;
        }

        if self.last_hash_sample.elapsed() > Duration::from_millis(560) {
            self.hashrate_history.push_back(self.displayed_khs.max(0.0));
            while self.hashrate_history.len() > HASH_HISTORY_SAMPLES {
                self.hashrate_history.pop_front();
            }
            self.last_hash_sample = Instant::now();
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
        };
        if let Ok(raw) = serde_json::to_string(&p) {
            storage.set_string("mine_prefs", raw);
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
        self.last_ok = "Mining stopped.".into();
        self.push_log(LogKind::Info, "Mining stopped".into());
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
                                    "Board live at {:.0} H/s — nonce {} · {} hashes",
                                    self.status.hashrate_hs,
                                    if self.status.nonce.is_empty() {
                                        "—"
                                    } else {
                                        &self.status.nonce
                                    },
                                    self.status.hashes
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
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:.2}", self.displayed_khs))
                                    .color(C_TEXT)
                                    .font(display_font(72.0)),
                            );
                            ui.vertical(|ui| {
                                ui.add_space(28.0);
                                ui.label(
                                    RichText::new("kH/s")
                                        .color(C_LIME)
                                        .font(display_font(26.0)),
                                );
                            });
                        });
                        ui.add_space(10.0);
                        sparkline(ui, &self.hashrate_history, self.pulse);
                        ui.add_space(8.0);
                        hash_activity_bars(ui, self.displayed_khs, self.pulse, self.board_hashing());
                    });

                    ui.add_space(12.0);
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        let (acc, rej) = if self.stratum_live.authorized {
                            (self.accepted, self.rejected)
                        } else {
                            (0, 0)
                        };
                        paint_board_screen(
                            ui,
                            self.displayed_khs,
                            acc,
                            rej,
                            self.target_mhz,
                            self.board_hashing(),
                            self.usb_open,
                            self.pulse,
                        );
                        ui.add_space(12.0);
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
                        ui.add_space(8.0);
                        if soft_button(ui, "Bench board", 210.0).clicked() {
                            let _ = self.cmd_tx.send(NetCmd::Bench);
                            self.push_log(LogKind::Usb, "Bench requested".into());
                        }
                    });
                });
            });
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
            let (acc, rej) = if authed {
                (self.accepted, self.rejected)
            } else {
                // Hide handshake / pre-auth noise (often shows a couple rejects).
                (0, 0)
            };
            ui.horizontal_wrapped(|ui| {
                metric(ui, "Accepted", &acc.to_string(), C_LIME);
                metric(
                    ui,
                    "Rejected",
                    &rej.to_string(),
                    if rej > 0 { C_ERR } else { C_MUTED },
                );
                metric(ui, "Board shares", &self.status.shares.to_string(), C_TEXT);
                metric(ui, "Raw H/s", &format!("{:.0}", self.status.hashrate_hs), C_TEXT);
                metric(ui, "Hashes", &self.status.hashes.to_string(), C_BUBBLE_HI);
                metric(
                    ui,
                    "Nonce",
                    if self.status.nonce.is_empty() {
                        "—"
                    } else {
                        &self.status.nonce
                    },
                    C_TEXT,
                );
            });
            if !authed {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Share counts start after pool authorize")
                        .color(C_DIM)
                        .font(mono_ui_font(11.0)),
                );
            }
            ui.add_space(12.0);
            telemetry_line(
                ui,
                "Firmware",
                if self.fw_label.is_empty() { "—" } else { &self.fw_label },
            );
            telemetry_line(
                ui,
                "Nonce",
                if self.status.nonce.is_empty() {
                    "—"
                } else {
                    &self.status.nonce
                },
            );
            telemetry_line(
                ui,
                "Job",
                if self.status.job.is_empty() {
                    "—"
                } else {
                    &self.status.job
                },
            );
            telemetry_line(
                ui,
                "Uptime",
                &format!("{}s · {} MHz", self.status.uptime_secs, self.target_mhz),
            );
            ui.add_space(10.0);
            if !self.last_ok.is_empty() {
                ui.label(RichText::new(&self.last_ok).color(C_LIME).size(12.0));
            }
            if !self.last_error.is_empty() {
                ui.label(RichText::new(&self.last_error).color(C_ERR).size(12.0));
            }
        });
    }

    fn ui_stratum_panel(&self, ui: &mut egui::Ui) {
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
                mini_stat(
                    ui,
                    "Job id",
                    if s.last_job.is_empty() { "—" } else { &s.last_job },
                );
            });
            ui.add_space(8.0);
            stratum_line(ui, "Last TX → pool", &trunc(&s.last_tx, 150));
            stratum_line(ui, "Last RX ← pool", &trunc(&s.last_rx, 150));
        });
    }

    fn ui_logs_panel(&mut self, ui: &mut egui::Ui) {
        soft_panel(ui, "Event log", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.log_auto_scroll, "Auto-scroll");
                if soft_button(ui, "Clear logs", 110.0).clicked() {
                    self.logs.clear();
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
                    self.accepted = accepted;
                    self.rejected = rejected;
                    self.pool_phase = phase;
                }
                NetMsg::Stratum(live) => {
                    self.stratum_live = live;
                    self.accepted = self.stratum_live.accepted;
                    self.rejected = self.stratum_live.rejected;
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
            }
        }

        if self.usb_open && self.last_poll.elapsed() > Duration::from_millis(800) {
            let _ = self.cmd_tx.send(NetCmd::PollStatus);
            self.last_poll = Instant::now();
        }

        self.update_motion(ctx);
        self.live.poll();

        egui::TopBottomPanel::bottom("live_ticker_bar")
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
                paint_background(ui, ui.max_rect(), self.pulse, self.mining || self.board_hashing());
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
                            Tab::Debug => self.ui_debug(ui),
                        }
                        ui.add_space(28.0);
                    });
            });

        ctx.request_repaint_after(Duration::from_millis(40));
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

fn paint_background(ui: &mut egui::Ui, rect: Rect, pulse: f32, mining: bool) {
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

    let step = 34.0;
    let drift = (pulse * 18.0) % step;
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

    // Slow diagonal sweep — presence, not noise.
    let sweep = (pulse * 0.12).fract();
    let x = rect.left() + rect.width() * sweep;
    painter.line_segment(
        [
            Pos2::new(x, rect.bottom() - 18.0),
            Pos2::new(x + rect.height() * 0.55, rect.top() + 18.0),
        ],
        Stroke::new(
            2.0_f32,
            Color32::from_rgba_unmultiplied(198, 255, 64, if mining { 28 } else { 12 }),
        ),
    );
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

fn paint_board_screen(
    ui: &mut egui::Ui,
    khs: f32,
    accepted: u32,
    rejected: u32,
    mhz: u8,
    hashing: bool,
    usb: bool,
    pulse: f32,
) {
    let desired = Vec2::new(248.0, 186.0);
    let (outer, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter_at(outer);

    // Thin device bezel — screen is the product.
    painter.rect_filled(outer, Rounding::same(14.0), Color32::from_rgb(16, 22, 20));
    painter.rect_stroke(
        outer,
        Rounding::same(14.0),
        Stroke::new(1.0_f32, Color32::from_rgb(48, 68, 62)),
    );
    let screen = outer.shrink2(Vec2::new(12.0, 14.0));
    painter.rect_filled(screen, Rounding::same(4.0), Color32::from_rgb(5, 8, 10));

    // Companion-like chrome: lime rule + left accent.
    painter.rect_filled(
        Rect::from_min_size(screen.min, Vec2::new(screen.width(), 3.0)),
        Rounding::ZERO,
        C_LIME,
    );
    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(screen.left(), screen.top() + 8.0),
            Vec2::new(4.0, screen.height() - 20.0),
        ),
        Rounding::ZERO,
        C_LIME,
    );

    let breath = 0.5 + 0.5 * pulse.sin();
    painter.text(
        Pos2::new(screen.left() + 14.0, screen.top() + 12.0),
        egui::Align2::LEFT_TOP,
        "CYD",
        display_font(20.0),
        C_LIME,
    );
    painter.text(
        Pos2::new(screen.left() + 58.0, screen.top() + 18.0),
        egui::Align2::LEFT_TOP,
        "Companion · SHA-256",
        mono_ui_font(10.0),
        C_MUTED,
    );
    painter.text(
        Pos2::new(screen.left() + 14.0, screen.top() + 42.0),
        egui::Align2::LEFT_TOP,
        format!("{khs:.2}"),
        display_font(32.0),
        C_TEXT,
    );
    painter.text(
        Pos2::new(screen.left() + 128.0, screen.top() + 58.0),
        egui::Align2::LEFT_TOP,
        "kH/s",
        mono_ui_font(12.0),
        C_LIME,
    );

    // Activity bars (basic Companion strip).
    let bar_area = Rect::from_min_size(
        Pos2::new(screen.left() + 14.0, screen.top() + 92.0),
        Vec2::new(screen.width() - 28.0, 14.0),
    );
    let n = 14;
    let gap = 2.0;
    let bw = ((bar_area.width() - gap * (n as f32 - 1.0)) / n as f32).max(3.0);
    for i in 0..n {
        let phase = pulse * 2.2 + i as f32 * 0.4;
        let breathe = 0.4 + 0.6 * (0.5 + 0.5 * phase.sin());
        let level = if hashing {
            ((khs / 80.0).clamp(0.1, 1.0) * breathe).clamp(0.14, 1.0)
        } else {
            0.1 + 0.05 * (0.5 + 0.5 * (pulse + i as f32 * 0.25).sin())
        };
        let h = bar_area.height() * level;
        let x = bar_area.left() + i as f32 * (bw + gap);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(x, bar_area.bottom() - h),
                Pos2::new(x + bw, bar_area.bottom()),
            ),
            Rounding::same(1.0),
            if hashing {
                rgba(C_LIME, (90.0 + level * 120.0) as u8)
            } else {
                rgba(C_BUBBLE_HI, 55)
            },
        );
    }

    painter.text(
        Pos2::new(screen.left() + 14.0, screen.top() + 114.0),
        egui::Align2::LEFT_TOP,
        if hashing {
            "HASHING"
        } else if usb {
            "USB READY"
        } else {
            "WAIT USB"
        },
        mono_ui_font(11.0),
        if hashing { C_LIME } else { C_MUTED },
    );

    // Flat accept / reject / clock — no cards.
    painter.text(
        Pos2::new(screen.left() + 14.0, screen.top() + 136.0),
        egui::Align2::LEFT_TOP,
        "ACCEPT",
        mono_ui_font(9.0),
        C_MUTED,
    );
    painter.text(
        Pos2::new(screen.left() + 90.0, screen.top() + 136.0),
        egui::Align2::LEFT_TOP,
        "REJECT",
        mono_ui_font(9.0),
        C_MUTED,
    );
    painter.text(
        Pos2::new(screen.left() + 166.0, screen.top() + 136.0),
        egui::Align2::LEFT_TOP,
        "CLOCK",
        mono_ui_font(9.0),
        C_MUTED,
    );
    painter.text(
        Pos2::new(screen.left() + 14.0, screen.top() + 150.0),
        egui::Align2::LEFT_TOP,
        format!("{accepted}"),
        mono_ui_font(14.0),
        C_LIME,
    );
    painter.text(
        Pos2::new(screen.left() + 90.0, screen.top() + 150.0),
        egui::Align2::LEFT_TOP,
        format!("{rejected}"),
        mono_ui_font(14.0),
        if rejected > 0 { C_ERR } else { C_TEXT },
    );
    painter.text(
        Pos2::new(screen.left() + 166.0, screen.top() + 150.0),
        egui::Align2::LEFT_TOP,
        format!("{mhz} MHz"),
        mono_ui_font(14.0),
        C_TEXT,
    );

    let pip = Pos2::new(screen.right() - 16.0, screen.top() + 20.0);
    painter.circle_filled(
        pip,
        4.0,
        if hashing {
            rgba(C_LIME, (140.0 + breath * 100.0) as u8)
        } else {
            rgba(C_MUTED, 120)
        },
    );
}

fn sparkline(ui: &mut egui::Ui, values: &VecDeque<f32>, pulse: f32) {
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
    let points: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = inner.left() + inner.width() * i as f32 / (len - 1) as f32;
            let y = inner.bottom() - inner.height() * (v.max(0.0) / max).clamp(0.0, 1.0);
            Pos2::new(x, y)
        })
        .collect();
    for pair in points.windows(2) {
        painter.line_segment([pair[0], pair[1]], Stroke::new(6.0_f32, rgba(C_LIME, 22)));
        painter.line_segment([pair[0], pair[1]], Stroke::new(2.25_f32, C_LIME));
    }

    let scan_t = (pulse * 0.16).fract();
    let scan_x = inner.left() + inner.width() * scan_t;
    painter.line_segment(
        [Pos2::new(scan_x, inner.top()), Pos2::new(scan_x, inner.bottom())],
        Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(198, 255, 64, 80),
        ),
    );
    if let Some(last) = points.last() {
        painter.circle_filled(*last, 4.5, C_LIME);
        painter.circle_filled(*last, 10.0 + pulse.sin().max(0.0) * 4.0, rgba(C_LIME, 28));
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
        .rounding(Rounding::same(14.0))
        .inner_margin(Margin::symmetric(11.0, 7.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(label).color(C_DIM).font(mono_ui_font(10.0)));
                ui.label(RichText::new(value).color(C_TEXT).size(13.0));
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
                    if let Some(p) = usb.as_mut() {
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
                            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                "Pool connecting {endpoint} — board already hashing"
                            ))));
                        }
                        Err(e) => {
                            mining = true;
                            let _ = msg_tx.send(NetMsg::Action(Err(format!(
                                "Pool error (board still hashing locally): {e}"
                            ))));
                        }
                    }
                }
                NetCmd::StopMine => {
                    mining = false;
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
            }
        }

        // Constant stratum communication updates while linked / mining.
        if let Some(client) = stratum.as_mut() {
            match client.poll() {
                Ok(()) => {
                    for line in client.take_recent() {
                        log_msg(&msg_tx, LogKind::Stratum, line);
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
                            // Only push share tallies after authorize — avoids board showing
                            // handshake rejects from the previous moment.
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
                    thread::sleep(Duration::from_millis(400));
                }
            }
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
