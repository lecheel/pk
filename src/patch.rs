#[derive(Debug, Clone)]
pub struct PatchHunk {
    pub filename: String,
    pub search: Vec<String>,
    pub replace: Vec<String>,
}

pub fn parse_patches(content: &str) -> Vec<PatchHunk> {
    if content.contains("*** Begin Patch")
        || content.contains("diff --git")
        || content.contains("--- a/")
        || content.contains("+++ b/")
    {
        return parse_git_or_unified_patches(content);
    }
    let mut hunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut current_filename = String::new();
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("filename ") {
            current_filename = trimmed["filename ".len()..].trim().to_string();
            i += 1;
        } else if trimmed.contains("<<<<<<< SEARCH") {
            if current_filename.is_empty() {
                i += 1;
                continue;
            }
            i += 1;
            let mut search = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with("=======") {
                search.push(lines[i].to_string());
                i += 1;
            }
            if i >= lines.len() {
                break;
            }
            i += 1;
            let mut replace = Vec::new();
            while i < lines.len() && !lines[i].contains(">>>>>>> REPLACE") {
                if lines[i].trim().starts_with("filename ") || lines[i].contains("<<<<<<< SEARCH") {
                    break;
                }
                replace.push(lines[i].to_string());
                i += 1;
            }
            if i < lines.len() && lines[i].contains(">>>>>>> REPLACE") {
                i += 1;
            }
            if !search.is_empty() || !replace.is_empty() {
                hunks.push(PatchHunk {
                    filename: current_filename.clone(),
                    search,
                    replace,
                });
            }
        } else {
            i += 1;
        }
    }
    hunks
}

fn parse_git_or_unified_patches(content: &str) -> Vec<PatchHunk> {
    let mut hunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut current_filename = String::new();
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("*** Update File: ") {
            current_filename = line["*** Update File: ".len()..].trim().to_string();
        } else if line.starts_with("*** Add File: ") {
            current_filename = line["*** Add File: ".len()..].trim().to_string();
        } else if line.starts_with("diff --git ") {
            if let Some(b_part) = line.split(" b/").nth(1) {
                current_filename = b_part.trim().to_string();
            }
        } else if line.starts_with("+++ b/") {
            current_filename = line["+++ b/".len()..].trim().to_string();
        } else if line.starts_with("@@ ") || line.trim() == "@@" {
            i += 1;
            let mut search = Vec::new();
            let mut replace = Vec::new();
            while i < lines.len() {
                let l = lines[i];
                if l.starts_with("diff --git ")
                    || l.starts_with("*** ")
                    || l.starts_with("@@ ")
                    || l.trim() == "@@"
                {
                    break;
                }
                if let Some(stripped) = l.strip_prefix(' ') {
                    search.push(stripped.to_string());
                    replace.push(stripped.to_string());
                } else if let Some(stripped) = l.strip_prefix('-') {
                    search.push(stripped.to_string());
                } else if let Some(stripped) = l.strip_prefix('+') {
                    replace.push(stripped.to_string());
                } else if l.is_empty() {
                    search.push(String::new());
                    replace.push(String::new());
                }
                i += 1;
            }
            if !search.is_empty() || !replace.is_empty() {
                if current_filename.is_empty() {
                    current_filename = "unknown_file".to_string();
                }
                hunks.push(PatchHunk {
                    filename: current_filename.clone(),
                    search,
                    replace,
                });
            }
            continue;
        }
        i += 1;
    }
    hunks
}
