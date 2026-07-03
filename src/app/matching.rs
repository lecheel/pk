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
            let best = diff::find_best_match(&hunk.search, &self.file_lines, self.ignore_comments);
            // Tier 1: Hard floor. Discard completely unrecognizable matches.
            if best.score < self.min_match_floor {
                self.match_result = None;
                self.search_rows = Vec::new();
                return;
            }
            let mut mr = if best.candidates.is_empty() {
                best
            } else {
                let idx = self.candidate_index.min(best.candidates.len() - 1);
                if idx == 0 {
                    best
                } else {
                    let (start, end, _) = best.candidates[idx];
                    let cands = best.candidates.clone();
                    let mut mr = diff::compute_match_for_window(
                        &hunk.search,
                        &self.file_lines,
                        start,
                        end,
                        self.ignore_comments,
                    );
                    mr.candidates = cands;
                    mr
                }
            };
            fix_missing_trailing_brackets(hunk, &mut mr, &self.file_lines);
            self.search_rows = Self::build_search_rows(hunk, &mr);
            if self.cursor_line.is_none() {
                self.cursor_line = Some(mr.file_start);
            }
            self.match_result = Some(mr);
            self.scroll_to_match = true;
        }
    }
    fn build_search_rows(hunk: &PatchHunk, mr: &MatchResult) -> Vec<SearchRow> {
        // Build rows directly from the real search<->file alignment (mr.rows),
        // instead of an unrelated search<->replace diff. mr.rows already has the
        // correct per-line file index (right_num, 1-based) for every matched line,
        // including cases where the file contains extra lines not present in search
        // (e.g. an inserted line like `self.history.clear();`).
        let mut rows = Vec::new();
        for row in &mr.rows {
            if let Some(left) = &row.left {
                match row.kind {
                    RowKind::Equal => {
                        rows.push(SearchRow {
                            text: left.clone(),
                            // right_num is 1-based; convert to 0-based file index
                            file_idx: row.right_num.map(|n| n.saturating_sub(1)),
                            kind: RowKind::Equal,
                        });
                    }
                    RowKind::Delete => {
                        rows.push(SearchRow {
                            text: left.clone(),
                            file_idx: None,
                            kind: RowKind::Delete,
                        });
                    }
                    RowKind::Insert => {
                        // Insert rows in mr.rows only have `right` (file-only lines),
                        // never `left`, so this branch is unreachable here.
                    }
                }
            }
        }
        let _ = hunk; // hunk.search order/count is implicitly preserved via mr.rows
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

fn fix_missing_trailing_brackets(hunk: &PatchHunk, mr: &mut MatchResult, file_lines: &[String]) {
    let mut s = hunk.search.clone();
    while s.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        s.pop();
    }
    let mut r = hunk.replace.clone();
    while r.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        r.pop();
    }

    if r.is_empty() || s.is_empty() {
        return;
    }

    // Count trailing closing brackets in replace block
    let mut r_brackets = 0;
    for line in r.iter().rev() {
        if line.trim().starts_with('}') {
            r_brackets += 1;
        } else {
            break;
        }
    }

    // Count trailing closing brackets in search block
    let mut s_brackets = 0;
    for line in s.iter().rev() {
        if line.trim().starts_with('}') {
            s_brackets += 1;
        } else {
            break;
        }
    }

    // If replace has more closing brackets than search, and the file has them
    // immediately after the match, extend the match to include them.
    if r_brackets > s_brackets {
        let extra_brackets = r_brackets - s_brackets;
        let mut file_idx = mr.file_end;
        for _ in 0..extra_brackets {
            if file_idx >= file_lines.len() {
                return;
            }
            let trimmed = file_lines[file_idx].trim();
            if trimmed.starts_with('}') {
                file_idx += 1;
            } else {
                return;
            }
        }
        mr.file_end = file_idx;
    }
}