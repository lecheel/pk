/// Vim-style command parsing shared by any panel that wants dd/yy/p/P/gg/G
/// navigation without depending on a specific buffer type.
#[derive(Debug, Clone, Copy)]
pub enum VimCmd {
    DeleteLines(usize),
    Yank,
    PasteBelow,
    PasteAbove,
    GotoTop,
    GotoBottom,
    Undo,
    RepeatLast,
    NextSearchMatch,
    PrevSearchMatch,
    NextGitHunk,
    PrevGitHunk,
    RevertToHead,
}

/// Feed accumulated keystroke text into this. Returns the recognized
/// command (if any) and whether the caller should clear its buffer.
pub fn parse_vim_buffer(buf: &str) -> (Option<VimCmd>, bool) {
    let buf_t = buf.trim();
    let lower = buf_t.to_lowercase();
    if buf_t == "n" {
        return (Some(VimCmd::NextSearchMatch), true);
    }
    if buf_t == "N" {
        return (Some(VimCmd::PrevSearchMatch), true);
    }
    if buf_t == "]h" {
        return (Some(VimCmd::NextGitHunk), true);
    }
    if buf_t == "[h" {
        return (Some(VimCmd::PrevGitHunk), true);
    }
    if lower == "u" {
        return (Some(VimCmd::Undo), true);
    }
    if lower == "." {
        return (Some(VimCmd::RepeatLast), true);
    }
    if buf_t == "gg" {
        return (Some(VimCmd::GotoTop), true);
    }
    if buf_t == "G" {
        return (Some(VimCmd::GotoBottom), true);
    }
    if buf_t == "yy" {
        return (Some(VimCmd::Yank), true);
    }
    if lower == "p" {
        return (Some(VimCmd::PasteBelow), true);
    }
    if buf_t == "P" {
        return (Some(VimCmd::PasteAbove), true);
    }
    if lower.ends_with("dd") {
        let num_part = &lower[..lower.len() - 2];
        let n = if num_part.is_empty() {
            1
        } else {
            num_part.parse::<usize>().unwrap_or(0)
        };
        if n > 0 {
            return (Some(VimCmd::DeleteLines(n)), true);
        }
        return (None, true);
    }
    if buf_t.len() > 5 {
        return (None, true);
    }
    let allowed = buf_t.chars().all(|c| {
        c.is_ascii_digit()
            || c == 'd'
            || c == 'g'
            || c == 'G'
            || c == '['
            || c == ']'
            || c == 'h'
            || c == 'y'
            || c == 'p'
            || c == 'P'
    });
    if !allowed {
        return (None, true);
    }
    (None, false)
}
