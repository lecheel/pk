use eframe::egui::*;

use super::palette::pal;
use super::state::MergeApp;

pub fn render_status_bar(app: &MergeApp, ctx: &Context) {
    TopBottomPanel::bottom("status")
        .frame(
            Frame::none()
                .fill(pal::BG_TOOLBAR)
                .inner_margin(Margin::symmetric(8.0, 3.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(hunk) = app.current_hunk() {
                    ui.label(
                        RichText::new(format!("📄 {}", hunk.filename))
                            .color(pal::TEXT_NORMAL)
                            .monospace(),
                    );
                    if let Some(ref mr) = app.match_result {
                        ui.add(Separator::default().vertical());
                        ui.label(
                            RichText::new(format!(
                                "match {}–{}  │  search {} ln  │  replace {} ln",
                                mr.file_start + 1,
                                mr.file_end,
                                hunk.search.len(),
                                hunk.replace.len()
                            ))
                            .color(pal::TEXT_DIM)
                            .monospace()
                            .small(),
                        );
                    }
                }

                if let Some(line) = app.cursor_line {
                    ui.add(Separator::default().vertical());
                    ui.label(
                        RichText::new(format!("ln {}  of {}", line + 1, app.file_lines.len()))
                            .color(pal::TEXT_DIM)
                            .monospace()
                            .small(),
                    );
                }

                if !app.vim_buffer.is_empty() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("  {}█", app.vim_buffer))
                                .color(Color32::from_rgb(200, 200, 100))
                                .monospace(),
                        );
                    });
                }

                if let Some(ref msg) = app.message {
                    ui.add(Separator::default().vertical());
                    ui.label(RichText::new(&msg.text).color(msg.color()).small());
                }
            });
        });
}
