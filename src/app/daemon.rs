use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
pub struct RepoInfo {
    pub id: String,
    pub source_path: String,
    pub git_branch: Option<String>,
    pub file_count: Option<u64>,
    pub last_sync: Option<u64>,
}

pub fn base_url() -> String {
    "http://127.0.0.1:7890".to_string()
}

pub fn active_repo_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".concat_rust");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("active")
}

pub fn get_active_repo() -> Option<String> {
    std::fs::read_to_string(active_repo_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "none")
}

pub fn set_active_repo(repo: &str) {
    let _ = std::fs::write(active_repo_path(), repo);
}

pub fn clear_active_repo() {
    let _ = std::fs::write(active_repo_path(), "none");
}

pub fn fetch_repos() -> Result<Vec<RepoInfo>, String> {
    let url = format!("{}/repos", base_url());
    let resp = reqwest::blocking::get(&url).map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Vec<RepoInfo>>().map_err(|e| e.to_string())
}

pub fn sync_repo(id: &str) -> Result<String, String> {
    let url = format!("{}/sync/{}", base_url(), id);
    let client = reqwest::blocking::Client::new();
    let resp = client.post(&url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().map_err(|e| e.to_string())
}
