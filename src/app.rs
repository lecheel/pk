use eframe::egui::*;

use crate::diff::{self, MatchResult, RowKind};
use crate::patch::{self, PatchHunk};

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

    // merge result
    merged_preview: Option<String>,
    merged_range: Option<(usize, usize)>,
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
            merged_preview: None,
            merged_range: None,
            show_merged: false,
            message: None,
        };

        let mut loaded_patch = false;
        if let Some(patch_file) = initial_patch {
            let path = std::path::Path::new(&patch_file);
            if let Ok(content) = std::fs::read_to_string(path) {
                app.patch_text = content;
                // Set base_dir to the patch file's directory to resolve target file paths
                if let Some(parent) = path.parent() {
                    app.base_dir = parent.display().to_string();
                }
                loaded_patch = true;
                app.message = Some(format!("Loaded patch file: {}", path.display()));
            } else {
                eprintln!("Failed to read patch file: {}", patch_file);
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
        self.merged_preview = None;
        self.merged_range = None;
        self.show_merged = false;
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
                self.message = Some(format!("Loaded target file: {}", path.display()));
            }
            Err(e) => {
                // fall back to embedded sample for files matching the default
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

        self.recompute_match();
    }

    fn recompute_match(&mut self) {
        let hunk = match self.hunks.get(self.current_hunk) {
            Some(h) => h,
            None => {
                self.match_result = None;
                return;
            }
        };

        if self.file_lines.is_empty() {
            self.match_result = None;
        } else {
            self.match_result = Some(diff::find_best_match(&hunk.search, &self.file_lines));
        }
    }

    fn apply_merge(&mut self) {
        let hunk = match self.hunks.get(self.current_hunk) {
            Some(h) => h,
            None => return,
        };
        let mr = match &self.match_result {
            Some(m) => m,
            None => return,
        };

        let mut merged: Vec<String> = Vec::new();

        // lines before the matched region
        merged.extend(self.file_lines[..mr.file_start].iter().cloned());

        let replace_start = merged.len();
        merged.extend(hunk.replace.iter().cloned());
        let replace_end = merged.len();

        // lines after the matched region
        merged.extend(self.file_lines[mr.file_end..].iter().cloned());

        self.merged_preview = Some(merged.join("\n"));
        self.merged_range = Some((replace_start, replace_end));
        self.show_merged = true;
    }

    fn save_merged(&mut self) {
        let merged = match &self.merged_preview {
            Some(m) => m.clone(),
            None => return,
        };
        let path = if self.file_path.is_empty() {
            "merged_output.txt".to_string()
        } else {
            self.file_path.clone()
        };
        match std::fs::write(&path, &merged) {
            Ok(_) => self.message = Some(format!("Saved to {}", path)),
            Err(e) => self.message = Some(format!("Save failed: {}", e)),
        }
    }

    // ---- helpers ----

    fn current_hunk(&self) -> Option<&PatchHunk> {
        self.hunks.get(self.current_hunk)
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
                self.render_merged_preview(ui);
            } else {
                self.render_diff_view(ui);
            }
        });
    }
}

// =========================================================================
// Toolbar
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
                            self.recompute_match();
                        }
                    }
                }

                ui.separator();

                // hunk navigation
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

                // match score badge
                if let Some(ref mr) = self.match_result {
                    let (color, icon) = if mr.score >= 80.0 {
                        (Color32::from_rgb(80, 200, 80), "✓")
                    } else if mr.score >= 50.0 {
                        (Color32::from_rgb(220, 180, 50), "≈")
                    } else {
                        (Color32::from_rgb(220, 80, 80), "✗")
                    };

                    let score_text = format!("Match Score: {:.0}% {}", mr.score, icon);
                    let frame = Frame::none()
                        .fill(color.linear_multiply(0.25))
                        .stroke(Stroke::new(1.0, color))
                        .rounding(Rounding::same(4.0))
                        .inner_margin(Margin::symmetric(10.0, 4.0));
                    frame.show(ui, |ui| {
                        ui.label(RichText::new(&score_text).color(color).strong());
                    });
                }

                ui.separator();

                if ui.button("Apply Merge").clicked() {
                    self.apply_merge();
                }

                if self.merged_preview.is_some() {
                    if self.show_merged {
                        if ui.button("Show Diff").clicked() {
                            self.show_merged = false;
                        }
                    } else if ui.button("Show Merged").clicked() {
                        self.show_merged = true;
                    }
                    if ui.button("Save Merged…").clicked() {
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
                ui.style_mut().visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;

                if let Some(ref msg) = self.message {
                    ui.colored_label(Color32::from_rgb(220, 180, 60), msg);
                } else if let Some(hunk) = self.current_hunk() {
                    ui.label(format!("📄 {}", hunk.filename));
                    ui.separator();
                    if let Some(ref mr) = self.match_result {
                        ui.label(format!(
                            "Matched file lines {}–{}",
                            mr.file_start + 1,
                            mr.file_end
                        ));
                        ui.separator();
                        ui.label(format!(
                            "Search: {} lines | Replace: {} lines",
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
// Diff view (side-by-side)
// =========================================================================

impl MergeApp {
    fn render_diff_view(&self, ui: &mut Ui) {
        let hunk = match self.current_hunk() {
            Some(h) => h,
            None => return,
        };
        let mr = match &self.match_result {
            Some(m) => m,
            None => {
                ui.label("No file loaded.");
                return;
            }
        };

        // ---- column headers ----
        ui.horizontal(|ui| {
            let half = ui.available_width() / 2.0 - 4.0;
            Frame::none()
                .fill(Color32::from_rgb(40, 50, 70))
                .inner_margin(Margin::symmetric(6.0, 3.0))
                .show(ui, |ui| {
                    ui.set_min_width(half);
                    ui.label(
                        RichText::new("◀ SEARCH (from patch)")
                            .color(Color32::from_rgb(120, 180, 255))
                            .strong(),
                    );
                });
            Frame::none()
                .fill(Color32::from_rgb(40, 60, 50))
                .inner_margin(Margin::symmetric(6.0, 3.0))
                .show(ui, |ui| {
                    ui.set_min_width(half);
                    ui.label(
                        RichText::new("FILE (actual content) ▶")
                            .color(Color32::from_rgb(120, 220, 160))
                            .strong(),
                    );
                });
        });

        ui.separator();

        // ---- compute column widths from data ----
        let char_w = 7.5_f32;
        let num_w = 60.0_f32; // " 123 │ "
        let pad = 16.0_f32;

        let max_left = mr
            .rows
            .iter()
            .filter_map(|r| r.left.as_ref())
            .map(|s| s.len())
            .max()
            .unwrap_or(40);
        let max_right = mr
            .rows
            .iter()
            .filter_map(|r| r.right.as_ref())
            .map(|s| s.len())
            .max()
            .unwrap_or(40);

        let left_w = (max_left as f32 * char_w + num_w + pad).max(350.0);
        let right_w = (max_right as f32 * char_w + num_w + pad).max(350.0);

        let row_h = ui.text_style_height(&TextStyle::Monospace) + 4.0;

        // ---- scrollable diff ----
        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // context before match
                let ctx_start = mr.file_start.saturating_sub(3);
                for i in ctx_start..mr.file_start {
                    self.draw_context_row(ui, left_w, right_w, row_h, i);
                }

                // diff rows
                for row in &mr.rows {
                    self.draw_diff_row(ui, left_w, right_w, row_h, row);
                }

                // context after match
                let ctx_end = (mr.file_end + 3).min(self.file_lines.len());
                for i in mr.file_end..ctx_end {
                    self.draw_context_row(ui, left_w, right_w, row_h, i);
                }

                // ---- REPLACE preview ----
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(
                    RichText::new("▼ REPLACE block (will be applied)")
                        .color(Color32::from_rgb(255, 200, 100))
                        .strong(),
                );
                ui.add_space(2.0);

                for (i, line) in hunk.replace.iter().enumerate() {
                    let is_new = !hunk.search.contains(line);
                    let bg = if is_new {
                        Color32::from_rgb(35, 70, 35)
                    } else {
                        Color32::from_rgb(35, 40, 50)
                    };
                    let color = if is_new {
                        Color32::from_rgb(150, 255, 150)
                    } else {
                        Color32::from_gray(170)
                    };

                    let desired = Vec2::new(left_w + right_w + 8.0, row_h);
                    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, bg);
                    ui.painter().text(
                        Pos2::new(rect.left() + 6.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("{:>4} │ {}", i + 1, line),
                        FontId::monospace(13.0),
                        color,
                    );
                }
            });
    }

    fn draw_context_row(
        &self,
        ui: &mut Ui,
        left_w: f32,
        right_w: f32,
        row_h: f32,
        file_idx: usize,
    ) {
        ui.horizontal(|ui| {
            // left — empty
            let desired = Vec2::new(left_w, row_h);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            ui.painter().rect_filled(rect, 0.0, Color32::from_gray(24));
            ui.painter().text(
                Pos2::new(rect.left() + 6.0, rect.center().y),
                Align2::LEFT_CENTER,
                "     │ ~",
                FontId::monospace(13.0),
                Color32::from_gray(60),
            );

            // right — context line
            let desired = Vec2::new(right_w, row_h);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            ui.painter().rect_filled(rect, 0.0, Color32::from_gray(24));
            ui.painter().text(
                Pos2::new(rect.left() + 6.0, rect.center().y),
                Align2::LEFT_CENTER,
                format!("{:>4} │ {}", file_idx + 1, self.file_lines[file_idx]),
                FontId::monospace(13.0),
                Color32::from_gray(100),
            );
        });
    }

    fn draw_diff_row(
        &self,
        ui: &mut Ui,
        left_w: f32,
        right_w: f32,
        row_h: f32,
        row: &crate::diff::DiffRow,
    ) {
        ui.horizontal(|ui| {
            // ---- left cell (SEARCH) ----
            let left_bg = match row.kind {
                RowKind::Delete => Color32::from_rgb(70, 35, 35),
                _ => Color32::from_gray(30),
            };
            let left_text = match (&row.left, row.left_num) {
                (Some(content), Some(num)) => format!("{:>4} │ {}", num, content),
                (None, _) => "     │ ~".to_string(),
                _ => String::new(),
            };
            let left_color = match row.kind {
                RowKind::Delete => Color32::from_rgb(255, 150, 150),
                _ => Color32::from_gray(200),
            };

            let desired = Vec2::new(left_w, row_h);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            ui.painter().rect_filled(rect, 0.0, left_bg);
            ui.painter().text(
                Pos2::new(rect.left() + 6.0, rect.center().y),
                Align2::LEFT_CENTER,
                &left_text,
                FontId::monospace(13.0),
                left_color,
            );

            // ---- right cell (FILE) ----
            let right_bg = match row.kind {
                RowKind::Insert => Color32::from_rgb(35, 70, 35),
                _ => Color32::from_gray(30),
            };
            let right_text = match (&row.right, row.right_num) {
                (Some(content), Some(num)) => format!("{:>4} │ {}", num, content),
                (None, _) => "     │ ~".to_string(),
                _ => String::new(),
            };
            let right_color = match row.kind {
                RowKind::Insert => Color32::from_rgb(150, 255, 150),
                _ => Color32::from_gray(200),
            };

            let desired = Vec2::new(right_w, row_h);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            ui.painter().rect_filled(rect, 0.0, right_bg);
            ui.painter().text(
                Pos2::new(rect.left() + 6.0, rect.center().y),
                Align2::LEFT_CENTER,
                &right_text,
                FontId::monospace(13.0),
                right_color,
            );
        });
    }
}

// =========================================================================
// Merged preview
// =========================================================================

impl MergeApp {
    fn render_merged_preview(&self, ui: &mut Ui) {
        let merged = match &self.merged_preview {
            Some(m) => m,
            None => return,
        };
        let (rstart, rend) = match self.merged_range {
            Some(r) => r,
            None => return,
        };

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("✓ Merged Result Preview")
                    .color(Color32::from_rgb(100, 220, 100))
                    .strong(),
            );
            ui.separator();
            ui.label(format!(
                "Replaced lines {}–{} with {} new lines",
                rstart + 1,
                rend,
                rend - rstart
            ));
        });
        ui.separator();

        let row_h = ui.text_style_height(&TextStyle::Monospace) + 4.0;

        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, line) in merged.lines().enumerate() {
                    let in_replace = i >= rstart && i < rend;

                    let bg = if in_replace {
                        Color32::from_rgb(35, 55, 40)
                    } else {
                        Color32::from_gray(28)
                    };
                    let color = if in_replace {
                        Color32::from_rgb(160, 240, 160)
                    } else {
                        Color32::from_gray(200)
                    };

                    let desired = Vec2::new(ui.available_width(), row_h);
                    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, bg);
                    ui.painter().text(
                        Pos2::new(rect.left() + 6.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("{:>4} │ {}", i + 1, line),
                        FontId::monospace(13.0),
                        color,
                    );
                }
            });
    }
}
