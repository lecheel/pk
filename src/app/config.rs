use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub format_on_save: bool,
    pub fmt_command: String,
    pub active_repo_id: Option<String>,
    pub concat_server_enabled: bool,
    #[serde(default)]
    pub ignore_comments: bool,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            format_on_save: true,
            fmt_command: "rustfmt".to_string(),
            active_repo_id: None,
            concat_server_enabled: true,
            ignore_comments: false,
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
                    println!("[DEBUG config] Loaded config: ignore_comments={}", config.ignore_comments);
                    return config;
                }
            }
        }
        println!("[DEBUG config] Using default config: ignore_comments=false");
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