use crate::patch::PatchHunk;

#[cfg(not(target_arch = "wasm32"))]
pub fn get_clipboard_text() -> Option<String> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut cb| cb.get_text().ok())
}

#[cfg(target_arch = "wasm32")]
pub fn get_clipboard_text() -> Option<String> {
    None
}

pub fn parse_clipboard_patch(pasted: &str) -> Vec<PatchHunk> {
    let trimmed = pasted.trim();
    if trimmed.contains("<patch>") {
        return crate::patch::parse_patches(pasted);
    }
    if trimmed.contains("diff --git") || trimmed.contains("--- ") || trimmed.contains("+++ ") {
        let hunks = crate::patch::parse_patches(pasted);
        if !hunks.is_empty() {
            return hunks;
        }
    }
    let lines: Vec<String> = pasted.lines().map(|s| s.to_string()).collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let mut filename = String::new();
    let mut search_start = 0;
    let first_line = lines[0].trim();
    if first_line.starts_with("filename ") {
        filename = first_line
            .strip_prefix("filename ")
            .unwrap()
            .trim()
            .to_string();
        search_start = 1;
    } else if first_line.starts_with("+++ b/") {
        filename = first_line
            .strip_prefix("+++ b/")
            .unwrap()
            .trim()
            .to_string();
        search_start = 1;
    } else if first_line.starts_with("+++ ") {
        filename = first_line.strip_prefix("+++ ").unwrap().trim().to_string();
        search_start = 1;
    }
    let search_lines: Vec<String> = lines[search_start..]
        .iter()
        .filter(|l| {
            !l.starts_with("<<<<<<<") && !l.starts_with("=======") && !l.starts_with(">>>>>>>")
        })
        .cloned()
        .collect();
    vec![PatchHunk {
        filename,
        search: search_lines,
        replace: Vec::new(),
    }]
}
