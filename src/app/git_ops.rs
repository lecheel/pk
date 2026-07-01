//--+ file:///src/app/git_ops.rs
use crate::diff::{diff_patch, RowKind};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GitStatus {
    Unchanged,
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Debug)]
pub struct GitDiffHunk {
    pub rows: Vec<crate::diff::DiffRow>,
    pub current_line_range: std::ops::Range<usize>,
}

#[derive(Clone, Debug)]
pub struct GitLogEntry {
    pub hash: String,
    pub author: String,
    pub message: String,
}

pub fn get_git_log(base_dir: &std::path::Path) -> Vec<GitLogEntry> {
    let repo = match git2::Repository::discover(base_dir) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut walk = match repo.revwalk() {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };
    if walk.push_head().is_err() {
        return Vec::new();
    }
    
    let mut log = Vec::new();
    for oid in walk {
        if let Ok(oid) = oid {
            if let Ok(commit) = repo.find_commit(oid) {
                let hash = commit.id().to_string()[0..7].to_string();
                let author = commit.author();
                let name = author.name().unwrap_or("Unknown").to_string();
                let message = commit.message().unwrap_or("").lines().next().unwrap_or("").to_string();
                log.push(GitLogEntry { hash, author: name, message });
            }
        }
    }
    log
}

pub fn group_git_hunks(
    diff_rows: &[crate::diff::DiffRow],
    total_current_lines: usize,
) -> Vec<GitDiffHunk> {
    let mut hunks = Vec::new();
    let mut i = 0;
    while i < diff_rows.len() {
        if diff_rows[i].kind != RowKind::Equal {
            let start_idx = i;
            while i < diff_rows.len() && diff_rows[i].kind != RowKind::Equal {
                i += 1;
            }
            let end_idx = i;
            let hunk_rows = diff_rows[start_idx..end_idx].to_vec();

            let mut min_line = None;
            let mut max_line = None;
            for r in &hunk_rows {
                if let Some(rn) = r.right_num {
                    let zero_idx = rn - 1;
                    if min_line.is_none() || zero_idx < min_line.unwrap() {
                        min_line = Some(zero_idx);
                    }
                    if max_line.is_none() || zero_idx > max_line.unwrap() {
                        max_line = Some(zero_idx);
                    }
                }
            }

            let range = match (min_line, max_line) {
                (Some(min_l), Some(max_l)) => min_l..max_l + 1,
                _ => {
                    let mut assoc_line = total_current_lines.saturating_sub(1);
                    for r in &diff_rows[end_idx..] {
                        if let Some(rn) = r.right_num {
                            assoc_line = rn - 1;
                            break;
                        }
                    }
                    assoc_line..assoc_line + 1
                }
            };

            hunks.push(GitDiffHunk {
                rows: hunk_rows,
                current_line_range: range,
            });
        } else {
            i += 1;
        }
    }
    hunks
}

pub fn get_line_statuses(
    base_dir: &Path,
    file_path: &Path,
    current_lines: &[String],
) -> (Vec<GitStatus>, Vec<crate::diff::DiffRow>) {
    let mut statuses = vec![GitStatus::Unchanged; current_lines.len()];
    let repo = match git2::Repository::discover(base_dir) {
        Ok(r) => r,
        Err(_) => return (statuses, Vec::new()),
    };
    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return (statuses, Vec::new()),
    };
    let abs_file_path = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(file_path)
    } else {
        file_path.to_path_buf()
    };
    let abs_workdir = if workdir.is_absolute() {
        workdir.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(workdir)
    } else {
        workdir.to_path_buf()
    };
    let clean_path = |p: &Path| -> String {
        let s = p.to_string_lossy().replace('\\', "/");
        if let Some(stripped) = s.strip_prefix("//?/") {
            stripped.to_string()
        } else {
            s
        }
    };
    let clean_file = clean_path(&abs_file_path);
    let clean_work = clean_path(&abs_workdir);
    let rel_path_str = if clean_file.starts_with(&clean_work) {
        clean_file[clean_work.len()..]
            .trim_start_matches('/')
            .to_string()
    } else {
        match file_path.strip_prefix(workdir) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => return (statuses, Vec::new()),
        }
    };
    let rel_path = Path::new(&rel_path_str);
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => {
            return (vec![GitStatus::Added; current_lines.len()], Vec::new());
        }
    };
    let tree = match head.peel_to_tree() {
        Ok(t) => t,
        Err(_) => return (statuses, Vec::new()),
    };
    let entry = match tree.get_path(rel_path) {
        Ok(e) => e,
        Err(_) => {
            return (vec![GitStatus::Added; current_lines.len()], Vec::new());
        }
    };
    let blob = match repo.find_blob(entry.id()) {
        Ok(b) => b,
        Err(_) => return (statuses, Vec::new()),
    };
    let head_content = String::from_utf8_lossy(blob.content());
    let head_lines: Vec<String> = head_content.lines().map(String::from).collect();
    let diff = diff_patch(&head_lines, current_lines);
    let diff_rows = crate::diff::build_rows(&diff, 1, 1);

    let mut cur_idx = 0;
    let mut pending_deletes = 0;
    for (kind, _left, _right) in &diff {
        match kind {
            RowKind::Equal => {
                if pending_deletes > 0 {
                    if cur_idx < statuses.len() {
                        statuses[cur_idx] = GitStatus::Deleted;
                    }
                    pending_deletes = 0;
                }
                cur_idx += 1;
            }
            RowKind::Delete => {
                pending_deletes += 1;
            }
            RowKind::Insert => {
                if cur_idx < statuses.len() {
                    if pending_deletes > 0 {
                        statuses[cur_idx] = GitStatus::Modified;
                        pending_deletes -= 1;
                    } else {
                        statuses[cur_idx] = GitStatus::Added;
                    }
                }
                cur_idx += 1;
            }
        }
    }
    if pending_deletes > 0 && !statuses.is_empty() {
        let last_idx = statuses.len() - 1;
        statuses[last_idx] = GitStatus::Deleted;
    }
    (statuses, diff_rows)
}