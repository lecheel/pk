use eframe::egui::*;

use super::palette::pal;
use super::state::MergeApp;

pub fn render_with_minimap(app: &mut MergeApp, ui: &mut Ui) {
    let available = ui.available_size();
    let minimap_w = 160.0_f32;

    ui.horizontal(|ui| {
        let (minimap_rect, _) =
            ui.allocate_exact_size(Vec2::new(minimap_w, available.y), Sense::hover());
        let mut minimap_ui = ui.child_ui(minimap_rect, Layout::top_down(Align::LEFT), None);
        render_minimap(app, &mut minimap_ui, minimap_w, available.y);

        ui.add(Separator::default().vertical());
        super::split_view::render_split_view(app, ui);
    });
}

pub fn render_minimap(app: &mut MergeApp, ui: &mut Ui, w: f32, h: f32) {
    Frame::none()
        .fill(Color32::from_rgb(18, 22, 28))
        .inner_margin(Margin::symmetric(6.0, 6.0))
        .show(ui, |ui| {
            ui.set_min_width(w - 12.0);
            ui.set_max_width(w - 12.0);

            ui.label(RichText::new("HUNKS").color(pal::TEXT_DIM).small().strong());
            ui.add_space(4.0);

            let n = app.hunks.len();
            if n == 0 {
                return;
            }

            let row_h = ((h - 60.0) / (n as f32 + 1.0)).clamp(18.0, 32.0);
            let mut jump_to: Option<usize> = None;

            for idx in 0..n {
                let is_current = idx == app.current_hunk;
                let is_applied = app.applied_hunks.contains(&idx);
                let hunk = &app.hunks[idx];

                let (bg, fg) = if is_current {
                    (Color32::from_rgb(30, 45, 75), pal::HUNK_CURRENT)
                } else if is_applied {
                    (Color32::from_rgb(18, 35, 22), pal::HUNK_APPLIED)
                } else {
                    (Color32::from_rgb(25, 28, 34), pal::TEXT_DIM)
                };

                let desired = Vec2::new(ui.available_width(), row_h);
                let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());

                if resp.hovered() {
                    ui.painter()
                        .rect_filled(rect, 3.0, Color32::from_rgb(35, 42, 58));
                } else {
                    ui.painter().rect_filled(rect, 3.0, bg);
                }

                let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                ui.painter().rect_filled(bar, 0.0, fg);

                ui.painter().text(
                    Pos2::new(rect.left() + 8.0, rect.center().y - 5.0),
                    Align2::LEFT_CENTER,
                    format!("{}", idx + 1),
                    FontId::monospace(10.0),
                    if is_current { Color32::WHITE } else { fg },
                );

                let icon = if is_applied {
                    "✓"
                } else if is_current {
                    "▶"
                } else {
                    "○"
                };
                ui.painter().text(
                    Pos2::new(rect.left() + 8.0, rect.center().y + 6.0),
                    Align2::LEFT_CENTER,
                    icon,
                    FontId::monospace(9.0),
                    fg,
                );

                let fname = std::path::Path::new(&hunk.filename)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&hunk.filename);
                let max_fname = ((w - 30.0) / 6.5).floor() as usize;
                ui.painter().text(
                    Pos2::new(rect.left() + 22.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    MergeApp::truncate_owned(fname, max_fname),
                    FontId::monospace(9.5),
                    if is_current {
                        pal::TEXT_NORMAL
                    } else {
                        Color32::from_gray(120)
                    },
                );

                if resp.clicked() {
                    jump_to = Some(idx);
                }

                if resp.hovered() {
                    resp.on_hover_ui_at_pointer(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "#{} {}{}",
                                idx + 1,
                                hunk.filename,
                                if is_applied { " ✓ applied" } else { "" }
                            ))
                            .monospace()
                            .small(),
                        );
                        ui.label(format!(
                            "search {} ln  →  replace {} ln",
                            hunk.search.len(),
                            hunk.replace.len()
                        ));
                    });
                }
            }

            if let Some(idx) = jump_to {
                app.current_hunk = idx;
                app.load_hunk();
            }
        });
}
