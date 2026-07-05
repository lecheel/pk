use super::chat::ChatMode;
use super::palette::pal;
use super::state::MergeApp;
use eframe::egui::*;
pub fn render_status_bar(app: &mut MergeApp, ctx: &Context) {
    TopBottomPanel::bottom("status")
        .frame(
            Frame::none()
                .fill(pal::BG_TOOLBAR)
                .inner_margin(Margin::symmetric(8.0, 3.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // --- MODIFIED / SAVED LED INDICATOR ---
                let is_modified = !app.history.is_empty();
                let (led_color, led_text) = if is_modified {
                    (pal::ACCENT_WARN, "Modified")
                } else {
                    (pal::ACCENT_GOOD, "Saved")
                };

                // Draw an actual circle for the LED to avoid missing font glyphs (square boxes)
                let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), Sense::hover());
                ui.painter()
                    .circle_filled(rect.center(), rect.height() / 2.0, led_color);
                ui.label(
                    RichText::new(led_text)
                        .color(led_color)
                        .strong()
                        .monospace()
                        .size(13.0),
                );
                ui.add(Separator::default().vertical());

                if let Some(hunk) = app.current_hunk() {
                    ui.label(
                        RichText::new(format!("📄 {}", hunk.filename))
                            .color(pal::TEXT_NORMAL)
                            .monospace()
                            .size(13.0),
                    );
                    if let Some(ref mr) = app.match_result {
                        ui.add(Separator::default().vertical());
                        ui.label(
                            RichText::new(format!(
                                "match {}-{}  |  search {} ln  |  replace {} ln",
                                mr.file_start + 1,
                                mr.file_end,
                                hunk.search.len(),
                                hunk.replace.len()
                            ))
                            .color(pal::TEXT_DIM)
                            .monospace()
                            .size(13.0),
                        );
                    }
                }
                if let Some(line) = app.cursor_line {
                    ui.add(Separator::default().vertical());
                    ui.label(
                        RichText::new(format!("ln {}  of {}", line + 1, app.file_lines.len()))
                            .color(pal::TEXT_DIM)
                            .monospace()
                            .size(13.0),
                    );
                }
                if !app.file_anchors.is_empty() {
                    let labels: Vec<String> =
                        app.file_anchors.values().map(|f| f.label()).collect();
                    ui.add(Separator::default().vertical());
                    ui.label(
                        RichText::new(format!("⚓ {}", labels.join("  ")))
                            .color(pal::TEXT_ANCHOR)
                            .monospace()
                            .size(13.0),
                    );
                }
                if app.mark_pending.is_some() {
                    ui.add(Separator::default().vertical());
                    ui.label(
                        RichText::new("m>_")
                            .color(pal::ACCENT_WARN)
                            .monospace()
                            .strong()
                            .size(13.0),
                    );
                }
                if !app.vim_buffer.is_empty() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("  {}█", app.vim_buffer))
                                .color(Color32::from_rgb(200, 200, 100))
                                .monospace()
                                .size(13.0),
                        );
                    });
                }
                if let Some(ref msg) = app.message {
                    ui.add(Separator::default().vertical());
                    ui.label(RichText::new(&msg.text).color(msg.color()).size(13.0));
                }

                if app.show_chat_window {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let provider = app.current_chat_provider();
                        ui.label(
                            RichText::new(format!("{} / {}", provider.name(), provider.model))
                                .color(pal::TEXT_NORMAL)
                                .monospace()
                                .size(13.0),
                        );
                        ui.add(Separator::default().vertical());
                        
                        for mode in [ChatMode::Impl, ChatMode::Commit, ChatMode::Chat] {
                            let is_active = app.chat_mode == mode;
                            let rich_text = RichText::new(mode.short_label())
                                .color(if is_active { mode.color() } else { pal::TEXT_DIM })
                                .strong()
                                .size(13.0);
                            if ui.selectable_label(is_active, rich_text).clicked() {
                                app.chat_mode = mode;
                            }
                            ui.label(RichText::new("·").color(pal::TEXT_DIM).size(13.0));
                        }

                        ui.label(
                            RichText::new("Chat Tab:")
                                .color(pal::TEXT_DIM)
                                .monospace()
                                .size(13.0),
                        );
                    });
                }
            });
        });
}