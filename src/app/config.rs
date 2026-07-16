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
    pub short_search_display: bool,
    #[serde(default)]
    pub disable_llm: bool,
    #[serde(default)]
    pub llm_config: LlmConfig,
    #[serde(default = "default_rustconcat_api_url")]
    pub rustconcat_api_url: String,
    #[serde(default)]
    pub impl_tools: ImplToolsConfig,
    #[serde(default)]
    pub debug_impl_llm: bool,
    #[serde(default)]
    pub project_dirs: Vec<String>,
    #[serde(default)]
    pub active_project_dir: Option<String>,
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
            short_search_display: false,
            disable_llm: false,
            llm_config: LlmConfig::default(),
            rustconcat_api_url: default_rustconcat_api_url(),
            impl_tools: ImplToolsConfig::default(),
            debug_impl_llm: false,
            project_dirs: Vec::new(),
            active_project_dir: None,
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
                if let Ok(mut config) = serde_json::from_str::<AppConfig>(&content) {
                    if config.llm_config.models.is_empty() {
                        #[derive(Deserialize)]
                        struct OldLlmProvider {
                            api_type: String,
                            base_url: String,
                            model: String,
                            api_key: Option<String>,
                            num_ctx: u64,
                            timeout_secs: u64,
                        }
                        #[derive(Deserialize)]
                        struct OldLlmConfig {
                            chat_provider: OldLlmProvider,
                            commit_provider: OldLlmProvider,
                            impl_provider: OldLlmProvider,
                            chat_system_prompt: String,
                            commit_system_prompt: String,
                            impl_system_prompt: String,
                        }
                        #[derive(Deserialize)]
                        struct OldAppConfig {
                            llm_config: OldLlmConfig,
                        }
                        if let Ok(old) = serde_json::from_str::<OldAppConfig>(&content) {
                            let to_new = |p: OldLlmProvider| super::llm::LlmProvider {
                                api_type: p.api_type,
                                base_url: p.base_url,
                                model: p.model,
                                api_key: p.api_key,
                                num_ctx: p.num_ctx,
                                timeout_secs: p.timeout_secs,
                            };
                            config.llm_config.models = vec![
                                to_new(old.llm_config.chat_provider),
                                to_new(old.llm_config.commit_provider),
                                to_new(old.llm_config.impl_provider),
                            ];
                            config.llm_config.chat_model_idx = 0;
                            config.llm_config.commit_model_idx = 1;
                            config.llm_config.impl_model_idx = 2;
                            config.llm_config.chat_system_prompt = old.llm_config.chat_system_prompt;
                            config.llm_config.commit_system_prompt = old.llm_config.commit_system_prompt;
                            config.llm_config.impl_system_prompt = old.llm_config.impl_system_prompt;
                        } else {
                            config.llm_config = super::llm::LlmConfig::default();
                        }
                    }
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
