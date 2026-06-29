#[derive(Debug, Clone)]
pub struct PatchHunk {
    pub filename: String,
    pub search: Vec<String>,
    pub replace: Vec<String>,
}

pub fn parse_patches(content: &str) -> Vec<PatchHunk> {
    if content.contains("*** Begin Patch") {
        return parse_unified_patches(content);
    }

    let mut hunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("filename ") {
            let filename = trimmed["filename ".len()..].trim().to_string();
            i += 1;
            while i < lines.len() && !lines[i].contains("<<<<<<< SEARCH") {
                i += 1;
            }
            if i >= lines.len() {
                break;
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
                replace.push(lines[i].to_string());
                i += 1;
            }
            hunks.push(PatchHunk {
                filename,
                search,
                replace,
            });
        }
        i += 1;
    }
    hunks
}

fn parse_unified_patches(content: &str) -> Vec<PatchHunk> {
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
        } else if line.trim() == "@@" {
            i += 1;
            let mut search = Vec::new();
            let mut replace = Vec::new();
            while i < lines.len() {
                let l = lines[i];
                if l.trim() == "@@" || l.starts_with("*** ") {
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
