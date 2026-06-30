//--+ file:///src/app/toolbar.rs
use super::matching::MergeMatching;
use super::palette::pal;
use super::state::MergeApp;
use super::types::StatusMessage;
use eframe::egui::*;
pub fn render_toolbar(app: &mut MergeApp, ctx: &Context) {
    TopBottomPanel::top("toolbar")
        .frame(
            Frame::none()
                .fill(pal::BG_TOOLBAR)
                .inner_margin(Margin::symmetric(8.0, 4.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(
                    RichText::new("patch·merge")
                        .color(pal::ACCENT_INFO)
                        .strong()
                        .monospace(),
                );
                ui.add(Separator::default().vertical().spacing(12.0));
                if ui.button("Open Patch…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Patch", &["md", "txt"])
                        .pick_file()
                    {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            app.patch_text = content;
                            if let Some(parent) = path.parent() {
                                app.base_dir = parent.display().to_string();
                            }
                            app.reparse();
                        }
                    }
                }
                if ui.button("Open File…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            app.file_text = content;
                            app.file_lines = app.file_text.lines().map(String::from).collect();
                            app.file_path = path.display().to_string();
                            if let Some(parent) = path.parent() {
                                app.base_dir = parent.display().to_string();
                            }
                            app.reset_for_new_file();
                            app.recompute_match();
                        }
                    }
                }
                ui.add(Separator::default().vertical().spacing(12.0));
                if let Some(ref mr) = app.match_result {
                    let (bg, fg, icon) = MergeApp::score_appearance(mr.score);
                    let frame = Frame::none()
                        .fill(bg)
                        .stroke(Stroke::new(1.0, fg))
                        .rounding(Rounding::same(4.0))
                        .inner_margin(Margin::symmetric(8.0, 3.0));
                    frame.show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{:.0}% {}", mr.score, icon))
                                .color(fg)
                                .strong()
                                .monospace(),
                        );
                    });
                    ui.add(Separator::default().vertical().spacing(12.0));
                }
                let (applied, pending, total) = app.hunk_summary();
                if total > 0 {
                    let frac = applied as f32 / total as f32;
                    let bar_w = 80.0_f32;
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 16.0), Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, pal::HUNK_PENDING);
                    let filled =
                        Rect::from_min_size(rect.min, Vec2::new(bar_w * frac, rect.height()));
                    ui.painter().rect_filled(filled, 3.0, pal::HUNK_APPLIED);
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        format!("{}/{}", applied, total),
                        FontId::monospace(10.0),
                        Color32::WHITE,
                    );
                    ui.label(
                        RichText::new(if pending == 0 { "all done" } else { "hunks" })
                            .color(pal::TEXT_DIM)
                            .small(),
                    );
                    ui.add(Separator::default().vertical().spacing(12.0));
                }
                let mut filter_low = app.filter_low_matches;
                if ui
                    .checkbox(&mut filter_low, "Filter <60%")
                    .on_hover_text("Skip and hide hunks with less than 60% match score")
                    .changed()
                {
                    app.filter_low_matches = filter_low;
                    if filter_low {
                        app.ensure_valid_filtered_hunk();
                    }
                }
                ui.add(Separator::default().vertical().spacing(12.0));
                let has_unsaved = !app.applied_hunks.is_empty();
                ui.add_enabled_ui(has_unsaved, |ui| {
                    if ui
                        .button(RichText::new("💾 Save").color(if has_unsaved {
                            pal::ACCENT_GOOD
                        } else {
                            pal::TEXT_DIM
                        }))
                        .on_hover_text("Save current file (Ctrl+S)")
                        .clicked()
                    {
                        app.save_file();
                    }
                });
                if ui
                    .button("💾 Save All")
                    .on_hover_text("Save every modified file")
                    .clicked()
                {
                    app.save_all_files();
                }
                ui.add(Separator::default().vertical().spacing(12.0));
                if ui.button("⚙").on_hover_text("Settings").clicked() {
                    app.show_settings = !app.show_settings;
                }
                if ui
                    .button("📂 Repos")
                    .on_hover_text("List and switch active repository")
                    .clicked()
                {
                    app.show_repos_window = !app.show_repos_window;
                }
                if ui.button("?").on_hover_text("Keyboard shortcuts").clicked() {
                    app.show_help = !app.show_help;
                }
            });
        });
}
