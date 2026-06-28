use super::matching::MergeMatching;
use super::palette::pal;
use super::state::MergeApp;
use super::types::{Action, SearchRow};
use crate::diff::RowKind;
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
        Frame::none()
            .fill(Color32::from_rgb(28, 38, 58))
            .inner_margin(Margin::symmetric(8.0, 3.0))
            .show(ui, |ui| {
                ui.set_min_width(left_w);
                ui.set_max_width(left_w);
                let hunk = app.current_hunk().unwrap();
                ui.label(
                    RichText::new(format!("SEARCH  ·  {}", hunk.filename))
                        .color(Color32::from_rgb(120, 180, 255))
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
                ui.label(
                    RichText::new(format!(
                        "FILE  ·  {} lines  ·  match @ {}–{}",
                        app.file_lines.len(),
                        mr.file_start + 1,
                        mr.file_end,
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
    render_search_panel(app, &mut left_ui, &mr, row_h, char_w, left_w);

    let mut right_ui = ui.child_ui(right_rect, Layout::top_down(Align::LEFT), None);
    render_file_panel(app, &mut right_ui, &mr, row_h, char_w, right_w);
}

fn render_search_panel(
    app: &MergeApp,
    ui: &mut Ui,
    mr: &crate::diff::MatchResult,
    row_h: f32,
    char_w: f32,
    panel_w: f32,
) {
    let max_chars = ((panel_w - 58.0) / char_w).floor() as usize;
    ScrollArea::vertical()
        .id_source("search_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let hunk = match app.current_hunk() {
                Some(h) => h,
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

            for (line_idx, line) in hunk.search.iter().enumerate() {
                let file_idx = search_file_map.get(line_idx).copied().flatten();
                let is_matched = file_idx.is_some();

                let (bg, prefix_color, prefix) = if is_matched {
                    (pal::BG_MATCH, pal::TEXT_INSERT, "= ")
                } else {
                    (pal::BG_DELETE, pal::TEXT_DELETE, "- ")
                };

                let desired = Vec2::new(ui.available_width(), row_h);
                let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
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
    let mut clear_anchor = false;
    let mut apply_clicked = false;
    let mut find_text = false;
    let mut prev_anchor_match = false;
    let mut next_anchor_match = false;

    let current_hunk_idx = app.current_hunk;
    let total_hunks = app.hunks.len();
    let manual_anchor = app.manual_anchor;
    let candidate_count = mr.candidates.len();
    let candidate_idx = app.candidate_index;
    let is_applied = app.applied_hunks.contains(&app.current_hunk);
    let can_apply = !is_applied && (app.match_result.is_some() || app.manual_anchor.is_some());
    let apply_line = if let Some(anchor) = manual_anchor {
        anchor + 1
    } else {
        mr.file_start + 1
    };

    Frame::none()
        .fill(Color32::from_rgb(25, 32, 42))
        .inner_margin(Margin::symmetric(6.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                ui.label(RichText::new("Hunk:").color(pal::TEXT_DIM).small());
                if ui
                    .add_enabled(current_hunk_idx > 0, Button::new("◀").small())
                    .on_hover_text("Previous hunk (Shift+L)")
                    .clicked()
                {
                    prev_hunk = true;
                }
                ui.label(
                    RichText::new(format!("{}/{}", current_hunk_idx + 1, total_hunks)).monospace(),
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

                if let Some(anchor) = manual_anchor {
                    ui.label(
                        RichText::new(format!("⚓ {}", anchor + 1))
                            .color(pal::TEXT_ANCHOR)
                            .monospace(),
                    );
                    if ui
                        .small_button("✕")
                        .on_hover_text("Clear anchor (Esc)")
                        .clicked()
                    {
                        clear_anchor = true;
                    }
                } else {
                    ui.label(RichText::new("Cand:").color(pal::TEXT_DIM).small());
                    if ui
                        .add_enabled(candidate_idx > 0, Button::new("◀").small())
                        .clicked()
                    {
                        prev_candidate = true;
                    }
                    ui.label(
                        RichText::new(format!("{}/{}", candidate_idx + 1, candidate_count.max(1)))
                            .monospace(),
                    );
                    if ui
                        .add_enabled(
                            candidate_idx + 1 < candidate_count,
                            Button::new("▶").small(),
                        )
                        .clicked()
                    {
                        next_candidate = true;
                    }
                }

                ui.add(Separator::default().vertical());

                ui.label(RichText::new("🔍").monospace());
                let resp = ui.add(
                    TextEdit::singleline(&mut app.file_search_query)
                        .hint_text("find…")
                        .desired_width(72.0)
                        .font(TextStyle::Monospace),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    find_text = true;
                }
                if ui.small_button("Go").clicked() {
                    find_text = true;
                }

                if !app.anchor_matches.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "{}/{}",
                            app.anchor_match_idx + 1,
                            app.anchor_matches.len()
                        ))
                        .color(pal::TEXT_ANCHOR)
                        .small()
                        .monospace(),
                    );
                    if ui.small_button("◀").clicked() {
                        prev_anchor_match = true;
                    }
                    if ui.small_button("▶").clicked() {
                        next_anchor_match = true;
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

                if current_hunk_idx < total_hunks - 1 {
                    if ui
                        .small_button("Skip →")
                        .on_hover_text("Skip to next hunk without applying")
                        .clicked()
                    {
                        next_hunk = true;
                    }
                }
            });
        });

    ui.add(Separator::default());

    let len = app.file_lines.len();
    let mut go_next_hunk = false;
    let mut go_prev_hunk = false;

    if len > 0 && !ui.ctx().wants_keyboard_input() {
        let mut cursor_changed = false;
        let mut new_text = String::new();

        ui.input(|i| {
            let cur = app.cursor_line.unwrap_or(0);

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

            if i.key_pressed(Key::L) {
                if i.modifiers.shift {
                    go_prev_hunk = true;
                } else {
                    go_next_hunk = true;
                }
            }

            if i.key_pressed(Key::A) {
                let in_hunk = if let Some(anchor) = manual_anchor {
                    cur == anchor
                } else {
                    cur >= mr.file_start && cur < mr.file_end
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
                    if txt != "?" && txt != "m" && txt != "M" {
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

            if lower_buf == "u" {
                app.undo();
                clear_buffer = true;
            } else if lower_buf == "." {
                if let Some(action) = app.last_action.clone() {
                    match action {
                        Action::DeleteLines(count) => app.delete_lines(count),
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
                let allowed = buf
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == 'd' || c == 'D' || c == 'g' || c == 'G');
                let d_count = buf.matches('d').count() + buf.matches('D').count();
                if !allowed || d_count > 2 {
                    clear_buffer = true;
                }
            }

            if clear_buffer {
                app.vim_buffer.clear();
            }
        }

        if cursor_changed {
            app.scroll_to_match = true;
        }
    }

    if prev_hunk && current_hunk_idx > 0 {
        app.current_hunk -= 1;
        app.load_hunk();
        return;
    }
    if next_hunk && current_hunk_idx < total_hunks - 1 {
        app.current_hunk += 1;
        app.load_hunk();
        return;
    }

    if clear_anchor {
        app.manual_anchor = None;
        app.anchor_matches.clear();
        app.scroll_to_match = true;
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

    if find_text {
        let q = app.file_search_query.trim().to_lowercase();
        if !q.is_empty() {
            app.anchor_matches = app
                .file_lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
            if !app.anchor_matches.is_empty() {
                app.anchor_match_idx = 0;
                app.manual_anchor = Some(app.anchor_matches[0]);
                app.scroll_to_match = true;
            } else {
                app.manual_anchor = None;
                app.set_message(super::types::StatusMessage::warning(format!(
                    "No matches for '{}'",
                    q
                )));
            }
        }
    }

    if prev_anchor_match && !app.anchor_matches.is_empty() {
        if app.anchor_match_idx > 0 {
            app.anchor_match_idx -= 1;
        } else {
            app.anchor_match_idx = app.anchor_matches.len() - 1;
        }
        app.manual_anchor = Some(app.anchor_matches[app.anchor_match_idx]);
        app.scroll_to_match = true;
    }
    if next_anchor_match && !app.anchor_matches.is_empty() {
        app.anchor_match_idx = (app.anchor_match_idx + 1) % app.anchor_matches.len();
        app.manual_anchor = Some(app.anchor_matches[app.anchor_match_idx]);
        app.scroll_to_match = true;
    }

    if go_next_hunk {
        if app.current_hunk < app.hunks.len() - 1 {
            app.current_hunk += 1;
            app.load_hunk();
            return;
        } else {
            app.cursor_line = Some(mr.file_start);
            app.scroll_to_match = true;
        }
    }
    if go_prev_hunk {
        if app.current_hunk > 0 {
            app.current_hunk -= 1;
            app.load_hunk();
            return;
        } else {
            app.cursor_line = Some(mr.file_start);
            app.scroll_to_match = true;
        }
    }

    let file_lines = app.file_lines.clone();
    let manual_anchor_check = app.manual_anchor;
    let merged_range = app.merged_range;
    let auto_start = mr.file_start;
    let auto_end = mr.file_end;
    let auto_score = mr.score;
    let search_query = app.file_search_query.clone();
    let scroll_to_match = app.scroll_to_match;
    let cursor_line = app.cursor_line;
    let mut did_scroll = false;
    let mut set_anchor: Option<usize> = None;
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
                let is_anchor = manual_anchor_check == Some(i);
                let is_cursor = cursor_line == Some(i);
                let in_merged = merged_range.map_or(false, |(rs, re)| i >= rs && i < re);
                let is_delete = in_auto_match && delete_file_indices.contains(&i);
                let is_equal = in_auto_match && equal_file_indices.contains(&i);
                let is_search_hit = !search_query.is_empty()
                    && line.to_lowercase().contains(&search_query.to_lowercase());

                let row_is_tall = is_anchor
                    || (in_auto_match && i == auto_start && manual_anchor_check.is_none());
                let desired = Vec2::new(
                    ui.available_width(),
                    if row_is_tall { row_h + 6.0 } else { row_h },
                );
                let (rect, row_resp) = ui.allocate_exact_size(desired, Sense::click());

                let should_scroll = scroll_to_match
                    && (is_cursor
                        || (cursor_line.is_none() && is_anchor)
                        || (cursor_line.is_none()
                            && manual_anchor_check.is_none()
                            && i == auto_start));
                if should_scroll {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    did_scroll = true;
                }

                if is_anchor {
                    ui.painter().rect_filled(rect, 2.0, pal::BG_ANCHOR);
                    let dash_y = rect.center().y;
                    let mut x = rect.left() + 4.0;
                    while x < rect.right() - 120.0 {
                        ui.painter().line_segment(
                            [
                                Pos2::new(x, dash_y),
                                Pos2::new((x + 8.0).min(rect.right() - 120.0), dash_y),
                            ],
                            Stroke::new(1.5, pal::BAR_ANCHOR),
                        );
                        x += 14.0;
                    }
                    ui.painter().text(
                        Pos2::new(rect.left() + 10.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("⚓ insert before line {}", i + 1),
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
                                RichText::new("⚡ Apply here")
                                    .color(Color32::WHITE)
                                    .strong()
                                    .monospace(),
                            )
                            .fill(Color32::from_rgb(90, 70, 15))
                            .stroke(Stroke::new(1.0, pal::BAR_ANCHOR)),
                        )
                        .clicked()
                    {
                        apply_clicked = true;
                    }
                } else if in_auto_match && i == auto_start && manual_anchor_check.is_none() {
                    ui.painter()
                        .rect_filled(rect, 2.0, Color32::from_rgb(28, 60, 40));
                    ui.painter().text(
                        Pos2::new(rect.left() + 10.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!(
                            "▼ auto match  {}–{}  ({:.0}%)",
                            auto_start + 1,
                            auto_end,
                            auto_score
                        ),
                        FontId::monospace(10.5),
                        Color32::from_rgb(120, 230, 160),
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
                                RichText::new("⚡ Apply here")
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
                } else {
                    let base_bg = if in_merged {
                        pal::BG_MERGED
                    } else if is_delete {
                        pal::BG_DELETE
                    } else if is_cursor {
                        pal::BG_CURSOR
                    } else if in_auto_match && manual_anchor_check.is_none() {
                        pal::BG_MATCH
                    } else if i % 2 == 0 {
                        pal::BG_ROW_EVEN
                    } else {
                        pal::BG_ROW_ODD
                    };
                    let row_bg = if is_search_hit {
                        pal::BG_SEARCH_HIT
                    } else {
                        base_bg
                    };
                    ui.painter().rect_filled(rect, 0.0, row_bg);

                    let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                    let bar_color = if in_merged {
                        pal::BAR_MERGED
                    } else if is_delete {
                        pal::TEXT_DELETE
                    } else if is_cursor {
                        pal::BAR_CURSOR
                    } else if is_anchor {
                        pal::BAR_ANCHOR
                    } else if in_auto_match && manual_anchor_check.is_none() {
                        pal::BAR_MATCH
                    } else if is_search_hit {
                        pal::BAR_SEARCH
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(bar, 0.0, bar_color);

                    let diff_prefix = if in_auto_match && manual_anchor_check.is_none() {
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

                    if row_resp.clicked() {
                        set_cursor = Some(i);
                        if !search_query.is_empty() {
                            set_anchor = Some(i);
                        }
                    }

                    let num_color = if in_merged {
                        pal::TEXT_LNUM_ACTIVE
                    } else if is_delete {
                        pal::TEXT_DELETE
                    } else if in_auto_match && manual_anchor_check.is_none() {
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
                    } else if in_auto_match && manual_anchor_check.is_none() {
                        pal::TEXT_MATCH
                    } else if is_search_hit {
                        pal::TEXT_SEARCH
                    } else {
                        pal::TEXT_NORMAL
                    };
                    let display = MergeApp::truncate_owned(line, max_chars);
                    ui.painter().text(
                        Pos2::new(rect.left() + 58.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        FontId::monospace(11.0),
                        text_color,
                    );
                }

                if in_auto_match && i == auto_end.saturating_sub(1) && manual_anchor_check.is_none()
                {
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

    if let Some(anchor_line) = set_anchor {
        app.manual_anchor = Some(anchor_line);
        app.set_message(super::types::StatusMessage::info(format!(
            "⚓ Anchor at line {} — click ⚡ Apply here or press A",
            anchor_line + 1
        )));
    }
    if let Some(cur_line) = set_cursor {
        app.cursor_line = Some(cur_line);
    }

    if apply_clicked {
        app.apply_merge();
    }
}
