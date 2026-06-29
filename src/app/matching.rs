//--+ file:///src/app/matching.rs
use super::palette::pal;
use super::state::MergeApp;
use super::types::SearchRow;
use crate::diff::{self, MatchResult, RowKind};
use crate::patch::PatchHunk;
use eframe::egui::Color32;
pub trait MergeMatching {
    fn recompute_match(&mut self);
    fn build_search_rows(hunk: &PatchHunk, mr: &MatchResult) -> Vec<SearchRow>;
    fn score_appearance(score: f32) -> (Color32, Color32, &'static str);
}
impl MergeMatching for MergeApp {
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
            if hunk.search.is_empty() && !hunk.replace.is_empty() {
                // New file creation: build rows for the replace lines
                let mr = MatchResult {
                    score: 100.0,
                    file_start: 0,
                    file_end: 0,
                    rows: vec![],
                    candidates: vec![],
                };
                self.search_rows = Self::build_search_rows(hunk, &mr);
                self.match_result = Some(mr);
                if self.cursor_line.is_none() {
                    self.cursor_line = Some(0);
                }
                self.scroll_to_match = true;
            } else {
                self.match_result = None;
                self.search_rows = Vec::new();
            }
        } else {
            let best = diff::find_best_match(&hunk.search, &self.file_lines);
            if best.score <= 15.0 {
                // Ignore extremely low scores/trivial matches
                self.match_result = None;
                self.search_rows = Vec::new();
                return;
            }
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
}