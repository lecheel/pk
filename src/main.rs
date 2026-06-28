use eframe::egui;

mod app;
mod diff;
mod patch;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut initial_patch = args.get(1).cloned();

    // If no argument is provided, look for impl.md or todo.md in the current directory
    if initial_patch.is_none() {
        for candidate in &["impl.md", "todo.md"] {
            if std::path::Path::new(candidate).exists() {
                initial_patch = Some(candidate.to_string());
                break;
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("Patch Merge GUI"),
        ..Default::default()
    };

    eframe::run_native(
        "Patch Merge GUI",
        options,
        Box::new(move |cc| Ok(Box::new(app::MergeApp::new(cc, initial_patch)))),
    )
}
