use super::chat::{ChatEntry, ChatMode, ChatSession, ChatSessions};
use super::clipboard_utils::get_clipboard_text;
use super::config::AppConfig;
use super::daemon::{self, RepoInfo};
use super::git_ops::GitStatus;
use super::llm::LlmConfig;
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
    pub last_save_all_result: Option<(usize, usize)>,
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
    pub short_search_display: bool,
    pub disable_llm: bool,
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
    pub anchor_link_source: Option<Pos2>,
    pub anchor_link_target: Option<Pos2>,
    pub git_status_selected_idx: Option<usize>,
    pub show_commit_prompt: bool,
    pub show_git_commit_window: bool,
    pub commit_message: String,
    pub git_status_selected_path: Option<String>,
    pub show_chat_window: bool,
    pub chat_mode: ChatMode,
    pub chat_sessions: ChatSessions,
    pub llm_config: LlmConfig,
    pub show_system_prompt: bool,
    pub commit_ai_session: ChatSession,
    pub rustconcat_api_url: String,
    pub impl_tools: super::config::ImplToolsConfig,
    pub debug_impl_llm: bool,
    pub impl_step: usize,
    pub impl_is_running: bool,
    pub impl_result_indicator: String,
    pub impl_skeleton: String,
    pub impl_files: String,
    pub impl_hashes: String,
    pub daemon_sync_warning: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkPending {
    WaitingKey,
}

impl Drop for MergeApp {
    fn drop(&mut self) {
        self.save_config();
    }
}
