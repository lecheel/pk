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

#[derive(Clone)]
struct InteractiveRow {
    kind: RowKind,
    left: String,
    right: Option<String>,
    right_num: Option<usize>,
    applied: bool,
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
    interactive_rows: Vec<InteractiveRow>,

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
            interactive_rows: Vec::new(),
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
        self.interactive_rows = self.build_interactive_rows();
    }

    fn build_interactive_rows(&self) -> Vec<InteractiveRow> {
        let hunk = match self.current_hunk() {
            Some(h) => h,
            None => return Vec::new(),
        };
        let mr = match &self.match_result {
            Some(m) => m,
            None => return Vec::new(),
        };

        let patch_diff = diff::diff_patch(&hunk.search, &hunk.replace);
        let mut rows = Vec::new();
        let mut file_idx = mr.file_start;

        for (kind, left, _right) in patch_diff {
            match kind {
                RowKind::Equal => {
                    let file_line = self.file_lines.get(file_idx).cloned();
                    let num = self.file_lines.get(file_idx).map(|_| file_idx + 1);
                    file_idx += 1;
                    rows.push(InteractiveRow {
                        kind,
                        left: left.unwrap_or_default(),
                        right: file_line,
                        right_num: num,
                        applied: true,
                    });
                }
                RowKind::Delete => {
                    let file_line = self.file_lines.get(file_idx).cloned();
                    let num = self.file_lines.get(file_idx).map(|_| file_idx + 1);
                    file_idx += 1;
                    rows.push(InteractiveRow {
                        kind,
                        left: left.unwrap_or_default(),
                        right: file_line,
                        right_num: num,
                        applied: false,
                    });
                }
                RowKind::Insert => {
                    rows.push(InteractiveRow {
                        kind,
                        left: left.unwrap_or_default(),
                        right: None,
                        right_num: None,
                        applied: false,
                    });
                }
            }
        }

        // Append any leftover file lines from the matched region
        while file_idx < mr.file_end {
            let file_line = self.file_lines.get(file_idx).cloned();
            let num = Some(file_idx + 1);
            rows.push(InteractiveRow {
                kind: RowKind::Equal,
                left: String::new(),
                right: file_line,
                right_num: num,
                applied: true,
            });
            file_idx += 1;
        }

        rows
    }

    fn apply_merge(&mut self) {
        let mut output = Vec::new();
        let mr = match &self.match_result {
            Some(m) => m,
            None => return,
        };

        output.extend(self.file_lines[..mr.file_start].iter().cloned());

        let mut interactive_len = 0;
        for row in &self.interactive_rows {
            let in_output = match row.kind {
                RowKind::Equal => true,
                RowKind::Insert => row.applied,
                RowKind::Delete => !row.applied,
            };
            if in_output {
                let line = match row.kind {
                    RowKind::Insert => row.left.clone(),
                    _ => row.right.clone().unwrap_or_default(),
                };
                output.push(line);
                interactive_len += 1;
            }
        }

        output.extend(self.file_lines[mr.file_end..].iter().cloned());

        self.merged_preview = Some(output.join("\n"));
        self.merged_range = Some((mr.file_start, mr.file_start + interactive_len));
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

    fn current_hunk(&self) -> Option<&PatchHunk> {
        self.hunks.get(self.current_hunk)
    }

    fn truncate(text: &str, max_chars: usize) -> String {
        if text.chars().count() > max_chars {
            let mut t: String = text.chars().take(max_chars.saturating_sub(1)).collect();
            t.push('…');
            t
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
                self.render_merged_preview(ui);
            } else {
                self.render_diff_view(ui);
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
// Interactive Diff View (3 Panels)
// =========================================================================

impl MergeApp {
    fn render_diff_view(&mut self, ui: &mut Ui) {
        let mr = match &self.match_result {
            Some(m) => m,
            None => {
                ui.label("No file loaded.");
                return;
            }
        };

        // ---- column headers ----
        ui.horizontal(|ui| {
            let half = (ui.available_width() / 2.0 - 15.0).max(300.0);
            Frame::none()
                .fill(Color32::from_rgb(40, 50, 70))
                .inner_margin(Margin::symmetric(6.0, 3.0))
                .show(ui, |ui| {
                    ui.set_min_width(half);
                    ui.label(
                        RichText::new("◀ TAB A (Patch)")
                            .color(Color32::from_rgb(120, 180, 255))
                            .strong(),
                    );
                });

            ui.add_space(30.0);

            Frame::none()
                .fill(Color32::from_rgb(40, 60, 50))
                .inner_margin(Margin::symmetric(6.0, 3.0))
                .show(ui, |ui| {
                    ui.set_min_width(half);
                    ui.label(
                        RichText::new("TAB B (File) ▶")
                            .color(Color32::from_rgb(120, 220, 160))
                            .strong(),
                    );
                });
        });

        ui.separator();

        let available_width = ui.available_width();
        let half = (available_width / 2.0 - 15.0).max(300.0);
        let left_w = half;
        let right_w = half;
        let gutter_w = 30.0;

        let char_w = 7.5_f32;
        let pad = 16.0_f32;
        let max_chars_left = ((left_w - pad) / char_w).floor() as usize;
        let max_chars_right = ((right_w - pad) / char_w).floor() as usize;

        let row_h = ui.text_style_height(&TextStyle::Monospace) + 4.0;

        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Context before match
                let ctx_start = mr.file_start.saturating_sub(3);
                for i in ctx_start..mr.file_start {
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(left_w, row_h),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new("~").color(Color32::from_gray(60)).monospace(),
                                );
                            },
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(gutter_w, row_h),
                            Layout::centered_and_justified(Direction::LeftToRight),
                            |ui| {},
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(right_w, row_h),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{:>4} │ {}",
                                        i + 1,
                                        Self::truncate(&self.file_lines[i], max_chars_right)
                                    ))
                                    .color(Color32::from_gray(100))
                                    .monospace(),
                                );
                            },
                        );
                    });
                }

                // Interactive rows
                let mut i = 0;
                while i < self.interactive_rows.len() {
                    let row = self.interactive_rows[i].clone();
                    let is_equal = row.kind == RowKind::Equal;
                    let is_block_start = !is_equal
                        && (i == 0
                            || self.interactive_rows[i - 1].kind == RowKind::Equal
                            || self.interactive_rows[i - 1].applied != row.applied);

                    ui.horizontal(|ui| {
                        // Left Panel
                        ui.allocate_ui_with_layout(
                            Vec2::new(left_w, row_h),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                let (symbol, color) = match row.kind {
                                    RowKind::Equal => ("=", Color32::from_gray(100)),
                                    RowKind::Insert => (">", Color32::from_rgb(100, 200, 100)),
                                    RowKind::Delete => ("<", Color32::from_rgb(200, 100, 100)),
                                };
                                let text = format!(
                                    "{} {}",
                                    symbol,
                                    Self::truncate(&row.left, max_chars_left)
                                );
                                ui.label(RichText::new(text).color(color).monospace());
                            },
                        );

                        // Middle Gutter
                        ui.allocate_ui_with_layout(
                            Vec2::new(gutter_w, row_h),
                            Layout::centered_and_justified(Direction::LeftToRight),
                            |ui| {
                                if is_block_start {
                                    let btn_text = if row.applied { "◀" } else { "▶" };
                                    if ui.button(btn_text).clicked() {
                                        let new_applied = !row.applied;
                                        let mut j = i;
                                        while j < self.interactive_rows.len() {
                                            let r = &self.interactive_rows[j];
                                            if r.kind == RowKind::Equal || r.applied != row.applied
                                            {
                                                break;
                                            }
                                            self.interactive_rows[j].applied = new_applied;
                                            j += 1;
                                        }
                                    }
                                }
                            },
                        );

                        // Right Panel
                        ui.allocate_ui_with_layout(
                            Vec2::new(right_w, row_h),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                let show_right = match row.kind {
                                    RowKind::Equal => true,
                                    RowKind::Insert => row.applied,
                                    RowKind::Delete => !row.applied,
                                };
                                if show_right {
                                    let text = match row.kind {
                                        RowKind::Insert => row.left.clone(),
                                        _ => row.right.clone().unwrap_or_default(),
                                    };
                                    let color = match row.kind {
                                        RowKind::Insert => Color32::from_rgb(150, 255, 150),
                                        RowKind::Delete => Color32::from_gray(200),
                                        _ => Color32::from_gray(200),
                                    };
                                    let num_str = match row.right_num {
                                        Some(n) => format!("{:>4}", n),
                                        None => "   ~".to_string(),
                                    };
                                    ui.label(
                                        RichText::new(format!(
                                            "{} │ {}",
                                            num_str,
                                            Self::truncate(&text, max_chars_right)
                                        ))
                                        .color(color)
                                        .monospace(),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("   ~ │ ~")
                                            .color(Color32::from_gray(60))
                                            .monospace(),
                                    );
                                }
                            },
                        );
                    });
                    i += 1;
                }

                // Context after match
                let ctx_end = (mr.file_end + 3).min(self.file_lines.len());
                for i in mr.file_end..ctx_end {
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(left_w, row_h),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new("~").color(Color32::from_gray(60)).monospace(),
                                );
                            },
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(gutter_w, row_h),
                            Layout::centered_and_justified(Direction::LeftToRight),
                            |ui| {},
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(right_w, row_h),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{:>4} │ {}",
                                        i + 1,
                                        Self::truncate(&self.file_lines[i], max_chars_right)
                                    ))
                                    .color(Color32::from_gray(100))
                                    .monospace(),
                                );
                            },
                        );
                    });
                }
            });
    }
}

// =========================================================================
// Merged Preview
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
        let char_w = 7.5_f32;
        let num_w = 60.0_f32;
        let pad = 16.0_f32;
        let max_chars = ((ui.available_width() - num_w - pad) / char_w).floor() as usize;

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
                        format!("{:>4} │ {}", i + 1, Self::truncate(line, max_chars)),
                        FontId::monospace(13.0),
                        color,
                    );
                }
            });
    }
}
