#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([360.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Folio — PDF Library",
        options,
        Box::new(|cc| Ok(Box::new(app::Folio::new(cc)))),
    )
}
