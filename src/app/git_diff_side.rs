use super::matching::MergeMatching;
use super::palette::pal;
use super::state::MergeApp;
use super::types::StatusMessage;
use crate::diff::RowKind;
use crate::git_diff_vim::{parse_vim_buffer, VimCmd};
use eframe::egui::*;

pub fn render_git_diff_side_panel(
    app: &mut MergeApp,
    ui: &mut Ui,
    row_h: f32,
    char_w: f32,
    panel_w: f32,
) {
    let row_font = FontId::monospace(11.0);
    let half_w = (panel_w - 10.0) / 2.0;
    let lnum_w = 5.0 * char_w;
    let text_x_off = 4.0 + lnum_w + 6.0;
    let max_chars = ((half_w - text_x_off - 6.0) / char_w).floor().max(4.0) as usize;

    // Group contiguous non-Equal rows into navigable "hunks".
    let rows = app.git_diff_rows.clone();
    let mut hunk_row_starts: Vec<usize> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let is_change = !matches!(row.kind, RowKind::Equal);
        let prev_is_change = i > 0 && !matches!(rows[i - 1].kind, RowKind::Equal);
        if is_change && !prev_is_change {
            hunk_row_starts.push(i);
        }
    }
    let hunk_count = hunk_row_starts.len();
    if app.diff_side_hunk_idx >= hunk_count && hunk_count > 0 {
        app.diff_side_hunk_idx = hunk_count - 1;
    }

    // Keyboard navigation: l = next hunk, L (Shift+l) = previous hunk
    if !ui.ctx().wants_keyboard_input() && hunk_count > 0 {
        ui.input(|i| {
            if i.key_pressed(Key::L) && app.git_diff_vim_buffer.is_empty() {
                if i.modifiers.shift {
                    if app.diff_side_hunk_idx > 0 {
                        app.diff_side_hunk_idx -= 1;
                    } else {
                        app.diff_side_hunk_idx = hunk_count.saturating_sub(1);
                    }
                } else if app.diff_side_hunk_idx + 1 < hunk_count {
                    app.diff_side_hunk_idx += 1;
                } else {
                    app.diff_side_hunk_idx = 0;
                }
                app.diff_side_scroll_target = hunk_row_starts.get(app.diff_side_hunk_idx).copied();
            }
        });
    }
    // Cursor movement + vim-style dd/yy/p/P/gg/G editing, scoped to this panel.
    if app.git_diff_insert_mode {
        ui.ctx().set_cursor_icon(CursorIcon::Text);
        handle_git_diff_insert_mode(app, ui);
    } else if !ui.ctx().wants_keyboard_input() && !rows.is_empty() {
        let cur = app.git_diff_cursor.unwrap_or(0).min(rows.len() - 1);
        let mut new_text = String::new();
        let mut moved = false;
        let mut revert_to_head = false;
        let mut enter_insert = false;
        let mut enter_insert_at_start = false;
        let mut x_pressed = false;
        let mut f7_pressed = false;
        ui.input(|i| {
            if i.key_pressed(Key::ArrowDown) {
                app.git_diff_cursor = Some((cur + 1).min(rows.len() - 1));
                moved = true;
            }
            if i.key_pressed(Key::ArrowUp) {
                app.git_diff_cursor = Some(cur.saturating_sub(1));
                moved = true;
            }
            if app.git_diff_cursor.is_none() {
                app.git_diff_cursor = Some(cur);
            }
            if i.key_pressed(Key::F7) {
                f7_pressed = true;
            }
            if i.key_pressed(Key::ArrowLeft) {
                if app.git_diff_cursor_col > 0 {
                    app.git_diff_cursor_col -= 1;
                }
            }
            if i.key_pressed(Key::ArrowRight) {
                let max_len = app.git_diff_rows.get(cur).and_then(|r| r.right.as_ref()).map(|s| s.chars().count()).unwrap_or(0);
                if app.git_diff_cursor_col < max_len {
                    app.git_diff_cursor_col += 1;
                }
            }
            for event in i.events.clone() {
                if let Event::Text(txt) = event {
                    match txt.as_str() {
                        "r" if app.git_diff_vim_buffer.is_empty() => revert_to_head = true,
                        "i" if app.git_diff_vim_buffer.is_empty() => enter_insert = true,
                        "I" if app.git_diff_vim_buffer.is_empty() => enter_insert_at_start = true,
                        "x" if app.git_diff_vim_buffer.is_empty() => x_pressed = true,
                        _ => new_text.push_str(&txt),
                    }
                }
            }
        });
        if moved {
            app.git_diff_scroll_to_cursor = true;
        }
        if x_pressed {
            if let Some(fl) = app.git_diff_rows.get(cur).and_then(|r| r.right_num).map(|n| n - 1) {
                if fl < app.file_lines.len() {
                    let line = app.file_lines[fl].clone();
                    let mut chars: Vec<char> = line.chars().collect();
                    if app.git_diff_cursor_col < chars.len() {
                        app.save_history();
                        chars.remove(app.git_diff_cursor_col);
                        app.file_lines[fl] = chars.iter().collect();
                        let new_len = app.file_lines[fl].chars().count();
                        if app.git_diff_cursor_col >= new_len && app.git_diff_cursor_col > 0 {
                            app.git_diff_cursor_col -= 1;
                        }
                        app.recompute_match();
                        app.update_git_statuses();
                        app.refresh_git_diff_side_rows();
                    }
                }
            }
        }
        if f7_pressed {
            if let Some(fl) = app.git_diff_rows.get(cur).and_then(|r| r.right_num).map(|n| n - 1) {
                if fl < app.file_lines.len() {
                    let line = app.file_lines[fl].clone();
                    let chars: Vec<char> = line.chars().collect();
                    let open_brackets = ['(', '{', '['];
                    let close_brackets = [')', '}', ']'];
                    let mut target_c = None;
                    let mut start_col = app.git_diff_cursor_col;
                    if start_col < chars.len() {
                        if open_brackets.contains(&chars[start_col]) || close_brackets.contains(&chars[start_col]) {
                            target_c = Some(chars[start_col]);
                        }
                    }
                    if target_c.is_none() {
                        for i in app.git_diff_cursor_col..chars.len() {
                            if open_brackets.contains(&chars[i]) || close_brackets.contains(&chars[i]) {
                                target_c = Some(chars[i]);
                                start_col = i;
                                break;
                            }
                        }
                    }
                    if let Some(tc) = target_c {
                        let mut matched_line = None;
                        let mut matched_col = 0;
                        if open_brackets.contains(&tc) {
                            let match_c = close_brackets[open_brackets.iter().position(|&c| c == tc).unwrap()];
                            let mut depth = 0;
                            'outer: for l in fl..app.file_lines.len() {
                                let l_chars: Vec<char> = app.file_lines[l].chars().collect();
                                let c_start = if l == fl { start_col } else { 0 };
                                for c_idx in c_start..l_chars.len() {
                                    let ch = l_chars[c_idx];
                                    if ch == tc {
                                        depth += 1;
                                    } else if ch == match_c {
                                        depth -= 1;
                                        if depth == 0 {
                                            matched_line = Some(l);
                                            matched_col = c_idx;
                                            break 'outer;
                                        }
                                    }
                                }
                            }
                        } else {
                            let match_c = open_brackets[close_brackets.iter().position(|&c| c == tc).unwrap()];
                            let mut depth = 0;
                            'outer: for l in (0..=fl).rev() {
                                let l_chars: Vec<char> = app.file_lines[l].chars().collect();
                                let c_end = if l == fl { start_col + 1 } else { l_chars.len() };
                                for c_idx in (0..c_end).rev() {
                                    let ch = l_chars[c_idx];
                                    if ch == tc {
                                        depth += 1;
                                    } else if ch == match_c {
                                        depth -= 1;
                                        if depth == 0 {
                                            matched_line = Some(l);
                                            matched_col = c_idx;
                                            break 'outer;
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(ml) = matched_line {
                            if let Some(row_idx) = app.git_diff_rows.iter().position(|r| r.right_num == Some(ml + 1)) {
                                app.git_diff_cursor = Some(row_idx);
                                app.git_diff_cursor_col = matched_col;
                                app.git_diff_scroll_to_cursor = true;
                            }
                        }
                    }
                }
            }
        }
        if revert_to_head {
            apply_git_diff_vim_cmd(app, VimCmd::RevertToHead, cur, &hunk_row_starts);
        }
        if enter_insert || enter_insert_at_start {
            if let Some(fl) = app
                .git_diff_rows
                .get(cur)
                .and_then(|r| r.right_num)
                .map(|n| n - 1)
            {
                app.cursor_line = Some(fl);
                app.git_diff_insert_mode = true;
                app.insert_cursor = if enter_insert_at_start {
                    0
                } else {
                    app.git_diff_cursor_col
                };
            }
        }
        if !new_text.is_empty() {
            app.git_diff_vim_buffer.push_str(&new_text);
            let (cmd, clear) = parse_vim_buffer(&app.git_diff_vim_buffer);
            if let Some(cmd) = cmd {
                apply_git_diff_vim_cmd(app, cmd, cur, &hunk_row_starts);
            }
            if clear {
                app.git_diff_vim_buffer.clear();
            }
        }
    }
    let file_count = app.git_changed_files.len();
    let file_idx = app.git_changed_file_idx;
    let current_rel_file = app.git_changed_files.get(file_idx).cloned();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Full-file diff vs HEAD (side by side)")
                .color(pal::TEXT_DIM)
                .size(12.0),
        );
        if ui.button(RichText::new("🔄 Refresh").size(12.0)).clicked() {
            app.refresh_git_changed_files();
            app.update_git_statuses();
        }
        ui.add(Separator::default().vertical());
        ui.label(RichText::new("File:").color(pal::TEXT_DIM).size(12.0));
        ui.add_enabled_ui(file_count > 0, |ui| {
            if ui
                .button(RichText::new("◀ File").size(12.0).monospace())
                .on_hover_text("Previous changed file")
                .clicked()
            {
                let new_idx = if file_idx == 0 {
                    file_count - 1
                } else {
                    file_idx - 1
                };
                app.load_git_changed_file(new_idx);
            }
            if ui
                .button(RichText::new("File ▶").size(12.0).monospace())
                .on_hover_text("Next changed file")
                .clicked()
            {
                let new_idx = if file_idx + 1 < file_count {
                    file_idx + 1
                } else {
                    0
                };
                app.load_git_changed_file(new_idx);
            }
        });
        if file_count > 0 {
            ui.label(
                RichText::new(format!("{}/{}", file_idx + 1, file_count))
                    .color(pal::TEXT_DIM)
                    .monospace()
                    .size(12.0),
            );
            if let Some(rel) = &current_rel_file {
                ui.label(
                    RichText::new(format!("({})", rel))
                        .color(pal::TEXT_NORMAL)
                        .monospace()
                        .size(12.0),
                );
            }
        } else {
            ui.label(
                RichText::new("no changed files")
                    .color(pal::TEXT_DIM)
                    .size(12.0),
            );
        }
        ui.add(Separator::default().vertical());
        ui.add_enabled_ui(hunk_count > 0, |ui| {
            if ui
                .button(RichText::new("▲ Prev Hunk").size(12.0).monospace())
                .on_hover_text("Jump to previous changed block (L)")
                .clicked()
            {
                if app.diff_side_hunk_idx > 0 {
                    app.diff_side_hunk_idx -= 1;
                } else {
                    app.diff_side_hunk_idx = hunk_count.saturating_sub(1);
                }
                app.diff_side_scroll_target = hunk_row_starts.get(app.diff_side_hunk_idx).copied();
            }
            if ui
                .button(RichText::new("▼ Next Hunk").size(12.0).monospace())
                .on_hover_text("Jump to next changed block (l)")
                .clicked()
            {
                if app.diff_side_hunk_idx + 1 < hunk_count {
                    app.diff_side_hunk_idx += 1;
                } else {
                    app.diff_side_hunk_idx = 0;
                }
                app.diff_side_scroll_target = hunk_row_starts.get(app.diff_side_hunk_idx).copied();
            }
        });
        if hunk_count > 0 {
            ui.label(
                RichText::new(format!(
                    "Hunk {}/{}",
                    app.diff_side_hunk_idx + 1,
                    hunk_count
                ))
                .color(pal::TEXT_DIM)
                .monospace()
                .size(12.0),
            );
        }
    });
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("OLD (HEAD)")
                .color(pal::TEXT_DIM)
                .size(12.0)
                .strong(),
        );
        ui.add_space((half_w - 90.0).max(0.0));
        ui.label(
            RichText::new("NEW (working)")
                .color(pal::TEXT_DIM)
                .size(12.0)
                .strong(),
        );
    });
    ui.add(Separator::default());
    if rows.is_empty() {
        ui.add_space(6.0);
        ui.label(
            RichText::new("No changes vs HEAD, or file is not tracked by git.")
                .color(pal::TEXT_DIM),
        );
        return;
    }
    // Action toolbar: reflects last frame's selection state (same one-frame
    // lag pattern used by the other drag-selection HUDs in this app).
    if app.diff_side_left_selection.is_some() || app.diff_side_right_selection.is_some() {
        let mut insert_clicked = false;
        let mut delete_clicked = false;
        let mut clear_left_sel = false;
        let mut clear_right_sel = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if let Some((lo, hi)) = app.diff_side_left_selection {
                let count = hi - lo + 1;
                ui.label(
                    RichText::new(format!("⬅ HEAD selection: {} line(s)", count))
                        .color(Color32::from_rgb(190, 160, 255))
                        .small()
                        .monospace(),
                );
                if app.diff_side_insert_anchor.is_some() {
                    let btn = Button::new(
                        RichText::new("⚡ Insert into working →")
                            .color(Color32::WHITE)
                            .strong()
                            .small()
                            .monospace(),
                    )
                    .fill(Color32::from_rgb(70, 45, 100));
                    if ui.add(btn).clicked() {
                        insert_clicked = true;
                    }
                } else {
                    ui.label(
                        RichText::new("→ click ⚓ on a NEW line to set insert point")
                            .color(pal::TEXT_DIM)
                            .small(),
                    );
                }
                if ui.small_button("✕").clicked() {
                    clear_left_sel = true;
                }
            }
            if let Some((lo, hi)) = app.diff_side_right_selection {
                let count = hi - lo + 1;
                ui.add(Separator::default().vertical());
                ui.label(
                    RichText::new(format!("➡ Working selection: {} line(s)", count))
                        .color(pal::TEXT_DELETE)
                        .small()
                        .monospace(),
                );
                let btn = Button::new(
                    RichText::new("🗑 Delete")
                        .color(Color32::WHITE)
                        .strong()
                        .small()
                        .monospace(),
                )
                .fill(Color32::from_rgb(120, 40, 40));
                if ui.add(btn).clicked() {
                    delete_clicked = true;
                }
                if ui.small_button("✕").clicked() {
                    clear_right_sel = true;
                }
            }
        });
        ui.add_space(2.0);
        if insert_clicked {
            app.insert_diff_side_selection();
        }
        if delete_clicked {
            app.delete_diff_side_selection();
        }
        if clear_left_sel {
            app.diff_side_left_selection = None;
            app.diff_side_left_drag_anchor = None;
        }
        if clear_right_sel {
            app.diff_side_right_selection = None;
            app.diff_side_right_drag_anchor = None;
        }
    }
    let scroll_target = app.diff_side_scroll_target;
    let mut scrolled = false;
    let pointer_pos = ui.input(|i| i.pointer.interact_pos());
    let primary_down = ui.input(|i| i.pointer.primary_down());
    let mut local_left_drag_anchor = app.diff_side_left_drag_anchor;
    let mut local_left_selection = app.diff_side_left_selection;
    let mut local_right_drag_anchor = app.diff_side_right_drag_anchor;
    let mut local_right_selection = app.diff_side_right_selection;
    let mut set_insert_anchor: Option<usize> = None;
    ScrollArea::vertical()
        .id_source("git_diff_side_scroll")
        .auto_shrink([false, false])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            for (row_idx, row) in rows.iter().enumerate() {
                let is_cursor = app.git_diff_cursor == Some(row_idx);
                let is_left_sel =
                    local_left_selection.map_or(false, |(s, e)| row_idx >= s && row_idx <= e);
                let is_right_sel =
                    local_right_selection.map_or(false, |(s, e)| row_idx >= s && row_idx <= e);
                let is_insert_anchor = app.diff_side_insert_anchor == Some(row_idx);
                let (lbg, rbg) = if is_cursor {
                    (pal::BG_CURSOR, pal::BG_CURSOR)
                } else {
                    let base = match row.kind {
                        RowKind::Equal => (pal::BG_ROW_EVEN, pal::BG_ROW_EVEN),
                        RowKind::Delete => (pal::BG_DELETE, Color32::TRANSPARENT),
                        RowKind::Insert => (Color32::TRANSPARENT, pal::BG_INSERT),
                    };
                    let l = if is_left_sel {
                        Color32::from_rgb(55, 40, 85)
                    } else {
                        base.0
                    };
                    let r = if is_right_sel {
                        Color32::from_rgb(70, 30, 30)
                    } else {
                        base.1
                    };
                    (l, r)
                };
                let desired = Vec2::new(half_w * 2.0 + 8.0, row_h);
                let (rect, resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());
                if resp.clicked() {
                    app.git_diff_cursor = Some(row_idx);
                }
                if resp.secondary_clicked() {
                    if let Some(pos) = pointer_pos {
                        let mut rr = rect;
                        rr.min.x = rect.min.x + half_w + 8.0;
                        rr.set_width(half_w);
                        if rr.contains(pos) {
                            set_insert_anchor = Some(row_idx);
                        }
                    }
                }
                if scroll_target == Some(row_idx) {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    scrolled = true;
                }
                if app.git_diff_scroll_to_cursor && app.git_diff_cursor == Some(row_idx) {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    scrolled = true;
                }
                let mut left_rect = rect;
                left_rect.set_width(half_w);
                let mut right_rect = rect;
                right_rect.min.x = rect.min.x + half_w + 8.0;
                right_rect.set_width(half_w);
                if primary_down {
                    if let Some(pos) = pointer_pos {
                        if left_rect.contains(pos) {
                            let anchor = *local_left_drag_anchor.get_or_insert(row_idx);
                            local_left_selection = Some((anchor.min(row_idx), anchor.max(row_idx)));
                        } else if right_rect.contains(pos) {
                            let anchor = *local_right_drag_anchor.get_or_insert(row_idx);
                            local_right_selection =
                                Some((anchor.min(row_idx), anchor.max(row_idx)));
                        }
                    }
                }
                ui.painter().rect_filled(left_rect, 0.0, lbg);
                ui.painter().rect_filled(right_rect, 0.0, rbg);
                if is_insert_anchor {
                    ui.painter().rect_stroke(
                        right_rect,
                        0.0,
                        Stroke::new(2.0, Color32::from_rgb(230, 190, 90)),
                    );
                }
                if row.right_num.is_some() {
                    let anchor_btn_w = 20.0;
                    let anchor_btn_rect = Rect::from_min_size(
                        Pos2::new(right_rect.right() - anchor_btn_w - 2.0, rect.top() + 1.0),
                        Vec2::new(anchor_btn_w, rect.height() - 2.0),
                    );
                    let anchor_btn = Button::new(RichText::new("⚓").small().monospace().color(
                        if is_insert_anchor {
                            Color32::from_rgb(255, 220, 120)
                        } else {
                            pal::TEXT_DIM
                        },
                    ))
                    .fill(if is_insert_anchor {
                        Color32::from_rgb(70, 55, 15)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .frame(false);
                    if ui
                        .put(anchor_btn_rect, anchor_btn)
                        .on_hover_text("Set insert anchor at this working line")
                        .clicked()
                    {
                        app.diff_side_insert_anchor = Some(row_idx);
                    }
                }
                if let Some(l) = &row.left {
                    let num_text = row
                        .left_num
                        .map(|n| format!("{:>4}", n))
                        .unwrap_or_default();
                    ui.painter().text(
                        Pos2::new(left_rect.left() + 4.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &num_text,
                        row_font.clone(),
                        pal::TEXT_DIM,
                    );
                    let display = MergeApp::truncate_owned(l, max_chars);
                    ui.painter().text(
                        Pos2::new(left_rect.left() + text_x_off, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        row_font.clone(),
                        if matches!(row.kind, RowKind::Delete) {
                            pal::TEXT_DELETE
                        } else {
                            pal::TEXT_NORMAL
                        },
                    );
                }
                if let Some(r) = &row.right {
                    let num_text = row
                        .right_num
                        .map(|n| format!("{:>4}", n))
                        .unwrap_or_default();
                    ui.painter().text(
                        Pos2::new(right_rect.left() + 4.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &num_text,
                        row_font.clone(),
                        pal::TEXT_DIM,
                    );
                    let display = MergeApp::truncate_owned(r, max_chars);
                    ui.painter().text(
                        Pos2::new(right_rect.left() + text_x_off, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        row_font.clone(),
                        if matches!(row.kind, RowKind::Insert) {
                            pal::TEXT_INSERT
                        } else {
                            pal::TEXT_NORMAL
                        },
                    );

                    if is_cursor {
                        let col = if app.git_diff_insert_mode {
                            app.insert_cursor
                        } else {
                            app.git_diff_cursor_col
                        };
                        let cursor_x = right_rect.left() + text_x_off + (col as f32 * char_w);
                        let cursor_rect = Rect::from_min_size(
                            Pos2::new(cursor_x, rect.top() + 2.0),
                            Vec2::new(char_w.min(2.0), rect.height() - 4.0),
                        );
                        ui.painter().rect_filled(cursor_rect, 0.0, pal::BAR_CURSOR);
                    }
                }
                let sep = Rect::from_min_size(
                    Pos2::new(rect.min.x + half_w + 3.0, rect.top()),
                    Vec2::new(2.0, rect.height()),
                );
                ui.painter().rect_filled(sep, 0.0, pal::SEPARATOR);
            }
        });
    if !primary_down {
        local_left_drag_anchor = None;
        local_right_drag_anchor = None;
    }
    app.diff_side_left_drag_anchor = local_left_drag_anchor;
    app.diff_side_left_selection = local_left_selection;
    app.diff_side_right_drag_anchor = local_right_drag_anchor;
    app.diff_side_right_selection = local_right_selection;
    if let Some(row_idx) = set_insert_anchor {
        app.diff_side_insert_anchor = Some(row_idx);
    }
    if scrolled {
        app.diff_side_scroll_target = None;
        app.git_diff_scroll_to_cursor = false;
    }
}

fn apply_git_diff_vim_cmd(
    app: &mut MergeApp,
    cmd: VimCmd,
    cursor_row: usize,
    hunk_row_starts: &[usize],
) {
    // Map a diff-row index to the corresponding line in the working buffer
    // (app.file_lines) via the row's right-side (new/working) line number.
    let row_to_file_line = |app: &MergeApp, row_idx: usize| -> Option<usize> {
        app.git_diff_rows
            .get(row_idx)
            .and_then(|r| r.right_num)
            .map(|n| n - 1)
    };
    match cmd {
        VimCmd::DeleteLines(n) => {
            if let Some(fl) = row_to_file_line(app, cursor_row) {
                app.save_history();
                let end = (fl + n).min(app.file_lines.len());
                if fl < end {
                    app.file_lines.drain(fl..end);
                    app.recompute_match();
                    app.update_git_statuses();
                    app.refresh_git_diff_side_rows();
                    app.git_diff_cursor =
                        Some(cursor_row.min(app.git_diff_rows.len().saturating_sub(1)));
                }
            }
        }
        VimCmd::Yank => {
            if let Some(fl) = row_to_file_line(app, cursor_row) {
                app.yanked_line = app.file_lines.get(fl).cloned();
                app.set_message(StatusMessage::info(format!("Yanked line {}", fl + 1)));
            }
        }
        VimCmd::PasteBelow => {
            if let (Some(fl), Some(text)) =
                (row_to_file_line(app, cursor_row), app.yanked_line.clone())
            {
                app.save_history();
                if fl + 1 <= app.file_lines.len() {
                    app.file_lines.insert(fl + 1, text);
                    app.recompute_match();
                    app.update_git_statuses();
                    app.refresh_git_diff_side_rows();
                    app.set_message(StatusMessage::info("Pasted below"));
                }
            }
        }
        VimCmd::PasteAbove => {
            if let (Some(fl), Some(text)) =
                (row_to_file_line(app, cursor_row), app.yanked_line.clone())
            {
                app.save_history();
                app.file_lines.insert(fl, text);
                app.recompute_match();
                app.update_git_statuses();
                app.refresh_git_diff_side_rows();
                app.set_message(StatusMessage::info("Pasted above"));
            }
        }
        VimCmd::GotoTop => {
            app.git_diff_cursor = Some(0);
            app.git_diff_scroll_to_cursor = true;
        }
        VimCmd::GotoBottom => {
            app.git_diff_cursor = Some(app.git_diff_rows.len().saturating_sub(1));
            app.git_diff_scroll_to_cursor = true;
        }
        VimCmd::Undo => {
            app.undo();
            app.refresh_git_diff_side_rows();
        }
        VimCmd::NextGitHunk => {
            if let Some(&next) = hunk_row_starts.iter().find(|&&s| s > cursor_row) {
                app.git_diff_cursor = Some(next);
            } else if let Some(&first) = hunk_row_starts.first() {
                app.git_diff_cursor = Some(first);
            }
            app.git_diff_scroll_to_cursor = true;
        }
        VimCmd::PrevGitHunk => {
            if let Some(&prev) = hunk_row_starts.iter().rev().find(|&&s| s < cursor_row) {
                app.git_diff_cursor = Some(prev);
            } else if let Some(&last) = hunk_row_starts.last() {
                app.git_diff_cursor = Some(last);
            }
            app.git_diff_scroll_to_cursor = true;
        }
        VimCmd::RevertToHead => {
            // Pair up a Delete row (HEAD content) with the Insert/Equal row that
            // replaced it, and restore the HEAD text into the working buffer.
            if let Some(row) = app.git_diff_rows.get(cursor_row).cloned() {
                let head_text = match row.kind {
                    RowKind::Delete => row.left.clone(),
                    RowKind::Insert => {
                        // find nearest preceding Delete row to pull HEAD text from
                        app.git_diff_rows[..cursor_row]
                            .iter()
                            .rev()
                            .find(|r| matches!(r.kind, RowKind::Delete))
                            .and_then(|r| r.left.clone())
                    }
                    RowKind::Equal => row.left.clone(),
                };
                if let (Some(text), Some(fl)) = (
                    head_text,
                    row.right_num.map(|n| n - 1).or_else(|| {
                        // Delete-only row has no right_num; revert means "insert HEAD
                        // line back" at the position right before the next right_num.
                        app.git_diff_rows[cursor_row..]
                            .iter()
                            .find_map(|r| r.right_num)
                            .map(|n| n - 1)
                    }),
                ) {
                    app.save_history();
                    match row.kind {
                        RowKind::Delete => {
                            app.file_lines.insert(fl, text);
                        }
                        _ => {
                            if fl < app.file_lines.len() {
                                app.file_lines[fl] = text;
                            }
                        }
                    }
                    app.recompute_match();
                    app.update_git_statuses();
                    app.refresh_git_diff_side_rows();
                    app.set_message(StatusMessage::success("Reverted line to HEAD"));
                }
            }
        }
        VimCmd::OpenLineBelow => {
            if let Some(fl) = row_to_file_line(app, cursor_row) {
                app.save_history();
                let new_fl = fl + 1;
                app.file_lines.insert(new_fl, String::new());
                app.cursor_line = Some(new_fl);
                app.insert_cursor = 0;
                app.recompute_match();
                app.refresh_git_diff_side_rows();
                app.set_message(StatusMessage::info("Opened new line below"));
            }
        }
        VimCmd::OpenLineAbove => {
            if let Some(fl) = row_to_file_line(app, cursor_row) {
                app.save_history();
                let new_fl = fl;
                app.file_lines.insert(new_fl, String::new());
                app.cursor_line = Some(new_fl);
                app.insert_cursor = 0;
                app.recompute_match();
                app.refresh_git_diff_side_rows();
                app.set_message(StatusMessage::info("Opened new line above"));
            }
        }
        VimCmd::RepeatLast | VimCmd::NextSearchMatch | VimCmd::PrevSearchMatch => {}
    }
}

fn handle_git_diff_insert_mode(app: &mut MergeApp, ui: &mut Ui) {
    let mut changed = false;
    ui.input(|i| {
        if i.key_pressed(Key::Escape) {
            app.git_diff_insert_mode = false;
            app.refresh_git_diff_side_rows();
            return;
        }
        let Some(cur) = app.cursor_line else {
            return;
        };
        if i.key_pressed(Key::ArrowLeft) {
            app.insert_cursor = app.insert_cursor.saturating_sub(1);
        }
        if i.key_pressed(Key::ArrowRight) {
            let max_len = app
                .file_lines
                .get(cur)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            app.insert_cursor = (app.insert_cursor + 1).min(max_len);
        }
        if i.key_pressed(Key::Backspace) && app.insert_cursor > 0 {
            app.save_history();
            let line = app.file_lines[cur].clone();
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(app.insert_cursor - 1);
            app.file_lines[cur] = chars.iter().collect();
            app.insert_cursor -= 1;
            changed = true;
        }
        if i.key_pressed(Key::Delete) {
            let max_len = app
                .file_lines
                .get(cur)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            if app.insert_cursor < max_len {
                app.save_history();
                let line = app.file_lines[cur].clone();
                let mut chars: Vec<char> = line.chars().collect();
                chars.remove(app.insert_cursor);
                app.file_lines[cur] = chars.iter().collect();
                changed = true;
            }
        }
        if i.key_pressed(Key::Delete) {
            let max_len = app
                .file_lines
                .get(cur)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            if app.insert_cursor < max_len {
                app.save_history();
                let line = app.file_lines[cur].clone();
                let mut chars: Vec<char> = line.chars().collect();
                chars.remove(app.insert_cursor);
                app.file_lines[cur] = chars.iter().collect();
                changed = true;
            }
        }
        for event in i.events.clone() {
            if let Event::Text(txt) = event {
                if txt != "\n" && txt != "\r" {
                    app.save_history();
                    let line = app.file_lines[cur].clone();
                    let mut new_line = String::new();
                    let mut count = 0;
                    for c in line.chars() {
                        if count == app.insert_cursor {
                            new_line.push_str(&txt);
                        }
                        new_line.push(c);
                        count += 1;
                    }
                    if count == app.insert_cursor {
                        new_line.push_str(&txt);
                    }
                    app.file_lines[cur] = new_line;
                    app.insert_cursor += txt.chars().count();
                    changed = true;
                }
            }
        }
    });
    // Perform expensive recomputes only once per frame instead of per keystroke
    if changed {
        app.recompute_match();
        app.refresh_git_diff_side_rows();
    }
}