use super::aux_panels::{
    render_debug_panel, render_fmt_error_panel, render_repos_panel, render_settings_panel,
    render_welcome_panel,
};
use super::chat::{self, ChatMode};
use super::clipboard_utils::{get_clipboard_text, parse_clipboard_patch};
use super::file_panel::render_file_panel;
use super::git_diff_side::render_git_diff_side_panel;
use super::git_panels::{
    render_git_commit_detail_panel, render_git_diff_panel, render_git_log_panel,
    render_git_status_panel,
};
use super::matching::MergeMatching;
use super::palette::pal;
use super::search_panel::render_search_panel;
use super::state::MergeApp;
use super::types::StatusMessage;
use crate::diff::RowKind;
use eframe::egui::*;

pub fn render_split_view(app: &mut MergeApp, ui: &mut Ui) {
    if app.show_llm_config {
        super::chat::render_llm_config_panel(app, ui);
        return;
    }
    let mr = match app.match_result.clone() {
        Some(m) => m,
        None => crate::diff::MatchResult {
            score: 0.0,
            file_start: 0,
            file_end: 0,
            rows: vec![],
            candidates: vec![],
        },
    };
    let available = ui.available_size();
    let divider = 0.38_f32;
    let left_w = (available.x * divider).floor() - 1.0;
    let right_w = available.x - left_w - 2.0;
    let row_font = ui
        .style()
        .text_styles
        .get(&TextStyle::Monospace)
        .cloned()
        .unwrap_or(FontId::monospace(11.0));
    let mono_h = ui.fonts(|f| f.row_height(&row_font));
    let row_h = mono_h + 4.0;
    let char_w = ui.fonts(|f| {
        let w1 = f
            .layout_no_wrap("0".to_string(), row_font.clone(), Color32::WHITE)
            .rect
            .width();
        let w2 = f
            .layout_no_wrap("00".to_string(), row_font.clone(), Color32::WHITE)
            .rect
            .width();
        w2 - w1
    });

    ui.horizontal(|ui| {
        Frame::none()
            .fill(pal::BG_PANEL)
            .inner_margin(Margin::symmetric(4.0, 2.0))
            .show(ui, |ui| {
                ui.set_min_width(left_w);
                ui.set_max_width(left_w);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.spacing_mut().button_padding = Vec2::new(4.0, 2.0);

                    let current_tab = if app.show_fmt_error && app.fmt_error.is_some() {
                        "Error"
                    } else if app.show_settings {
                        "Settings"
                    } else if app.show_repos_window {
                        "Repos"
                    } else if app.show_debug {
                        "Debug"
                    } else if app.show_git_status_window {
                        "Git Status"
                    } else if app.show_git_diff_side {
                        "Git Diff Side"
                    } else if app.show_git_diff_window {
                        "Git Diff"
                    } else if app.show_git_log_window {
                        "Git Log"
                    } else if app.show_chat_window && !app.disable_llm {
                        "Chat"
                    } else {
                        "Search"
                    };
                    let show_repos = app.concat_server_enabled;
                    let all_tabs = [
                        ("🔍 Search", "Search", Color32::from_rgb(120, 180, 255)),
                        ("Status(F1)", "Git Status", Color32::from_rgb(120, 230, 160)),
                        ("Diff(F2)", "Git Diff", Color32::from_rgb(235, 120, 120)),
                        (
                            "Diff Side(F3)",
                            "Git Diff Side",
                            Color32::from_rgb(100, 210, 220),
                        ),
                        ("Log/Commit(F4)", "Git Log", Color32::from_rgb(180, 130, 230)),
                        ("Repos", "Repos", Color32::from_rgb(120, 230, 160)),
                        ("Chat(F9)", "Chat", Color32::from_rgb(180, 140, 255)),
                    ];
                    let tabs: Vec<(&str, &str, Color32)> = all_tabs
                        .iter()
                        .cloned()
                        .filter(|t| t.1 != "Repos" || show_repos)
                        .filter(|t| t.1 != "Chat" || !app.disable_llm)
                        .collect();

                    if app.fmt_error.is_some() {
                        let is_active = current_tab == "Error";
                        let rich_text = RichText::new("⚠ Error")
                            .color(if is_active {
                                pal::ACCENT_BAD
                            } else {
                                pal::TEXT_DIM
                            })
                            .strong()
                            .size(12.0);
                        if ui.selectable_label(is_active, rich_text).clicked() {
                            app.show_fmt_error = true;
                            app.show_settings = false;
                            app.show_repos_window = false;
                            app.show_debug = false;
                            app.show_git_status_window = false;
                            app.show_git_diff_window = false;
                            app.show_git_log_window = false;
                        }
                    }

                    for (label, tab_name, color) in tabs.iter() {
                        let is_active = current_tab == *tab_name;
                        let rich_text = RichText::new(*label)
                            .color(if is_active { *color } else { pal::TEXT_DIM })
                            .strong()
                            .size(12.0);
                        if ui.selectable_label(is_active, rich_text).clicked() {
                            app.show_fmt_error = false;
                            app.show_settings = false;
                            app.show_repos_window = false;
                            app.show_debug = false;
                            app.show_git_status_window = false;
                            app.show_git_diff_window = false;
                            app.show_git_diff_side = false;
                            app.show_git_log_window = false;
                            app.show_git_commit_window = false;
                            app.show_chat_window = false;
                            match *tab_name {
                                "Settings" => app.show_settings = true,
                                "Repos" => app.show_repos_window = true,
                                "Debug" => app.show_debug = true,
                                "Git Status" => app.show_git_status_window = true,
                                "Git Diff" => app.show_git_diff_window = true,
                                "Git Diff Side" => {
                                    app.show_git_diff_side = true;
                                    app.refresh_git_changed_files();
                                }
                                "Git Log" => {
                                    app.show_git_log_window = true;
                                    app.git_log_entries = super::git_ops::get_git_log(
                                        std::path::Path::new(&app.base_dir),
                                    );
                                }
                                "Chat" => {
                                    app.show_chat_window = true;
                                }
                                _ => {}
                            }
                        }
                    }
                });
            });
        ui.add_space(2.0);
        Frame::none()
            .fill(Color32::from_rgb(28, 45, 35))
            .inner_margin(Margin::symmetric(8.0, 3.0))
            .show(ui, |ui| {
                ui.set_min_width(right_w);
                ui.horizontal(|ui| {
                    let mark_label = if app.file_anchors.is_empty() {
                        String::new()
                    } else {
                        let labels: Vec<String> =
                            app.file_anchors.values().map(|f| f.label()).collect();
                        format!("  ·  {}", labels.join("  "))
                    };
                    let file_header_text = if app.file_lines.is_empty() {
                        "FILE  ·  no file loaded".to_string()
                    } else {
                        let match_info = if !app.file_search_query.is_empty() {
                            format!("  ({} matches)", app.search_matches.len())
                        } else {
                            String::new()
                        };
                        format!(
                            "FILE  ·  {} lines  ·  match @ {}–{}{}{}",
                            app.file_lines.len(),
                            mr.file_start + 1,
                            mr.file_end,
                            mark_label,
                            match_info,
                        )
                    };

                    // Reflects the loading state of whichever chat mode is
                    // currently active (Chat/Commit/Impl each have their own
                    // session now), so this header spinner tracks the tab
                    // the user is actually looking at.
                    let active_mode = app.chat_mode.clone();
                    let active_is_loading = app.chat_sessions.get_mut(&active_mode).is_loading;
                    let right_reserved = if active_is_loading { 150.0 } else { 0.0 };
                    let available = ui.available_width().max(right_reserved) - right_reserved;

                    // Allocate space for the left side so it doesn't push the right side off-screen
                    ui.allocate_ui_with_layout(
                        Vec2::new(available, ui.available_height()),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new(file_header_text)
                                    .color(Color32::from_rgb(120, 220, 160))
                                    .strong()
                                    .monospace(),
                            );
                        },
                    );

                    if active_is_loading {
                        let session = app.chat_sessions.get_mut(&active_mode);
                        if session.start_time.is_none() {
                            session.start_time = Some(ui.ctx().input(|i| i.time));
                        }
                        let elapsed = ui.ctx().input(|i| i.time)
                            - app
                                .chat_sessions
                                .get_mut(&active_mode)
                                .start_time
                                .unwrap_or_default();
                        ui.allocate_ui_with_layout(
                            Vec2::new(right_reserved, ui.available_height()),
                            Layout::right_to_left(Align::Center),
                            |ui| {
                                if ui.button("⏹ Cancel").clicked() {
                                    app.cancel_llm();
                                }
                                ui.label(
                                    RichText::new(format!("{:.1}s", elapsed))
                                        .color(pal::TEXT_DIM)
                                        .small(),
                                );
                                ui.spinner();
                            },
                        );
                        ui.ctx().request_repaint();
                    } else {
                        app.chat_sessions.get_mut(&active_mode).start_time = None;
                    }
                });
            });
    });
    ui.add(Separator::default());
    let body_rect = ui.available_rect_before_wrap();
    // Full-width tabs bypass the left/right split entirely.
    if app.show_git_diff_side {
        let mut full_ui = ui.child_ui(body_rect, Layout::top_down(Align::LEFT), None);
        render_git_diff_side_panel(app, &mut full_ui, row_h, char_w, body_rect.width());
        return;
    }
    let mut left_rect = body_rect;
    left_rect.set_width(left_w);
    let mut right_rect = body_rect;
    right_rect.min.x = body_rect.min.x + left_w + 2.0;
    right_rect.set_width(right_w);
    let mut left_ui = ui.child_ui(left_rect, Layout::top_down(Align::LEFT), None);
    if app.show_fmt_error && app.fmt_error.is_some() {
        render_fmt_error_panel(app, &mut left_ui);
    } else if app.show_settings {
        render_settings_panel(app, &mut left_ui);
    } else if app.show_repos_window {
        render_repos_panel(app, &mut left_ui);
    } else if app.show_debug {
        render_debug_panel(app, &mut left_ui);
    } else if app.show_chat_window && !app.disable_llm {
        if app.show_system_prompt {
            if let Some(prompt) = app.active_system_prompt() {
                let mode_color = app.chat_mode.color();
                let mode_label = app.chat_mode.short_label();
                let mut close_prompt = false;
                Frame::none()
                    .fill(Color32::from_rgb(20, 28, 40))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(50, 70, 100)))
                    .rounding(4.0)
                    .inner_margin(Margin::symmetric(10.0, 6.0))
                    .show(&mut left_ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("🤖 {} System Prompt:", mode_label))
                                    .color(mode_color)
                                    .strong()
                                    .monospace()
                                    .size(11.0),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add(
                                        Button::new(
                                            RichText::new("✕").color(pal::TEXT_DIM).small(),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    close_prompt = true;
                                }
                            });
                        });
                        ui.add_space(2.0);
                        ScrollArea::vertical()
                            .id_source("left_panel_prompt_scroll")
                            .max_height(120.0)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(&prompt)
                                        .color(pal::TEXT_NORMAL)
                                        .monospace()
                                        .size(11.0),
                                );
                            });
                    });
                left_ui.add(Separator::default());
                if close_prompt {
                    app.show_system_prompt = false;
                }
            } else {
                app.show_system_prompt = false;
            }
        }
        chat::render_chat_panel(app, &mut left_ui, left_w);
    } else if app.show_git_status_window {
        render_git_status_panel(app, &mut left_ui, row_h, char_w, left_w);
    } else if app.show_git_diff_window {
        render_git_diff_panel(app, &mut left_ui, row_h, char_w, left_w);
    } else if app.show_git_log_window {
        render_git_log_panel(app, &mut left_ui, row_h, char_w, left_w);
    } else {
        render_search_panel(app, &mut left_ui, &mr, row_h, char_w, left_w, &row_font);
    }

    let mut right_ui = ui.child_ui(right_rect, Layout::top_down(Align::LEFT), None);
    if app.show_git_log_window {
        render_git_commit_detail_panel(app, &mut right_ui, right_w);
    } else if app.hunks.is_empty() && app.file_lines.is_empty() {
        right_ui.horizontal(|ui| {
            ui.add_space(40.0); // Left indentation
            ui.vertical(|ui| {
                const BODY_SIZE: f32 = 14.0;
                let body = |s: &str| RichText::new(s).size(BODY_SIZE).color(pal::TEXT_NORMAL);
                let key = |s: &str| {
                    RichText::new(s)
                        .size(BODY_SIZE)
                        .color(Color32::from_rgb(120, 220, 160))
                        .strong()
                };

                ui.add_space(60.0);
                ui.heading("Welcome to PCode Merge");
                ui.add_space(20.0);
                ui.label(
                    RichText::new("Workflow Guide")
                        .size(BODY_SIZE)
                        .color(pal::TEXT_NORMAL)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(body("1. Click '📋 Paste Patch', press '"));
                    ui.label(key("*"));
                    ui.label(body("', or '📝 Paste Manually' on the left"));
                });
                ui.label(body(
                    "2. Enter the target file path in the 'Target File' box",
                ));
                ui.horizontal(|ui| {
                    ui.label(body("3. Use '"));
                    ui.label(key("l / L"));
                    ui.label(body("' to navigate between hunks"));
                });
                ui.horizontal(|ui| {
                    ui.label(body("4. Press '"));
                    ui.label(key("a"));
                    ui.label(body("' or click ⚡ Apply to merge the hunk"));
                });
                ui.horizontal(|ui| {
                    ui.label(body("5. Press '"));
                    ui.label(key("Alt+w"));
                    ui.label(body("' to save the current file to disk"));
                });
                ui.horizontal(|ui| {
                    ui.label(body("6. Press '"));
                    ui.label(key("w"));
                    ui.label(body("' to save all modified files to disk"));
                });
                ui.add_space(30.0);
                ui.label(
                    RichText::new("Patch Style Prompt")
                        .size(BODY_SIZE)
                        .color(pal::TEXT_NORMAL)
                        .strong(),
                );
                ui.add_space(4.0);

                let prompt_text =
                    "Please apply changes using this style format in single code block:
```                
// src/filename1
<<<<<<< SEARCH
[exact original lines (include enough context to be unique, avoid too thin blocks)]
=======
[modified lines]
>>>>>>> REPLACE

// src/filename2
<<<<<<< SEARCH
[exact original lines (include enough context to be unique, avoid too thin blocks)]
=======
[modified lines]
>>>>>>> REPLACE
```";

                let do_copy = |ui: &Ui, app: &mut MergeApp| {
                    ui.ctx().copy_text(prompt_text.to_string());
                    app.set_message(StatusMessage::success("Patch prompt copied to clipboard!"));
                };

                ui.horizontal(|ui| {
                    ui.label(body("Instruct your AI to use this format:"));
                    let copy_btn =
                        ui.add(Button::new(RichText::new("📋 Copy").size(BODY_SIZE)).small());
                    if copy_btn.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if copy_btn
                        .on_hover_text("Click to copy, or double-click the box below")
                        .clicked()
                    {
                        do_copy(ui, app);
                    }
                });
                ui.add_space(8.0);
                Frame::none()
                    .fill(pal::BG_PANEL)
                    .inner_margin(Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        let label = Label::new(
                            RichText::new(prompt_text)
                                .monospace()
                                .size(BODY_SIZE)
                                .color(pal::TEXT_NORMAL),
                        )
                        .wrap()
                        .sense(Sense::click());
                        let resp = ui.add(label);
                        if resp.double_clicked() {
                            do_copy(ui, app);
                        }
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                    });
                ui.add_space(20.0);
                ui.label(
                    RichText::new("Press ? for keyboard shortcuts")
                        .size(12.0)
                        .color(pal::TEXT_DIM),
                );
            });
        });
    } else {
        render_file_panel(app, &mut right_ui, &mr, row_h, char_w, right_w, &row_font);
    }
    if let (Some(src), Some(dst)) = (app.anchor_link_source, app.anchor_link_target) {
        let painter = ui.ctx().layer_painter(LayerId::new(
            Order::Foreground,
            Id::new("anchor_link_overlay"),
        ));
        let stroke = Stroke::new(2.0, pal::BAR_ANCHOR);
        let mid_x = (src.x + dst.x) / 2.0;
        let c1 = Pos2::new(mid_x, src.y);
        let c2 = Pos2::new(mid_x, dst.y);
        painter.add(Shape::CubicBezier(
            epaint::CubicBezierShape::from_points_stroke(
                [src, c1, c2, dst],
                false,
                Color32::TRANSPARENT,
                stroke,
            ),
        ));
        let dot_stroke = Stroke::NONE;
        painter.circle(src, 3.0, pal::BAR_ANCHOR, dot_stroke);
        painter.circle(dst, 3.0, pal::BAR_ANCHOR, dot_stroke);
    }
}