use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::mpsc;
use std::time::Duration;

fn default_api_type() -> String {
    "ollama".to_string()
}
fn default_num_ctx() -> u64 {
    4096
}
fn default_timeout() -> u64 {
    120
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmProvider {
    #[serde(default = "default_api_type")]
    pub api_type: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_num_ctx")]
    pub num_ctx: u64,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

impl Default for LlmProvider {
    fn default() -> Self {
        Self {
            api_type: "ollama".to_string(),
            base_url: "http://localhost:8080".to_string(),
            model: "default".to_string(),
            api_key: None,
            num_ctx: 4096,
            timeout_secs: 120,
        }
    }
}

impl LlmProvider {
    pub fn preset_ollama() -> Self {
        Self {
            api_type: "ollama".to_string(),
            base_url: "http://localhost:8080".to_string(),
            model: "default".to_string(),
            api_key: None,
            num_ctx: 4096,
            timeout_secs: 120,
        }
    }

    pub fn preset_openai() -> Self {
        Self {
            api_type: "openai".to_string(),
            base_url: "https://api.openai.com".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: None,
            num_ctx: 4096,
            timeout_secs: 120,
        }
    }

    pub fn preset_anthropic() -> Self {
        Self {
            api_type: "anthropic".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: None,
            num_ctx: 4096,
            timeout_secs: 120,
        }
    }

    pub fn name(&self) -> &str {
        match self.api_type.as_str() {
            "openai" => "OpenAI",
            "anthropic" => "Anthropic",
            _ => "Ollama",
        }
    }

    pub fn variant_index(&self) -> usize {
        match self.api_type.as_str() {
            "openai" => 0,
            "anthropic" => 1,
            _ => 2,
        }
    }

    pub fn set_variant(&mut self, idx: usize) {
        let model = self.model.clone();
        let api_key = self.api_key.clone();
        *self = match idx {
            0 => {
                let mut p = Self::preset_openai();
                p.model = model;
                p.api_key = api_key;
                p
            }
            1 => {
                let mut p = Self::preset_anthropic();
                p.model = model;
                p.api_key = api_key;
                p
            }
            _ => {
                let mut p = Self::preset_ollama();
                p.model = model;
                p
            }
        };
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub chat_provider: LlmProvider,
    #[serde(default)]
    pub commit_provider: LlmProvider,
    #[serde(default)]
    pub impl_provider: LlmProvider,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            chat_provider: LlmProvider::default(),
            commit_provider: LlmProvider::default(),
            impl_provider: LlmProvider::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum LlmResponse {
    Text(String),
    Error(String),
    Done,
}

pub fn send_to_llm(
    provider: LlmProvider,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
) -> mpsc::Receiver<LlmResponse> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = match provider.api_type.as_str() {
            "anthropic" => call_anthropic(&provider, messages, system_prompt),
            "openai" => call_openai(&provider, messages, system_prompt),
            _ => call_ollama(&provider, messages, system_prompt),
        };

        match result {
            Ok(text) => {
                let _ = tx.send(LlmResponse::Text(text));
                let _ = tx.send(LlmResponse::Done);
            }
            Err(e) => {
                let _ = tx.send(LlmResponse::Error(e));
            }
        }
    });
    rx
}

fn call_openai(
    provider: &LlmProvider,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
) -> Result<String, String> {
    let base = provider.base_url.trim_end_matches('/');
    let url = if base.ends_with("/v1") {
        format!("{}/chat/completions", base)
    } else {
        format!("{}/v1/chat/completions", base)
    };

    let mut json_messages = Vec::new();
    if let Some(sys) = system_prompt {
        json_messages.push(json!({"role": "system", "content": sys}));
    }
    for msg in &messages {
        json_messages.push(json!({"role": msg.role, "content": msg.content}));
    }

    let mut body = json!({
        "model": provider.model,
        "messages": json_messages,
        "stream": false,
    });
    body["options"] = json!({ "num_ctx": provider.num_ctx });

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let mut req = client.post(&url).json(&body);
    if let Some(ref key) = provider.api_key {
        req = req.bearer_auth(key);
    }

    let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, text));
    }

    let data: serde_json::Value = resp.json().map_err(|e| format!("Failed to parse response: {}", e))?;
    data["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No content in response".to_string())
}

fn call_anthropic(
    provider: &LlmProvider,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
) -> Result<String, String> {
    let base = provider.base_url.trim_end_matches('/');
    let url = if base.ends_with("/v1") {
        format!("{}/messages", base)
    } else {
        format!("{}/v1/messages", base)
    };

    let mut json_messages = Vec::new();
    for msg in &messages {
        json_messages.push(json!({"role": msg.role, "content": msg.content}));
    }

    let mut body = json!({
        "model": provider.model,
        "max_tokens": 4096,
        "messages": json_messages,
        "stream": false
    });

    if let Some(sys) = system_prompt {
        body["system"] = json!(sys);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let mut req = client.post(&url).json(&body);
    if let Some(ref key) = provider.api_key {
        req = req
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01");
    }

    let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("Anthropic API error {}: {}", status, text));
    }

    let data: serde_json::Value = resp.json().map_err(|e| format!("Failed to parse response: {}", e))?;
    
    data["content"]
        .as_array()
        .and_then(|arr| arr.iter().find(|b| b["type"].as_str() == Some("text")))
        .and_then(|b| b["text"].as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No content in response".to_string())
}

fn call_ollama(
    provider: &LlmProvider,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
) -> Result<String, String> {
    // llama.cpp uses OpenAI-compatible endpoint at /v1/chat/completions
    let base = provider.base_url.trim_end_matches('/');
    let url = if base.ends_with("/v1") {
        format!("{}/chat/completions", base)
    } else {
        format!("{}/v1/chat/completions", base)
    };

    let mut json_messages = Vec::new();
    if let Some(sys) = system_prompt {
        json_messages.push(json!({"role": "system", "content": sys}));
    }
    for msg in &messages {
        json_messages.push(json!({"role": msg.role, "content": msg.content}));
    }

    let mut body = json!({
        "model": provider.model,
        "messages": json_messages,
        "stream": false,
    });
    body["options"] = json!({ "num_ctx": provider.num_ctx });

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp = client.post(&url).json(&body).send().map_err(|e| format!("Request failed: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("Ollama/llama.cpp API error {}: {}", status, text));
    }

    let data: serde_json::Value = resp.json().map_err(|e| format!("Failed to parse response: {}", e))?;
    data["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No content in response".to_string())
}