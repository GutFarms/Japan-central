mod app;
mod process_util;
mod settings;
mod worker;

use app::ApplianceApp;
use worker::appliance_root_from_exe;

fn main() -> eframe::Result<()> {
    let appliance_root = appliance_root_from_exe();
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([820.0, 640.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("Cursor Appliance"),
        ..Default::default()
    };

    eframe::run_native(
        "Cursor Appliance",
        native_options,
        Box::new(move |cc| Ok(Box::new(ApplianceApp::new(cc, appliance_root)))),
    )
}
