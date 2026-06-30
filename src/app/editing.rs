// file:///src/app/editing.rs
use super::matching::MergeMatching;
use super::state::MergeApp;
use super::types::{Action, StatusMessage};

impl MergeApp {
    pub fn apply_merge(&mut self, forced_line: Option<usize>, anchor_id: Option<char>) {
        if self.applied_hunks.contains(&self.current_hunk) {
            self.set_message(StatusMessage::warning(format!(
                "Hunk {} already applied",
                self.current_hunk + 1
            )));
            return;
        }
        let hunk = match self.hunks.get(self.current_hunk) {
            Some(h) => h.clone(),
            None => return,
        };

        let (file_start, file_end) = if let Some(id) = anchor_id {
            if let Some(anchor) = self.file_anchors.get(&id) {
                let start = anchor.line;
                let end = if id == 'a' {
                    // For 'a', default to auto-match end if no explicit end_line is set
                    anchor.end_line.unwrap_or(
                        self.match_result
                            .as_ref()
                            .map(|mr| mr.file_end.saturating_sub(1))
                            .unwrap_or(start),
                    )
                } else {
                    anchor.end_line.unwrap_or(start)
                };
                println!(
                    "[DEBUG apply_merge] Anchor {} found: start={}, end={}. Returning ({}, {})",
                    id,
                    start,
                    end,
                    start,
                    end + 1
                );
                (start, end + 1)
            } else {
                self.set_message(StatusMessage::error(format!("Marker {} not found", id)));
                return;
            }
        } else if let Some(ln) = forced_line {
            println!(
                "[DEBUG apply_merge] Forced line found: ({}, {})",
                ln,
                ln + 1
            );
            (ln, ln + 1)
        } else {
            match self.resolve_apply_range() {
                Some(r) => r,
                None => {
                    println!("[DEBUG apply_merge] resolve_apply_range returned None. Aborting.");
                    return;
                }
            }
        };

        println!(
            "[DEBUG apply_merge] Final range to replace: file_start={}, file_end={}",
            file_start, file_end
        );
        println!(
            "[DEBUG apply_merge] File lines len={}, replacing {} lines with {} lines",
            self.file_lines.len(),
            file_end - file_start,
            hunk.replace.len()
        );

        if let Some((ms, me)) = self.merged_range {
            if file_start < me && file_end > ms {
                self.set_message(StatusMessage::warning(
                    "⚠ This hunk overlaps an already-merged region — applied anyway, check result",
                ));
            }
        }

        self.history
            .push((self.file_lines.clone(), self.current_hunk));
        let mut output: Vec<String> = Vec::new();
        output.extend_from_slice(&self.file_lines[..file_start]);
        let replace_start = output.len();
        output.extend(hunk.replace.iter().cloned());
        let replace_end = output.len();
        output.extend_from_slice(&self.file_lines[file_end..]);
        self.file_lines = output;
        self.merged_range = Some((replace_start, replace_end));
        self.applied_hunks.insert(self.current_hunk);

        if let Some(id) = anchor_id {
            self.file_anchors.remove(&id);
        } else if forced_line.is_none() {
            self.file_anchors.clear();
        }

        self.mark_pending = None;
        self.cursor_line = Some(replace_start);
        self.scroll_to_match = true;
        self.set_message(StatusMessage::success(format!(
            "✓ Hunk {} applied at line {} — {} line(s) replaced with {}",
            self.current_hunk + 1,
            file_start + 1,
            file_end - file_start,
            hunk.replace.len(),
        )));
        self.recompute_match();
        self.update_git_statuses(); // Update Git gutter
    }

    pub fn advance_to_next_unapplied(&mut self) {
        let start = self.current_hunk;
        let n = self.hunks.len();
        for offset in 1..n {
            let idx = (start + offset) % n;
            if !self.applied_hunks.contains(&idx) {
                self.current_hunk = idx;
                self.load_hunk();
                return;
            }
        }
        self.set_message(StatusMessage::success("All hunks applied!"));
    }

    pub fn undo(&mut self) {
        if let Some((prev_lines, hunk_idx)) = self.history.pop() {
            self.file_lines = prev_lines;
            self.applied_hunks.remove(&hunk_idx);
            self.merged_range = None;
            self.scroll_to_match = true;
            self.set_message(StatusMessage::info(format!(
                "Undone — hunk {} unapplied",
                hunk_idx + 1
            )));
            self.recompute_match();
            self.update_git_statuses(); // Update Git gutter
        } else {
            self.set_message(StatusMessage::warning("Nothing to undo"));
        }
    }

    pub fn delete_lines(&mut self, count: usize) {
        if let Some(start) = self.cursor_line {
            if start < self.file_lines.len() {
                self.history
                    .push((self.file_lines.clone(), self.current_hunk));
                let end = (start + count).min(self.file_lines.len());
                self.file_lines.drain(start..end);
                self.merged_range = None;
                self.recompute_match();
                self.update_git_statuses(); // Update Git gutter
                let new_len = self.file_lines.len();
                if new_len == 0 {
                    self.cursor_line = None;
                } else if start >= new_len {
                    self.cursor_line = Some(new_len - 1);
                } else {
                    self.cursor_line = Some(start);
                }
                self.scroll_to_match = true;
                self.set_message(StatusMessage::info(format!(
                    "Deleted {} line(s)",
                    end - start
                )));
            }
        }
    }

    pub fn save_file(&mut self) {
        let content = self.file_lines.join("\n");
        let path = if self.file_path.is_empty() {
            "merged_output.txt".to_string()
        } else {
            self.file_path.clone()
        };
        match std::fs::write(&path, &content) {
            Ok(_) => {
                if self.format_on_save {
                    let cmd = if self.fmt_command.is_empty() {
                        "rustfmt".to_string()
                    } else {
                        self.fmt_command.clone()
                    };
                    let parts: Vec<&str> = cmd.split_whitespace().collect();
                    if !parts.is_empty() {
                        let mut command = std::process::Command::new(parts[0]);
                        for arg in &parts[1..] {
                            command.arg(arg);
                        }
                        command.arg(&path);

                        match command.output() {
                            Ok(output) => {
                                if output.status.success() {
                                    self.fmt_error = None;
                                    if let Ok(formatted_content) = std::fs::read_to_string(&path) {
                                        self.file_text = formatted_content.clone();
                                        self.file_lines =
                                            self.file_text.lines().map(String::from).collect();
                                        self.recompute_match();
                                        self.update_git_statuses();
                                    }
                                    self.save_file_state();
                                    self.set_message(StatusMessage::success(format!(
                                        "Saved & formatted → {}",
                                        path
                                    )));
                                } else {
                                    let err = String::from_utf8_lossy(&output.stderr).to_string();
                                    self.fmt_error = Some(err);
                                    self.show_fmt_error = true;
                                    self.set_message(StatusMessage::error(
                                        "Format failed. See error window.",
                                    ));
                                }
                            }
                            Err(e) => {
                                self.fmt_error = Some(format!("Failed to execute command: {}", e));
                                self.set_message(StatusMessage::error(format!(
                                    "Save ok, but failed to run fmt: {}",
                                    e
                                )));
                            }
                        }
                        return;
                    }
                }
                self.save_file_state();
                self.set_message(StatusMessage::success(format!("Saved → {}", path)));
            }
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Save failed: {}", e)));
            }
        }
    }

    pub fn save_all_files(&mut self) {
        self.save_file_state();
        let mut saved = 0usize;
        let mut failed = 0usize;
        for (path, state) in &self.file_states {
            if path.is_empty() || state.applied_hunks.is_empty() {
                continue;
            }
            let content = state.lines.join("\n");
            match std::fs::write(path, &content) {
                Ok(_) => saved += 1,
                Err(_) => failed += 1,
            }
        }
        if failed == 0 {
            self.set_message(StatusMessage::success(format!("Saved {} file(s)", saved)));
        } else {
            self.set_message(StatusMessage::error(format!(
                "Saved {}, {} failed",
                saved, failed
            )));
        }
    }

    // In src/app/editing.rs
    pub fn delete_function_around_cursor(&mut self) {
        let cursor = match self.cursor_line {
            Some(line) => line,
            None => return,
        };

        // 1. Scan backwards to find the nearest function signature line
        let mut fn_start_line = None;
        for i in (0..=cursor).rev() {
            if self.is_fn_line(&self.file_lines[i]) {
                // Find any immediately preceding attributes or doc comments to delete as well
                let mut real_start = i;
                while real_start > 0 {
                    let prev = self.file_lines[real_start - 1].trim();
                    if prev.starts_with("#[") || prev.starts_with("///") || prev.starts_with("//!")
                    {
                        real_start -= 1;
                    } else {
                        break;
                    }
                }
                fn_start_line = Some((i, real_start));
                break;
            }
        }

        // 2. Scan forwards from the signature to balance the curly braces {}
        let mut fn_end_line = None;
        if let Some((sig_line, _)) = fn_start_line {
            let mut balance = 0;
            let mut found_open = false;
            for j in sig_line..self.file_lines.len() {
                let line = &self.file_lines[j];
                let mut in_string = false;
                let mut in_char = false;
                let mut chars = line.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '/' && chars.peek() == Some(&'/') {
                        // Ignore remaining characters in comment line
                        break;
                    }
                    if c == '"' && !in_char {
                        in_string = !in_string;
                    } else if c == '\'' && !in_string {
                        in_char = !in_char;
                    } else if !in_string && !in_char {
                        if c == '{' {
                            if !found_open {
                                found_open = true;
                            }
                            balance += 1;
                        } else if c == '}' {
                            if found_open {
                                balance -= 1;
                                if balance == 0 {
                                    fn_end_line = Some(j);
                                    break;
                                }
                            }
                        }
                    }
                }
                if fn_end_line.is_some() {
                    break;
                }
            }
        }

        // 3. Perform the deletion if a valid function enclosing the cursor was parsed
        let mut target_range = None;
        if let Some((_, start)) = fn_start_line {
            if let Some(end) = fn_end_line {
                if cursor >= start && cursor <= end {
                    target_range = Some((start, end));
                }
            }
        }

        if let Some((start, end)) = target_range {
            self.history
                .push((self.file_lines.clone(), self.current_hunk));
            self.file_lines.drain(start..=end);
            self.recompute_match();
            self.update_git_statuses();
            self.cursor_line = Some(start.min(self.file_lines.len().saturating_sub(1)));
            self.last_action = Some(Action::DeleteFunction); // Added here to register as repeatable action
            self.set_message(StatusMessage::success("Function block deleted"));
        } else {
            self.set_message(StatusMessage::warning("No function found around cursor"));
        }
    }

    pub fn delete_block_range(&mut self, min: usize, max: usize) {
        if min < self.file_lines.len() && max < self.file_lines.len() {
            self.history
                .push((self.file_lines.clone(), self.current_hunk));
            let count = max - min + 1;
            self.file_lines.drain(min..=max);
            self.merged_range = None;
            self.del_start = None;
            self.del_end = None;
            self.recompute_match();
            self.update_git_statuses();
            let new_len = self.file_lines.len();
            if new_len == 0 {
                self.cursor_line = None;
            } else if min >= new_len {
                self.cursor_line = Some(new_len - 1);
            } else {
                self.cursor_line = Some(min);
            }
            self.scroll_to_match = true;
            self.set_message(StatusMessage::success(format!(
                "Deleted block of {} line(s) (lines {} to {})",
                count,
                min + 1,
                max + 1
            )));
        }
    }

    /// Checks if a line contains a function signature declaration
    fn is_fn_line(&self, line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
            return false;
        }

        let bytes = trimmed.as_bytes();
        for i in 0..bytes.len().saturating_sub(1) {
            if &bytes[i..i + 2] == b"fn" {
                let ok_before = if i == 0 {
                    true
                } else {
                    let c = bytes[i - 1];
                    c.is_ascii_whitespace() || c == b')' || c == b']'
                };

                let ok_after = if i + 2 >= bytes.len() {
                    true
                } else {
                    let c = bytes[i + 2];
                    c.is_ascii_whitespace() || c == b'<' || c == b'(' || c == b'{'
                };

                if ok_before && ok_after {
                    return true;
                }
            }
        }
        false
    }
}
