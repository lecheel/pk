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

// ---- per-search-line match state ----
#[derive(Clone)]
struct SearchRow {
    text: String,
    /// index into file_lines that this row matched against (None = unmatched/insert)
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

    // right-panel search / manual anchor
    file_search_query: String,
    file_search_matches: HashSet<usize>, // line indices that match the query
    manual_anchor: Option<usize>,        // user-chosen insert-before line (overrides auto match)

    // merge result
    merged_lines: Option<Vec<String>>,
    merged_range: Option<(usize, usize)>, // (start, end) in merged_lines
    show_merged: bool,

    message: Option<String>,
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
            merged_lines: None,
            merged_range: None,
            show_merged: false,
            message: None,
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
        self.manual_anchor = None;
        self.file_search_query.clear();
        self.file_search_matches.clear();
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
        self.manual_anchor = None;
        self.file_search_query.clear();
        self.file_search_matches.clear();
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
            let mr = diff::find_best_match(&hunk.search, &self.file_lines);
            self.search_rows = Self::build_search_rows(hunk, &mr);
            self.match_result = Some(mr);
        }
    }

    fn build_search_rows(hunk: &PatchHunk, mr: &MatchResult) -> Vec<SearchRow> {
        // Align search lines against the matched file window using the diff rows
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

    fn recompute_file_search(&mut self) {
        let q = self.file_search_query.to_lowercase();
        if q.is_empty() {
            self.file_search_matches.clear();
        } else {
            self.file_search_matches = self
                .file_lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect::<HashSet<usize>>();
        }
    }

    fn apply_merge(&mut self) {
        let hunk = match self.hunks.get(self.current_hunk) {
            Some(h) => h.clone(),
            None => return,
        };

        // Use manual anchor if set, else fall back to auto match
        let (file_start, file_end) = if let Some(anchor) = self.manual_anchor {
            // Insert-before semantics: replace zero lines at anchor position
            // (pure insert), unless user clicked anchor inside matched region
            (anchor, anchor)
        } else {
            match &self.match_result {
                Some(m) => (m.file_start, m.file_end),
                None => return,
            }
        };

        let mut output: Vec<String> = Vec::new();
        output.extend_from_slice(&self.file_lines[..file_start]);
        let replace_start = output.len();
        output.extend(hunk.replace.iter().cloned());
        let replace_end = output.len();
        output.extend_from_slice(&self.file_lines[file_end..]);

        let mode = if self.manual_anchor.is_some() {
            format!("anchor line {}", file_start + 1)
        } else {
            format!("auto match lines {}–{}", file_start + 1, file_end)
        };

        self.merged_lines = Some(output);
        self.merged_range = Some((replace_start, replace_end));
        self.show_merged = true;
        self.message = Some(format!(
            "Applied via {} → inserted {} lines",
            mode,
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

    #[allow(dead_code)]
    fn truncate(text: &str, max_chars: usize) -> &str {
        // return a byte-safe prefix slice — we won't append ellipsis to keep it zero-alloc
        // for display; callers that want "…" can wrap this
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
                            self.manual_anchor = None;
                            self.file_search_query.clear();
                            self.file_search_matches.clear();
                            self.recompute_match();
                        }
                    }
                }

                ui.separator();

                if !self.hunks.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "Hunk {}/{}",
                            self.current_hunk + 1,
                            self.hunks.len()
                        ))
                        .strong(),
                    );
                    if ui.button("◀ Prev").clicked() && self.current_hunk > 0 {
                        self.current_hunk -= 1;
                        self.reload_file();
                    }
                    if ui.button("Next ▶").clicked() && self.current_hunk < self.hunks.len() - 1 {
                        self.current_hunk += 1;
                        self.reload_file();
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

                // Apply / toggle merged / save
                let can_apply = (self.match_result.is_some() || self.manual_anchor.is_some())
                    && !self.show_merged;

                if let Some(anchor) = self.manual_anchor {
                    // Show anchor indicator badge
                    let frame = Frame::none()
                        .fill(Color32::from_rgb(60, 45, 15))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(220, 160, 40)))
                        .rounding(Rounding::same(4.0))
                        .inner_margin(Margin::symmetric(8.0, 4.0));
                    frame.show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("⚓ anchor @ line {}", anchor + 1))
                                .color(Color32::from_rgb(255, 200, 60))
                                .strong()
                                .monospace(),
                        );
                    });
                    if ui.button("✕ clear").clicked() {
                        self.manual_anchor = None;
                    }
                    ui.separator();
                }

                if can_apply && ui.button("⚡ Apply").clicked() {
                    self.apply_merge();
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
        let divider = 0.38; // left panel takes 38% of width
        let left_w = (available.x * divider).floor() - 1.0;
        let right_w = available.x - left_w - 2.0; // 2px for separator

        let mono_h = ui.text_style_height(&TextStyle::Monospace);
        let row_h = mono_h + 4.0;
        let char_w = mono_h * 0.60; // rough monospace char width

        // ---- Column headers ----
        ui.horizontal(|ui| {
            // Left header
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
            // Right header
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

        // ---- Body: two synchronized-scroll panels ----
        // We use a shared ScrollArea for the right (full file) panel.
        // The left panel shows search lines anchored to the top.

        let body_rect = ui.available_rect_before_wrap();

        // Left panel — search pattern lines (fixed, no scroll needed for typical patches)
        let mut left_rect = body_rect;
        left_rect.set_width(left_w);

        let mut right_rect = body_rect;
        right_rect.min.x = body_rect.min.x + left_w + 2.0;
        right_rect.set_width(right_w);

        // Draw left panel
        let mut left_ui = ui.child_ui(left_rect, Layout::top_down(Align::LEFT), None);
        self.render_search_panel(&mut left_ui, &mr, row_h, char_w, left_w);

        // Draw right panel
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

                // Header: match confidence banner
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

                // Search lines
                for (line_idx, line) in hunk.search.iter().enumerate() {
                    // Find if this search line matched something in the file
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

                    // Line number in search block
                    let num_text = format!("{:>3} ", line_idx + 1);
                    ui.painter().text(
                        Pos2::new(rect.left() + 4.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &num_text,
                        FontId::monospace(12.0),
                        Color32::from_gray(90),
                    );

                    // Prefix symbol
                    ui.painter().text(
                        Pos2::new(rect.left() + 36.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        prefix,
                        FontId::monospace(12.0),
                        prefix_color,
                    );

                    // Line content
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

                // Replace preview section
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

        // ---- Search bar ----
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").monospace());
            let search_edit = TextEdit::singleline(&mut self.file_search_query)
                .hint_text("search file…  (click result to set insert anchor)")
                .desired_width(ui.available_width() - 120.0)
                .font(TextStyle::Monospace);
            let resp = ui.add(search_edit);
            if resp.changed() {
                self.recompute_file_search();
                // clear anchor when search changes
                self.manual_anchor = None;
            }

            let match_count = self.file_search_matches.len();
            if !self.file_search_query.is_empty() {
                let label = if match_count == 0 {
                    RichText::new("no match").color(Color32::from_rgb(200, 80, 80))
                } else {
                    RichText::new(format!(
                        "{} hit{}",
                        match_count,
                        if match_count == 1 { "" } else { "s" }
                    ))
                    .color(Color32::from_rgb(120, 200, 120))
                };
                ui.label(label.monospace());
            }

            if self.manual_anchor.is_some() && ui.small_button("✕ anchor").clicked() {
                self.manual_anchor = None;
            }
        });
        ui.add_space(1.0);

        // Snapshot what we need to avoid borrow conflicts inside closure
        let file_lines = self.file_lines.clone();
        let file_search_matches: HashSet<usize> = self.file_search_matches.clone();
        let manual_anchor = self.manual_anchor;
        let auto_start = mr.file_start;
        let auto_end = mr.file_end;
        let auto_score = mr.score;
        let search_query = self.file_search_query.clone();

        let mut apply_clicked = false;
        let mut set_anchor: Option<usize> = None;

        ScrollArea::both()
            .id_source("file_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, line) in file_lines.iter().enumerate() {
                    let in_auto_match = i >= auto_start && i < auto_end;
                    let is_search_hit =
                        !search_query.is_empty() && file_search_matches.contains(&i);
                    let is_anchor = manual_anchor == Some(i);

                    // ---- Anchor marker row (insert-before indicator) ----
                    if is_anchor {
                        let desired = Vec2::new(ui.available_width(), row_h + 4.0);
                        let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                        ui.painter()
                            .rect_filled(rect, 2.0, Color32::from_rgb(50, 40, 10));

                        // dashed line across
                        let dash_y = rect.center().y;
                        let mut x = rect.left() + 4.0;
                        while x < rect.right() - 90.0 {
                            ui.painter().line_segment(
                                [
                                    Pos2::new(x, dash_y),
                                    Pos2::new((x + 8.0).min(rect.right() - 90.0), dash_y),
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

                        // Apply here button
                        let btn_size = Vec2::new(100.0, row_h);
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
                                    .color(Color32::from_rgb(255, 230, 80))
                                    .strong()
                                    .monospace(),
                            )
                            .fill(Color32::from_rgb(70, 55, 10))
                            .stroke(Stroke::new(1.5, Color32::from_rgb(220, 160, 40))),
                        );
                        if btn_resp.clicked() {
                            apply_clicked = true;
                        }
                    }

                    // ---- Auto-match banner (first line of auto match, only if no manual anchor) ----
                    if in_auto_match && i == auto_start && manual_anchor.is_none() {
                        let desired = Vec2::new(ui.available_width(), row_h + 6.0);
                        let (banner_rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                        let banner_bg = Color32::from_rgb(40, 80, 55);
                        ui.painter().rect_filled(banner_rect, 2.0, banner_bg);
                        ui.painter().text(
                            Pos2::new(banner_rect.left() + 8.0, banner_rect.center().y),
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
                        let btn_size = Vec2::new(80.0, row_h);
                        let btn_rect = Rect::from_min_size(
                            Pos2::new(
                                banner_rect.right() - btn_size.x - 8.0,
                                banner_rect.center().y - btn_size.y / 2.0,
                            ),
                            btn_size,
                        );
                        let btn_resp = ui.put(
                            btn_rect,
                            Button::new(
                                RichText::new("⚡ Apply")
                                    .color(Color32::from_rgb(255, 230, 80))
                                    .strong()
                                    .monospace(),
                            )
                            .fill(Color32::from_rgb(60, 100, 40))
                            .stroke(Stroke::new(1.5, Color32::from_rgb(180, 220, 80))),
                        );
                        if btn_resp.clicked() {
                            apply_clicked = true;
                        }
                    }

                    // ---- File line row ----
                    let base_bg = if in_auto_match && manual_anchor.is_none() {
                        Color32::from_rgb(30, 50, 35)
                    } else if i % 2 == 0 {
                        Color32::from_gray(24)
                    } else {
                        Color32::from_gray(27)
                    };

                    // Search hit gets a subtle yellow tint
                    let row_bg = if is_search_hit {
                        Color32::from_rgb(55, 50, 18)
                    } else {
                        base_bg
                    };

                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, row_resp) = ui.allocate_exact_size(desired, Sense::click());
                    ui.painter().rect_filled(rect, 0.0, row_bg);

                    // Left accent bar: anchor line or auto-match
                    if is_anchor {
                        let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                        ui.painter()
                            .rect_filled(bar, 0.0, Color32::from_rgb(220, 160, 40));
                    } else if in_auto_match && manual_anchor.is_none() {
                        let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                        ui.painter()
                            .rect_filled(bar, 0.0, Color32::from_rgb(60, 160, 90));
                    } else if is_search_hit {
                        let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                        ui.painter()
                            .rect_filled(bar, 0.0, Color32::from_rgb(180, 150, 40));
                    }

                    // Hover highlight (shows it's clickable when searching)
                    if row_resp.hovered() && !search_query.is_empty() {
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            Color32::from_rgba_premultiplied(80, 80, 40, 30),
                        );
                        // Tooltip
                        ui.painter().text(
                            Pos2::new(rect.right() - 6.0, rect.center().y),
                            Align2::RIGHT_CENTER,
                            "click → set anchor",
                            FontId::monospace(10.0),
                            Color32::from_rgb(160, 140, 60),
                        );
                    }

                    // Click: set manual anchor
                    if row_resp.clicked() && !search_query.is_empty() {
                        set_anchor = Some(i);
                    }

                    // Line number gutter
                    let num_color = if in_auto_match && manual_anchor.is_none() {
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

                    // Content — highlight search term inline
                    let text_color = if in_auto_match && manual_anchor.is_none() {
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

                    // Auto-match bottom border
                    if in_auto_match && i == auto_end.saturating_sub(1) && manual_anchor.is_none() {
                        let sep_desired = Vec2::new(ui.available_width(), 3.0);
                        let (sep_rect, _) = ui.allocate_exact_size(sep_desired, Sense::hover());
                        ui.painter()
                            .rect_filled(sep_rect, 0.0, Color32::from_rgb(60, 140, 80));
                    }
                }

                ui.add_space(row_h * 3.0);
            });

        // Apply mutations after borrow ends
        if let Some(anchor_line) = set_anchor {
            self.manual_anchor = Some(anchor_line);
            self.message = Some(format!(
                "Anchor set at line {} — click ⚡ Apply here or toolbar Apply",
                anchor_line + 1
            ));
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

                    // Left accent bar for replaced region
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
