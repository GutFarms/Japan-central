use chrono::{DateTime, FixedOffset, Utc};
use eframe::{self, egui, App};
use std::time::Duration;

struct ClockApp {
    japan_time: String,
    central_time: String,
}

impl Default for ClockApp {
    fn default() -> Self {
        Self {
            japan_time: get_japan_time(),
            central_time: get_central_time(),
        }
    }
}

impl App for ClockApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update the time every second
        self.japan_time = get_japan_time();
        self.central_time = get_central_time();

        // UI Layout
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("World Clock");
            ui.label("Current Time in Japan (JST):");
            ui.label(&self.japan_time);
            ui.separator();
            ui.label("Current Time in Central Time (CT):");
            ui.label(&self.central_time);
        });

        // Request a repaint every second to keep time updated
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

fn get_japan_time() -> String {
    let jst_offset = FixedOffset::east_opt(9 * 3600).expect("Invalid offset for JST");
    let jst_time: DateTime<FixedOffset> = Utc::now().with_timezone(&jst_offset);
    jst_time.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn get_central_time() -> String {
    let ct_offset = FixedOffset::west_opt(6 * 3600).expect("Invalid offset for CT");
    let ct_time: DateTime<FixedOffset> = Utc::now().with_timezone(&ct_offset);
    ct_time.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn main() {
    let app = ClockApp::default();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(" JC Time: Japan & Central Time", native_options, Box::new(|_| Ok(Box::new(app))));
}