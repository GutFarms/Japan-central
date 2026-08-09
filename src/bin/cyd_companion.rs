//! CYD Companion — GPU desktop app to configure & overclock the ESP32-2432S028 miner.
//!
//! ```text
//! cargo run --no-default-features --features companion --bin cyd-companion --release
//! # Windows:
//! cargo build --no-default-features --features companion --bin cyd-companion \
//!   --release --target x86_64-pc-windows-gnu
//! ```

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, FontFamily, FontId, Frame, Layout, Margin, RichText, Rounding, Sense,
    Stroke, Vec2,
};
use eframe::{App, NativeOptions};
use serde::Deserialize;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([880.0, 600.0])
            .with_title("CYD Companion · Scrypt Miner Control"),
        // eframe 0.27: OpenGL/glow (GPU) with MSAA.
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
    // Forge / copper industrial — not purple, not cream brochure.
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
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::new(14.5, FontFamily::Proportional));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace));
    ctx.set_style(style);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Settings,
    Overclock,
    Discover,
}

#[derive(Debug, Clone, Default, Deserialize)]
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
}

enum NetCmd {
    PollStatus(String),
    FetchConfig(String),
    Post { base: String, path: String, body: String },
    Probe(String),
}

struct CompanionApp {
    tab: Tab,
    board_ip: String,
    auth_password: String,
    status: StatusJson,
    last_error: String,
    last_ok: String,
    connected_ui: bool,
    // settings editors
    edit_worker: String,
    edit_stratum: String,
    edit_password: String,
    edit_wifi_ssid: String,
    edit_wifi_password: String,
    // overclock
    target_mhz: u8,
    // discover
    discover_base: String,
    discover_log: String,
    // net
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
        Self {
            tab: Tab::Dashboard,
            board_ip: "192.168.1.50".into(),
            auth_password: "x".into(),
            status: StatusJson::default(),
            last_error: String::new(),
            last_ok: "Enter board IP and connect.".into(),
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

    fn connect(&mut self) {
        self.connected_ui = true;
        self.last_ok = format!("Polling {} …", self.base_url());
        self.last_error.clear();
        let _ = self.cmd_tx.send(NetCmd::FetchConfig(self.base_url()));
        let _ = self.cmd_tx.send(NetCmd::PollStatus(self.base_url()));
        let _ = self.cmd_tx.send(NetCmd::Probe(self.base_url()));
    }

    fn post(&mut self, path: &str, body: String) {
        let _ = self.cmd_tx.send(NetCmd::Post {
            base: self.base_url(),
            path: path.into(),
            body,
        });
    }

    fn apply_settings(&mut self) {
        let body = format!(
            "auth={}&worker={}&stratum={}&password={}&wifi_ssid={}&wifi_password={}&reconnect=true",
            urlenc(&self.auth_password),
            urlenc(&self.edit_worker),
            urlenc(&self.edit_stratum),
            urlenc(&self.edit_password),
            urlenc(&self.edit_wifi_ssid),
            urlenc(&self.edit_wifi_password),
        );
        self.post("/api/config", body);
        self.last_ok = "Settings queued — board may reboot if WiFi changed.".into();
    }

    fn apply_clock(&mut self) {
        let body = format!(
            "auth={}&cpu_mhz={}&reboot=true",
            urlenc(&self.auth_password),
            self.target_mhz
        );
        self.post("/api/clock", body);
        self.last_ok = format!(
            "CPU {} MHz queued — board soft-resets to apply.",
            self.target_mhz
        );
    }

    fn drain_net(&mut self) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
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
                    // masked password hint only — keep local edit if empty mask
                    if self.edit_wifi_password.is_empty() && c.wifi_password.contains('*') {
                        // leave blank so user retypes to change
                    }
                    if c.cpu_mhz != 0 {
                        self.target_mhz = c.cpu_mhz;
                    }
                    if !c.fw.is_empty() {
                        self.fw_label = c.fw;
                    }
                    self.last_ok = "Config loaded from board.".into();
                }
                NetMsg::Config(Err(e)) => self.last_error = e,
                NetMsg::Action(Ok(s)) => self.last_ok = s,
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
            let _ = self.cmd_tx.send(NetCmd::PollStatus(self.base_url()));
            self.last_poll = Instant::now();
        }

        // Atmospheric top band
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
                            RichText::new("ESP32-2432S028 · scrypt control · wgpu")
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
                    ui.label("Board IP");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.board_ip)
                            .desired_width(180.0)
                            .hint_text("192.168.x.x"),
                    );
                    ui.label("Auth (pool password)");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.auth_password)
                            .desired_width(120.0)
                            .password(true),
                    );
                    if ui
                        .add(egui::Button::new(RichText::new("Connect").strong()).min_size(Vec2::new(100.0, 32.0)))
                        .clicked()
                    {
                        self.connect();
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
                    (Tab::Settings, "Settings"),
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
                        RichText::new("LAN only · auth = pool password")
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
                    Tab::Settings => self.ui_settings(ui),
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
                ui.label(format!("uptime {}s", self.status.uptime_secs));
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

        // Subtle animated meter bar
        ui.add_space(18.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 9.0, Color32::from_rgb(28, 24, 20));
        let frac = ((rate / 25.0) as f32).clamp(0.05, 1.0);
        let mut fill = rect;
        fill.set_width(rect.width() * frac);
        let wave = (self.pulse.sin() * 0.5 + 0.5) * 20.0;
        painter.rect_filled(
            fill,
            9.0,
            Color32::from_rgb(200, 90 + wave as u8, 30),
        );
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Board settings")
                .size(22.0)
                .color(Color32::from_rgb(255, 160, 70)),
        );
        ui.label("Changes save to flash. WiFi edits reboot the board. Auth is the pool password.");
        ui.add_space(10.0);

        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label("WiFi SSID");
                ui.add(egui::TextEdit::singleline(&mut self.edit_wifi_ssid).desired_width(360.0));
                ui.end_row();
                ui.label("WiFi password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.edit_wifi_password)
                        .desired_width(360.0)
                        .password(true),
                );
                ui.end_row();
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
                        .password(true),
                );
                ui.end_row();
            });

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Save to board").strong())
                        .fill(Color32::from_rgb(196, 92, 28))
                        .min_size(Vec2::new(160.0, 36.0)),
                )
                .clicked()
            {
                self.apply_settings();
            }
            if ui.button("Reload from board").clicked() {
                let _ = self.cmd_tx.send(NetCmd::FetchConfig(self.base_url()));
            }
        });
    }

    fn ui_overclock(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("CPU clock")
                .size(22.0)
                .color(Color32::from_rgb(255, 160, 70)),
        );
        ui.label(
            "ESP32 profiles: 80 / 160 / 240 MHz. Applied on soft-reset. Higher clocks raise hashrate and heat; WiFi can get flaky above stock if the board is warm.",
        );
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
                "Running now: {} MHz · Target: {} MHz",
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
            RichText::new("LAN discover")
                .size(22.0)
                .color(Color32::from_rgb(255, 160, 70)),
        );
        ui.label("Probe /probe on a /24 subnet for SCRYPT-CYD boards (same shape as NM Monitor).");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Subnet base");
            ui.add(
                egui::TextEdit::singleline(&mut self.discover_base)
                    .desired_width(140.0)
                    .hint_text("192.168.1"),
            );
            if ui.button("Scan .1–.254 (slow)").clicked() {
                self.discover_log.clear();
                let base = self.discover_base.trim().to_string();
                for i in 1..=254u16 {
                    let url = format!("http://{base}.{i}");
                    let _ = self.cmd_tx.send(NetCmd::Probe(url));
                }
                self.last_ok = "Scan queued…".into();
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
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            NetCmd::PollStatus(base) => {
                let r = agent
                    .get(&format!("{base}/api/status"))
                    .call()
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.into_json::<StatusJson>().map_err(|e| e.to_string()));
                let _ = msg_tx.send(NetMsg::Status(r));
            }
            NetCmd::FetchConfig(base) => {
                let r = agent
                    .get(&format!("{base}/api/config"))
                    .call()
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.into_json::<ConfigJson>().map_err(|e| e.to_string()));
                let _ = msg_tx.send(NetMsg::Config(r));
            }
            NetCmd::Post { base, path, body } => {
                let r = agent
                    .post(&format!("{base}{path}"))
                    .set("Content-Type", "application/x-www-form-urlencoded")
                    .send_string(&body)
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.into_string().map_err(|e| e.to_string()));
                let _ = msg_tx.send(NetMsg::Action(r));
            }
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
