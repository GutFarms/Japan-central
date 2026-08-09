//! CYD Companion — GPU desktop app (LAN + USB) for the ESP32-2432S028 miner.
//!
//! ```text
//! cargo run --no-default-features --features companion --bin cyd-companion --release
//! cargo build --no-default-features --features companion --bin cyd-companion \
//!   --release --target x86_64-pc-windows-gnu
//! ```

// Hide the Windows console when the GUI starts.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, FontFamily, FontId, Frame, Layout, Margin, RichText, Rounding, Sense,
    Stroke, Vec2,
};
use eframe::{App, NativeOptions};
use serde::Deserialize;
use serialport::SerialPort;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([880.0, 600.0])
            .with_title("CYD Companion · Scrypt Miner Control"),
        multisampling: 8,
        depth_buffer: 0,
        ..Default::default()
    };
    eframe::run_native(
        "CYD Companion",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            apply_theme(&cc.egui_ctx);
            Box::new(CompanionApp::new())
        }),
    )
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = Color32::from_rgb(12, 14, 16);
    style.visuals.window_fill = Color32::from_rgb(18, 20, 22);
    style.visuals.extreme_bg_color = Color32::from_rgb(8, 9, 10);
    style.visuals.faint_bg_color = Color32::from_rgb(28, 24, 20);
    style.visuals.override_text_color = Some(Color32::from_rgb(236, 228, 214));
    style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(22, 24, 26);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(36, 30, 24);
    style.visuals.widgets.inactive.fg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(200, 170, 130));
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(70, 48, 28);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(196, 92, 28);
    style.visuals.selection.bg_fill = Color32::from_rgb(180, 80, 24);
    style.visuals.widgets.inactive.rounding = Rounding::same(6.0);
    style.visuals.widgets.hovered.rounding = Rounding::same(6.0);
    style.visuals.widgets.active.rounding = Rounding::same(6.0);
    style.spacing.item_spacing = Vec2::new(12.0, 10.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(28.0, FontFamily::Proportional),
    );
    ctx.set_style(style);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Wifi,
    Pool,
    Overclock,
    Discover,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Transport {
    Usb,
    Lan,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)] // JSON fields retained for forward-compatible deserialization
struct StatusJson {
    #[serde(default)]
    hashrate_hs: f64,
    #[serde(default)]
    shares: u64,
    #[serde(default)]
    accepted: u32,
    #[serde(default)]
    rejected: u32,
    #[serde(default)]
    dropped: u32,
    #[serde(default)]
    pool: String,
    #[serde(default)]
    connected: bool,
    #[serde(default)]
    wifi: String,
    #[serde(default)]
    ip: Option<String>,
    #[serde(default)]
    address: String,
    #[serde(default)]
    stratum: String,
    #[serde(default)]
    difficulty: u32,
    #[serde(default)]
    uptime_secs: u64,
    #[serde(default)]
    cpu_mhz: u8,
    #[serde(default)]
    nonce: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)] // JSON fields retained for forward-compatible deserialization
struct ConfigJson {
    #[serde(default)]
    worker: String,
    #[serde(default)]
    stratum: String,
    #[serde(default)]
    wifi_ssid: String,
    #[serde(default)]
    wifi_password: String,
    #[serde(default)]
    cpu_mhz: u8,
    #[serde(default)]
    fw: String,
}

enum NetMsg {
    Status(Result<StatusJson, String>),
    Config(Result<ConfigJson, String>),
    Action(Result<String, String>),
    Probe(Result<String, String>),
    Ports(Vec<String>),
}

enum NetCmd {
    ListPorts,
    OpenUsb(String),
    CloseUsb,
    PollStatus { transport: Transport, base: String },
    FetchConfig { transport: Transport, base: String },
    Post {
        transport: Transport,
        base: String,
        path: String,
        body: String,
    },
    Probe(String),
    UsbPing,
}

struct CompanionApp {
    tab: Tab,
    transport: Transport,
    board_ip: String,
    com_port: String,
    ports: Vec<String>,
    auth_password: String,
    status: StatusJson,
    last_error: String,
    last_ok: String,
    connected_ui: bool,
    edit_worker: String,
    edit_stratum: String,
    edit_password: String,
    edit_wifi_ssid: String,
    edit_wifi_password: String,
    target_mhz: u8,
    discover_base: String,
    discover_log: String,
    cmd_tx: Sender<NetCmd>,
    msg_rx: Receiver<NetMsg>,
    last_poll: Instant,
    pulse: f32,
    fw_label: String,
}

impl CompanionApp {
    fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<NetCmd>();
        let (msg_tx, msg_rx) = mpsc::channel::<NetMsg>();
        thread::spawn(move || net_worker(cmd_rx, msg_tx));
        let _ = cmd_tx.send(NetCmd::ListPorts);
        Self {
            tab: Tab::Dashboard,
            transport: Transport::Usb,
            board_ip: "192.168.1.50".into(),
            com_port: String::new(),
            ports: Vec::new(),
            auth_password: "x".into(),
            status: StatusJson::default(),
            last_error: String::new(),
            last_ok: "Pick a USB COM port (CH340) or switch to LAN.".into(),
            connected_ui: false,
            edit_worker: String::new(),
            edit_stratum: "stratum+tcp://ltc.viabtc.io:3333".into(),
            edit_password: "x".into(),
            edit_wifi_ssid: String::new(),
            edit_wifi_password: String::new(),
            target_mhz: 240,
            discover_base: "192.168.1".into(),
            discover_log: String::new(),
            cmd_tx,
            msg_rx,
            last_poll: Instant::now() - Duration::from_secs(10),
            pulse: 0.0,
            fw_label: "—".into(),
        }
    }

    fn base_url(&self) -> String {
        let ip = self.board_ip.trim().trim_start_matches("http://");
        format!("http://{ip}")
    }

    fn endpoint(&self) -> String {
        match self.transport {
            Transport::Usb => self.com_port.clone(),
            Transport::Lan => self.base_url(),
        }
    }

    fn connect(&mut self) {
        self.last_error.clear();
        match self.transport {
            Transport::Usb => {
                if self.com_port.is_empty() {
                    self.last_error = "Select a COM port first.".into();
                    return;
                }
                let _ = self.cmd_tx.send(NetCmd::OpenUsb(self.com_port.clone()));
                self.last_ok = format!("Opening USB {} @ 115200…", self.com_port);
            }
            Transport::Lan => {
                self.connected_ui = true;
                self.last_ok = format!("Polling {} …", self.base_url());
                self.refresh_board();
            }
        }
    }

    fn refresh_board(&mut self) {
        let base = self.endpoint();
        let t = self.transport;
        let _ = self.cmd_tx.send(NetCmd::FetchConfig {
            transport: t,
            base: base.clone(),
        });
        let _ = self.cmd_tx.send(NetCmd::PollStatus {
            transport: t,
            base,
        });
    }

    fn post(&mut self, path: &str, body: String) {
        let _ = self.cmd_tx.send(NetCmd::Post {
            transport: self.transport,
            base: self.endpoint(),
            path: path.into(),
            body,
        });
    }

    fn apply_wifi(&mut self) {
        let mut body = format!(
            "auth={}&wifi_ssid={}",
            urlenc(&self.auth_password),
            urlenc(&self.edit_wifi_ssid),
        );
        if !self.edit_wifi_password.is_empty() {
            body.push_str(&format!(
                "&wifi_password={}",
                urlenc(&self.edit_wifi_password)
            ));
        }
        self.post("/api/config", body);
        self.last_ok = "WiFi sent — SSID/password changes reboot the board.".into();
    }

    fn apply_pool(&mut self) {
        let mut body = format!(
            "auth={}&worker={}&stratum={}&reconnect=true",
            urlenc(&self.auth_password),
            urlenc(&self.edit_worker),
            urlenc(&self.edit_stratum),
        );
        if !self.edit_password.is_empty() {
            body.push_str(&format!("&password={}", urlenc(&self.edit_password)));
        }
        self.post("/api/config", body);
        self.last_ok = "Pool settings sent — stratum reloads without reboot.".into();
    }

    fn apply_clock(&mut self) {
        let body = format!(
            "auth={}&cpu_mhz={}&reboot=true",
            urlenc(&self.auth_password),
            self.target_mhz
        );
        self.post("/api/clock", body);
        self.last_ok = format!("CPU {} MHz sent — board soft-resets.", self.target_mhz);
    }

    fn drain_net(&mut self) {
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
                NetMsg::Status(Ok(s)) => {
                    self.status = s;
                    self.connected_ui = true;
                    self.last_error.clear();
                    if self.target_mhz == 0 {
                        self.target_mhz = self.status.cpu_mhz.max(80);
                    }
                }
                NetMsg::Status(Err(e)) => {
                    self.last_error = e;
                    self.connected_ui = false;
                }
                NetMsg::Config(Ok(c)) => {
                    self.edit_worker = c.worker;
                    self.edit_stratum = c.stratum;
                    self.edit_wifi_ssid = c.wifi_ssid;
                    if c.cpu_mhz != 0 {
                        self.target_mhz = c.cpu_mhz;
                    }
                    if !c.fw.is_empty() {
                        self.fw_label = c.fw;
                    }
                    self.connected_ui = true;
                    self.last_ok = "Config loaded from board.".into();
                }
                NetMsg::Config(Err(e)) => self.last_error = e,
                NetMsg::Action(Ok(s)) => {
                    self.last_ok = s;
                    self.connected_ui = true;
                }
                NetMsg::Action(Err(e)) => self.last_error = e,
                NetMsg::Probe(Ok(s)) => {
                    if self.discover_log.len() > 4000 {
                        self.discover_log.clear();
                    }
                    self.discover_log.push_str(&s);
                    self.discover_log.push('\n');
                }
                NetMsg::Probe(Err(e)) => {
                    self.discover_log.push_str(&format!("fail: {e}\n"));
                }
            }
        }
    }
}

impl App for CompanionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_net();
        self.pulse = (self.pulse + ctx.input(|i| i.unstable_dt) * 1.4) % 6.2832;

        if self.connected_ui && self.last_poll.elapsed() >= Duration::from_millis(1200) {
            let _ = self.cmd_tx.send(NetCmd::PollStatus {
                transport: self.transport,
                base: self.endpoint(),
            });
            self.last_poll = Instant::now();
        }

        egui::TopBottomPanel::top("hero")
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(16, 18, 20))
                    .inner_margin(Margin::symmetric(22.0, 16.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 40, 24))),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("CYD COMPANION")
                                .color(Color32::from_rgb(255, 140, 48))
                                .size(30.0)
                                .strong(),
                        );
                        ui.label(
                            RichText::new("ESP32-2432S028 · USB serial or LAN · OpenGL")
                                .color(Color32::from_rgb(140, 150, 140))
                                .size(13.0),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let glow = ((self.pulse.sin() * 0.5 + 0.5) * 40.0) as u8;
                        let chip = if self.connected_ui {
                            (
                                format!("LIVE · {} MHz", self.status.cpu_mhz.max(1)),
                                Color32::from_rgb(40 + glow / 2, 180, 90),
                            )
                        } else {
                            ("OFFLINE".into(), Color32::from_rgb(160, 70, 60))
                        };
                        ui.label(RichText::new(chip.0).color(chip.1).strong().size(15.0));
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(format!("fw {}", self.fw_label))
                                .color(Color32::from_rgb(120, 120, 110))
                                .monospace(),
                        );
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Link");
                    ui.selectable_value(&mut self.transport, Transport::Usb, "USB");
                    ui.selectable_value(&mut self.transport, Transport::Lan, "LAN");
                    ui.separator();
                    match self.transport {
                        Transport::Usb => {
                            ui.label("Port");
                            egui::ComboBox::from_id_source("com_ports")
                                .selected_text(if self.com_port.is_empty() {
                                    "Select COM…"
                                } else {
                                    self.com_port.as_str()
                                })
                                .width(140.0)
                                .show_ui(ui, |ui| {
                                    for p in &self.ports.clone() {
                                        ui.selectable_value(&mut self.com_port, p.clone(), p);
                                    }
                                });
                            if ui.button("Refresh").clicked() {
                                let _ = self.cmd_tx.send(NetCmd::ListPorts);
                            }
                        }
                        Transport::Lan => {
                            ui.label("Board IP");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.board_ip)
                                    .desired_width(160.0)
                                    .hint_text("192.168.x.x"),
                            );
                        }
                    }
                    ui.label("Auth");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.auth_password)
                            .desired_width(100.0)
                            .password(true),
                    );
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Connect").strong())
                                .min_size(Vec2::new(100.0, 32.0)),
                        )
                        .clicked()
                    {
                        self.connect();
                    }
                    if ui.button("Disconnect").clicked() {
                        let _ = self.cmd_tx.send(NetCmd::CloseUsb);
                        self.connected_ui = false;
                        self.last_ok = "Disconnected.".into();
                    }
                    if ui.button("Reconnect pool").clicked() {
                        self.post("/api/reconnect", String::new());
                    }
                    if ui.button("Reboot").clicked() {
                        let body = format!("auth={}&reboot=true", urlenc(&self.auth_password));
                        self.post("/api/reboot", body);
                    }
                });
            });

        egui::SidePanel::left("tabs")
            .exact_width(168.0)
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(14, 15, 16))
                    .inner_margin(Margin::symmetric(12.0, 16.0)),
            )
            .show(ctx, |ui| {
                for (tab, label) in [
                    (Tab::Dashboard, "Dashboard"),
                    (Tab::Wifi, "WiFi"),
                    (Tab::Pool, "Pool"),
                    (Tab::Overclock, "Overclock"),
                    (Tab::Discover, "Discover"),
                ] {
                    let selected = self.tab == tab;
                    let fill = if selected {
                        Color32::from_rgb(196, 92, 28)
                    } else {
                        Color32::from_rgb(28, 30, 32)
                    };
                    let text = if selected {
                        Color32::from_rgb(255, 245, 230)
                    } else {
                        Color32::from_rgb(190, 180, 160)
                    };
                    let btn = egui::Button::new(RichText::new(label).color(text).size(15.0))
                        .fill(fill)
                        .min_size(Vec2::new(140.0, 40.0))
                        .rounding(Rounding::same(8.0));
                    if ui.add(btn).clicked() {
                        self.tab = tab;
                    }
                    ui.add_space(6.0);
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("USB: CH340 @ 115200\nAuth = pool password")
                            .small()
                            .color(Color32::from_rgb(100, 100, 96)),
                    );
                });
            });

        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(10, 11, 12))
                    .inner_margin(Margin::symmetric(20.0, 16.0)),
            )
            .show(ctx, |ui| {
                if !self.last_error.is_empty() {
                    ui.colored_label(Color32::from_rgb(230, 90, 70), &self.last_error);
                    ui.add_space(6.0);
                }
                if !self.last_ok.is_empty() {
                    ui.colored_label(Color32::from_rgb(120, 200, 140), &self.last_ok);
                    ui.add_space(8.0);
                }
                match self.tab {
                    Tab::Dashboard => self.ui_dashboard(ui),
                    Tab::Wifi => self.ui_wifi(ui),
                    Tab::Pool => self.ui_pool(ui),
                    Tab::Overclock => self.ui_overclock(ui),
                    Tab::Discover => self.ui_discover(ui),
                }
            });

        ctx.request_repaint_after(Duration::from_millis(33));
    }
}

impl CompanionApp {
    fn card(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
        Frame::none()
            .fill(Color32::from_rgb(20, 22, 24))
            .rounding(Rounding::same(12.0))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 40, 32)))
            .inner_margin(Margin::same(14.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(title)
                        .color(Color32::from_rgb(160, 140, 110))
                        .size(12.0)
                        .strong(),
                );
                ui.add_space(6.0);
                add(ui);
            });
    }

    fn ui_dashboard(&mut self, ui: &mut egui::Ui) {
        let rate = self.status.hashrate_hs;
        ui.horizontal(|ui| {
            Self::card(ui, "HASHRATE", |ui| {
                ui.label(
                    RichText::new(format!("{rate:.2}"))
                        .size(42.0)
                        .color(Color32::from_rgb(255, 140, 48))
                        .strong(),
                );
                ui.label(RichText::new("H/s scrypt").color(Color32::from_rgb(140, 140, 130)));
            });
            ui.add_space(12.0);
            Self::card(ui, "POOL", |ui| {
                let c = if self.status.connected {
                    Color32::from_rgb(90, 220, 130)
                } else {
                    Color32::from_rgb(220, 120, 90)
                };
                ui.label(RichText::new(&self.status.pool).color(c).size(22.0).strong());
                ui.label(format!(
                    "acc {} · rej {} · drop {}",
                    self.status.accepted, self.status.rejected, self.status.dropped
                ));
            });
            ui.add_space(12.0);
            Self::card(ui, "CPU", |ui| {
                ui.label(
                    RichText::new(format!("{} MHz", self.status.cpu_mhz))
                        .size(28.0)
                        .color(Color32::from_rgb(230, 200, 140))
                        .strong(),
                );
                ui.label(match self.transport {
                    Transport::Usb => "link: USB",
                    Transport::Lan => "link: LAN",
                });
            });
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            Self::card(ui, "WORKER", |ui| {
                ui.label(RichText::new(&self.status.address).monospace().size(14.0));
                ui.label(RichText::new(&self.status.stratum).small().color(Color32::GRAY));
            });
            ui.add_space(12.0);
            Self::card(ui, "WIFI", |ui| {
                ui.label(format!("{} · {:?}", self.status.wifi, self.status.ip));
                ui.label(format!(
                    "shares {} · diff {} · nonce {}",
                    self.status.shares, self.status.difficulty, self.status.nonce
                ));
            });
        });
        ui.add_space(18.0);
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 9.0, Color32::from_rgb(28, 24, 20));
        let frac = ((rate / 25.0) as f32).clamp(0.05, 1.0);
        let mut fill = rect;
        fill.set_width(rect.width() * frac);
        let wave = (self.pulse.sin() * 0.5 + 0.5) * 20.0;
        painter.rect_filled(fill, 9.0, Color32::from_rgb(200, 90 + wave as u8, 30));
    }

    fn ui_wifi(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("WiFi")
                .size(22.0)
                .color(Color32::from_rgb(255, 160, 70)),
        );
        ui.label("Station credentials only. Changing SSID or password soft-resets the board.");
        ui.add_space(10.0);
        Self::card(ui, "LINK STATUS", |ui| {
            ui.label(format!(
                "{} · IP {:?}",
                self.status.wifi, self.status.ip
            ));
        });
        ui.add_space(12.0);
        egui::Grid::new("wifi_grid")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label("SSID");
                ui.add(egui::TextEdit::singleline(&mut self.edit_wifi_ssid).desired_width(360.0));
                ui.end_row();
                ui.label("Password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.edit_wifi_password)
                        .desired_width(360.0)
                        .password(true)
                        .hint_text("leave blank to keep current"),
                );
                ui.end_row();
            });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Save WiFi").strong())
                        .fill(Color32::from_rgb(196, 92, 28))
                        .min_size(Vec2::new(140.0, 36.0)),
                )
                .clicked()
            {
                self.apply_wifi();
            }
            if ui.button("Reload from board").clicked() {
                self.refresh_board();
            }
        });
    }

    fn ui_pool(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Pool")
                .size(22.0)
                .color(Color32::from_rgb(255, 160, 70)),
        );
        ui.label("Stratum endpoint and worker. Auth for writes is the current pool password.");
        ui.add_space(10.0);
        Self::card(ui, "POOL STATUS", |ui| {
            let c = if self.status.connected {
                Color32::from_rgb(90, 220, 130)
            } else {
                Color32::from_rgb(220, 120, 90)
            };
            ui.label(RichText::new(&self.status.pool).color(c).size(18.0).strong());
            ui.label(format!(
                "acc {} · rej {} · drop {} · diff {}",
                self.status.accepted,
                self.status.rejected,
                self.status.dropped,
                self.status.difficulty
            ));
        });
        ui.add_space(12.0);
        egui::Grid::new("pool_grid")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label("Stratum");
                ui.add(egui::TextEdit::singleline(&mut self.edit_stratum).desired_width(360.0));
                ui.end_row();
                ui.label("Worker");
                ui.add(egui::TextEdit::singleline(&mut self.edit_worker).desired_width(360.0));
                ui.end_row();
                ui.label("Pool password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.edit_password)
                        .desired_width(360.0)
                        .password(true)
                        .hint_text("leave blank to keep current"),
                );
                ui.end_row();
            });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Save pool").strong())
                        .fill(Color32::from_rgb(196, 92, 28))
                        .min_size(Vec2::new(140.0, 36.0)),
                )
                .clicked()
            {
                self.apply_pool();
            }
            if ui.button("Reload from board").clicked() {
                self.refresh_board();
            }
        });
    }

    fn ui_overclock(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("CPU clock")
                .size(22.0)
                .color(Color32::from_rgb(255, 160, 70)),
        );
        ui.label("80 / 160 / 240 MHz. Applied on soft-reset (USB or LAN).");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            for mhz in [80_u8, 160, 240] {
                let selected = self.target_mhz == mhz;
                let label = match mhz {
                    80 => "80 MHz\nEfficient",
                    160 => "160 MHz\nBalanced",
                    _ => "240 MHz\nMax / OC",
                };
                let fill = if selected {
                    Color32::from_rgb(196, 92, 28)
                } else {
                    Color32::from_rgb(32, 28, 24)
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(label).size(15.0))
                            .fill(fill)
                            .min_size(Vec2::new(140.0, 72.0))
                            .rounding(Rounding::same(10.0)),
                    )
                    .clicked()
                {
                    self.target_mhz = mhz;
                }
            }
        });
        ui.add_space(16.0);
        Self::card(ui, "ACTIVE", |ui| {
            ui.label(format!(
                "Running: {} MHz · Target: {} MHz",
                self.status.cpu_mhz, self.target_mhz
            ));
        });
        ui.add_space(12.0);
        if ui
            .add(
                egui::Button::new(RichText::new("Apply clock & reboot").strong())
                    .fill(Color32::from_rgb(180, 60, 30))
                    .min_size(Vec2::new(200.0, 40.0)),
            )
            .clicked()
        {
            self.apply_clock();
        }
    }

    fn ui_discover(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Discover")
                .size(22.0)
                .color(Color32::from_rgb(255, 160, 70)),
        );
        ui.label("LAN /probe scan, or USB ping on the open COM port.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("USB ping").clicked() {
                let _ = self.cmd_tx.send(NetCmd::UsbPing);
            }
            ui.label("Subnet");
            ui.add(
                egui::TextEdit::singleline(&mut self.discover_base)
                    .desired_width(140.0)
                    .hint_text("192.168.1"),
            );
            if ui.button("Scan LAN .1–.254").clicked() {
                self.discover_log.clear();
                let base = self.discover_base.trim().to_string();
                for i in 1..=254u16 {
                    let _ = self
                        .cmd_tx
                        .send(NetCmd::Probe(format!("http://{base}.{i}")));
                }
                self.last_ok = "LAN scan queued…".into();
            }
            if ui.button("Probe current IP").clicked() {
                let _ = self.cmd_tx.send(NetCmd::Probe(self.base_url()));
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.discover_log)
                    .desired_width(f32::INFINITY)
                    .font(FontId::new(12.5, FontFamily::Monospace)),
            );
        });
    }
}

fn net_worker(cmd_rx: Receiver<NetCmd>, msg_tx: Sender<NetMsg>) {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(600))
        .timeout_read(Duration::from_millis(1500))
        .build();
    let mut usb: Option<Box<dyn SerialPort>> = None;
    let mut usb_rx = String::new();

    while let Ok(cmd) = cmd_rx.recv() {
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
                match serialport::new(&name, 115_200)
                    .timeout(Duration::from_millis(80))
                    .open()
                {
                    Ok(mut port) => {
                        let _ = port.clear(serialport::ClearBuffer::All);
                        // Wake / identify
                        let _ = port.write_all(b"\r\ncmp ping\r\n");
                        let _ = port.flush();
                        thread::sleep(Duration::from_millis(120));
                        drain_serial(port.as_mut(), &mut usb_rx);
                        usb = Some(port);
                        let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                            "USB open {name} @ 115200"
                        ))));
                        // Auto fetch
                        if let Some(p) = usb.as_mut() {
                            match usb_cmd(p.as_mut(), &mut usb_rx, "cmp config") {
                                Ok(line) => {
                                    let _ = msg_tx.send(NetMsg::Config(parse_cmp_config(&line)));
                                }
                                Err(e) => {
                                    let _ = msg_tx.send(NetMsg::Config(Err(e)));
                                }
                            }
                            match usb_cmd(p.as_mut(), &mut usb_rx, "cmp status") {
                                Ok(line) => {
                                    let _ = msg_tx.send(NetMsg::Status(parse_cmp_status(&line)));
                                }
                                Err(e) => {
                                    let _ = msg_tx.send(NetMsg::Status(Err(e)));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = msg_tx.send(NetMsg::Action(Err(format!("USB open failed: {e}"))));
                    }
                }
            }
            NetCmd::CloseUsb => {
                usb = None;
                usb_rx.clear();
            }
            NetCmd::UsbPing => {
                if let Some(p) = usb.as_mut() {
                    match usb_cmd(p.as_mut(), &mut usb_rx, "cmp ping") {
                        Ok(line) => {
                            let _ = msg_tx.send(NetMsg::Probe(Ok(format!("USB → {line}"))));
                        }
                        Err(e) => {
                            let _ = msg_tx.send(NetMsg::Probe(Err(e)));
                        }
                    }
                } else {
                    let _ = msg_tx.send(NetMsg::Probe(Err("USB not connected".into())));
                }
            }
            NetCmd::PollStatus { transport, base } => match transport {
                Transport::Lan => {
                    let r = agent
                        .get(&format!("{base}/api/status"))
                        .call()
                        .map_err(|e| e.to_string())
                        .and_then(|r| r.into_json::<StatusJson>().map_err(|e| e.to_string()));
                    let _ = msg_tx.send(NetMsg::Status(r));
                }
                Transport::Usb => {
                    if let Some(p) = usb.as_mut() {
                        let r = usb_cmd(p.as_mut(), &mut usb_rx, "cmp status")
                            .and_then(|l| parse_cmp_status(&l));
                        let _ = msg_tx.send(NetMsg::Status(r));
                    } else if !base.is_empty() {
                        // reopen if we only have the name stored as base
                        let _ = msg_tx.send(NetMsg::Status(Err("USB not open — Connect".into())));
                    }
                }
            },
            NetCmd::FetchConfig { transport, base } => match transport {
                Transport::Lan => {
                    let r = agent
                        .get(&format!("{base}/api/config"))
                        .call()
                        .map_err(|e| e.to_string())
                        .and_then(|r| r.into_json::<ConfigJson>().map_err(|e| e.to_string()));
                    let _ = msg_tx.send(NetMsg::Config(r));
                }
                Transport::Usb => {
                    if let Some(p) = usb.as_mut() {
                        let r = usb_cmd(p.as_mut(), &mut usb_rx, "cmp config")
                            .and_then(|l| parse_cmp_config(&l));
                        let _ = msg_tx.send(NetMsg::Config(r));
                    } else {
                        let _ = msg_tx.send(NetMsg::Config(Err("USB not open".into())));
                    }
                }
            },
            NetCmd::Post {
                transport,
                base,
                path,
                body,
            } => match transport {
                Transport::Lan => {
                    let r = agent
                        .post(&format!("{base}{path}"))
                        .set("Content-Type", "application/x-www-form-urlencoded")
                        .send_string(&body)
                        .map_err(|e| e.to_string())
                        .and_then(|r| r.into_string().map_err(|e| e.to_string()));
                    let _ = msg_tx.send(NetMsg::Action(r));
                }
                Transport::Usb => {
                    if let Some(p) = usb.as_mut() {
                        let verb = if path.contains("clock") {
                            "clock"
                        } else if path.contains("reboot") {
                            "reboot"
                        } else {
                            "set"
                        };
                        let cmd = if path.contains("reconnect") {
                            if body.is_empty() {
                                "cmp set reconnect=true".to_string()
                            } else {
                                format!("cmp set {body}")
                            }
                        } else {
                            format!("cmp {verb} {body}")
                        };
                        let r = usb_cmd(p.as_mut(), &mut usb_rx, &cmd);
                        let _ = msg_tx.send(NetMsg::Action(r));
                    } else {
                        let _ = msg_tx.send(NetMsg::Action(Err("USB not open".into())));
                    }
                }
            },
            NetCmd::Probe(base) => {
                let r = agent
                    .get(&format!("{base}/probe"))
                    .call()
                    .map_err(|e| e.to_string())
                    .and_then(|resp| {
                        let body = resp.into_string().map_err(|e| e.to_string())?;
                        if body.contains("SCRYPT") || body.contains("hr") {
                            Ok(format!("{base} → {body}"))
                        } else {
                            Err("not a scrypt miner".into())
                        }
                    });
                let _ = msg_tx.send(NetMsg::Probe(r));
            }
        }
    }
}

fn drain_serial(port: &mut dyn SerialPort, buf: &mut String) {
    let mut tmp = [0u8; 256];
    for _ in 0..20 {
        match port.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                buf.push_str(&String::from_utf8_lossy(&tmp[..n]));
                if buf.len() > 8192 {
                    let keep = buf[buf.len() - 2048..].to_string();
                    *buf = keep;
                }
            }
        }
    }
}

fn usb_cmd(port: &mut dyn SerialPort, buf: &mut String, cmd: &str) -> Result<String, String> {
    drain_serial(port, buf);
    buf.clear();
    let line = format!("{cmd}\r\n");
    port.write_all(line.as_bytes())
        .map_err(|e| format!("USB write: {e}"))?;
    port.flush().map_err(|e| format!("USB flush: {e}"))?;

    let deadline = Instant::now() + Duration::from_millis(900);
    while Instant::now() < deadline {
        drain_serial(port, buf);
        for raw in buf.lines() {
            let t = raw.trim();
            if t.starts_with("CMPSTATUS ")
                || t.starts_with("CMPCONFIG ")
                || t.starts_with("CMPACK")
                || t.starts_with("CMP ok")
                || t.starts_with("CMPERR")
            {
                return Ok(t.to_string());
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(format!(
        "USB timeout waiting for reply to `{cmd}` (got {} bytes)",
        buf.len()
    ))
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

fn urlenc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
