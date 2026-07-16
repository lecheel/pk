//--+ file:///src/app/toolbar.rs
use super::clipboard_utils::{get_clipboard_text, parse_clipboard_patch};
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
                if ui
                    .button(RichText::new("🏠").size(14.0))
                    .on_hover_text("Home — reset to blank start state")
                    .clicked()
                {
                    app.go_home();
                }
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
                if ui
                    .button("📋 Paste Patch (*)")
                    .on_hover_text("Load and reparse a patch directly from your system clipboard")
                    .clicked()
                {
                    if let Some(pasted) = get_clipboard_text() {
                        let parsed_hunks = parse_clipboard_patch(&pasted);
                        if !parsed_hunks.is_empty() {
                            let _ = std::fs::write("temp.md", &pasted);
                            app.initial_patch_path = Some("temp.md".to_string());
                            app.patch_text = pasted;
                            app.reparse();
                            if !app.hunks.is_empty() && app.hunks[0].filename.is_empty() {
                                app.set_message(StatusMessage::warning(
                                    "Search pattern loaded. Enter the target filename below.",
                                ));
                            } else {
                                app.set_message(StatusMessage::success(
                                    "Loaded patch from clipboard (saved as temp.md)",
                                ));
                            }
                        } else {
                            app.set_message(StatusMessage::error(
                                "Clipboard content is empty or invalid",
                            ));
                        }
                    } else {
                        app.set_message(StatusMessage::error("Could not read text from clipboard"));
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
                    .checkbox(
                        &mut filter_low,
                        format!("Filter <{:.0}%", app.min_match_score),
                    )
                    .on_hover_text(format!(
                        "Skip and hide hunks with less than {:.0}% match score",
                        app.min_match_score
                    ))
                    .changed()
                {
                    app.filter_low_matches = filter_low;
                    if filter_low {
                        app.ensure_valid_filtered_hunk();
                    }
                }
                ui.add(Separator::default().vertical().spacing(12.0));
                let has_unsaved = !app.history.is_empty();
                let any_unsaved =
                    has_unsaved || app.file_states.values().any(|f| !f.history.is_empty());
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
                    .button(RichText::new("💾 Save All").color(if any_unsaved {
                        pal::ACCENT_GOOD
                    } else {
                        pal::TEXT_DIM
                    }))
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
                if !app.project_dirs.is_empty() {
                    ui.add(Separator::default().vertical().spacing(12.0));
                    ui.label(RichText::new("Project:").color(pal::TEXT_DIM).small());
                    // Persisted via egui memory (shared id with the Settings panel picker)
                    // so a dropdown click survives to the next frame instead of being
                    // silently discarded before "Set Active" is pressed.
                    let sel_id = Id::new("settings_project_dir_pending_selection");
                    let mut selected_dir: String = ui
                        .data(|d| d.get_temp(sel_id))
                        .unwrap_or_else(|| app.active_project_dir.clone().unwrap_or_default());
                    if !selected_dir.is_empty() && !app.project_dirs.contains(&selected_dir) {
                        selected_dir = app.active_project_dir.clone().unwrap_or_default();
                    }
                    ComboBox::from_id_source("toolbar_project_dir_combo")
                        .width(140.0)
                        .selected_text(if selected_dir.is_empty() {
                            "<None>".to_string()
                        } else {
                            std::path::Path::new(&selected_dir)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| selected_dir.clone())
                        })
                        .show_ui(ui, |ui| {
                            for dir in app.project_dirs.clone().iter() {
                                if ui.selectable_label(*dir == selected_dir, dir).clicked() {
                                    selected_dir = dir.clone();
                                }
                            }
                        });
                    ui.data_mut(|d| d.insert_temp(sel_id, selected_dir.clone()));
                    let is_already_active =
                        app.active_project_dir.as_deref() == Some(selected_dir.as_str());
                    let set_active_btn = ui.add_enabled(
                        !selected_dir.is_empty() && !is_already_active,
                        Button::new("Set Active"),
                    );
                    if set_active_btn.clicked() {
                        let dir = selected_dir;
                        app.active_project_dir = Some(dir.clone());
                        app.base_dir = dir.clone();
                        app.start_pwd = dir.clone();
                        app.start_pwd_is_repo = git2::Repository::discover(&dir).is_ok();
                        app.save_config();
                        app.reparse();
                        app.git_log_entries =
                            super::git_ops::get_git_log(std::path::Path::new(&app.base_dir));
                        app.set_message(StatusMessage::success(format!(
                            "Active project dir set to: {}",
                            dir
                        )));
                    }
                }
                if ui.button("?").on_hover_text("Keyboard shortcuts").clicked() {
                    app.show_help = !app.show_help;
                }
                if !app.disable_llm {
                    ui.add(Separator::default().vertical().spacing(12.0));
                    let has_prompt = app.active_system_prompt().is_some();
                    let prompt_btn_color = if app.show_system_prompt {
                        app.chat_mode.color()
                    } else if has_prompt {
                        pal::TEXT_DIM
                    } else {
                        Color32::from_gray(40)
                    };
                    if ui
                        .button(
                            RichText::new(format!("{} prompt", app.chat_mode.short_label()))
                                .color(prompt_btn_color)
                                .size(12.0),
                        )
                        .on_hover_text(if has_prompt {
                            format!(
                                "Show/hide {} system prompt (current LLM instruction context)",
                                app.chat_mode.short_label()
                            )
                        } else {
                            "No system prompt for Chat mode — switch to Commit or Impl".to_string()
                        })
                        .clicked()
                    {
                        if has_prompt {
                            app.show_system_prompt = !app.show_system_prompt;
                        } else {
                            app.set_message(StatusMessage::info(
                                "No system prompt in Chat mode. Try Commit or Impl mode.",
                            ));
                        }
                    }
                }
                if let Some(warning) = &app.daemon_sync_warning {
                    ui.label(RichText::new(warning).color(pal::ACCENT_BAD).strong());
                }
            });
        });
}
