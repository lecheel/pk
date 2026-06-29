// file: src/app/help.rs
use super::palette::pal;
use super::state::MergeApp;
use eframe::egui::*;

pub fn render_help_overlay(app: &mut MergeApp, ctx: &Context) {
    let screen = ctx.screen_rect();
    let overlay_w = 440.0_f32;
    let overlay_h = 420.0_f32;
    let pos = Pos2::new(
        (screen.center().x - overlay_w / 2.0).max(8.0),
        (screen.center().y - overlay_h / 2.0).max(8.0),
    );
    let overlay_rect = Rect::from_min_size(pos, Vec2::new(overlay_w, overlay_h));
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("help_overlay")));
    painter.rect_filled(screen, 0.0, Color32::from_black_alpha(160));
    painter.rect_filled(overlay_rect, 6.0, Color32::from_rgb(22, 28, 38));
    painter.rect_stroke(overlay_rect, 6.0, Stroke::new(1.0, pal::SEPARATOR));

    Area::new(Id::new("help_area"))
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.set_min_size(Vec2::new(overlay_w, overlay_h));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Keyboard shortcuts")
                        .color(pal::ACCENT_INFO)
                        .strong()
                        .heading(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(12.0);
                    if ui.button("✕").clicked() {
                        app.show_help = false;
                    }
                });
            });
            ui.add_space(8.0);
            ui.add(Separator::default());
            ui.add_space(4.0);

            let shortcuts: &[(&str, &str)] = &[
                ("Navigation", ""),
                ("↑ / ↓",       "Move cursor one line"),
                ("PgUp / PgDn", "Move cursor 10 lines"),
                ("Home / End",  "Jump to first / last line"),
                ("gg / G",      "Jump to top / bottom (vim)"),
                ("", ""),
                ("Hunk control", ""),
                ("L",           "Next hunk"),
                ("Shift+L",     "Previous hunk"),
                ("◀ ▶ (toolbar)", "Navigate candidates"),
                ("", ""),
                ("File-panel marks  (right buffer)", ""),
                ("Space",       "Quick set ma at cursor (point insert)"),
                ("ma",          "Set mark-a (start of replace range)"),
                ("mb",          "Set mark-b (end of replace range, inclusive)"),
                ("",            "  → selects lines [ma, mb] in right buffer"),
                ("",            "  → links to left-panel selection via > button"),
                ("Esc",         "Clear both marks / cancel pending 'm'"),
                ("", ""),
                ("Editing", ""),
                ("A",           "Apply current hunk (cursor in match or on ma)"),
                ("> (toolbar)", "Apply replace at ma/mb (or left selection)"),
                ("dd / Ndd",    "Delete 1 or N lines at cursor"),
                ("u",           "Undo last edit"),
                (".",           "Repeat last action"),
                ("", ""),
                ("Search", ""),
                ("/",           "Enter file search"),
                ("n / N",       "Next / previous search match"),
                ("", ""),
                ("UI", ""),
                ("?",           "Toggle this help"),
                ("o",           "Toggle hunk minimap"),
            ];

            ScrollArea::vertical()
                .max_height(overlay_h - 110.0)
                .show(ui, |ui| {
                    for (key, desc) in shortcuts {
                        if desc.is_empty() {
                            if !key.is_empty() {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(*key).color(pal::TEXT_DIM).small().strong(),
                                );
                            } else {
                                ui.add_space(2.0);
                            }
                        } else {
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                let key_rect = ui
                                    .allocate_exact_size(Vec2::new(140.0, 18.0), Sense::hover());
                                ui.painter_at(key_rect.0).text(
                                    key_rect.0.left_center(),
                                    Align2::LEFT_CENTER,
                                    *key,
                                    FontId::monospace(11.0),
                                    Color32::from_rgb(180, 210, 255),
                                );
                                ui.label(RichText::new(*desc).color(pal::TEXT_NORMAL).small());
                            });
                        }
                    }
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Esc or click ✕ to close")
                        .color(pal::TEXT_DIM)
                        .small(),
                );
            });
        });
}
