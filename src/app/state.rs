use super::clipboard_utils::get_clipboard_text;
use super::config::AppConfig;
use super::daemon::{self, RepoInfo};
use super::git_ops::GitStatus;
use super::matching::MergeMatching;
use super::types::{Action, FileAnchor, FileState, StatusMessage};
use crate::app::pal;
use crate::diff::MatchResult;
use crate::patch::PatchHunk;
use eframe::egui::*;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc;

#[derive(Clone, Debug)]
pub struct SyncAnchor {
    pub id: usize,
    pub left_line: usize,
    pub right_line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PendingSync {
    WaitingRight { left_line: usize },
    WaitingLeft { right_line: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineAction {
    pub line_idx: usize,
    pub kind: LineActionKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LineActionKind {
    Remove,
}

pub struct MergeApp {
    pub quit_requested: bool,
    pub patch_text: String,
    pub hunks: Vec<PatchHunk>,
    pub current_hunk: usize,
    pub file_text: String,
    pub file_lines: Vec<String>,
    pub file_path: String,
    pub base_dir: String,
    pub match_result: Option<MatchResult>,
    pub search_rows: Vec<super::types::SearchRow>,
    pub file_search_query: String,
    pub search_matches: Vec<usize>,
    pub search_match_idx: usize,
    pub is_searching: bool,
    pub candidate_index: usize,
    pub scroll_to_match: bool,
    pub message: Option<StatusMessage>,
    pub message_until: Option<f64>,
    pub cursor_line: Option<usize>,
    pub cursor_col: usize,
    pub applied_hunks: HashSet<usize>,
    pub merged_range: Option<(usize, usize)>,
    pub history: Vec<(Vec<String>, usize)>,
    pub vim_buffer: String,
    pub yanked_line: Option<String>,
    pub d_pending: bool,
    pub last_action: Option<Action>,
    pub show_manual_paste: bool,
    pub manual_paste_text: String,
    pub initial_patch_path: Option<String>,
    pub file_states: HashMap<String, FileState>,
    pub show_help: bool,
    pub show_debug: bool,
    pub start_pwd: String,
    pub start_pwd_is_repo: bool,
    pub left_selection: Option<(usize, usize)>,
    pub right_selection: Option<(usize, usize)>,
    pub right_drag_anchor: Option<usize>,
    pub file_drag_selection: Option<(usize, usize)>,
    pub file_drag_anchor: Option<usize>,
    pub file_anchors: BTreeMap<char, FileAnchor>,
    pub mark_pending: Option<MarkPending>,
    pub git_statuses: Vec<GitStatus>,
    pub git_diff_rows: Vec<crate::diff::DiffRow>,
    pub git_hunks: Vec<super::git_ops::GitDiffHunk>,
    pub git_log_entries: Vec<super::git_ops::GitLogEntry>,
    pub selected_git_log_entry: Option<usize>,
    pub show_git_diff_window: bool,
    pub show_git_diff_side: bool,
    pub show_git_status_window: bool,
    pub show_git_log_window: bool,
    pub show_repos_window: bool,
    pub filter_low_matches: bool,
    pub sync_anchors: Vec<SyncAnchor>,
    pub pending_sync: Option<PendingSync>,
    pub next_sync_id: usize,
    pub pending_line_actions: Vec<LineAction>,
    pub del_start: Option<usize>,
    pub del_end: Option<usize>,
    pub is_visual_mode: bool,
    pub visual_start: Option<usize>,
    pub is_insert_mode: bool,
    pub insert_cursor: usize,
    pub format_on_save: bool,
    pub fmt_command: String,
    pub show_settings: bool,
    pub fmt_error: Option<String>,
    pub show_fmt_error: bool,
    pub drag_start_active: bool,
    pub drag_end_active: bool,
    pub available_repos: Vec<RepoInfo>,
    pub active_repo_id: Option<String>,
    pub daemon_error: Option<String>,
    pub repo_receiver: mpsc::Receiver<Result<Vec<RepoInfo>, String>>,
    pub concat_server_enabled: bool,
    pub ignore_comments: bool,
    pub min_match_score: f32,
    pub min_match_floor: f32,
    pub diff_side_hunk_idx: usize,
    pub diff_side_scroll_target: Option<usize>,
    pub git_changed_files: Vec<String>,
    pub git_changed_file_idx: usize,
    pub git_diff_cursor: Option<usize>,
    pub git_diff_vim_buffer: String,
    pub git_diff_scroll_to_cursor: bool,
    pub git_diff_insert_mode: bool,
    pub diff_side_left_selection: Option<(usize, usize)>,
    pub diff_side_right_selection: Option<(usize, usize)>,
    pub diff_side_left_drag_anchor: Option<usize>,
    pub diff_side_right_drag_anchor: Option<usize>,
    pub diff_side_insert_anchor: Option<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkPending {
    WaitingKey,
}

impl MergeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_patch: Option<String>) -> Self {
        cc.egui_ctx.set_visuals(Visuals::dark());
        let current_pwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let start_repo = git2::Repository::discover(&current_pwd).ok();
        let start_pwd_is_repo = start_repo.is_some();
        let start_pwd = start_repo
            .as_ref()
            .and_then(|r| r.workdir().map(|w| w.display().to_string()))
            .unwrap_or_else(|| current_pwd.clone());

        let config = AppConfig::load();
        let active_repo_id = if config.concat_server_enabled {
            config
                .active_repo_id
                .clone()
                .or_else(daemon::get_active_repo)
        } else {
            None
        };
        let (tx, rx) = mpsc::channel();
        if config.concat_server_enabled {
            std::thread::spawn(move || {
                let res = daemon::fetch_repos();
                let _ = tx.send(res);
            });
        }
        let active_repo_id = config
            .active_repo_id
            .clone()
            .or_else(daemon::get_active_repo);
        let mut app = Self {
            quit_requested: false,
            patch_text: String::new(),
            hunks: Vec::new(),
            current_hunk: 0,
            file_text: String::new(),
            file_lines: Vec::new(),
            file_path: String::new(),
            base_dir: start_pwd.clone(),
            match_result: None,
            search_rows: Vec::new(),
            file_search_query: String::new(),
            search_matches: Vec::new(),
            search_match_idx: 0,
            is_searching: false,
            candidate_index: 0,
            scroll_to_match: true,
            message: None,
            message_until: None,
            cursor_line: None,
            cursor_col: 0,
            applied_hunks: HashSet::new(),
            merged_range: None,
            history: Vec::new(),
            vim_buffer: String::new(),
            yanked_line: None,
            d_pending: false,
            last_action: None,
            show_manual_paste: false,
            manual_paste_text: String::new(),
            initial_patch_path: initial_patch.clone(),
            file_states: HashMap::new(),
            show_help: false,
            show_debug: false,
            start_pwd,
            start_pwd_is_repo,
            left_selection: None,
            right_selection: None,
            right_drag_anchor: None,
            file_drag_selection: None,
            file_drag_anchor: None,
            file_anchors: BTreeMap::new(),
            mark_pending: None,
            git_statuses: Vec::new(),
            git_diff_rows: Vec::new(),
            git_hunks: Vec::new(),
            git_log_entries: Vec::new(),
            selected_git_log_entry: None,
            show_git_diff_window: false,
            show_git_diff_side: false,
            show_git_status_window: false,
            show_git_log_window: false,
            show_repos_window: false,
            filter_low_matches: false,
            sync_anchors: Vec::new(),
            pending_sync: None,
            next_sync_id: 1,
            pending_line_actions: Vec::new(),
            del_start: None,
            del_end: None,
            is_visual_mode: false,
            visual_start: None,
            is_insert_mode: false,
            insert_cursor: 0,
            format_on_save: config.format_on_save,
            fmt_command: config.fmt_command.clone(),
            show_settings: false,
            fmt_error: None,
            show_fmt_error: false,
            drag_start_active: false,
            drag_end_active: false,
            available_repos: Vec::new(),
            active_repo_id: active_repo_id.clone(),
            daemon_error: None,
            repo_receiver: rx,
            concat_server_enabled: config.concat_server_enabled,
            ignore_comments: config.ignore_comments,
            min_match_score: config.min_match_score,
            min_match_floor: config.min_match_floor,
            diff_side_hunk_idx: 0,
            diff_side_scroll_target: None,
            git_changed_files: Vec::new(),
            git_changed_file_idx: 0,
            git_diff_cursor: None,
            git_diff_vim_buffer: String::new(),
            git_diff_scroll_to_cursor: false,
            git_diff_insert_mode: false,
            diff_side_left_selection: None,
            diff_side_right_selection: None,
            diff_side_left_drag_anchor: None,
            diff_side_right_drag_anchor: None,
            diff_side_insert_anchor: None,
        };
        let mut loaded_patch = false;
        if let Some(patch_file) = initial_patch {
            let path = std::path::Path::new(&patch_file);
            if let Ok(content) = std::fs::read_to_string(path) {
                app.patch_text = content;
                if let Some(parent) = path.parent() {
                    let parent_str = parent.display().to_string();
                    if !parent_str.is_empty() {
                        let patch_repo = git2::Repository::discover(&parent_str).ok();
                        app.base_dir = patch_repo
                            .as_ref()
                            .and_then(|r| r.workdir().map(|w| w.display().to_string()))
                            .unwrap_or(parent_str);
                    }
                }
                loaded_patch = true;
                app.set_message(StatusMessage::success(format!(
                    "Loaded patch file: {}",
                    path.display()
                )));
            } else {
                app.set_message(StatusMessage::error(format!(
                    "Failed to read patch file: {}",
                    patch_file
                )));
            }
        }

        if !loaded_patch {
            app.set_message(StatusMessage::info(
                "Welcome! Open a .md file or paste a patch to begin.",
            ));
        }
        app.git_log_entries = super::git_ops::get_git_log(std::path::Path::new(&app.base_dir));
        app.reparse();
        app
    }

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
                        // Only include modified or deleted files, exclude newly added (staged or untracked)
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
        // Try to keep the currently loaded file selected, if it's in the list.
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
    pub fn load_git_changed_file(&mut self, idx: usize) {
        if idx >= self.git_changed_files.len() {
            return;
        }
        self.git_changed_file_idx = idx;
        let rel = self.git_changed_files[idx].clone();
        let path = std::path::Path::new(&self.base_dir)
            .join(&rel)
            .display()
            .to_string();
        if path == self.file_path {
            return;
        }
        self.save_file_state();
        self.file_path = path.clone();
        if let Some(saved) = self.file_states.get(&path).cloned() {
            self.file_lines = saved.lines;
            self.applied_hunks = saved.applied_hunks;
            self.history = saved.history;
            self.merged_range = saved.merged_range;
        } else {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    self.file_text = content;
                    self.file_lines = self.file_text.lines().map(String::from).collect();
                    self.applied_hunks.clear();
                    self.merged_range = None;
                    self.history.clear();
                }
                Err(e) => {
                    self.file_text.clear();
                    self.file_lines.clear();
                    self.set_message(StatusMessage::error(format!("Cannot read {}: {}", path, e)));
                    return;
                }
            }
        }
        self.diff_side_hunk_idx = 0;
        self.diff_side_scroll_target = Some(0);
        self.update_git_statuses();
        self.set_message(StatusMessage::info(format!("Diff: {}", rel)));
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
        self.set_mark('a', line);
    }

    pub fn set_mark_b(&mut self, line: usize) {
        self.set_mark('b', line);
    }

    pub fn set_mark_a_end(&mut self, line: usize) {
        if let Some(anchor) = self.file_anchors.get_mut(&'a') {
            anchor.end_line = Some(line);
            self.set_message(StatusMessage::info(format!(
                "⚓ mA range end set at line {}",
                line + 1
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
            self.set_message(StatusMessage::warning(
                "⚠ Set ma start first, but created mA end anyway",
            ));
        }
        self.scroll_to_match = true;
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
            let start = a.line;
            let end = if let Some(b) = self.file_anchors.get(&'b') {
                let mut e = a.line.max(b.line);
                if let Some(a_end) = a.end_line {
                    e = e.max(a_end);
                }
                if let Some(b_end) = b.end_line {
                    e = e.max(b_end);
                }
                e
            } else {
                a.end_line.unwrap_or(
                    self.match_result
                        .as_ref()
                        .map(|mr| mr.file_end.saturating_sub(1))
                        .unwrap_or(start),
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
        self.git_log_entries = super::git_ops::get_git_log(repo_root);
    }
    /// Recompute the HEAD-vs-working diff shown in the diff-side panel.
    /// Called after any in-place edit made from that panel (dd/yy/p/insert/revert)
    /// so the view stays in sync with the corrected buffer.
    /// Recompute the HEAD-vs-working diff shown in the diff-side panel.
    /// Called after any in-place edit made from that panel (dd/yy/p/insert/revert)
    /// so the view stays in sync with the corrected buffer.
    pub fn refresh_git_diff_side_rows(&mut self) {
        self.update_git_statuses();
    }
    /// Resolve a diff-side row index to the working-file line it sits after,
    /// by walking forward (or, failing that, backward) to the nearest row
    /// that still has a right-side (working) line number.
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
    /// Insert the currently selected HEAD (left-side) lines into the working
    /// buffer, right after the chosen insert anchor row.
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
    /// Delete the currently selected working (right-side) lines directly
    /// from the working buffer.
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
            self.set_message(StatusMessage::warning("Nothing to delete in that selection"));
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

    pub fn reparse(&mut self) {
        self.save_file_state();
        self.hunks = crate::patch::parse_patches(&self.patch_text);
        self.current_hunk = 0;
        self.applied_hunks.clear();
        self.merged_range = None;
        self.history.clear();
        self.vim_buffer.clear();
        self.show_manual_paste = false;
        self.manual_paste_text.clear();
        self.d_pending = false;
        self.last_action = None;
        self.file_path.clear();
        self.file_states.clear();
        self.sync_anchors.clear();
        self.pending_sync = None;
        self.pending_line_actions.clear();
        self.del_start = None;
        self.del_end = None;
        self.load_hunk();
    }

    pub fn save_file_state(&mut self) {
        if self.file_path.is_empty() {
            return;
        }
        self.file_states.insert(
            self.file_path.clone(),
            FileState {
                lines: self.file_lines.clone(),
                applied_hunks: self.applied_hunks.clone(),
                history: self.history.clone(),
                merged_range: self.merged_range,
            },
        );
    }

    pub fn load_hunk(&mut self) {
        let hunk = match self.hunks.get(self.current_hunk) {
            Some(h) => h.clone(),
            None => return,
        };
        let path = std::path::Path::new(&self.base_dir)
            .join(&hunk.filename)
            .display()
            .to_string();
        if path != self.file_path {
            self.save_file_state();
            self.file_path = path.clone();
            if let Some(saved) = self.file_states.get(&path).cloned() {
                self.file_lines = saved.lines;
                self.applied_hunks = saved.applied_hunks;
                self.history = saved.history;
                self.merged_range = saved.merged_range;
                self.set_message(StatusMessage::info(format!("Restored edits for: {}", path)));
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        self.file_text = content;
                        self.file_lines = self.file_text.lines().map(String::from).collect();
                        self.applied_hunks.clear();
                        self.merged_range = None;
                        self.history.clear();
                        self.set_message(StatusMessage::success(format!("Loaded: {}", path)));
                    }
                    Err(e) => {
                        if hunk.search.is_empty() {
                            self.file_text = String::new();
                            self.file_lines = Vec::new();
                            self.applied_hunks.clear();
                            self.merged_range = None;
                            self.history.clear();
                            self.set_message(StatusMessage::info(format!(
                                "✚ Ready to create new file: {}",
                                path
                            )));
                        } else {
                            self.file_text = String::new();
                            self.file_lines = Vec::new();
                            self.applied_hunks.clear();
                            self.merged_range = None;
                            self.history.clear();
                            self.set_message(StatusMessage::error(format!(
                                "Cannot read {}: {}",
                                path, e
                            )));
                        }
                    }
                }
            }
        }
        self.file_anchors.clear();
        self.mark_pending = None;
        self.file_search_query.clear();
        self.search_matches.clear();
        self.search_match_idx = 0;
        self.is_searching = false;
        self.candidate_index = 0;
        self.cursor_line = None;
        self.scroll_to_match = true;
        self.vim_buffer.clear();
        self.d_pending = false;
        self.show_manual_paste = false;
        self.manual_paste_text.clear();
        self.last_action = None;
        self.left_selection = None;
        self.right_selection = None;
        self.right_drag_anchor = None;
        self.file_drag_selection = None;
        self.file_drag_anchor = None;
        self.sync_anchors.clear();
        self.pending_sync = None;
        self.pending_line_actions.clear();
        self.del_start = None;
        self.del_end = None;
        self.is_visual_mode = false;
        self.visual_start = None;
        self.is_insert_mode = false;
        self.insert_cursor = 0;
        self.update_git_statuses();
        self.recompute_match();
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

    pub fn save_config(&self) {
        let config = AppConfig {
            format_on_save: self.format_on_save,
            fmt_command: self.fmt_command.clone(),
            active_repo_id: self.active_repo_id.clone(),
            concat_server_enabled: self.concat_server_enabled,
            ignore_comments: self.ignore_comments,
            min_match_score: self.min_match_score,
            min_match_floor: self.min_match_floor,
        };
        config.save();
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
impl Drop for MergeApp {
    fn drop(&mut self) {
        self.save_config();
    }
}

impl eframe::App for MergeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(Key::Q) && i.modifiers.alt) {
            self.quit_requested = true;
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

        if ctx.input(|i| i.key_pressed(Key::F4)) {
            self.show_fmt_error = false;
            self.show_settings = false;
            self.show_repos_window = false;
            self.show_debug = false;
            self.show_git_status_window = false;
            self.show_git_diff_side = false;
            self.show_git_log_window = false;
            self.show_git_diff_window = !self.show_git_diff_window;
        }
        if ctx.input(|i| i.key_pressed(Key::F5)) {
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
        if ctx.input(|i| i.key_pressed(Key::F3)) {
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
            self.show_git_status_window = !self.show_git_status_window;
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
                    } else if self.show_git_diff_side {
                        self.show_git_diff_side = false;
                    } else if self.show_git_diff_window {
                        self.show_git_diff_window = false;
                    } else if self.show_git_status_window {
                        self.show_git_status_window = false;
                    } else if self.show_git_log_window {
                        self.show_git_log_window = false;
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