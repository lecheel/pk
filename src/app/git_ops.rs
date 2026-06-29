use crate::diff::{diff_patch, RowKind};
use std::path::Path;

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

    let rel_path = match file_path.strip_prefix(workdir) {
        Ok(p) => p,
        Err(_) => return statuses,
    };

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => {
            // No commits yet (unborn HEAD), everything is Added
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
            // File is new/untracked in Git
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
