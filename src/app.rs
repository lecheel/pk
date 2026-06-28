use crate::diff::{self, MatchResult, RowKind};
use crate::patch::{self, PatchHunk};
use eframe::egui::*;

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

// ---- per-search-line match state ----
#[derive(Clone)]
struct SearchRow {
    text: String,
    file_idx: Option<usize>,
    kind: RowKind,
}

pub struct MergeApp {
    // patch state
    patch_text: String,
    hunks: Vec<PatchHunk>,
    current_hunk: usize,

    // file state
    file_text: String,
    file_lines: Vec<String>,
    file_path: String,
    base_dir: String,

    // computed
    match_result: Option<MatchResult>,
    search_rows: Vec<SearchRow>,

    // merge result
    merged_lines: Option<Vec<String>>,
    merged_range: Option<(usize, usize)>,
    show_merged: bool,

    message: Option<String>,

    /// Which candidate match is selected (index into MatchResult.candidates)
    candidate_index: usize,
    /// Manual anchor: (search_line_idx, file_line_idx)
    manual_anchor: Option<(usize, usize)>,
    /// When true, clicking a line in the file panel sets the manual anchor
    anchor_mode: bool,
    /// When true, scroll the file panel to center on the matched region
    scroll_to_match: bool,

    /// Text input for the anchor string search (e.g. "xxxxkkkk")
    anchor_search_text: String,
    /// List of file line indices matching the anchor search text
    anchor_matches: Vec<usize>,
    /// Current selected index within anchor_matches
    anchor_match_idx: usize,
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
            merged_lines: None,
            merged_range: None,
            show_merged: false,
            message: None,
            candidate_index: 0,
            manual_anchor: None,
            anchor_mode: false,
            scroll_to_match: true,
            anchor_search_text: String::new(),
            anchor_matches: Vec::new(),
            anchor_match_idx: 0,
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

    // ---- state updates ----

    fn reparse(&mut self) {
        self.hunks = patch::parse_patches(&self.patch_text);
        self.current_hunk = 0;
        self.merged_lines = None;
        self.merged_range = None;
        self.show_merged = false;
        self.candidate_index = 0;
        self.manual_anchor = None;
        self.anchor_mode = false;
        self.scroll_to_match = true;
        self.anchor_search_text.clear();
        self.anchor_matches.clear();
        self.anchor_match_idx = 0;
        self.reload_file();
    }

    fn reload_file(&mut self) {
        let hunk = match self.hunks.get(self.current_hunk) {
            Some(h) => h,
            None => return,
        };
        let path = std::path::Path::new(&self.base_dir).join(&hunk.filename);
        self.file_path = path.display().to_string();

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.file_text = content;
                self.file_lines = self.file_text.lines().map(String::from).collect();
                self.message = Some(format!("Loaded: {}", path.display()));
            }
            Err(e) => {
                if hunk.filename.ends_with("mod.rs") {
                    self.file_text = DEFAULT_FILE.to_string();
                    self.file_lines = self.file_text.lines().map(String::from).collect();
                    self.message = Some(format!("File not found — using embedded sample ({})", e));
                } else {
                    self.file_text = String::new();
                    self.file_lines = Vec::new();
                    self.message = Some(format!("Cannot read {}: {}", path.display(), e));
                }
            }
        }

        self.merged_lines = None;
        self.merged_range = None;
        self.show_merged = false;
        self.candidate_index = 0;
        self.manual_anchor = None;
        self.anchor_mode = false;
        self.scroll_to_match = true;
        self.anchor_search_text.clear();
        self.anchor_matches.clear();
        self.anchor_match_idx = 0;
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
            return;
        }

        let mr = if let Some((s_idx, f_idx)) = self.manual_anchor {
            diff::find_best_match_with_anchor(&hunk.search, &self.file_lines, s_idx, f_idx)
        } else {
            let best = diff::find_best_match(&hunk.search, &self.file_lines);
            if best.candidates.is_empty() {
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
            }
        };

        self.search_rows = Self::build_search_rows(hunk, &mr);
        self.match_result = Some(mr);
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
        let mr = match &self.match_result {
            Some(m) => m.clone(),
            None => return,
        };
        let hunk = match self.hunks.get(self.current_hunk) {
            Some(h) => h.clone(),
            None => return,
        };

        let mut output: Vec<String> = Vec::new();
        output.extend_from_slice(&self.file_lines[..mr.file_start]);
        let replace_start = output.len();
        output.extend(hunk.replace.iter().cloned());
        let replace_end = output.len();
        output.extend_from_slice(&self.file_lines[mr.file_end..]);

        self.merged_lines = Some(output);
        self.merged_range = Some((replace_start, replace_end));
        self.show_merged = true;
        self.message = Some(format!(
            "Applied: replaced lines {}–{} with {} new lines",
            mr.file_start + 1,
            mr.file_end,
            hunk.replace.len()
        ));
    }

    fn save_merged(&mut self) {
        let lines = match &self.merged_lines {
            Some(l) => l.clone(),
            None => return,
        };
        let content = lines.join("\n");
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

    fn truncate(text: &str, max_chars: usize) -> &str {
        let mut end = 0;
        for (i, (byte_pos, _)) in text.char_indices().enumerate() {
            if i >= max_chars {
                return &text[..end];
            }
            end = byte_pos;
        }
        text
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

// =========================================================================
// eframe::App
// =========================================================================
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
            if self.show_merged {
                self.render_merged_view(ui);
            } else {
                self.render_split_view(ui);
            }
        });
    }
}

// =========================================================================
// Toolbar & Status
// =========================================================================
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
                            self.merged_lines = None;
                            self.merged_range = None;
                            self.show_merged = false;
                            self.recompute_match();
                        }
                    }
                }

                ui.separator();

                // Match score badge
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

                if self.merged_lines.is_some() {
                    let toggle_label = if self.show_merged {
                        "Show Diff"
                    } else {
                        "Show Merged"
                    };
                    if ui.button(toggle_label).clicked() {
                        self.show_merged = !self.show_merged;
                    }
                    if ui.button("💾 Save").clicked() {
                        self.save_merged();
                    }
                }
            });
            ui.add_space(2.0);
        });
    }

    fn render_status_bar(&self, ctx: &Context) {
        TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if self.anchor_mode {
                    ui.colored_label(
                        Color32::from_rgb(255, 200, 60),
                        "⚓ Click a line in the file buffer (right panel) to anchor",
                    );
                } else if let Some(ref msg) = self.message {
                    ui.colored_label(Color32::from_rgb(220, 180, 60), msg);
                } else if let Some(hunk) = self.current_hunk() {
                    ui.label(format!("📄 {}", hunk.filename));
                    ui.separator();
                    if let Some(ref mr) = self.match_result {
                        let anchor_tag = if self.manual_anchor.is_some() {
                            "  🔗"
                        } else {
                            ""
                        };
                        ui.label(format!(
                            "Match: lines {}–{}  |  search {} ln  |  replace {} ln{}",
                            mr.file_start + 1,
                            mr.file_end,
                            hunk.search.len(),
                            hunk.replace.len(),
                            anchor_tag,
                        ));
                    }
                }
            });
            ui.add_space(2.0);
        });
    }
}

// =========================================================================
// Split view: left = search pattern, right = full file buffer
// =========================================================================
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

                let (banner_color, banner_text) = if mr.score >= 80.0 {
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
                        Color32::from_gray(210),
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
                            Color32::from_rgb(160, 240, 170),
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
        let candidate_count = mr.candidates.len();

        // State captures for action bar
        let mut prev_hunk = false;
        let mut next_hunk = false;
        let mut clear_anchor = false;
        let mut find_anchor = false;
        let mut prev_candidate = false;
        let mut next_candidate = false;
        let mut prev_anchor_match = false;
        let mut next_anchor_match = false;
        let mut apply_clicked = false;

        let current_hunk_idx = self.current_hunk;
        let total_hunks = self.hunks.len();
        let manual_anchor = self.manual_anchor;
        let can_apply = self.match_result.is_some() && !self.show_merged;

        // --- RIGHT WINDOW ACTION BAR (Always Visible at Top) ---
        Frame::none()
            .fill(Color32::from_rgb(35, 45, 55))
            .inner_margin(Margin::symmetric(6.0, 4.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // Hunk Navigation
                    ui.label(RichText::new("Hunk:").color(Color32::from_gray(160)));
                    if ui.button("◀").clicked() {
                        prev_hunk = true;
                    }
                    ui.label(format!("{}/{}", current_hunk_idx + 1, total_hunks));
                    if ui.button("▶").clicked() {
                        next_hunk = true;
                    }

                    ui.separator();

                    // Search/Filter Anchor Input
                    ui.label(RichText::new("Anchor Search:").color(Color32::from_gray(160)));
                    let response = ui.add(
                        TextEdit::singleline(&mut self.anchor_search_text)
                            .desired_width(80.0)
                            .hint_text("xxxxkkkk"),
                    );
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        find_anchor = true;
                    }
                    if ui.button("🔍 Find").clicked() {
                        find_anchor = true;
                    }

                    if manual_anchor.is_some() {
                        if ui.button("✕ Clear Anchor").clicked() {
                            clear_anchor = true;
                        }
                    }
                });
            });
        ui.separator();

        // Handle Action Bar Clicks
        if prev_hunk && current_hunk_idx > 0 {
            self.current_hunk -= 1;
            self.reload_file();
        }
        if next_hunk && current_hunk_idx < total_hunks - 1 {
            self.current_hunk += 1;
            self.reload_file();
        }
        if clear_anchor {
            self.manual_anchor = None;
            self.candidate_index = 0;
            self.anchor_matches.clear();
            self.scroll_to_match = true;
            self.recompute_match();
        }
        if find_anchor {
            let query = self.anchor_search_text.trim().to_string();
            if !query.is_empty() {
                self.anchor_matches = self
                    .file_lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| l.contains(&query))
                    .map(|(i, _)| i)
                    .collect();

                if !self.anchor_matches.is_empty() {
                    self.anchor_match_idx = 0;
                    self.manual_anchor = Some((0, self.anchor_matches[0]));
                    self.scroll_to_match = true;
                    self.recompute_match();
                } else {
                    self.manual_anchor = None;
                    self.recompute_match();
                }
            }
        }

        if prev_anchor_match && !self.anchor_matches.is_empty() {
            if self.anchor_match_idx > 0 {
                self.anchor_match_idx -= 1;
            } else {
                self.anchor_match_idx = self.anchor_matches.len() - 1;
            }
            self.manual_anchor = Some((0, self.anchor_matches[self.anchor_match_idx]));
            self.scroll_to_match = true;
            self.recompute_match();
        }

        if next_anchor_match && !self.anchor_matches.is_empty() {
            if self.anchor_match_idx + 1 < self.anchor_matches.len() {
                self.anchor_match_idx += 1;
            } else {
                self.anchor_match_idx = 0;
            }
            self.manual_anchor = Some((0, self.anchor_matches[self.anchor_match_idx]));
            self.scroll_to_match = true;
            self.recompute_match();
        }

        if prev_candidate && self.candidate_index > 0 {
            self.candidate_index -= 1;
            self.scroll_to_match = true;
            self.recompute_match();
        }
        if next_candidate && self.candidate_index + 1 < candidate_count {
            self.candidate_index += 1;
            self.scroll_to_match = true;
            self.recompute_match();
        }

        // --- FILE SCROLL AREA ---
        let scroll_to_match = self.scroll_to_match;
        let mut did_scroll = false;
        let file_lines = self.file_lines.clone();
        let manual_anchor_check = self.manual_anchor;
        let anchor_mode = self.anchor_mode;
        let mut anchor_line_click: Option<usize> = None;

        ScrollArea::both()
            .id_source("file_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, line) in file_lines.iter().enumerate() {
                    let in_match = i >= mr.file_start && i < mr.file_end;

                    if in_match && i == mr.file_start {
                        if scroll_to_match {
                            ui.scroll_to_cursor(Some(Align::Center));
                            did_scroll = true;
                        }

                        let desired = Vec2::new(ui.available_width(), row_h + 6.0);
                        let (banner_rect, _) = ui.allocate_exact_size(desired, Sense::hover());

                        let banner_bg = if manual_anchor_check.is_some() {
                            Color32::from_rgb(50, 60, 90)
                        } else {
                            Color32::from_rgb(40, 80, 55)
                        };
                        ui.painter().rect_filled(banner_rect, 2.0, banner_bg);

                        let banner_text = if manual_anchor_check.is_some() {
                            format!(
                                "🔗 manual anchor lines {}–{}  ({:.0}%)",
                                mr.file_start + 1,
                                mr.file_end,
                                mr.score
                            )
                        } else {
                            format!(
                                "▼ match lines {}–{}  ({:.0}%)",
                                mr.file_start + 1,
                                mr.file_end,
                                mr.score
                            )
                        };
                        let banner_color = if manual_anchor_check.is_some() {
                            Color32::from_rgb(120, 180, 255)
                        } else {
                            Color32::from_rgb(120, 230, 160)
                        };
                        ui.painter().text(
                            Pos2::new(banner_rect.left() + 8.0, banner_rect.center().y),
                            Align2::LEFT_CENTER,
                            &banner_text,
                            FontId::monospace(11.0),
                            banner_color,
                        );

                        let mut right_x = banner_rect.right() - 8.0;
                        let btn_h = row_h;

                        // Apply button (Restored inside match banner)
                        if can_apply {
                            let apply_w = 80.0;
                            let apply_rect = Rect::from_min_size(
                                Pos2::new(right_x - apply_w, banner_rect.center().y - btn_h / 2.0),
                                Vec2::new(apply_w, btn_h),
                            );
                            right_x -= apply_w + 6.0;
                            let apply_resp = ui.put(
                                apply_rect,
                                Button::new(
                                    RichText::new("⚡ Apply")
                                        .color(Color32::from_rgb(255, 230, 80))
                                        .strong()
                                        .monospace(),
                                )
                                .fill(Color32::from_rgb(60, 100, 40))
                                .stroke(Stroke::new(1.5, Color32::from_rgb(180, 220, 80))),
                            );
                            if apply_resp.clicked() {
                                apply_clicked = true;
                            }
                        }

                        // Anchor matches navigation (Replace #1/3 [◀] [▶])
                        if manual_anchor_check.is_some() && !self.anchor_matches.is_empty() {
                            let nav_w = 28.0;
                            let count_text = format!(
                                "Replace #{}/{}",
                                self.anchor_match_idx + 1,
                                self.anchor_matches.len()
                            );
                            let count_size = ui
                                .painter()
                                .layout(
                                    count_text.clone(),
                                    FontId::monospace(11.0),
                                    Color32::from_gray(200),
                                    f32::INFINITY,
                                )
                                .size();
                            let count_rect = Rect::from_min_size(
                                Pos2::new(
                                    right_x - count_size.x - 4.0,
                                    banner_rect.center().y - count_size.y / 2.0,
                                ),
                                Vec2::new(count_size.x + 8.0, count_size.y + 2.0),
                            );
                            right_x -= count_rect.width() + 4.0;
                            ui.painter().text(
                                count_rect.center(),
                                Align2::CENTER_CENTER,
                                &count_text,
                                FontId::monospace(11.0),
                                Color32::from_gray(200),
                            );

                            let next_rect = Rect::from_min_size(
                                Pos2::new(right_x - nav_w, banner_rect.center().y - btn_h / 2.0),
                                Vec2::new(nav_w, btn_h),
                            );
                            right_x -= nav_w + 4.0;
                            let next_resp = ui.put(
                                next_rect,
                                Button::new(RichText::new("▶").monospace())
                                    .fill(Color32::from_rgb(50, 70, 50)),
                            );
                            if next_resp.clicked() {
                                next_anchor_match = true;
                            }

                            let prev_rect = Rect::from_min_size(
                                Pos2::new(right_x - nav_w, banner_rect.center().y - btn_h / 2.0),
                                Vec2::new(nav_w, btn_h),
                            );
                            let prev_resp = ui.put(
                                prev_rect,
                                Button::new(RichText::new("◀").monospace())
                                    .fill(Color32::from_rgb(50, 70, 50)),
                            );
                            if prev_resp.clicked() {
                                prev_anchor_match = true;
                            }
                        }
                        // Auto-matched candidates navigation (1/3 [◀] [▶])
                        else if manual_anchor_check.is_none() && candidate_count > 1 {
                            let nav_w = 28.0;
                            let count_text =
                                format!("{}/{}", self.candidate_index + 1, candidate_count);
                            let count_size = ui
                                .painter()
                                .layout(
                                    count_text.clone(),
                                    FontId::monospace(11.0),
                                    Color32::from_gray(200),
                                    f32::INFINITY,
                                )
                                .size();
                            let count_rect = Rect::from_min_size(
                                Pos2::new(
                                    right_x - count_size.x - 4.0,
                                    banner_rect.center().y - count_size.y / 2.0,
                                ),
                                Vec2::new(count_size.x + 8.0, count_size.y + 2.0),
                            );
                            right_x -= count_rect.width() + 4.0;
                            ui.painter().text(
                                count_rect.center(),
                                Align2::CENTER_CENTER,
                                &count_text,
                                FontId::monospace(11.0),
                                Color32::from_gray(200),
                            );

                            let next_rect = Rect::from_min_size(
                                Pos2::new(right_x - nav_w, banner_rect.center().y - btn_h / 2.0),
                                Vec2::new(nav_w, btn_h),
                            );
                            right_x -= nav_w + 4.0;
                            let next_resp = ui.put(
                                next_rect,
                                Button::new(RichText::new("▶").monospace())
                                    .fill(Color32::from_rgb(50, 70, 50)),
                            );
                            if next_resp.clicked() {
                                next_candidate = true;
                            }

                            let prev_rect = Rect::from_min_size(
                                Pos2::new(right_x - nav_w, banner_rect.center().y - btn_h / 2.0),
                                Vec2::new(nav_w, btn_h),
                            );
                            let prev_resp = ui.put(
                                prev_rect,
                                Button::new(RichText::new("◀").monospace())
                                    .fill(Color32::from_rgb(50, 70, 50)),
                            );
                            if prev_resp.clicked() {
                                prev_candidate = true;
                            }
                        }
                    }

                    let sense = if anchor_mode {
                        Sense::click()
                    } else {
                        Sense::hover()
                    };
                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, resp) = ui.allocate_exact_size(desired, sense);

                    let bg = if anchor_mode && resp.hovered() {
                        Color32::from_rgb(90, 70, 20)
                    } else if in_match {
                        Color32::from_rgb(30, 50, 35)
                    } else if i % 2 == 0 {
                        Color32::from_gray(24)
                    } else {
                        Color32::from_gray(27)
                    };
                    ui.painter().rect_filled(rect, 0.0, bg);

                    let num_color = if in_match {
                        Color32::from_rgb(80, 160, 100)
                    } else {
                        Color32::from_gray(70)
                    };
                    let num_text = format!("{:>4} │", i + 1);
                    ui.painter().text(
                        Pos2::new(rect.left() + 4.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &num_text,
                        FontId::monospace(12.0),
                        num_color,
                    );

                    let content_color = if in_match {
                        Color32::from_rgb(200, 240, 210)
                    } else {
                        Color32::from_gray(190)
                    };
                    let display = Self::truncate_owned(line, max_chars);
                    ui.painter().text(
                        Pos2::new(rect.left() + 56.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        FontId::monospace(12.0),
                        content_color,
                    );

                    if in_match && i == mr.file_end.saturating_sub(1) {
                        let desired = Vec2::new(ui.available_width(), 3.0);
                        let (sep_rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                        ui.painter()
                            .rect_filled(sep_rect, 0.0, Color32::from_rgb(60, 140, 80));
                    }

                    if resp.clicked() && anchor_mode {
                        anchor_line_click = Some(i);
                    }
                }
                ui.add_space(row_h * 3.0);
            });

        // Handle Scroll and Anchor Clicks
        if did_scroll {
            self.scroll_to_match = false;
        }

        if let Some(file_idx) = anchor_line_click {
            self.manual_anchor = Some((0, file_idx)); // Default to search line 0
            self.anchor_mode = false;
            self.scroll_to_match = true;
            self.recompute_match();
        }

        if apply_clicked {
            self.apply_merge();
        }
    }
}

// =========================================================================
// Merged view: full file with replaced region highlighted
// =========================================================================
impl MergeApp {
    fn render_merged_view(&self, ui: &mut Ui) {
        let lines = match &self.merged_lines {
            Some(l) => l,
            None => return,
        };
        let (rstart, rend) = match self.merged_range {
            Some(r) => r,
            None => return,
        };

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("✓ Merged Result")
                    .color(Color32::from_rgb(100, 220, 100))
                    .strong(),
            );
            ui.separator();
            ui.label(format!(
                "Inserted {} lines at {}–{}",
                rend - rstart,
                rstart + 1,
                rend
            ));
        });
        ui.separator();

        let mono_h = ui.text_style_height(&TextStyle::Monospace);
        let row_h = mono_h + 4.0;
        let char_w = mono_h * 0.60;
        let max_chars = ((ui.available_width() - 64.0) / char_w).floor() as usize;

        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, line) in lines.iter().enumerate() {
                    let in_replace = i >= rstart && i < rend;

                    let bg = if in_replace {
                        Color32::from_rgb(28, 52, 32)
                    } else if i % 2 == 0 {
                        Color32::from_gray(24)
                    } else {
                        Color32::from_gray(27)
                    };
                    let color = if in_replace {
                        Color32::from_rgb(160, 245, 170)
                    } else {
                        Color32::from_gray(195)
                    };
                    let num_color = if in_replace {
                        Color32::from_rgb(80, 160, 100)
                    } else {
                        Color32::from_gray(70)
                    };

                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, bg);

                    if in_replace {
                        let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                        ui.painter()
                            .rect_filled(bar, 0.0, Color32::from_rgb(80, 200, 100));
                    }

                    ui.painter().text(
                        Pos2::new(rect.left() + 6.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("{:>4} │", i + 1),
                        FontId::monospace(12.0),
                        num_color,
                    );

                    let display = Self::truncate_owned(line, max_chars);
                    ui.painter().text(
                        Pos2::new(rect.left() + 58.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &display,
                        FontId::monospace(12.0),
                        color,
                    );
                }
                ui.add_space(row_h * 3.0);
            });
    }
}
