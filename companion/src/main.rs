//! CYD Companion — USB-only mining control.
//! PC owns stratum/WiFi; board only hashes work received over USB-C.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod stratum;

use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Color32, FontFamily, FontId, Frame, Margin, RichText, Rounding, Stroke, Vec2,
};
use eframe::{App, NativeOptions};
use serde::{Deserialize, Serialize};
use serialport::SerialPort;
use stratum::{encode_job_cmd, StratumClient};

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([920.0, 640.0])
            .with_min_inner_size([780.0, 520.0])
            .with_title("CYD Companion · USB Scrypt Miner"),
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
            .min_size(Vec2::new(120.0, 36.0)),
    )
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
}

struct CompanionApp {
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
}

impl CompanionApp {
    fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<NetCmd>();
        let (msg_tx, msg_rx) = mpsc::channel::<NetMsg>();
        thread::spawn(move || mine_worker(cmd_rx, msg_tx));
        let _ = cmd_tx.send(NetCmd::ListPorts);

        let mut edit_stratum = "stratum+tcp://scrypt.mysolopool.com:3341".into();
        let mut edit_worker = String::new();
        let mut edit_password = "d=1".into();
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

        Self {
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
            last_ok: "Connect USB-C, then Start mining. Pool runs on this PC.".into(),
            cmd_tx,
            msg_rx,
            last_poll: Instant::now() - Duration::from_secs(10),
            pulse: 0.0,
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
    }

    fn start_mine(&mut self) {
        self.last_error.clear();
        if !self.usb_open {
            self.last_error = "Connect USB first.".into();
            return;
        }
        if self.edit_worker.trim().is_empty() {
            self.last_error = "Enter worker / Litecoin address.".into();
            return;
        }
        let _ = self.cmd_tx.send(NetCmd::StartMine {
            stratum: self.edit_stratum.trim().to_string(),
            worker: self.edit_worker.trim().to_string(),
            password: self.edit_password.clone(),
        });
        self.mining = true;
        self.last_ok = "Starting pool on PC → pushing work over USB…".into();
    }

    fn stop_mine(&mut self) {
        let _ = self.cmd_tx.send(NetCmd::StopMine);
        self.mining = false;
        self.last_ok = "Mining stopped.".into();
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
                    self.last_ok = s;
                    if self.last_ok.to_lowercase().contains("usb open") {
                        self.usb_open = true;
                    }
                    if self.last_ok.to_lowercase().contains("closed") {
                        self.usb_open = false;
                        self.mining = false;
                    }
                }
                NetMsg::Action(Err(e)) => {
                    self.last_error = e;
                    if self.last_error.to_lowercase().contains("stratum") {
                        self.mining = false;
                    }
                }
                NetMsg::Status(Ok(s)) => {
                    self.status = s;
                    self.last_error.clear();
                }
                NetMsg::Status(Err(e)) => self.last_error = e,
                NetMsg::Config(Ok(c)) => {
                    self.fw_label = if c.fw.is_empty() {
                        "0.3.0-usb".into()
                    } else {
                        c.fw
                    };
                    if c.cpu_mhz == 80 || c.cpu_mhz == 160 || c.cpu_mhz == 240 {
                        self.target_mhz = c.cpu_mhz;
                    }
                }
                NetMsg::Config(Err(e)) => self.last_error = e,
                NetMsg::MineStats {
                    accepted,
                    rejected,
                    phase,
                } => {
                    self.accepted = accepted;
                    self.rejected = rejected;
                    self.pool_phase = phase;
                }
            }
        }

        if self.usb_open && self.last_poll.elapsed() > Duration::from_millis(1500) {
            let _ = self.cmd_tx.send(NetCmd::PollStatus);
            self.last_poll = Instant::now();
        }

        self.pulse = (self.pulse + ctx.input(|i| i.unstable_dt) * 1.4) % std::f32::consts::TAU;
        let glow = 0.55 + 0.45 * self.pulse.sin();

        egui::CentralPanel::default()
            .frame(Frame::none().fill(C_BG).inner_margin(Margin::same(22.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("CYD Companion").color(C_LIME).strong());
                    ui.label(
                        RichText::new(format!("  ·  fw {fw}", fw = self.fw_label)).color(C_MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let link = if self.usb_open { "USB linked" } else { "USB idle" };
                        let _ = glow;
                        ui.label(
                            RichText::new(link)
                                .color(if self.usb_open { C_LIME } else { C_MUTED })
                                .strong(),
                        );
                    });
                });
                ui.label(
                    RichText::new("Board hashes over USB-C only · pool + WiFi stay on this PC")
                        .color(C_MUTED)
                        .size(13.0),
                );
                ui.add_space(12.0);

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
                        }
                    });
                });

                ui.add_space(10.0);
                panel(ui, "Pool (on this PC)", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Stratum").color(C_MUTED));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.edit_stratum)
                                .desired_width(520.0)
                                .hint_text("stratum+tcp://host:port"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Worker").color(C_MUTED));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.edit_worker)
                                .desired_width(360.0)
                                .hint_text("Litecoin address"),
                        );
                        ui.label(RichText::new("Pass").color(C_MUTED));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.edit_password).desired_width(100.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        if !self.mining {
                            if bubble(ui, "Start mining", true).clicked() {
                                self.start_mine();
                            }
                        } else if bubble(ui, "Stop mining", false).clicked() {
                            self.stop_mine();
                        }
                        ui.label(
                            RichText::new(format!("pool: {}", self.pool_phase)).color(C_MUTED),
                        );
                    });
                });

                ui.add_space(10.0);
                panel(ui, "Live", |ui| {
                    ui.horizontal(|ui| {
                        stat(ui, "Hashrate", &format!("{:.2} H/s", self.status.hashrate_hs));
                        stat(ui, "Accepted", &self.accepted.to_string());
                        stat(ui, "Rejected", &self.rejected.to_string());
                        stat(ui, "Board shares", &self.status.shares.to_string());
                        stat(
                            ui,
                            "CPU",
                            &format!("{} MHz", if self.status.cpu_mhz > 0 {
                                self.status.cpu_mhz
                            } else {
                                self.target_mhz
                            }),
                        );
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Board clock").color(C_MUTED));
                        for mhz in [80u8, 160, 240] {
                            let on = self.target_mhz == mhz;
                            if bubble(ui, &format!("{mhz}"), on).clicked() {
                                self.target_mhz = mhz;
                                let _ = self.cmd_tx.send(NetCmd::SetClock(mhz));
                            }
                        }
                    });
                });

                ui.add_space(12.0);
                if !self.last_ok.is_empty() {
                    ui.label(RichText::new(&self.last_ok).color(C_LIME));
                }
                if !self.last_error.is_empty() {
                    ui.label(RichText::new(&self.last_error).color(Color32::from_rgb(255, 120, 100)));
                }
            });

        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

fn panel(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    Frame::none()
        .fill(C_PANEL)
        .rounding(Rounding::same(18.0))
        .stroke(Stroke::new(1.0, Color32::from_rgb(40, 60, 80)))
        .inner_margin(Margin::same(14.0))
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
        .inner_margin(Margin::symmetric(14.0, 10.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(label).color(C_MUTED).size(11.0));
                ui.label(RichText::new(value).color(C_LIME).strong().size(18.0));
            });
        });
}

fn mine_worker(cmd_rx: Receiver<NetCmd>, msg_tx: Sender<NetMsg>) {
    let mut usb: Option<Box<dyn SerialPort>> = None;
    let mut usb_rx = String::new();
    let mut stratum: Option<StratumClient> = None;
    let mut mining = false;
    let mut last_stats_push = Instant::now() - Duration::from_secs(10);

    loop {
        // Non-blocking command drain with short wait when idle.
        let cmd = if mining {
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
                            let _ = msg_tx.send(NetMsg::Action(Err(format!("USB open failed: {e}"))));
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
                    let mut client = StratumClient::new(worker, password);
                    match client.connect(&endpoint) {
                        Ok(()) => {
                            stratum = Some(client);
                            mining = true;
                            let _ = msg_tx.send(NetMsg::Action(Ok(format!(
                                "Pool connecting {endpoint}"
                            ))));
                        }
                        Err(e) => {
                            let _ = msg_tx.send(NetMsg::Action(Err(e)));
                            mining = false;
                        }
                    }
                }
                NetCmd::StopMine => {
                    mining = false;
                    if let Some(mut s) = stratum.take() {
                        s.disconnect();
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
                }
                NetCmd::SetClock(mhz) => {
                    if let Some(p) = usb.as_mut() {
                        let cmd = format!("cmp clock cpu_mhz={mhz}");
                        let r = usb_cmd(p.as_mut(), &mut usb_rx, &cmd);
                        let _ = msg_tx.send(NetMsg::Action(r.map(|_| format!("Clock {mhz} MHz queued"))));
                    }
                }
                NetCmd::PollStatus => {
                    if let Some(p) = usb.as_mut() {
                        match usb_cmd(p.as_mut(), &mut usb_rx, "cmp status") {
                            Ok(line) => {
                                // Also harvest any CMPSHARE that arrived with status traffic.
                                harvest_shares(p.as_mut(), &mut usb_rx, stratum.as_mut(), &msg_tx);
                                let _ = msg_tx.send(NetMsg::Status(parse_cmp_status(&line)));
                            }
                            Err(e) => {
                                let _ = msg_tx.send(NetMsg::Status(Err(e)));
                            }
                        }
                    }
                }
            }
        }

        if !mining {
            continue;
        }

        // Stratum + USB work loop
        if let Some(client) = stratum.as_mut() {
            if let Err(e) = client.poll() {
                let _ = msg_tx.send(NetMsg::Action(Err(e)));
                mining = false;
                continue;
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
            let _ = msg_tx.send(NetMsg::MineStats {
                accepted: client.accepted,
                rejected: client.rejected,
                phase: client.phase.clone(),
            });
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

        if let Some(p) = usb.as_mut() {
            harvest_shares(p.as_mut(), &mut usb_rx, stratum.as_mut(), &msg_tx);
        }
        thread::sleep(Duration::from_millis(15));
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
            "CMPACK",
            "CMP ok",
            "CMPERR",
            "CMPSHARE ",
        ] {
            if let Some(idx) = t.find(prefix) {
                // Don't treat CMPSHARE as the reply to a command.
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
    for _ in 0..3 {
        drain_serial(port, buf);
        // Preserve pending shares; only clear non-share noise after harvest isn't available here.
        let mut keep = String::new();
        for line in buf.lines() {
            if line.trim().starts_with("CMPSHARE ") {
                keep.push_str(line.trim());
                keep.push('\n');
            }
        }
        *buf = keep;
        let line = format!("\r\n{cmd}\r\n");
        port.write_all(line.as_bytes())
            .map_err(|e| format!("USB write: {e}"))?;
        port.flush().map_err(|e| format!("USB flush: {e}"))?;
        let deadline = Instant::now() + Duration::from_millis(5000);
        while Instant::now() < deadline {
            drain_serial(port, buf);
            if let Some(reply) = cmp_reply_line(buf) {
                return Ok(reply);
            }
            thread::sleep(Duration::from_millis(15));
        }
        last_err = format!("USB timeout waiting for reply to `{cmd}`");
        thread::sleep(Duration::from_millis(80));
    }
    Err(last_err)
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
