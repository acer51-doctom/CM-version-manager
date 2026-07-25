mod api;
mod app;
mod installer;
mod launcher; // Add this!
mod models;
mod updater;

use app::CmManagerApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([750.0, 500.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ChroMapper Version Manager",
        options,
        Box::new(|_cc| Ok(Box::<CmManagerApp>::default())),
    )
}