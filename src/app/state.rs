use super::constants::{DEFAULT_FILE, DEFAULT_PATCH};
use super::matching::MergeMatching;
use super::types::{Action, FileAnchor, FileState, StatusMessage};
use crate::app::pal;
use crate::diff::MatchResult;
use crate::patch::PatchHunk;
use eframe::egui::*;
use std::collections::{HashMap, HashSet};

pub struct MergeApp {
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
    pub last_action: Option<Action>,
    pub file_states: HashMap<String, FileState>,
    pub show_help: bool,
    pub show_minimap: bool,

    // ── Left panel (search) selection ────────────────────────────────────────
    /// Row range selected in the search/replace panel (search-panel row indices).
    pub left_selection: Option<(usize, usize)>,

    // ── Right panel (file) marks ─────────────────────────────────────────────
    /// `ma` / `mb` marks in the file panel.
    /// Replaces the old `manual_anchor`, `mark_mode`, and `mark_a` fields.
    /// None  → use auto-match anchor.
    /// Some  → user-placed anchor; `b` filled after `mb` is set.
    pub file_anchor: Option<FileAnchor>,

    /// True while waiting for the user to press 'a' or 'b' after 'm'.
    pub mark_pending: Option<MarkPending>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkPending {
    /// User pressed 'm', waiting for 'a' or 'b'
    WaitingKey,
}

impl MergeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_patch: Option<String>) -> Self {
        cc.egui_ctx.set_visuals(Visuals::dark());
        let mut app = Self {
            patch_text: String::new(),
            hunks: Vec::new(),
            current_hunk: 0,
            file_text: String::new(),
            file_lines: Vec::new(),
            file_path: String::new(),
            base_dir: std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
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
            last_action: None,
            file_states: HashMap::new(),
            show_help: false,
            show_minimap: true,
            left_selection: None,
            file_anchor: None,
            mark_pending: None,
        };

        let mut loaded_patch = false;
        if let Some(patch_file) = initial_patch {
            let path = std::path::Path::new(&patch_file);
            if let Ok(content) = std::fs::read_to_string(path) {
                app.patch_text = content;
                if let Some(parent) = path.parent() {
                    app.base_dir = parent.display().to_string();
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

    // ── message ───────────────────────────────────────────────────────────────

    pub fn set_message(&mut self, msg: StatusMessage) {
        self.message = Some(msg);
        self.message_until = None;
    }

    // ── mark helpers ──────────────────────────────────────────────────────────

    /// Set mark-a at `line` in the file panel.
    /// Clears any existing b, and links to the current left_selection.
    pub fn set_mark_a(&mut self, line: usize) {
        self.file_anchor = Some(FileAnchor::start_only(line));
        self.set_message(StatusMessage::info(format!(
            "⚓ ma set at line {} — press mb to set end, or > / A to apply",
            line + 1
        )));
    }

    pub fn set_mark_b(&mut self, line: usize) {
        let (a, b) = if let Some(anchor) = &mut self.file_anchor {
            let b = line.max(anchor.a);
            anchor.b = Some(b + 1);
            (anchor.a + 1, b + 1) // a and b are line numbers (1‑based) for the message
        } else {
            self.set_message(StatusMessage::warning(
                "Set ma first (cursor on line, then 'ma')",
            ));
            return;
        };
        self.set_message(StatusMessage::info(format!(
            "⚓ ma:{}–mb:{} range set — press > / A to apply",
            a, b,
        )));
    }

    /// Clear both marks.
    pub fn clear_marks(&mut self) {
        self.file_anchor = None;
        self.mark_pending = None;
        self.scroll_to_match = true;
    }

    // ── apply_merge uses file_anchor ──────────────────────────────────────────

    /// Returns (file_start, file_end) for the current apply operation.
    /// Priority: file_anchor > auto match_result.
    pub fn resolve_apply_range(&self) -> Option<(usize, usize)> {
        if let Some(fa) = self.file_anchor {
            return Some((fa.file_start(), fa.file_end()));
        }
        self.match_result
            .as_ref()
            .map(|mr| (mr.file_start, mr.file_end))
    }

    // ── parse / load ──────────────────────────────────────────────────────────

    pub fn reparse(&mut self) {
        self.save_file_state();
        self.hunks = crate::patch::parse_patches(&self.patch_text);
        self.current_hunk = 0;
        self.applied_hunks.clear();
        self.merged_range = None;
        self.history.clear();
        self.vim_buffer.clear();
        self.last_action = None;
        self.file_path.clear();
        self.file_states.clear();
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

        self.file_anchor = None;
        self.mark_pending = None;
        self.file_search_query.clear();
        self.search_matches.clear();
        self.search_match_idx = 0;
        self.is_searching = false;
        self.candidate_index = 0;
        self.cursor_line = None;
        self.scroll_to_match = true;
        self.vim_buffer.clear();
        self.last_action = None;
        self.left_selection = None;
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
        self.file_anchor = None;
        self.mark_pending = None;
        self.file_search_query.clear();
        self.search_matches.clear();
        self.search_match_idx = 0;
        self.is_searching = false;
        self.candidate_index = 0;
        self.cursor_line = None;
        self.scroll_to_match = true;
        self.left_selection = None;
    }
}

impl eframe::App for MergeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // message expiry
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

        if !ctx.wants_keyboard_input() || self.is_searching {
            ctx.input(|i| {
                if i.key_pressed(Key::Escape) {
                    if self.show_help {
                        self.show_help = false;
                    } else if self.is_searching {
                        self.is_searching = false;
                        self.file_search_query.clear();
                        self.search_matches.clear();
                        self.scroll_to_match = true;
                    } else if self.file_anchor.is_some() || self.mark_pending.is_some() {
                        self.clear_marks();
                    } else if self.left_selection.is_some() {
                        self.left_selection = None;
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
                        .any(|e| matches!(e, Event::Text(t) if t == "o" || t == "O"))
                {
                    self.show_minimap = !self.show_minimap;
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

        // Mark-pending HUD — show at bottom when 'm' was pressed
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
                                "→ press  a  (set ma start)  ·  b  (set mb end)  ·  ESC cancel",
                            )
                            .color(pal::TEXT_NORMAL)
                            .monospace()
                            .small(),
                        );
                        if let Some(fa) = self.file_anchor {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(fa.label())
                                        .color(pal::TEXT_ANCHOR)
                                        .monospace()
                                        .small(),
                                );
                            });
                        }
                    });
                });
        }

        super::toolbar::render_toolbar(self, ctx);
        super::status_bar::render_status_bar(self, ctx);

        if self.show_help {
            super::help::render_help_overlay(self, ctx);
        }

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
            if self.show_minimap {
                super::minimap::render_with_minimap(self, ui);
            } else {
                SidePanel::left("minimap_collapsed")
                    .resizable(false)
                    .exact_width(0.0)
                    .show_inside(ui, |_| {});
                super::split_view::render_split_view(self, ui);
            }
        });
    }
}
