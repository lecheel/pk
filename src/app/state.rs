use super::constants::{DEFAULT_FILE, DEFAULT_PATCH};
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
    pub applied_hunks: HashSet<usize>,
    pub merged_range: Option<(usize, usize)>,
    pub history: Vec<(Vec<String>, usize)>,
    pub vim_buffer: String,
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
    pub file_anchors: BTreeMap<char, FileAnchor>,
    pub mark_pending: Option<MarkPending>,
    pub git_statuses: Vec<GitStatus>,
    pub git_diff_rows: Vec<crate::diff::DiffRow>,
    pub git_hunks: Vec<super::git_ops::GitDiffHunk>,
    pub show_git_diff_window: bool,
    pub show_git_status_window: bool,
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
    pub available_repos: Vec<RepoInfo>,
    pub active_repo_id: Option<String>,
    pub daemon_error: Option<String>,
    pub repo_receiver: mpsc::Receiver<Result<Vec<RepoInfo>, String>>,
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

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let res = daemon::fetch_repos();
            let _ = tx.send(res);
        });

        let active_repo_id = daemon::get_active_repo();

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
            applied_hunks: HashSet::new(),
            merged_range: None,
            history: Vec::new(),
            vim_buffer: String::new(),
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
            file_anchors: BTreeMap::new(),
            mark_pending: None,
            git_statuses: Vec::new(),
            git_diff_rows: Vec::new(),
            git_hunks: Vec::new(),
            show_git_diff_window: false,
            show_git_status_window: false,
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
            available_repos: Vec::new(),
            active_repo_id: active_repo_id.clone(),
            daemon_error: None,
            repo_receiver: rx,
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
            app.patch_text = DEFAULT_PATCH.to_string();
            app.set_message(StatusMessage::info(
                "No patch file provided — using embedded demo patch. Press ? for help.",
            ));
        }
        app.reparse();
        app
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
            let best = crate::diff::find_best_match(&hunk.search, &self.file_lines);
            best.score >= 60.0
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
                self.set_message(StatusMessage::warning("No hunks have match score >= 60%"));
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
        
        // If 'a' and 'b' marks are set, use them as the explicit replace range
        if let Some(a) = self.file_anchors.get(&'a') {
            let start = a.line;
            let end = if let Some(b) = self.file_anchors.get(&'b') {
                let mut e = a.line.max(b.line);
                if let Some(a_end) = a.end_line { e = e.max(a_end); }
                if let Some(b_end) = b.end_line { e = e.max(b_end); }
                e
            } else {
                a.end_line.unwrap_or(start)
            };
            return Some((start, end + 1)); // +1 because file_end is exclusive
        }

        self.match_result
            .as_ref()
            .map(|mr| (mr.file_start, mr.file_end))
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
        let (statuses, diff_rows) =
            super::git_ops::get_line_statuses(repo_root, file_path, &self.file_lines);
        self.git_statuses = statuses;
        self.git_diff_rows = diff_rows;
        self.git_hunks =
            super::git_ops::group_git_hunks(&self.git_diff_rows, self.file_lines.len());
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
                        if hunk.filename.ends_with("mod.rs") {
                            self.file_text = DEFAULT_FILE.to_string();
                            self.file_lines = self.file_text.lines().map(String::from).collect();
                            self.applied_hunks.clear();
                            self.merged_range = None;
                            self.history.clear();
                            self.set_message(StatusMessage::warning(format!(
                                "File not found — using embedded sample ({})",
                                e
                            )));
                        } else if hunk.search.is_empty() {
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
        self.sync_anchors.clear();
        self.pending_sync = None;
        self.pending_line_actions.clear();
        self.del_start = None;
        self.del_end = None;
        self.is_visual_mode = false;
        self.visual_start = None;
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
        self.git_statuses.clear();
        self.git_diff_rows.clear();
        self.git_hunks.clear();
        self.show_git_status_window = false;
        self.sync_anchors.clear();
        self.pending_sync = None;
        self.pending_line_actions.clear();
        self.del_start = None;
        self.del_end = None;
        self.is_visual_mode = false;
        self.visual_start = None;
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
            self.show_git_diff_window = !self.show_git_diff_window;
        }
        if ctx.input(|i| i.key_pressed(Key::F3)) {
            self.show_debug = !self.show_debug;
        }
        if ctx.input(|i| i.key_pressed(Key::F1)) {
            self.show_git_status_window = !self.show_git_status_window;
        }
        if !ctx.wants_keyboard_input() || self.is_searching {
            ctx.input(|i| {
                if i.key_pressed(Key::Escape) {
                    if self.show_repos_window {
                        self.show_repos_window = false;
                    } else if self.show_help {
                        self.show_help = false;
                    } else if self.show_debug {
                        self.show_debug = false;
                    } else if self.show_git_diff_window {
                        self.show_git_diff_window = false;
                    } else if self.show_git_status_window {
                        self.show_git_status_window = false;
                    } else if self.is_searching {
                        self.is_searching = false;
                        self.file_search_query.clear();
                        self.search_matches.clear();
                        self.scroll_to_match = true;
                    } else if self.pending_sync.is_some() {
                        self.pending_sync = None;
                    } else if !self.file_anchors.is_empty() || self.mark_pending.is_some() {
                        self.clear_marks();
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
                        if blink {
                            ui.label(RichText::new("█").color(pal::TEXT_NORMAL).monospace());
                        } else {
                            ui.label(RichText::new(" ").monospace());
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
        super::toolbar::render_toolbar(self, ctx);
        super::status_bar::render_status_bar(self, ctx);
        CentralPanel::default().show(ctx, |ui| {
            if self.hunks.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.heading("No patches found");
                    ui.label("Open a .md file containing <patch> blocks.");
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Press ? for keyboard shortcuts")
                            .color(super::palette::pal::TEXT_DIM)
                            .small(),
                    );
                });
                return;
            }
            super::split_view::render_split_view(self, ui);
        });
        if self.show_help {
            super::help::render_help_overlay(self, ctx);
        }
        if self.show_repos_window {
            let mut show_repos = self.show_repos_window;
            Window::new("📂 Active Repository")
                .open(&mut show_repos)
                .collapsible(false)
                .resizable(false)
                .default_size(Vec2::new(450.0, 300.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Select the repository where file paths should resolve:")
                                .color(pal::TEXT_NORMAL),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("🔄 Refresh").clicked() {
                                let (tx, rx) = mpsc::channel();
                                self.repo_receiver = rx;
                                std::thread::spawn(move || {
                                    let res = daemon::fetch_repos();
                                    let _ = tx.send(res);
                                });
                            }
                        });
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if self.available_repos.is_empty() {
                        ui.label(RichText::new(if self.daemon_error.is_some() {
                            format!("⚠️ Daemon error: {}", self.daemon_error.as_ref().unwrap())
                        } else {
                            "No repos registered. Use 'cli add-repo' to register one.".to_string()
                        }).color(pal::TEXT_DIM));
                    } else {
                        ScrollArea::vertical().show(ui, |ui| {
                            let repos_clone = self.available_repos.clone();
                            for repo in repos_clone.iter() {
                                let is_active = self.active_repo_id.as_deref() == Some(repo.id.as_str());
                                let bg = if is_active {
                                    Color32::from_rgb(30, 45, 30)
                                } else {
                                    pal::BG_PANEL
                                };
                                Frame::none()
                                    .fill(bg)
                                    .stroke(Stroke::new(
                                        1.0,
                                        if is_active { pal::ACCENT_GOOD } else { pal::SEPARATOR },
                                    ))
                                    .rounding(4.0)
                                    .inner_margin(Margin::symmetric(8.0, 6.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(if is_active { "→ " } else { "  " })
                                                    .color(pal::ACCENT_GOOD)
                                                    .monospace(),
                                            );
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new(&repo.id)
                                                            .color(pal::TEXT_NORMAL)
                                                            .strong()
                                                            .monospace(),
                                                    );
                                                    if let Some(branch) = &repo.git_branch {
                                                        ui.label(
                                                            RichText::new(format!("[{}]", branch))
                                                                .color(pal::TEXT_DIM)
                                                                .small(),
                                                        );
                                                    }
                                                    if is_active {
                                                        ui.label(
                                                            RichText::new("↑ active")
                                                                .color(pal::ACCENT_GOOD)
                                                                .small(),
                                                        );
                                                    }
                                                });
                                                let files = repo.file_count.unwrap_or(0);
                                                ui.label(
                                                    RichText::new(format!("{}  ({} files)", repo.source_path, files))
                                                        .color(pal::TEXT_DIM)
                                                        .small(),
                                                );
                                            });
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                if is_active {
                                                    if ui.button("✕ Clear").clicked() {
                                                        self.active_repo_id = None;
                                                        daemon::clear_active_repo();
                                                        self.set_message(StatusMessage::info("Cleared active repo. Paths must be fully qualified."));
                                                    }
                                                } else {
                                                    if ui.button("Use").clicked() {
                                                        self.active_repo_id = Some(repo.id.clone());
                                                        self.base_dir = repo.source_path.clone();
                                                        self.start_pwd = repo.source_path.clone();
                                                        self.start_pwd_is_repo = true;
                                                        daemon::set_active_repo(&repo.id);
                                                        self.set_message(StatusMessage::success(format!(
                                                            "✅ Active repo: {}. Files will be looked up in repo '{}'",
                                                            repo.id, repo.id
                                                        )));
                                                        self.show_repos_window = false;
                                                        self.reparse();
                                                    }
                                                }
                                                if ui.button("🔄 Sync").clicked() {
                                                    let id = repo.id.clone();
                                                    let ctx_clone = ctx.clone();
                                                    self.set_message(StatusMessage::info(format!("Syncing {}...", id)));
                                                    std::thread::spawn(move || {
                                                        match daemon::sync_repo(&id) {
                                                            Ok(_) => {},
                                                            Err(_) => {},
                                                        }
                                                        ctx_clone.request_repaint();
                                                    });
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(4.0);
                            }
                        });
                    }
                });
            self.show_repos_window = show_repos;
        }
        if self.show_debug {
            let mut show_debug = self.show_debug;
            Window::new("🐞 App diagnostics")
                .open(&mut show_debug)
                .default_size(Vec2::new(550.0, 420.0))
                .show(ctx, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        let mut report = String::new();
                        ui.heading("Paths & directory mappings");
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Start PWD:").strong());
                            ui.label(RichText::new(&self.start_pwd).monospace());
                            if self.start_pwd_is_repo {
                                ui.colored_label(Color32::from_rgb(120, 220, 160), "(Git Repo)");
                            } else {
                                ui.colored_label(Color32::from_rgb(230, 100, 100), "(Not Git Repo)");
                            }
                        });
                        report.push_str(&format!("Start PWD: {} (Git Repo: {})\n", self.start_pwd, self.start_pwd_is_repo));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Base directory:").strong());
                            ui.label(RichText::new(&self.base_dir).monospace());
                        });
                        report.push_str(&format!("Base Directory: {}\n", self.base_dir));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Current file path:").strong());
                            ui.label(RichText::new(&self.file_path).monospace());
                        });
                        report.push_str(&format!("Current File Path: {}\n", self.file_path));
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.heading("Git mapping diagnostics");
                        let repo_root = std::path::Path::new(&self.base_dir);
                        match git2::Repository::discover(repo_root) {
                            Ok(repo) => {
                                ui.colored_label(Color32::from_rgb(120, 220, 160), "✔ Git repository found");
                                report.push_str("Git Repo: Found\n");
                                if let Some(workdir) = repo.workdir() {
                                    ui.horizontal(|ui| {
                                        ui.label("Repo workdir:");
                                        ui.label(RichText::new(workdir.to_string_lossy()).monospace());
                                    });
                                    report.push_str(&format!("Repo workdir: {}\n", workdir.to_string_lossy()));
                                    let file_path = std::path::Path::new(&self.file_path);
                                    let abs_file_path = if file_path.is_absolute() {
                                        file_path.to_path_buf()
                                    } else if let Ok(cwd) = std::env::current_dir() {
                                        cwd.join(file_path)
                                    } else {
                                        file_path.to_path_buf()
                                    };
                                    let abs_workdir = if workdir.is_absolute() {
                                        workdir.to_path_buf()
                                    } else if let Ok(cwd) = std::env::current_dir() {
                                        cwd.join(workdir)
                                    } else {
                                        workdir.to_path_buf()
                                    };
                                    let clean_path = |p: &std::path::Path| -> String {
                                        let s = p.to_string_lossy().replace('\\', "/");
                                        if let Some(stripped) = s.strip_prefix("//?/") {
                                            stripped.to_string()
                                        } else {
                                            s
                                        }
                                    };
                                    let clean_file = clean_path(&abs_file_path);
                                    let clean_work = clean_path(&abs_workdir);
                                    ui.horizontal(|ui| {
                                        ui.label("Normalized file path:");
                                        ui.label(RichText::new(&clean_file).monospace());
                                    });
                                    report.push_str(&format!("Normalized file path: {}\n", clean_file));
                                    ui.horizontal(|ui| {
                                        ui.label("Normalized workdir:");
                                        ui.label(RichText::new(&clean_work).monospace());
                                    });
                                    report.push_str(&format!("Normalized workdir: {}\n", clean_work));
                                    if clean_file.starts_with(&clean_work) {
                                        let rel = &clean_file[clean_work.len()..].trim_start_matches('/');
                                        ui.colored_label(
                                            Color32::from_rgb(120, 220, 160),
                                            format!("✔ Relative path match: {}", rel)
                                        );
                                        report.push_str(&format!("Relative path match: {}\n", rel));
                                    } else {
                                        ui.colored_label(
                                            Color32::from_rgb(230, 100, 100),
                                            "❌ Path mismatch: File is not inside the repo workdir."
                                        );
                                        report.push_str("Relative path match: Mismatch (File not inside repo workdir)\n");
                                    }
                                } else {
                                    ui.colored_label(Color32::from_rgb(230, 100, 100), "❌ Git repo missing working directory");
                                    report.push_str("Git Repo Workdir: Missing\n");
                                }
                            }
                            Err(e) => {
                                ui.colored_label(
                                    Color32::from_rgb(230, 100, 100),
                                    format!("❌ Git lookup error: {}", e)
                                );
                                report.push_str(&format!("Git lookup error: {}\n", e));
                            }
                        }
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.heading("Buffers & state summary");
                        ui.label(format!("Total patches in file: {}", self.hunks.len()));
                        ui.label(format!("Current hunk index: {}", self.current_hunk));
                        ui.label(format!("Applied hunks indices: {:?}", self.applied_hunks));
                        ui.label(format!("File lines: {}", self.file_lines.len()));
                        ui.label(format!("Git status indexes: {}", self.git_statuses.len()));
                        report.push_str(&format!("Total patches: {}\n", self.hunks.len()));
                        report.push_str(&format!("Current hunk index: {}\n", self.current_hunk));
                        report.push_str(&format!("Applied hunk indices: {:?}\n", self.applied_hunks));
                        report.push_str(&format!("File lines: {}\n", self.file_lines.len()));
                        report.push_str(&format!("Git status indexes: {}\n", self.git_statuses.len()));
                        let (mut unchanged, mut added, mut modified, mut deleted) = (0, 0, 0, 0);
                        for status in &self.git_statuses {
                            match status {
                                GitStatus::Unchanged => unchanged += 1,
                                GitStatus::Added => added += 1,
                                GitStatus::Modified => modified += 1,
                                GitStatus::Deleted => deleted += 1,
                            }
                        }
                        ui.horizontal(|ui| {
                            ui.label("Gutter distribution:");
                            ui.colored_label(Color32::from_gray(160), format!("Unchanged: {} ", unchanged));
                            ui.colored_label(Color32::from_rgb(120, 220, 160), format!("Added: {} ", added));
                            ui.colored_label(Color32::from_rgb(220, 200, 100), format!("Modified: {} ", modified));
                            ui.colored_label(Color32::from_rgb(235, 120, 120), format!("Deleted: {}", deleted));
                        });
                        report.push_str(&format!("Gutter: Unchanged: {}, Added: {}, Modified: {}, Deleted: {}\n", unchanged, added, modified, deleted));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button("📋 Copy Diagnostics").clicked() {
                                ui.ctx().copy_text(report);
                            }
                            if ui.button("Force update git status").clicked() {
                                self.update_git_statuses();
                            }
                        });
                    });
                });
            self.show_debug = show_debug;
        }
    }
}