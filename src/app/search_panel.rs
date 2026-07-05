use super::clipboard_utils::parse_clipboard_patch;
use super::matching::MergeMatching;
use super::palette::pal;
use super::state::MergeApp;
use super::types::StatusMessage;
use crate::diff::RowKind;
use eframe::egui::*;

pub fn render_search_panel(
    app: &mut MergeApp,
    ui: &mut Ui,
    mr: &crate::diff::MatchResult,
    row_h: f32,
    char_w: f32,
    panel_w: f32,
    row_font: &FontId,
) {
    let lnum_w = 4.0 * char_w;
    let text_x_base = 4.0 + lnum_w + 6.0 + 2.0 * char_w;
    let max_chars = ((panel_w - text_x_base - 10.0) / char_w).floor() as usize;
    let mut set_selection: Option<(usize, usize)> = None;
    let mut apply_clicked_id: Option<char> = None;
    let mut apply_clicked = false;
    let mut apply_clicked_line: Option<usize> = None;
    let mut apply_selection: Option<(usize, (usize, usize))> = None;
    let pointer_pos = ui.input(|i| i.pointer.interact_pos());
    let primary_down = ui.input(|i| i.pointer.primary_down());

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if ui
            .button("📝 Paste Manually")
            .on_hover_text("Open a manual input area to paste using Ctrl+V or Shift+Insert")
            .clicked()
        {
            app.show_manual_paste = !app.show_manual_paste;
        }
        if ui
            .button("🔄 Reset File")
            .on_hover_text("Discard all unsaved edits and reload the current file from disk")
            .clicked()
        {
            app.file_states.remove(&app.file_path);
            app.load_hunk();
            app.set_message(StatusMessage::success("File reloaded and edits discarded"));
        }
        if let Some(orig_path) = app.initial_patch_path.clone() {
            let label = if orig_path.contains("imp.md") || orig_path.ends_with("imp.md") {
                "🔄 Reload imp.md"
            } else if orig_path.contains("todo.md") || orig_path.ends_with("todo.md") {
                "🔄 Reload todo.md"
            } else if orig_path == "temp.md" {
                "🔄 Reload temp.md"
            } else {
                "🔄 Reload Original"
            };
            if ui
                .button(label)
                .on_hover_text(format!(
                    "Reload the current session patch file from disk: {}",
                    orig_path
                ))
                .clicked()
            {
                if let Ok(content) = std::fs::read_to_string(&orig_path) {
                    app.patch_text = content;
                    app.reparse();
                    app.set_message(StatusMessage::success(format!(
                        "Reloaded patch from disk: {}",
                        orig_path
                    )));
                } else {
                    app.set_message(StatusMessage::error(format!(
                        "Failed to read {}",
                        orig_path
                    )));
                }
            }
        }
    });
    ui.add_space(4.0);

    if app.show_manual_paste {
        ui.group(|ui| {
            ui.label(
                RichText::new("Paste patch/search pattern here (Ctrl+V / Shift+Ins):")
                    .small()
                    .color(pal::TEXT_DIM),
            );
            ScrollArea::vertical()
                .id_source("manual_paste_scroll")
                .max_height(row_h * 5.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut app.manual_paste_text)
                            .font(FontId::monospace(9.5))
                            .desired_width(panel_w - 32.0)
                            .desired_rows(5),
                    );
                });
            ui.horizontal(|ui| {
                if ui.button("⚡ Save to temp.md & Load").clicked() {
                    let content = app.manual_paste_text.clone();
                    let filename = "temp.md";
                    let _ = std::fs::write(filename, &content);
                    let parsed_hunks = parse_clipboard_patch(&content);
                    if !parsed_hunks.is_empty() {
                        app.initial_patch_path = Some("temp.md".to_string());
                        app.patch_text = content;
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
                        app.show_manual_paste = false;
                        if app.hunks[0].filename.is_empty() {
                            app.set_message(StatusMessage::warning(
                                "Search pattern loaded. Enter the target filename below.",
                            ));
                        } else {
                            app.load_hunk();
                            app.set_message(StatusMessage::success(
                                "Saved to temp.md & successfully loaded!",
                            ));
                        }
                    } else {
                        app.set_message(StatusMessage::error("Input content is empty or invalid"));
                    }
                }
                if ui.button("Cancel").clicked() {
                    app.show_manual_paste = false;
                }
            });
        });
        ui.add_space(4.0);
    }

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
            let is_new_file_creation = app
                .current_hunk()
                .map(|h| h.search.is_empty())
                .unwrap_or(false);

            let (banner_bg, banner_text) = if is_applied {
                (
                    Color32::from_rgb(30, 40, 30),
                    format!("✓ Applied — hunk {}", app.current_hunk + 1),
                )
            } else if is_new_file_creation {
                (
                    Color32::from_rgb(20, 45, 25),
                    "✚ New file / Append".to_string(),
                )
            } else {
                let cand_suffix = if mr.candidates.len() > 1 {
                    format!(
                        "  ·  candidate {}/{}",
                        app.candidate_index + 1,
                        mr.candidates.len()
                    )
                } else {
                    String::new()
                };
                (
                    banner_bg,
                    format!(
                        "{:.0}%  match @ lines {}–{}{}",
                        mr.score,
                        mr.file_start + 1,
                        mr.file_end,
                        cand_suffix
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
                let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
                if resp.clicked() {
                    set_selection = Some((line_idx, line_idx));
                }
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
                            app.set_message(StatusMessage::info(format!(
                                "🔍 Searched '{}': {} matches. Press n/N to cycle.",
                                q,
                                app.search_matches.len()
                            )));
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
                let lnum_x = rect.left() + 4.0;
                let prefix_x = lnum_x + lnum_w + 6.0;
                let text_x = prefix_x + 2.0 * char_w;

                ui.painter().text(
                    Pos2::new(lnum_x, rect.center().y),
                    Align2::LEFT_CENTER,
                    &num_text,
                    row_font.clone(),
                    if is_matched {
                        pal::TEXT_LNUM_ACTIVE
                    } else {
                        pal::TEXT_DIM
                    },
                );
                ui.painter().text(
                    Pos2::new(prefix_x, rect.center().y),
                    Align2::LEFT_CENTER,
                    prefix,
                    row_font.clone(),
                    prefix_color,
                );
                let display = MergeApp::truncate_owned(line, max_chars);
                ui.painter().text(
                    Pos2::new(text_x, rect.center().y),
                    Align2::LEFT_CENTER,
                    &display,
                    row_font.clone(),
                    if is_applied {
                        pal::TEXT_DIM
                    } else {
                        pal::TEXT_NORMAL
                    },
                );
            }
            ui.add_space(4.0);
            let (sep_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(sep_rect, 0.0, pal::SEPARATOR);
            ui.add_space(2.0);

            let (hdr_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover());
            let has_anchor = !app.file_anchors.is_empty();
            let (hdr_bg, hdr_text, hdr_color) = if hunk.replace.is_empty() {
                (Color32::from_rgb(45, 20, 20), "DELETE →", pal::TEXT_DELETE)
            } else if has_anchor {
                (
                    Color32::from_rgb(50, 40, 12),
                    "REPLACE ⚓ →",
                    pal::TEXT_ANCHOR,
                )
            } else {
                (Color32::from_rgb(22, 44, 28), "REPLACE →", pal::TEXT_INSERT)
            };
            ui.painter().rect_filled(hdr_rect, 4.0, hdr_bg);
            ui.painter().text(
                Pos2::new(hdr_rect.left() + 8.0, hdr_rect.center().y),
                Align2::LEFT_CENTER,
                &hdr_text,
                FontId::monospace(10.0),
                hdr_color,
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
            if let Some((&id, _)) = app.file_anchors.iter().next() {
                if id == 'a' {
                    app.anchor_link_source = Some(btn_rect.right_center());
                } else {
                    app.anchor_link_source = None;
                }
            } else {
                app.anchor_link_source = None;
            }
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
                    RichText::new(format!(">({}", cur_ln + 1))
                        .color(Color32::WHITE)
                        .strong()
                        .monospace(),
                )
                .fill(Color32::from_rgb(40, 90, 55))
                .min_size(btn_line_size);
                if ui.put(btn_line_rect, btn_line).clicked() {
                    apply_clicked_line = Some(cur_ln);
                }
                x_offset += btn_line_size.x + 4.0;
                let btn_star_rect = Rect::from_min_size(
                    Pos2::new(
                        hdr_rect.right() - btn_line_size.x - x_offset,
                        hdr_rect.center().y - btn_line_size.y / 2.0,
                    ),
                    btn_line_size,
                );
                let btn_star = Button::new(
                    RichText::new(format!("*({}", cur_ln + 1))
                        .color(Color32::WHITE)
                        .strong()
                        .monospace(),
                )
                .fill(Color32::from_rgb(40, 90, 55))
                .min_size(btn_line_size);
                if ui.put(btn_star_rect, btn_star).clicked() {
                    apply_clicked_line = Some(cur_ln);
                }
                x_offset += btn_line_size.x + 4.0;
                if let Some((lo, hi)) = app.right_selection {
                    let sel_btn_size = Vec2::new(120.0, row_h - 4.0);
                    let sel_btn_rect = Rect::from_min_size(
                        Pos2::new(
                            hdr_rect.right() - sel_btn_size.x - x_offset,
                            hdr_rect.center().y - sel_btn_size.y / 2.0,
                        ),
                        sel_btn_size,
                    );
                    let sel_btn = Button::new(
                        RichText::new(format!("⚡Apply {}-{}", lo + 1, hi + 1))
                            .color(Color32::WHITE)
                            .strong()
                            .small()
                            .monospace(),
                    )
                    .fill(Color32::from_rgb(70, 45, 100));
                    if ui.put(sel_btn_rect, sel_btn).clicked() {
                        apply_selection = Some((cur_ln, (lo, hi)));
                    }
                }
                for (line_idx, line) in hunk.replace.iter().enumerate() {
                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
                    let lnum_x = rect.left() + 4.0;
                    let prefix_x = lnum_x + lnum_w + 6.0;
                    let text_x = prefix_x + 2.0 * char_w;
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
                                app.set_message(StatusMessage::info(format!(
                                    "🔍 Searched '{}': {} matches. Press n/N to cycle.",
                                    q,
                                    app.search_matches.len()
                                )));
                            } else {
                                app.search_matches.clear();
                                app.set_message(StatusMessage::warning(format!(
                                    "No matches found for '{}'",
                                    q
                                )));
                            }
                        }
                    }
                    if primary_down {
                        if let Some(pos) = pointer_pos {
                            if rect.contains(pos) {
                                if app.right_drag_anchor.is_none() {
                                    app.right_drag_anchor = Some(line_idx);
                                }
                                let anchor = app.right_drag_anchor.unwrap();
                                let lo = anchor.min(line_idx);
                                let hi = anchor.max(line_idx);
                                app.right_selection = Some((lo, hi));
                            }
                        }
                    }
                    let is_replace_selected = app
                        .right_selection
                        .map_or(false, |(s, e)| line_idx >= s && line_idx <= e);
                    let replace_bg = if is_replace_selected {
                        Color32::from_rgb(55, 40, 85)
                    } else if !app.file_anchors.is_empty() {
                        Color32::from_rgba_premultiplied(45, 38, 15, 60)
                    } else {
                        pal::BG_INSERT
                    };
                    ui.painter().rect_filled(rect, 0.0, replace_bg);
                    let bar = Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height()));
                    ui.painter().rect_filled(
                        bar,
                        0.0,
                        if is_replace_selected {
                            Color32::from_rgb(140, 100, 220)
                        } else if !app.file_anchors.is_empty() {
                            pal::BAR_ANCHOR
                        } else {
                            pal::BAR_MATCH
                        },
                    );
                    ui.painter().text(
                        Pos2::new(lnum_x, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("{:>4}", line_idx + 1),
                        row_font.clone(),
                        pal::TEXT_DIM,
                    );
                    ui.painter().text(
                        Pos2::new(prefix_x, rect.center().y),
                        Align2::LEFT_CENTER,
                        "+ ",
                        row_font.clone(),
                        pal::TEXT_INSERT,
                    );
                    let display = MergeApp::truncate_owned(line, max_chars);
                    ui.painter().text(
                        Pos2::new(text_x, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        row_font.clone(),
                        if is_applied {
                            pal::TEXT_DIM
                        } else if !app.file_anchors.is_empty() {
                            pal::TEXT_ANCHOR
                        } else {
                            Color32::from_rgb(155, 235, 165)
                        },
                    );
                }
                if !primary_down {
                    app.right_drag_anchor = None;
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
    if let Some((target_line, range)) = apply_selection {
        app.apply_merge_partial(Some(target_line), None, range);
        app.right_selection = None;
    }
}