use super::chat;
use super::matching::MergeMatching;
use super::palette::pal;
use super::state::MergeApp;
use super::types::StatusMessage;
use eframe::egui::*;
use std::sync::mpsc;

pub fn render_welcome_panel(app: &mut MergeApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
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

            let prompt_text = "Please apply changes using this style format in single code block:
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
}

pub fn render_fmt_error_panel(app: &mut MergeApp, ui: &mut Ui) {
    ui.add_space(12.0);
    ui.label(
        RichText::new("rustfmt failed to format the file. Please fix the syntax errors:")
            .color(pal::ACCENT_BAD)
            .size(15.0),
    );
    ui.add_space(8.0);
    ScrollArea::vertical()
        .id_source("fmt_error_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            if let Some(err) = &app.fmt_error {
                ui.label(
                    RichText::new(err)
                        .color(pal::TEXT_NORMAL)
                        .font(FontId::monospace(14.0)),
                );
            }
        });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.button("✕ Dismiss Error").clicked() {
            app.fmt_error = None;
            app.show_fmt_error = false;
        }
    });
}

pub fn render_settings_panel(app: &mut MergeApp, ui: &mut Ui) {
    ScrollArea::vertical()
        .id_source("settings_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Diff Settings");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut app.ignore_comments, "Ignore Comments in LCS")
                    .changed()
                {
                    app.save_config();
                    app.recompute_match();
                    app.update_git_statuses();
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .checkbox(
                        &mut app.short_search_display,
                        "Short Search/Replace Display",
                    )
                    .on_hover_text(
                        "Truncate search and replace blocks to first and last 10 lines when long",
                    )
                    .changed()
                {
                    app.save_config();
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Min match score to auto-apply:");
                if ui
                    .add(Slider::new(&mut app.min_match_score, 0.0..=100.0).suffix("%"))
                    .changed()
                {
                    app.save_config();
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .checkbox(
                        &mut app.short_search_display,
                        "Short Search/Replace Display",
                    )
                    .on_hover_text(
                        "Truncate search and replace blocks to first and last 10 lines when long",
                    )
                    .changed()
                {
                    app.save_config();
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut app.disable_llm, "Disable LLM Features")
                    .on_hover_text("Hide and turn off all chat, commit helper, and LLM-based tools")
                    .changed()
                {
                    app.save_config();
                }
            });
            ui.add_space(8.0);
            ui.heading("Formatter Settings");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut app.format_on_save, "Format on Save");
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Command:");
                ui.add(
                    TextEdit::singleline(&mut app.fmt_command)
                        .desired_width(ui.available_width() - 80.0)
                        .hint_text("rustfmt"),
                );
            });
            ui.add_space(8.0);
            ui.label(RichText::new("Examples:").color(pal::TEXT_DIM).small());
            ui.label(
                RichText::new("rustfmt")
                    .color(pal::TEXT_DIM)
                    .small()
                    .monospace(),
            );
            ui.label(
                RichText::new("rustfmt --edition 2021")
                    .color(pal::TEXT_DIM)
                    .small()
                    .monospace(),
            );
            ui.label(
                RichText::new("cargo fmt --")
                    .color(pal::TEXT_DIM)
                    .small()
                    .monospace(),
            );
            ui.add_space(16.0);
            ui.heading("Daemon Settings");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut app.concat_server_enabled, "Enable Concat Server")
                    .changed()
                {
                    app.save_config();
                    if app.concat_server_enabled {
                        app.set_message(StatusMessage::info(
                            "Concat server enabled. Restart app to fetch repos.",
                        ));
                    } else {
                        app.set_message(StatusMessage::info(
                            "Concat server disabled. Restart app to apply.",
                        ));
                    }
                }
            });
            ui.add_space(16.0);
            ui.separator();
            ui.heading("Impl Workflow Settings");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("RustConcat API URL:");
                ui.add(
                    TextEdit::singleline(&mut app.rustconcat_api_url)
                        .desired_width(ui.available_width() - 100.0)
                        .hint_text("http://127.0.0.1:7890"),
                );
            });
            ui.add_space(8.0);
            ui.label("Enable Tools for Impl Role:");
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut app.impl_tools.skeleton, "Skeleton")
                    .changed()
                {
                    app.save_config();
                }
                if ui.checkbox(&mut app.impl_tools.files, "Files").changed() {
                    app.save_config();
                }
                if ui.checkbox(&mut app.impl_tools.hashes, "Hashes").changed() {
                    app.save_config();
                }
            });
            ui.add_space(8.0);
            if ui
                .checkbox(&mut app.debug_impl_llm, "Debug Impl LLM (Log to console)")
                .changed()
            {
                app.save_config();
            }

            if !app.disable_llm {
                ui.add_space(16.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("⚙ Open LLM Configuration").clicked() {
                        app.show_llm_config = true;
                        app.show_settings = false;
                    }
                });
            }
        });
}

pub fn render_repos_panel(app: &mut MergeApp, ui: &mut Ui) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Select the repository where file paths should resolve:");
        if ui.button("🔄 Refresh").clicked() {
            let (tx, rx) = mpsc::channel();
            app.repo_receiver = rx;
            std::thread::spawn(move || {
                let res = super::daemon::fetch_repos();
                let _ = tx.send(res);
            });
        }
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);
    if app.available_repos.is_empty() {
        ui.label(
            RichText::new(if app.daemon_error.is_some() {
                format!("⚠️ Daemon error: {}", app.daemon_error.as_ref().unwrap())
            } else {
                "No repos registered. Use 'cli add-repo' to register one.".to_string()
            })
            .color(pal::TEXT_DIM),
        );
    } else {
        ScrollArea::vertical()
            .id_source("repos_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let repos_clone = app.available_repos.clone();
                for repo in repos_clone.iter() {
                    let is_active = app.active_repo_id.as_deref() == Some(repo.id.as_str());
                    let bg = if is_active { Color32::from_rgb(30, 45, 30) } else { pal::BG_PANEL };
                    Frame::none()
                        .fill(bg)
                        .stroke(Stroke::new(1.0, if is_active { pal::ACCENT_GOOD } else { pal::SEPARATOR }))
                        .rounding(4.0)
                        .inner_margin(Margin::symmetric(8.0, 6.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(if is_active { "→ " } else { "  " }).color(pal::ACCENT_GOOD).monospace());
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&repo.id).color(pal::TEXT_NORMAL).strong().monospace());
                                        if let Some(branch) = &repo.git_branch {
                                            ui.label(RichText::new(format!("[{}]", branch)).color(pal::TEXT_DIM).small());
                                        }
                                        if is_active {
                                            ui.label(RichText::new("↑ active").color(pal::ACCENT_GOOD).small());
                                        }
                                    });
                                    let files = repo.file_count.unwrap_or(0);
                                    ui.label(RichText::new(format!("{}  ({} files)", repo.source_path, files)).color(pal::TEXT_DIM).small());
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if is_active {
                                        if ui.button("✕ Clear").clicked() {
                                            app.active_repo_id = None;
                                            super::daemon::clear_active_repo();
                                            app.set_message(StatusMessage::info("Cleared active repo. Paths must be fully qualified."));
                                        }
                                    } else {
                                        if ui.button("Use").clicked() {
                                            app.active_repo_id = Some(repo.id.clone());
                                            app.base_dir = repo.source_path.clone();
                                            app.start_pwd = repo.source_path.clone();
                                            app.start_pwd_is_repo = true;
                                            super::daemon::set_active_repo(&repo.id);
                                            app.set_message(StatusMessage::success(format!("✅ Active repo: {}. Files will be looked up in repo '{}'", repo.id, repo.id)));
                                            app.show_repos_window = false;
                                            app.reparse();
                                        }
                                    }
                                    if ui.button("🔄 Sync").clicked() {
                                        let id = repo.id.clone();
                                        let ctx_clone = ui.ctx().clone();
                                        app.set_message(StatusMessage::info(format!("Syncing {}...", id)));
                                        std::thread::spawn(move || {
                                            match super::daemon::sync_repo(&id) {
                                                Ok(_) => {},
                                                Err(_) => {},
                                            }
                                            ctx_clone.request_repaint();
                                        });
                                    }
                                });
                            });
                        });
                    ui.add_space(4.0);
                }
            });
    }
}

pub fn render_debug_panel(app: &mut MergeApp, ui: &mut Ui) {
    ui.add_space(8.0);

    ScrollArea::vertical()
        .id_source("debug_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let mut report = String::new();
            ui.heading("Paths & directory mappings");
            ui.horizontal(|ui| {
                ui.label(RichText::new("Start PWD:").strong());
                ui.label(RichText::new(&app.start_pwd).monospace());
                if app.start_pwd_is_repo {
                    ui.colored_label(Color32::from_rgb(120, 220, 160), "(Git Repo)");
                } else {
                    ui.colored_label(Color32::from_rgb(230, 100, 100), "(Not Git Repo)");
                }
            });
            report.push_str(&format!(
                "Start PWD: {} (Git Repo: {})\n",
                app.start_pwd, app.start_pwd_is_repo
            ));
            ui.horizontal(|ui| {
                ui.label(RichText::new("Base directory:").strong());
                ui.label(RichText::new(&app.base_dir).monospace());
            });
            report.push_str(&format!("Base Directory: {}\n", app.base_dir));
            ui.horizontal(|ui| {
                ui.label(RichText::new("Current file path:").strong());
                ui.label(RichText::new(&app.file_path).monospace());
            });
            report.push_str(&format!("Current File Path: {}\n", app.file_path));
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.heading("Git mapping diagnostics");
            let repo_root = std::path::Path::new(&app.base_dir);
            match git2::Repository::discover(repo_root) {
                Ok(repo) => {
                    ui.colored_label(Color32::from_rgb(120, 220, 160), "✔ Git repository found");
                    report.push_str("Git Repo: Found\n");
                    if let Some(workdir) = repo.workdir() {
                        ui.horizontal(|ui| {
                            ui.label("Repo workdir:");
                            ui.label(RichText::new(workdir.to_string_lossy()).monospace());
                        });
                        report.push_str(&format!("Repo workdir: {}\n", workdir.to_string_lossy()));
                        let file_path = std::path::Path::new(&app.file_path);
                        let abs_file_path = if file_path.is_absolute() {
                            file_path.to_path_buf()
                        } else if let Ok(cwd) = std::env::current_dir() {
                            cwd.join(file_path)
                        } else {
                            file_path.to_path_buf()
                        };
                        let abs_workdir = if workdir.is_absolute() {
                            workdir.to_path_buf()
                        } else if let Ok(cwd) = std::env::current_dir() {
                            cwd.join(workdir)
                        } else {
                            workdir.to_path_buf()
                        };
                        let clean_path = |p: &std::path::Path| -> String {
                            let s = p.to_string_lossy().replace('\\', "/");
                            if let Some(stripped) = s.strip_prefix("//?/") {
                                stripped.to_string()
                            } else {
                                s
                            }
                        };
                        let clean_file = clean_path(&abs_file_path);
                        let clean_work = clean_path(&abs_workdir);
                        ui.horizontal(|ui| {
                            ui.label("Normalized file path:");
                            ui.label(RichText::new(&clean_file).monospace());
                        });
                        report.push_str(&format!("Normalized file path: {}\n", clean_file));
                        ui.horizontal(|ui| {
                            ui.label("Normalized workdir:");
                            ui.label(RichText::new(&clean_work).monospace());
                        });
                        report.push_str(&format!("Normalized workdir: {}\n", clean_work));
                        if clean_file.starts_with(&clean_work) {
                            let rel = &clean_file[clean_work.len()..].trim_start_matches('/');
                            ui.colored_label(
                                Color32::from_rgb(120, 220, 160),
                                format!("✔ Relative path match: {}", rel),
                            );
                            report.push_str(&format!("Relative path match: {}\n", rel));
                        } else {
                            ui.colored_label(
                                Color32::from_rgb(230, 100, 100),
                                "❌ Path mismatch: File is not inside the repo workdir.",
                            );
                            report.push_str(
                                "Relative path match: Mismatch (File not inside repo workdir)\n",
                            );
                        }
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(230, 100, 100),
                            "❌ Git repo missing working directory",
                        );
                        report.push_str("Git Repo Workdir: Missing\n");
                    }
                }
                Err(e) => {
                    ui.colored_label(
                        Color32::from_rgb(230, 100, 100),
                        format!("❌ Git lookup error: {}", e),
                    );
                    report.push_str(&format!("Git lookup error: {}\n", e));
                }
            }
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.heading("Buffers & state summary");
            ui.label(format!("Total patches in file: {}", app.hunks.len()));
            ui.label(format!("Current hunk index: {}", app.current_hunk));
            ui.label(format!("Applied hunks indices: {:?}", app.applied_hunks));
            ui.label(format!("File lines: {}", app.file_lines.len()));
            ui.label(format!("Git status indexes: {}", app.git_statuses.len()));
            report.push_str(&format!("Total patches: {}\n", app.hunks.len()));
            report.push_str(&format!("Current hunk index: {}\n", app.current_hunk));
            report.push_str(&format!("Applied hunk indices: {:?}\n", app.applied_hunks));
            report.push_str(&format!("File lines: {}\n", app.file_lines.len()));
            report.push_str(&format!("Git status indexes: {}\n", app.git_statuses.len()));
            let (mut unchanged, mut added, mut modified, mut deleted) = (0, 0, 0, 0);
            for status in &app.git_statuses {
                match status {
                    super::git_ops::GitStatus::Unchanged => unchanged += 1,
                    super::git_ops::GitStatus::Added => added += 1,
                    super::git_ops::GitStatus::Modified => modified += 1,
                    super::git_ops::GitStatus::Deleted => deleted += 1,
                }
            }
            ui.horizontal(|ui| {
                ui.label("Gutter distribution:");
                ui.colored_label(
                    Color32::from_gray(160),
                    format!("Unchanged: {} ", unchanged),
                );
                ui.colored_label(
                    Color32::from_rgb(120, 220, 160),
                    format!("Added: {} ", added),
                );
                ui.colored_label(
                    Color32::from_rgb(220, 200, 100),
                    format!("Modified: {} ", modified),
                );
                ui.colored_label(
                    Color32::from_rgb(235, 120, 120),
                    format!("Deleted: {}", deleted),
                );
            });
            report.push_str(&format!(
                "Gutter: Unchanged: {}, Added: {}, Modified: {}, Deleted: {}\n",
                unchanged, added, modified, deleted
            ));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("📋 Copy Diagnostics").clicked() {
                    ui.ctx().copy_text(report);
                }
                if ui.button("Force update git status").clicked() {
                    app.update_git_statuses();
                }
            });
        });
}
