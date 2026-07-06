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
    360
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
            timeout_secs: 360,
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
            num_ctx: 512,
            timeout_secs: 360,
        }
    }

    pub fn preset_openai() -> Self {
        Self {
            api_type: "openai".to_string(),
            base_url: "https://api.openai.com".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: None,
            num_ctx: 4096,
            timeout_secs: 360,
        }
    }

    pub fn preset_anthropic() -> Self {
        Self {
            api_type: "anthropic".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: None,
            num_ctx: 4096,
            timeout_secs: 360,
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
    pub chat_provider: LlmProvider,
    pub commit_provider: LlmProvider,
    pub impl_provider: LlmProvider,
    #[serde(default)]
    pub chat_system_prompt: String,
    #[serde(default)]
    pub commit_system_prompt: String,
    #[serde(default)]
    pub impl_system_prompt: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            chat_provider: LlmProvider::default(),
            commit_provider: LlmProvider::default(),
            impl_provider: LlmProvider::default(),
            chat_system_prompt: String::new(),
            commit_system_prompt: String::new(),
            impl_system_prompt: String::new(),
        }
    }
}

use super::config::ImplToolsConfig;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}
#[derive(Debug, Clone)]
pub enum LlmResponse {
    Text(String),
    ToolUse {
        name: String,
        arguments: String,
        id: String,
    },
    ToolResult {
        id: String,
        name: String,
        result: String,
    },
    Error(String),
    Done,
}
enum LlmOutput {
    Text(String),
    ToolCall {
        name: String,
        arguments: String,
        id: String,
    },
}
pub fn send_to_llm(
    provider: LlmProvider,
    mut messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
    tools_config: Option<ImplToolsConfig>,
    concat_base_url: String,
    base_dir: String,
    debug: bool,
) -> mpsc::Receiver<LlmResponse> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if debug {
            println!("\n=== [IMPL LLM DEBUG] Starting LLM request ===");
            println!(
                "[IMPL LLM DEBUG] Provider: {} ({})",
                provider.name(),
                provider.model
            );
            println!("[IMPL LLM DEBUG] System Prompt: {:?}", system_prompt);
            println!("[IMPL LLM DEBUG] Tools Config: {:?}", tools_config);
        }
        // Max 5 tool calls to prevent infinite loops
        for i in 0..5 {
            if debug {
                println!("\n[IMPL LLM DEBUG] --- Loop iteration {} ---", i + 1);
                println!(
                    "[IMPL LLM DEBUG] Sending messages to LLM: {}",
                    serde_json::to_string_pretty(&messages).unwrap_or_default()
                );
            }

            let result = match provider.api_type.as_str() {
                "anthropic" => call_anthropic(
                    &provider,
                    &messages,
                    &system_prompt,
                    tools_config.as_ref(),
                    debug,
                ),
                "openai" => call_openai(
                    &provider,
                    &messages,
                    &system_prompt,
                    tools_config.as_ref(),
                    debug,
                ),
                _ => call_ollama(
                    &provider,
                    &messages,
                    &system_prompt,
                    tools_config.as_ref(),
                    debug,
                ),
            };

            if debug {
                match &result {
                    Ok(LlmOutput::Text(t)) => {
                        println!("[IMPL LLM DEBUG] Received Text output: {}", t)
                    }
                    Ok(LlmOutput::ToolCall {
                        name,
                        arguments,
                        id,
                    }) => println!(
                        "[IMPL LLM DEBUG] Received ToolCall: name={}, id={}, args={}",
                        name, id, arguments
                    ),
                    Err(e) => println!("[IMPL LLM DEBUG] Received Error: {}", e),
                }
            }

            match result {
                Ok(LlmOutput::Text(text)) => {
                    let _ = tx.send(LlmResponse::Text(text));
                    let _ = tx.send(LlmResponse::Done);
                    return;
                }
                Ok(LlmOutput::ToolCall {
                    name,
                    arguments,
                    id,
                }) => {
                    let _ = tx.send(LlmResponse::ToolUse {
                        name: name.clone(),
                        arguments: arguments.clone(),
                        id: id.clone(),
                    });
                    let tool_result = execute_tool(&name, &arguments, &concat_base_url, &base_dir);
                    if debug {
                        println!(
                            "[IMPL LLM DEBUG] Tool '{}' executed. Result (first 500 chars): {}",
                            name,
                            &tool_result.chars().take(500).collect::<String>()
                        );
                    }
                    let _ = tx.send(LlmResponse::ToolResult {
                        id: id.clone(),
                        name: name.clone(),
                        result: tool_result.clone(),
                    });
                    // Add assistant message with tool call

                    // Add assistant message with tool call
                    let assistant_msg = ChatMessage {
                        role: "assistant".to_string(),
                        content: String::new(),
                        tool_calls: Some(serde_json::json!([{
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": arguments
                            }
                        }])),
                        tool_call_id: None,
                    };
                    messages.push(assistant_msg);

                    // Add tool response message
                    let tool_msg = ChatMessage {
                        role: "tool".to_string(),
                        content: tool_result,
                        tool_calls: None,
                        tool_call_id: Some(id),
                    };
                    messages.push(tool_msg);
                }
                Err(e) => {
                    let _ = tx.send(LlmResponse::Error(e));
                    return;
                }
            }
        }
        let _ = tx.send(LlmResponse::Error("Max tool calls reached".to_string()));
    });
    rx
}
fn execute_tool(name: &str, arguments: &str, base_url: &str, base_dir: &str) -> String {
    match name {
        "get_skeleton" => {
            let url = format!("{}/skeleton", base_url);
            match reqwest::blocking::get(&url) {
                Ok(resp) => {
                    if resp.status().is_success() {
                        resp.text().unwrap_or_else(|e| format!("Failed to read response: {}", e))
                    } else {
                        format!("HTTP Error: {}", resp.status())
                    }
                }
                Err(e) => format!("Request failed: {}", e),
            }
        }
        "get_files" => {
            let url = format!("{}/files", base_url);
            match reqwest::blocking::get(&url) {
                Ok(resp) => {
                    if resp.status().is_success() {
                        resp.text().unwrap_or_else(|e| format!("Failed to read response: {}", e))
                    } else {
                        format!("HTTP Error: {}", resp.status())
                    }
                }
                Err(e) => format!("Request failed: {}", e),
            }
        }
        "get_hashes" => {
            let url = format!("{}/hashes", base_url);
            match reqwest::blocking::get(&url) {
                Ok(resp) => {
                    if resp.status().is_success() {
                        resp.text().unwrap_or_else(|e| format!("Failed to read response: {}", e))
                    } else {
                        format!("HTTP Error: {}", resp.status())
                    }
                }
                Err(e) => format!("Request failed: {}", e),
            }
        }
        "save_impl_patch" => {
            let parsed: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();
            let patch = parsed["patch"].as_str().unwrap_or_default();
            if patch.is_empty() {
                return "Error: patch argument is empty".to_string();
            }
            let todo_path = std::path::Path::new(base_dir).join("todo.md");
            match std::fs::write(&todo_path, patch) {
                Ok(_) => "Patch successfully saved to todo.md".to_string(),
                Err(e) => format!("Failed to save todo.md: {}", e),
            }
        }
        _ => format!("Unknown tool: {}", name),
    }
}
fn build_tools_json(tools_config: Option<&ImplToolsConfig>) -> Option<serde_json::Value> {
    let config = tools_config?;
    let mut tools = Vec::new();

    if config.skeleton {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "get_skeleton",
                "description": "Get the project skeleton structure",
                "parameters": { "type": "object", "properties": {} }
            }
        }));
    }
    if config.files {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "get_files",
                "description": "Get a list of all files in the project",
                "parameters": { "type": "object", "properties": {} }
            }
        }));
    }
    if config.hashes {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "get_hashes",
                "description": "Get hashes of the project files",
                "parameters": { "type": "object", "properties": {} }
            }
        }));
    }
    
    tools.push(json!({
        "type": "function",
        "function": {
            "name": "save_impl_patch",
            "description": "Save the generated search/replace patch to todo.md in the repository directory",
            "parameters": {
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": "The complete search/replace code blocks to be saved"
                    }
                },
                "required": ["patch"]
            }
        }
    }));
    
    if tools.is_empty() {
        None
    } else {
        Some(json!(tools))
    }
}
/// Anthropic's Messages API uses a flat `{name, description, input_schema}`
/// tool shape instead of OpenAI's `{type: "function", function: {...}}`
/// wrapper, so tool definitions must be converted per-provider.
fn to_anthropic_tools(tools_config: Option<&ImplToolsConfig>) -> Option<serde_json::Value> {
    let openai_tools = build_tools_json(tools_config)?;
    let arr = openai_tools.as_array()?;
    let converted: Vec<serde_json::Value> = arr
        .iter()
        .map(|t| {
            json!({
                "name": t["function"]["name"],
                "description": t["function"]["description"],
                "input_schema": t["function"]["parameters"]
            })
        })
        .collect();
    Some(json!(converted))
}
fn call_openai(
    provider: &LlmProvider,
    messages: &[ChatMessage],
    system_prompt: &Option<String>,
    tools_config: Option<&ImplToolsConfig>,
    debug: bool,
) -> Result<LlmOutput, String> {
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
    for msg in messages {
        let mut m = json!({"role": msg.role, "content": msg.content});
        if let Some(tc) = &msg.tool_calls {
            m["tool_calls"] = tc.clone();
        }
        if let Some(id) = &msg.tool_call_id {
            m["tool_call_id"] = json!(id);
        }
        json_messages.push(m);
    }
    let mut body = json!({
        "model": provider.model,
        "messages": json_messages,
        "stream": false,
    });
    body["options"] = json!({ "num_ctx": provider.num_ctx });

    if let Some(tools) = build_tools_json(tools_config) {
        body["tools"] = tools;
    }

    if debug {
        println!("[IMPL LLM DEBUG] OpenAI Request URL: {}", url);
        println!(
            "[IMPL LLM DEBUG] OpenAI Request Body: {}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
    }

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
        if debug {
            println!("[IMPL LLM DEBUG] OpenAI API Error Response: {}", text);
        }
        return Err(format!("OpenAI API error {}: {}", status, text));
    }
    let text = resp
        .text()
        .map_err(|e| format!("Failed to read response text: {}", e))?;
    if debug {
        println!("[IMPL LLM DEBUG] OpenAI Raw Response: {}", text);
    }
    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Failed to parse response: {}", e))?;

    let msg = &data["choices"][0]["message"];
    if let Some(tool_calls) = msg["tool_calls"].as_array() {
        if !tool_calls.is_empty() {
            let tc = &tool_calls[0];
            return Ok(LlmOutput::ToolCall {
                name: tc["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                arguments: tc["function"]["arguments"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                id: tc["id"].as_str().unwrap_or_default().to_string(),
            });
        }
    }

    msg["content"]
        .as_str()
        .map(|s| LlmOutput::Text(s.to_string()))
        .ok_or_else(|| "No content in response".to_string())
}
fn call_anthropic(
    provider: &LlmProvider,
    messages: &[ChatMessage],
    system_prompt: &Option<String>,
    tools_config: Option<&ImplToolsConfig>,
    debug: bool,
) -> Result<LlmOutput, String> {
    let base = provider.base_url.trim_end_matches('/');
    let url = if base.ends_with("/v1") {
        format!("{}/messages", base)
    } else {
        format!("{}/v1/messages", base)
    };
    let mut json_messages = Vec::new();
    for msg in messages {
        if msg.role == "tool" {
            // Anthropic expects tool results as a user message containing a
            // tool_result content block, not an OpenAI-style tool message.
            json_messages.push(json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": msg.tool_call_id.clone().unwrap_or_default(),
                    "content": msg.content
                }]
            }));
            continue;
        }
        if let Some(tc) = &msg.tool_calls {
            // Anthropic expects tool calls as tool_use content blocks on an
            // assistant message, not an OpenAI-style tool_calls array.
            let mut blocks = Vec::new();
            if !msg.content.is_empty() {
                blocks.push(json!({"type": "text", "text": msg.content}));
            }
            if let Some(arr) = tc.as_array() {
                for call in arr {
                    let args_str = call["function"]["arguments"].as_str().unwrap_or("{}");
                    let input: serde_json::Value =
                        serde_json::from_str(args_str).unwrap_or_else(|_| json!({}));
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call["id"],
                        "name": call["function"]["name"],
                        "input": input
                    }));
                }
            }
            json_messages.push(json!({"role": "assistant", "content": blocks}));
            continue;
        }
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
    if let Some(tools) = to_anthropic_tools(tools_config) {
        body["tools"] = tools;
    }
    if debug {
        println!("[IMPL LLM DEBUG] Anthropic Request URL: {}", url);
        println!(
            "[IMPL LLM DEBUG] Anthropic Request Body: {}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
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
        if debug {
            println!("[IMPL LLM DEBUG] Anthropic API Error Response: {}", text);
        }
        return Err(format!("Anthropic API error {}: {}", status, text));
    }
    let text = resp
        .text()
        .map_err(|e| format!("Failed to read response text: {}", e))?;
    if debug {
        println!("[IMPL LLM DEBUG] Anthropic Raw Response: {}", text);
    }
    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Failed to parse response: {}", e))?;

    if let Some(content) = data["content"].as_array() {
        for block in content {
            if block["type"].as_str() == Some("tool_use") {
                return Ok(LlmOutput::ToolCall {
                    name: block["name"].as_str().unwrap_or_default().to_string(),
                    arguments: block["input"].to_string(),
                    id: block["id"].as_str().unwrap_or_default().to_string(),
                });
            }
        }
    }

    data["content"]
        .as_array()
        .and_then(|arr| arr.iter().find(|b| b["type"].as_str() == Some("text")))
        .and_then(|b| b["text"].as_str())
        .map(|s| LlmOutput::Text(s.to_string()))
        .ok_or_else(|| "No content in response".to_string())
}
fn call_ollama(
    provider: &LlmProvider,
    messages: &[ChatMessage],
    system_prompt: &Option<String>,
    tools_config: Option<&ImplToolsConfig>,
    debug: bool,
) -> Result<LlmOutput, String> {
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
    for msg in messages {
        let mut m = json!({"role": msg.role, "content": msg.content});
        if let Some(tc) = &msg.tool_calls {
            m["tool_calls"] = tc.clone();
        }
        if let Some(id) = &msg.tool_call_id {
            m["tool_call_id"] = json!(id);
        }
        json_messages.push(m);
    }
    let mut body = json!({
        "model": provider.model,
        "messages": json_messages,
        "stream": false,
    });
    body["options"] = json!({ "num_ctx": provider.num_ctx });

    if let Some(tools) = build_tools_json(tools_config) {
        body["tools"] = tools;
    }
    if debug {
        println!("[IMPL LLM DEBUG] Ollama Request URL: {}", url);
        println!(
            "[IMPL LLM DEBUG] Ollama Request Body: {}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if debug {
            println!("[IMPL LLM DEBUG] Ollama API Error Response: {}", text);
        }
        return Err(format!("Ollama/llama.cpp API error {}: {}", status, text));
    }
    let text = resp
        .text()
        .map_err(|e| format!("Failed to read response text: {}", e))?;
    if debug {
        println!("[IMPL LLM DEBUG] Ollama Raw Response: {}", text);
    }
    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Failed to parse response: {}", e))?;

    let msg = &data["choices"][0]["message"];
    if let Some(tool_calls) = msg["tool_calls"].as_array() {
        if !tool_calls.is_empty() {
            let tc = &tool_calls[0];
            return Ok(LlmOutput::ToolCall {
                name: tc["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                arguments: tc["function"]["arguments"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                id: tc["id"].as_str().unwrap_or_default().to_string(),
            });
        }
    }

    msg["content"]
        .as_str()
        .map(|s| LlmOutput::Text(s.to_string()))
        .ok_or_else(|| "No content in response".to_string())
}