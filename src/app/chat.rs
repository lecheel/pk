// src/app/chat.rs
// src/app/chat.rs
use super::llm::{self, ChatMessage, LlmResponse};
use super::palette::pal;
use super::state::MergeApp;
use super::types::StatusMessage;
use eframe::egui::*;
use std::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
pub enum ChatMode {
    Chat,
    Commit,
    Impl,
}

impl ChatMode {
    pub fn label(&self) -> &'static str {
        match self {
            ChatMode::Chat => "💬 Chat",
            ChatMode::Commit => "📝 Commit",
            ChatMode::Impl => "⚡ Impl",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            ChatMode::Chat => "Chat",
            ChatMode::Commit => "Commit",
            ChatMode::Impl => "Impl",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            ChatMode::Chat => Color32::from_rgb(120, 180, 255),
            ChatMode::Commit => Color32::from_rgb(120, 230, 160),
            ChatMode::Impl => Color32::from_rgb(230, 200, 120),
        }
    }

    pub fn system_prompt(&self) -> String {
        match self {
            ChatMode::Chat => "You are a helpful and concise AI assistant embedded in a code editor. \
                 Help the user with programming tasks, answer questions, and provide clear explanations."
                .to_string(),
            ChatMode::Commit => "You are a helpful assistant that generates concise, meaningful git commit messages. \
                 Only output the commit message, nothing else. Use conventional commit format if appropriate."
                .to_string(),
            ChatMode::Impl => "You are an autonomous code implementation assistant. Follow these steps strictly:\n\
### 1. PREPARE\n\
- Analyze the user's request to determine if it is an implementation or bugfix intention.\n\
- Use the `get_skeleton` tool to view the project overview and understand the architecture.\n\
- NEVER guess the existing code structure, whitespace, or indentation.\n\
- Based on the project size (LOC), use `get_files` or `get_hashes` to locate the specific files that need changes.\n\
\n\
### 2. SUMMARY\n\
- Once you have gathered enough context, provide a concise summary of your intention and the files you plan to modify.\n\
- STOP and wait for the user to confirm your intention before proceeding to the implementation step.\n\
\n\
### 3. IMPL\n\
- Only after the user confirms, generate the search/replace patch.\n\
- Use the `save_impl_patch` tool to save the generated patch to `todo.md`. Do not output the patch directly in the chat.\n\
- After saving, inform the user that the patch has been saved to `todo.md`.\n\
\n\
Search/replace format:\n\
```// src/filename.rs\n<<<<<<< SEARCH\n[exact original lines (include enough context to be unique)]\n=======\n[modified lines]\n>>>>>>> REPLACE```\n\
\n\
If multiple files need changes, include multiple blocks in the `patch` argument of the `save_impl_patch` tool. Ensure the `SEARCH` block exactly matches the existing code, including whitespace.".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub role: String,
    pub content: String,
    pub is_error: bool,
    pub tool_name: Option<String>,
    pub tool_id: Option<String>,
}
/// Per-mode LLM session. Chat, Commit, and Impl each get their own
/// history/receiver so switching modes never mixes context or responses.
#[derive(Default)]
pub struct ChatSession {
    pub history: Vec<ChatEntry>,
    pub input: String,
    pub receiver: Option<mpsc::Receiver<LlmResponse>>,
    pub is_loading: bool,
    pub start_time: Option<f64>,
}
impl ChatSession {
    pub fn cancel(&mut self) {
        self.is_loading = false;
        self.start_time = None;
        self.receiver = None;
    }
}
#[derive(Default)]
pub struct ChatSessions {
    pub chat: ChatSession,
    pub commit: ChatSession,
    pub impl_: ChatSession,
}
impl ChatSessions {
    pub fn get_mut(&mut self, mode: &ChatMode) -> &mut ChatSession {
        match mode {
            ChatMode::Chat => &mut self.chat,
            ChatMode::Commit => &mut self.commit,
            ChatMode::Impl => &mut self.impl_,
        }
    }
}
/// Rebuilds the outgoing message list from a session's full history,
/// including prior tool calls/results, so multi-turn Impl workflows keep
/// their memory of which tools already ran instead of starting fresh.
fn history_to_messages(history: &[ChatEntry]) -> Vec<ChatMessage> {
    history
        .iter()
        .filter_map(|e| match e.role.as_str() {
            "user" | "assistant" => Some(ChatMessage {
                role: e.role.clone(),
                content: e.content.clone(),
                tool_calls: None,
                tool_call_id: None,
            }),
            "tool_call" => Some(ChatMessage {
                role: "assistant".to_string(),
                content: String::new(),
                tool_calls: Some(serde_json::json!([{
                    "id": e.tool_id.clone().unwrap_or_default(),
                    "type": "function",
                    "function": {
                        "name": e.tool_name.clone().unwrap_or_default(),
                        "arguments": e.content.clone()
                    }
                }])),
                tool_call_id: None,
            }),
            "tool_result" => Some(ChatMessage {
                role: "tool".to_string(),
                content: e.content.clone(),
                tool_calls: None,
                tool_call_id: e.tool_id.clone(),
            }),
            _ => None,
        })
        .collect()
}

pub fn render_chat_panel(app: &mut MergeApp, ui: &mut Ui, panel_w: f32) {
    let row_font = FontId::monospace(11.0);
    let char_w = ui.fonts(|f| {
        let w1 = f
            .layout_no_wrap("0".to_string(), row_font.clone(), Color32::WHITE)
            .rect
            .width();
        let w2 = f
            .layout_no_wrap("00".to_string(), row_font.clone(), Color32::WHITE)
            .rect
            .width();
        w2 - w1
    });
    // Mode selector and provider info
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for m in [ChatMode::Chat, ChatMode::Commit, ChatMode::Impl] {
            let is_active = app.chat_mode == m;
            let rich_text = RichText::new(m.label())
                .color(if is_active {
                    m.color()
                } else {
                    pal::TEXT_DIM
                })
                .strong()
                .size(12.0);
            if ui.selectable_label(is_active, rich_text).clicked() {
                app.chat_mode = m.clone();
                if m == ChatMode::Impl && app.impl_skeleton.is_empty() && !app.impl_is_running {
                    app.start_impl_round();
                }
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let provider = app.current_chat_provider();
            ui.label(
                RichText::new(format!("{} / {}", provider.name(), provider.model))
                    .color(pal::TEXT_DIM)
                    .small(),
            );
        });
    });
    // Snapshot the mode *after* the selector runs, so a click this frame is
    // reflected immediately instead of lagging one frame behind.
    let mode = app.chat_mode.clone();
    ui.add(Separator::default());
    if mode == ChatMode::Impl {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Status: {}", app.impl_result_indicator))
                    .color(pal::TEXT_NORMAL)
                    .strong(),
            );
            if !app.impl_is_running {
                let btn = Button::new(
                    RichText::new("🔄 Refetch Context")
                        .color(Color32::WHITE)
                        .strong(),
                )
                .fill(Color32::from_rgb(40, 90, 55));
                if ui.add(btn).clicked() {
                    app.start_impl_round();
                }
            } else {
                ui.label(RichText::new("Fetching...").color(pal::ACCENT_INFO));
            }
        });
        ui.add(Separator::default());
    }
    // Drain only *this* mode's session receiver. Chat/Commit/Impl each own
    // their history and in-flight receiver, so switching tabs can never mix
    // context or let a stale response land in the wrong conversation.
    {
        let receiver = app.chat_sessions.get_mut(&mode).receiver.take();
        if let Some(receiver) = receiver {
            let mut finished = false;
            while let Ok(response) = receiver.try_recv() {
                match response {
                    LlmResponse::Text(text) => {
                        app.chat_sessions.get_mut(&mode).history.push(ChatEntry {
                            role: "assistant".to_string(),
                            content: text,
                            is_error: false,
                            tool_name: None,
                            tool_id: None,
                        });
                    }
                    LlmResponse::ToolUse { name, arguments, id } => {
                        // Persisted into history (not just used locally by the
                        // request thread) so the *next* turn still remembers
                        // which tools already ran.
                        app.chat_sessions.get_mut(&mode).history.push(ChatEntry {
                            role: "tool_call".to_string(),
                            content: arguments,
                            is_error: false,
                            tool_name: Some(name),
                            tool_id: Some(id),
                        });
                    }
                    LlmResponse::ToolResult { id, name, result } => {
                        app.chat_sessions.get_mut(&mode).history.push(ChatEntry {
                            role: "tool_result".to_string(),
                            content: result,
                            is_error: false,
                            tool_name: Some(name),
                            tool_id: Some(id),
                        });
                    }
                    LlmResponse::Error(err) => {
                        app.chat_sessions.get_mut(&mode).history.push(ChatEntry {
                            role: "error".to_string(),
                            content: err,
                            is_error: true,
                            tool_name: None,
                            tool_id: None,
                        });
                        finished = true;
                    }
                    LlmResponse::Done => {
                        finished = true;
                    }
                }
            }
            let session = app.chat_sessions.get_mut(&mode);
            if finished {
                session.is_loading = false;
                session.receiver = None;
            } else {
                session.receiver = Some(receiver);
            }
        }
    }
    let is_loading = app.chat_sessions.get_mut(&mode).is_loading;
    let history_snapshot = app.chat_sessions.get_mut(&mode).history.clone();
    let available_height = ui.available_height() - 50.0;
    ScrollArea::vertical()
        .id_source("chat_history_scroll")
        .auto_shrink([false, false])
        .max_height(available_height.max(100.0))
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if history_snapshot.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("💬 LLM Chat").color(pal::TEXT_DIM).size(16.0));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Type a message and press Enter to send")
                            .color(pal::TEXT_DIM)
                            .small(),
                    );
                    ui.add_space(4.0);
                    let hint = match mode {
                        ChatMode::Chat => "Chat mode: General conversation with AI",
                        ChatMode::Commit => {
                            "Commit mode: Describe changes to generate commit message"
                        }
                        ChatMode::Impl => "Impl mode: Describe what to implement",
                    };
                    ui.label(RichText::new(hint).color(pal::TEXT_DIM).small().italics());
                });
            } else {
                let max_chars = ((panel_w - 30.0) / char_w).floor() as usize;
                for entry in &history_snapshot {
                    render_chat_entry(ui, entry, max_chars, &row_font);
                }
            }
            // Loading indicator
            if is_loading {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let dots =
                        ".".repeat(((ui.input(|i| i.time) * 2.0).floor() as usize % 4) + 1);
                    ui.label(
                        RichText::new(format!("Thinking{}", dots))
                            .color(pal::ACCENT_INFO)
                            .italics(),
                    );
                });
            }
        });
    ui.add(Separator::default());
    // Input area - single line for Enter to submit
    let mut submit = false;
    let mut input_buf = std::mem::take(&mut app.chat_sessions.get_mut(&mode).input);
    ui.horizontal(|ui| {
        let hint_text = match mode {
            ChatMode::Chat => "Message... (Enter to send)",
            ChatMode::Commit => "Describe changes... (Enter to send)",
            ChatMode::Impl => "Describe implementation... (Enter to send)",
        };
        let text_edit = TextEdit::singleline(&mut input_buf)
            .font(FontId::monospace(11.0))
            .desired_width(panel_w - 120.0)
            .hint_text(hint_text)
            .text_color(pal::TEXT_NORMAL);
        let response = ui.add(text_edit);
        // Handle Enter key for submit
        if response.lost_focus()
            && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
        {
            submit = true;
            response.request_focus();
        }
        let send_btn = Button::new(
            RichText::new("Send >>")
                .color(Color32::WHITE)
                .strong()
                .size(11.0),
        )
        .fill(if is_loading {
            Color32::from_gray(60)
        } else {
            Color32::from_rgb(40, 90, 55)
        });
        if ui.add(send_btn).clicked() {
            submit = true;
        }
        if ui
            .add(
                Button::new(RichText::new("Clear").color(Color32::WHITE).size(11.0))
                    .small()
                    .fill(Color32::from_rgb(80, 40, 40)),
            )
            .on_hover_text("Clear this mode's chat history")
            .clicked()
        {
            app.chat_sessions.get_mut(&mode).history.clear();
        }
    });
    if submit && !input_buf.trim().is_empty() && !is_loading {
        let input = input_buf.trim().to_string();
        input_buf.clear();
        // Add user message to this mode's history
        app.chat_sessions.get_mut(&mode).history.push(ChatEntry {
            role: "user".to_string(),
            content: input.clone(),
            is_error: false,
            tool_name: None,
            tool_id: None,
        });
        // For commit mode, prepend diff info. Chat/Impl no longer manually
        // stuff skeleton/files/hashes text here — Impl mode gets that
        // context on demand via tool calling instead, so it isn't sent
        // twice (once as text, once as a callable tool).
        let final_input = if mode == ChatMode::Commit {
            let mut context = String::new();
            if !app.git_diff_rows.is_empty() {
                context.push_str("Current staged/working changes:\n```\n");
                for row in &app.git_diff_rows {
                    match row.kind {
                        crate::diff::RowKind::Equal => {
                            if let Some(l) = &row.left {
                                context.push_str(&format!(" {}\n", l));
                            }
                        }
                        crate::diff::RowKind::Delete => {
                            if let Some(l) = &row.left {
                                context.push_str(&format!("-{}\n", l));
                            }
                        }
                        crate::diff::RowKind::Insert => {
                            if let Some(l) = &row.right {
                                context.push_str(&format!("+{}\n", l));
                            }
                        }
                    }
                }
                context.push_str("```\n\n");
            }
            context.push_str(&input);
            context
        } else {
            input.clone()
        };
        // Rebuild the full request from this mode's history, including any
        // prior tool_call/tool_result turns, so multi-round Impl workflows
        // retain memory of which tools already ran.
        let mut messages = history_to_messages(&app.chat_sessions.get_mut(&mode).history);
        if let Some(last) = messages.last_mut() {
            if last.role == "user" {
                last.content = final_input;
            }
        }
        let provider = app.current_chat_provider().clone();
        let system_prompt = app.active_system_prompt();
        let tools_config = if mode == ChatMode::Impl {
            Some(app.impl_tools.clone())
        } else {
            None
        };
        let base_dir = app.base_dir.clone();
        let concat_base_url = app.rustconcat_api_url.clone();
        let debug = app.debug_impl_llm;
        let session = app.chat_sessions.get_mut(&mode);
        session.receiver = Some(llm::send_to_llm(
            provider,
            messages,
            system_prompt,
            tools_config,
            concat_base_url,
            base_dir,
            debug,
        ));
        session.is_loading = true;
    }
    app.chat_sessions.get_mut(&mode).input = input_buf;
}

fn render_chat_entry(ui: &mut Ui, entry: &ChatEntry, max_chars: usize, font: &FontId) {
    let (prefix, color, bg) = match entry.role.as_str() {
        "user" => (
            "You:",
            Color32::from_rgb(120, 180, 255),
            Color32::from_rgb(25, 35, 55),
        ),
        "assistant" => (
            "AI:",
            Color32::from_rgb(120, 230, 160),
            Color32::from_rgb(25, 45, 30),
        ),
        "tool_call" => (
            "Tool call:",
            Color32::from_rgb(230, 200, 120),
            Color32::from_rgb(45, 40, 20),
        ),
        "tool_result" => (
            "Tool result:",
            Color32::from_rgb(180, 180, 190),
            Color32::from_rgb(35, 35, 40),
        ),
        "error" => (
            "Error:",
            Color32::from_rgb(230, 100, 100),
            Color32::from_rgb(50, 25, 25),
        ),
        _ => ("???:", pal::TEXT_DIM, pal::BG_PANEL),
    };

    ui.add_space(2.0);
    Frame::none()
        .fill(bg)
        .inner_margin(Margin::symmetric(8.0, 4.0))
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(prefix)
                        .color(color)
                        .strong()
                        .font(font.clone()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(entry.role.as_str())
                            .color(Color32::from_gray(60))
                            .small(),
                    );
                });
            });

            // Wrap text
            // Wrap text
            let content_display = if let Some(name) = &entry.tool_name {
                format!("[{}] {}", name, entry.content)
            } else {
                entry.content.clone()
            };
            let lines = wrap_text(&content_display, max_chars);
            for line in lines {
                ui.label(
                    RichText::new(line)
                        .color(if entry.is_error {
                            pal::TEXT_DELETE
                        } else {
                            pal::TEXT_NORMAL
                        })
                        .font(font.clone()),
                );
            }
        });
    ui.add_space(2.0);
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut result = Vec::new();
    for line in text.lines() {
        if line.len() <= max_chars {
            result.push(line.to_string());
        } else {
            // Word wrap
            let mut current = String::new();
            for word in line.split_whitespace() {
                if current.len() + word.len() + 1 > max_chars && !current.is_empty() {
                    result.push(current);
                    current = word.to_string();
                } else {
                    if !current.is_empty() {
                        current.push(' ');
                    }
                    current.push_str(word);
                }
            }
            if !current.is_empty() {
                result.push(current);
            }
        }
    }
    result
}

pub fn render_llm_settings(app: &mut MergeApp, ui: &mut Ui) {
    ui.add_space(8.0);
    ui.heading("LLM Configuration");
    ui.add_space(8.0);

    let providers = ["OpenAI", "Anthropic", "Ollama"];

    render_provider_config(
        ui,
        "Chat Provider:",
        &mut app.llm_config.chat_provider,
        &providers,
    );
    ui.add_space(12.0);
    render_provider_config(
        ui,
        "Commit Provider:",
        &mut app.llm_config.commit_provider,
        &providers,
    );
    ui.add_space(12.0);
    render_provider_config(
        ui,
        "Impl Provider:",
        &mut app.llm_config.impl_provider,
        &providers,
    );

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);
    ui.heading("Custom System Prompts");
    ui.add_space(4.0);
    ui.label(
        RichText::new("Leave empty to use built-in defaults.")
            .color(pal::TEXT_DIM)
            .small(),
    );
    ui.add_space(8.0);

    ui.label(RichText::new("Chat Prompt:").strong());
    ui.add(
        TextEdit::multiline(&mut app.llm_config.chat_system_prompt)
            .desired_rows(3)
            .hint_text("Default: Helpful assistant...")
            .font(FontId::monospace(10.0)),
    );
    ui.add_space(8.0);

    ui.label(RichText::new("Commit Prompt:").strong());
    ui.add(
        TextEdit::multiline(&mut app.llm_config.commit_system_prompt)
            .desired_rows(3)
            .hint_text("Default: Generate conventional commit message...")
            .font(FontId::monospace(10.0)),
    );
    ui.add_space(8.0);

    ui.label(RichText::new("Impl Prompt:").strong());
    ui.add(
        TextEdit::multiline(&mut app.llm_config.impl_system_prompt)
            .desired_rows(3)
            .hint_text("Default: Code implementation assistant...")
            .font(FontId::monospace(10.0)),
    );

    ui.add_space(16.0);
    if ui.button("💾 Save LLM Config").clicked() {
        app.save_config();
        app.set_message(super::types::StatusMessage::success("LLM config saved"));
    }
}

fn render_provider_config(
    ui: &mut Ui,
    label: &str,
    provider: &mut super::llm::LlmProvider,
    provider_names: &[&str],
) {
    ui.label(RichText::new(label).strong().color(pal::TEXT_NORMAL));
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label("Type:");
        for (i, name) in provider_names.iter().enumerate() {
            let is_active = provider.variant_index() == i;
            if ui.selectable_label(is_active, *name).clicked() {
                provider.set_variant(i);
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Model:");
        ui.add(
            TextEdit::singleline(&mut provider.model)
                .desired_width(200.0)
                .font(FontId::monospace(10.0)),
        );
    });

    ui.horizontal(|ui| {
        ui.label("URL:");
        ui.add(
            TextEdit::singleline(&mut provider.base_url)
                .desired_width(300.0)
                .hint_text("http://localhost:11434")
                .font(FontId::monospace(10.0)),
        );
    });

    ui.horizontal(|ui| {
        ui.label("Key: ");
        let mut key_str = provider.api_key.clone().unwrap_or_default();
        let resp = ui.add(
            TextEdit::singleline(&mut key_str)
                .desired_width(300.0)
                .password(true)
                .hint_text("sk-... (leave empty for Ollama)")
                .font(FontId::monospace(10.0)),
        );
        if resp.changed() {
            provider.api_key = if key_str.is_empty() {
                None
            } else {
                Some(key_str)
            };
        }
    });
}