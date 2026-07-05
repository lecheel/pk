// src/app/llm.rs
use serde::{Deserialize, Serialize};
use std::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LlmProvider {
    OpenAi {
        api_key: String,
        model: String,
        base_url: Option<String>,
    },
    Anthropic {
        api_key: String,
        model: String,
    },
    Ollama {
        base_url: String,
        model: String,
    },
}

impl Default for LlmProvider {
    fn default() -> Self {
        LlmProvider::Ollama {
            base_url: "http://localhost:11434".to_string(),
            model: "llama3".to_string(),
        }
    }
}

impl LlmProvider {
    pub fn name(&self) -> &str {
        match self {
            LlmProvider::OpenAi { .. } => "OpenAI",
            LlmProvider::Anthropic { .. } => "Anthropic",
            LlmProvider::Ollama { .. } => "Ollama",
        }
    }

    pub fn model(&self) -> &str {
        match self {
            LlmProvider::OpenAi { model, .. } => model,
            LlmProvider::Anthropic { model, .. } => model,
            LlmProvider::Ollama { model, .. } => model,
        }
    }

    pub fn set_model(&mut self, model: &str) {
        match self {
            LlmProvider::OpenAi { model: m, .. } => *m = model.to_string(),
            LlmProvider::Anthropic { model: m, .. } => *m = model.to_string(),
            LlmProvider::Ollama { model: m, .. } => *m = model.to_string(),
        }
    }

    pub fn variant_index(&self) -> usize {
        match self {
            LlmProvider::OpenAi { .. } => 0,
            LlmProvider::Anthropic { .. } => 1,
            LlmProvider::Ollama { .. } => 2,
        }
    }

    pub fn set_variant(&mut self, idx: usize) {
        let model = self.model().to_string();
        *self = match idx {
            0 => LlmProvider::OpenAi {
                api_key: String::new(),
                model,
                base_url: None,
            },
            1 => LlmProvider::Anthropic {
                api_key: String::new(),
                model,
            },
            _ => LlmProvider::Ollama {
                base_url: "http://localhost:11434".to_string(),
                model,
            },
        };
    }

    pub fn api_key_mut(&mut self) -> Option<&mut String> {
        match self {
            LlmProvider::OpenAi { api_key, .. } => Some(api_key),
            LlmProvider::Anthropic { api_key, .. } => Some(api_key),
            LlmProvider::Ollama { .. } => None,
        }
    }

    pub fn base_url_mut(&mut self) -> Option<&mut String> {
        match self {
            LlmProvider::OpenAi { base_url, .. } => base_url.as_mut(),
            LlmProvider::Ollama { base_url, .. } => Some(base_url),
            LlmProvider::Anthropic { .. } => None,
        }
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
        let result = match &provider {
            LlmProvider::OpenAi {
                api_key,
                model,
                base_url,
            } => call_openai(api_key, model, base_url.as_deref(), messages, system_prompt),
            LlmProvider::Anthropic { api_key, model } => {
                call_anthropic(api_key, model, messages, system_prompt)
            }
            LlmProvider::Ollama { base_url, model } => {
                call_ollama(base_url, model, messages, system_prompt)
            }
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
    api_key: &str,
    model: &str,
    base_url: Option<&str>,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("OpenAI API key not set".to_string());
    }
    let url = base_url
        .unwrap_or("https://api.openai.com/v1")
        .trim_end_matches('/')
        .to_string()
        + "/chat/completions";

    let mut all_messages = Vec::new();
    if let Some(sys) = system_prompt {
        all_messages.push(ChatMessage {
            role: "system".to_string(),
            content: sys,
        });
    }
    all_messages.extend(messages);

    let body = serde_json::json!({
        "model": model,
        "messages": all_messages,
        "max_tokens": 4096,
    });

    send_openai_compatible_request(&url, &format!("Bearer {}", api_key), &body)
}

fn call_anthropic(
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("Anthropic API key not set".to_string());
    }
    let url = "https://api.anthropic.com/v1/messages".to_string();

    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_tokens": 4096,
    });

    if let Some(sys) = system_prompt {
        body["system"] = serde_json::json!(sys);
    }

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to create HTTP client: {}", e)),
    };

    let resp = match client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
    {
        Ok(r) => r,
        Err(e) => return Err(format!("Request failed: {}", e)),
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("Anthropic API error {}: {}", status, text));
    }

    let json: serde_json::Value = match resp.json() {
        Ok(j) => j,
        Err(e) => return Err(format!("Failed to parse response: {}", e)),
    };

    json["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|block| block["text"].as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No content in response".to_string())
}

fn call_ollama(
    base_url: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
) -> Result<String, String> {
    let url = base_url.trim_end_matches('/').to_string() + "/api/chat";

    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
    });

    if let Some(sys) = system_prompt {
        body["system"] = serde_json::json!(sys);
    }

    send_openai_compatible_request(&url, "", &body)
}

fn send_openai_compatible_request(
    url: &str,
    auth_header: &str,
    body: &serde_json::Value,
) -> Result<String, String> {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to create HTTP client: {}", e)),
    };

    let mut req = client.post(url).header("content-type", "application/json");

    if !auth_header.is_empty() {
        req = req.header("authorization", auth_header);
    }

    let resp = match req.json(body).send() {
        Ok(r) => r,
        Err(e) => return Err(format!("Request failed: {}", e)),
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("API error {}: {}", status, text));
    }

    let json: serde_json::Value = match resp.json() {
        Ok(j) => j,
        Err(e) => return Err(format!("Failed to parse response: {}", e)),
    };

    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| json["message"]["content"].as_str().map(|s| s.to_string()))
        .ok_or_else(|| "No content in response".to_string())
}
