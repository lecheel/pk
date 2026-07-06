use super::chat::ChatMode;
use super::config::AppConfig;
use super::daemon;
use super::matching::MergeMatching;
use super::state::MergeApp;
use super::types::{FileState, StatusMessage};
use eframe::egui::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc;

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
            last_save_all_result: None,
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
            anchor_link_source: None,
            anchor_link_target: None,
            show_commit_prompt: false,
            show_git_commit_window: false,
            commit_message: String::new(),
            git_status_selected_idx: None,
            git_status_selected_path: None,
            show_chat_window: false,
            chat_mode: ChatMode::Chat,
            chat_sessions: super::chat::ChatSessions::default(),
            llm_config: config.llm_config.clone(),
            show_system_prompt: false,
            commit_ai_session: super::chat::ChatSession::default(),
            rustconcat_api_url: config.rustconcat_api_url.clone(),
            impl_tools: config.impl_tools.clone(),
            impl_step: 0,
            debug_impl_llm: config.debug_impl_llm,
            impl_is_running: false,
            impl_result_indicator: String::new(),
            impl_skeleton: String::new(),
            impl_files: String::new(),
            impl_hashes: String::new(),
            daemon_sync_warning: None,
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
                "Welcome! Open a .md file or paste a patch to begin. (F1 git status) (F4 git log)",
            ));
        }
        app.git_log_entries = super::git_ops::get_git_log(std::path::Path::new(&app.base_dir));
        app.reparse();
        app
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

    pub fn go_home(&mut self) {
        self.patch_text.clear();
        self.hunks.clear();
        self.current_hunk = 0;
        self.file_text.clear();
        self.file_lines.clear();
        self.file_path.clear();
        self.base_dir = self.start_pwd.clone();
        self.match_result = None;
        self.search_rows.clear();
        self.file_search_query.clear();
        self.search_matches.clear();
        self.search_match_idx = 0;
        self.is_searching = false;
        self.candidate_index = 0;
        self.scroll_to_match = false;
        self.cursor_line = None;
        self.cursor_col = 0;
        self.applied_hunks.clear();
        self.merged_range = None;
        self.history.clear();
        self.vim_buffer.clear();
        self.yanked_line = None;
        self.d_pending = false;
        self.last_action = None;
        self.show_manual_paste = false;
        self.manual_paste_text.clear();
        self.initial_patch_path = None;
        self.file_states.clear();
        self.show_help = false;
        self.show_debug = false;
        self.left_selection = None;
        self.right_selection = None;
        self.right_drag_anchor = None;
        self.file_drag_selection = None;
        self.file_drag_anchor = None;
        self.file_anchors.clear();
        self.mark_pending = None;
        self.git_statuses.clear();
        self.git_diff_rows.clear();
        self.git_hunks.clear();
        self.show_git_diff_window = false;
        self.show_git_diff_side = false;
        self.show_git_status_window = false;
        self.show_git_log_window = false;
        self.show_repos_window = false;
        self.show_settings = false;
        self.show_fmt_error = false;
        self.fmt_error = None;
        self.filter_low_matches = false;
        self.sync_anchors.clear();
        self.pending_sync = None;
        self.pending_line_actions.clear();
        self.del_start = None;
        self.del_end = None;
        self.is_visual_mode = false;
        self.visual_start = None;
        self.is_insert_mode = false;
        self.insert_cursor = 0;
        self.git_changed_files.clear();
        self.git_changed_file_idx = 0;
        self.git_diff_cursor = None;
        self.git_diff_vim_buffer.clear();
        self.git_diff_scroll_to_cursor = false;
        self.git_diff_insert_mode = false;
        self.diff_side_hunk_idx = 0;
        self.diff_side_scroll_target = None;
        self.diff_side_left_selection = None;
        self.diff_side_right_selection = None;
        self.diff_side_left_drag_anchor = None;
        self.diff_side_right_drag_anchor = None;
        self.diff_side_insert_anchor = None;
        self.anchor_link_source = None;
        self.anchor_link_target = None;
        self.git_status_selected_idx = None;
        self.show_commit_prompt = false;
        self.commit_message.clear();
        self.show_git_commit_window = false;
        self.git_status_selected_path = None;
        self.show_chat_window = false;
        self.chat_mode = ChatMode::Chat;
        self.chat_sessions = super::chat::ChatSessions::default();
        self.show_system_prompt = false;
        self.commit_ai_session = super::chat::ChatSession::default();
        self.git_log_entries = super::git_ops::get_git_log(std::path::Path::new(&self.base_dir));
        self.set_message(StatusMessage::info(
            "Welcome! Open a .md file or paste a patch to begin.",
        ));
    }
}