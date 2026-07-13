use super::llm::{self, ChatMessage};
use super::matching::MergeMatching;
use super::palette::pal;
use super::state::MergeApp;
use crate::diff::RowKind;
use eframe::egui::*;
pub fn render_git_log_panel(
    app: &mut MergeApp,
    ui: &mut Ui,
    row_h: f32,
    char_w: f32,
    panel_w: f32,
) {
    let max_chars = ((panel_w - 90.0) / char_w).floor() as usize;
    let mut move_delta: i32 = 0;
    let mut copy_files = false;
    let mut copy_diff = false;
    if !ui.ctx().wants_keyboard_input() {
        ui.input(|i| {
            if i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::J) {
                move_delta = 1;
            }
            if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::K) {
                move_delta = -1;
            }
            if i.key_pressed(Key::C) {
                copy_files = true;
            }
            if i.key_pressed(Key::D) {
                copy_diff = true;
            }
        });
    }
    let just_moved = move_delta != 0;
    if just_moved && !app.git_log_entries.is_empty() {
        let len = app.git_log_entries.len() as i32;
        let cur = app.selected_git_log_entry.map(|i| i as i32).unwrap_or(-1);
        let new_idx = (cur + move_delta).clamp(0, len - 1);
        app.selected_git_log_entry = Some(new_idx as usize);
    }
    if copy_files || copy_diff {
        let entry_clone = app
            .selected_git_log_entry
            .and_then(|idx| app.git_log_entries.get(idx))
            .cloned();
        if let Some(entry) = entry_clone {
            if copy_files {
                let mut content = String::new();
                let mut copied_count = 0;
                if let Ok(repo) = git2::Repository::discover(&app.base_dir) {
                    if let Ok(oid) = git2::Oid::from_str(&entry.full_hash) {
                        if let Ok(commit) = repo.find_commit(oid) {
                            if let Ok(tree) = commit.tree() {
                                for f in &entry.files_changed {
                                    let file_text = tree
                                        .get_path(std::path::Path::new(&f.path))
                                        .ok()
                                        .and_then(|entry| repo.find_blob(entry.id()).ok())
                                        .map(|blob| {
                                            String::from_utf8_lossy(blob.content()).to_string()
                                        });
                                    match file_text {
                                        Some(text) => {
                                            content.push_str(&format!(
                                                "===== {} =====\n{}\n\n",
                                                f.path, text
                                            ));
                                            copied_count += 1;
                                        }
                                        None => {
                                            content.push_str(&format!(
                                                "===== {} =====\n(file deleted or unreadable at this commit)\n\n",
                                                f.path
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ui.ctx().copy_text(content);
                app.set_message(super::types::StatusMessage::success(format!(
                    "Copied full content of {}/{} file(s) for {}",
                    copied_count,
                    entry.files_changed.len(),
                    entry.hash
                )));
            }
            if copy_diff {
                let mut diff_text = String::new();
                for f in &entry.files_changed {
                    diff_text.push_str(&format!(
                        "===== {} (+{} -{}) =====\n{}\n\n",
                        f.path, f.additions, f.deletions, f.patch
                    ));
                }
                ui.ctx().copy_text(diff_text);
                app.set_message(super::types::StatusMessage::success(format!(
                    "Copied diff of {} file(s) for {}",
                    entry.files_changed.len(),
                    entry.hash
                )));
            }
        }
    }
    ScrollArea::vertical()
        .id_source("git_log_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let desired = Vec2::new(ui.available_width(), row_h + 2.0);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgb(30, 20, 45));
            ui.painter().text(
                Pos2::new(rect.left() + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                "📜 GIT LOG",
                FontId::monospace(11.0),
                Color32::from_rgb(180, 130, 230),
            );
            ui.add_space(4.0);

            if app.git_log_entries.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new("No commits found or not a git repository.")
                            .color(pal::TEXT_DIM),
                    );
                });
                return;
            }

            for (idx, entry) in app.git_log_entries.iter().enumerate() {
                let desired = Vec2::new(ui.available_width(), row_h);
                let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
                let is_hovered = resp.hovered();
                let is_selected = app.selected_git_log_entry == Some(idx);
                if is_selected && just_moved {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                }
                let bg = if is_selected {
                    Color32::from_rgba_premultiplied(70, 50, 100, 180)
                } else if is_hovered {
                    Color32::from_rgba_premultiplied(50, 50, 60, 150)
                } else {
                    pal::BG_ROW_EVEN
                };
                ui.painter().rect_filled(rect, 0.0, bg);

                let node_x = rect.left() + 12.0;
                ui.painter().line_segment(
                    [
                        Pos2::new(node_x, rect.top()),
                        Pos2::new(node_x, rect.bottom()),
                    ],
                    Stroke::new(1.0, Color32::from_rgb(100, 80, 120)),
                );
                ui.painter().circle(
                    Pos2::new(node_x, rect.center().y),
                    5.0,
                    if is_selected {
                        pal::BAR_CURSOR
                    } else {
                        Color32::from_rgb(180, 130, 230)
                    },
                    Stroke::new(1.5, Color32::WHITE),
                );

                if resp.clicked() {
                    app.selected_git_log_entry = Some(idx);
                }
                ui.painter().text(
                    Pos2::new(rect.left() + 24.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &entry.hash,
                    FontId::monospace(10.5),
                    Color32::from_rgb(200, 150, 250),
                );
                let author_x = rect.left() + 75.0;
                let display_author = MergeApp::truncate_owned(&entry.author, 15);
                ui.painter().text(
                    Pos2::new(author_x, rect.center().y),
                    Align2::LEFT_CENTER,
                    &display_author,
                    FontId::monospace(10.0),
                    pal::TEXT_DIM,
                );
                let msg_x = rect.left() + 175.0;
                let display_msg =
                    MergeApp::truncate_owned(&entry.message, max_chars.saturating_sub(22));
                ui.painter().text(
                    Pos2::new(msg_x, rect.center().y),
                    Align2::LEFT_CENTER,
                    &display_msg,
                    FontId::monospace(11.0),
                    pal::TEXT_NORMAL,
                );
            }
        });
}
pub fn render_git_commit_detail_panel(app: &mut MergeApp, ui: &mut Ui, panel_w: f32) {
    if let Some(receiver) = app.commit_ai_session.receiver.take() {
        let mut finished = false;
        while let Ok(response) = receiver.try_recv() {
            match response {
                llm::LlmResponse::Text(text) => {
                    app.commit_message = text;
                }
                llm::LlmResponse::Error(err) => {
                    app.set_message(super::types::StatusMessage::error(err));
                    finished = true;
                }
                llm::LlmResponse::Done => {
                    finished = true;
                }
                _ => {}
            }
        }
        if finished {
            app.commit_ai_session.is_loading = false;
            app.commit_ai_session.receiver = None;
            app.commit_ai_session.start_time = None;
        } else {
            app.commit_ai_session.receiver = Some(receiver);
        }
    }

    ScrollArea::vertical()
        .id_source("git_commit_detail_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("💾 GIT COMMIT")
                    .color(Color32::from_rgb(230, 200, 120))
                    .strong()
                    .monospace(),
            );
            ui.add_space(8.0);

            let mut staged_files: Vec<String> = Vec::new();
            if let Ok(repo) = git2::Repository::discover(&app.base_dir) {
                let mut opts = git2::StatusOptions::new();
                opts.include_untracked(false);
                if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
                    for entry in statuses.iter() {
                        if let Some(path) = entry.path() {
                            let status = entry.status();
                            if status.is_index_new()
                                || status.is_index_modified()
                                || status.is_index_deleted()
                                || status.is_index_renamed()
                            {
                                staged_files.push(path.to_string());
                            }
                        }
                    }
                }
            }

            if !staged_files.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "📝 Staged files to commit ({}):",
                        staged_files.len()
                    ))
                    .color(pal::TEXT_DIM),
                );
                ui.add_space(2.0);
                ScrollArea::vertical()
                    .id_source("git_commit_files_scroll")
                    .max_height(120.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for path in &staged_files {
                            ui.label(
                                RichText::new(format!("• {}", path))
                                    .color(pal::TEXT_NORMAL)
                                    .monospace(),
                            );
                        }
                    });
                ui.add_space(8.0);
            }

            ui.label(RichText::new("Commit message:").color(pal::TEXT_DIM));
            ui.add_space(2.0);
            ui.add(
                TextEdit::multiline(&mut app.commit_message)
                    .desired_width(panel_w - 16.0)
                    .desired_rows(5)
                    .font(FontId::monospace(12.0)),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if ui.button("✅ Commit").clicked() {
                    app.commit_changes();
                }
                if ui.button("🗑 Clear").clicked() {
                    app.commit_message.clear();
                }
            let btn_text = if app.commit_ai_session.is_loading {
                if app.commit_ai_session.start_time.is_none() {
                    app.commit_ai_session.start_time = Some(ui.ctx().input(|i| i.time));
                }
                let elapsed = ui.ctx().input(|i| i.time) - app.commit_ai_session.start_time.unwrap_or_default();
                let dots = match (elapsed * 2.0) as usize % 4 {
                    0 => "   ",
                    1 => ".  ",
                    2 => ".. ",
                    _ => "...",
                };
                format!("AI Commit [{}] {:.1}s", dots, elapsed)
            } else {
                "AI Commit".to_string()
            };
            let ai_btn = Button::new(RichText::new(btn_text).strong())
                .fill(if app.commit_ai_session.is_loading { Color32::from_gray(60) } else { Color32::from_rgb(40, 90, 55) });
            if ui.add_enabled(!app.commit_ai_session.is_loading, ai_btn).clicked() {
                    let recent_commits = app.git_log_entries.iter().take(5).map(|e| format!("- {}", e.message)).collect::<Vec<String>>().join("\n");
                    let diff = std::process::Command::new("git")
                        .args(["diff", "--staged"])
                        .current_dir(&app.base_dir)
                        .output()
                        .ok()
                        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                        .unwrap_or_default();
                    let diff = if diff.trim().is_empty() {
                        std::process::Command::new("git")
                            .args(["diff"])
                            .current_dir(&app.base_dir)
                            .output()
                            .ok()
                            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                            .unwrap_or_default()
                    } else {
                        diff
                    };
                    let (system_prompt, user_msg) = if app.llm_config.commit_system_prompt.is_empty() {
                        let prompt = format!(
                            "Generate a git commit message following this structure no explanation just the core:\n\
1. First line: conventional commit format (type: concise description) (use semantic types like feat, fix, docs, style, refactor, perf, test, chore, etc.).\n\
2. Optional bullet points if necessary:\n\
   - Keep the second line blank\n\
   - Keep them short and direct\n\
   - Focus on what changed\n\
   - Avoid overly formal or fluffy language\n\
Examples:\n\
feat: add user auth system\n\
<empty line>\n\
- Add JWT tokens for API auth\n\
- Handle token refresh for long sessions\n\
fix: resolve memory leak in worker pool\n\
- Clean up idle connections\n\
- Add timeout for stale workers\n\
Simple change example:\n\
fix: typo in README.md\n\
Your message must be based on the provided git diff, with a bit of styling from recent commits.\n\
Recent commits for reference:\n{}\n\
Git diff:\n{}", recent_commits, diff
                        );
                        (Some(super::chat::ChatMode::Commit.system_prompt()), prompt)
                    } else {
                        let prompt = format!("Recent commits for reference:\n{}\n\nGit diff:\n{}", recent_commits, diff);
                        (Some(app.llm_config.commit_system_prompt.clone()), prompt)
                    };
                    let messages = vec![ChatMessage {
                        role: "user".to_string(),
                        content: user_msg,
                        tool_calls: None,
                        tool_call_id: None,
                    }];
                    let provider = app.llm_config.models.get(app.llm_config.commit_model_idx).cloned().unwrap_or_default();
                app.commit_ai_session.receiver = Some(llm::send_to_llm(provider, messages, system_prompt, None, String::new(), String::new(), false));
                app.commit_ai_session.is_loading = true;
            }
            if app.commit_ai_session.is_loading {
                ui.ctx().request_repaint();
            }
        });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.label(
                RichText::new("📜 COMMIT DETAILS")
                    .color(Color32::from_rgb(180, 130, 230))
                    .strong()
                    .monospace(),
            );
            ui.add_space(8.0);

            let entry = match app.selected_git_log_entry {
                Some(idx) => match app.git_log_entries.get(idx) {
                    Some(e) => e.clone(),
                    None => return,
                },
                None => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new("Select a commit from the left to see details")
                                .color(pal::TEXT_DIM),
                        );
                    });
                    return;
                }
            };

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Commit: ")
                        .color(pal::TEXT_DIM)
                        .strong()
                        .monospace(),
                );
                ui.label(
                    RichText::new(&entry.full_hash)
                        .color(Color32::from_rgb(200, 150, 250))
                        .monospace(),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Author: ")
                        .color(pal::TEXT_DIM)
                        .strong()
                        .monospace(),
                );
                ui.label(
                    RichText::new(format!("{} <{}>", entry.author, entry.email))
                        .color(pal::TEXT_NORMAL)
                        .monospace(),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Date:   ")
                        .color(pal::TEXT_DIM)
                        .strong()
                        .monospace(),
                );
                ui.label(
                    RichText::new(&entry.time)
                        .color(pal::TEXT_NORMAL)
                        .monospace(),
                );
            });
            ui.add_space(10.0);

            ui.label(
                RichText::new("Message:")
                    .color(pal::TEXT_DIM)
                    .strong()
                    .monospace(),
            );
            ui.add_space(2.0);
            Frame::none()
                .fill(pal::BG_PANEL)
                .inner_margin(Margin::symmetric(8.0, 4.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(&entry.body)
                            .color(pal::TEXT_NORMAL)
                            .monospace(),
                    );
                });

            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("Files changed ({}):", entry.files_changed.len()))
                    .color(pal::TEXT_DIM)
                    .strong()
                    .monospace(),
            );
            ui.add_space(4.0);
            for file in &entry.files_changed {
                ui.horizontal(|ui| {
                    let badge_color = match file.status {
                        'A' => Color32::from_rgb(40, 150, 60),
                        'D' => Color32::from_rgb(200, 40, 40),
                        _ => Color32::from_rgb(200, 160, 40),
                    };
                    ui.label(
                        RichText::new(format!("[{}]", file.status))
                            .color(badge_color)
                            .monospace()
                            .strong(),
                    );
                    ui.label(
                        RichText::new(format!("+{} -{}", file.additions, file.deletions))
                            .color(pal::TEXT_DIM)
                            .monospace()
                            .size(10.0),
                    );
                    ui.label(
                        RichText::new(&file.path)
                            .color(pal::TEXT_NORMAL)
                            .monospace(),
                    );
                });
                ui.add_space(2.0);
                Frame::none()
                    .fill(pal::BG_PANEL)
                    .inner_margin(Margin::symmetric(8.0, 4.0))
                    .show(ui, |ui| {
                        for line in file.patch.lines() {
                            let color = if line.starts_with('+') {
                                pal::TEXT_INSERT
                            } else if line.starts_with('-') {
                                pal::TEXT_DELETE
                            } else if line.starts_with('@') {
                                Color32::from_rgb(100, 160, 230)
                            } else {
                                pal::TEXT_NORMAL
                            };
                            ui.label(RichText::new(line).color(color).monospace().size(10.0));
                        }
                    });
                ui.add_space(6.0);
            }
        });
}

pub fn render_git_diff_panel(
    app: &mut MergeApp,
    ui: &mut Ui,
    row_h: f32,
    char_w: f32,
    panel_w: f32,
) {
    let max_chars = ((panel_w - 68.0) / char_w).floor() as usize;
    let current_line = app.cursor_line.unwrap_or(0);
    let active_hunk = app
        .git_hunks
        .iter()
        .find(|h| h.current_line_range.contains(&current_line))
        .cloned();
    let mut revert_clicked = false;
    let mut prev_hunk_clicked = false;
    let mut next_hunk_clicked = false;
    let hunk_starts: Vec<usize> = {
        let mut starts: Vec<usize> = app
            .git_hunks
            .iter()
            .map(|h| h.current_line_range.start)
            .collect();
        starts.sort();
        starts
    };
    let hunk_count = hunk_starts.len();
    let hunk_pos = hunk_starts.iter().position(|&s| {
        active_hunk
            .as_ref()
            .map_or(false, |h| h.current_line_range.start == s)
    });
    ScrollArea::vertical()
        .id_source("git_diff_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let desired = Vec2::new(ui.available_width(), row_h + 2.0);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgb(45, 20, 20));
            let header_text = if hunk_count > 0 {
                match hunk_pos {
                    Some(idx) => format!(
                        "📝 GIT DIFF  ·  hunk {}/{}  ·  press ESC to close",
                        idx + 1,
                        hunk_count
                    ),
                    None => format!(
                        "📝 GIT DIFF  ·  {} hunk(s) in file  ·  press ESC to close",
                        hunk_count
                    ),
                }
            } else {
                "📝 GIT DIFF for current hunk  ·  press ESC to close".to_string()
            };

            ui.allocate_ui_with_layout(rect.size(), Layout::left_to_right(Align::Center), |ui| {
                ui.set_min_size(rect.size());
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(4.0);
                ui.label(
                    RichText::new(&header_text)
                        .color(Color32::from_rgb(230, 120, 120))
                        .strong()
                        .monospace(),
                );

                if active_hunk.is_some() {
                    ui.add_space(4.0);
                    let btn = Button::new(
                        RichText::new("Revert")
                            .font(FontId::monospace(11.0))
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(120, 40, 40));
                    if ui
                        .add(btn)
                        .on_hover_text("Revert this hunk back to the HEAD version")
                        .clicked()
                    {
                        revert_clicked = true;
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add_space(4.0);
                    if hunk_count > 0 {
                        let next_btn = Button::new(
                            RichText::new("▼")
                                .color(Color32::WHITE)
                                .strong()
                                .monospace(),
                        )
                        .fill(Color32::from_rgb(60, 30, 30));
                        if ui
                            .add(next_btn)
                            .on_hover_text("Next git hunk (]h)")
                            .clicked()
                        {
                            next_hunk_clicked = true;
                        }

                        let prev_btn = Button::new(
                            RichText::new("▲")
                                .color(Color32::WHITE)
                                .strong()
                                .monospace(),
                        )
                        .fill(Color32::from_rgb(60, 30, 30));
                        if ui
                            .add(prev_btn)
                            .on_hover_text("Previous git hunk ([h)")
                            .clicked()
                        {
                            prev_hunk_clicked = true;
                        }
                    }
                });
            });
            ui.add_space(4.0);
            let active_hunk = match &active_hunk {
                Some(h) => h,
                None => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("No git hunk found at the current cursor line.")
                                .color(pal::TEXT_DIM),
                        );
                    });
                    return;
                }
            };
            for row in &active_hunk.rows {
                let (base_bg, text_color, prefix) = match row.kind {
                    RowKind::Delete => (pal::BG_DELETE, pal::TEXT_DELETE, "- "),
                    RowKind::Insert => (pal::BG_INSERT, pal::TEXT_INSERT, "+ "),
                    RowKind::Equal => (Color32::TRANSPARENT, pal::TEXT_NORMAL, "  "),
                };
                let desired = Vec2::new(ui.available_width(), row_h);
                let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                if base_bg != Color32::TRANSPARENT {
                    ui.painter().rect_filled(rect, 0.0, base_bg);
                }
                let left_num = row.left_num.map_or(String::new(), |n| n.to_string());
                let right_num = row.right_num.map_or(String::new(), |n| n.to_string());
                ui.painter().text(
                    Pos2::new(rect.left() + 4.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    format!("{:>3}", left_num),
                    FontId::monospace(9.5),
                    pal::TEXT_DIM,
                );
                ui.painter().text(
                    Pos2::new(rect.left() + 26.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    format!("{:>3}", right_num),
                    FontId::monospace(9.5),
                    pal::TEXT_DIM,
                );
                ui.painter().text(
                    Pos2::new(rect.left() + 52.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    prefix,
                    FontId::monospace(11.0),
                    text_color,
                );
                let text = match row.kind {
                    RowKind::Delete => row.left.as_deref().unwrap_or(""),
                    _ => row.right.as_deref().unwrap_or(""),
                };
                let display = MergeApp::truncate_owned(text, max_chars.saturating_sub(2));
                ui.painter().text(
                    Pos2::new(rect.left() + 64.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &display,
                    FontId::monospace(11.0),
                    text_color,
                );
            }
        });
    if revert_clicked {
        if let Some(hunk) = active_hunk {
            app.revert_git_hunk(&hunk);
        }
    }
    if next_hunk_clicked && !hunk_starts.is_empty() {
        let cur = app.cursor_line.unwrap_or(0);
        let target = hunk_starts
            .iter()
            .copied()
            .find(|&s| s > cur)
            .unwrap_or(hunk_starts[0]);
        app.cursor_line = Some(target);
        app.scroll_to_match = true;
    }
    if prev_hunk_clicked && !hunk_starts.is_empty() {
        let cur = app.cursor_line.unwrap_or(0);
        let target = hunk_starts
            .iter()
            .rev()
            .copied()
            .find(|&s| s < cur)
            .unwrap_or(*hunk_starts.last().unwrap());
        app.cursor_line = Some(target);
        app.scroll_to_match = true;
    }
}

struct BranchInfo {
    name: String,
    time: i64,
    is_current: bool,
}

fn get_branch_list(repo: &git2::Repository) -> Vec<BranchInfo> {
    let mut branches = Vec::new();
    if let Ok(iter) = repo.branches(Some(git2::BranchType::Local)) {
        for b in iter.flatten() {
            let (branch, _) = b;
            if let Ok(Some(name)) = branch.name() {
                let is_current = branch.is_head();
                let time = branch
                    .get()
                    .peel_to_commit()
                    .map(|c| c.time().seconds())
                    .unwrap_or(0);
                branches.push(BranchInfo {
                    name: name.to_string(),
                    time,
                    is_current,
                });
            }
        }
    }
    branches.sort_by(|a, b| b.time.cmp(&a.time));
    branches.into_iter().take(7).collect()
}

fn get_stash_list(repo: &mut git2::Repository) -> Vec<String> {
    let mut stashes = Vec::new();
    let _ = repo.stash_foreach(|_idx, msg, _oid| {
        stashes.push(msg.to_string());
        true
    });
    stashes
}

fn format_relative_time(unix_secs: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(unix_secs);
    let diff = (now - unix_secs).max(0);
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{} minutes ago", diff / 60)
    } else if diff < 86400 {
        format!("{} hours ago", diff / 3600)
    } else {
        format!("{} days ago", diff / 86400)
    }
}

fn open_file_for_status_row(app: &mut MergeApp, path: &str) {
    if let Some(pos) = app
        .hunks
        .iter()
        .position(|h| h.filename == path || path.contains(&h.filename))
    {
        app.current_hunk = pos;
        app.load_hunk();
        return;
    }
    let target_path = std::path::Path::new(&app.base_dir)
        .join(path)
        .display()
        .to_string();
    if target_path != app.file_path {
        app.save_file_state();
        app.file_path = target_path;
        if let Ok(content) = std::fs::read_to_string(&app.file_path) {
            app.file_text = content;
            app.file_lines = app.file_text.lines().map(String::from).collect();
            app.applied_hunks.clear();
            app.merged_range = None;
            app.history.clear();
            app.recompute_match();
            app.update_git_statuses();
        }
    }
}

pub fn render_git_status_panel(
    app: &mut MergeApp,
    ui: &mut Ui,
    row_h: f32,
    _char_w: f32,
    _panel_w: f32,
) {
    ScrollArea::vertical()
        .id_source("git_status_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let desired = Vec2::new(ui.available_width(), row_h + 2.0);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgb(20, 35, 25));
            ui.painter().text(
                Pos2::new(rect.left() + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                "📝 GIT REPOSITORY STATUS  ·  press ESC to close",
                FontId::monospace(11.0),
                Color32::from_rgb(120, 230, 160),
            );
            ui.add_space(6.0);

            let repo = git2::Repository::discover(&app.base_dir).ok();

            let mut staged: Vec<(String, char, i32, i32)> = Vec::new();
            let mut unstaged: Vec<(String, char, i32, i32)> = Vec::new();
            let mut untracked: Vec<String> = Vec::new();

            if let Some(ref r) = repo {
                let mut opts = git2::StatusOptions::new();
                opts.include_untracked(true);
                if let Ok(statuses) = r.statuses(Some(&mut opts)) {
                    for entry in statuses.iter() {
                        if let Some(path) = entry.path() {
                            let status = entry.status();
                            let is_staged = status.is_index_new()
                                || status.is_index_modified()
                                || status.is_index_deleted()
                                || status.is_index_renamed()
                                || status.is_index_typechange();
                            let is_wt_change = status.is_wt_modified()
                                || status.is_wt_deleted()
                                || status.is_wt_renamed()
                                || status.is_wt_typechange();
                            let is_untracked = status.is_wt_new() && !is_staged;

                            let (mut additions, mut deletions) = (0, 0);
                            let mut diff_opts = git2::DiffOptions::new();
                            diff_opts.pathspec(path);
                            if let Ok(diff) = r.diff_index_to_workdir(None, Some(&mut diff_opts)) {
                                let _ = diff.foreach(
                                    &mut |_, _| true,
                                    None,
                                    None,
                                    Some(&mut |_, _, line| {
                                        match line.origin() {
                                            '+' => additions += 1,
                                            '-' => deletions += 1,
                                            _ => {}
                                        }
                                        true
                                    }),
                                );
                            }

                            if is_untracked {
                                untracked.push(path.to_string());
                            } else {
                                let status_char =
                                    if status.is_index_deleted() || status.is_wt_deleted() {
                                        'D'
                                    } else if status.is_index_new() {
                                        'A'
                                    } else {
                                        'M'
                                    };
                                if is_staged {
                                    staged.push((
                                        path.to_string(),
                                        status_char,
                                        additions,
                                        deletions,
                                    ));
                                }
                                if is_wt_change {
                                    unstaged.push((
                                        path.to_string(),
                                        status_char,
                                        additions,
                                        deletions,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            untracked.sort();

            // Flat, click/keyboard-selectable order matching the render order below.
            let mut flat: Vec<String> = Vec::new();
            for (p, ..) in &staged {
                flat.push(p.clone());
            }
            for (p, ..) in &unstaged {
                flat.push(p.clone());
            }
            for p in &untracked {
                flat.push(p.clone());
            }

            if !ui.ctx().wants_keyboard_input() && !flat.is_empty() {
                ui.input(|i| {
                    if i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowUp) {
                        let cur_pos = app
                            .git_status_selected_path
                            .as_ref()
                            .and_then(|p| flat.iter().position(|f| f == p));
                        let going_down = i.key_pressed(Key::ArrowDown);
                        let next_pos = match (cur_pos, going_down) {
                            (Some(p), true) => (p + 1).min(flat.len() - 1),
                            (Some(p), false) => p.saturating_sub(1),
                            (None, _) => 0,
                        };
                        app.git_status_selected_path = flat.get(next_pos).cloned();
                    }
                    if i.key_pressed(Key::S) {
                        if let Some(path) = app.git_status_selected_path.clone() {
                            app.toggle_stage_file(&path);
                        }
                    }
                    if i.key_pressed(Key::Z) {
                        app.stash_changes();
                    }
                });
                if ui.ctx().input(|i| i.key_pressed(Key::C)) {
                    stage_all_and_open_commit(app);
                }
            }
            render_status_section(
                app,
                ui,
                row_h,
                &format!("  Stage Changes ({})", staged.len()),
                &staged,
            );
            render_status_section(
                app,
                ui,
                row_h,
                &format!("  Unstage Changes ({})", unstaged.len()),
                &unstaged,
            );

            ui.label(
                RichText::new(format!("  Untracked Files ({})", untracked.len()))
                    .color(Color32::from_rgb(200, 210, 220))
                    .strong()
                    .monospace(),
            );
            let (sep_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(sep_rect, 0.0, pal::SEPARATOR);
            if untracked.is_empty() {
                ui.label(RichText::new("    (none)").color(pal::TEXT_DIM).monospace());
            } else {
                for path in &untracked {
                    let is_selected =
                        app.git_status_selected_path.as_deref() == Some(path.as_str());
                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
                    let bg = if is_selected {
                        Color32::from_rgba_premultiplied(50, 60, 70, 200)
                    } else if resp.hovered() {
                        Color32::from_rgba_premultiplied(50, 50, 60, 150)
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, 0.0, bg);
                    ui.painter().text(
                        Pos2::new(rect.left() + 24.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        path,
                        FontId::monospace(11.0),
                        pal::TEXT_NORMAL,
                    );
                    if resp.clicked() {
                        app.git_status_selected_path = Some(path.clone());
                        open_file_for_status_row(app, path);
                    }
                }
            }
            ui.add_space(10.0);

            ui.label(
                RichText::new("  ------ Branch ------")
                    .color(Color32::from_rgb(180, 130, 230))
                    .strong()
                    .monospace(),
            );
            let (sep_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(sep_rect, 0.0, pal::SEPARATOR);
            if let Some(ref r) = repo {
                let branches = get_branch_list(r);
                if branches.is_empty() {
                    ui.label(
                        RichText::new("    (no branches)")
                            .color(pal::TEXT_DIM)
                            .monospace(),
                    );
                } else {
                    for b in &branches {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(if b.is_current { "  * " } else { "    " })
                                    .color(pal::ACCENT_GOOD)
                                    .monospace(),
                            );
                            let name_text =
                                RichText::new(&b.name).monospace().color(if b.is_current {
                                    pal::ACCENT_GOOD
                                } else {
                                    pal::TEXT_NORMAL
                                });
                            ui.label(if b.is_current {
                                name_text.strong()
                            } else {
                                name_text
                            });
                            ui.label(
                                RichText::new(format_relative_time(b.time)).color(pal::TEXT_DIM),
                            );
                        });
                    }
                }
            }
            ui.add_space(10.0);

            ui.label(
                RichText::new("  ------ Stash ------")
                    .color(Color32::from_rgb(120, 230, 160))
                    .strong()
                    .monospace(),
            );
            let (sep_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(sep_rect, 0.0, pal::SEPARATOR);
            let stashes = if let Ok(mut r2) = git2::Repository::discover(&app.base_dir) {
                get_stash_list(&mut r2)
            } else {
                Vec::new()
            };
            if stashes.is_empty() {
                ui.label(RichText::new("    (none)").color(pal::TEXT_DIM).monospace());
            } else {
                for s in &stashes {
                    ui.label(
                        RichText::new(format!("    {}", s))
                            .color(pal::TEXT_NORMAL)
                            .monospace(),
                    );
                }
            }
            ui.add_space(10.0);

            let (sep_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(sep_rect, 0.0, pal::SEPARATOR);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("[c] Stage all and commit").clicked() {
                    stage_all_and_open_commit(app);
                }
                if ui.button("[s] Toggle staged").clicked() {
                    if let Some(path) = app.git_status_selected_path.clone() {
                        app.toggle_stage_file(&path);
                    }
                }
                if ui.button("[z] stash").clicked() {
                    app.stash_changes();
                }
            });
        });
}

fn render_status_section(
    app: &mut MergeApp,
    ui: &mut Ui,
    row_h: f32,
    title: &str,
    items: &[(String, char, i32, i32)],
) {
    ui.label(
        RichText::new(title)
            .color(Color32::from_rgb(200, 210, 220))
            .strong()
            .monospace(),
    );
    let (sep_rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(sep_rect, 0.0, pal::SEPARATOR);
    if items.is_empty() {
        ui.label(RichText::new("    (none)").color(pal::TEXT_DIM).monospace());
    } else {
        for (path, status_char, additions, deletions) in items {
            let is_selected = app.git_status_selected_path.as_deref() == Some(path.as_str());
            let badge_color = match status_char {
                'A' => Color32::from_rgb(40, 150, 60),
                'D' => Color32::from_rgb(200, 40, 40),
                _ => Color32::from_rgb(200, 160, 40),
            };
            let desired = Vec2::new(ui.available_width(), row_h);
            let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
            let bg = if is_selected {
                Color32::from_rgba_premultiplied(50, 60, 70, 200)
            } else if resp.hovered() {
                Color32::from_rgba_premultiplied(50, 50, 60, 150)
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, 0.0, bg);
            ui.painter().text(
                Pos2::new(rect.left() + 4.0, rect.center().y),
                Align2::LEFT_CENTER,
                format!("[{}]", status_char),
                FontId::monospace(10.5),
                badge_color,
            );
            let mut stats = String::new();
            if *additions > 0 {
                stats.push_str(&format!("+{} ", additions));
            }
            if *deletions > 0 {
                stats.push_str(&format!("-{} ", deletions));
            }
            ui.painter().text(
                Pos2::new(rect.left() + 32.0, rect.center().y),
                Align2::LEFT_CENTER,
                &stats,
                FontId::monospace(10.0),
                pal::TEXT_DIM,
            );
            ui.painter().text(
                Pos2::new(rect.left() + 92.0, rect.center().y),
                Align2::LEFT_CENTER,
                &MergeApp::truncate_owned(path, 60),
                FontId::monospace(11.0),
                pal::TEXT_NORMAL,
            );
            if resp.clicked() {
                app.git_status_selected_path = Some(path.clone());
                open_file_for_status_row(app, path);
            }
        }
    }
    ui.add_space(6.0);
}

fn stage_all_and_open_commit(app: &mut MergeApp) {
    if let Ok(repo) = git2::Repository::discover(&app.base_dir) {
        if let Ok(mut index) = repo.index() {
            let mut opts = git2::StatusOptions::new();
            opts.include_untracked(true);
            if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
                for entry in statuses.iter() {
                    if let Some(path) = entry.path() {
                        let status = entry.status();
                        // Skip untracked files (wt_new and not index_new)
                        let is_untracked = status.is_wt_new() && !status.is_index_new();
                        if !is_untracked {
                            let _ = index.add_path(std::path::Path::new(path));
                        }
                    }
                }
            }
            let _ = index.write();
        }
    }
    app.show_git_status_window = false;
    app.show_git_log_window = true;
    app.git_log_entries = super::git_ops::get_git_log(std::path::Path::new(&app.base_dir));
    app.set_message(super::types::StatusMessage::info(
        "Staged all tracked changes — write your commit message in the Log tab"
    ));
}