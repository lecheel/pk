use super::palette::pal;
use super::state::MergeApp;
use eframe::egui::*;

pub fn render_help_overlay(app: &mut MergeApp, ctx: &Context) {
    let screen = ctx.screen_rect();
    let overlay_w = 460.0_f32.min(screen.width() - 16.0);
    let overlay_h = 440.0_f32.min(screen.height() - 16.0);
    let pos = Pos2::new(
        (screen.center().x - overlay_w / 2.0).max(8.0),
        (screen.center().y - overlay_h / 2.0).max(8.0),
    );
    let overlay_rect = Rect::from_min_size(pos, Vec2::new(overlay_w, overlay_h));
    let mut open = app.show_help;
    Window::new("⌨ Keyboard shortcuts")
        .open(&mut open)
        .fixed_rect(overlay_rect)
        .collapsible(false)
        .resizable(false)
        .frame(
            Frame::none()
                .fill(Color32::from_rgb(22, 28, 38))
                .inner_margin(Margin::symmetric(16.0, 12.0))
                .rounding(6.0)
                .stroke(Stroke::new(1.0, pal::SEPARATOR)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Keyboard shortcuts")
                        .color(pal::ACCENT_INFO)
                        .strong()
                        .heading(),
                );
            });
            ui.add_space(8.0);
            ui.add(Separator::default());
            ui.add_space(4.0);
            let shortcuts: &[(&str, &str)] = &[
                ("Navigation", ""),
                ("↑ / ↓", "Move cursor one line"),
                ("PgUp / PgDn", "Move cursor 10 lines"),
                ("Home / End", "Move to start / end of current line"),
                ("gg / G", "Jump to top / bottom (vim)"),
                ("]h / [h", "Jump to next / previous git hunk"),
                ("Alt+= / Alt+-", "Jump to next / previous file"),
                ("", ""),
                ("Hunk control", ""),
                ("l", "Next hunk"),
                ("L", "Previous hunk"),
                ("◀ ▶ (toolbar)", "Navigate candidates"),
                ("", ""),
                ("File-panel marks  (right buffer)", ""),
                ("Space", "Quick set ma at cursor (point insert)"),
                ("mb", "Set mark-b (start of replace range)"),
                ("mB", "Set mark-B (end of replace range, inclusive)"),
                ("", "  → selects lines [mb, mB] in right buffer"),
                ("", "  → links to left-panel selection via > button"),
                ("Esc", "Clear both marks / cancel pending 'm'"),
                ("", ""),
                ("Git Diff Display", ""),
                ("F4", "Toggle full git diff window"),
                ("Mouse Hover", "Hover modified line to show diff"),
                ("", ""),
                ("Editing", ""),
                ("a", "Apply current hunk (cursor in match or on ma)"),
                ("> (toolbar)", "Apply replace at ma/mb (or left selection)"),
                ("dd / Ndd", "Delete 1 or N lines at cursor"),
                ("u", "Undo last edit"),
                (".", "Repeat last action"),
                ("Alt+w", "Save buffer to file"),
                ("w", "Save all files"),
                ("", ""),
                ("Search", ""),
                ("/", "Enter file search"),
                ("n / N", "Next / previous search match"),
                ("", ""),
                ("Patch", ""),
                ("*", "Paste patch from clipboard"),
                ("", ""),
                ("UI", ""),
                ("?", "Toggle this help"),
            ];
            ScrollArea::vertical()
                .max_height(overlay_h - 100.0)
                .show(ui, |ui| {
                    ui.set_min_width(overlay_w - 40.0);
                    ui.set_max_width(overlay_w - 40.0);

                    for (key, desc) in shortcuts {
                        if desc.is_empty() {
                            if !key.is_empty() {
                                ui.add_space(6.0);
                                ui.label(RichText::new(*key).color(pal::TEXT_DIM).small().strong());
                            } else {
                                ui.add_space(2.0);
                            }
                        } else {
                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                ui.add_sized(
                                    [120.0, 18.0],
                                    Label::new(
                                        RichText::new(*key)
                                            .color(Color32::from_rgb(180, 210, 255))
                                            .monospace()
                                            .size(11.0),
                                    ),
                                );
                                ui.add(
                                    Label::new(
                                        RichText::new(*desc).color(pal::TEXT_NORMAL).small(),
                                    )
                                    .wrap(),
                                );
                            });
                        }
                    }
                });
        });
    app.show_help = open;
}
