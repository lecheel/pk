#[derive(Debug, Clone)]
pub struct PatchHunk {
    pub filename: String,
    pub search: Vec<String>,
    pub replace: Vec<String>,
}

pub fn parse_patches(content: &str) -> Vec<PatchHunk> {
    let trimmed = content.trim();
    if trimmed.contains("<patch>") || trimmed.contains("<<<<<<< SEARCH") {
        return parse_aider_patches(content);
    }
    if trimmed.contains("diff --git") || trimmed.contains("--- ") || trimmed.contains("+++ ") {
        let hunks = parse_git_or_unified_patches(content);
        if !hunks.is_empty() {
            return hunks;
        }
    }
    parse_raw_paste(content)
}

fn parse_aider_patches(content: &str) -> Vec<PatchHunk> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<PatchHunk> = None;
    let mut state = 0; // 0: outside, 1: search, 2: replace
    let mut current_filename = String::new();

    for line in content.lines() {
        if line.starts_with("<<<<<<< SEARCH") {
            current_hunk = Some(PatchHunk {
                filename: current_filename.clone(),
                search: Vec::new(),
                replace: Vec::new(),
            });
            state = 1;
            current_filename.clear();
        } else if line.starts_with("=======") {
            state = 2;
        } else if line.starts_with(">>>>>>> REPLACE") {
            if let Some(h) = current_hunk.take() {
                hunks.push(h);
            }
            state = 0;
        } else {
            if state == 0 {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("// ") {
                    if !rest.is_empty() && (rest.contains('/') || rest.ends_with(".rs")) {
                        current_filename = rest.trim().to_string();
                    }
                } else if let Some(rest) = trimmed.strip_prefix("# ") {
                    if !rest.is_empty() && (rest.contains('/') || rest.ends_with(".rs")) {
                        current_filename = rest.trim().to_string();
                    }
                } else if let Some(rest) = trimmed.strip_prefix("filename ") {
                    current_filename = rest.trim().to_string();
                } else if let Some(rest) = trimmed.strip_prefix("+++ b/") {
                    current_filename = rest.trim().to_string();
                } else if let Some(rest) = trimmed.strip_prefix("+++ ") {
                    current_filename = rest.trim().to_string();
                }
            } else if state == 1 {
                if let Some(h) = current_hunk.as_mut() {
                    h.search.push(line.to_string());
                }
            } else if state == 2 {
                if let Some(h) = current_hunk.as_mut() {
                    h.replace.push(line.to_string());
                }
            }
        }
    }

    for h in &mut hunks {
        while h.search.last().map(|l| l.is_empty()).unwrap_or(false) {
            h.search.pop();
        }
        while h.replace.last().map(|l| l.is_empty()).unwrap_or(false) {
            h.replace.pop();
        }
    }

    hunks
}

fn parse_git_or_unified_patches(content: &str) -> Vec<PatchHunk> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<PatchHunk> = None;
    let mut current_filename = String::new();
    let mut state = 0; // 0: outside, 1: search, 2: replace

    for line in content.lines() {
        if line.starts_with("diff --git") {
            if let Some(h) = current_hunk.take() {
                hunks.push(h);
            }
            state = 0;
        } else if line.starts_with("+++ b/") {
            current_filename = line.strip_prefix("+++ b/").unwrap().trim().to_string();
        } else if line.starts_with("+++ ") {
            current_filename = line.strip_prefix("+++ ").unwrap().trim().to_string();
        } else if line.starts_with("@@") {
            if let Some(h) = current_hunk.take() {
                hunks.push(h);
            }
            current_hunk = Some(PatchHunk {
                filename: current_filename.clone(),
                search: Vec::new(),
                replace: Vec::new(),
            });
            state = 1;
        } else if line.starts_with("-") && !line.starts_with("---") {
            if state == 1 {
                if let Some(h) = current_hunk.as_mut() {
                    h.search.push(line[1..].to_string());
                }
            }
        } else if line.starts_with("+") && !line.starts_with("+++") {
            if state == 1 {
                if let Some(h) = current_hunk.as_mut() {
                    h.replace.push(line[1..].to_string());
                }
            }
        } else if line.starts_with(" ") {
            if state == 1 {
                if let Some(h) = current_hunk.as_mut() {
                    h.search.push(line[1..].to_string());
                    h.replace.push(line[1..].to_string());
                }
            }
        }
    }

    if let Some(h) = current_hunk.take() {
        hunks.push(h);
    }

    hunks
}

fn parse_raw_paste(content: &str) -> Vec<PatchHunk> {
    let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut filename = String::new();
    let mut search_start = 0;

    // Look for the first non-empty line for a potential smart filename
    while search_start < lines.len() {
        let first_line = lines[search_start].trim();
        if first_line.is_empty() {
            search_start += 1;
            continue;
        }

        if let Some(rest) = first_line.strip_prefix("// ") {
            if !rest.is_empty() && (rest.contains('/') || rest.ends_with(".rs")) {
                filename = rest.trim().to_string();
                search_start += 1;
                break;
            }
        } else if let Some(rest) = first_line.strip_prefix("# ") {
            if !rest.is_empty() && (rest.contains('/') || rest.ends_with(".rs")) {
                filename = rest.trim().to_string();
                search_start += 1;
                break;
            }
        } else if first_line.starts_with("filename ") {
            filename = first_line
                .strip_prefix("filename ")
                .unwrap()
                .trim()
                .to_string();
            search_start += 1;
            break;
        } else if first_line.starts_with("+++ b/") {
            filename = first_line
                .strip_prefix("+++ b/")
                .unwrap()
                .trim()
                .to_string();
            search_start += 1;
            break;
        } else if first_line.starts_with("+++ ") {
            filename = first_line.strip_prefix("+++ ").unwrap().trim().to_string();
            search_start += 1;
            break;
        }
        break;
    }

    let search_lines: Vec<String> = lines[search_start..]
        .iter()
        .filter(|l| {
            !l.starts_with("<<<<<<<") && !l.starts_with("=======") && !l.starts_with(">>>>>>>")
        })
        .cloned()
        .collect();

    if search_lines.is_empty() && filename.is_empty() {
        return Vec::new();
    }

    vec![PatchHunk {
        filename,
        search: search_lines,
        replace: Vec::new(),
    }]
}
