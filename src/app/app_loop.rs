use super::chat::ChatMode;
use super::clipboard_utils::get_clipboard_text;
use super::llm::LlmResponse;
use super::palette::pal;
use super::state::{MarkPending, MergeApp, PendingSync};
use super::types::{FileAnchor, StatusMessage};
use eframe::egui::*;

impl eframe::App for MergeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.tick_impl_workflow(ctx);
        if ctx.input(|i| i.key_pressed(Key::Q) && i.modifiers.alt) {
            self.quit_requested = true;
        }
        if ctx.input(|i| i.key_pressed(Key::W) && i.modifiers.alt && i.key_down(Key::Tab)) {
            self.show_commit_prompt = true;
        }
        if self.quit_requested {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            self.quit_requested = false;
        }

        if let Ok(res) = self.repo_receiver.try_recv() {
            match res {
                Ok(repos) => {
                    if let Some(id) = &self.active_repo_id {
                        if let Some(repo) = repos.iter().find(|r| &r.id == id) {
                            self.base_dir = repo.source_path.clone();
                            self.start_pwd = repo.source_path.clone();
                            self.start_pwd_is_repo = true;
                            self.git_log_entries =
                                super::git_ops::get_git_log(std::path::Path::new(&self.base_dir));
                        }
                    }
                    self.available_repos = repos;
                    self.daemon_error = None;
                }
                Err(e) => {
                    self.daemon_error = Some(e);
                }
            }
        }

        if self.is_llm_loading && self.is_llm_for_commit {
            if let Some(receiver) = self.llm_response_receiver.take() {
                let mut done = false;
                while let Ok(response) = receiver.try_recv() {
                    match response {
                        LlmResponse::Text(text) => {
                            self.commit_message = text;
                            self.set_message(StatusMessage::info("AI commit message generated."));
                        }
                        LlmResponse::Error(err) => {
                            self.set_message(StatusMessage::error(format!(
                                "AI Commit failed: {}",
                                err
                            )));
                            done = true;
                        }
                        LlmResponse::Done => {
                            done = true;
                        }
                    }
                }
                if !done {
                    self.llm_response_receiver = Some(receiver);
                } else {
                    self.is_llm_loading = false;
                    self.is_llm_for_commit = false;
                }
            }
        }
        if self.message.is_some() {
            if self.message_until.is_none() {
                self.message_until = Some(ctx.input(|i| i.time) + 6.0);
            }
            if let Some(until) = self.message_until {
                if ctx.input(|i| i.time) > until {
                    self.message = None;
                    self.message_until = None;
                }
            }
        }

        if ctx.input(|i| i.key_pressed(Key::F2)) {
            self.show_fmt_error = false;
            self.show_settings = false;
            self.show_repos_window = false;
            self.show_debug = false;
            self.show_git_status_window = false;
            self.show_git_diff_side = false;
            self.show_git_log_window = false;
            self.show_git_diff_window = !self.show_git_diff_window;
        }
        if ctx.input(|i| i.key_pressed(Key::F3)) {
            self.show_fmt_error = false;
            self.show_settings = false;
            self.show_repos_window = false;
            self.show_debug = false;
            self.show_git_status_window = false;
            self.show_git_diff_window = false;
            self.show_git_log_window = false;
            self.show_git_diff_side = !self.show_git_diff_side;
            if self.show_git_diff_side {
                self.refresh_git_changed_files();
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F10)) {
            self.show_fmt_error = false;
            self.show_settings = false;
            self.show_repos_window = false;
            self.show_git_status_window = false;
            self.show_git_diff_window = false;
            self.show_git_diff_side = false;
            self.show_git_log_window = false;
            self.show_debug = !self.show_debug;
        }
        if ctx.input(|i| i.key_pressed(Key::F1)) {
            self.show_fmt_error = false;
            self.show_settings = false;
            self.show_repos_window = false;
            self.show_debug = false;
            self.show_git_diff_window = false;
            self.show_git_diff_side = false;
            self.show_git_log_window = false;
            self.show_git_commit_window = false;
            self.show_git_status_window = !self.show_git_status_window;
        }
        if ctx.input(|i| i.key_pressed(Key::F4)) {
            self.show_fmt_error = false;
            self.show_settings = false;
            self.show_repos_window = false;
            self.show_debug = false;
            self.show_git_diff_window = false;
            self.show_git_diff_side = false;
            self.show_git_status_window = false;
            self.show_git_log_window = !self.show_git_log_window;
            self.show_chat_window = false;
            if self.show_git_log_window {
                self.git_log_entries =
                    super::git_ops::get_git_log(std::path::Path::new(&self.base_dir));
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F9)) {
            self.show_fmt_error = false;
            self.show_settings = false;
            self.show_repos_window = false;
            self.show_debug = false;
            self.show_git_diff_window = false;
            self.show_git_diff_side = false;
            self.show_git_status_window = false;
            self.show_git_log_window = false;
            self.show_git_commit_window = false;
            self.show_chat_window = !self.show_chat_window;
        }
        if !ctx.wants_keyboard_input() || self.is_searching {
            ctx.input(|i| {
                if i.key_pressed(Key::Escape) {
                    if self.show_repos_window {
                        self.show_repos_window = false;
                    } else if self.show_settings {
                        self.show_settings = false;
                    } else if self.show_help {
                        self.show_help = false;
                    } else if self.show_debug {
                        self.show_debug = false;
                    } else if self.show_git_diff_window {
                        self.show_git_diff_window = false;
                    } else if self.show_git_status_window {
                        self.show_git_status_window = false;
                    } else if self.show_git_log_window {
                        self.show_git_log_window = false;
                    } else if self.show_git_commit_window {
                        self.show_git_commit_window = false;
                    } else if self.show_chat_window {
                        self.show_chat_window = false;
                    } else if self.is_searching {
                        self.is_searching = false;
                        self.file_search_query.clear();
                        self.search_matches.clear();
                        self.scroll_to_match = true;
                    } else if self.pending_sync.is_some() {
                        self.pending_sync = None;
                    } else if !self.file_anchors.is_empty() || self.mark_pending.is_some() {
                        self.clear_marks();
                    } else if self.file_drag_selection.is_some() {
                        self.file_drag_selection = None;
                        self.file_drag_anchor = None;
                    } else if self.right_selection.is_some() {
                        self.right_selection = None;
                        self.right_drag_anchor = None;
                    } else if self.left_selection.is_some() {
                        self.left_selection = None;
                    } else if self.del_start.is_some() || self.del_end.is_some() {
                        self.del_start = None;
                        self.del_end = None;
                    }
                }
                if !self.is_searching
                    && i.events
                        .iter()
                        .any(|e| matches!(e, Event::Text(t) if t == "?"))
                {
                    self.show_help = !self.show_help;
                }
                if !self.is_searching
                    && i.events
                        .iter()
                        .any(|e| matches!(e, Event::Text(t) if t == "*"))
                {
                    if let Some(text) = get_clipboard_text() {
                        self.patch_text = text;
                        self.reparse();
                    } else {
                        self.show_manual_paste = true;
                        self.set_message(StatusMessage::warning(
                            "Clipboard is empty or inaccessible. Use manual paste window.",
                        ));
                    }
                }
            });
        }

        if self.is_insert_mode {
            TopBottomPanel::bottom("insert_hud")
                .frame(
                    Frame::none()
                        .fill(Color32::from_rgb(25, 30, 45))
                        .inner_margin(Margin::symmetric(8.0, 4.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("-- INSERT --")
                                .color(pal::ACCENT_INFO)
                                .strong()
                                .monospace(),
                        );
                        ui.label(
                            RichText::new("ESC to exit · Type to insert · Backspace to delete")
                                .color(pal::TEXT_DIM)
                                .small(),
                        );
                    });
                });
        }
        if self.is_searching {
            TopBottomPanel::bottom("vim_search_prompt")
                .frame(
                    Frame::none()
                        .fill(pal::BG_TOOLBAR)
                        .inner_margin(Margin::symmetric(8.0, 4.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.label(
                            RichText::new("/")
                                .color(pal::ACCENT_WARN)
                                .strong()
                                .monospace(),
                        );
                        ui.label(
                            RichText::new(&self.file_search_query)
                                .color(pal::TEXT_NORMAL)
                                .monospace(),
                        );
                        let blink = (ctx.input(|i| i.time) * 2.0).floor() as i64 % 2 == 0;
                        ui.label(
                            RichText::new(if blink { "█" } else { " " })
                                .color(pal::TEXT_NORMAL)
                                .monospace(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.label(
                                RichText::new("ENTER search · ESC cancel")
                                    .color(pal::TEXT_DIM)
                                    .small(),
                            );
                        });
                    });
                });
        }

        if self.mark_pending.is_some() {
            TopBottomPanel::bottom("mark_hud")
                .frame(
                    Frame::none()
                        .fill(Color32::from_rgb(40, 32, 10))
                        .inner_margin(Margin::symmetric(10.0, 4.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("m")
                                .color(pal::TEXT_ANCHOR)
                                .strong()
                                .monospace(),
                        );
                        ui.label(
                            RichText::new(
                                "→ press any letter (a, b, c...) to set marker  ·  ESC cancel",
                            )
                            .color(pal::TEXT_NORMAL)
                            .monospace()
                            .small(),
                        );
                        if !self.file_anchors.is_empty() {
                            let labels: Vec<String> =
                                self.file_anchors.values().map(|f| f.label()).collect();
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(labels.join("  "))
                                        .color(pal::TEXT_ANCHOR)
                                        .monospace()
                                        .small(),
                                );
                            });
                        }
                    });
                });
        }

        if self.pending_sync.is_some() {
            TopBottomPanel::bottom("sync_hud")
                .frame(
                    Frame::none()
                        .fill(Color32::from_rgb(35, 30, 15))
                        .inner_margin(Margin::symmetric(10.0, 4.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let text = match &self.pending_sync {
                            Some(PendingSync::WaitingRight { left_line }) => {
                                format!(
                                    "Sync L@{} \u{2192} click file line or press S \u{00b7} ESC cancel",
                                    left_line + 1
                                )
                            }
                            Some(PendingSync::WaitingLeft { right_line }) => {
                                format!(
                                    "Sync R@{} \u{2192} click search line \u{00b7} ESC cancel",
                                    right_line + 1
                                )
                            }
                            None => String::new(),
                        };
                        ui.label(
                            RichText::new(text)
                                .color(Color32::from_rgb(255, 200, 80))
                                .monospace()
                                .small(),
                        );
                    });
                });
        }

        if self.del_start.is_some() || self.del_end.is_some() {
            TopBottomPanel::bottom("block_delete_hud")
                .frame(
                    Frame::none()
                        .fill(Color32::from_rgb(45, 20, 20))
                        .inner_margin(Margin::symmetric(10.0, 4.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let mut msg = "Block Delete:".to_string();
                        if let Some(start) = self.del_start {
                            msg.push_str(&format!(" Start @ Line {}", start + 1));
                        }
                        if let Some(end) = self.del_end {
                            msg.push_str(&format!(" End @ Line {}", end + 1));
                        }
                        ui.label(
                            RichText::new(&msg)
                                .color(pal::TEXT_DELETE)
                                .strong()
                                .monospace()
                                .small(),
                        );
                        if self.del_start.is_some() && self.del_end.is_some() {
                            let start = self.del_start.unwrap();
                            let end = self.del_end.unwrap();
                            let min = start.min(end);
                            let max = start.max(end);
                            let count = max - min + 1;
                            ui.add(Separator::default().vertical());
                            let btn = Button::new(
                                RichText::new(format!("Delete block ({} lines)", count))
                                    .color(Color32::WHITE)
                                    .strong()
                                    .small()
                                    .monospace(),
                            )
                            .fill(Color32::from_rgb(120, 40, 40));
                            if ui.add(btn).clicked() {
                                self.delete_block_range(min, max);
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Clear block selection").clicked() {
                                self.del_start = None;
                                self.del_end = None;
                            }
                        });
                    });
                });
        }
        if let Some((s, e)) = self.file_drag_selection {
            let (lo, hi) = (s.min(e), s.max(e));
            TopBottomPanel::bottom("file_drag_hud")
                .frame(
                    Frame::none()
                        .fill(Color32::from_rgb(32, 24, 48))
                        .inner_margin(Margin::symmetric(10.0, 4.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Drag selection: lines {}–{} ({} lines)",
                                lo + 1,
                                hi + 1,
                                hi - lo + 1
                            ))
                            .color(Color32::from_rgb(190, 160, 255))
                            .strong()
                            .monospace()
                            .small(),
                        );
                        ui.add(Separator::default().vertical());
                        let del_btn = Button::new(
                            RichText::new("🗑 Delete block")
                                .color(Color32::WHITE)
                                .strong()
                                .small()
                                .monospace(),
                        )
                        .fill(Color32::from_rgb(120, 40, 40));
                        if ui.add(del_btn).clicked() {
                            self.delete_block_range(lo, hi);
                            self.file_drag_selection = None;
                            self.file_drag_anchor = None;
                        }
                        let anchor_btn = Button::new(
                            RichText::new("⚓ Set anchor ma")
                                .color(Color32::WHITE)
                                .strong()
                                .small()
                                .monospace(),
                        )
                        .fill(Color32::from_rgb(90, 70, 15));
                        if ui.add(anchor_btn).clicked() {
                            self.file_anchors.insert(
                                'a',
                                FileAnchor {
                                    id: 'a',
                                    line: lo,
                                    end_line: Some(hi),
                                },
                            );
                            self.scroll_to_match = true;
                            self.set_message(StatusMessage::info(format!(
                                "⚓ ma set at lines {}-{}",
                                lo + 1,
                                hi + 1
                            )));
                            self.file_drag_selection = None;
                            self.file_drag_anchor = None;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("✕ Clear").clicked() {
                                self.file_drag_selection = None;
                                self.file_drag_anchor = None;
                            }
                        });
                    });
                });
        }
        if self.show_commit_prompt {
            let mut show_prompt = self.show_commit_prompt;
            Window::new("Git Commit")
                .open(&mut show_prompt)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Commit message:");
                    ui.text_edit_multiline(&mut self.commit_message);
                    ui.horizontal(|ui| {
                        if ui.button("Commit (c)").clicked() {
                            self.commit_changes();
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_commit_prompt = false;
                        }
                    });
                });
            self.show_commit_prompt = show_prompt;
        }
        super::toolbar::render_toolbar(self, ctx);
        super::status_bar::render_status_bar(self, ctx);
        CentralPanel::default().show(ctx, |ui| {
            super::split_view::render_split_view(self, ui);
        });
        if self.show_help {
            super::help::render_help_overlay(self, ctx);
        }
    }
}
