// //--+ src/app.rs
// //--+ file:///src/app.rs
use crate::diff::{self, MatchResult, RowKind};
use crate::patch::{self, PatchHunk};
use eframe::egui::*;
use std::collections::{HashMap, HashSet};

const DEFAULT_PATCH: &str = r#"<patch>
filename src/repl/mod.rs
<<<<<<< SEARCH
pub(crate) enum CommandResult {
    Continue,
    Quit,
    ClearScreen,
}
=======
pub(crate) enum CommandResult {
    Continue,
    Quit,
    ClearScreen,
}
#[derive(Clone, Copy)]
pub(crate) enum RepeatAction {
    NextHunk,
    PrevHunk,
    NextFunc,
    PrevFunc,
}
>>>>>>> REPLACE
</patch>"#;

const DEFAULT_FILE: &str = r#"//! REPL module for the pcode debugger.
use std::io::{self, BufRead, Write};
/// Result of executing a REPL command.
pub(crate) enum CommandResult {
    Continue,
    Quit,
    ClearScreen,
}
/// Run the interactive REPL loop.
pub(crate) fn run_repl() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    loop {
        write!(stdout, "pcode> ")?;
        stdout.flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim();
        match trimmed {
            "quit" | "exit" => return Ok(()),
            "clear" | "cls" => return Ok(()),
            "" => continue,
            _ => {
                writeln!(stdout, "Unknown command: {trimmed}")?;
            }
        }
    }
    Ok(())
}
"#;

// ── Palette ──────────────────────────────────────────────────────────────────
mod pal {
    use eframe::egui::Color32;
    pub const BG_BASE: Color32 = Color32::from_rgb(18, 20, 24);
    pub const BG_PANEL: Color32 = Color32::from_rgb(24, 28, 36);
    pub const BG_TOOLBAR: Color32 = Color32::from_rgb(20, 24, 30);
    pub const BG_ROW_EVEN: Color32 = Color32::from_rgb(22, 25, 31);
    pub const BG_ROW_ODD: Color32 = Color32::from_rgb(25, 29, 36);
    pub const BG_MATCH: Color32 = Color32::from_rgb(22, 44, 30);
    pub const BG_MERGED: Color32 = Color32::from_rgb(20, 48, 28);
    pub const BG_CURSOR: Color32 = Color32::from_rgb(28, 38, 62);
    pub const BG_ANCHOR: Color32 = Color32::from_rgb(42, 34, 12);
    pub const BG_SEARCH_HIT: Color32 = Color32::from_rgb(48, 42, 14);
    pub const BG_DELETE: Color32 = Color32::from_rgb(48, 22, 22);
    pub const BG_INSERT: Color32 = Color32::from_rgb(18, 42, 24);
    pub const BAR_MATCH: Color32 = Color32::from_rgb(60, 160, 90);
    pub const BAR_MERGED: Color32 = Color32::from_rgb(80, 200, 100);
    pub const BAR_CURSOR: Color32 = Color32::from_rgb(90, 140, 220);
    pub const BAR_ANCHOR: Color32 = Color32::from_rgb(220, 160, 40);
    pub const BAR_SEARCH: Color32 = Color32::from_rgb(180, 150, 40);
    pub const TEXT_NORMAL: Color32 = Color32::from_rgb(195, 200, 210);
    pub const TEXT_DIM: Color32 = Color32::from_gray(100);
    pub const TEXT_MATCH: Color32 = Color32::from_rgb(180, 235, 195);
    pub const TEXT_MERGED: Color32 = Color32::from_rgb(150, 240, 165);
    pub const TEXT_ANCHOR: Color32 = Color32::from_rgb(255, 205, 70);
    pub const TEXT_SEARCH: Color32 = Color32::from_rgb(240, 225, 140);
    pub const TEXT_DELETE: Color32 = Color32::from_rgb(220, 100, 100);
    pub const TEXT_INSERT: Color32 = Color32::from_rgb(100, 210, 120);
    pub const TEXT_LNUM: Color32 = Color32::from_gray(60);
    pub const TEXT_LNUM_ACTIVE: Color32 = Color32::from_rgb(80, 160, 100);
    pub const ACCENT_GOOD: Color32 = Color32::from_rgb(80, 200, 80);
    pub const ACCENT_WARN: Color32 = Color32::from_rgb(220, 180, 50);
    pub const ACCENT_BAD: Color32 = Color32::from_rgb(220, 80, 80);
    pub const ACCENT_INFO: Color32 = Color32::from_rgb(100, 160, 230);
    pub const HUNK_APPLIED: Color32 = Color32::from_rgb(60, 140, 80);
    pub const HUNK_CURRENT: Color32 = Color32::from_rgb(100, 150, 230);
    pub const HUNK_PENDING: Color32 = Color32::from_rgb(60, 65, 75);
    pub const HUNK_CONFLICT: Color32 = Color32::from_rgb(180, 60, 60);
    pub const SEPARATOR: Color32 = Color32::from_rgb(40, 45, 55);
}

#[derive(Clone, Debug)]
enum Action {
    DeleteLines(usize),
}

#[derive(Clone)]
struct SearchRow {
    text: String,
    file_idx: Option<usize>,
    kind: RowKind,
}

/// Flash message with optional urgency level.
#[derive(Clone)]
struct StatusMessage {
    text: String,
    kind: MessageKind,
}

#[derive(Clone, PartialEq)]
enum MessageKind {
    Info,
    Success,
    Warning,
    Error,
}

impl StatusMessage {
    fn info(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            kind: MessageKind::Info,
        }
    }
    fn success(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            kind: MessageKind::Success,
        }
    }
    fn warning(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            kind: MessageKind::Warning,
        }
    }
    fn error(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            kind: MessageKind::Error,
        }
    }
    fn color(&self) -> Color32 {
        match self.kind {
            MessageKind::Info => pal::ACCENT_INFO,
            MessageKind::Success => pal::ACCENT_GOOD,
            MessageKind::Warning => pal::ACCENT_WARN,
            MessageKind::Error => pal::ACCENT_BAD,
        }
    }
}

/// Tracks per-file state for multi-file patch sets.
#[derive(Clone)]
struct FileState {
    lines: Vec<String>,
    applied_hunks: HashSet<usize>,
    history: Vec<(Vec<String>, usize)>,
    merged_range: Option<(usize, usize)>,
}

pub struct MergeApp {
    patch_text: String,
    hunks: Vec<PatchHunk>,
    current_hunk: usize,
    file_text: String,
    file_lines: Vec<String>,
    file_path: String,
    base_dir: String,
    match_result: Option<MatchResult>,
    search_rows: Vec<SearchRow>,
    file_search_query: String,
    file_search_matches: HashSet<usize>,
    manual_anchor: Option<usize>,
    anchor_matches: Vec<usize>,
    anchor_match_idx: usize,
    candidate_index: usize,
    scroll_to_match: bool,
    message: Option<StatusMessage>,
    // Message auto-dismiss timer (set to ctx.input time + duration)
    message_until: Option<f64>,
    cursor_line: Option<usize>,
    applied_hunks: HashSet<usize>,
    merged_range: Option<(usize, usize)>,
    history: Vec<(Vec<String>, usize)>,
    vim_buffer: String,
    last_action: Option<Action>,
    // Per-file state so switching files doesn't lose edits
    file_states: HashMap<String, FileState>,
    // Show keyboard shortcut overlay
    show_help: bool,
    // Hunk minimap: whether to show the sidebar
    show_minimap: bool,
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
            file_search_matches: HashSet::new(),
            manual_anchor: None,
            anchor_matches: Vec::new(),
            anchor_match_idx: 0,
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

    fn set_message(&mut self, msg: StatusMessage) {
        self.message = Some(msg);
        // Messages auto-dismiss after 6 s (cleared in update via elapsed time)
        self.message_until = None; // will be set on first render when we have ctx time
    }

    fn reparse(&mut self) {
        // Save current file state before reparsing
        self.save_file_state();
        self.hunks = patch::parse_patches(&self.patch_text);
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

    /// Persist current file edits into `file_states` keyed by path.
    fn save_file_state(&mut self) {
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

    fn load_hunk(&mut self) {
        let hunk = match self.hunks.get(self.current_hunk) {
            Some(h) => h.clone(),
            None => return,
        };

        let path = std::path::Path::new(&self.base_dir)
            .join(&hunk.filename)
            .display()
            .to_string();

        if path != self.file_path {
            // Persist edits for the old file before switching
            self.save_file_state();

            self.file_path = path.clone();

            // Restore saved state for this file if we've visited it before
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

        self.manual_anchor = None;
        self.anchor_matches.clear();
        self.anchor_match_idx = 0;
        self.file_search_query.clear();
        self.file_search_matches.clear();
        self.candidate_index = 0;
        self.cursor_line = None;
        self.scroll_to_match = true;
        self.vim_buffer.clear();
        self.last_action = None;
        self.recompute_match();
    }

    fn recompute_match(&mut self) {
        let hunk = match self.hunks.get(self.current_hunk) {
            Some(h) => h,
            None => {
                self.match_result = None;
                self.search_rows = Vec::new();
                return;
            }
        };

        if self.file_lines.is_empty() {
            self.match_result = None;
            self.search_rows = Vec::new();
        } else {
            let best = diff::find_best_match(&hunk.search, &self.file_lines);
            let mr = if best.candidates.is_empty() {
                best
            } else {
                let idx = self.candidate_index.min(best.candidates.len() - 1);
                if idx == 0 {
                    best
                } else {
                    let (start, end, _) = best.candidates[idx];
                    let cands = best.candidates.clone();
                    let mut mr =
                        diff::compute_match_for_window(&hunk.search, &self.file_lines, start, end);
                    mr.candidates = cands;
                    mr
                }
            };
            self.search_rows = Self::build_search_rows(hunk, &mr);
            if self.cursor_line.is_none() {
                self.cursor_line = Some(mr.file_start);
            }
            self.match_result = Some(mr);
            self.scroll_to_match = true;
        }
    }

    fn build_search_rows(hunk: &PatchHunk, mr: &MatchResult) -> Vec<SearchRow> {
        let patch_diff = diff::diff_patch(&hunk.search, &hunk.replace);
        let mut rows = Vec::new();
        let mut file_idx = mr.file_start;
        for (kind, left, _right) in &patch_diff {
            match kind {
                RowKind::Equal => {
                    rows.push(SearchRow {
                        text: left.clone().unwrap_or_default(),
                        file_idx: Some(file_idx),
                        kind: RowKind::Equal,
                    });
                    file_idx += 1;
                }
                RowKind::Delete => {
                    rows.push(SearchRow {
                        text: left.clone().unwrap_or_default(),
                        file_idx: Some(file_idx),
                        kind: RowKind::Delete,
                    });
                    file_idx += 1;
                }
                RowKind::Insert => {
                    rows.push(SearchRow {
                        text: left.clone().unwrap_or_default(),
                        file_idx: None,
                        kind: RowKind::Insert,
                    });
                }
            }
        }
        rows
    }

    fn apply_merge(&mut self) {
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

        let (file_start, file_end) = if let Some(anchor) = self.manual_anchor {
            (anchor, anchor)
        } else {
            match &self.match_result {
                Some(m) => (m.file_start, m.file_end),
                None => return,
            }
        };

        // Warn if this range overlaps an already-applied hunk's merged range
        // (simple proximity check — exact overlap tracking would need more bookkeeping)
        if let Some((ms, me)) = self.merged_range {
            let overlaps = file_start < me && file_end > ms;
            if overlaps {
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
        // file_end is exclusive (past-the-end), so this is correct
        output.extend_from_slice(&self.file_lines[file_end..]);

        self.file_lines = output;
        self.merged_range = Some((replace_start, replace_end));
        self.applied_hunks.insert(self.current_hunk);
        self.manual_anchor = None;
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

        // Auto-advance to next unapplied hunk (skip already-applied ones)
        // User can also navigate manually; this is a convenience only
        // (commented out — can be enabled if desired)
        // self.advance_to_next_unapplied();
    }

    /// Jump to the next hunk that hasn't been applied yet.
    fn advance_to_next_unapplied(&mut self) {
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
        // All applied
        self.set_message(StatusMessage::success("All hunks applied!"));
    }

    fn undo(&mut self) {
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
        } else {
            self.set_message(StatusMessage::warning("Nothing to undo"));
        }
    }

    fn delete_lines(&mut self, count: usize) {
        if let Some(start) = self.cursor_line {
            if start < self.file_lines.len() {
                self.history
                    .push((self.file_lines.clone(), self.current_hunk));
                let end = (start + count).min(self.file_lines.len());
                self.file_lines.drain(start..end);
                self.merged_range = None;
                self.recompute_match();
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

    fn save_file(&mut self) {
        let content = self.file_lines.join("\n");
        let path = if self.file_path.is_empty() {
            "merged_output.txt".to_string()
        } else {
            self.file_path.clone()
        };
        match std::fs::write(&path, &content) {
            Ok(_) => {
                // Persist state so we know it's saved
                self.save_file_state();
                self.set_message(StatusMessage::success(format!("Saved → {}", path)));
            }
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Save failed: {}", e)));
            }
        }
    }

    /// Save all modified files in `file_states` plus the currently-open file.
    fn save_all_files(&mut self) {
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

    fn current_hunk(&self) -> Option<&PatchHunk> {
        self.hunks.get(self.current_hunk)
    }

    fn truncate_owned(text: &str, max_chars: usize) -> String {
        if text.chars().count() > max_chars {
            let mut s: String = text.chars().take(max_chars.saturating_sub(1)).collect();
            s.push('…');
            s
        } else {
            text.to_string()
        }
    }

    /// Score → (background tint, foreground color, icon)
    fn score_appearance(score: f32) -> (Color32, Color32, &'static str) {
        if score >= 95.0 {
            (Color32::from_rgb(20, 70, 30), pal::ACCENT_GOOD, "✓✓")
        } else if score >= 80.0 {
            (Color32::from_rgb(18, 60, 25), pal::ACCENT_GOOD, "✓")
        } else if score >= 60.0 {
            (Color32::from_rgb(60, 55, 15), pal::ACCENT_WARN, "≈")
        } else if score >= 40.0 {
            (
                Color32::from_rgb(70, 40, 10),
                Color32::from_rgb(230, 150, 50),
                "~",
            )
        } else {
            (Color32::from_rgb(70, 20, 20), pal::ACCENT_BAD, "✗")
        }
    }

    /// Count hunks for each state across all hunk indices.
    fn hunk_summary(&self) -> (usize, usize, usize) {
        // (applied, pending, total)
        let applied = self.applied_hunks.len();
        let total = self.hunks.len();
        (applied, total - applied, total)
    }
}

// ── eframe::App ──────────────────────────────────────────────────────────────

impl eframe::App for MergeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Auto-dismiss messages after 6 seconds of wall-clock time.
        // We use egui's time because it's always available.
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

        // Keyboard shortcuts that work globally (not swallowed by text fields)
        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                if i.key_pressed(Key::Escape) {
                    // Escape clears help overlay, then anchor
                    if self.show_help {
                        self.show_help = false;
                    } else if self.manual_anchor.is_some() {
                        self.manual_anchor = None;
                        self.anchor_matches.clear();
                        self.scroll_to_match = true;
                    }
                }
                // ? shows help
                if i.events
                    .iter()
                    .any(|e| matches!(e, Event::Text(t) if t == "?"))
                {
                    self.show_help = !self.show_help;
                }
                // M toggles minimap
                if i.events
                    .iter()
                    .any(|e| matches!(e, Event::Text(t) if t == "m" || t == "M"))
                {
                    self.show_minimap = !self.show_minimap;
                }
            });
        }

        self.render_toolbar(ctx);
        self.render_status_bar(ctx);

        // Help overlay
        if self.show_help {
            self.render_help_overlay(ctx);
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
                            .color(pal::TEXT_DIM)
                            .small(),
                    );
                });
                return;
            }
            if self.show_minimap {
                self.render_with_minimap(ui);
            } else {
                SidePanel::left("minimap_collapsed")
                    .resizable(false)
                    .exact_width(0.0)
                    .show_inside(ui, |_| {});
                self.render_split_view(ui);
            }
        });
    }
}

// ── Toolbar & Status Bar ─────────────────────────────────────────────────────

impl MergeApp {
    fn render_toolbar(&mut self, ctx: &Context) {
        TopBottomPanel::top("toolbar")
            .frame(
                Frame::none()
                    .fill(pal::BG_TOOLBAR)
                    .inner_margin(Margin::symmetric(8.0, 4.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
                    ui.spacing_mut().item_spacing.x = 6.0;

                    ui.label(
                        RichText::new("patch·merge")
                            .color(pal::ACCENT_INFO)
                            .strong()
                            .monospace(),
                    );
                    ui.add(Separator::default().vertical().spacing(12.0));

                    // File open controls
                    if ui.button("Open Patch…").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Patch", &["md", "txt"])
                            .pick_file()
                        {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                self.patch_text = content;
                                if let Some(parent) = path.parent() {
                                    self.base_dir = parent.display().to_string();
                                }
                                self.reparse();
                            }
                        }
                    }

                    if ui.button("Open File…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                self.file_text = content;
                                self.file_lines =
                                    self.file_text.lines().map(String::from).collect();
                                self.file_path = path.display().to_string();
                                if let Some(parent) = path.parent() {
                                    self.base_dir = parent.display().to_string();
                                }
                                self.applied_hunks.clear();
                                self.merged_range = None;
                                self.history.clear();
                                self.vim_buffer.clear();
                                self.manual_anchor = None;
                                self.file_search_query.clear();
                                self.file_search_matches.clear();
                                self.candidate_index = 0;
                                self.cursor_line = None;
                                self.scroll_to_match = true;
                                self.recompute_match();
                            }
                        }
                    }

                    ui.add(Separator::default().vertical().spacing(12.0));

                    // Match score badge
                    if let Some(ref mr) = self.match_result {
                        let (bg, fg, icon) = Self::score_appearance(mr.score);
                        let frame = Frame::none()
                            .fill(bg)
                            .stroke(Stroke::new(1.0, fg))
                            .rounding(Rounding::same(4.0))
                            .inner_margin(Margin::symmetric(8.0, 3.0));
                        frame.show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{:.0}% {}", mr.score, icon))
                                    .color(fg)
                                    .strong()
                                    .monospace(),
                            );
                        });
                        ui.add(Separator::default().vertical().spacing(12.0));
                    }

                    // Hunk progress (applied / total)
                    let (applied, pending, total) = self.hunk_summary();
                    if total > 0 {
                        let frac = applied as f32 / total as f32;
                        let bar_w = 80.0_f32;
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(bar_w, 16.0), Sense::hover());
                        ui.painter().rect_filled(rect, 3.0, pal::HUNK_PENDING);
                        let filled =
                            Rect::from_min_size(rect.min, Vec2::new(bar_w * frac, rect.height()));
                        ui.painter().rect_filled(filled, 3.0, pal::HUNK_APPLIED);
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            format!("{}/{}", applied, total),
                            FontId::monospace(10.0),
                            Color32::WHITE,
                        );
                        ui.label(
                            RichText::new(if pending == 0 { "all done" } else { "hunks" })
                                .color(pal::TEXT_DIM)
                                .small(),
                        );
                        ui.add(Separator::default().vertical().spacing(12.0));
                    }

                    // Save controls
                    let has_unsaved = !self.applied_hunks.is_empty();
                    ui.add_enabled_ui(has_unsaved, |ui| {
                        if ui
                            .button(RichText::new("💾 Save").color(if has_unsaved {
                                pal::ACCENT_GOOD
                            } else {
                                pal::TEXT_DIM
                            }))
                            .on_hover_text("Save current file (Ctrl+S)")
                            .clicked()
                        {
                            self.save_file();
                        }
                    });

                    if ui
                        .button("💾 Save All")
                        .on_hover_text("Save every modified file")
                        .clicked()
                    {
                        self.save_all_files();
                    }

                    ui.add(Separator::default().vertical().spacing(12.0));

                    // Minimap toggle + help
                    let minimap_label = if self.show_minimap {
                        "▪ Map"
                    } else {
                        "□ Map"
                    };
                    if ui
                        .button(minimap_label)
                        .on_hover_text("Toggle hunk minimap (M)")
                        .clicked()
                    {
                        self.show_minimap = !self.show_minimap;
                    }

                    if ui.button("?").on_hover_text("Keyboard shortcuts").clicked() {
                        self.show_help = !self.show_help;
                    }
                });
            });
    }

    fn render_status_bar(&self, ctx: &Context) {
        TopBottomPanel::bottom("status")
            .frame(
                Frame::none()
                    .fill(pal::BG_TOOLBAR)
                    .inner_margin(Margin::symmetric(8.0, 3.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Always show file path on the left
                    if let Some(hunk) = self.current_hunk() {
                        ui.label(
                            RichText::new(format!("📄 {}", hunk.filename))
                                .color(pal::TEXT_NORMAL)
                                .monospace(),
                        );
                        if let Some(ref mr) = self.match_result {
                            ui.add(Separator::default().vertical());
                            ui.label(
                                RichText::new(format!(
                                    "match {}–{}  │  search {} ln  │  replace {} ln",
                                    mr.file_start + 1,
                                    mr.file_end,
                                    hunk.search.len(),
                                    hunk.replace.len()
                                ))
                                .color(pal::TEXT_DIM)
                                .monospace()
                                .small(),
                            );
                        }
                    }

                    // Cursor position
                    if let Some(line) = self.cursor_line {
                        ui.add(Separator::default().vertical());
                        ui.label(
                            RichText::new(format!("ln {}  of {}", line + 1, self.file_lines.len()))
                                .color(pal::TEXT_DIM)
                                .monospace()
                                .small(),
                        );
                    }

                    // Vim buffer indicator — fixed width, right-aligned within the bar
                    if !self.vim_buffer.is_empty() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("  {}█", self.vim_buffer))
                                    .color(Color32::from_rgb(200, 200, 100))
                                    .monospace(),
                            );
                        });
                    }

                    // Flash message — shown in its own row when present, overlaid below
                    // (We append it at the end of the horizontal so it doesn't collapse hunk info)
                    if let Some(ref msg) = self.message {
                        ui.add(Separator::default().vertical());
                        ui.label(RichText::new(&msg.text).color(msg.color()).small());
                    }
                });
            });
    }

    fn render_help_overlay(&mut self, ctx: &Context) {
        let screen = ctx.screen_rect();
        let overlay_w = 420.0_f32;
        let overlay_h = 380.0_f32;
        let pos = Pos2::new(
            (screen.center().x - overlay_w / 2.0).max(8.0),
            (screen.center().y - overlay_h / 2.0).max(8.0),
        );

        let overlay_rect = Rect::from_min_size(pos, Vec2::new(overlay_w, overlay_h));
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("help_overlay")));
        painter.rect_filled(screen, 0.0, Color32::from_black_alpha(160));
        painter.rect_filled(overlay_rect, 6.0, Color32::from_rgb(22, 28, 38));
        painter.rect_stroke(overlay_rect, 6.0, Stroke::new(1.0, pal::SEPARATOR));

        Area::new(Id::new("help_area"))
            .fixed_pos(pos)
            .show(ctx, |ui| {
                ui.set_min_size(Vec2::new(overlay_w, overlay_h));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("Keyboard shortcuts")
                            .color(pal::ACCENT_INFO)
                            .strong()
                            .heading(),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        if ui.button("✕").clicked() {
                            self.show_help = false;
                        }
                    });
                });
                ui.add_space(8.0);
                ui.add(Separator::default());
                ui.add_space(4.0);

                let shortcuts: &[(&str, &str)] = &[
                    ("Navigation", ""),
                    ("↑ / ↓", "Move cursor one line"),
                    ("PgUp / PgDn", "Move cursor 10 lines"),
                    ("Home / End", "Jump to first / last line"),
                    ("gg / G", "Jump to top / bottom (vim)"),
                    ("", ""),
                    ("Hunk control", ""),
                    ("L", "Next hunk"),
                    ("Shift+L", "Previous hunk"),
                    ("◀ ▶ (toolbar)", "Navigate candidates"),
                    ("", ""),
                    ("Editing", ""),
                    ("A", "Apply current hunk (when cursor is in match)"),
                    ("dd / Ndd", "Delete 1 or N lines at cursor"),
                    ("u", "Undo last edit"),
                    (".", "Repeat last action"),
                    ("", ""),
                    ("UI", ""),
                    ("?", "Toggle this help"),
                    ("M", "Toggle hunk minimap"),
                    ("Esc", "Close help / clear anchor"),
                ];

                ScrollArea::vertical()
                    .max_height(overlay_h - 100.0)
                    .show(ui, |ui| {
                        for (key, desc) in shortcuts {
                            if desc.is_empty() {
                                if !key.is_empty() {
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(*key).color(pal::TEXT_DIM).small().strong(),
                                    );
                                } else {
                                    ui.add_space(2.0);
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    ui.add_space(12.0);
                                    let key_rect = ui.allocate_exact_size(
                                        Vec2::new(130.0, 18.0),
                                        Sense::hover(),
                                    );
                                    ui.painter_at(key_rect.0).text(
                                        key_rect.0.left_center(),
                                        Align2::LEFT_CENTER,
                                        *key,
                                        FontId::monospace(11.0),
                                        Color32::from_rgb(180, 210, 255),
                                    );
                                    ui.label(RichText::new(*desc).color(pal::TEXT_NORMAL).small());
                                });
                            }
                        }
                    });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("Esc or click ✕ to close")
                            .color(pal::TEXT_DIM)
                            .small(),
                    );
                });
            });
    }
}

// ── Minimap sidebar ───────────────────────────────────────────────────────────

impl MergeApp {
    fn render_with_minimap(&mut self, ui: &mut Ui) {
        let available = ui.available_size();
        let minimap_w = 160.0_f32;

        ui.horizontal(|ui| {
            // Minimap panel
            let (minimap_rect, _) =
                ui.allocate_exact_size(Vec2::new(minimap_w, available.y), Sense::hover());
            let mut minimap_ui = ui.child_ui(minimap_rect, Layout::top_down(Align::LEFT), None);
            self.render_minimap(&mut minimap_ui, minimap_w, available.y);

            ui.add(Separator::default().vertical());

            // Main split view
            self.render_split_view(ui);
        });
    }

    fn render_minimap(&mut self, ui: &mut Ui, w: f32, h: f32) {
        Frame::none()
            .fill(Color32::from_rgb(18, 22, 28))
            .inner_margin(Margin::symmetric(6.0, 6.0))
            .show(ui, |ui| {
                ui.set_min_width(w - 12.0);
                ui.set_max_width(w - 12.0);

                ui.label(RichText::new("HUNKS").color(pal::TEXT_DIM).small().strong());
                ui.add_space(4.0);

                let n = self.hunks.len();
                if n == 0 {
                    return;
                }

                // Group hunks by file
                let row_h = ((h - 60.0) / (n as f32 + 1.0)).clamp(18.0, 32.0);
                let mut jump_to: Option<usize> = None;

                for idx in 0..n {
                    let is_current = idx == self.current_hunk;
                    let is_applied = self.applied_hunks.contains(&idx);
                    let hunk = &self.hunks[idx];

                    let (bg, fg) = if is_current {
                        (Color32::from_rgb(30, 45, 75), pal::HUNK_CURRENT)
                    } else if is_applied {
                        (Color32::from_rgb(18, 35, 22), pal::HUNK_APPLIED)
                    } else {
                        (Color32::from_rgb(25, 28, 34), pal::TEXT_DIM)
                    };

                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());

                    if resp.hovered() {
                        ui.painter()
                            .rect_filled(rect, 3.0, Color32::from_rgb(35, 42, 58));
                    } else {
                        ui.painter().rect_filled(rect, 3.0, bg);
                    }

                    // Left accent bar
                    let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                    ui.painter().rect_filled(bar, 0.0, fg);

                    // Hunk number
                    ui.painter().text(
                        Pos2::new(rect.left() + 8.0, rect.center().y - 5.0),
                        Align2::LEFT_CENTER,
                        format!("{}", idx + 1),
                        FontId::monospace(10.0),
                        if is_current { Color32::WHITE } else { fg },
                    );

                    // Status icon
                    let icon = if is_applied {
                        "✓"
                    } else if is_current {
                        "▶"
                    } else {
                        "○"
                    };
                    ui.painter().text(
                        Pos2::new(rect.left() + 8.0, rect.center().y + 6.0),
                        Align2::LEFT_CENTER,
                        icon,
                        FontId::monospace(9.0),
                        fg,
                    );

                    // File name (truncated)
                    let fname = std::path::Path::new(&hunk.filename)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&hunk.filename);
                    let max_fname = ((w - 30.0) / 6.5).floor() as usize;
                    ui.painter().text(
                        Pos2::new(rect.left() + 22.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        Self::truncate_owned(fname, max_fname),
                        FontId::monospace(9.5),
                        if is_current {
                            pal::TEXT_NORMAL
                        } else {
                            Color32::from_gray(120)
                        },
                    );

                    if resp.clicked() {
                        jump_to = Some(idx);
                    }

                    // Hover tooltip
                    if resp.hovered() {
                        resp.on_hover_ui_at_pointer(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "#{} {}{}",
                                    idx + 1,
                                    hunk.filename,
                                    if is_applied { " ✓ applied" } else { "" }
                                ))
                                .monospace()
                                .small(),
                            );
                            ui.label(format!(
                                "search {} ln  →  replace {} ln",
                                hunk.search.len(),
                                hunk.replace.len()
                            ));
                        });
                    }
                }

                if let Some(idx) = jump_to {
                    self.current_hunk = idx;
                    self.load_hunk();
                }
            });
    }
}

// ── Split view ────────────────────────────────────────────────────────────────

impl MergeApp {
    fn render_split_view(&mut self, ui: &mut Ui) {
        let mr = match self.match_result.clone() {
            Some(m) => m,
            None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("No file loaded or no match found.")
                            .color(Color32::from_gray(140)),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Open a file or patch above")
                            .color(pal::TEXT_DIM)
                            .small(),
                    );
                });
                return;
            }
        };

        let available = ui.available_size();
        let divider = 0.38_f32;
        let left_w = (available.x * divider).floor() - 1.0;
        let right_w = available.x - left_w - 2.0;
        let mono_h = ui.text_style_height(&TextStyle::Monospace);
        let row_h = mono_h + 4.0;
        let char_w = mono_h * 0.60;

        // Panel header row
        ui.horizontal(|ui| {
            Frame::none()
                .fill(Color32::from_rgb(28, 38, 58))
                .inner_margin(Margin::symmetric(8.0, 3.0))
                .show(ui, |ui| {
                    ui.set_min_width(left_w);
                    ui.set_max_width(left_w);
                    let hunk = self.current_hunk().unwrap();
                    ui.label(
                        RichText::new(format!("SEARCH  ·  {}", hunk.filename))
                            .color(Color32::from_rgb(120, 180, 255))
                            .strong()
                            .monospace(),
                    );
                });
            ui.add_space(2.0);
            Frame::none()
                .fill(Color32::from_rgb(28, 45, 35))
                .inner_margin(Margin::symmetric(8.0, 3.0))
                .show(ui, |ui| {
                    ui.set_min_width(right_w);
                    ui.label(
                        RichText::new(format!(
                            "FILE  ·  {} lines  ·  match @ {}–{}",
                            self.file_lines.len(),
                            mr.file_start + 1,
                            mr.file_end,
                        ))
                        .color(Color32::from_rgb(120, 220, 160))
                        .strong()
                        .monospace(),
                    );
                });
        });

        ui.add(Separator::default());

        let body_rect = ui.available_rect_before_wrap();
        let mut left_rect = body_rect;
        left_rect.set_width(left_w);
        let mut right_rect = body_rect;
        right_rect.min.x = body_rect.min.x + left_w + 2.0;
        right_rect.set_width(right_w);

        let mut left_ui = ui.child_ui(left_rect, Layout::top_down(Align::LEFT), None);
        self.render_search_panel(&mut left_ui, &mr, row_h, char_w, left_w);

        let mut right_ui = ui.child_ui(right_rect, Layout::top_down(Align::LEFT), None);
        self.render_file_panel(&mut right_ui, &mr, row_h, char_w, right_w);
    }

    fn render_search_panel(
        &self,
        ui: &mut Ui,
        mr: &MatchResult,
        row_h: f32,
        char_w: f32,
        panel_w: f32,
    ) {
        let max_chars = ((panel_w - 58.0) / char_w).floor() as usize;
        ScrollArea::vertical()
            .id_source("search_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let hunk = match self.current_hunk() {
                    Some(h) => h,
                    None => return,
                };
                let is_applied = self.applied_hunks.contains(&self.current_hunk);

                // Banner
                let (banner_bg, banner_fg, _icon) = Self::score_appearance(mr.score);
                let (banner_bg, banner_text) = if is_applied {
                    (
                        Color32::from_rgb(30, 40, 30),
                        format!("✓ Applied — hunk {}", self.current_hunk + 1),
                    )
                } else {
                    (
                        banner_bg,
                        format!(
                            "{:.0}%  match @ lines {}–{}",
                            mr.score,
                            mr.file_start + 1,
                            mr.file_end
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

                // Build per-line lookup: search line → matched file index
                // Use position-based match (search_rows already has this), keyed by (text, occurrence)
                let search_file_map: Vec<Option<usize>> = self
                    .search_rows
                    .iter()
                    .filter(|r| matches!(r.kind, RowKind::Equal | RowKind::Delete))
                    .map(|r| r.file_idx)
                    .collect();

                for (line_idx, line) in hunk.search.iter().enumerate() {
                    let file_idx = search_file_map.get(line_idx).copied().flatten();
                    let is_matched = file_idx.is_some();

                    let (bg, prefix_color, prefix) = if is_matched {
                        (pal::BG_MATCH, pal::TEXT_INSERT, "= ")
                    } else {
                        (pal::BG_DELETE, pal::TEXT_DELETE, "- ")
                    };

                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, bg);

                    // Left accent stripe
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

                    // Line number from file (if matched) else patch index
                    let num_text = if let Some(fi) = file_idx {
                        format!("{:>4}", fi + 1)
                    } else {
                        format!("{:>4}", line_idx + 1)
                    };
                    ui.painter().text(
                        Pos2::new(rect.left() + 4.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &num_text,
                        FontId::monospace(11.0),
                        if is_matched {
                            pal::TEXT_LNUM_ACTIVE
                        } else {
                            pal::TEXT_DIM
                        },
                    );

                    ui.painter().text(
                        Pos2::new(rect.left() + 38.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        prefix,
                        FontId::monospace(11.0),
                        prefix_color,
                    );

                    let display = Self::truncate_owned(line, max_chars);
                    ui.painter().text(
                        Pos2::new(rect.left() + 54.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        FontId::monospace(11.0),
                        if is_applied {
                            pal::TEXT_DIM
                        } else {
                            pal::TEXT_NORMAL
                        },
                    );
                }

                // Replace section
                if !hunk.replace.is_empty() {
                    ui.add_space(4.0);
                    let (sep_rect, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                    ui.painter().rect_filled(sep_rect, 0.0, pal::SEPARATOR);
                    ui.add_space(2.0);

                    let (hdr_rect, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_h),
                        Sense::hover(),
                    );
                    ui.painter()
                        .rect_filled(hdr_rect, 0.0, Color32::from_rgb(22, 44, 28));
                    ui.painter().text(
                        Pos2::new(hdr_rect.left() + 8.0, hdr_rect.center().y),
                        Align2::LEFT_CENTER,
                        "REPLACE →",
                        FontId::monospace(10.0),
                        pal::TEXT_INSERT,
                    );

                    for (line_idx, line) in hunk.replace.iter().enumerate() {
                        let desired = Vec2::new(ui.available_width(), row_h);
                        let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                        ui.painter().rect_filled(rect, 0.0, pal::BG_INSERT);

                        let bar = Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height()));
                        ui.painter().rect_filled(bar, 0.0, pal::BAR_MATCH);

                        ui.painter().text(
                            Pos2::new(rect.left() + 4.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!("{:>4}", line_idx + 1),
                            FontId::monospace(11.0),
                            pal::TEXT_DIM,
                        );
                        ui.painter().text(
                            Pos2::new(rect.left() + 38.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            "+ ",
                            FontId::monospace(11.0),
                            pal::TEXT_INSERT,
                        );
                        let display = Self::truncate_owned(line, max_chars);
                        ui.painter().text(
                            Pos2::new(rect.left() + 54.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            &display,
                            FontId::monospace(11.0),
                            if is_applied {
                                pal::TEXT_DIM
                            } else {
                                Color32::from_rgb(155, 235, 165)
                            },
                        );
                    }
                }
            });
    }

    fn render_file_panel(
        &mut self,
        ui: &mut Ui,
        mr: &MatchResult,
        row_h: f32,
        char_w: f32,
        panel_w: f32,
    ) {
        let max_chars = ((panel_w - 68.0) / char_w).floor() as usize;

        // ── Deferred action flags ─────────────────────────────────────────────
        let mut prev_hunk = false;
        let mut next_hunk = false;
        let mut prev_candidate = false;
        let mut next_candidate = false;
        let mut clear_anchor = false;
        let mut apply_clicked = false;
        let mut find_text = false;
        let mut prev_anchor_match = false;
        let mut next_anchor_match = false;

        let current_hunk_idx = self.current_hunk;
        let total_hunks = self.hunks.len();
        let manual_anchor = self.manual_anchor;
        let candidate_count = mr.candidates.len();
        let candidate_idx = self.candidate_index;
        let is_applied = self.applied_hunks.contains(&self.current_hunk);
        let can_apply =
            !is_applied && (self.match_result.is_some() || self.manual_anchor.is_some());
        let apply_line = if let Some(anchor) = manual_anchor {
            anchor + 1
        } else {
            mr.file_start + 1
        };

        // ── Controls bar ──────────────────────────────────────────────────────
        Frame::none()
            .fill(Color32::from_rgb(25, 32, 42))
            .inner_margin(Margin::symmetric(6.0, 4.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // Hunk navigation
                    ui.label(RichText::new("Hunk:").color(pal::TEXT_DIM).small());
                    if ui
                        .add_enabled(current_hunk_idx > 0, Button::new("◀").small())
                        .on_hover_text("Previous hunk (Shift+L)")
                        .clicked()
                    {
                        prev_hunk = true;
                    }
                    ui.label(
                        RichText::new(format!("{}/{}", current_hunk_idx + 1, total_hunks))
                            .monospace(),
                    );
                    if ui
                        .add_enabled(current_hunk_idx < total_hunks - 1, Button::new("▶").small())
                        .on_hover_text("Next hunk (L)")
                        .clicked()
                    {
                        next_hunk = true;
                    }

                    if is_applied {
                        ui.label(RichText::new("✓").color(pal::ACCENT_GOOD).strong());
                    }

                    ui.add(Separator::default().vertical());

                    // Candidate/anchor navigation
                    if let Some(anchor) = manual_anchor {
                        ui.label(
                            RichText::new(format!("⚓ {}", anchor + 1))
                                .color(pal::TEXT_ANCHOR)
                                .monospace(),
                        );
                        if ui
                            .small_button("✕")
                            .on_hover_text("Clear anchor (Esc)")
                            .clicked()
                        {
                            clear_anchor = true;
                        }
                    } else {
                        ui.label(RichText::new("Cand:").color(pal::TEXT_DIM).small());
                        if ui
                            .add_enabled(candidate_idx > 0, Button::new("◀").small())
                            .clicked()
                        {
                            prev_candidate = true;
                        }
                        ui.label(
                            RichText::new(format!(
                                "{}/{}",
                                candidate_idx + 1,
                                candidate_count.max(1)
                            ))
                            .monospace(),
                        );
                        if ui
                            .add_enabled(
                                candidate_idx + 1 < candidate_count,
                                Button::new("▶").small(),
                            )
                            .clicked()
                        {
                            next_candidate = true;
                        }
                    }

                    ui.add(Separator::default().vertical());

                    // Text search
                    ui.label(RichText::new("🔍").monospace());
                    let resp = ui.add(
                        TextEdit::singleline(&mut self.file_search_query)
                            .hint_text("find…")
                            .desired_width(72.0)
                            .font(TextStyle::Monospace),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        find_text = true;
                    }
                    if ui.small_button("Go").clicked() {
                        find_text = true;
                    }

                    if !self.anchor_matches.is_empty() {
                        ui.label(
                            RichText::new(format!(
                                "{}/{}",
                                self.anchor_match_idx + 1,
                                self.anchor_matches.len()
                            ))
                            .color(pal::TEXT_ANCHOR)
                            .small()
                            .monospace(),
                        );
                        if ui.small_button("◀").clicked() {
                            prev_anchor_match = true;
                        }
                        if ui.small_button("▶").clicked() {
                            next_anchor_match = true;
                        }
                    }

                    ui.add(Separator::default().vertical());

                    // Apply button — prominent
                    ui.add_enabled_ui(can_apply, |ui| {
                        let btn_text = if is_applied {
                            "✓ Applied".to_string()
                        } else {
                            format!("⚡ Apply @ {}", apply_line)
                        };
                        let btn = Button::new(RichText::new(&btn_text).strong().monospace()).fill(
                            if can_apply {
                                Color32::from_rgb(40, 90, 55)
                            } else {
                                Color32::from_gray(35)
                            },
                        );
                        if ui
                            .add(btn)
                            .on_hover_text(
                                "Apply this hunk to the file (A when cursor is in match)",
                            )
                            .clicked()
                        {
                            apply_clicked = true;
                        }
                    });

                    // Skip (advance without applying)
                    if current_hunk_idx < total_hunks - 1 {
                        if ui
                            .small_button("Skip →")
                            .on_hover_text("Skip to next hunk without applying")
                            .clicked()
                        {
                            next_hunk = true;
                        }
                    }
                });
            });

        ui.add(Separator::default());

        // ── Keyboard input ───────────────────────────────────────────────────
        let len = self.file_lines.len();
        let mut go_next_hunk = false;
        let mut go_prev_hunk = false;

        if len > 0 && !ui.ctx().wants_keyboard_input() {
            let mut cursor_changed = false;
            let mut new_text = String::new();

            ui.input(|i| {
                let cur = self.cursor_line.unwrap_or(0);

                if i.key_pressed(Key::ArrowDown) {
                    self.cursor_line = Some((cur + 1).min(len - 1));
                    cursor_changed = true;
                }
                if i.key_pressed(Key::ArrowUp) {
                    self.cursor_line = Some(cur.saturating_sub(1));
                    cursor_changed = true;
                }
                if i.key_pressed(Key::PageDown) {
                    self.cursor_line = Some((cur + 20).min(len - 1));
                    cursor_changed = true;
                }
                if i.key_pressed(Key::PageUp) {
                    self.cursor_line = Some(cur.saturating_sub(20));
                    cursor_changed = true;
                }
                if i.key_pressed(Key::Home) {
                    self.cursor_line = Some(0);
                    cursor_changed = true;
                }
                if i.key_pressed(Key::End) {
                    self.cursor_line = Some(len - 1);
                    cursor_changed = true;
                }

                // Ctrl+S → save
                if i.key_pressed(Key::S) && i.modifiers.ctrl {
                    // handled after borrow ends
                }

                if i.key_pressed(Key::L) {
                    if i.modifiers.shift {
                        go_prev_hunk = true;
                    } else {
                        go_next_hunk = true;
                    }
                }

                // A → apply when cursor is within match region
                if i.key_pressed(Key::A) {
                    let in_hunk = if let Some(anchor) = manual_anchor {
                        cur == anchor
                    } else {
                        cur >= mr.file_start && cur < mr.file_end
                    };
                    if is_applied {
                        // message set below
                    } else if in_hunk {
                        apply_clicked = true;
                    } else {
                        // Move cursor into the match region and re-try next press
                        self.cursor_line = Some(mr.file_start);
                        cursor_changed = true;
                    }
                }

                for event in i.events.clone() {
                    if let Event::Text(txt) = event {
                        // Filter out '?' and 'm' which are handled globally
                        if txt != "?" && txt != "m" && txt != "M" {
                            new_text.push_str(&txt);
                        }
                    }
                }
            });

            // Vim buffer
            if !new_text.is_empty() {
                self.vim_buffer.push_str(&new_text);
                let buf = self.vim_buffer.trim().to_string();
                let lower_buf = buf.to_lowercase();
                let mut clear_buffer = false;

                if lower_buf == "u" {
                    self.undo();
                    clear_buffer = true;
                } else if lower_buf == "." {
                    if let Some(action) = self.last_action.clone() {
                        match action {
                            Action::DeleteLines(count) => self.delete_lines(count),
                        }
                    }
                    clear_buffer = true;
                } else if buf == "gg" {
                    self.cursor_line = Some(0);
                    self.scroll_to_match = true;
                    clear_buffer = true;
                } else if buf == "G" {
                    self.cursor_line = Some(self.file_lines.len().saturating_sub(1));
                    self.scroll_to_match = true;
                    clear_buffer = true;
                } else if lower_buf.ends_with("dd") {
                    let num_part = &lower_buf[..lower_buf.len() - 2];
                    let count = if num_part.is_empty() {
                        1
                    } else {
                        num_part.parse::<usize>().unwrap_or(0)
                    };
                    if count > 0 {
                        self.delete_lines(count);
                        self.last_action = Some(Action::DeleteLines(count));
                    }
                    clear_buffer = true;
                } else if buf.len() > 5 {
                    clear_buffer = true;
                } else {
                    let allowed = buf.chars().all(|c| {
                        c.is_ascii_digit() || c == 'd' || c == 'D' || c == 'g' || c == 'G'
                    });
                    let d_count = buf.matches('d').count() + buf.matches('D').count();
                    if !allowed || d_count > 2 {
                        clear_buffer = true;
                    }
                }

                if clear_buffer {
                    self.vim_buffer.clear();
                }
            }

            if cursor_changed {
                self.scroll_to_match = true;
            }
        }

        // ── Deferred mutations ────────────────────────────────────────────────
        if prev_hunk && current_hunk_idx > 0 {
            self.current_hunk -= 1;
            self.load_hunk();
            return;
        }
        if next_hunk && current_hunk_idx < total_hunks - 1 {
            self.current_hunk += 1;
            self.load_hunk();
            return;
        }

        if clear_anchor {
            self.manual_anchor = None;
            self.anchor_matches.clear();
            self.scroll_to_match = true;
        }

        if prev_candidate && self.candidate_index > 0 {
            self.candidate_index -= 1;
            // Don't reset cursor_line — keep user's position
            self.scroll_to_match = true;
            self.recompute_match();
            return;
        }
        if next_candidate && self.candidate_index + 1 < candidate_count {
            self.candidate_index += 1;
            self.scroll_to_match = true;
            self.recompute_match();
            return;
        }

        if find_text {
            let q = self.file_search_query.trim().to_lowercase();
            if !q.is_empty() {
                self.anchor_matches = self
                    .file_lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| l.to_lowercase().contains(&q))
                    .map(|(i, _)| i)
                    .collect();
                if !self.anchor_matches.is_empty() {
                    self.anchor_match_idx = 0;
                    self.manual_anchor = Some(self.anchor_matches[0]);
                    self.scroll_to_match = true;
                } else {
                    self.manual_anchor = None;
                    self.set_message(StatusMessage::warning(format!("No matches for '{}'", q)));
                }
            }
        }

        if prev_anchor_match && !self.anchor_matches.is_empty() {
            if self.anchor_match_idx > 0 {
                self.anchor_match_idx -= 1;
            } else {
                self.anchor_match_idx = self.anchor_matches.len() - 1;
            }
            self.manual_anchor = Some(self.anchor_matches[self.anchor_match_idx]);
            self.scroll_to_match = true;
        }
        if next_anchor_match && !self.anchor_matches.is_empty() {
            self.anchor_match_idx = (self.anchor_match_idx + 1) % self.anchor_matches.len();
            self.manual_anchor = Some(self.anchor_matches[self.anchor_match_idx]);
            self.scroll_to_match = true;
        }

        if go_next_hunk {
            if self.current_hunk < self.hunks.len() - 1 {
                self.current_hunk += 1;
                self.load_hunk();
                return;
            } else {
                self.cursor_line = Some(mr.file_start);
                self.scroll_to_match = true;
            }
        }
        if go_prev_hunk {
            if self.current_hunk > 0 {
                self.current_hunk -= 1;
                self.load_hunk();
                return;
            } else {
                self.cursor_line = Some(mr.file_start);
                self.scroll_to_match = true;
            }
        }

        // ── Snapshot local state for the scroll loop ─────────────────────────
        let file_lines = self.file_lines.clone();
        let manual_anchor_check = self.manual_anchor;
        let merged_range = self.merged_range;
        let auto_start = mr.file_start;
        let auto_end = mr.file_end;
        let auto_score = mr.score;
        let search_query = self.file_search_query.clone();
        let scroll_to_match = self.scroll_to_match;
        let cursor_line = self.cursor_line;
        let mut did_scroll = false;
        let mut set_anchor: Option<usize> = None;
        let mut set_cursor: Option<usize> = None;

        // Build a set of file-line indices that appear in the diff as deletions
        // so we can mark them inline in the file view.
        let delete_file_indices: HashSet<usize> = self
            .search_rows
            .iter()
            .filter(|r| matches!(r.kind, RowKind::Delete))
            .filter_map(|r| r.file_idx)
            .collect();
        let equal_file_indices: HashSet<usize> = self
            .search_rows
            .iter()
            .filter(|r| matches!(r.kind, RowKind::Equal))
            .filter_map(|r| r.file_idx)
            .collect();

        // ── File line scroll area ─────────────────────────────────────────────
        ScrollArea::both()
            .id_source("file_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, line) in file_lines.iter().enumerate() {
                    let in_auto_match = i >= auto_start && i < auto_end;
                    let is_anchor = manual_anchor_check == Some(i);
                    let is_cursor = cursor_line == Some(i);
                    let in_merged = merged_range.map_or(false, |(rs, re)| i >= rs && i < re);
                    let is_delete = in_auto_match && delete_file_indices.contains(&i);
                    let is_equal = in_auto_match && equal_file_indices.contains(&i);
                    let is_search_hit = !search_query.is_empty()
                        && line.to_lowercase().contains(&search_query.to_lowercase());

                    // Row height: slightly taller for anchor and match-start banners
                    let row_is_tall = is_anchor
                        || (in_auto_match && i == auto_start && manual_anchor_check.is_none());
                    let desired = Vec2::new(
                        ui.available_width(),
                        if row_is_tall { row_h + 6.0 } else { row_h },
                    );
                    let (rect, row_resp) = ui.allocate_exact_size(desired, Sense::click());

                    // Scroll to bring this row into view
                    let should_scroll = scroll_to_match
                        && (is_cursor
                            || (cursor_line.is_none() && is_anchor)
                            || (cursor_line.is_none()
                                && manual_anchor_check.is_none()
                                && i == auto_start));
                    if should_scroll {
                        ui.scroll_to_rect(rect, Some(Align::Center));
                        did_scroll = true;
                    }

                    // ── Anchor row ────────────────────────────────────────────
                    if is_anchor {
                        ui.painter().rect_filled(rect, 2.0, pal::BG_ANCHOR);
                        // Dashed line
                        let dash_y = rect.center().y;
                        let mut x = rect.left() + 4.0;
                        while x < rect.right() - 120.0 {
                            ui.painter().line_segment(
                                [
                                    Pos2::new(x, dash_y),
                                    Pos2::new((x + 8.0).min(rect.right() - 120.0), dash_y),
                                ],
                                Stroke::new(1.5, pal::BAR_ANCHOR),
                            );
                            x += 14.0;
                        }
                        ui.painter().text(
                            Pos2::new(rect.left() + 10.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!("⚓ insert before line {}", i + 1),
                            FontId::monospace(10.5),
                            pal::TEXT_ANCHOR,
                        );
                        let btn_size = Vec2::new(100.0, row_h);
                        let btn_rect = Rect::from_min_size(
                            Pos2::new(rect.right() - 106.0, rect.center().y - row_h / 2.0),
                            btn_size,
                        );
                        if ui
                            .put(
                                btn_rect,
                                Button::new(
                                    RichText::new("⚡ Apply here")
                                        .color(Color32::WHITE)
                                        .strong()
                                        .monospace(),
                                )
                                .fill(Color32::from_rgb(90, 70, 15))
                                .stroke(Stroke::new(1.0, pal::BAR_ANCHOR)),
                            )
                            .clicked()
                        {
                            apply_clicked = true;
                        }
                    }
                    // ── Auto-match start banner ───────────────────────────────
                    else if in_auto_match && i == auto_start && manual_anchor_check.is_none() {
                        ui.painter()
                            .rect_filled(rect, 2.0, Color32::from_rgb(28, 60, 40));
                        ui.painter().text(
                            Pos2::new(rect.left() + 10.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!(
                                "▼ auto match  {}–{}  ({:.0}%)",
                                auto_start + 1,
                                auto_end,
                                auto_score
                            ),
                            FontId::monospace(10.5),
                            Color32::from_rgb(120, 230, 160),
                        );
                        let btn_size = Vec2::new(100.0, row_h);
                        let btn_rect = Rect::from_min_size(
                            Pos2::new(rect.right() - 106.0, rect.center().y - row_h / 2.0),
                            btn_size,
                        );
                        if ui
                            .put(
                                btn_rect,
                                Button::new(
                                    RichText::new("⚡ Apply here")
                                        .color(Color32::WHITE)
                                        .strong()
                                        .monospace(),
                                )
                                .fill(Color32::from_rgb(35, 85, 50))
                                .stroke(Stroke::new(1.0, pal::BAR_MATCH)),
                            )
                            .clicked()
                        {
                            apply_clicked = true;
                        }
                    }
                    // ── Normal file row ───────────────────────────────────────
                    else {
                        // Background
                        let base_bg = if in_merged {
                            pal::BG_MERGED
                        } else if is_delete {
                            pal::BG_DELETE
                        } else if is_cursor {
                            pal::BG_CURSOR
                        } else if in_auto_match && manual_anchor_check.is_none() {
                            pal::BG_MATCH
                        } else if i % 2 == 0 {
                            pal::BG_ROW_EVEN
                        } else {
                            pal::BG_ROW_ODD
                        };
                        let row_bg = if is_search_hit {
                            pal::BG_SEARCH_HIT
                        } else {
                            base_bg
                        };
                        ui.painter().rect_filled(rect, 0.0, row_bg);

                        // Left accent bar
                        let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                        let bar_color = if in_merged {
                            pal::BAR_MERGED
                        } else if is_delete {
                            pal::TEXT_DELETE
                        } else if is_cursor {
                            pal::BAR_CURSOR
                        } else if is_anchor {
                            pal::BAR_ANCHOR
                        } else if in_auto_match && manual_anchor_check.is_none() {
                            pal::BAR_MATCH
                        } else if is_search_hit {
                            pal::BAR_SEARCH
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(bar, 0.0, bar_color);

                        // Inline diff prefix (+/=/-) shown within match region
                        let diff_prefix = if in_auto_match && manual_anchor_check.is_none() {
                            if is_delete {
                                Some(("-", pal::TEXT_DELETE))
                            } else if is_equal {
                                Some(("=", Color32::from_gray(60)))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        // Click to set cursor / anchor
                        if row_resp.clicked() {
                            set_cursor = Some(i);
                            if !search_query.is_empty() {
                                set_anchor = Some(i);
                            }
                        }

                        // Line number
                        let num_color = if in_merged {
                            pal::TEXT_LNUM_ACTIVE
                        } else if is_delete {
                            pal::TEXT_DELETE
                        } else if in_auto_match && manual_anchor_check.is_none() {
                            pal::TEXT_LNUM_ACTIVE
                        } else if is_search_hit {
                            Color32::from_rgb(180, 160, 60)
                        } else {
                            pal::TEXT_LNUM
                        };

                        ui.painter().text(
                            Pos2::new(rect.left() + 6.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!("{:>4} │", i + 1),
                            FontId::monospace(11.0),
                            num_color,
                        );

                        // Optional diff glyph column (between line number and text)
                        if let Some((glyph, glyph_color)) = diff_prefix {
                            ui.painter().text(
                                Pos2::new(rect.left() + 48.0, rect.center().y),
                                Align2::LEFT_CENTER,
                                glyph,
                                FontId::monospace(11.0),
                                glyph_color,
                            );
                        }

                        // Line text
                        let text_color = if in_merged {
                            pal::TEXT_MERGED
                        } else if is_delete {
                            pal::TEXT_DELETE
                        } else if in_auto_match && manual_anchor_check.is_none() {
                            pal::TEXT_MATCH
                        } else if is_search_hit {
                            pal::TEXT_SEARCH
                        } else {
                            pal::TEXT_NORMAL
                        };
                        let display = Self::truncate_owned(line, max_chars);
                        ui.painter().text(
                            Pos2::new(rect.left() + 58.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            &display,
                            FontId::monospace(11.0),
                            text_color,
                        );
                    }

                    // Match-end separator line
                    if in_auto_match
                        && i == auto_end.saturating_sub(1)
                        && manual_anchor_check.is_none()
                    {
                        let (sep_rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 2.0),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(sep_rect, 0.0, pal::BAR_MATCH);
                    }
                }

                ui.add_space(row_h * 3.0);
            });

        if scroll_to_match && !did_scroll {
            did_scroll = true;
        }
        if did_scroll {
            self.scroll_to_match = false;
        }

        if let Some(anchor_line) = set_anchor {
            self.manual_anchor = Some(anchor_line);
            self.set_message(StatusMessage::info(format!(
                "⚓ Anchor at line {} — click ⚡ Apply here or press A",
                anchor_line + 1
            )));
        }
        if let Some(cur_line) = set_cursor {
            self.cursor_line = Some(cur_line);
        }

        if apply_clicked {
            self.apply_merge();
        }
    }
}
