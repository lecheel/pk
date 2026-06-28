use eframe::egui;

mod app;
mod diff;
mod patch;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("Patch Merge GUI"),
        ..Default::default()
    };

    eframe::run_native(
        "Patch Merge GUI",
        options,
        Box::new(|cc| Ok(Box::new(app::MergeApp::new(cc)))),
    )
}
