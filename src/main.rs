mod api;
mod app;
mod installer;
mod launcher;
mod logger; // 1. Add module declaration
mod models;
mod updater;

use app::CmManagerApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // 2. Initialize logger
    logger::init("cm_manager.log");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([750.0, 500.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ChroMapper Version Manager",
        options,
        Box::new(|_cc| Box::<CmManagerApp>::default()),
    )
}