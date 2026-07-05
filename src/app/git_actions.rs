use super::state::MergeApp;
use super::types::StatusMessage;

impl MergeApp {
    pub fn toggle_stage_file(&mut self, path: &str) {
        let repo = match git2::Repository::discover(&self.base_dir) {
            Ok(r) => r,
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Git error: {}", e)));
                return;
            }
        };
        let mut index = match repo.index() {
            Ok(i) => i,
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Index error: {}", e)));
                return;
            }
        };

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        opts.pathspec(path);

        let status = {
            let statuses = match repo.statuses(Some(&mut opts)) {
                Ok(s) => s,
                Err(_) => return,
            };
            statuses.get(0).map(|entry| entry.status())
        };

        if let Some(s) = status {
            if s.is_index_new() || s.is_index_modified() || s.is_index_deleted() {
                match repo.head().and_then(|head| head.peel_to_commit()) {
                    Ok(commit) => {
                        match repo
                            .reset_default(Some(commit.as_object()), &[std::path::Path::new(path)])
                        {
                            Ok(_) => {
                                self.set_message(StatusMessage::info(format!("Unstaged {}", path)))
                            }
                            Err(e) => self.set_message(StatusMessage::error(format!(
                                "Unstage failed: {}",
                                e
                            ))),
                        }
                    }
                    Err(e) => {
                        if let Ok(mut index) = repo.index() {
                            match index.remove_path(std::path::Path::new(path)) {
                                Ok(_) => {
                                    let _ = index.write();
                                    self.set_message(StatusMessage::info(format!(
                                        "Unstaged {}",
                                        path
                                    )));
                                }
                                Err(e2) => self.set_message(StatusMessage::error(format!(
                                    "Unstage failed: {}",
                                    e2
                                ))),
                            }
                        } else {
                            self.set_message(StatusMessage::error(format!(
                                "Unstage failed: {}",
                                e
                            )));
                        }
                    }
                }
            } else {
                let res = if s.is_wt_deleted() || s.is_index_deleted() {
                    index.remove_path(std::path::Path::new(path))
                } else {
                    index.add_path(std::path::Path::new(path))
                };
                match res {
                    Ok(_) => {
                        let _ = index.write();
                        self.set_message(StatusMessage::info(format!("Staged {}", path)))
                    }
                    Err(e) => {
                        self.set_message(StatusMessage::error(format!("Stage failed: {}", e)))
                    }
                }
            }
        }
    }

    pub fn commit_changes(&mut self) {
        if self.commit_message.trim().is_empty() {
            self.set_message(StatusMessage::warning("Commit message cannot be empty"));
            return;
        }
        let repo = match git2::Repository::discover(&self.base_dir) {
            Ok(r) => r,
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Git error: {}", e)));
                return;
            }
        };
        let mut index = match repo.index() {
            Ok(i) => i,
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Index error: {}", e)));
                return;
            }
        };
        let sig = match repo.signature() {
            Ok(s) => s,
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Signature error: {}", e)));
                return;
            }
        };
        let tree_id = match index.write_tree() {
            Ok(id) => id,
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Write tree failed: {}", e)));
                return;
            }
        };
        let tree = match repo.find_tree(tree_id) {
            Ok(t) => t,
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Find tree failed: {}", e)));
                return;
            }
        };
        let head = repo.head().ok();
        let parents = if let Some(ref h) = head {
            vec![repo.find_commit(h.target().unwrap()).unwrap()]
        } else {
            vec![]
        };
        let parents_ref: Vec<&git2::Commit> = parents.iter().collect();
        match repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &self.commit_message,
            &tree,
            &parents_ref,
        ) {
            Ok(oid) => {
                self.set_message(StatusMessage::success(format!(
                    "Commit successful: {}",
                    oid
                )));
                self.commit_message.clear();
                self.show_commit_prompt = false;
                self.git_log_entries =
                    super::git_ops::get_git_log(std::path::Path::new(&self.base_dir));
            }
            Err(e) => self.set_message(StatusMessage::error(format!("Commit failed: {}", e))),
        }
    }

    pub fn stash_changes(&mut self) {
        let mut repo = match git2::Repository::discover(&self.base_dir) {
            Ok(r) => r,
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Git error: {}", e)));
                return;
            }
        };
        let sig = match repo.signature() {
            Ok(s) => s,
            Err(e) => {
                self.set_message(StatusMessage::error(format!("Signature error: {}", e)));
                return;
            }
        };
        match repo.stash_save(&sig, "PCodeMerge WIP", None) {
            Ok(_) => self.set_message(StatusMessage::success("Stashed changes")),
            Err(e) => {
                if e.code() == git2::ErrorCode::NotFound {
                    self.set_message(StatusMessage::info("No local changes to stash"));
                } else {
                    self.set_message(StatusMessage::error(format!("Stash failed: {}", e)));
                }
            }
        }
    }
}