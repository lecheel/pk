#[derive(Debug, Clone)]
pub struct PatchHunk {
    pub filename: String,
    pub search: Vec<String>,
    pub replace: Vec<String>,
}

pub fn parse_patches(content: &str) -> Vec<PatchHunk> {
    let trimmed = content.trim();
    if trimmed.contains("<patch>") || trimmed.contains("<<<<<<< SEARCH") {
        let hunks = parse_aider_patches(content);
        if !hunks.is_empty() {
            return hunks;
        }
    }
    if trimmed.contains("// === SKELETON MODE") || trimmed.contains("//--+ file:///") {
        return parse_skeleton_patches(content);
    }
    if trimmed.contains("diff --git") || trimmed.contains("--- ") || trimmed.contains("+++ ") {
        let hunks = parse_git_or_unified_patches(content);
        if !hunks.is_empty() {
            return hunks;
        }
    }
    parse_raw_paste(content)
}

fn parse_skeleton_patches(content: &str) -> Vec<PatchHunk> {
    let mut hunks = Vec::new();
    let mut current_filename = String::new();
    let mut current_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        // Check for skeleton file header: //--+ file:///src/main.rs [27 LOC | 1 bodies]
        if line.starts_with("//--+ file:///") {
            // Save previous hunk if exists
            if !current_filename.is_empty() {
                hunks.push(PatchHunk {
                    filename: current_filename.clone(),
                    search: current_lines.clone(),
                    replace: Vec::new(),
                });
                current_lines.clear();
            }

            // Extract filename from file:/// URL
            if let Some(rest) = line.strip_prefix("//--+ file:///") {
                // Remove metadata like [27 LOC | 1 bodies]
                let filename = if let Some(bracket_pos) = rest.find('[') {
                    rest[..bracket_pos].trim()
                } else {
                    rest.trim()
                };
                current_filename = filename.to_string();
            }
        } else if line.trim() == "// === SKELETON MODE (COMPRESSED) ===" {
            // Skip skeleton mode marker
            continue;
        } else if !current_filename.is_empty() {
            // Collect file content
            current_lines.push(line.to_string());
        }
    }

    // Don't forget the last file
    if !current_filename.is_empty() {
        hunks.push(PatchHunk {
            filename: current_filename,
            search: current_lines,
            replace: Vec::new(),
        });
    }

    hunks
}

fn parse_aider_patches(content: &str) -> Vec<PatchHunk> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<PatchHunk> = None;
    let mut state = 0;
    let mut current_filename = String::new();
    for line in content.lines() {
        if line.starts_with("<<<<<<< SEARCH") {
            current_hunk = Some(PatchHunk {
                filename: current_filename.clone(),
                search: Vec::new(),
                replace: Vec::new(),
            });
            state = 1;
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
                if trimmed.starts_with("// === SKELETON MODE")
                    || trimmed.starts_with("//--+ file:///")
                {
                    continue;
                }
                let mut found_fn = None;
                if let Some(start_idx) = trimmed.find('`') {
                    if let Some(end_idx) = trimmed[start_idx + 1..].find('`') {
                        let potential = trimmed[start_idx + 1..start_idx + 1 + end_idx].trim();
                        if !potential.is_empty()
                            && (potential.contains('/')
                                || potential.contains('.')
                                || potential.ends_with(".rs"))
                        {
                            found_fn = Some(potential.to_string());
                        }
                    }
                }
                if found_fn.is_none() {
                    if let Some(rest) = trimmed.strip_prefix("// ") {
                        if !rest.is_empty()
                            && (rest.contains('/') || rest.contains('.') || rest.ends_with(".rs"))
                        {
                            found_fn = Some(rest.trim().trim_matches('`').to_string());
                        }
                    } else if let Some(rest) = trimmed.strip_prefix("# ") {
                        if !rest.is_empty()
                            && (rest.contains('/') || rest.contains('.') || rest.ends_with(".rs"))
                        {
                            found_fn = Some(rest.trim().trim_matches('`').to_string());
                        }
                    } else if let Some(rest) = trimmed.strip_prefix("filename ") {
                        found_fn = Some(rest.trim().trim_matches('`').to_string());
                    } else if let Some(rest) = trimmed.strip_prefix("filename:") {
                        found_fn = Some(rest.trim().trim_matches('`').to_string());
                    } else if let Some(rest) = trimmed.strip_prefix("file:") {
                        found_fn = Some(rest.trim().trim_matches('`').to_string());
                    } else if let Some(rest) = trimmed.strip_prefix("+++ b/") {
                        found_fn = Some(rest.trim().trim_matches('`').to_string());
                    } else if let Some(rest) = trimmed.strip_prefix("+++ ") {
                        found_fn = Some(rest.trim().trim_matches('`').to_string());
                    } else if !trimmed.is_empty()
                        && (trimmed.contains('/')
                            || trimmed.ends_with(".rs")
                            || trimmed.ends_with(".toml")
                            || trimmed.ends_with(".md"))
                    {
                        found_fn = Some(trimmed.trim_matches('`').to_string());
                    }
                }
                if let Some(fname) = found_fn {
                    current_filename = fname;
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
    let mut state = 0;
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
