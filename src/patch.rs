#[derive(Debug, Clone)]
pub struct PatchHunk {
    pub filename: String,
    pub search: Vec<String>,
    pub replace: Vec<String>,
}

pub fn parse_patches(content: &str) -> Vec<PatchHunk> {
    eprintln!("\n--- [DEBUG parse_patches] Starting to parse patch content ---");
    if content.contains("*** Begin Patch")
        || content.contains("diff --git")
        || content.contains("--- a/")
        || content.contains("+++ b/")
    {
        eprintln!("[DEBUG parse_patches] Detected Git/Unified diff format. Delegating to parse_git_or_unified_patches.");
        return parse_git_or_unified_patches(content);
    }
    eprintln!("[DEBUG parse_patches] Detected Aider SEARCH/REPLACE format.");
    let mut hunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut current_filename = String::new();
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("filename ") {
            current_filename = trimmed["filename ".len()..].trim().to_string();
            eprintln!("[DEBUG parse_patches] Found filename: {}", current_filename);
            i += 1;
        } else if trimmed.contains("<<<<<<< SEARCH") {
            eprintln!("[DEBUG parse_patches] Found <<<<<<< SEARCH at line {}", i);
            i += 1;
            let mut search = Vec::new();
            while i < lines.len() {
                let lt = lines[i].trim();
                if lt == "======="
                    || lt == ">>>>>>> REPLACE"
                    || lt == "<<<<<<< SEARCH"
                    || lt.starts_with("filename ")
                {
                    break;
                }
                search.push(lines[i].to_string());
                i += 1;
            }
            eprintln!(
                "[DEBUG parse_patches] Parsed SEARCH block ({} lines). Next line: {:?}",
                search.len(),
                lines.get(i).unwrap_or(&"EOF")
            );

            if i >= lines.len() {
                eprintln!("[DEBUG parse_patches] Reached EOF prematurely while parsing SEARCH.");
                break;
            }
            if lines[i].trim() == "=======" {
                eprintln!(
                    "[DEBUG parse_patches] Found ======= delimiter at line {}",
                    i
                );
                i += 1;
            } else {
                eprintln!("[DEBUG parse_patches] WARNING: Expected ======= but found something else! Breaking.");
            }

            let mut replace = Vec::new();
            while i < lines.len() {
                let lt = lines[i].trim();
                if lt == ">>>>>>> REPLACE"
                    || lt == "======="
                    || lt == "<<<<<<< SEARCH"
                    || lt.starts_with("filename ")
                {
                    break;
                }
                replace.push(lines[i].to_string());
                i += 1;
            }
            eprintln!(
                "[DEBUG parse_patches] Parsed REPLACE block ({} lines). Next line: {:?}",
                replace.len(),
                lines.get(i).unwrap_or(&"EOF")
            );

            if i < lines.len() && lines[i].trim() == ">>>>>>> REPLACE" {
                eprintln!(
                    "[DEBUG parse_patches] Found >>>>>>> REPLACE delimiter at line {}",
                    i
                );
                i += 1;
            } else {
                eprintln!("[DEBUG parse_patches] WARNING: Expected >>>>>>> REPLACE but found something else! Breaking.");
            }

            if !search.is_empty() || !replace.is_empty() {
                eprintln!(
                    "[DEBUG parse_patches] Successfully created PatchHunk for {}",
                    current_filename
                );
                hunks.push(PatchHunk {
                    filename: current_filename.clone(),
                    search,
                    replace,
                });
            } else {
                eprintln!(
                    "[DEBUG parse_patches] SKIPPED empty PatchHunk for {}",
                    current_filename
                );
            }
        } else {
            i += 1;
        }
    }
    eprintln!(
        "[DEBUG parse_patches] Finished parsing. Total hunks: {}\n",
        hunks.len()
    );
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
