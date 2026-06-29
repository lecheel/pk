use super::matching::MergeMatching;
use super::palette::pal;
use super::state::MergeApp;
use crate::diff::RowKind;
use eframe::egui::*;

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
    ScrollArea::vertical()
        .id_source("git_diff_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let desired = Vec2::new(ui.available_width(), row_h + 2.0);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgb(45, 20, 20));
            ui.painter().text(
                Pos2::new(rect.left() + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                "📝 GIT DIFF vs HEAD  ·  press ESC to close",
                FontId::monospace(11.0),
                Color32::from_rgb(230, 120, 120),
            );
            ui.add_space(4.0);
            if app.git_diff_rows.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new("No git differences or not in a Git repository.")
                            .color(pal::TEXT_DIM),
                    );
                });
                return;
            }
            for row in &app.git_diff_rows {
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
}
