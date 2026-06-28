use eframe::egui::*;

use super::matching::MergeMatching;
use super::palette::pal;
use super::state::MergeApp;

pub fn render_split_view(app: &mut MergeApp, ui: &mut Ui) {
    let mr = match app.match_result.clone() {
        Some(m) => m,
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("No file loaded or no match found.")
                        .color(Color32::from_gray(140)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Open a file or patch above")
                        .color(pal::TEXT_DIM)
                        .small(),
                );
            });
            return;
        }
    };

    let available = ui.available_size();
    let divider = 0.38_f32;
    let left_w = (available.x * divider).floor() - 1.0;
    let _right_w = available.x - left_w - 2.0;
    let _mono_h = ui.text_style_height(&TextStyle::Monospace);
    let _row_h = _mono_h + 4.0;
    let _char_w = _mono_h * 0.60;

    // Panel header row
    ui.horizontal(|ui| {
        Frame::none()
            .fill(Color32::from_rgb(28, 38, 58))
            .inner_margin(Margin::symmetric(8.0, 3.0))
            .show(ui, |ui| {
                ui.set_min_width(left_w);
                ui.set_max_width(left_w);
                let hunk = app.current_hunk().unwrap();
                ui.label(
                    RichText::new(format!("SEARCH  ·  {}", hunk.filename))
                        .color(Color32::from_rgb(120, 180, 255))
                        .strong()
                        .monospace(),
                );
            });
        ui.add_space(2.0);
        Frame::none()
            .fill(Color32::from_rgb(28, 45, 35))
            .inner_margin(Margin::symmetric(8.0, 3.0))
            .show(ui, |ui| {
                // FILE header content - continue from truncated original
            });
    });

    // Note: The original file was truncated here. Complete the implementation
    // with the remaining split view rendering logic (file panel, scroll areas, etc.)
}
