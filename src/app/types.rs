use super::palette::pal;
use crate::diff::RowKind;
use eframe::egui::Color32;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub enum Action {
    DeleteLines(usize),
}

#[derive(Clone)]
pub struct SearchRow {
    pub text: String,
    pub file_idx: Option<usize>,
    pub kind: RowKind,
}

#[derive(Clone)]
pub struct StatusMessage {
    pub text: String,
    pub kind: MessageKind,
}

#[derive(Clone, PartialEq)]
pub enum MessageKind {
    Info,
    Success,
    Warning,
    Error,
}

impl StatusMessage {
    pub fn info(s: impl Into<String>) -> Self {
        Self { text: s.into(), kind: MessageKind::Info }
    }
    pub fn success(s: impl Into<String>) -> Self {
        Self { text: s.into(), kind: MessageKind::Success }
    }
    pub fn warning(s: impl Into<String>) -> Self {
        Self { text: s.into(), kind: MessageKind::Warning }
    }
    pub fn error(s: impl Into<String>) -> Self {
        Self { text: s.into(), kind: MessageKind::Error }
    }
    pub fn color(&self) -> Color32 {
        match self.kind {
            MessageKind::Info    => pal::ACCENT_INFO,
            MessageKind::Success => pal::ACCENT_GOOD,
            MessageKind::Warning => pal::ACCENT_WARN,
            MessageKind::Error   => pal::ACCENT_BAD,
        }
    }
}

#[derive(Clone)]
pub struct FileState {
    pub lines: Vec<String>,
    pub applied_hunks: HashSet<usize>,
    pub history: Vec<(Vec<String>, usize)>,
    pub merged_range: Option<(usize, usize)>,
}

/// A pair of line indices in the **file panel** (right buffer).
/// `ma` = start (inclusive), `mb` = end (exclusive, like file_end).
/// When only `ma` is set, the replace is inserted *before* that line
/// (same semantics as `manual_anchor`).  When both are set, lines
/// `[ma, mb)` are replaced by the hunk's replace block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FileAnchor {
    pub a: usize,           // line index of mark-a  (start)
    pub b: Option<usize>,   // line index of mark-b  (end, exclusive)
}

impl FileAnchor {
    pub fn start_only(a: usize) -> Self { Self { a, b: None } }
    pub fn range(a: usize, b: usize) -> Self { Self { a, b: Some(b) } }

    /// file_start for apply: always `a`
    pub fn file_start(&self) -> usize { self.a }

    /// file_end for apply: `b` if set, else `a` (point insert)
    pub fn file_end(&self) -> usize { self.b.unwrap_or(self.a) }

    pub fn label(&self) -> String {
        match self.b {
            None    => format!("ma:{}", self.a + 1),
            Some(b) => format!("ma:{}–mb:{}", self.a + 1, b),
        }
    }
}
