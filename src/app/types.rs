use super::palette::pal;
use crate::diff::RowKind;
use eframe::egui::Color32;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub enum Action {
    DeleteLines(usize),
    DeleteFunction,
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
        Self {
            text: s.into(),
            kind: MessageKind::Info,
        }
    }
    pub fn success(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            kind: MessageKind::Success,
        }
    }
    pub fn warning(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            kind: MessageKind::Warning,
        }
    }
    pub fn error(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            kind: MessageKind::Error,
        }
    }
    pub fn color(&self) -> Color32 {
        match self.kind {
            MessageKind::Info => pal::ACCENT_INFO,
            MessageKind::Success => pal::ACCENT_GOOD,
            MessageKind::Warning => pal::ACCENT_WARN,
            MessageKind::Error => pal::ACCENT_BAD,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FileAnchor {
    pub id: char,
    pub line: usize,
    pub end_line: Option<usize>,
}

impl FileAnchor {
    pub fn start_only(id: char, line: usize) -> Self {
        Self {
            id,
            line,
            end_line: None,
        }
    }
    pub fn file_start(&self) -> usize {
        self.line
    }
    pub fn file_end(&self) -> usize {
        self.line
    }
    pub fn label(&self) -> String {
        format!("m{}:{}", self.id, self.line + 1)
    }
}
