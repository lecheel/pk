//--+ file:///src/app/git_ops.rs
use crate::diff::{diff_patch, RowKind};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

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
pub struct GitLogFileChange {
    pub path: String,
    pub status: char,
    pub additions: usize,
    pub deletions: usize,
    pub patch: String,
}

#[derive(Clone, Debug)]
pub struct GitLogEntry {
    pub hash: String,
    pub full_hash: String,
    pub author: String,
    pub email: String,
    pub time: String,
    pub message: String,
    pub body: String,
    pub files_changed: Vec<GitLogFileChange>,
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
                let full_hash = commit.id().to_string();
                let hash = full_hash[0..7].to_string();
                let author = commit.author();
                let name = author.name().unwrap_or("Unknown").to_string();
                let email = author.email().unwrap_or("").to_string();
                let time = author.when().seconds().to_string();
                let message = commit.summary().unwrap_or("").to_string();
                let body = commit.message().unwrap_or("").to_string();

                let files_changed_rc = Rc::new(RefCell::new(Vec::new()));
                if let Ok(tree) = commit.tree() {
                    let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());
                    if let Ok(diff) =
                        repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
                    {
                        let fc_clone1 = files_changed_rc.clone();
                        let fc_clone2 = files_changed_rc.clone();
                        let fc_clone3 = files_changed_rc.clone();
                        let _ = diff.foreach(
                            &mut move |d, _| {
                                let path = d.new_file().path().or_else(|| d.old_file().path());
                                if let Some(p) = path {
                                    let status = match d.status() {
                                        git2::Delta::Added => 'A',
                                        git2::Delta::Deleted => 'D',
                                        git2::Delta::Modified => 'M',
                                        _ => 'M',
                                    };
                                    fc_clone1.borrow_mut().push(GitLogFileChange {
                                        path: p.display().to_string(),
                                        status,
                                        additions: 0,
                                        deletions: 0,
                                        patch: String::new(),
                                    });
                                }
                                true
                            },
                            None,
                            Some(&mut move |_, h| {
                                let mut fc_ref = fc_clone2.borrow_mut();
                                if let Some(fc) = fc_ref.last_mut() {
                                    fc.patch.push_str(&format!(
                                        "@@ -{},{} +{},{} @@\n",
                                        h.old_start(),
                                        h.old_lines(),
                                        h.new_start(),
                                        h.new_lines()
                                    ));
                                }
                                true
                            }),
                            Some(&mut move |_, _, line| {
                                let mut fc_ref = fc_clone3.borrow_mut();
                                if let Some(fc) = fc_ref.last_mut() {
                                    let origin = line.origin();
                                    if origin == '+' {
                                        fc.additions += 1;
                                    } else if origin == '-' {
                                        fc.deletions += 1;
                                    }
                                    let content = String::from_utf8_lossy(line.content());
                                    fc.patch.push_str(&format!("{}{}", origin, content));
                                }
                                true
                            }),
                        );
                    }
                }
                let files_changed = files_changed_rc.borrow().clone();
                log.push(GitLogEntry {
                    hash,
                    full_hash,
                    author: name,
                    email,
                    time,
                    message,
                    body,
                    files_changed,
                });
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
    ignore_comments: bool,
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
    let diff = diff_patch(&head_lines, current_lines, ignore_comments);
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