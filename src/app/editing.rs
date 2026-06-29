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
                (anchor.line, anchor.line)
            } else {
                self.set_message(StatusMessage::error(format!("Marker {} not found", id)));
                return;
            }
        } else if let Some(ln) = forced_line {
            (ln, ln)
        } else {
            match self.resolve_apply_range() {
                Some(r) => r,
                None => return,
            }
        };

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
}
