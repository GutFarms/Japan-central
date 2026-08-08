use eframe::egui::{self, Color32, CornerRadius, FontId, Frame, Margin, RichText, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

use crate::agent::{Agent, AgentEvent};
use crate::client::XaiClient;
use crate::config::{save_settings, Config};
use crate::tools::ToolRuntime;

#[derive(Clone)]
enum Role {
    User,
    Assistant,
    Tool,
    System,
    Error,
}

struct ChatMessage {
    role: Role,
    text: String,
    born: Instant,
}

enum UiCommand {
    Run { prompt: String, cfg: Config },
    Reset,
}

enum WorkerMsg {
    Event(AgentEvent),
    Busy(bool),
}

pub struct GrokApp {
    cfg: Config,
    draft: String,
    messages: Vec<ChatMessage>,
    status: String,
    busy: bool,
    show_settings: bool,
    scroll_to_bottom: bool,
    started: Instant,

    // settings drafts
    api_key: String,
    base_url: String,
    model: String,
    workspace: String,
    web_search: bool,
    code_interpreter: bool,
    max_turns: String,

    cmd_tx: Sender<UiCommand>,
    event_rx: Receiver<WorkerMsg>,
    runtime_alive: Arc<AtomicBool>,
}

impl GrokApp {
    pub fn new(cfg: Config) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<UiCommand>();
        let (event_tx, event_rx) = mpsc::channel::<WorkerMsg>();
        let runtime_alive = Arc::new(AtomicBool::new(true));
        let alive = runtime_alive.clone();

        std::thread::Builder::new()
            .name("grok-agent-worker".into())
            .spawn(move || worker_loop(cmd_rx, event_tx, alive))
            .expect("spawn worker");

        let api_key = cfg.api_key().to_string();
        let base_url = cfg.base_url.clone();
        let model = cfg.model.clone();
        let workspace = cfg.workspace.display().to_string();
        let web_search = cfg.web_search;
        let code_interpreter = cfg.code_interpreter;
        let max_turns = cfg.max_turns.to_string();

        let mut app = Self {
            cfg,
            draft: String::new(),
            messages: Vec::new(),
            status: "Ready".into(),
            busy: false,
            show_settings: false,
            scroll_to_bottom: true,
            started: Instant::now(),
            api_key,
            base_url,
            model,
            workspace,
            web_search,
            code_interpreter,
            max_turns,
            cmd_tx,
            event_rx,
            runtime_alive,
        };

        if app.cfg.api_key().trim().is_empty() {
            app.show_settings = true;
            app.push(
                Role::System,
                "Welcome to grok-agent. Add your xAI API key in Settings to start chatting with Grok 4.5.",
            );
        } else {
            app.push(
                Role::System,
                format!(
                    "Connected to {} · workspace {}",
                    app.cfg.model,
                    app.cfg.workspace.display()
                ),
            );
        }

        app
    }

    fn push(&mut self, role: Role, text: impl Into<String>) {
        self.messages.push(ChatMessage {
            role,
            text: text.into(),
            born: Instant::now(),
        });
        self.scroll_to_bottom = true;
    }

    fn poll_events(&mut self) {
        while let Ok(msg) = self.event_rx.try_recv() {
            match msg {
                WorkerMsg::Busy(busy) => {
                    self.busy = busy;
                    if !busy
                        && (self.status.starts_with("Thinking")
                            || self.status.starts_with("Running"))
                    {
                        self.status = "Ready".into();
                    }
                }
                WorkerMsg::Event(ev) => match ev {
                    AgentEvent::Status(s) => self.status = s,
                    AgentEvent::AssistantDelta(text) => {
                        if let Some(last) = self.messages.last_mut() {
                            if matches!(last.role, Role::Assistant) && last.text.is_empty() {
                                last.text = text;
                                self.scroll_to_bottom = true;
                                continue;
                            }
                        }
                        self.push(Role::Assistant, text);
                    }
                    AgentEvent::ToolStart { name, args } => {
                        self.push(Role::Tool, format!("→ {name}({args})"));
                    }
                    AgentEvent::ToolResult { name, preview } => {
                        self.push(Role::Tool, format!("← {name}: {preview}"));
                    }
                    AgentEvent::Finished => {
                        self.status = "Ready".into();
                    }
                    AgentEvent::Error(err) => {
                        self.push(Role::Error, err);
                        self.status = "Error".into();
                    }
                },
            }
        }
    }

    fn send_prompt(&mut self) {
        let prompt = self.draft.trim().to_string();
        if prompt.is_empty() || self.busy {
            return;
        }
        if let Err(err) = self.cfg.require_api_key() {
            self.push(Role::Error, format!("{err:#}"));
            self.show_settings = true;
            return;
        }

        self.draft.clear();
        self.push(Role::User, prompt.clone());
        self.push(Role::Assistant, String::new());
        self.status = "Thinking…".into();
        self.busy = true;
        let _ = self.cmd_tx.send(UiCommand::Run {
            prompt,
            cfg: self.cfg.clone(),
        });
    }

    fn apply_settings(&mut self) {
        let max_turns = self.max_turns.parse::<usize>().unwrap_or(24).max(1);
        let workspace = PathBuf::from(self.workspace.trim());
        match self.cfg.apply_saved_fields(
            self.api_key.trim().to_string(),
            self.base_url.trim().to_string(),
            self.model.trim().to_string(),
            workspace,
            self.web_search,
            self.code_interpreter,
            max_turns,
        ) {
            Ok(()) => {
                if let Err(err) = save_settings(&self.cfg.to_saved()) {
                    self.push(Role::Error, format!("Could not save settings: {err:#}"));
                } else {
                    self.push(
                        Role::System,
                        format!(
                            "Settings saved · {} · {}",
                            self.cfg.model,
                            self.cfg.workspace.display()
                        ),
                    );
                    self.show_settings = false;
                    let _ = self.cmd_tx.send(UiCommand::Reset);
                }
            }
            Err(err) => self.push(Role::Error, format!("Invalid settings: {err:#}")),
        }
    }

    fn reset_chat(&mut self) {
        self.messages.clear();
        self.push(Role::System, "Conversation reset.");
        let _ = self.cmd_tx.send(UiCommand::Reset);
        self.status = "Ready".into();
    }
}

impl Drop for GrokApp {
    fn drop(&mut self) {
        self.runtime_alive.store(false, Ordering::SeqCst);
    }
}

impl eframe::App for GrokApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
        apply_theme(ctx);

        let pulse = ((self.started.elapsed().as_secs_f32() * 2.2).sin() + 1.0) * 0.5;

        egui::TopBottomPanel::top("header")
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(14, 18, 24))
                    .inner_margin(Margin::symmetric(18, 14))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(32, 42, 54))),
            )
            .show(ctx, |ui| {
                ui.set_min_height(56.0);
                ui.horizontal_centered(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("grok-agent")
                                .font(FontId::proportional(28.0))
                                .color(Color32::from_rgb(232, 236, 242))
                                .strong(),
                        );
                        ui.label(
                            RichText::new("Local agent · Grok 4.5")
                                .size(13.0)
                                .color(Color32::from_rgb(120, 140, 158)),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let settings = ui.add(pill_button("Settings", self.show_settings));
                        if settings.clicked() || settings.secondary_clicked() {
                            self.show_settings = !self.show_settings;
                        }
                        if ui.add(pill_button("Reset", false)).clicked() {
                            self.reset_chat();
                        }

                        let status_color = if self.busy {
                            Color32::from_rgb(
                                40 + (pulse * 40.0) as u8,
                                180 + (pulse * 40.0) as u8,
                                170,
                            )
                        } else {
                            Color32::from_rgb(94, 176, 164)
                        };
                        ui.label(
                            RichText::new(format!("● {}", self.status))
                                .size(13.0)
                                .color(status_color),
                        );
                    });
                });
            });

        if self.show_settings {
            egui::SidePanel::right("settings")
                .resizable(true)
                .default_width(320.0)
                .frame(
                    Frame::new()
                        .fill(Color32::from_rgb(16, 22, 30))
                        .inner_margin(Margin::same(16))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(36, 48, 62))),
                )
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("Settings")
                            .size(18.0)
                            .color(Color32::from_rgb(230, 236, 242))
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Stored in ~/.grok-agent/settings.json")
                            .size(12.0)
                            .color(Color32::from_rgb(120, 140, 158)),
                    );
                    ui.add_space(12.0);

                    labeled_field(ui, "API key", &mut self.api_key, true);
                    labeled_field(ui, "Base URL", &mut self.base_url, false);
                    labeled_field(ui, "Model", &mut self.model, false);
                    labeled_field(ui, "Workspace", &mut self.workspace, false);
                    labeled_field(ui, "Max turns", &mut self.max_turns, false);

                    ui.add_space(8.0);
                    ui.checkbox(&mut self.web_search, "Enable web_search");
                    ui.checkbox(&mut self.code_interpreter, "Enable code_interpreter");

                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.add(primary_button("Save")).clicked() {
                            self.apply_settings();
                        }
                        if ui.add(pill_button("Close", false)).clicked() {
                            self.show_settings = false;
                        }
                    });
                });
        }

        egui::TopBottomPanel::bottom("composer")
            .exact_height(132.0)
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(14, 18, 24))
                    .inner_margin(Margin::symmetric(18, 14))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(32, 42, 54))),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "{}  ·  {}",
                        self.cfg.model,
                        short_path(&self.cfg.workspace)
                    ))
                    .size(12.0)
                    .color(Color32::from_rgb(110, 130, 148)),
                );
                ui.add_space(6.0);

                let mut send_now = false;
                if ui.memory(|m| m.has_focus(egui::Id::new("composer_input"))) {
                    ui.input(|i| {
                        if i.key_pressed(egui::Key::Enter) && !i.modifiers.shift {
                            send_now = true;
                        }
                    });
                }

                let text_edit = egui::TextEdit::multiline(&mut self.draft)
                    .id(egui::Id::new("composer_input"))
                    .desired_width(f32::INFINITY)
                    .desired_rows(3)
                    .hint_text("Ask grok-agent to inspect code, edit files, or run commands…")
                    .frame(true);
                ui.add(text_edit);

                if send_now {
                    // Multiline TextEdit may insert a newline before we handle Enter.
                    while self.draft.ends_with('\n') {
                        self.draft.pop();
                    }
                    self.send_prompt();
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Enter to send · Shift+Enter for newline")
                            .size(11.0)
                            .color(Color32::from_rgb(90, 110, 128)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let send = ui.add_enabled(!self.busy, primary_button(if self.busy {
                            "Working…"
                        } else {
                            "Send"
                        }));
                        if send.clicked() {
                            self.send_prompt();
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(10, 13, 18))
                    .inner_margin(Margin::symmetric(22, 16)),
            )
            .show(ctx, |ui| {
                // soft vignette feel via top gradient band
                let band = ((self.started.elapsed().as_secs_f32() * 0.35).sin() + 1.0) * 0.5;
                ui.label(
                    RichText::new("Conversation")
                        .size(12.0)
                        .color(Color32::from_rgb(
                            90 + (band * 20.0) as u8,
                            120 + (band * 20.0) as u8,
                            140,
                        )),
                );
                ui.add_space(8.0);

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        for (idx, msg) in self.messages.iter().enumerate() {
                            let age = msg.born.elapsed().as_secs_f32();
                            let alpha = (age * 4.0).clamp(0.35, 1.0);
                            message_bubble(ui, msg, alpha);
                            if idx + 1 != self.messages.len() {
                                ui.add_space(10.0);
                            }
                        }
                        if self.scroll_to_bottom {
                            ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                            self.scroll_to_bottom = false;
                        }
                    });
            });

        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(33));
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}

fn worker_loop(cmd_rx: Receiver<UiCommand>, event_tx: Sender<WorkerMsg>, alive: Arc<AtomicBool>) {
    let rt = Runtime::new().expect("tokio runtime");
    let mut agent: Option<Agent> = None;
    let mut agent_key = String::new();

    while alive.load(Ordering::SeqCst) {
        match cmd_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(UiCommand::Reset) => {
                if let Some(a) = agent.as_mut() {
                    a.reset();
                }
            }
            Ok(UiCommand::Run { prompt, cfg }) => {
                let _ = event_tx.send(WorkerMsg::Busy(true));
                let fingerprint = format!(
                    "{}|{}|{}|{}|{}|{}",
                    cfg.api_key(),
                    cfg.base_url,
                    cfg.model,
                    cfg.workspace.display(),
                    cfg.web_search,
                    cfg.code_interpreter
                );

                if agent.is_none() || agent_key != fingerprint {
                    match XaiClient::new(&cfg) {
                        Ok(client) => {
                            let tools = Arc::new(ToolRuntime::new(&cfg));
                            let (tx, rx) = mpsc::channel::<AgentEvent>();
                            let forward_tx = event_tx.clone();
                            std::thread::spawn(move || {
                                while let Ok(ev) = rx.recv() {
                                    if forward_tx.send(WorkerMsg::Event(ev)).is_err() {
                                        break;
                                    }
                                }
                            });
                            agent = Some(Agent::new(&cfg, client, tools).with_events(tx));
                            agent_key = fingerprint;
                        }
                        Err(err) => {
                            let _ = event_tx.send(WorkerMsg::Event(AgentEvent::Error(format!(
                                "{err:#}"
                            ))));
                            let _ = event_tx.send(WorkerMsg::Busy(false));
                            continue;
                        }
                    }
                }

                if let Some(a) = agent.as_mut() {
                    let result = rt.block_on(a.run_turn(&prompt));
                    if let Err(err) = result {
                        let _ = event_tx
                            .send(WorkerMsg::Event(AgentEvent::Error(format!("{err:#}"))));
                    }
                }
                let _ = event_tx.send(WorkerMsg::Busy(false));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

pub fn run_gui(cfg: Config) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 720.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("grok-agent"),
        ..Default::default()
    };

    eframe::run_native(
        "grok-agent",
        options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            Ok(Box::new(GrokApp::new(cfg)))
        }),
    )
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.visuals.window_fill = Color32::from_rgb(10, 13, 18);
    style.visuals.panel_fill = Color32::from_rgb(10, 13, 18);
    style.visuals.override_text_color = Some(Color32::from_rgb(220, 228, 236));
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(28, 36, 46);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 50, 64);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(48, 64, 80);
    style.visuals.selection.bg_fill = Color32::from_rgb(36, 120, 112);
    style.visuals.extreme_bg_color = Color32::from_rgb(18, 24, 32);
    ctx.set_style(style);

}

fn message_bubble(ui: &mut egui::Ui, msg: &ChatMessage, alpha: f32) {
    let (label, fill, stroke, text_color, mono) = match msg.role {
        Role::User => (
            "You",
            Color32::from_rgb(22, 40, 48),
            Color32::from_rgb(48, 120, 118),
            Color32::from_rgb(220, 240, 236),
            false,
        ),
        Role::Assistant => (
            "Grok",
            Color32::from_rgb(24, 28, 36),
            Color32::from_rgb(56, 68, 84),
            Color32::from_rgb(232, 236, 242),
            false,
        ),
        Role::Tool => (
            "Tool",
            Color32::from_rgb(18, 24, 20),
            Color32::from_rgb(70, 110, 80),
            Color32::from_rgb(180, 210, 180),
            true,
        ),
        Role::System => (
            "System",
            Color32::from_rgb(20, 24, 32),
            Color32::from_rgb(50, 60, 74),
            Color32::from_rgb(150, 166, 182),
            false,
        ),
        Role::Error => (
            "Error",
            Color32::from_rgb(48, 22, 22),
            Color32::from_rgb(160, 70, 70),
            Color32::from_rgb(240, 190, 190),
            false,
        ),
    };

    let a = (alpha * 255.0) as u8;
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), a))
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(stroke.r(), stroke.g(), stroke.b(), a),
        ))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            ui.label(
                RichText::new(label)
                    .size(11.0)
                    .color(Color32::from_rgba_unmultiplied(
                        stroke.r(),
                        stroke.g(),
                        stroke.b(),
                        a,
                    ))
                    .strong(),
            );
            if msg.text.is_empty() {
                ui.label(
                    RichText::new("…")
                        .color(Color32::from_rgba_unmultiplied(
                            text_color.r(),
                            text_color.g(),
                            text_color.b(),
                            a,
                        ))
                        .italics(),
                );
            } else {
                let mut body = RichText::new(&msg.text).color(
                    Color32::from_rgba_unmultiplied(text_color.r(), text_color.g(), text_color.b(), a),
                );
                if mono {
                    body = body.monospace().size(12.5);
                } else {
                    body = body.size(14.5);
                }
                ui.label(body);
            }
        });
}

fn labeled_field(ui: &mut egui::Ui, label: &str, value: &mut String, password: bool) {
    ui.label(
        RichText::new(label)
            .size(12.0)
            .color(Color32::from_rgb(140, 158, 174)),
    );
    let mut edit = egui::TextEdit::singleline(value)
        .desired_width(f32::INFINITY)
        .password(password);
    if password {
        edit = edit.hint_text("xai-...");
    }
    ui.add(edit);
    ui.add_space(6.0);
}

fn primary_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label).color(Color32::from_rgb(10, 18, 18)).strong())
        .fill(Color32::from_rgb(94, 196, 178))
        .corner_radius(CornerRadius::same(8))
        .min_size(Vec2::new(96.0, 34.0))
}

fn pill_button(label: &str, active: bool) -> egui::Button<'static> {
    let fill = if active {
        Color32::from_rgb(42, 64, 72)
    } else {
        Color32::from_rgb(28, 36, 46)
    };
    egui::Button::new(RichText::new(label).color(Color32::from_rgb(210, 222, 230)))
        .fill(fill)
        .corner_radius(CornerRadius::same(8))
        .min_size(Vec2::new(88.0, 32.0))
}

fn short_path(path: &std::path::Path) -> String {
    let s = path.display().to_string();
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = s.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    s
}
