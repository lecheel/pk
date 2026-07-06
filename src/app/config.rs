use super::llm::LlmConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub format_on_save: bool,
    pub fmt_command: String,
    pub active_repo_id: Option<String>,
    pub concat_server_enabled: bool,
    #[serde(default)]
    pub ignore_comments: bool,
    #[serde(default = "default_min_match_score")]
    pub min_match_score: f32,
    #[serde(default = "default_min_match_floor")]
    pub min_match_floor: f32,
    #[serde(default)]
    pub llm_config: LlmConfig,
    #[serde(default = "default_rustconcat_api_url")]
    pub rustconcat_api_url: String,
    #[serde(default)]
    pub impl_tools: ImplToolsConfig,
}
fn default_min_match_score() -> f32 {
    60.0
}
fn default_min_match_floor() -> f32 {
    15.0
}
fn default_rustconcat_api_url() -> String {
    "http://127.0.0.1:7890".to_string()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplToolsConfig {
    pub skeleton: bool,
    pub files: bool,
    pub hashes: bool,
}
impl Default for ImplToolsConfig {
    fn default() -> Self {
        Self {
            skeleton: true,
            files: false,
            hashes: false,
        }
    }
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            format_on_save: true,
            fmt_command: "rustfmt".to_string(),
            active_repo_id: None,
            concat_server_enabled: true,
            ignore_comments: false,
            min_match_score: default_min_match_score(),
            min_match_floor: default_min_match_floor(),
            llm_config: LlmConfig::default(),
            rustconcat_api_url: default_rustconcat_api_url(),
            impl_tools: ImplToolsConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let dir = PathBuf::from(appdata).join("pk");
                if let Err(e) = fs::create_dir_all(&dir) {
                    eprintln!("[Config] Failed to create config dir {:?}: {}", dir, e);
                }
                return Some(dir.join("config.json"));
            }
            None
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(home) = std::env::var("HOME") {
                let dir = PathBuf::from(home).join(".config/pk");
                if let Err(e) = fs::create_dir_all(&dir) {
                    eprintln!("[Config] Failed to create config dir {:?}: {}", dir, e);
                }
                return Some(dir.join("config.json"));
            }
            None
        }
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(&path, json);
            }
        }
    }
}