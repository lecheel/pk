use super::chat::ChatMode;
use super::config::AppConfig;
use super::matching::MergeMatching;
use super::state::{LineAction, LineActionKind, MergeApp};
use super::types::{FileAnchor, StatusMessage};
use crate::patch::PatchHunk;
use std::collections::HashSet;

impl MergeApp {
    pub fn refresh_git_changed_files(&mut self) {
        let repo_root = std::path::Path::new(&self.base_dir);
        let repo = match git2::Repository::discover(repo_root) {
            Ok(r) => r,
            Err(_) => {
                self.git_changed_files.clear();
                self.git_changed_file_idx = 0;
                return;
            }
        };
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(false);
        let mut files: Vec<String> = repo
            .statuses(Some(&mut opts))
            .map(|statuses| {
                statuses
                    .iter()
                    .filter_map(|e| {
                        let s = e.status();
                        if s.is_index_new() || s.is_wt_new() {
                            None
                        } else {
                            e.path().map(|p| p.to_string())
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files.dedup();
        self.git_changed_files = files;
        if let Some(workdir) = repo.workdir() {
            let cur = std::path::Path::new(&self.file_path);
            if let Ok(rel) = cur.strip_prefix(workdir) {
                let rel_str = rel.display().to_string().replace('\\', "/");
                if let Some(idx) = self.git_changed_files.iter().position(|f| *f == rel_str) {
                    self.git_changed_file_idx = idx;
                    return;
                }
            }
        }
        if self.git_changed_file_idx >= self.git_changed_files.len() {
            self.git_changed_file_idx = 0;
        }
    }

    pub fn is_hunk_match_ok(&self, hunk_idx: usize) -> bool {
        if let Some(hunk) = self.hunks.get(hunk_idx) {
            if hunk.search.is_empty() {
                return true;
            }
            let current_file_name = self
                .current_hunk()
                .map(|h| h.filename.clone())
                .unwrap_or_default();
            if hunk.filename != current_file_name {
                return true;
            }
            if self.file_lines.is_empty() {
                return true;
            }
            let best =
                crate::diff::find_best_match(&hunk.search, &self.file_lines, self.ignore_comments);
            best.score >= self.min_match_score
        } else {
            false
        }
    }

    pub fn save_history(&mut self) {
        self.history
            .push((self.file_lines.clone(), self.cursor_line.unwrap_or(0)));
    }

    pub fn ensure_valid_filtered_hunk(&mut self) {
        if !self.filter_low_matches {
            return;
        }
        if self.hunks.is_empty() {
            return;
        }
        if !self.is_hunk_match_ok(self.current_hunk) {
            let mut found = None;
            for i in self.current_hunk..self.hunks.len() {
                if self.is_hunk_match_ok(i) {
                    found = Some(i);
                    break;
                }
            }
            if found.is_none() {
                for i in 0..self.current_hunk {
                    if self.is_hunk_match_ok(i) {
                        found = Some(i);
                        break;
                    }
                }
            }
            if let Some(idx) = found {
                self.current_hunk = idx;
                self.load_hunk();
            } else {
                self.set_message(StatusMessage::warning(format!(
                    "No hunks have match score >= {:.0}%",
                    self.min_match_score
                )));
            }
        }
    }

    pub fn set_message(&mut self, msg: StatusMessage) {
        self.message = Some(msg);
        self.message_until = None;
    }

    pub fn set_mark(&mut self, id: char, line: usize) {
        self.file_anchors.insert(
            id,
            FileAnchor {
                id,
                line,
                end_line: None,
            },
        );
        self.scroll_to_match = true;
        self.set_message(StatusMessage::info(format!(
            "⚓ m{} set at line {} — press >{} to apply",
            id,
            line + 1,
            id
        )));
    }

    pub fn set_mark_a(&mut self, line: usize) {
        if let Some(anchor) = self.file_anchors.get(&'a') {
            if let Some(end) = anchor.end_line {
                let s = line.min(end);
                let e = line.max(end);
                self.file_anchors.insert(
                    'a',
                    FileAnchor {
                        id: 'a',
                        line: s,
                        end_line: Some(e),
                    },
                );
                self.scroll_to_match = true;
                self.set_message(StatusMessage::info(format!(
                    "⚓ ma range adjusted: lines {}-{}",
                    s + 1,
                    e + 1
                )));
                return;
            }
        }
        self.set_mark('a', line);
    }
    pub fn set_mark_b(&mut self, line: usize) {
        self.set_mark('b', line);
    }
    pub fn set_mark_a_end(&mut self, line: usize) {
        if let Some(anchor) = self.file_anchors.get(&'a') {
            let s = line.min(anchor.line);
            let e = line.max(anchor.line);
            self.file_anchors.insert(
                'a',
                FileAnchor {
                    id: 'a',
                    line: s,
                    end_line: Some(e),
                },
            );
            self.scroll_to_match = true;
            self.set_message(StatusMessage::info(format!(
                "⚓ mA range adjusted: lines {}-{}",
                s + 1,
                e + 1
            )));
        } else {
            self.file_anchors.insert(
                'a',
                FileAnchor {
                    id: 'a',
                    line,
                    end_line: Some(line),
                },
            );
            self.set_message(StatusMessage::info(format!(
                "⚓ mA end set at line {}",
                line + 1
            )));
            self.scroll_to_match = true;
        }
    }

    pub fn clear_marks(&mut self) {
        self.file_anchors.clear();
        self.mark_pending = None;
        self.scroll_to_match = true;
    }

    pub fn resolve_apply_range(&self) -> Option<(usize, usize)> {
        if let Some(hunk) = self.current_hunk() {
            if hunk.search.is_empty() {
                return Some((self.file_lines.len(), self.file_lines.len()));
            }
        }
        if let Some(a) = self.file_anchors.get(&'a') {
            let start = a.file_start();
            let end = if let Some(b) = self.file_anchors.get(&'b') {
                a.file_end().max(b.file_end())
            } else {
                a.end_line.map_or(
                    self.match_result
                        .as_ref()
                        .map(|mr| mr.file_end.saturating_sub(1))
                        .unwrap_or(start),
                    |_| a.file_end(),
                )
            };
            return Some((start, end + 1));
        }
        if let Some(mr) = self.match_result.as_ref() {
            println!(
                "[DEBUG resolve_apply_range] No anchors. Using match_result: ({}, {})",
                mr.file_start, mr.file_end
            );
            return Some((mr.file_start, mr.file_end));
        }
        println!("[DEBUG resolve_apply_range] No match and no anchors. Returning None.");
        None
    }

    pub fn toggle_line_removal(&mut self, line_idx: usize) {
        if let Some(pos) = self
            .pending_line_actions
            .iter()
            .position(|a| a.line_idx == line_idx)
        {
            self.pending_line_actions.remove(pos);
        } else {
            self.pending_line_actions.push(LineAction {
                line_idx,
                kind: LineActionKind::Remove,
            });
        }
    }

    pub fn is_pending_remove(&self, line_idx: usize) -> bool {
        self.pending_line_actions
            .iter()
            .any(|a| a.line_idx == line_idx)
    }

    pub fn apply_line_removals(&mut self) {
        if self.pending_line_actions.is_empty() {
            return;
        }
        self.save_history();
        let indices: HashSet<usize> = self
            .pending_line_actions
            .iter()
            .map(|a| a.line_idx)
            .collect();
        self.file_lines = self
            .file_lines
            .iter()
            .enumerate()
            .filter(|(i, _)| !indices.contains(i))
            .map(|(_, l)| l.clone())
            .collect();
        let count = self.pending_line_actions.len();
        self.pending_line_actions.clear();
        self.recompute_match();
        self.scroll_to_match = true;
        self.set_message(StatusMessage::success(format!("Removed {} lines", count)));
    }

    pub fn update_git_statuses(&mut self) {
        let repo_root = std::path::Path::new(&self.base_dir);
        let file_path = std::path::Path::new(&self.file_path);
        let (statuses, diff_rows) = super::git_ops::get_line_statuses(
            repo_root,
            file_path,
            &self.file_lines,
            self.ignore_comments,
        );
        self.git_statuses = statuses;
        self.git_diff_rows = diff_rows;
        self.git_hunks =
            super::git_ops::group_git_hunks(&self.git_diff_rows, self.file_lines.len());
    }

    pub fn refresh_git_diff_side_rows(&mut self) {
        self.update_git_statuses();
    }

    fn diff_side_row_to_insert_after(&self, anchor_row: usize) -> Option<usize> {
        if let Some(n) = self.git_diff_rows.get(anchor_row).and_then(|r| r.right_num) {
            return Some(n - 1);
        }
        self.git_diff_rows[..anchor_row]
            .iter()
            .rev()
            .find_map(|r| r.right_num)
            .map(|n| n - 1)
    }

    pub fn insert_diff_side_selection(&mut self) {
        let (Some((lo, hi)), Some(anchor_row)) =
            (self.diff_side_left_selection, self.diff_side_insert_anchor)
        else {
            return;
        };
        if hi >= self.git_diff_rows.len() {
            return;
        }
        let texts: Vec<String> = self.git_diff_rows[lo..=hi]
            .iter()
            .filter_map(|r| r.left.clone())
            .collect();
        if texts.is_empty() {
            self.set_message(StatusMessage::warning(
                "Nothing to insert in that HEAD selection",
            ));
            return;
        }
        let Some(after_line) = self.diff_side_row_to_insert_after(anchor_row) else {
            self.set_message(StatusMessage::warning(
                "Couldn't resolve insert point — pick a NEW-side line",
            ));
            return;
        };
        self.save_history();
        let pos = (after_line + 1).min(self.file_lines.len());
        let count = texts.len();
        for (i, t) in texts.into_iter().enumerate() {
            self.file_lines.insert(pos + i, t);
        }
        self.diff_side_left_selection = None;
        self.diff_side_insert_anchor = None;
        self.recompute_match();
        self.refresh_git_diff_side_rows();
        self.set_message(StatusMessage::success(format!(
            "Inserted {} HEAD line(s) at line {}",
            count,
            pos + 1
        )));
    }

    pub fn delete_diff_side_selection(&mut self) {
        let Some((lo, hi)) = self.diff_side_right_selection else {
            return;
        };
        if hi >= self.git_diff_rows.len() {
            return;
        }
        let to_delete: HashSet<usize> = self.git_diff_rows[lo..=hi]
            .iter()
            .filter_map(|r| r.right_num)
            .map(|n| n - 1)
            .collect();
        if to_delete.is_empty() {
            self.set_message(StatusMessage::warning(
                "Nothing to delete in that selection",
            ));
            return;
        }
        self.save_history();
        let count = to_delete.len();
        self.file_lines = self
            .file_lines
            .iter()
            .enumerate()
            .filter(|(i, _)| !to_delete.contains(i))
            .map(|(_, l)| l.clone())
            .collect();
        self.diff_side_right_selection = None;
        self.recompute_match();
        self.refresh_git_diff_side_rows();
        self.set_message(StatusMessage::success(format!("Deleted {} line(s)", count)));
    }

    pub fn current_hunk(&self) -> Option<&PatchHunk> {
        self.hunks.get(self.current_hunk)
    }

    pub fn hunk_summary(&self) -> (usize, usize, usize) {
        let applied = self.applied_hunks.len();
        let total = self.hunks.len();
        (applied, total - applied, total)
    }

    pub fn truncate_owned(text: &str, max_chars: usize) -> String {
        if text.chars().count() > max_chars {
            let mut s: String = text.chars().take(max_chars.saturating_sub(1)).collect();
            s.push('…');
            s
        } else {
            text.to_string()
        }
    }

    pub fn current_chat_provider(&self) -> &super::llm::LlmProvider {
        match self.chat_mode {
            ChatMode::Chat => &self.llm_config.chat_provider,
            ChatMode::Commit => &self.llm_config.commit_provider,
            ChatMode::Impl => &self.llm_config.impl_provider,
        }
    }
    pub fn active_system_prompt(&self) -> Option<String> {
        let default = self.chat_mode.system_prompt();
        let custom = match self.chat_mode {
            ChatMode::Chat => &self.llm_config.chat_system_prompt,
            ChatMode::Commit => &self.llm_config.commit_system_prompt,
            ChatMode::Impl => &self.llm_config.impl_system_prompt,
        };
        if custom.is_empty() {
            Some(default)
        } else {
            Some(custom.clone())
        }
    }

    pub fn cancel_llm(&mut self) {
        if self.is_llm_loading {
            self.is_llm_loading = false;
            self.llm_start_time = None;
            self.llm_response_receiver = None;
            self.set_message(StatusMessage::info("LLM request cancelled."));
        }
    }
    pub fn save_config(&self) {
        let config = AppConfig {
            format_on_save: self.format_on_save,
            fmt_command: self.fmt_command.clone(),
            active_repo_id: self.active_repo_id.clone(),
            concat_server_enabled: self.concat_server_enabled,
            ignore_comments: self.ignore_comments,
            min_match_score: self.min_match_score,
            min_match_floor: self.min_match_floor,
            llm_config: self.llm_config.clone(),
            rustconcat_api_url: self.rustconcat_api_url.clone(),
            impl_tools: self.impl_tools.clone(),
        };
        config.save();
    }
    pub fn start_impl_round(&mut self) {
        self.impl_step = 1;
        self.impl_is_running = true;
        self.impl_result_indicator = "⏳".to_string();
        self.impl_skeleton.clear();
        self.impl_files.clear();
        self.impl_hashes.clear();
        self.set_message(StatusMessage::info("Fetching context for Impl..."));
    }
    pub fn tick_impl_workflow(&mut self, ctx: &eframe::egui::Context) {
        if !self.impl_is_running {
            return;
        }
        let base_url = self.rustconcat_api_url.clone();

        match self.impl_step {
            1 => {
                if self.impl_tools.skeleton {
                    let url = format!("{}/skeleton", base_url);
                    if let Ok(resp) = reqwest::blocking::get(&url) {
                        if resp.status().is_success() {
                            self.impl_skeleton = resp.text().unwrap_or_default();
                        }
                    }
                }
                self.impl_step = 2;
                ctx.request_repaint();
            }
            2 => {
                if self.impl_tools.files {
                    let url = format!("{}/files", base_url);
                    if let Ok(resp) = reqwest::blocking::get(&url) {
                        if resp.status().is_success() {
                            self.impl_files = resp.text().unwrap_or_default();
                        }
                    }
                }
                self.impl_step = 3;
                ctx.request_repaint();
            }
            3 => {
                if self.impl_tools.hashes {
                    let url = format!("{}/hashes", base_url);
                    if let Ok(resp) = reqwest::blocking::get(&url) {
                        if resp.status().is_success() {
                            self.impl_hashes = resp.text().unwrap_or_default();
                        }
                    }
                }
                self.impl_step = 4;
                ctx.request_repaint();
            }
            4 => {
                self.impl_is_running = false;
                self.impl_step = 0;
                self.impl_result_indicator = "✅".to_string();
                self.set_message(StatusMessage::success("Context fetched. Ready to implement."));
                ctx.request_repaint();
            }
            _ => {}
        }
    }
    pub fn reset_for_new_file(&mut self) {
        self.applied_hunks.clear();
        self.merged_range = None;
        self.history.clear();
        self.vim_buffer.clear();
        self.d_pending = false;
        self.show_manual_paste = false;
        self.manual_paste_text.clear();
        self.file_anchors.clear();
        self.mark_pending = None;
        self.file_search_query.clear();
        self.search_matches.clear();
        self.search_match_idx = 0;
        self.is_searching = false;
        self.candidate_index = 0;
        self.cursor_line = None;
        self.scroll_to_match = true;
        self.left_selection = None;
        self.right_selection = None;
        self.right_drag_anchor = None;
        self.file_drag_selection = None;
        self.file_drag_anchor = None;
        self.git_statuses.clear();
        self.git_diff_rows.clear();
        self.git_hunks.clear();
        self.show_git_status_window = false;
        self.drag_start_active = false;
        self.drag_end_active = false;
        self.sync_anchors.clear();
        self.pending_sync = None;
        self.pending_line_actions.clear();
        self.del_start = None;
        self.del_end = None;
        self.is_visual_mode = false;
        self.visual_start = None;
        self.is_insert_mode = false;
        self.insert_cursor = 0;
    }

    pub fn apply_merge_partial(
        &mut self,
        target_line: Option<usize>,
        marker: Option<char>,
        range: (usize, usize),
    ) {
        let hunk = match self.current_hunk() {
            Some(h) => h.clone(),
            None => return,
        };
        if hunk.replace.is_empty() {
            return;
        }
        let (lo, hi) = range;
        let hi = hi.min(hunk.replace.len().saturating_sub(1));
        if lo > hi {
            return;
        }
        let sliced: Vec<String> = hunk.replace[lo..=hi].to_vec();
        let insert_at = if let Some(id) = marker {
            self.file_anchors.get(&id).map(|a| a.line)
        } else {
            target_line.or(self.cursor_line)
        };
        let insert_at = match insert_at {
            Some(l) => l,
            None => return,
        };
        self.save_history();
        let pos = (insert_at + 1).min(self.file_lines.len());
        for (offset, line) in sliced.iter().enumerate() {
            self.file_lines.insert(pos + offset, line.clone());
        }
        self.merged_range = Some((pos, pos + sliced.len()));
        self.scroll_to_match = true;
        self.recompute_match();
        self.update_git_statuses();
        self.set_message(StatusMessage::success(format!(
            "Inserted {} selected REPLACE line(s) at line {}",
            sliced.len(),
            pos + 1
        )));
    }
}