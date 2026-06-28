//--+ file:///src/app.rs
use crate::diff::{self, MatchResult, RowKind};
use crate::patch::{self, PatchHunk};
use eframe::egui::*;
use std::collections::HashSet;

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
    message: Option<String>,
    cursor_line: Option<usize>,
    applied_hunks: HashSet<usize>,
    merged_range: Option<(usize, usize)>,
    history: Vec<(Vec<String>, usize)>,
    vim_buffer: String,
    last_action: Option<Action>,
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
            cursor_line: None,
            applied_hunks: HashSet::new(),
            merged_range: None,
            history: Vec::new(),
            vim_buffer: String::new(),
            last_action: None,
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
                app.message = Some(format!("Loaded patch file: {}", path.display()));
            } else {
                app.message = Some(format!("Failed to read patch file: {}", patch_file));
            }
        }
        if !loaded_patch {
            app.patch_text = DEFAULT_PATCH.to_string();
            app.message = Some("No patch file provided. Using default embedded patch.".to_string());
        }
        app.reparse();
        app
    }

    fn reparse(&mut self) {
        self.hunks = patch::parse_patches(&self.patch_text);
        self.current_hunk = 0;
        self.applied_hunks.clear();
        self.merged_range = None;
        self.history.clear();
        self.vim_buffer.clear();
        self.last_action = None;
        self.file_path.clear();
        self.load_hunk();
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
            self.file_path = path.clone();
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    self.file_text = content;
                    self.file_lines = self.file_text.lines().map(String::from).collect();
                    self.message = Some(format!("Loaded: {}", path));
                }
                Err(e) => {
                    if hunk.filename.ends_with("mod.rs") {
                        self.file_text = DEFAULT_FILE.to_string();
                        self.file_lines = self.file_text.lines().map(String::from).collect();
                        self.message =
                            Some(format!("File not found — using embedded sample ({})", e));
                    } else {
                        self.file_text = String::new();
                        self.file_lines = Vec::new();
                        self.message = Some(format!("Cannot read {}: {}", path, e));
                    }
                }
            }
        }

        self.manual_anchor = None;
        self.merged_range = None;
        self.anchor_matches.clear();
        self.anchor_match_idx = 0;
        self.file_search_query.clear();
        self.file_search_matches.clear();
        self.candidate_index = 0;
        self.scroll_to_match = true;
        self.cursor_line = None;
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
            self.match_result = Some(mr.clone());

            // 👇 FIX: Only set cursor_line if it's currently None
            if self.cursor_line.is_none() {
                self.cursor_line = Some(mr.file_start);
            }
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
            self.message = Some(format!("Hunk {} already applied", self.current_hunk + 1));
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
        self.manual_anchor = None;
        self.scroll_to_match = true;
        self.cursor_line = Some(file_start);
        self.message = Some(format!(
            "Applied hunk {} at line {} → inserted {} lines",
            self.current_hunk + 1,
            file_start + 1,
            hunk.replace.len()
        ));

        self.recompute_match();
    }

    fn undo(&mut self) {
        if let Some((prev_lines, hunk_idx)) = self.history.pop() {
            self.file_lines = prev_lines;
            self.applied_hunks.remove(&hunk_idx);
            self.merged_range = None;
            self.message = Some(format!("Undone action for hunk {}", hunk_idx + 1));
            self.scroll_to_match = true;
            self.recompute_match();
        } else {
            self.message = Some("Nothing to undo".to_string());
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

                // 1. Call recompute_match FIRST.
                // It will temporarily set the cursor to mr.file_start.
                self.recompute_match();

                // 2. Now override it with the correct line index after deletion
                let new_len = self.file_lines.len();
                if new_len == 0 {
                    self.cursor_line = None;
                } else if start >= new_len {
                    self.cursor_line = Some(new_len - 1);
                } else {
                    // Keeps the cursor on the line that shifted up into the deleted position
                    self.cursor_line = Some(start);
                }

                self.scroll_to_match = true;
                self.message = Some(format!("Deleted {} lines", end - start));
            }
        }
    }

    fn save_merged(&mut self) {
        let content = self.file_lines.join("\n");
        let path = if self.file_path.is_empty() {
            "merged_output.txt".to_string()
        } else {
            self.file_path.clone()
        };
        match std::fs::write(&path, &content) {
            Ok(_) => self.message = Some(format!("Saved to {}", path)),
            Err(e) => self.message = Some(format!("Save failed: {}", e)),
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
}

impl eframe::App for MergeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.render_toolbar(ctx);
        self.render_status_bar(ctx);
        CentralPanel::default().show(ctx, |ui| {
            if self.hunks.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.heading("No patches found");
                    ui.label("Open a .md file containing <patch> blocks.");
                });
                return;
            }
            self.render_split_view(ui);
        });
    }
}

impl MergeApp {
    fn render_toolbar(&mut self, ctx: &Context) {
        TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
                ui.heading("Patch Merge");
                ui.separator();
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
                            self.file_lines = self.file_text.lines().map(String::from).collect();
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
                            self.scroll_to_match = true;
                            self.cursor_line = None;
                            self.recompute_match();
                        }
                    }
                }
                ui.separator();
                if let Some(ref mr) = self.match_result {
                    let (color, icon) = if mr.score >= 80.0 {
                        (Color32::from_rgb(80, 200, 80), "✓")
                    } else if mr.score >= 50.0 {
                        (Color32::from_rgb(220, 180, 50), "≈")
                    } else {
                        (Color32::from_rgb(220, 80, 80), "✗")
                    };
                    let frame = Frame::none()
                        .fill(color.linear_multiply(0.25))
                        .stroke(Stroke::new(1.0, color))
                        .rounding(Rounding::same(4.0))
                        .inner_margin(Margin::symmetric(10.0, 4.0));
                    frame.show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("Match: {:.0}% {}", mr.score, icon))
                                .color(color)
                                .strong(),
                        );
                    });
                    ui.separator();
                }
                if ui.button("💾 Save").clicked() {
                    self.save_merged();
                }
            });
            ui.add_space(2.0);
        });
    }

    fn render_status_bar(&self, ctx: &Context) {
        TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if let Some(ref msg) = self.message {
                    ui.colored_label(Color32::from_rgb(220, 180, 60), msg);
                } else if let Some(hunk) = self.current_hunk() {
                    ui.label(format!("📄 {}", hunk.filename));
                    ui.separator();
                    if let Some(ref mr) = self.match_result {
                        ui.label(format!(
                            "Match: lines {}–{}  |  search {} ln  |  replace {} ln",
                            mr.file_start + 1,
                            mr.file_end,
                            hunk.search.len(),
                            hunk.replace.len()
                        ));
                    }
                }
                if !self.vim_buffer.is_empty() {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("Key Buffer: {}", self.vim_buffer))
                            .color(Color32::from_rgb(200, 200, 100)),
                    );
                }
            });
            ui.add_space(2.0);
        });
    }
}

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
                });
                return;
            }
        };
        let available = ui.available_size();
        let divider = 0.38;
        let left_w = (available.x * divider).floor() - 1.0;
        let right_w = available.x - left_w - 2.0;
        let mono_h = ui.text_style_height(&TextStyle::Monospace);
        let row_h = mono_h + 4.0;
        let char_w = mono_h * 0.60;
        ui.horizontal(|ui| {
            Frame::none()
                .fill(Color32::from_rgb(35, 45, 65))
                .inner_margin(Margin::symmetric(6.0, 3.0))
                .show(ui, |ui| {
                    ui.set_min_width(left_w);
                    ui.set_max_width(left_w);
                    let hunk = self.current_hunk().unwrap();
                    ui.label(
                        RichText::new(format!("SEARCH  ({})", hunk.filename))
                            .color(Color32::from_rgb(120, 180, 255))
                            .strong()
                            .monospace(),
                    );
                });
            ui.add_space(2.0);
            Frame::none()
                .fill(Color32::from_rgb(35, 55, 45))
                .inner_margin(Margin::symmetric(6.0, 3.0))
                .show(ui, |ui| {
                    ui.set_min_width(right_w);
                    ui.label(
                        RichText::new(format!(
                            "FILE BUFFER  ({} lines)  match @ {}–{}",
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
        ui.separator();
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
        let max_chars = ((panel_w - 56.0) / char_w).floor() as usize;
        ScrollArea::vertical()
            .id_source("search_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let hunk = match self.current_hunk() {
                    Some(h) => h,
                    None => return,
                };
                let is_applied = self.applied_hunks.contains(&self.current_hunk);
                let (banner_color, banner_text) = if is_applied {
                    (
                        Color32::from_rgb(50, 50, 50),
                        format!("✓ Applied Hunk {}", self.current_hunk + 1),
                    )
                } else if mr.score >= 80.0 {
                    (
                        Color32::from_rgb(40, 90, 40),
                        format!(
                            "✓ Match found at lines {}–{}",
                            mr.file_start + 1,
                            mr.file_end
                        ),
                    )
                } else if mr.score >= 50.0 {
                    (
                        Color32::from_rgb(80, 70, 20),
                        format!(
                            "≈ Partial match ({:.0}%) at lines {}–{}",
                            mr.score,
                            mr.file_start + 1,
                            mr.file_end
                        ),
                    )
                } else {
                    (
                        Color32::from_rgb(80, 30, 30),
                        format!("✗ Poor match ({:.0}%)", mr.score),
                    )
                };
                let desired = Vec2::new(ui.available_width(), row_h + 2.0);
                let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                ui.painter().rect_filled(rect, 2.0, banner_color);
                ui.painter().text(
                    Pos2::new(rect.left() + 8.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &banner_text,
                    FontId::monospace(12.0),
                    Color32::from_gray(220),
                );
                ui.add_space(2.0);
                for (line_idx, line) in hunk.search.iter().enumerate() {
                    let matched_file_row = self
                        .search_rows
                        .iter()
                        .find(|r| r.text == *line && r.file_idx.is_some());
                    let (bg, prefix_color, prefix) = if matched_file_row.is_some() {
                        (
                            Color32::from_rgb(28, 45, 28),
                            Color32::from_rgb(80, 180, 80),
                            "= ",
                        )
                    } else {
                        (
                            Color32::from_rgb(50, 30, 30),
                            Color32::from_rgb(200, 100, 100),
                            "- ",
                        )
                    };
                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, bg);
                    let num_text = format!("{:>3} ", line_idx + 1);
                    ui.painter().text(
                        Pos2::new(rect.left() + 4.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &num_text,
                        FontId::monospace(12.0),
                        Color32::from_gray(90),
                    );
                    ui.painter().text(
                        Pos2::new(rect.left() + 36.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        prefix,
                        FontId::monospace(12.0),
                        prefix_color,
                    );
                    let display = Self::truncate_owned(line, max_chars);
                    ui.painter().text(
                        Pos2::new(rect.left() + 52.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        FontId::monospace(12.0),
                        if is_applied {
                            Color32::from_gray(100)
                        } else {
                            Color32::from_gray(210)
                        },
                    );
                }
                ui.add_space(6.0);
                if !hunk.replace.is_empty() {
                    let sep_desired = Vec2::new(ui.available_width(), 1.0);
                    let (sep_rect, _) = ui.allocate_exact_size(sep_desired, Sense::hover());
                    ui.painter()
                        .rect_filled(sep_rect, 0.0, Color32::from_gray(55));
                    ui.add_space(2.0);
                    let hdr_desired = Vec2::new(ui.available_width(), row_h);
                    let (hdr_rect, _) = ui.allocate_exact_size(hdr_desired, Sense::hover());
                    ui.painter()
                        .rect_filled(hdr_rect, 0.0, Color32::from_rgb(30, 55, 35));
                    ui.painter().text(
                        Pos2::new(hdr_rect.left() + 8.0, hdr_rect.center().y),
                        Align2::LEFT_CENTER,
                        "REPLACE →",
                        FontId::monospace(11.0),
                        Color32::from_rgb(100, 200, 120),
                    );
                    for (line_idx, line) in hunk.replace.iter().enumerate() {
                        let desired = Vec2::new(ui.available_width(), row_h);
                        let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                        ui.painter()
                            .rect_filled(rect, 0.0, Color32::from_rgb(22, 44, 28));
                        let num_text = format!("{:>3} ", line_idx + 1);
                        ui.painter().text(
                            Pos2::new(rect.left() + 4.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            &num_text,
                            FontId::monospace(12.0),
                            Color32::from_gray(80),
                        );
                        ui.painter().text(
                            Pos2::new(rect.left() + 36.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            "+ ",
                            FontId::monospace(12.0),
                            Color32::from_rgb(80, 200, 100),
                        );
                        let display = Self::truncate_owned(line, max_chars);
                        ui.painter().text(
                            Pos2::new(rect.left() + 52.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            &display,
                            FontId::monospace(12.0),
                            if is_applied {
                                Color32::from_gray(100)
                            } else {
                                Color32::from_rgb(160, 240, 170)
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
        let max_chars = ((panel_w - 64.0) / char_w).floor() as usize;
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
        Frame::none()
            .fill(Color32::from_rgb(35, 45, 55))
            .inner_margin(Margin::symmetric(6.0, 4.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Hunk:").color(Color32::from_gray(160)));
                    if ui
                        .add_enabled(current_hunk_idx > 0, Button::new("◀"))
                        .clicked()
                    {
                        prev_hunk = true;
                    }
                    ui.label(format!("{}/{}", current_hunk_idx + 1, total_hunks));
                    if ui
                        .add_enabled(current_hunk_idx < total_hunks - 1, Button::new("▶"))
                        .clicked()
                    {
                        next_hunk = true;
                    }
                    if is_applied {
                        ui.label(
                            RichText::new("✓ Applied")
                                .color(Color32::from_rgb(100, 200, 100))
                                .strong(),
                        );
                    }
                    ui.separator();
                    if let Some(anchor) = manual_anchor {
                        ui.label(format!("⚓ Anchor @ line {}", anchor + 1));
                        if ui.button("✕ Clear").clicked() {
                            clear_anchor = true;
                        }
                    } else {
                        ui.label(RichText::new("Match:").color(Color32::from_gray(160)));
                        if ui
                            .add_enabled(candidate_idx > 0, Button::new("◀"))
                            .clicked()
                        {
                            prev_candidate = true;
                        }
                        ui.label(format!("{}/{}", candidate_idx + 1, candidate_count.max(1)));
                        if ui
                            .add_enabled(candidate_idx + 1 < candidate_count, Button::new("▶"))
                            .clicked()
                        {
                            next_candidate = true;
                        }
                    }
                    ui.separator();
                    ui.label(RichText::new("🔍").monospace());
                    let search_edit = TextEdit::singleline(&mut self.file_search_query)
                        .hint_text("search text...")
                        .desired_width(80.0)
                        .font(TextStyle::Monospace);
                    let resp = ui.add(search_edit);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        find_text = true;
                    }
                    if ui.button("Find").clicked() {
                        find_text = true;
                    }
                    if !self.anchor_matches.is_empty() {
                        ui.label(format!(
                            "Replace #{}/{}",
                            self.anchor_match_idx + 1,
                            self.anchor_matches.len()
                        ));
                        if ui.button("◀").clicked() {
                            prev_anchor_match = true;
                        }
                        if ui.button("▶").clicked() {
                            next_anchor_match = true;
                        }
                    }
                    ui.separator();
                    ui.add_enabled_ui(can_apply, |ui| {
                        if ui
                            .button(format!("⚡ Apply @ Line {}", apply_line))
                            .clicked()
                        {
                            apply_clicked = true;
                        }
                    });
                });
            });
        ui.separator();

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
                    self.cursor_line = Some((cur + 10).min(len - 1));
                    cursor_changed = true;
                }
                if i.key_pressed(Key::PageUp) {
                    self.cursor_line = Some(cur.saturating_sub(10));
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
                if i.key_pressed(Key::L) {
                    if i.modifiers.shift {
                        go_prev_hunk = true;
                    } else {
                        go_next_hunk = true;
                    }
                }
                if i.key_pressed(Key::A) {
                    let in_hunk = if let Some(anchor) = manual_anchor {
                        cur == anchor
                    } else {
                        cur >= mr.file_start && cur < mr.file_end
                    };

                    if is_applied {
                        self.message =
                            Some(format!("Hunk {} already applied", self.current_hunk + 1));
                    } else if in_hunk {
                        apply_clicked = true;
                    } else {
                        self.message = Some("Move cursor to the hunk block to apply".to_string());
                    }
                }
                for event in i.events.clone() {
                    if let Event::Text(txt) = event {
                        new_text.push_str(&txt);
                    }
                }
            });

            if !new_text.is_empty() {
                self.vim_buffer.push_str(&new_text);
                let buf = self.vim_buffer.trim();
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
                } else if buf.len() > 4 {
                    clear_buffer = true;
                } else {
                    // Allow digits, d/D, and g/G. Single 'g' stays in buffer to form 'gg'
                    let allowed_chars = buf.chars().all(|c| {
                        c.is_ascii_digit() || c == 'd' || c == 'D' || c == 'g' || c == 'G'
                    });
                    let d_count = buf.matches('d').count() + buf.matches('D').count();
                    if !allowed_chars || d_count > 2 {
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
            self.cursor_line = None;
            self.scroll_to_match = true;
            self.recompute_match();
            return;
        }
        if next_candidate && self.candidate_index + 1 < candidate_count {
            self.candidate_index += 1;
            self.cursor_line = None;
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
            if self.anchor_match_idx + 1 < self.anchor_matches.len() {
                self.anchor_match_idx += 1;
            } else {
                self.anchor_match_idx = 0;
            }
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

        let file_lines = self.file_lines.clone();
        let file_search_matches: HashSet<usize> = self.file_search_matches.clone();
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

        ScrollArea::both()
            .id_source("file_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, line) in file_lines.iter().enumerate() {
                    let in_auto_match = i >= auto_start && i < auto_end;
                    let is_search_hit =
                        !search_query.is_empty() && file_search_matches.contains(&i);
                    let is_anchor = manual_anchor_check == Some(i);
                    let is_cursor = cursor_line == Some(i);
                    let in_merged = if let Some((rstart, rend)) = merged_range {
                        i >= rstart && i < rend
                    } else {
                        false
                    };

                    let desired = if is_anchor {
                        Vec2::new(ui.available_width(), row_h + 4.0)
                    } else if in_auto_match && i == auto_start && manual_anchor_check.is_none() {
                        Vec2::new(ui.available_width(), row_h + 6.0)
                    } else {
                        Vec2::new(ui.available_width(), row_h)
                    };

                    let sense = Sense::click();

                    let (rect, row_resp) = ui.allocate_exact_size(desired, sense);

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

                    if is_anchor {
                        ui.painter()
                            .rect_filled(rect, 2.0, Color32::from_rgb(50, 40, 10));
                        let dash_y = rect.center().y;
                        let mut x = rect.left() + 4.0;
                        while x < rect.right() - 120.0 {
                            ui.painter().line_segment(
                                [
                                    Pos2::new(x, dash_y),
                                    Pos2::new((x + 8.0).min(rect.right() - 120.0), dash_y),
                                ],
                                Stroke::new(1.5, Color32::from_rgb(220, 160, 40)),
                            );
                            x += 14.0;
                        }
                        ui.painter().text(
                            Pos2::new(rect.left() + 8.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!("⚓ insert before line {}", i + 1),
                            FontId::monospace(11.0),
                            Color32::from_rgb(255, 200, 60),
                        );
                        let btn_size = Vec2::new(110.0, row_h);
                        let btn_rect = Rect::from_min_size(
                            Pos2::new(
                                rect.right() - btn_size.x - 6.0,
                                rect.center().y - btn_size.y / 2.0,
                            ),
                            btn_size,
                        );
                        let btn_resp = ui.put(
                            btn_rect,
                            Button::new(
                                RichText::new("⚡ Apply here")
                                    .color(Color32::from_rgb(255, 255, 255))
                                    .strong()
                                    .monospace(),
                            )
                            .fill(Color32::from_rgb(100, 80, 20))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(220, 160, 40))),
                        );
                        if btn_resp.clicked() {
                            apply_clicked = true;
                        }
                    } else if in_auto_match && i == auto_start && manual_anchor_check.is_none() {
                        let banner_bg = Color32::from_rgb(40, 80, 55);
                        ui.painter().rect_filled(rect, 2.0, banner_bg);
                        ui.painter().text(
                            Pos2::new(rect.left() + 8.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!(
                                "▼ auto match  lines {}–{}  ({:.0}%)",
                                auto_start + 1,
                                auto_end,
                                auto_score
                            ),
                            FontId::monospace(11.0),
                            Color32::from_rgb(120, 230, 160),
                        );

                        let btn_size = Vec2::new(110.0, row_h);
                        let btn_rect = Rect::from_min_size(
                            Pos2::new(
                                rect.right() - btn_size.x - 6.0,
                                rect.center().y - btn_size.y / 2.0,
                            ),
                            btn_size,
                        );
                        let btn_resp = ui.put(
                            btn_rect,
                            Button::new(
                                RichText::new("⚡ Apply here")
                                    .color(Color32::from_rgb(255, 255, 255))
                                    .strong()
                                    .monospace(),
                            )
                            .fill(Color32::from_rgb(40, 100, 60))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(60, 160, 90))),
                        );
                        if btn_resp.clicked() {
                            apply_clicked = true;
                        }
                    } else {
                        let base_bg = if in_merged {
                            Color32::from_rgb(28, 52, 32)
                        } else if is_cursor {
                            Color32::from_rgb(35, 45, 65)
                        } else if in_auto_match && manual_anchor_check.is_none() {
                            Color32::from_rgb(30, 50, 35)
                        } else if i % 2 == 0 {
                            Color32::from_gray(24)
                        } else {
                            Color32::from_gray(27)
                        };
                        let row_bg = if is_search_hit {
                            Color32::from_rgb(55, 50, 18)
                        } else {
                            base_bg
                        };

                        ui.painter().rect_filled(rect, 0.0, row_bg);

                        if in_merged {
                            let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                            ui.painter()
                                .rect_filled(bar, 0.0, Color32::from_rgb(80, 200, 100));
                        } else if is_cursor {
                            let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                            ui.painter()
                                .rect_filled(bar, 0.0, Color32::from_rgb(100, 150, 220));
                        } else if is_anchor {
                            let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                            ui.painter()
                                .rect_filled(bar, 0.0, Color32::from_rgb(220, 160, 40));
                        } else if in_auto_match && manual_anchor_check.is_none() {
                            let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                            ui.painter()
                                .rect_filled(bar, 0.0, Color32::from_rgb(60, 160, 90));
                        } else if is_search_hit {
                            let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                            ui.painter()
                                .rect_filled(bar, 0.0, Color32::from_rgb(180, 150, 40));
                        }

                        if row_resp.clicked() {
                            set_cursor = Some(i);
                            if !search_query.is_empty() {
                                set_anchor = Some(i);
                            }
                        }

                        let num_color = if in_merged {
                            Color32::from_rgb(80, 160, 100)
                        } else if in_auto_match && manual_anchor_check.is_none() {
                            Color32::from_rgb(80, 160, 100)
                        } else if is_search_hit {
                            Color32::from_rgb(180, 160, 60)
                        } else {
                            Color32::from_gray(70)
                        };
                        ui.painter().text(
                            Pos2::new(rect.left() + 6.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!("{:>4} │", i + 1),
                            FontId::monospace(12.0),
                            num_color,
                        );
                        let text_color = if in_merged {
                            Color32::from_rgb(160, 245, 170)
                        } else if in_auto_match && manual_anchor_check.is_none() {
                            Color32::from_rgb(200, 240, 210)
                        } else if is_search_hit {
                            Color32::from_rgb(240, 230, 150)
                        } else {
                            Color32::from_gray(190)
                        };
                        let display = Self::truncate_owned(line, max_chars);
                        ui.painter().text(
                            Pos2::new(rect.left() + 56.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            &display,
                            FontId::monospace(12.0),
                            text_color,
                        );
                    }

                    if in_auto_match
                        && i == auto_end.saturating_sub(1)
                        && manual_anchor_check.is_none()
                    {
                        let sep_desired = Vec2::new(ui.available_width(), 3.0);
                        let (sep_rect, _) = ui.allocate_exact_size(sep_desired, Sense::hover());
                        ui.painter()
                            .rect_filled(sep_rect, 0.0, Color32::from_rgb(60, 140, 80));
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
            self.message = Some(format!(
                "Anchor set at line {} — click ⚡ Apply here or toolbar Apply",
                anchor_line + 1
            ));
        }
        if let Some(cur_line) = set_cursor {
            self.cursor_line = Some(cur_line);
        }

        // 👇 CHECK apply_clicked AT THE VERY END
        if apply_clicked {
            self.apply_merge();
        }
    }
}
