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
                "📜 GIT LOG  ·  press ESC to close",
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
                    [Pos2::new(node_x, rect.top()), Pos2::new(node_x, rect.bottom())],
                    Stroke::new(1.0, Color32::from_rgb(100, 80, 120)),
                );
                ui.painter().circle(
                    Pos2::new(node_x, rect.center().y),
                    5.0,
                    if is_selected { pal::BAR_CURSOR } else { Color32::from_rgb(180, 130, 230) },
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
pub fn render_git_commit_detail_panel(app: &mut MergeApp, ui: &mut Ui) {
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

    ScrollArea::vertical()
        .id_source("git_commit_detail_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(10.0);
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
                            ui.label(
                                RichText::new(line)
                                    .color(color)
                                    .monospace()
                                    .size(10.0),
                            );
                        }
                    });
                ui.add_space(6.0);
            }
        });
}
pub fn render_git_status_panel(
    app: &mut MergeApp,
    ui: &mut Ui,
    row_h: f32,
    char_w: f32,
    panel_w: f32,
) {
    let max_chars = ((panel_w - 90.0) / char_w).floor() as usize;
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
            ui.add_space(4.0);
            let repo = git2::Repository::discover(&app.base_dir).ok();
            let mut file_statuses = Vec::new();
            if let Some(ref r) = repo {
                let mut opts = git2::StatusOptions::new();
                opts.include_untracked(true);
                if let Ok(statuses) = r.statuses(Some(&mut opts)) {
                    for entry in statuses.iter() {
                        if let Some(path) = entry.path() {
                            let status = entry.status();
                            let status_char = if status.is_index_new() || status.is_wt_new() {
                                'A'
                            } else if status.is_index_deleted() || status.is_wt_deleted() {
                                'D'
                            } else {
                                'M'
                            };
                            let mut additions = 0;
                            let mut deletions = 0;
                            let mut diff_opts = git2::DiffOptions::new();
                            diff_opts.pathspec(path);
                            if let Ok(diff) = r.diff_index_to_workdir(None, Some(&mut diff_opts)) {
                                let _ = diff.foreach(
                                    &mut |_, _| true,
                                    None,
                                    None,
                                    Some(&mut |_, _, line| {
                                        let origin = line.origin();
                                        if origin == '+' {
                                            additions += 1;
                                        } else if origin == '-' {
                                            deletions += 1;
                                        }
                                        true
                                    }),
                                );
                            }
                            file_statuses.push((
                                path.to_string(),
                                status_char,
                                additions,
                                deletions,
                            ));
                        }
                    }
                }
            }
            if file_statuses.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new("No modified or untracked files in the repository.")
                            .color(pal::TEXT_DIM),
                    );
                });
                return;
            }
            for (path, status_char, additions, deletions) in file_statuses {
                let (base_bg, badge_color, prefix) = match status_char {
                    'A' => (pal::BG_INSERT, Color32::from_rgb(40, 150, 60), "A"),
                    'D' => (pal::BG_DELETE, Color32::from_rgb(200, 40, 40), "D"),
                    _ => (
                        Color32::from_rgba_premultiplied(45, 38, 15, 100),
                        Color32::from_rgb(200, 160, 40),
                        "M",
                    ),
                };
                let desired = Vec2::new(ui.available_width(), row_h);
                let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
                let is_hovered = resp.hovered();
                let bg = if is_hovered {
                    Color32::from_rgba_premultiplied(50, 50, 60, 150)
                } else {
                    base_bg
                };
                ui.painter().rect_filled(rect, 0.0, bg);
                let status_bar = Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height()));
                ui.painter().rect_filled(status_bar, 0.0, badge_color);
                ui.painter().text(
                    Pos2::new(rect.left() + 8.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    format!("[{}]", prefix),
                    FontId::monospace(10.5),
                    badge_color,
                );
                let mut stats_str = String::new();
                if additions > 0 {
                    stats_str.push_str(&format!("+{} ", additions));
                }
                if deletions > 0 {
                    stats_str.push_str(&format!("-{} ", deletions));
                }
                ui.painter().text(
                    Pos2::new(rect.left() + 40.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &stats_str,
                    FontId::monospace(10.0),
                    if additions > 0 && deletions > 0 {
                        pal::TEXT_DIM
                    } else if additions > 0 {
                        pal::TEXT_INSERT
                    } else {
                        pal::TEXT_DELETE
                    },
                );
                let path_x = rect.left() + 100.0;
                let display = MergeApp::truncate_owned(&path, max_chars);
                ui.painter().text(
                    Pos2::new(path_x, rect.center().y),
                    Align2::LEFT_CENTER,
                    &display,
                    FontId::monospace(11.0),
                    pal::TEXT_NORMAL,
                );
                if resp.clicked() {
                    if let Some(pos) = app
                        .hunks
                        .iter()
                        .position(|h| h.filename == path || path.contains(&h.filename))
                    {
                        app.current_hunk = pos;
                        app.load_hunk();
                    } else {
                        let target_path = std::path::Path::new(&app.base_dir)
                            .join(&path)
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
                }
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