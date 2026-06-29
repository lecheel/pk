//--+ file:///src/app/git_ops.rs
// Hash: [Your calculated hash or let it be generated]
use crate::diff::{diff_patch, RowKind};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GitStatus {
    Unchanged,
    Added,
    Modified,
    Deleted,
}

pub fn get_line_statuses(
    base_dir: &Path,
    file_path: &Path,
    current_lines: &[String],
) -> Vec<GitStatus> {
    let mut statuses = vec![GitStatus::Unchanged; current_lines.len()];
    let repo = match git2::Repository::discover(base_dir) {
        Ok(r) => r,
        Err(_) => return statuses,
    };
    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return statuses,
    };

    // Ensure both paths are absolute
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

    // Normalize separators and strip potential Windows verbatim/UNC prefixes
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

    // Extract the relative path using cleaned path strings
    let rel_path_str = if clean_file.starts_with(&clean_work) {
        clean_file[clean_work.len()..]
            .trim_start_matches('/')
            .to_string()
    } else {
        match file_path.strip_prefix(workdir) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => return statuses,
        }
    };

    let rel_path = Path::new(&rel_path_str);

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => {
            return vec![GitStatus::Added; current_lines.len()];
        }
    };
    let tree = match head.peel_to_tree() {
        Ok(t) => t,
        Err(_) => return statuses,
    };
    let entry = match tree.get_path(rel_path) {
        Ok(e) => e,
        Err(_) => {
            return vec![GitStatus::Added; current_lines.len()];
        }
    };
    let blob = match repo.find_blob(entry.id()) {
        Ok(b) => b,
        Err(_) => return statuses,
    };
    let head_content = String::from_utf8_lossy(blob.content());
    let head_lines: Vec<String> = head_content.lines().map(String::from).collect();
    let diff = diff_patch(&head_lines, current_lines);
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
    statuses
}
