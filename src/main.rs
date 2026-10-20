#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let logo = image::load_from_memory(include_bytes!("../eder-landing/assets/eder-mark.png"))
        .expect("embedded Eder logo must be a valid PNG")
        .to_rgba8();
    let (width, height) = logo.dimensions();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([360.0, 420.0])
            .with_icon(std::sync::Arc::new(eframe::egui::IconData {
                rgba: logo.into_raw(),
                width,
                height,
            })),
        ..Default::default()
    };
    eframe::run_native(
        "Eder — PDF Library",
        options,
        Box::new(|cc| Ok(Box::new(eder::desktop::Eder::new(cc)))),
    )
}
