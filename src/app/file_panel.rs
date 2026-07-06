use super::git_ops::GitStatus;
use super::matching::MergeMatching;
use super::palette::pal;
use super::state::{MarkPending, MergeApp};
use super::types::{Action, FileAnchor, StatusMessage};
use crate::diff::RowKind;
use eframe::egui::*;
use std::collections::HashSet;

pub fn render_file_panel(
    app: &mut MergeApp,
    ui: &mut Ui,
    mr: &crate::diff::MatchResult,
    row_h: f32,
    char_w: f32,
    panel_w: f32,
    row_font: &FontId,
) {
    let lnum_w = 6.0 * char_w;
    let text_x_base = 6.0 + lnum_w + 6.0 + char_w;
    let max_chars = ((panel_w - text_x_base - 10.0) / char_w).floor() as usize;
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
    let mut visual_delete = false;
    let current_hunk_idx = app.current_hunk;
    let total_hunks = app.hunks.len();
    let file_anchors = app.file_anchors.clone();
    let candidate_count = mr.candidates.len();
    let candidate_idx = app.candidate_index;
    let is_new_file_creation = app
        .current_hunk()
        .map(|h| h.search.is_empty())
        .unwrap_or(false);
    let is_applied = app.applied_hunks.contains(&app.current_hunk);
    let score_ok =
        is_new_file_creation || mr.score >= app.min_match_score || !file_anchors.is_empty();
    let can_apply = !is_applied && score_ok;
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

    let patch_source_badge = app.initial_patch_path.as_ref().map(|p| {
        if p.contains("imp.md") || p.ends_with("imp.md") {
            ("📄 imp.md".to_string(), Color32::from_rgb(120, 180, 255))
        } else if p.contains("todo.md") || p.ends_with("todo.md") {
            ("📋 todo.md".to_string(), Color32::from_rgb(230, 180, 90))
        } else if p == "temp.md" {
            ("📎 temp.md".to_string(), Color32::from_rgb(180, 130, 230))
        } else {
            let name = std::path::Path::new(p)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| p.clone());
            (format!("📄 {}", name), Color32::from_rgb(235, 235, 235))
        }
    });

    Frame::none()
        .fill(Color32::from_rgb(25, 32, 42))
        .inner_margin(Margin::symmetric(6.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if let Some((label, color)) = &patch_source_badge {
                    // Perceptual luminance to pick a readable text color against the solid fill
                    let lum = 0.299 * color.r() as f32
                        + 0.587 * color.g() as f32
                        + 0.114 * color.b() as f32;
                    let text_color = if lum > 140.0 {
                        Color32::from_rgb(15, 15, 20)
                    } else {
                        Color32::from_rgb(245, 245, 245)
                    };
                    Frame::none()
                        .fill(*color)
                        .rounding(3.0)
                        .inner_margin(Margin::symmetric(6.0, 2.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(label)
                                    .color(text_color)
                                    .strong()
                                    .monospace()
                                    .small(),
                            );
                        });
                    ui.add(Separator::default().vertical());
                }
                if unique_files.len() > 1 {
                    ui.label(RichText::new("File:").color(pal::TEXT_DIM).size(12.0));
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
                        .add(
                            Button::new(RichText::new("▲").font(FontId::monospace(11.0)))
                                .min_size(Vec2::new(20.0, row_h - 4.0)),
                        )
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
                        .add(
                            Button::new(RichText::new("▼").font(FontId::monospace(11.0)))
                                .min_size(Vec2::new(20.0, row_h - 4.0)),
                        )
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
                if can_apply {
                    ui.add_enabled_ui(can_apply, |ui| {
                        let btn_text = if is_applied {
                            "Applied".to_string()
                        } else {
                            let (search_len, replace_len) = app
                                .current_hunk()
                                .map(|h| (h.search.len(), h.replace.len()))
                                .unwrap_or((0, 0));
                            format!(
                                "⚡ Apply @ {} ({} -> {} ln)",
                                apply_line, search_len, replace_len
                            )
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
                            .on_hover_text(
                                "Apply this hunk to the file (A when cursor is in match)",
                            )
                            .clicked()
                        {
                            apply_clicked = true;
                        }
                    });
                }
                ui.add(Separator::default().vertical());
                if ui
                    .selectable_label(app.show_git_status_window, "📝 Git Status (F1)")
                    .clicked()
                {
                    app.show_git_status_window = !app.show_git_status_window;
                }
                if ui
                    .selectable_label(app.show_git_diff_window, "📝 Git Diff (F2)")
                    .clicked()
                {
                    app.show_git_diff_window = !app.show_git_diff_window;
                }
                if ui
                    .selectable_label(app.show_debug, "🐞 Debug (F10)")
                    .clicked()
                {
                    app.show_debug = !app.show_debug;
                }
            });
        });
    if candidate_count > 1 {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(RichText::new("Candidates:").color(pal::TEXT_DIM).small());
            for (idx, &(cand_start, _cand_end, cand_score)) in mr.candidates.iter().enumerate() {
                let is_current = idx == candidate_idx;
                let color = if cand_score >= app.min_match_score {
                    pal::ACCENT_GOOD
                } else if cand_score >= app.min_match_score * 0.7 {
                    pal::ACCENT_WARN
                } else {
                    pal::ACCENT_BAD
                };
                let label = RichText::new(format!("{:.0}% @{}", cand_score, cand_start + 1))
                    .color(color)
                    .monospace()
                    .small();
                if ui.selectable_label(is_current, label).clicked() {
                    app.candidate_index = idx;
                    app.scroll_to_match = true;
                    app.recompute_match();
                }
            }
        });
    }
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
        } else if app.is_insert_mode {
            ui.ctx().set_cursor_icon(CursorIcon::Text);
            ui.input(|i| {
                if i.key_pressed(Key::Escape) {
                    app.is_insert_mode = false;
                }
                if i.key_pressed(Key::ArrowLeft) {
                    app.insert_cursor = app.insert_cursor.saturating_sub(1);
                }
                if i.key_pressed(Key::ArrowRight) {
                    let max_len = app
                        .file_lines
                        .get(app.cursor_line.unwrap_or(0))
                        .map(|l| l.chars().count())
                        .unwrap_or(0);
                    app.insert_cursor = (app.insert_cursor + 1).min(max_len);
                }
                if i.key_pressed(Key::ArrowUp) {
                    let cur = app.cursor_line.unwrap_or(0);
                    if cur > 0 {
                        app.cursor_line = Some(cur - 1);
                        let max_len = app
                            .file_lines
                            .get(cur - 1)
                            .map(|l| l.chars().count())
                            .unwrap_or(0);
                        app.insert_cursor = app.insert_cursor.min(max_len);
                    }
                }
                if i.key_pressed(Key::ArrowDown) {
                    let cur = app.cursor_line.unwrap_or(0);
                    if cur < app.file_lines.len() - 1 {
                        app.cursor_line = Some(cur + 1);
                        let max_len = app
                            .file_lines
                            .get(cur + 1)
                            .map(|l| l.chars().count())
                            .unwrap_or(0);
                        app.insert_cursor = app.insert_cursor.min(max_len);
                    }
                }
                if i.key_pressed(Key::Home) {
                    app.insert_cursor = 0;
                }
                if i.key_pressed(Key::End) {
                    let max_len = app
                        .file_lines
                        .get(app.cursor_line.unwrap_or(0))
                        .map(|l| l.chars().count())
                        .unwrap_or(0);
                    app.insert_cursor = max_len;
                }
                if i.key_pressed(Key::Enter) {
                    if let Some(cur) = app.cursor_line {
                        app.save_history();
                        let line = app.file_lines[cur].clone();
                        let left: String = line.chars().take(app.insert_cursor).collect();
                        let right: String = line.chars().skip(app.insert_cursor).collect();
                        app.file_lines[cur] = left;
                        app.file_lines.insert(cur + 1, right);
                        app.cursor_line = Some(cur + 1);
                        app.insert_cursor = 0;
                        app.scroll_to_match = true;
                        app.recompute_match();
                        app.update_git_statuses();
                    }
                }
                if i.key_pressed(Key::Backspace) {
                    if app.insert_cursor > 0 {
                        if let Some(cur) = app.cursor_line {
                            app.save_history();
                            let line = app.file_lines[cur].clone();
                            let mut chars: Vec<char> = line.chars().collect();
                            chars.remove(app.insert_cursor - 1);
                            app.file_lines[cur] = chars.iter().collect();
                            app.insert_cursor -= 1;
                            app.recompute_match();
                            app.update_git_statuses();
                        }
                    } else if let Some(cur) = app.cursor_line {
                        if cur > 0 {
                            app.save_history();
                            let line = app.file_lines[cur].clone();
                            let prev_len = app.file_lines[cur - 1].chars().count();
                            app.file_lines[cur - 1].push_str(&line);
                            app.file_lines.remove(cur);
                            app.cursor_line = Some(cur - 1);
                            app.insert_cursor = prev_len;
                            app.scroll_to_match = true;
                            app.recompute_match();
                            app.update_git_statuses();
                        }
                    }
                }
                for event in i.events.clone() {
                    if let Event::Text(txt) = event {
                        if txt != "\n" && txt != "\r" {
                            if let Some(cur) = app.cursor_line {
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
                                app.recompute_match();
                                app.update_git_statuses();
                            }
                        }
                    }
                }
            });
        } else if !ui.ctx().wants_keyboard_input() {
            let mut cursor_changed = false;
            let mut new_text = String::new();
            ui.input(|i| {
                if app.mark_pending == Some(MarkPending::WaitingKey) {
                    for event in i.events.clone() {
                        if let Event::Text(txt) = event {
                            if txt.len() == 1 {
                                let c = txt.chars().next().unwrap();
                                if c == 'a' {
                                    if let Some(cur) = app.cursor_line {
                                        app.set_mark_a(cur);
                                    }
                                } else if c == 'A' {
                                    if let Some(cur) = app.cursor_line {
                                        app.set_mark_a_end(cur);
                                    }
                                } else if c.is_ascii_alphabetic() {
                                    if let Some(cur) = app.cursor_line {
                                        app.set_mark(c, cur);
                                    }
                                }
                                app.mark_pending = None;
                            }
                        }
                    }
                    return;
                }
                let cur = app.cursor_line.unwrap_or(0);
                if i.key_pressed(Key::Equals) && i.modifiers.alt {
                    go_next_file = true;
                }
                if i.key_pressed(Key::Minus) && i.modifiers.alt {
                    go_prev_file = true;
                }
                if i.key_pressed(Key::W) && i.modifiers.alt {
                    app.save_file();
                }
                if i.key_pressed(Key::W)
                    && !i.modifiers.alt
                    && !i.modifiers.shift
                    && !i.modifiers.ctrl
                {
                    app.save_all_files();
                }
                if i.key_pressed(Key::Q) && i.modifiers.alt {
                    app.quit_requested = true;
                }
                if i.key_pressed(Key::ArrowDown) {
                    let new_cur = (cur + 1).min(len - 1);
                    app.cursor_line = Some(new_cur);
                    let max_col = app
                        .file_lines
                        .get(new_cur)
                        .map(|l| l.chars().count().saturating_sub(1))
                        .unwrap_or(0);
                    app.cursor_col = app.cursor_col.min(max_col);
                    cursor_changed = true;
                }
                if i.key_pressed(Key::ArrowUp) {
                    let new_cur = cur.saturating_sub(1);
                    app.cursor_line = Some(new_cur);
                    let max_col = app
                        .file_lines
                        .get(new_cur)
                        .map(|l| l.chars().count().saturating_sub(1))
                        .unwrap_or(0);
                    app.cursor_col = app.cursor_col.min(max_col);
                    cursor_changed = true;
                }
                if i.key_pressed(Key::ArrowLeft) {
                    app.cursor_col = app.cursor_col.saturating_sub(1);
                    cursor_changed = true;
                }
                if i.key_pressed(Key::ArrowRight) {
                    let max_col = app
                        .file_lines
                        .get(cur)
                        .map(|l| l.chars().count().saturating_sub(1))
                        .unwrap_or(0);
                    app.cursor_col = (app.cursor_col + 1).min(max_col);
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
                    app.cursor_col = 0;
                    cursor_changed = true;
                }
                if i.key_pressed(Key::End) {
                    let max_col = app
                        .file_lines
                        .get(cur)
                        .map(|l| l.chars().count().saturating_sub(1))
                        .unwrap_or(0);
                    app.cursor_col = max_col;
                    cursor_changed = true;
                }
                if i.key_pressed(Key::Escape) {
                    if app.is_visual_mode {
                        app.is_visual_mode = false;
                        app.visual_start = None;
                    } else if app.d_pending {
                        app.d_pending = false;
                        app.vim_buffer.clear();
                    }
                }
                if i.key_pressed(Key::D) {
                    if i.modifiers.alt {
                        app.delete_lines(1);
                        app.last_action = Some(Action::DeleteLines(1));
                    } else if app.is_visual_mode {
                        visual_delete = true;
                    } else {
                        app.d_pending = true;
                    }
                }
                if i.key_pressed(Key::X) && app.is_visual_mode {
                    visual_delete = true;
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
                    } else if score_ok && in_hunk {
                        apply_clicked = true;
                    } else {
                        app.cursor_line = Some(mr.file_start);
                        cursor_changed = true;
                    }
                }
                for event in i.events.clone() {
                    if let Event::Text(txt) = event {
                        if txt == "v" || txt == "V" {
                            if app.is_visual_mode {
                                app.is_visual_mode = false;
                                app.visual_start = None;
                            } else if app.cursor_line.is_some() {
                                app.is_visual_mode = true;
                                app.visual_start = app.cursor_line;
                            }
                        } else if txt == "m" {
                            app.mark_pending = Some(MarkPending::WaitingKey);
                        } else if txt == "o" {
                            if let Some(cur) = app.cursor_line {
                                app.save_history();
                                if cur + 1 <= app.file_lines.len() {
                                    app.file_lines.insert(cur + 1, String::new());
                                    app.cursor_line = Some(cur + 1);
                                    app.scroll_to_match = true;
                                    app.recompute_match();
                                    app.update_git_statuses();
                                    app.is_insert_mode = true;
                                    app.insert_cursor = 0;
                                    app.set_message(StatusMessage::info("Opened new line below"));
                                }
                            }
                        } else if txt == "O" {
                            if let Some(cur) = app.cursor_line {
                                app.save_history();
                                app.file_lines.insert(cur, String::new());
                                app.cursor_line = Some(cur);
                                app.scroll_to_match = true;
                                app.recompute_match();
                                app.update_git_statuses();
                                app.is_insert_mode = true;
                                app.insert_cursor = 0;
                                app.set_message(StatusMessage::info("Opened new line above"));
                            }
                        } else if txt == "+" || txt == "-" {
                            let delta: i32 = if txt == "+" { 1 } else { -1 };
                            if let Some(cur) = app.cursor_line {
                                // If no anchors exist, treat the auto match boundaries as anchors
                                if app.file_anchors.is_empty() && mr.score > 0.0 {
                                    if cur == mr.file_start || cur == mr.file_end.saturating_sub(1)
                                    {
                                        app.file_anchors.insert(
                                            'a',
                                            FileAnchor {
                                                id: 'a',
                                                line: mr.file_start,
                                                end_line: Some(mr.file_end.saturating_sub(1)),
                                            },
                                        );
                                    }
                                }

                                let on_end = app
                                    .file_anchors
                                    .values()
                                    .find(|a| a.end_line == Some(cur))
                                    .map(|a| a.id);
                                let on_start = app
                                    .file_anchors
                                    .values()
                                    .find(|a| a.line == cur)
                                    .map(|a| a.id);

                                if let Some(id) = on_end {
                                    if let Some(anchor) = app.file_anchors.get_mut(&id) {
                                        let current_end = anchor.end_line.unwrap_or(anchor.line);
                                        let new_end = (current_end as i32 + delta).clamp(
                                            anchor.line as i32,
                                            app.file_lines.len().saturating_sub(1) as i32,
                                        )
                                            as usize;
                                        anchor.end_line = Some(new_end);
                                        app.cursor_line = Some(new_end);
                                        cursor_changed = true;
                                        app.scroll_to_match = true;
                                    }
                                } else if let Some(id) = on_start {
                                    if let Some(anchor) = app.file_anchors.get_mut(&id) {
                                        let max_bound = anchor
                                            .end_line
                                            .unwrap_or(app.file_lines.len().saturating_sub(1))
                                            as i32;
                                        let new_start = (anchor.line as i32 + delta)
                                            .clamp(0, max_bound)
                                            as usize;
                                        anchor.line = new_start;
                                        app.cursor_line = Some(new_start);
                                        cursor_changed = true;
                                        app.scroll_to_match = true;
                                    }
                                }
                            }
                        } else if txt == "i" {
                            app.is_insert_mode = true;
                        } else if txt == "I" {
                            app.is_insert_mode = true;
                            app.insert_cursor = 0;
                        } else if txt != "?"
                            && txt != "m"
                            && txt != "v"
                            && txt != "V"
                            && txt != "o"
                            && txt != "O"
                            && txt != "+"
                            && txt != "-"
                            && txt != "i"
                            && txt != "I"
                            && (txt != "a" || app.d_pending)
                            && txt != "H"
                            && txt != "L"
                        {
                            new_text.push_str(&txt);
                        }
                    }
                }
            });
            if app.is_visual_mode {
                app.vim_buffer.clear();
                app.d_pending = false;
            }
            if !new_text.is_empty() {
                if app.is_visual_mode {
                    app.vim_buffer.clear();
                } else {
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
                    } else if buf == "yy" {
                        if let Some(cur) = app.cursor_line {
                            if let Some(line) = app.file_lines.get(cur) {
                                app.yanked_line = Some(line.clone());
                                app.set_message(StatusMessage::info(format!(
                                    "Yanked line {}",
                                    cur + 1
                                )));
                            }
                        }
                        clear_buffer = true;
                    } else if lower_buf == "p" {
                        if let Some(line) = app.yanked_line.clone() {
                            if let Some(cur) = app.cursor_line {
                                app.save_history();
                                if cur + 1 <= app.file_lines.len() {
                                    app.file_lines.insert(cur + 1, line);
                                    app.cursor_line = Some(cur + 1);
                                    app.scroll_to_match = true;
                                    app.recompute_match();
                                    app.update_git_statuses();
                                    app.set_message(StatusMessage::info("Pasted below"));
                                }
                            }
                        }
                        clear_buffer = true;
                    } else if buf == "P" {
                        if let Some(line) = app.yanked_line.clone() {
                            if let Some(cur) = app.cursor_line {
                                app.save_history();
                                app.file_lines.insert(cur, line);
                                app.cursor_line = Some(cur);
                                app.scroll_to_match = true;
                                app.recompute_match();
                                app.update_git_statuses();
                                app.set_message(StatusMessage::info("Pasted above"));
                            }
                        }
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
                                || c == 'y'
                                || c == 'p'
                                || c == 'P'
                        }) || lower_buf == "da"
                            || lower_buf == "daf";
                        let d_count = buf.matches('d').count() + buf.matches('D').count();
                        if !allowed || d_count > 2 {
                            clear_buffer = true;
                        }
                    }
                    if clear_buffer {
                        app.vim_buffer.clear();
                        app.d_pending = false;
                    }
                }
            }
            if cursor_changed {
                app.scroll_to_match = true;
                ui.ctx().request_repaint();
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
                app.set_message(StatusMessage::info(format!(
                    "No previous hunk matching >= {:.0}%",
                    app.min_match_score
                )));
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
                app.set_message(StatusMessage::info(format!(
                    "No next hunk matching >= {:.0}%",
                    app.min_match_score
                )));
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
    let visual_range = if app.is_visual_mode {
        if let Some(start) = app.visual_start {
            let cur = cursor_line.unwrap_or(start);
            Some((start.min(cur), start.max(cur)))
        } else {
            None
        }
    } else {
        None
    };

    let mut did_scroll = false;
    let mut set_cursor: Option<usize> = None;
    let mut set_del_start: Option<usize> = None;
    let mut set_del_end: Option<usize> = None;
    let mut clear_del = false;
    let mut perform_block_delete: Option<(usize, usize)> = None;
    let mut set_anchor_a_start: Option<usize> = None;
    let mut set_anchor_a_end: Option<usize> = None;
    let mut adjust_start_by: i32 = 0;
    let mut adjust_end_by: i32 = 0;
    let mut local_drag_anchor = app.file_drag_anchor;
    let mut local_drag_selection = app.file_drag_selection;
    let mut anchor_a_start_point: Option<Pos2> = None;
    let mut anchor_a_end_point: Option<Pos2> = None;
    let pointer_pos = ui.input(|i| i.pointer.interact_pos());
    let primary_down = ui.input(|i| i.pointer.primary_down());
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
    // Action toolbar for the current drag selection (one-frame lag, same
    // pattern as the diff-side HUD): reads app.file_drag_selection from the
    // previous frame, since this frame's selection is only known once the
    // row loop below runs.
    ScrollArea::both()
        .id_source("file_scroll")
        .auto_shrink([false, false])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            for (i, line) in file_lines.iter().enumerate() {
                let in_auto_match = i >= auto_start && i < auto_end;
                let anchor_here = file_anchors.values().find(|a| a.line == i);
                let anchor_end_here = file_anchors
                    .values()
                    .find(|a| a.id == 'a' && a.end_line == Some(i));
                let is_anchor = anchor_here.is_some() || anchor_end_here.is_some();
                let in_anchor_a_range = file_anchors.get(&'a').map_or(false, |a| {
                    let end = a.end_line.unwrap_or(a.line);
                    let (lo, hi) = (a.line.min(end), a.line.max(end));
                    i >= lo && i <= hi
                });
                let is_cursor = cursor_line == Some(i);
                let in_merged = merged_range.map_or(false, |(rs, re)| i >= rs && i < re);
                let is_delete = in_auto_match && delete_file_indices.contains(&i);
                let is_equal = in_auto_match && equal_file_indices.contains(&i);
                // Lines inside the match window that are in the file but were not
                // part of the search block at all (e.g. code inserted after the
                // last time this hunk was applied).
                let is_extra = in_auto_match && file_anchors.is_empty() && !is_delete && !is_equal;
                let in_block_delete = match (app.del_start, app.del_end) {
                    (Some(s), Some(e)) => i >= s.min(e) && i <= s.max(e),
                    (Some(s), None) => i == s,
                    (None, Some(e)) => i == e,
                    (None, None) => false,
                };
                let in_visual_selection =
                    visual_range.map_or(false, |(min, max)| i >= min && i <= max);
                let in_drag_selection =
                    local_drag_selection.map_or(false, |(s, e)| i >= s && i <= e);
                let is_search_hit = !search_query.is_empty()
                    && line.to_lowercase().contains(&search_query.to_lowercase());
                let is_current_search = is_search_hit && current_search_line == Some(i);
                let is_auto_start_line =
                    in_auto_match && i == auto_start && file_anchors.is_empty();
                let is_auto_end_line =
                    in_auto_match && i == auto_end.saturating_sub(1) && file_anchors.is_empty();
                let git_status = git_statuses.get(i).copied().unwrap_or(GitStatus::Unchanged);
                let row_is_tall = is_anchor;
                let desired = Vec2::new(
                    ui.available_width(),
                    if row_is_tall { row_h + 6.0 } else { row_h },
                );
                let (rect, row_resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());
                let should_scroll = scroll_to_match
                    && (is_cursor
                        || (cursor_line.is_none() && is_anchor)
                        || (cursor_line.is_none() && is_auto_start_line));
                if should_scroll {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    did_scroll = true;
                }
                let is_anchor_start = anchor_here.is_some();
                let is_anchor_end = anchor_end_here.is_some();
                let is_anchor_row = is_anchor_start || is_anchor_end;
                let base_bg = if in_drag_selection {
                    Color32::from_rgb(45, 30, 65)
                } else if in_visual_selection {
                    Color32::from_rgb(20, 50, 25)
                } else if is_anchor_row {
                    Color32::from_rgba_premultiplied(45, 38, 15, 60)
                } else if in_anchor_a_range {
                    Color32::from_rgba_premultiplied(45, 38, 15, 32)
                } else if in_block_delete {
                    pal::BG_DELETE
                } else if in_merged {
                    pal::BG_MERGED
                } else if is_delete {
                    pal::BG_DELETE
                } else if is_extra {
                    Color32::from_rgb(25, 40, 55)
                } else if is_cursor {
                    pal::BG_CURSOR
                } else if in_auto_match && file_anchors.is_empty() && !is_auto_start_line {
                    pal::BG_MATCH
                } else if i % 2 == 0 {
                    pal::BG_ROW_EVEN
                } else {
                    pal::BG_ROW_ODD
                };
                let row_bg = if is_current_search {
                    Color32::from_rgb(70, 60, 15)
                } else if is_search_hit {
                    pal::BG_SEARCH_HIT
                } else {
                    base_bg
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
                let bar = Rect::from_min_size(
                    Pos2::new(rect.left() + 2.0, rect.top()),
                    Vec2::new(3.0, rect.height()),
                );
                let bar_color = if in_drag_selection {
                    Color32::from_rgb(160, 110, 230)
                } else if in_visual_selection {
                    Color32::from_rgb(60, 120, 70)
                } else if is_anchor_row {
                    pal::BAR_ANCHOR
                } else if in_anchor_a_range {
                    Color32::from_rgba_premultiplied(
                        pal::BAR_ANCHOR.r(),
                        pal::BAR_ANCHOR.g(),
                        pal::BAR_ANCHOR.b(),
                        140,
                    )
                } else if in_block_delete {
                    pal::TEXT_DELETE
                } else if in_merged {
                    pal::BAR_MERGED
                } else if is_delete {
                    pal::TEXT_DELETE
                } else if is_extra {
                    Color32::from_rgb(90, 160, 220)
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
                    let max_col = line.chars().count().saturating_sub(1);
                    app.cursor_col = app.cursor_col.min(max_col);
                }
                if primary_down {
                    if let Some(pos) = pointer_pos {
                        if rect.contains(pos) {
                            if local_drag_anchor.is_none() {
                                local_drag_anchor = Some(i);
                            }
                            let anchor = local_drag_anchor.unwrap();
                            let lo = anchor.min(i);
                            let hi = anchor.max(i);
                            local_drag_selection = Some((lo, hi));
                        }
                    }
                }
                let num_color = if is_anchor_row {
                    pal::TEXT_ANCHOR
                } else if in_anchor_a_range {
                    pal::TEXT_ANCHOR
                } else if in_block_delete {
                    pal::TEXT_DELETE
                } else if in_merged {
                    pal::TEXT_LNUM_ACTIVE
                } else if is_delete {
                    pal::TEXT_DELETE
                } else if is_extra {
                    Color32::from_rgb(120, 180, 230)
                } else if in_auto_match && file_anchors.is_empty() {
                    pal::TEXT_LNUM_ACTIVE
                } else if is_search_hit {
                    Color32::from_rgb(180, 160, 60)
                } else {
                    pal::TEXT_LNUM
                };
                let lnum_x = rect.left() + 6.0;
                let diff_x = lnum_x + lnum_w + 6.0;
                let text_x = diff_x + char_w;

                ui.painter().text(
                    Pos2::new(lnum_x, rect.center().y),
                    Align2::LEFT_CENTER,
                    format!("{:>4} │", i + 1),
                    row_font.clone(),
                    num_color,
                );
                let diff_prefix = if in_auto_match && file_anchors.is_empty() {
                    if is_delete {
                        Some(("-", pal::TEXT_DELETE))
                    } else if is_equal {
                        Some(("=", Color32::from_gray(60)))
                    } else if is_extra {
                        Some(("+", Color32::from_rgb(120, 180, 230)))
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some((glyph, glyph_color)) = diff_prefix {
                    ui.painter().text(
                        Pos2::new(diff_x, rect.center().y),
                        Align2::LEFT_CENTER,
                        glyph,
                        row_font.clone(),
                        glyph_color,
                    );
                }
                let text_color = if is_anchor_row {
                    pal::TEXT_ANCHOR
                } else if in_anchor_a_range {
                    pal::TEXT_ANCHOR
                } else if in_block_delete {
                    pal::TEXT_DELETE
                } else if in_merged {
                    pal::TEXT_MERGED
                } else if is_delete {
                    pal::TEXT_DELETE
                } else if is_extra {
                    Color32::from_rgb(140, 190, 235)
                } else if in_auto_match && file_anchors.is_empty() {
                    pal::TEXT_MATCH
                } else if is_search_hit {
                    pal::TEXT_SEARCH
                } else {
                    pal::TEXT_NORMAL
                };
                let display_max_chars = if is_auto_start_line || is_anchor_start {
                    ((panel_w - text_x_base - 250.0) / char_w).floor() as usize
                } else if is_auto_end_line || is_anchor_end {
                    ((panel_w - text_x_base - 120.0) / char_w).floor() as usize
                } else {
                    max_chars
                };
                let display = MergeApp::truncate_owned(line, display_max_chars);
                ui.painter().text(
                    Pos2::new(text_x, rect.center().y),
                    Align2::LEFT_CENTER,
                    &display,
                    row_font.clone(),
                    text_color,
                );
                if is_cursor {
                    let cursor_x = text_x;
                    if app.is_insert_mode {
                        let col = app.insert_cursor.min(line.chars().count());
                        let char_x = cursor_x + (col as f32 * char_w);
                        ui.painter().line_segment(
                            [
                                Pos2::new(char_x, rect.top() + 2.0),
                                Pos2::new(char_x, rect.bottom() - 2.0),
                            ],
                            Stroke::new(2.0, Color32::from_rgb(255, 80, 80)),
                        );
                    } else {
                        let col = app.cursor_col.min(line.chars().count().saturating_sub(1));
                        let char_x = cursor_x + (col as f32 * char_w);
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                Pos2::new(char_x, rect.top() + 2.0),
                                Vec2::new(char_w, rect.height() - 4.0),
                            ),
                            0.0,
                            Color32::from_rgba_premultiplied(220, 40, 40, 200),
                        );
                    }
                }
                if is_anchor_start {
                    let anchor = anchor_here.unwrap();
                    let is_range_anchor = anchor.id == 'a' && anchor.end_line.is_some();
                    let right_box_width = if is_range_anchor { 250.0 } else { 106.0 };
                    let right_box_rect = Rect::from_min_size(
                        Pos2::new(rect.right() - right_box_width, rect.top()),
                        Vec2::new(right_box_width, rect.height()),
                    );
                    ui.painter().rect_filled(
                        right_box_rect,
                        2.0,
                        Color32::from_rgba_premultiplied(45, 38, 15, 230),
                    );
                    if anchor.id == 'a' {
                        app.anchor_link_target =
                            Some(Pos2::new(right_box_rect.left(), rect.center().y));
                    }
                    if is_range_anchor {
                        anchor_a_start_point =
                            Some(Pos2::new(right_box_rect.left(), rect.center().y));
                        let mut next_x = right_box_rect.left() + 8.0;
                        let btn_w = 18.0;
                        let btn_h = row_h - 6.0;
                        ui.painter().text(
                            Pos2::new(next_x, rect.center().y),
                            Align2::LEFT_CENTER,
                            "⚓ma S:",
                            FontId::monospace(10.0),
                            pal::TEXT_ANCHOR,
                        );
                        next_x += 55.0;
                        let dec_s_rect = Rect::from_min_size(
                            Pos2::new(next_x, rect.center().y - btn_h / 2.0),
                            Vec2::new(btn_w, btn_h),
                        );
                        if ui
                            .put(
                                dec_s_rect,
                                Button::new(RichText::new("▲").small().monospace())
                                    .fill(Color32::from_rgb(65, 50, 10)),
                            )
                            .clicked()
                        {
                            adjust_start_by = -1;
                        }
                        next_x += btn_w + 2.0;
                        let inc_s_rect = Rect::from_min_size(
                            Pos2::new(next_x, rect.center().y - btn_h / 2.0),
                            Vec2::new(btn_w, btn_h),
                        );
                        if ui
                            .put(
                                inc_s_rect,
                                Button::new(RichText::new("▼").small().monospace())
                                    .fill(Color32::from_rgb(65, 50, 10)),
                            )
                            .clicked()
                        {
                            adjust_start_by = 1;
                        }
                        let btn_size = Vec2::new(65.0, row_h - 4.0);
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
                                        .small()
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
                    }
                } else if is_anchor_end {
                    let right_box_width = 120.0;
                    let right_box_rect = Rect::from_min_size(
                        Pos2::new(rect.right() - right_box_width, rect.top()),
                        Vec2::new(right_box_width, rect.height()),
                    );
                    ui.painter().rect_filled(
                        right_box_rect,
                        2.0,
                        Color32::from_rgba_premultiplied(45, 38, 15, 230),
                    );
                    anchor_a_end_point = Some(Pos2::new(right_box_rect.left(), rect.center().y));
                    let mut next_x = right_box_rect.left() + 6.0;
                    let btn_w = 18.0;
                    let btn_h = row_h - 6.0;
                    ui.painter().text(
                        Pos2::new(next_x, rect.center().y),
                        Align2::LEFT_CENTER,
                        "⚓mA End:",
                        FontId::monospace(10.0),
                        pal::TEXT_ANCHOR,
                    );
                    next_x += 62.0;
                    let dec_e_rect = Rect::from_min_size(
                        Pos2::new(next_x, rect.center().y - btn_h / 2.0),
                        Vec2::new(btn_w, btn_h),
                    );
                    if ui
                        .put(
                            dec_e_rect,
                            Button::new(RichText::new("▲").small().monospace())
                                .fill(Color32::from_rgb(65, 50, 10)),
                        )
                        .clicked()
                    {
                        adjust_end_by = -1;
                    }
                    next_x += btn_w + 2.0;
                    let inc_e_rect = Rect::from_min_size(
                        Pos2::new(next_x, rect.center().y - btn_h / 2.0),
                        Vec2::new(btn_w, btn_h),
                    );
                    if ui
                        .put(
                            inc_e_rect,
                            Button::new(RichText::new("▼").small().monospace())
                                .fill(Color32::from_rgb(65, 50, 10)),
                        )
                        .clicked()
                    {
                        adjust_end_by = 1;
                    }
                } else if is_auto_start_line && mr.score > 0.0 {
                    let right_box_width = 250.0;
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
                        Pos2::new(right_box_rect.left() + 6.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("▼ {}–{} ({:.0}%)", auto_start + 1, auto_end, auto_score),
                        FontId::monospace(10.0),
                        Color32::from_rgb(120, 230, 160),
                    );
                    let mut next_x = right_box_rect.left() + 115.0;
                    let btn_w = 18.0;
                    let btn_h = row_h - 6.0;
                    ui.painter().text(
                        Pos2::new(next_x, rect.center().y),
                        Align2::LEFT_CENTER,
                        "S:",
                        FontId::monospace(10.0),
                        Color32::from_rgb(180, 220, 190),
                    );
                    next_x += 14.0;
                    let dec_s_rect = Rect::from_min_size(
                        Pos2::new(next_x, rect.center().y - btn_h / 2.0),
                        Vec2::new(btn_w, btn_h),
                    );
                    if ui
                        .put(
                            dec_s_rect,
                            Button::new(RichText::new("▲").small().monospace())
                                .fill(Color32::from_rgb(40, 55, 45)),
                        )
                        .clicked()
                    {
                        adjust_start_by = -1;
                    }
                    next_x += btn_w + 2.0;
                    let inc_s_rect = Rect::from_min_size(
                        Pos2::new(next_x, rect.center().y - btn_h / 2.0),
                        Vec2::new(btn_w, btn_h),
                    );
                    if ui
                        .put(
                            inc_s_rect,
                            Button::new(RichText::new("▼").small().monospace())
                                .fill(Color32::from_rgb(40, 55, 45)),
                        )
                        .clicked()
                    {
                        adjust_start_by = 1;
                    }
                    let btn_size = Vec2::new(65.0, row_h - 4.0);
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
                                    .small()
                                    .monospace(),
                            )
                            .fill(Color32::from_rgb(35, 85, 50))
                            .stroke(Stroke::new(1.0, pal::BAR_MATCH)),
                        )
                        .clicked()
                    {
                        apply_clicked = true;
                    }
                } else if is_auto_end_line && mr.score > 0.0 {
                    let right_box_width = 120.0;
                    let right_box_rect = Rect::from_min_size(
                        Pos2::new(rect.right() - right_box_width, rect.top()),
                        Vec2::new(right_box_width, rect.height()),
                    );
                    ui.painter().rect_filled(
                        right_box_rect,
                        2.0,
                        Color32::from_rgba_premultiplied(28, 60, 40, 230),
                    );
                    let mut next_x = right_box_rect.left() + 6.0;
                    let btn_w = 18.0;
                    let btn_h = row_h - 6.0;
                    ui.painter().text(
                        Pos2::new(next_x, rect.center().y),
                        Align2::LEFT_CENTER,
                        "End block:",
                        FontId::monospace(10.0),
                        Color32::from_rgb(120, 230, 160),
                    );
                    next_x += 62.0;
                    let dec_e_rect = Rect::from_min_size(
                        Pos2::new(next_x, rect.center().y - btn_h / 2.0),
                        Vec2::new(btn_w, btn_h),
                    );
                    if ui
                        .put(
                            dec_e_rect,
                            Button::new(RichText::new("▲").small().monospace())
                                .fill(Color32::from_rgb(40, 55, 45)),
                        )
                        .clicked()
                    {
                        adjust_end_by = -1;
                    }
                    next_x += btn_w + 2.0;
                    let inc_e_rect = Rect::from_min_size(
                        Pos2::new(next_x, rect.center().y - btn_h / 2.0),
                        Vec2::new(btn_w, btn_h),
                    );
                    if ui
                        .put(
                            inc_e_rect,
                            Button::new(RichText::new("▼").small().monospace())
                                .fill(Color32::from_rgb(40, 55, 45)),
                        )
                        .clicked()
                    {
                        adjust_end_by = 1;
                    }
                }
                if in_auto_match && i == auto_end.saturating_sub(1) && file_anchors.is_empty() {
                    let (sep_rect, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 2.0), Sense::hover());
                    ui.painter().rect_filled(sep_rect, 0.0, pal::BAR_MATCH);
                }
                row_resp.context_menu(|ui| {
                    ui.label(RichText::new(format!("Line {}", i + 1)).strong());
                    ui.separator();
                    if ui.button("Set Anchor 'a' (ma) Start").clicked() {
                        set_anchor_a_start = Some(i);
                        ui.close_menu();
                    }
                    if ui.button("Set Anchor 'a' (mA) End").clicked() {
                        set_anchor_a_end = Some(i);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Set Block Delete Start").clicked() {
                        set_del_start = Some(i);
                        ui.close_menu();
                    }
                    if ui.button("Set Block Delete End").clicked() {
                        set_del_end = Some(i);
                        ui.close_menu();
                    }
                    if app.del_start.is_some() || app.del_end.is_some() {
                        if ui.button("Clear Block Selection").clicked() {
                            clear_del = true;
                            ui.close_menu();
                        }
                    }
                    if let Some(start) = app.del_start {
                        if let Some(end) = app.del_end {
                            ui.separator();
                            let min = start.min(end);
                            let max = start.max(end);
                            let count = max - min + 1;
                            if ui
                                .button(format!("Delete Block ({} lines)", count))
                                .clicked()
                            {
                                perform_block_delete = Some((min, max));
                                ui.close_menu();
                            }
                        }
                    }
                });
            }
            if let (Some(p1), Some(p2)) = (anchor_a_start_point, anchor_a_end_point) {
                let link_x = p1.x - 6.0;
                let stroke = Stroke::new(2.0, pal::BAR_ANCHOR);
                ui.painter()
                    .line_segment([Pos2::new(link_x, p1.y), Pos2::new(link_x, p2.y)], stroke);
                ui.painter()
                    .line_segment([Pos2::new(link_x, p1.y), Pos2::new(p1.x, p1.y)], stroke);
                ui.painter()
                    .line_segment([Pos2::new(link_x, p2.y), Pos2::new(p2.x, p2.y)], stroke);
            }
            ui.add_space(row_h * 3.0);
        });
    if !primary_down {
        local_drag_anchor = None;
    }
    app.file_drag_anchor = local_drag_anchor;
    app.file_drag_selection = local_drag_selection;
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
    if let Some(val) = set_del_start {
        app.del_start = Some(val);
    }
    if let Some(val) = set_del_end {
        app.del_end = Some(val);
    }
    if clear_del {
        app.del_start = None;
        app.del_end = None;
    }
    if let Some((min, max)) = perform_block_delete {
        app.delete_block_range(min, max);
    }
    if let Some(val) = set_anchor_a_start {
        app.set_mark_a(val);
        local_drag_selection = None;
        local_drag_anchor = None;
    }
    if let Some(val) = set_anchor_a_end {
        app.set_mark_a_end(val);
    }
    if visual_delete {
        if let Some(start) = app.visual_start {
            let cur = app.cursor_line.unwrap_or(start);
            let min = start.min(cur);
            let max = start.max(cur);
            app.delete_block_range(min, max);
            app.is_visual_mode = false;
            app.visual_start = None;
        }
    }
    if adjust_start_by != 0 || adjust_end_by != 0 {
        if !app.file_anchors.contains_key(&'a') {
            app.file_anchors.insert(
                'a',
                FileAnchor {
                    id: 'a',
                    line: auto_start,
                    end_line: Some(auto_end.saturating_sub(1)),
                },
            );
        }
        let msg = {
            if let Some(anchor) = app.file_anchors.get_mut(&'a') {
                if adjust_start_by != 0 {
                    let current_start = anchor.line;
                    let new_start = (current_start as i32 + adjust_start_by)
                        .clamp(0, app.file_lines.len().saturating_sub(1) as i32)
                        as usize;
                    anchor.line = new_start;
                }
                if adjust_end_by != 0 {
                    let current_end = anchor.end_line.unwrap_or(anchor.line);
                    let new_end = (current_end as i32 + adjust_end_by).clamp(
                        anchor.line as i32,
                        app.file_lines.len().saturating_sub(1) as i32,
                    ) as usize;
                    anchor.end_line = Some(new_end);
                }
                Some(StatusMessage::info(format!(
                    "⚓ Adjusted ma range: lines {}-{}",
                    anchor.file_start() + 1,
                    anchor.file_end() + 1
                )))
            } else {
                None
            }
        };
        if let Some(msg) = msg {
            app.set_message(msg);
        }
    }
}