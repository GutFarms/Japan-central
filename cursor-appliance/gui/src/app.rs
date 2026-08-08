use crate::settings::GuiSettings;
use crate::worker::{path_exists, Backend, WorkerController, WorkerState};
use chrono::Local;
use eframe::egui::{self, Color32, RichText, Sense};
use std::path::PathBuf;
use std::time::Duration;

pub struct ApplianceApp {
    appliance_root: PathBuf,
    settings: GuiSettings,
    draft: GuiSettings,
    show_settings: bool,
    status_message: String,
    worker: WorkerController,
    api_key_visible: bool,
}

impl ApplianceApp {
    pub fn new(cc: &eframe::CreationContext<'_>, appliance_root: PathBuf) -> Self {
        let settings = GuiSettings::load(&appliance_root);
        apply_theme(&cc.egui_ctx, settings.dark_mode);

        let mut worker = WorkerController::new(appliance_root.clone());
        if settings.offline_mode {
            if let Err(err) = worker.start_local(&settings) {
                worker.state = WorkerState::Error(err);
            } else {
                worker.state = WorkerState::Offline;
            }
        } else if settings.auto_start_worker {
            if let Err(err) = worker.start_cloud(&settings) {
                worker.state = WorkerState::Error(err);
            }
        }

        let draft = settings.clone();
        Self {
            appliance_root,
            settings,
            draft,
            show_settings: false,
            status_message: "Ready.".into(),
            worker,
            api_key_visible: false,
        }
    }

    fn persist(&mut self) {
        match self.settings.save(&self.appliance_root) {
            Ok(()) => self.status_message = format!("Saved {}", Local::now().format("%H:%M:%S")),
            Err(err) => self.status_message = format!("Save failed: {err}"),
        }
    }

    fn apply_draft_settings(&mut self) {
        let was_offline = self.settings.offline_mode;
        self.settings = self.draft.clone();
        self.persist();
        if self.settings.offline_mode != was_offline {
            self.worker
                .set_offline(self.settings.offline_mode, &mut self.settings);
            self.draft.offline_mode = self.settings.offline_mode;
        }
        self.status_message = "Settings applied.".into();
    }
}

impl eframe::App for ApplianceApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx, self.settings.dark_mode);
        self.worker.tick(&self.settings);

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Cursor Appliance").strong());
                ui.label(
                    RichText::new("local My Machines control")
                        .small()
                        .color(muted(self.settings.dark_mode)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .selectable_label(self.show_settings, "⚙ Settings")
                        .clicked()
                    {
                        self.show_settings = !self.show_settings;
                        if self.show_settings {
                            self.draft = self.settings.clone();
                        }
                    }
                    ui.separator();
                    let mut dark = self.settings.dark_mode;
                    if ui
                        .checkbox(&mut dark, "Dark mode")
                        .on_hover_text("Switch egui visuals")
                        .changed()
                    {
                        self.settings.dark_mode = dark;
                        self.draft.dark_mode = dark;
                        self.persist();
                    }
                    ui.separator();
                    let mut offline = self.settings.offline_mode;
                    let offline_resp = ui.checkbox(&mut offline, "Offline mode").on_hover_text(
                        "Disconnect cloud worker and let the lightweight local worker take over",
                    );
                    if offline_resp.changed() {
                        self.worker.set_offline(offline, &mut self.settings);
                        self.draft.offline_mode = self.settings.offline_mode;
                        self.status_message = if offline {
                            "Offline mode — local worker takeover.".into()
                        } else {
                            "Offline mode disabled.".into()
                        };
                    }
                });
            });
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status_message).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(self.appliance_root.display().to_string())
                            .small()
                            .color(muted(self.settings.dark_mode)),
                    );
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            self.draw_status_card(ui);
            ui.add_space(12.0);
            self.draw_controls(ui);
            ui.add_space(12.0);
            self.draw_logs(ui);

            if self.show_settings {
                ui.add_space(16.0);
                self.draw_settings(ui);
            }
        });

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl ApplianceApp {
    fn draw_status_card(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (label, color) = match &self.worker.state {
                        WorkerState::Offline => ("OFFLINE/LOCAL", Color32::from_rgb(180, 140, 60)),
                        WorkerState::Local => ("LOCAL", Color32::from_rgb(210, 160, 70)),
                        WorkerState::FailingOver => ("FAILOVER", Color32::from_rgb(200, 120, 60)),
                        WorkerState::Running => ("CLOUD", Color32::from_rgb(70, 170, 110)),
                        WorkerState::Starting => ("STARTING", Color32::from_rgb(90, 140, 200)),
                        WorkerState::Stopped => ("STOPPED", Color32::from_rgb(140, 140, 140)),
                        WorkerState::Error(_) => ("ERROR", Color32::from_rgb(200, 80, 80)),
                    };
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 5.0, color);
                    ui.label(RichText::new(label).strong().color(color));
                    ui.label(format!("· worker “{}”", self.settings.appliance_name));
                    ui.label(
                        RichText::new(match self.worker.backend {
                            Backend::Cloud => "backend: cloud",
                            Backend::Local => "backend: local",
                            Backend::None => "backend: none",
                        })
                        .small()
                        .color(muted(self.settings.dark_mode)),
                    );
                });

                ui.add_space(6.0);
                ui.label(format!("Health: {}", self.worker.last_health));
                ui.label(format!(
                    "Cloud mgmt: http://{}/healthz",
                    self.settings.management_addr
                ));
                ui.label(format!(
                    "Local mgmt: http://{}/healthz",
                    self.settings.local_management_addr
                ));
                ui.label(format!("Checkout: {}", self.settings.worker_dir));
                if !self.worker.local_status.is_empty() {
                    ui.label(format!("Local: {}", self.worker.local_status));
                }
                if !path_exists(&self.settings.worker_dir) {
                    ui.colored_label(Color32::from_rgb(200, 80, 80), "Worker dir missing");
                }
                if let WorkerState::Error(err) = &self.worker.state {
                    ui.colored_label(Color32::from_rgb(200, 80, 80), err.clone());
                }
                if self.settings.local_failover {
                    ui.label(
                        RichText::new(
                            "Failover on: when the cloud worker stops, local worker takes over.",
                        )
                        .small()
                        .color(muted(self.settings.dark_mode)),
                    );
                }
                if self.settings.offline_mode {
                    ui.colored_label(
                        Color32::from_rgb(180, 140, 60),
                        "Offline mode: cloud disconnected; lightweight local worker is in charge.",
                    );
                }
            });
    }

    fn draw_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let offline = self.settings.offline_mode;
            let on_cloud = matches!(
                self.worker.state,
                WorkerState::Running | WorkerState::Starting
            );
            let on_local = matches!(
                self.worker.state,
                WorkerState::Local | WorkerState::Offline | WorkerState::FailingOver
            );

            ui.add_enabled_ui(!offline, |ui| {
                if ui
                    .button(RichText::new("Start cloud").strong())
                    .on_hover_text("Start the My Machines cloud worker")
                    .clicked()
                {
                    match self.worker.start_cloud(&self.settings) {
                        Ok(()) => self.status_message = "Starting cloud worker…".into(),
                        Err(err) => self.status_message = err,
                    }
                }
            });

            if ui
                .button("Start local")
                .on_hover_text("Start lightweight local worker now")
                .clicked()
            {
                match self.worker.start_local(&self.settings) {
                    Ok(()) => self.status_message = "Starting local worker…".into(),
                    Err(err) => self.status_message = err,
                }
            }

            ui.add_enabled_ui(on_cloud, |ui| {
                if ui
                    .button("Stop cloud")
                    .on_hover_text("Stop cloud; local worker takes over when failover is on")
                    .clicked()
                {
                    self.worker.stop_cloud(&self.settings);
                    self.status_message = "Cloud stopped — local takeover if enabled.".into();
                }
            });

            ui.add_enabled_ui(on_local && !offline, |ui| {
                if ui.button("Stop local").clicked() {
                    self.worker.stop_local();
                    self.worker.state = WorkerState::Stopped;
                    self.status_message = "Local worker stopped.".into();
                }
            });

            if ui.button("Stop all").clicked() {
                self.worker.stop_all();
                self.status_message = "All workers stopped.".into();
            }

            if ui.button("Refresh").clicked() {
                self.worker.tick(&self.settings);
                self.status_message = "Health refreshed.".into();
            }
        });
    }

    fn draw_logs(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Worker output").strong());
        egui::ScrollArea::vertical()
            .max_height(180.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let logs = self.worker.recent_logs();
                let text = if logs.is_empty() {
                    if self.worker.last_log.is_empty() {
                        "No worker output yet.".to_string()
                    } else {
                        self.worker.last_log.clone()
                    }
                } else {
                    logs
                };
                ui.label(
                    RichText::new(text)
                        .monospace()
                        .color(muted(self.settings.dark_mode)),
                );
            });
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Settings");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            self.show_settings = false;
                        }
                    });
                });
                ui.label(
                    RichText::new("Changes sync to data/gui-settings.json and .env")
                        .small()
                        .color(muted(self.settings.dark_mode)),
                );
                ui.add_space(8.0);

                egui::Grid::new("settings_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Appliance name");
                        ui.text_edit_singleline(&mut self.draft.appliance_name);
                        ui.end_row();

                        ui.label("Worker directory");
                        ui.text_edit_singleline(&mut self.draft.worker_dir);
                        ui.end_row();

                        ui.label("Cloud mgmt addr");
                        ui.text_edit_singleline(&mut self.draft.management_addr);
                        ui.end_row();

                        ui.label("Local mgmt addr");
                        ui.text_edit_singleline(&mut self.draft.local_management_addr);
                        ui.end_row();

                        ui.label("API key");
                        ui.horizontal(|ui| {
                            let edit = egui::TextEdit::singleline(&mut self.draft.api_key)
                                .password(!self.api_key_visible)
                                .desired_width(280.0);
                            ui.add(edit);
                            ui.checkbox(&mut self.api_key_visible, "Show");
                        });
                        ui.end_row();

                        ui.label("Idle release (sec)");
                        ui.add(
                            egui::DragValue::new(&mut self.draft.idle_release_timeout)
                                .speed(1.0)
                                .range(0..=86_400),
                        );
                        ui.end_row();

                        ui.label("Options");
                        ui.vertical(|ui| {
                            ui.checkbox(&mut self.draft.dark_mode, "Dark mode");
                            ui.checkbox(
                                &mut self.draft.offline_mode,
                                "Offline mode (local worker takeover)",
                            );
                            ui.checkbox(
                                &mut self.draft.local_failover,
                                "Auto local failover when cloud stops",
                            );
                            ui.checkbox(&mut self.draft.debug_worker, "Verbose worker debug");
                            ui.checkbox(
                                &mut self.draft.auto_start_worker,
                                "Auto-start cloud worker on GUI launch",
                            );
                        });
                        ui.end_row();
                    });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Save settings").strong()).clicked() {
                        self.apply_draft_settings();
                    }
                    if ui.button("Reset draft").clicked() {
                        self.draft = self.settings.clone();
                    }
                });
            });
    }
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    if dark {
        ctx.set_visuals(egui::Visuals::dark());
    } else {
        ctx.set_visuals(egui::Visuals::light());
    }
}

fn muted(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(160, 160, 160)
    } else {
        Color32::from_rgb(90, 90, 90)
    }
}
