use super::git_ops::GitStatus;
use super::matching::MergeMatching;
use super::palette::pal;
use super::state::{MarkPending, MergeApp};
use super::types::{Action, FileAnchor, SearchRow, StatusMessage};
use crate::diff::RowKind;
use crate::patch::PatchHunk;
use eframe::egui::*;
use std::collections::HashSet;

pub fn render_split_view(app: &mut MergeApp, ui: &mut Ui) {
    let mr = match app.match_result.clone() {
        Some(m) => m,
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("No file loaded or no match found.")
                        .color(Color32::from_gray(140)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Open a file or patch above")
                        .color(pal::TEXT_DIM)
                        .small(),
                );
            });
            return;
        }
    };
    let available = ui.available_size();
    let divider = 0.38_f32;
    let left_w = (available.x * divider).floor() - 1.0;
    let right_w = available.x - left_w - 2.0;
    let mono_h = ui.text_style_height(&TextStyle::Monospace);
    let row_h = mono_h + 4.0;
    let char_w = mono_h * 0.60;
    ui.horizontal(|ui| {
        let hunk = app.current_hunk().unwrap();
        let header_bg = if app.show_git_diff_window {
            Color32::from_rgb(58, 28, 28)
        } else {
            Color32::from_rgb(28, 38, 58)
        };
        Frame::none()
            .fill(header_bg)
            .inner_margin(Margin::symmetric(8.0, 3.0))
            .show(ui, |ui| {
                ui.set_min_width(left_w);
                ui.set_max_width(left_w);
                let header_text = if app.show_git_diff_window {
                    format!("GIT DIFF  ·  {}", hunk.filename)
                } else {
                    format!("SEARCH  ·  {}", hunk.filename)
                };
                let header_color = if app.show_git_diff_window {
                    Color32::from_rgb(235, 120, 120)
                } else {
                    Color32::from_rgb(120, 180, 255)
                };
                ui.label(
                    RichText::new(header_text)
                        .color(header_color)
                        .strong()
                        .monospace(),
                );
            });
        ui.add_space(2.0);
        Frame::none()
            .fill(Color32::from_rgb(28, 45, 35))
            .inner_margin(Margin::symmetric(8.0, 3.0))
            .show(ui, |ui| {
                ui.set_min_width(right_w);
                let mark_label = if app.file_anchors.is_empty() {
                    String::new()
                } else {
                    let labels: Vec<String> =
                        app.file_anchors.values().map(|f| f.label()).collect();
                    format!("  ·  {}", labels.join("  "))
                };
                ui.label(
                    RichText::new(format!(
                        "FILE  ·  {} lines  ·  match @ {}–{}{}",
                        app.file_lines.len(),
                        mr.file_start + 1,
                        mr.file_end,
                        mark_label,
                    ))
                    .color(Color32::from_rgb(120, 220, 160))
                    .strong()
                    .monospace(),
                );
            });
    });
    ui.add(Separator::default());
    let body_rect = ui.available_rect_before_wrap();
    let mut left_rect = body_rect;
    left_rect.set_width(left_w);
    let mut right_rect = body_rect;
    right_rect.min.x = body_rect.min.x + left_w + 2.0;
    right_rect.set_width(right_w);
    let mut left_ui = ui.child_ui(left_rect, Layout::top_down(Align::LEFT), None);
    if app.show_git_diff_window {
        render_git_diff_panel(app, &mut left_ui, row_h, char_w, left_w);
    } else {
        render_search_panel(app, &mut left_ui, &mr, row_h, char_w, left_w);
    }
    let mut right_ui = ui.child_ui(right_rect, Layout::top_down(Align::LEFT), None);
    render_file_panel(app, &mut right_ui, &mr, row_h, char_w, right_w);
}
fn render_git_diff_panel(app: &mut MergeApp, ui: &mut Ui, row_h: f32, char_w: f32, panel_w: f32) {
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

// In src/app/split_view.rs

#[cfg(not(target_arch = "wasm32"))]
fn get_clipboard_text() -> Option<String> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut cb| cb.get_text().ok())
}

#[cfg(target_arch = "wasm32")]
fn get_clipboard_text() -> Option<String> {
    None
}

/// Smart parser to convert raw clipboard text into search-only or standard hunks
fn parse_clipboard_patch(pasted: &str) -> Vec<PatchHunk> {
    let trimmed = pasted.trim();

    // If it is a standard patch block, parse it using the app's default parser
    if trimmed.contains("<patch>") {
        return crate::patch::parse_patches(pasted);
    }

    // If it has unified diff headers, try default patch parsing
    if trimmed.contains("diff --git") || trimmed.contains("--- ") || trimmed.contains("+++ ") {
        let hunks = crate::patch::parse_patches(pasted);
        if !hunks.is_empty() {
            return hunks;
        }
    }

    // Otherwise, treat the entire clipboard content as a raw search pattern
    let lines: Vec<String> = pasted.lines().map(|s| s.to_string()).collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut filename = String::new();
    let mut search_start = 0;

    let first_line = lines[0].trim();
    if first_line.starts_with("filename ") {
        filename = first_line
            .strip_prefix("filename ")
            .unwrap()
            .trim()
            .to_string();
        search_start = 1;
    } else if first_line.starts_with("+++ b/") {
        filename = first_line
            .strip_prefix("+++ b/")
            .unwrap()
            .trim()
            .to_string();
        search_start = 1;
    } else if first_line.starts_with("+++ ") {
        filename = first_line.strip_prefix("+++ ").unwrap().trim().to_string();
        search_start = 1;
    }

    // Gather all lines and strip diff markers if present
    let search_lines: Vec<String> = lines[search_start..]
        .iter()
        .filter(|l| {
            !l.starts_with("<<<<<<<") && !l.starts_with("=======") && !l.starts_with(">>>>>>>")
        })
        .cloned()
        .collect();

    vec![PatchHunk {
        filename,
        search: search_lines,
        replace: Vec::new(), // No replace block for raw search pattern pastes
    }]
}

fn render_search_panel(
    app: &mut MergeApp,
    ui: &mut Ui,
    mr: &crate::diff::MatchResult,
    row_h: f32,
    char_w: f32,
    panel_w: f32,
) {
    let max_chars = ((panel_w - 58.0) / char_w).floor() as usize;
    let mut set_selection: Option<(usize, usize)> = None;
    let mut apply_clicked_id: Option<char> = None;
    let mut apply_clicked = false;
    let mut apply_clicked_line: Option<usize> = None;

    // Clipboard and Filename Adjustments Header
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if ui
            .button("📋 Paste Patch")
            .on_hover_text("Load and reparse a patch directly from your system clipboard")
            .clicked()
        {
            if let Some(pasted) = get_clipboard_text() {
                let parsed_hunks = parse_clipboard_patch(&pasted);
                if !parsed_hunks.is_empty() {
                    app.hunks = parsed_hunks;
                    app.current_hunk = 0;
                    app.applied_hunks.clear();
                    app.merged_range = None;
                    app.history.clear();
                    app.vim_buffer.clear();
                    app.d_pending = false;
                    app.file_anchors.clear();
                    app.mark_pending = None;
                    app.file_search_query.clear();
                    app.search_matches.clear();
                    app.cursor_line = None;
                    app.scroll_to_match = true;
                    app.left_selection = None;

                    // If the parsed filename is empty, notify the user to input the filename manually
                    if app.hunks[0].filename.is_empty() {
                        app.set_message(StatusMessage::warning(
                            "Search pattern loaded. Enter the target filename below.",
                        ));
                    } else {
                        app.load_hunk();
                        app.set_message(StatusMessage::success(
                            "Successfully loaded patch from clipboard",
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
    });
    ui.add_space(4.0);

    let mut filename_changed = false;
    if let Some(hunk) = app.hunks.get_mut(app.current_hunk) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Target File:").color(pal::TEXT_DIM).small());
            let mut filename = hunk.filename.clone();
            let edit_resp = ui.add(
                TextEdit::singleline(&mut filename)
                    .text_color(pal::TEXT_NORMAL)
                    .font(FontId::monospace(10.0))
                    .desired_width(panel_w - 120.0),
            );
            if edit_resp.changed() {
                hunk.filename = filename;
            }
            if ui.small_button("Reload").clicked()
                || (edit_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
            {
                filename_changed = true;
            }
        });
        ui.add_space(4.0);
    }
    ui.separator();
    ui.add_space(2.0);

    if filename_changed {
        app.load_hunk();
    }

    ScrollArea::vertical()
        .id_source("search_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let hunk = match app.current_hunk() {
                Some(h) => h.clone(),
                None => return,
            };
            let is_applied = app.applied_hunks.contains(&app.current_hunk);
            let (banner_bg, banner_fg, _icon) = MergeApp::score_appearance(mr.score);
            let (banner_bg, banner_text) = if is_applied {
                (
                    Color32::from_rgb(30, 40, 30),
                    format!("✓ Applied — hunk {}", app.current_hunk + 1),
                )
            } else {
                (
                    banner_bg,
                    format!(
                        "{:.0}%  match @ lines {}–{}",
                        mr.score,
                        mr.file_start + 1,
                        mr.file_end
                    ),
                )
            };
            let desired = Vec2::new(ui.available_width(), row_h + 2.0);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            ui.painter().rect_filled(rect, 2.0, banner_bg);
            ui.painter().text(
                Pos2::new(rect.left() + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                &banner_text,
                FontId::monospace(11.0),
                if is_applied { pal::TEXT_DIM } else { banner_fg },
            );
            ui.add_space(2.0);
            let search_file_map: Vec<Option<usize>> = app
                .search_rows
                .iter()
                .filter(|r| matches!(r.kind, RowKind::Equal | RowKind::Delete))
                .map(|r| r.file_idx)
                .collect();
            let pointer_down = ui.input(|i| i.pointer.primary_down());
            let pointer_pressed = ui.input(|i| i.pointer.primary_pressed());
            let pointer_dragging = ui.input(|i| i.pointer.is_decidedly_dragging());
            for (line_idx, line) in hunk.search.iter().enumerate() {
                let file_idx = search_file_map.get(line_idx).copied().flatten();
                let is_matched = file_idx.is_some();
                let (base_bg, prefix_color, prefix) = if is_matched {
                    (pal::BG_MATCH, pal::TEXT_INSERT, "= ")
                } else {
                    (pal::BG_DELETE, pal::TEXT_DELETE, "- ")
                };
                let is_selected = app
                    .left_selection
                    .map_or(false, |(s, e)| line_idx >= s && line_idx <= e);
                let bg = if is_selected {
                    Color32::from_rgb(50, 50, 70)
                } else {
                    base_bg
                };
                let desired = Vec2::new(ui.available_width(), row_h);
                let (rect, resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());
                if resp.hovered() {
                    if pointer_pressed {
                        set_selection = Some((line_idx, line_idx));
                    } else if pointer_dragging && pointer_down {
                        if let Some(start_sel) = app.left_selection {
                            set_selection =
                                Some((start_sel.0.min(line_idx), start_sel.1.max(line_idx)));
                        }
                    }
                }

                // Double-click handler to search for the line text in the right-side file panel
                if resp.double_clicked() {
                    let q = line.trim().to_string();
                    app.file_search_query = q.clone();
                    let q_lower = q.to_lowercase();
                    if q_lower.is_empty() {
                        app.search_matches.clear();
                    } else {
                        app.search_matches = app
                            .file_lines
                            .iter()
                            .enumerate()
                            .filter(|(_, l)| l.to_lowercase().contains(&q_lower))
                            .map(|(i, _)| i)
                            .collect();
                        if !app.search_matches.is_empty() {
                            app.search_match_idx = 0;
                            app.cursor_line = Some(app.search_matches[0]);
                            app.scroll_to_match = true;
                        } else {
                            app.search_matches.clear();
                            app.set_message(StatusMessage::warning(format!(
                                "No matches found for '{}'",
                                q
                            )));
                        }
                    }
                }

                ui.painter().rect_filled(rect, 0.0, bg);
                let bar = Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height()));
                ui.painter().rect_filled(
                    bar,
                    0.0,
                    if is_matched {
                        pal::BAR_MATCH
                    } else {
                        pal::TEXT_DELETE
                    },
                );
                let num_text = if let Some(fi) = file_idx {
                    format!("{:>4}", fi + 1)
                } else {
                    format!("{:>4}", line_idx + 1)
                };
                ui.painter().text(
                    Pos2::new(rect.left() + 4.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &num_text,
                    FontId::monospace(11.0),
                    if is_matched {
                        pal::TEXT_LNUM_ACTIVE
                    } else {
                        pal::TEXT_DIM
                    },
                );
                ui.painter().text(
                    Pos2::new(rect.left() + 38.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    prefix,
                    FontId::monospace(11.0),
                    prefix_color,
                );
                let display = MergeApp::truncate_owned(line, max_chars);
                ui.painter().text(
                    Pos2::new(rect.left() + 54.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &display,
                    FontId::monospace(11.0),
                    if is_applied {
                        pal::TEXT_DIM
                    } else {
                        pal::TEXT_NORMAL
                    },
                );
            }
            if !hunk.replace.is_empty() {
                ui.add_space(4.0);
                let (sep_rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(sep_rect, 0.0, pal::SEPARATOR);
                ui.add_space(2.0);
                let (hdr_rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover());
                ui.painter()
                    .rect_filled(hdr_rect, 0.0, Color32::from_rgb(22, 44, 28));
                ui.painter().text(
                    Pos2::new(hdr_rect.left() + 8.0, hdr_rect.center().y),
                    Align2::LEFT_CENTER,
                    "REPLACE →",
                    FontId::monospace(10.0),
                    pal::TEXT_INSERT,
                );
                let btn_size = Vec2::new(30.0, row_h - 4.0);
                let btn_line_size = Vec2::new(55.0, row_h - 4.0);
                let mut x_offset = 4.0;
                let btn_rect = Rect::from_min_size(
                    Pos2::new(
                        hdr_rect.right() - btn_size.x - x_offset,
                        hdr_rect.center().y - btn_size.y / 2.0,
                    ),
                    btn_size,
                );
                let btn_text = if let Some((&id, _)) = app.file_anchors.iter().next() {
                    format!(">{}", id)
                } else {
                    ">".to_string()
                };
                let btn = Button::new(
                    RichText::new(&btn_text)
                        .color(Color32::WHITE)
                        .strong()
                        .monospace(),
                )
                .fill(Color32::from_rgb(40, 90, 55))
                .min_size(btn_size);
                let resp = ui.put(btn_rect, btn);
                if resp.clicked() {
                    if let Some((&id, _)) = app.file_anchors.iter().next() {
                        apply_clicked_id = Some(id);
                    } else {
                        apply_clicked = true;
                    }
                }
                resp.context_menu(|ui| {
                    if app.file_anchors.is_empty() {
                        ui.label("No markers set.");
                        ui.label("Use 'm' + letter in file panel.");
                    } else {
                        ui.label("Select target marker:");
                        ui.separator();
                        for (&mid, _) in app.file_anchors.iter() {
                            if ui.button(format!(">{}", mid)).clicked() {
                                apply_clicked_id = Some(mid);
                                ui.close_menu();
                            }
                        }
                    }
                });
                x_offset += btn_size.x + 4.0;
                if let Some(cur_ln) = app.cursor_line {
                    let btn_line_rect = Rect::from_min_size(
                        Pos2::new(
                            hdr_rect.right() - btn_line_size.x - x_offset,
                            hdr_rect.center().y - btn_line_size.y / 2.0,
                        ),
                        btn_line_size,
                    );
                    let btn_line = Button::new(
                        RichText::new(format!(">({})", cur_ln + 1))
                            .color(Color32::WHITE)
                            .strong()
                            .monospace(),
                    )
                    .fill(Color32::from_rgb(40, 90, 55))
                    .min_size(btn_line_size);
                    if ui.put(btn_line_rect, btn_line).clicked() {
                        apply_clicked_line = Some(cur_ln);
                    }
                }
                for (line_idx, line) in hunk.replace.iter().enumerate() {
                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, pal::BG_INSERT);
                    let bar = Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height()));
                    ui.painter().rect_filled(bar, 0.0, pal::BAR_MATCH);
                    ui.painter().text(
                        Pos2::new(rect.left() + 4.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("{:>4}", line_idx + 1),
                        FontId::monospace(11.0),
                        pal::TEXT_DIM,
                    );
                    ui.painter().text(
                        Pos2::new(rect.left() + 38.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        "+ ",
                        FontId::monospace(11.0),
                        pal::TEXT_INSERT,
                    );
                    let display = MergeApp::truncate_owned(line, max_chars);
                    ui.painter().text(
                        Pos2::new(rect.left() + 54.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        FontId::monospace(11.0),
                        if is_applied {
                            pal::TEXT_DIM
                        } else {
                            Color32::from_rgb(155, 235, 165)
                        },
                    );
                }
            }
        });
    if let Some(sel) = set_selection {
        app.left_selection = Some(sel);
    }
    if apply_clicked {
        app.apply_merge(None, None);
    }
    if let Some(id) = apply_clicked_id {
        app.apply_merge(None, Some(id));
    }
    if let Some(ln) = apply_clicked_line {
        app.apply_merge(Some(ln), None);
    }
}

fn render_file_panel(
    app: &mut MergeApp,
    ui: &mut Ui,
    mr: &crate::diff::MatchResult,
    row_h: f32,
    char_w: f32,
    panel_w: f32,
) {
    let max_chars = ((panel_w - 68.0) / char_w).floor() as usize;
    let mut prev_hunk = false;
    let mut next_hunk = false;
    let mut prev_candidate = false;
    let mut next_candidate = false;
    let mut clear_marks_flag = false;
    let mut apply_clicked = false;
    let mut apply_clicked_id: Option<char> = None;
    let mut find_text = false;
    let mut next_search_match = false;
    let mut prev_search_match = false;
    let mut clear_search = false;
    let mut go_next_hunk = false;
    let mut go_prev_hunk = false;
    let mut go_next_file = false;
    let mut go_prev_file = false;
    let current_hunk_idx = app.current_hunk;
    let total_hunks = app.hunks.len();
    let file_anchors = app.file_anchors.clone();
    let candidate_count = mr.candidates.len();
    let candidate_idx = app.candidate_index;
    let is_applied = app.applied_hunks.contains(&app.current_hunk);
    let can_apply = !is_applied && (app.match_result.is_some() || !app.file_anchors.is_empty());
    let apply_line = if file_anchors.is_empty() {
        mr.file_start + 1
    } else {
        file_anchors.values().next().unwrap().line + 1
    };
    let mut unique_files = Vec::new();
    for h in &app.hunks {
        if !unique_files.contains(&h.filename) {
            unique_files.push(h.filename.clone());
        }
    }
    let current_file_name = app
        .current_hunk()
        .map(|h| h.filename.clone())
        .unwrap_or_default();
    let current_file_idx = unique_files
        .iter()
        .position(|f| *f == current_file_name)
        .unwrap_or(0);
    Frame::none()
        .fill(Color32::from_rgb(25, 32, 42))
        .inner_margin(Margin::symmetric(6.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if unique_files.len() > 1 {
                    ui.label(RichText::new("File:").color(pal::TEXT_DIM).small());
                    if ui
                        .add_enabled(current_file_idx > 0, Button::new("◀").small())
                        .clicked()
                    {
                        go_prev_file = true;
                    }
                    ui.label(
                        RichText::new(format!(
                            "{}/{}{}",
                            current_file_idx + 1,
                            unique_files.len(),
                            if app.filter_low_matches {
                                " (filtered)"
                            } else {
                                ""
                            }
                        ))
                        .monospace(),
                    );
                    if ui
                        .add_enabled(
                            current_file_idx + 1 < unique_files.len(),
                            Button::new("▶").small(),
                        )
                        .clicked()
                    {
                        go_next_file = true;
                    }
                    ui.separator();
                }
                ui.label(RichText::new("Hunk:").color(pal::TEXT_DIM).small());
                if ui
                    .add_enabled(current_hunk_idx > 0, Button::new("◀").small())
                    .on_hover_text("Previous hunk (Shift+L)")
                    .clicked()
                {
                    prev_hunk = true;
                }
                ui.label(
                    RichText::new(format!(
                        "{}/{}{}",
                        current_hunk_idx + 1,
                        total_hunks,
                        if app.filter_low_matches {
                            " (filtered)"
                        } else {
                            ""
                        }
                    ))
                    .monospace(),
                );
                if ui
                    .add_enabled(current_hunk_idx < total_hunks - 1, Button::new("▶").small())
                    .on_hover_text("Next hunk (L)")
                    .clicked()
                {
                    next_hunk = true;
                }
                if is_applied {
                    ui.label(RichText::new("✓").color(pal::ACCENT_GOOD).strong());
                }
                ui.add(Separator::default().vertical());
                if !file_anchors.is_empty() {
                    let labels: Vec<String> = file_anchors.values().map(|f| f.label()).collect();
                    ui.label(
                        RichText::new(format!("⚓ {}", labels.join("  ")))
                            .color(pal::TEXT_ANCHOR)
                            .monospace(),
                    );
                    if ui
                        .small_button("✕")
                        .on_hover_text("Clear marks (Esc)")
                        .clicked()
                    {
                        clear_marks_flag = true;
                    }
                } else {
                    if ui
                        .add(Button::new("^").small())
                        .on_hover_text("Previous (Shift+L)")
                        .clicked()
                    {
                        if candidate_count > 1 && candidate_idx > 0 {
                            prev_candidate = true;
                        } else {
                            go_prev_hunk = true;
                        }
                    }
                    if ui
                        .add(Button::new("v").small())
                        .on_hover_text("Next (L)")
                        .clicked()
                    {
                        if candidate_count > 1 && candidate_idx + 1 < candidate_count {
                            next_candidate = true;
                        } else {
                            go_next_hunk = true;
                        }
                    }
                }
                ui.add(Separator::default().vertical());
                if !app.file_search_query.is_empty() && !app.is_searching {
                    ui.label(
                        RichText::new(format!("🔍 {}", app.file_search_query))
                            .color(pal::TEXT_SEARCH)
                            .monospace()
                            .small(),
                    );
                    if ui
                        .small_button("✕")
                        .on_hover_text("Clear search (Esc)")
                        .clicked()
                    {
                        clear_search = true;
                    }
                }
                ui.add(Separator::default().vertical());
                ui.add_enabled_ui(can_apply, |ui| {
                    let btn_text = if is_applied {
                        "✓ Applied".to_string()
                    } else {
                        format!("⚡ Apply @ {}", apply_line)
                    };
                    let btn = Button::new(RichText::new(&btn_text).strong().monospace()).fill(
                        if can_apply {
                            Color32::from_rgb(40, 90, 55)
                        } else {
                            Color32::from_gray(35)
                        },
                    );
                    if ui
                        .add(btn)
                        .on_hover_text("Apply this hunk to the file (A when cursor is in match)")
                        .clicked()
                    {
                        apply_clicked = true;
                    }
                });
                ui.add(Separator::default().vertical());
                if ui
                    .selectable_label(app.show_git_diff_window, "📝 Git Diff (F4)")
                    .clicked()
                {
                    app.show_git_diff_window = !app.show_git_diff_window;
                }
                if ui.selectable_label(app.show_debug, "🐞 Debug").clicked() {
                    app.show_debug = !app.show_debug;
                }
            });
        });
    ui.add(Separator::default());
    let len = app.file_lines.len();
    if len > 0 {
        if app.is_searching {
            ui.input(|i| {
                if i.key_pressed(Key::Enter) {
                    app.is_searching = false;
                    find_text = true;
                }
                for event in i.events.clone() {
                    match event {
                        Event::Text(txt) => {
                            if txt != "\n" && txt != "\r" {
                                app.file_search_query.push_str(&txt);
                            }
                        }
                        Event::Key {
                            key: Key::Backspace,
                            pressed: true,
                            ..
                        } => {
                            app.file_search_query.pop();
                        }
                        _ => {}
                    }
                }
            });

        // In src/app/split_view.rs (inside render_file_panel)
        } else if !ui.ctx().wants_keyboard_input() {
            let mut cursor_changed = false;
            let mut new_text = String::new();
            ui.input(|i| {
                let cur = app.cursor_line.unwrap_or(0);
                if i.key_pressed(Key::Equals) && i.modifiers.alt {
                    go_next_file = true;
                }
                if i.key_pressed(Key::Minus) && i.modifiers.alt {
                    go_prev_file = true;
                }
                if i.key_pressed(Key::ArrowDown) {
                    app.cursor_line = Some((cur + 1).min(len - 1));
                    cursor_changed = true;
                }
                if i.key_pressed(Key::ArrowUp) {
                    app.cursor_line = Some(cur.saturating_sub(1));
                    cursor_changed = true;
                }
                if i.key_pressed(Key::PageDown) {
                    app.cursor_line = Some((cur + 20).min(len - 1));
                    cursor_changed = true;
                }
                if i.key_pressed(Key::PageUp) {
                    app.cursor_line = Some(cur.saturating_sub(20));
                    cursor_changed = true;
                }
                if i.key_pressed(Key::Home) {
                    app.cursor_line = Some(0);
                    cursor_changed = true;
                }
                if i.key_pressed(Key::End) {
                    app.cursor_line = Some(len - 1);
                    cursor_changed = true;
                }
                if i.key_pressed(Key::Escape) {
                    if app.d_pending {
                        app.d_pending = false;
                        app.vim_buffer.clear();
                    }
                }
                if i.key_pressed(Key::D) {
                    app.d_pending = true;
                }
                if i.key_pressed(Key::L) && !app.d_pending && app.vim_buffer.is_empty() {
                    if i.modifiers.shift {
                        if candidate_count > 1 && candidate_idx > 0 {
                            prev_candidate = true;
                        } else {
                            go_prev_hunk = true;
                        }
                    } else {
                        if candidate_count > 1 && candidate_idx + 1 < candidate_count {
                            next_candidate = true;
                        } else {
                            go_next_hunk = true;
                        }
                    }
                }
                if i.key_pressed(Key::Slash) && !app.d_pending && app.vim_buffer.is_empty() {
                    app.is_searching = true;
                    app.file_search_query.clear();
                    app.search_matches.clear();
                    clear_search = true;
                }
                if i.key_pressed(Key::Space) && !app.d_pending && app.vim_buffer.is_empty() {
                    if let Some(cur) = app.cursor_line {
                        app.set_mark_a(cur);
                    }
                }
                if i.key_pressed(Key::A) && !app.d_pending && app.vim_buffer.is_empty() {
                    let in_hunk = if file_anchors.is_empty() {
                        cur >= mr.file_start && cur < mr.file_end
                    } else {
                        file_anchors.values().any(|f| f.line == cur)
                    };
                    if is_applied {
                    } else if in_hunk {
                        apply_clicked = true;
                    } else {
                        app.cursor_line = Some(mr.file_start);
                        cursor_changed = true;
                    }
                }
                for event in i.events.clone() {
                    if let Event::Text(txt) = event {
                        if txt == "m" {
                            app.mark_pending = Some(MarkPending::WaitingKey);
                        } else if app.mark_pending == Some(MarkPending::WaitingKey) {
                            if txt.len() == 1 {
                                let c = txt.chars().next().unwrap();
                                if c.is_ascii_alphabetic() {
                                    if let Some(cur) = app.cursor_line {
                                        app.set_mark(c, cur);
                                    }
                                    app.mark_pending = None;
                                } else {
                                    app.mark_pending = None;
                                }
                            }
                        } else if txt != "?" && txt != "m" && txt != "o" && txt != "O" {
                            new_text.push_str(&txt);
                        }
                    }
                }
            });
            if !new_text.is_empty() {
                app.vim_buffer.push_str(&new_text);
                let buf = app.vim_buffer.trim().to_string();
                let lower_buf = buf.to_lowercase();
                let mut clear_buffer = false;
                if buf == "n" {
                    next_search_match = true;
                    clear_buffer = true;
                } else if buf == "N" {
                    prev_search_match = true;
                    clear_buffer = true;
                } else if buf == "]h" {
                    let cur = app.cursor_line.unwrap_or(0);
                    let mut hunk_starts: Vec<usize> = app
                        .git_hunks
                        .iter()
                        .map(|h| h.current_line_range.start)
                        .collect();
                    hunk_starts.sort();
                    if !hunk_starts.is_empty() {
                        let mut next_line = None;
                        for &start in &hunk_starts {
                            if start > cur {
                                next_line = Some(start);
                                break;
                            }
                        }
                        let target = next_line.unwrap_or(hunk_starts[0]);
                        app.cursor_line = Some(target);
                        app.scroll_to_match = true;
                    }
                    clear_buffer = true;
                } else if buf == "[h" {
                    let cur = app.cursor_line.unwrap_or(0);
                    let mut hunk_starts: Vec<usize> = app
                        .git_hunks
                        .iter()
                        .map(|h| h.current_line_range.start)
                        .collect();
                    hunk_starts.sort();
                    if !hunk_starts.is_empty() {
                        let mut prev_line = None;
                        for &start in hunk_starts.iter().rev() {
                            if start < cur {
                                prev_line = Some(start);
                                break;
                            }
                        }
                        let target = prev_line.unwrap_or(*hunk_starts.last().unwrap());
                        app.cursor_line = Some(target);
                        app.scroll_to_match = true;
                    }
                    clear_buffer = true;
                } else if lower_buf == "u" {
                    app.undo();
                    clear_buffer = true;
                } else if lower_buf == "." {
                    if let Some(action) = app.last_action.clone() {
                        match action {
                            Action::DeleteLines(count) => app.delete_lines(count),
                            Action::DeleteFunction => app.delete_function_around_cursor(),
                        }
                    }
                    clear_buffer = true;
                } else if buf == "gg" {
                    app.cursor_line = Some(0);
                    app.scroll_to_match = true;
                    clear_buffer = true;
                } else if buf == "G" {
                    app.cursor_line = Some(app.file_lines.len().saturating_sub(1));
                    app.scroll_to_match = true;
                    clear_buffer = true;
                } else if lower_buf == "daf" {
                    app.delete_function_around_cursor();
                    app.last_action = Some(Action::DeleteFunction);
                    clear_buffer = true;
                } else if lower_buf.ends_with("dd") {
                    let num_part = &lower_buf[..lower_buf.len() - 2];
                    let count = if num_part.is_empty() {
                        1
                    } else {
                        num_part.parse::<usize>().unwrap_or(0)
                    };
                    if count > 0 {
                        app.delete_lines(count);
                        app.last_action = Some(Action::DeleteLines(count));
                    }
                    clear_buffer = true;
                } else if buf.len() > 5 {
                    clear_buffer = true;
                } else {
                    let allowed = buf.chars().all(|c| {
                        c.is_ascii_digit()
                            || c == 'd'
                            || c == 'D'
                            || c == 'g'
                            || c == 'G'
                            || c == '['
                            || c == ']'
                            || c == 'h'
                    }) || lower_buf == "da"
                        || lower_buf == "daf";
                    let d_count = buf.matches('d').count() + buf.matches('D').count();
                    if !allowed || d_count > 2 {
                        clear_buffer = true;
                    }
                }
                if clear_buffer {
                    app.vim_buffer.clear();
                    app.d_pending = false; // Reset pending state
                }
            }

            if cursor_changed {
                app.scroll_to_match = true;
            }
        }
    }
    if go_prev_file {
        let mut prev_file_hunk = None;
        for (i, h) in app.hunks.iter().enumerate() {
            if i < app.current_hunk && h.filename != current_file_name {
                if !app.filter_low_matches || app.is_hunk_match_ok(i) {
                    prev_file_hunk = Some(i);
                }
            }
        }
        if let Some(idx) = prev_file_hunk {
            app.current_hunk = idx;
            app.load_hunk();
            return;
        }
    }
    if go_next_file {
        let mut next_file_hunk = None;
        for (i, h) in app.hunks.iter().enumerate() {
            if i > app.current_hunk && h.filename != current_file_name {
                if !app.filter_low_matches || app.is_hunk_match_ok(i) {
                    next_file_hunk = Some(i);
                    break;
                }
            }
        }
        if let Some(idx) = next_file_hunk {
            app.current_hunk = idx;
            app.load_hunk();
            return;
        }
    }
    if prev_hunk && current_hunk_idx > 0 {
        if app.filter_low_matches {
            let mut target = None;
            for i in (0..current_hunk_idx).rev() {
                if app.is_hunk_match_ok(i) {
                    target = Some(i);
                    break;
                }
            }
            if let Some(idx) = target {
                app.current_hunk = idx;
                app.load_hunk();
                return;
            } else {
                app.set_message(StatusMessage::info("No previous hunk matching >= 60%"));
            }
        } else {
            app.current_hunk -= 1;
            app.load_hunk();
            return;
        }
    }
    if next_hunk && current_hunk_idx < total_hunks - 1 {
        if app.filter_low_matches {
            let mut target = None;
            for i in current_hunk_idx + 1..total_hunks {
                if app.is_hunk_match_ok(i) {
                    target = Some(i);
                    break;
                }
            }
            if let Some(idx) = target {
                app.current_hunk = idx;
                app.load_hunk();
                return;
            } else {
                app.set_message(StatusMessage::info("No next hunk matching >= 60%"));
            }
        } else {
            app.current_hunk += 1;
            app.load_hunk();
            return;
        }
    }
    if clear_marks_flag {
        app.clear_marks();
    }
    if prev_candidate && app.candidate_index > 0 {
        app.candidate_index -= 1;
        app.scroll_to_match = true;
        app.recompute_match();
        return;
    }
    if next_candidate && app.candidate_index + 1 < candidate_count {
        app.candidate_index += 1;
        app.scroll_to_match = true;
        app.recompute_match();
        return;
    }
    if clear_search {
        app.file_search_query.clear();
        app.search_matches.clear();
        app.scroll_to_match = true;
    }
    if find_text {
        let q = app.file_search_query.trim().to_lowercase();
        if q.is_empty() {
            app.search_matches.clear();
        } else {
            app.search_matches = app
                .file_lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
            if !app.search_matches.is_empty() {
                app.search_match_idx = 0;
                app.cursor_line = Some(app.search_matches[0]);
                app.scroll_to_match = true;
            } else {
                app.search_matches.clear();
                app.set_message(StatusMessage::warning(format!("No matches for '{}'", q)));
            }
        }
    }
    if next_search_match && !app.search_matches.is_empty() {
        app.search_match_idx = (app.search_match_idx + 1) % app.search_matches.len();
        app.cursor_line = Some(app.search_matches[app.search_match_idx]);
        app.scroll_to_match = true;
    }
    if prev_search_match && !app.search_matches.is_empty() {
        if app.search_match_idx > 0 {
            app.search_match_idx -= 1;
        } else {
            app.search_match_idx = app.search_matches.len() - 1;
        }
        app.cursor_line = Some(app.search_matches[app.search_match_idx]);
        app.scroll_to_match = true;
    }
    if go_next_hunk {
        if app.current_hunk < app.hunks.len() - 1 {
            if app.filter_low_matches {
                let mut target = None;
                for i in app.current_hunk + 1..app.hunks.len() {
                    if app.is_hunk_match_ok(i) {
                        target = Some(i);
                        break;
                    }
                }
                if let Some(idx) = target {
                    app.current_hunk = idx;
                    app.load_hunk();
                    return;
                }
            } else {
                app.current_hunk += 1;
                app.load_hunk();
                return;
            }
        } else {
            app.cursor_line = Some(mr.file_start);
            app.scroll_to_match = true;
        }
    }
    if go_prev_hunk {
        if app.current_hunk > 0 {
            if app.filter_low_matches {
                let mut target = None;
                for i in (0..app.current_hunk).rev() {
                    if app.is_hunk_match_ok(i) {
                        target = Some(i);
                        break;
                    }
                }
                if let Some(idx) = target {
                    app.current_hunk = idx;
                    app.load_hunk();
                    return;
                }
            } else {
                app.current_hunk -= 1;
                app.load_hunk();
                return;
            }
        } else {
            app.cursor_line = Some(mr.file_start);
            app.scroll_to_match = true;
        }
    }
    let file_lines = app.file_lines.clone();
    let merged_range = app.merged_range;
    let auto_start = mr.file_start;
    let auto_end = mr.file_end;
    let auto_score = mr.score;
    let search_query = app.file_search_query.clone();
    let current_search_line = app.search_matches.get(app.search_match_idx).copied();
    let scroll_to_match = app.scroll_to_match;
    let cursor_line = app.cursor_line;
    let git_statuses = app.git_statuses.clone();
    let mut did_scroll = false;
    let mut set_cursor: Option<usize> = None;
    let delete_file_indices: HashSet<usize> = app
        .search_rows
        .iter()
        .filter(|r| matches!(r.kind, RowKind::Delete))
        .filter_map(|r| r.file_idx)
        .collect();
    let equal_file_indices: HashSet<usize> = app
        .search_rows
        .iter()
        .filter(|r| matches!(r.kind, RowKind::Equal))
        .filter_map(|r| r.file_idx)
        .collect();
    ScrollArea::both()
        .id_source("file_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, line) in file_lines.iter().enumerate() {
                let in_auto_match = i >= auto_start && i < auto_end;
                let anchor_here = file_anchors.values().find(|a| a.line == i);
                let is_anchor = anchor_here.is_some();
                let is_cursor = cursor_line == Some(i);
                let in_merged = merged_range.map_or(false, |(rs, re)| i >= rs && i < re);
                let is_delete = in_auto_match && delete_file_indices.contains(&i);
                let is_equal = in_auto_match && equal_file_indices.contains(&i);
                let is_search_hit = !search_query.is_empty()
                    && line.to_lowercase().contains(&search_query.to_lowercase());
                let is_current_search = is_search_hit && current_search_line == Some(i);
                let is_auto_start_line =
                    in_auto_match && i == auto_start && file_anchors.is_empty();
                let git_status = git_statuses.get(i).copied().unwrap_or(GitStatus::Unchanged);
                let row_is_tall = is_anchor;
                let desired = Vec2::new(
                    ui.available_width(),
                    if row_is_tall { row_h + 6.0 } else { row_h },
                );
                let (rect, row_resp) = ui.allocate_exact_size(desired, Sense::click());
                let should_scroll = scroll_to_match
                    && (is_cursor
                        || (cursor_line.is_none() && is_anchor)
                        || (cursor_line.is_none() && is_auto_start_line));
                if should_scroll {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    did_scroll = true;
                }
                if let Some(anchor) = anchor_here {
                    let anchor_bg = pal::BG_ANCHOR;
                    ui.painter().rect_filled(rect, 2.0, anchor_bg);
                    let dash_y = rect.top() + 2.0;
                    let mut x = rect.left() + 4.0;
                    while x < rect.right() - 130.0 {
                        ui.painter().line_segment(
                            [
                                Pos2::new(x, dash_y),
                                Pos2::new((x + 8.0).min(rect.right() - 130.0), dash_y),
                            ],
                            Stroke::new(1.5, pal::BAR_ANCHOR),
                        );
                        x += 14.0;
                    }
                    let label =
                        format!("⚓ m{}:{} — insert / replace before here", anchor.id, i + 1);
                    ui.painter().text(
                        Pos2::new(rect.left() + 10.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        label,
                        FontId::monospace(10.5),
                        pal::TEXT_ANCHOR,
                    );
                    let btn_size = Vec2::new(100.0, row_h);
                    let btn_rect = Rect::from_min_size(
                        Pos2::new(rect.right() - 106.0, rect.center().y - row_h / 2.0),
                        btn_size,
                    );
                    if ui
                        .put(
                            btn_rect,
                            Button::new(
                                RichText::new(format!("⚡ >{}", anchor.id))
                                    .color(Color32::WHITE)
                                    .strong()
                                    .monospace(),
                            )
                            .fill(Color32::from_rgb(90, 70, 15))
                            .stroke(Stroke::new(1.0, pal::BAR_ANCHOR)),
                        )
                        .clicked()
                    {
                        apply_clicked_id = Some(anchor.id);
                    }
                } else {
                    let base_bg = if in_merged {
                        pal::BG_MERGED
                    } else if is_delete {
                        pal::BG_DELETE
                    } else if is_cursor {
                        pal::BG_CURSOR
                    } else if in_auto_match && file_anchors.is_empty() && !is_auto_start_line {
                        pal::BG_MATCH
                    } else if i % 2 == 0 {
                        pal::BG_ROW_EVEN
                    } else {
                        pal::BG_ROW_ODD
                    };
                    let final_bg = if is_auto_start_line {
                        Color32::TRANSPARENT
                    } else {
                        base_bg
                    };
                    let row_bg = if is_current_search {
                        Color32::from_rgb(70, 60, 15)
                    } else if is_search_hit {
                        pal::BG_SEARCH_HIT
                    } else {
                        final_bg
                    };
                    ui.painter().rect_filled(rect, 0.0, row_bg);
                    let git_color = match git_status {
                        GitStatus::Added => Color32::from_rgb(40, 130, 60),
                        GitStatus::Modified => Color32::from_rgb(200, 160, 40),
                        GitStatus::Deleted => Color32::from_rgb(180, 40, 40),
                        _ => Color32::TRANSPARENT,
                    };
                    if git_color != Color32::TRANSPARENT {
                        let git_bar = Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height()));
                        ui.painter().rect_filled(git_bar, 0.0, git_color);
                    }
                    if git_status != GitStatus::Unchanged {
                        if let Some(hunk) = app
                            .git_hunks
                            .iter()
                            .find(|h| h.current_line_range.contains(&i))
                        {
                            row_resp.clone().on_hover_ui(|ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    ui.label(
                                        RichText::new("Git Diff")
                                            .color(Color32::from_rgb(100, 160, 230))
                                            .strong(),
                                    );
                                });
                                ui.separator();
                                for row in &hunk.rows {
                                    match row.kind {
                                        RowKind::Delete => {
                                            if let Some(ref text) = row.left {
                                                ui.colored_label(
                                                    pal::TEXT_DELETE,
                                                    format!("- {}", text),
                                                );
                                            }
                                        }
                                        RowKind::Insert => {
                                            if let Some(ref text) = row.right {
                                                ui.colored_label(
                                                    pal::TEXT_INSERT,
                                                    format!("+ {}", text),
                                                );
                                            }
                                        }
                                        RowKind::Equal => {
                                            if let Some(ref text) = row.right {
                                                ui.colored_label(
                                                    pal::TEXT_DIM,
                                                    format!("  {}", text),
                                                );
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                    let bar = Rect::from_min_size(
                        Pos2::new(rect.left() + 2.0, rect.top()),
                        Vec2::new(3.0, rect.height()),
                    );
                    let bar_color = if in_merged {
                        pal::BAR_MERGED
                    } else if is_delete {
                        pal::TEXT_DELETE
                    } else if is_cursor {
                        pal::BAR_CURSOR
                    } else if in_auto_match && file_anchors.is_empty() {
                        pal::BAR_MATCH
                    } else if is_current_search {
                        pal::ACCENT_WARN
                    } else if is_search_hit {
                        pal::BAR_SEARCH
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(bar, 0.0, bar_color);
                    if row_resp.clicked() {
                        set_cursor = Some(i);
                    }
                    let num_color = if in_merged {
                        pal::TEXT_LNUM_ACTIVE
                    } else if is_delete {
                        pal::TEXT_DELETE
                    } else if in_auto_match && file_anchors.is_empty() {
                        pal::TEXT_LNUM_ACTIVE
                    } else if is_search_hit {
                        Color32::from_rgb(180, 160, 60)
                    } else {
                        pal::TEXT_LNUM
                    };
                    ui.painter().text(
                        Pos2::new(rect.left() + 6.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("{:>4} │", i + 1),
                        FontId::monospace(11.0),
                        num_color,
                    );
                    let diff_prefix = if in_auto_match && file_anchors.is_empty() {
                        if is_delete {
                            Some(("-", pal::TEXT_DELETE))
                        } else if is_equal {
                            Some(("=", Color32::from_gray(60)))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some((glyph, glyph_color)) = diff_prefix {
                        ui.painter().text(
                            Pos2::new(rect.left() + 48.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            glyph,
                            FontId::monospace(11.0),
                            glyph_color,
                        );
                    }
                    let text_color = if in_merged {
                        pal::TEXT_MERGED
                    } else if is_delete {
                        pal::TEXT_DELETE
                    } else if in_auto_match && file_anchors.is_empty() {
                        pal::TEXT_MATCH
                    } else if is_search_hit {
                        pal::TEXT_SEARCH
                    } else {
                        pal::TEXT_NORMAL
                    };
                    let display_max_chars = if is_auto_start_line {
                        ((panel_w - 68.0 - 215.0) / char_w).floor() as usize
                    } else {
                        max_chars
                    };
                    let display = MergeApp::truncate_owned(line, display_max_chars);
                    ui.painter().text(
                        Pos2::new(rect.left() + 58.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        FontId::monospace(11.0),
                        text_color,
                    );
                    if is_auto_start_line {
                        let right_box_width = 215.0;
                        let right_box_rect = Rect::from_min_size(
                            Pos2::new(rect.right() - right_box_width, rect.top()),
                            Vec2::new(right_box_width, rect.height()),
                        );
                        ui.painter().rect_filled(
                            right_box_rect,
                            2.0,
                            Color32::from_rgba_premultiplied(28, 60, 40, 230),
                        );
                        ui.painter().text(
                            Pos2::new(right_box_rect.left() + 8.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!("▼ {}–{} ({:.0}%)", auto_start + 1, auto_end, auto_score),
                            FontId::monospace(10.5),
                            Color32::from_rgb(120, 230, 160),
                        );
                        let btn_size = Vec2::new(90.0, row_h - 4.0);
                        let btn_rect = Rect::from_min_size(
                            Pos2::new(
                                right_box_rect.right() - btn_size.x - 4.0,
                                rect.center().y - btn_size.y / 2.0,
                            ),
                            btn_size,
                        );
                        if ui
                            .put(
                                btn_rect,
                                Button::new(
                                    RichText::new("⚡ Apply")
                                        .color(Color32::WHITE)
                                        .strong()
                                        .monospace(),
                                )
                                .fill(Color32::from_rgb(35, 85, 50))
                                .stroke(Stroke::new(1.0, pal::BAR_MATCH)),
                            )
                            .clicked()
                        {
                            apply_clicked = true;
                        }
                    }
                }
                if in_auto_match && i == auto_end.saturating_sub(1) && file_anchors.is_empty() {
                    let (sep_rect, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 2.0), Sense::hover());
                    ui.painter().rect_filled(sep_rect, 0.0, pal::BAR_MATCH);
                }
            }
            ui.add_space(row_h * 3.0);
        });
    if scroll_to_match && !did_scroll {
        did_scroll = true;
    }
    if did_scroll {
        app.scroll_to_match = false;
    }
    if let Some(cur_line) = set_cursor {
        app.cursor_line = Some(cur_line);
        if let Some(idx) = app.search_matches.iter().position(|&x| x == cur_line) {
            app.search_match_idx = idx;
        }
    }
    if apply_clicked {
        app.apply_merge(None, None);
    }
    if let Some(id) = apply_clicked_id {
        app.apply_merge(None, Some(id));
    }
}
