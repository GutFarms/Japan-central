//! CYD Companion — USB-only mining control.
//! PC owns stratum/WiFi; board only hashes work received over USB-C.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod stratum;

use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui::{
    self, Color32, FontFamily, FontId, Frame, Margin, RichText, Rounding, ScrollArea, Stroke, TextEdit,
    Vec2,
};
use eframe::{App, NativeOptions};
use serde::{Deserialize, Serialize};
use serialport::SerialPort;
use stratum::{encode_job_cmd, StratumClient};

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 780.0])
            .with_min_inner_size([900.0, 640.0])
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
            apply_theme(&cc.egui_ctx);
            Box::new(CompanionApp::new(cc.storage))
        }),
    )
}

const C_BG: Color32 = Color32::from_rgb(8, 12, 16);
const C_PANEL: Color32 = Color32::from_rgb(18, 26, 36);
const C_BUBBLE: Color32 = Color32::from_rgb(56, 84, 118);
const C_BUBBLE_HI: Color32 = Color32::from_rgb(110, 168, 220);
const C_LIME: Color32 = Color32::from_rgb(180, 240, 90);
const C_TEXT: Color32 = Color32::from_rgb(228, 238, 248);
const C_MUTED: Color32 = Color32::from_rgb(130, 150, 170);
const C_WARN: Color32 = Color32::from_rgb(255, 180, 90);
const C_ERR: Color32 = Color32::from_rgb(255, 120, 100);
const MAX_LOGS: usize = 500;

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = C_BG;
    style.visuals.window_fill = C_PANEL;
    style.visuals.extreme_bg_color = Color32::from_rgb(28, 40, 56);
    style.visuals.override_text_color = Some(C_TEXT);
    style.visuals.widgets.inactive.bg_fill = C_BUBBLE;
    style.visuals.widgets.hovered.bg_fill = C_BUBBLE_HI;
    style.visuals.widgets.active.bg_fill = C_LIME;
    style.visuals.widgets.inactive.rounding = Rounding::same(18.0);
    style.visuals.widgets.hovered.rounding = Rounding::same(18.0);
    style.visuals.widgets.active.rounding = Rounding::same(18.0);
    style.spacing.item_spacing = Vec2::new(12.0, 10.0);
    style.spacing.button_padding = Vec2::new(16.0, 10.0);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(28.0, FontFamily::Proportional),
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
    logs: VecDeque<LogEntry>,
    log_auto_scroll: bool,
    stratum_live: StratumLive,
    term_input: String,
    term_history: VecDeque<String>,
    term_out: VecDeque<String>,
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
            logs: VecDeque::new(),
            log_auto_scroll: true,
            stratum_live: StratumLive::default(),
            term_input: "cmp ping".into(),
            term_history: VecDeque::new(),
            term_out: VecDeque::new(),
        };
        app.push_log(LogKind::Info, "CYD Companion ready".into());
        app
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
        panel(ui, "USB-C", |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_source("com")
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
                if bubble(ui, "Refresh", false).clicked() {
                    let _ = self.cmd_tx.send(NetCmd::ListPorts);
                }
                if bubble(ui, "Connect", true).clicked() {
                    self.connect_usb();
                }
                if bubble(ui, "Disconnect", false).clicked() {
                    let _ = self.cmd_tx.send(NetCmd::CloseUsb);
                    self.usb_open = false;
                    self.mining = false;
                    self.push_log(LogKind::Usb, "Disconnect requested".into());
                }
            });
        });

        ui.add_space(8.0);
        panel(ui, "Pool (on this PC)", |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Stratum").color(C_MUTED));
                ui.add(
                    TextEdit::singleline(&mut self.edit_stratum)
                        .desired_width(520.0)
                        .hint_text("stratum+tcp://host:port"),
                );
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Worker").color(C_MUTED));
                ui.add(
                    TextEdit::singleline(&mut self.edit_worker)
                        .desired_width(360.0)
                        .hint_text("Bitcoin address (bc1… / 1… / 3…)"),
                );
                ui.label(RichText::new("Pass").color(C_MUTED));
                ui.add(TextEdit::singleline(&mut self.edit_password).desired_width(100.0));
            });
            ui.horizontal(|ui| {
                if !self.mining {
                    if bubble(ui, "Start mining", true).clicked() {
                        self.start_mine();
                    }
                } else if bubble(ui, "Stop mining", false).clicked() {
                    self.stop_mine();
                }
                if bubble(ui, "Bench", false).clicked() {
                    let _ = self.cmd_tx.send(NetCmd::Bench);
                    self.push_log(LogKind::Usb, "Bench requested".into());
                }
            });
        });

        ui.add_space(8.0);
        panel(ui, "Live board", |ui| {
            ui.horizontal(|ui| {
                let khs = if self.status.hashrate_khs > 0.0 {
                    self.status.hashrate_khs
                } else {
                    self.status.hashrate_hs / 1000.0
                };
                stat(ui, "Hashrate", &format!("{khs:.2} kH/s"));
                stat(ui, "H/s", &format!("{:.0}", self.status.hashrate_hs));
                stat(ui, "Accepted", &self.accepted.to_string());
                stat(ui, "Rejected", &self.rejected.to_string());
                stat(ui, "Board shares", &self.status.shares.to_string());
                stat(
                    ui,
                    "Nonce",
                    if self.status.nonce.is_empty() {
                        "—"
                    } else {
                        &self.status.nonce
                    },
                );
            });
        });

        ui.add_space(8.0);
        self.ui_stratum_panel(ui);

        ui.add_space(8.0);
        self.ui_logs_panel(ui);
    }

    fn ui_stratum_panel(&self, ui: &mut egui::Ui) {
        let s = &self.stratum_live;
        panel(ui, "Stratum communication (live)", |ui| {
            ui.horizontal(|ui| {
                let state = if s.authorized {
                    ("AUTHORIZED", C_LIME)
                } else if s.connected || s.phase != "off" && s.phase != "err" {
                    ("LINKING", C_WARN)
                } else if s.phase == "err" {
                    ("ERROR", C_ERR)
                } else {
                    ("IDLE", C_MUTED)
                };
                ui.label(
                    RichText::new(state.0)
                        .color(state.1)
                        .strong()
                        .size(15.0),
                );
                ui.label(
                    RichText::new(format!(
                        "  {}  ·  phase {}  ·  diff {:.6}",
                        if s.endpoint.is_empty() {
                            "—"
                        } else {
                            &s.endpoint
                        },
                        if s.phase.is_empty() { "off" } else { &s.phase },
                        s.difficulty
                    ))
                    .color(C_MUTED),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                mini_stat(ui, "TX lines", &s.lines_tx.to_string());
                mini_stat(ui, "RX lines", &s.lines_rx.to_string());
                mini_stat(ui, "Jobs", &s.jobs.to_string());
                mini_stat(ui, "Accept", &s.accepted.to_string());
                mini_stat(ui, "Reject", &s.rejected.to_string());
                mini_stat(
                    ui,
                    "Job id",
                    if s.last_job.is_empty() {
                        "—"
                    } else {
                        &s.last_job
                    },
                );
            });
            ui.add_space(6.0);
            ui.label(RichText::new("Last TX → pool").color(C_BUBBLE_HI).size(12.0));
            ui.label(
                RichText::new(trunc(&s.last_tx, 140))
                    .color(C_TEXT)
                    .monospace()
                    .size(12.0),
            );
            ui.label(RichText::new("Last RX ← pool").color(C_BUBBLE_HI).size(12.0));
            ui.label(
                RichText::new(trunc(&s.last_rx, 140))
                    .color(C_TEXT)
                    .monospace()
                    .size(12.0),
            );
        });
    }

    fn ui_logs_panel(&mut self, ui: &mut egui::Ui) {
        panel(ui, "Logs", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.log_auto_scroll, "Auto-scroll");
                if bubble(ui, "Clear logs", false).clicked() {
                    self.logs.clear();
                }
            });
            Frame::none()
                .fill(Color32::from_rgb(10, 14, 20))
                .rounding(Rounding::same(12.0))
                .inner_margin(Margin::same(8.0))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_source("logs_scroll")
                        .stick_to_bottom(self.log_auto_scroll)
                        .max_height(180.0)
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
                                            .monospace()
                                            .size(11.0),
                                    );
                                    ui.label(
                                        RichText::new(&e.text)
                                            .color(C_TEXT)
                                            .monospace()
                                            .size(11.0),
                                    );
                                });
                            }
                        });
                });
        });
    }

    fn ui_debug(&mut self, ui: &mut egui::Ui) {
        panel(ui, "USB terminal → ESP board", |ui| {
            ui.label(
                RichText::new("Send raw `cmp …` lines over USB-C. Examples: ping, status, config, stop, bench n=4")
                    .color(C_MUTED)
                    .size(12.0),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let resp = ui.add(
                    TextEdit::singleline(&mut self.term_input)
                        .desired_width(640.0)
                        .font(FontId::monospace(14.0))
                        .hint_text("cmp ping"),
                );
                if bubble(ui, "Send", true).clicked()
                    || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    self.send_term();
                }
                if bubble(ui, "Ping", false).clicked() {
                    self.term_input = "cmp ping".into();
                    self.send_term();
                }
                if bubble(ui, "Status", false).clicked() {
                    self.term_input = "cmp status".into();
                    self.send_term();
                }
                if bubble(ui, "Config", false).clicked() {
                    self.term_input = "cmp config".into();
                    self.send_term();
                }
            });
            ui.add_space(6.0);
            Frame::none()
                .fill(Color32::from_rgb(10, 14, 20))
                .rounding(Rounding::same(12.0))
                .inner_margin(Margin::same(8.0))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_source("term_scroll")
                        .stick_to_bottom(true)
                        .max_height(280.0)
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
                                ui.label(RichText::new(line).color(color).monospace().size(12.0));
                            }
                        });
                });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if bubble(ui, "Clear terminal", false).clicked() {
                    self.term_history.clear();
                    self.term_out.clear();
                }
            });
        });

        ui.add_space(8.0);
        self.ui_stratum_panel(ui);
        ui.add_space(8.0);
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

        if self.usb_open && self.last_poll.elapsed() > Duration::from_millis(1200) {
            let _ = self.cmd_tx.send(NetCmd::PollStatus);
            self.last_poll = Instant::now();
        }

        self.pulse = (self.pulse + ctx.input(|i| i.unstable_dt) * 1.4) % std::f32::consts::TAU;

        egui::CentralPanel::default()
            .frame(Frame::none().fill(C_BG).inner_margin(Margin::same(18.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("CYD Companion").color(C_LIME).strong());
                    ui.label(
                        RichText::new(format!("  ·  fw {}", self.fw_label)).color(C_MUTED),
                    );
                    ui.add_space(16.0);
                    ui.selectable_value(&mut self.tab, Tab::Mine, "Mine");
                    ui.selectable_value(&mut self.tab, Tab::Debug, "Debug / Terminal");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let link = if self.usb_open { "USB linked" } else { "USB idle" };
                        ui.label(
                            RichText::new(link)
                                .color(if self.usb_open { C_LIME } else { C_MUTED })
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format!("stratum: {}", self.pool_phase)).color(C_MUTED),
                        );
                    });
                });
                ui.label(
                    RichText::new("Bitcoin SHA-256 on the board · stratum pool on this PC")
                        .color(C_MUTED)
                        .size(13.0),
                );
                ui.add_space(10.0);

                match self.tab {
                    Tab::Mine => self.ui_mine(ui),
                    Tab::Debug => self.ui_debug(ui),
                }

                ui.add_space(8.0);
                if !self.last_ok.is_empty() {
                    ui.label(RichText::new(&self.last_ok).color(C_LIME).size(12.0));
                }
                if !self.last_error.is_empty() {
                    ui.label(RichText::new(&self.last_error).color(C_ERR).size(12.0));
                }
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

fn panel(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    Frame::none()
        .fill(C_PANEL)
        .rounding(Rounding::same(18.0))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            ui.label(RichText::new(title).color(C_BUBBLE_HI).strong().size(13.0));
            ui.add_space(6.0);
            add(ui);
        });
}

fn stat(ui: &mut egui::Ui, label: &str, value: &str) {
    Frame::none()
        .fill(Color32::from_rgb(24, 34, 48))
        .rounding(Rounding::same(14.0))
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(label).color(C_MUTED).size(11.0));
                ui.label(RichText::new(value).color(C_LIME).strong().size(17.0));
            });
        });
}

fn mini_stat(ui: &mut egui::Ui, label: &str, value: &str) {
    Frame::none()
        .fill(Color32::from_rgb(24, 34, 48))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(label).color(C_MUTED).size(10.0));
                ui.label(RichText::new(value).color(C_TEXT).strong().size(13.0));
            });
        });
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
                                    let _ = msg_tx.send(NetMsg::Config(parse_cmp_config(&line)));
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
                        match usb_cmd(p.as_mut(), &mut usb_rx, &warmup_job_cmd()) {
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
                                    &msg_tx,
                                );
                                let _ = msg_tx.send(NetMsg::Status(parse_cmp_status(&line)));
                            }
                            Err(e) => {
                                let _ = msg_tx.send(NetMsg::Status(Err(e)));
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
                            let cmd = encode_job_cmd(&job);
                            match usb_cmd(p.as_mut(), &mut usb_rx, &cmd) {
                                Ok(_) => {
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
                            let cmd = format!(
                                "cmp stats accepted={}&rejected={}",
                                client.accepted, client.rejected
                            );
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
                    accepted: client.accepted,
                    rejected: client.rejected,
                    phase: client.phase.clone(),
                });
                last_stratum_ui = Instant::now();
            }
        }

        if let Some(p) = usb.as_mut() {
            harvest_shares(p.as_mut(), &mut usb_rx, stratum.as_mut(), &msg_tx);
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
            match s.submit_share(&job, &en2, &ntime, &nonce) {
                Ok(()) => {
                    let _ = msg_tx.send(NetMsg::Action(Ok(format!("Share submitted {nonce}"))));
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
    let wait_ms = if cmd.contains("bench") {
        60_000
    } else if cmd.contains("job") || cmd.contains("status") {
        12_000
    } else {
        5_000
    };
    for _ in 0..3 {
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
        for chunk in line.as_bytes().chunks(64) {
            port.write_all(chunk)
                .map_err(|e| format!("USB write: {e}"))?;
            thread::sleep(Duration::from_millis(2));
        }
        port.flush().map_err(|e| format!("USB flush: {e}"))?;
        let deadline = Instant::now() + Duration::from_millis(wait_ms);
        while Instant::now() < deadline {
            drain_serial(port, buf);
            if let Some(reply) = cmp_reply_line(buf) {
                return Ok(reply);
            }
            thread::sleep(Duration::from_millis(20));
        }
        last_err = format!(
            "USB timeout waiting for reply to `{}`",
            cmd.chars().take(48).collect::<String>()
        );
        thread::sleep(Duration::from_millis(100));
    }
    Err(last_err)
}

fn warmup_job_cmd() -> String {
    use stratum::WorkJob;
    let mut header = [0u8; 80];
    header[0] = 0x01;
    header[68] = 0x5a;
    let mut target = [0xffu8; 32];
    target[31] = 0x0f;
    let job = WorkJob {
        job_id: "warmup".into(),
        header,
        target,
        extranonce2_hex: "00000000".into(),
        ntime_hex: "00000000".into(),
    };
    encode_job_cmd(&job)
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
