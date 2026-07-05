// src/app/chat.rs
use super::llm::{self, ChatMessage, LlmResponse};
use super::palette::pal;
use super::state::MergeApp;
use eframe::egui::*;

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

    pub fn system_prompt(&self) -> Option<String> {
        match self {
            ChatMode::Chat => None,
            ChatMode::Commit => Some(
                "You are a helpful assistant that generates concise, meaningful git commit messages. \
                 Only output the commit message, nothing else. Use conventional commit format if appropriate."
                    .to_string(),
            ),
            ChatMode::Impl => Some(
                "You are a code implementation assistant. When given a description or partial code, \
                 provide complete implementation in the appropriate language. Use the same style as \
                 the surrounding code. Output only the code, no explanations."
                    .to_string(),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub role: String,
    pub content: String,
    pub is_error: bool,
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
        for mode in [ChatMode::Chat, ChatMode::Commit, ChatMode::Impl] {
            let is_active = app.chat_mode == mode;
            let rich_text = RichText::new(mode.label())
                .color(if is_active {
                    mode.color()
                } else {
                    pal::TEXT_DIM
                })
                .strong()
                .size(12.0);
            if ui.selectable_label(is_active, rich_text).clicked() {
                app.chat_mode = mode;
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

    ui.add(Separator::default());

    // Check for LLM response before rendering
    let receiver = app.llm_response_receiver.take();
    if let Some(receiver) = receiver {
        let mut done = false;
        while let Ok(response) = receiver.try_recv() {
            match response {
                LlmResponse::Text(text) => {
                    app.chat_history.push(ChatEntry {
                        role: "assistant".to_string(),
                        content: text,
                        is_error: false,
                    });
                    done = true;
                    app.is_llm_loading = false;
                }
                LlmResponse::Error(err) => {
                    app.chat_history.push(ChatEntry {
                        role: "error".to_string(),
                        content: err,
                        is_error: true,
                    });
                    done = true;
                    app.is_llm_loading = false;
                }
                LlmResponse::Done => {
                    done = true;
                    app.is_llm_loading = false;
                }
            }
        }
        if !done {
            app.llm_response_receiver = Some(receiver);
        }
    }

    // Chat history (log-like buffer)
    let available_height = ui.available_height() - 50.0;
    ScrollArea::vertical()
        .id_source("chat_history_scroll")
        .auto_shrink([false, false])
        .max_height(available_height.max(100.0))
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if app.chat_history.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("🤖 LLM Chat").color(pal::TEXT_DIM).size(16.0));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Type a message and press Enter to send")
                            .color(pal::TEXT_DIM)
                            .small(),
                    );
                    ui.add_space(4.0);
                    let hint = match app.chat_mode {
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
                for entry in &app.chat_history {
                    render_chat_entry(ui, entry, max_chars, &row_font);
                }
            }

            // Loading indicator
            if app.is_llm_loading {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let dots = ".".repeat(((ui.input(|i| i.time) * 2.0).floor() as usize % 4) + 1);
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
    ui.horizontal(|ui| {
        let hint_text = match app.chat_mode {
            ChatMode::Chat => "Message... (Enter to send)",
            ChatMode::Commit => "Describe changes... (Enter to send)",
            ChatMode::Impl => "Describe implementation... (Enter to send)",
        };

        let text_edit = TextEdit::singleline(&mut app.chat_input)
            .font(FontId::monospace(11.0))
            .desired_width(panel_w - 120.0)
            .hint_text(hint_text)
            .text_color(pal::TEXT_NORMAL);

        let response = ui.add(text_edit);

        // Handle Enter key for submit
        if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift) {
            submit = true;
            response.request_focus();
        }

        let send_btn = Button::new(
            RichText::new("Send >>")
                .color(Color32::WHITE)
                .strong()
                .size(11.0),
        )
        .fill(if app.is_llm_loading {
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
            .on_hover_text("Clear chat history")
            .clicked()
        {
            app.chat_history.clear();
        }
    });

    if submit && !app.chat_input.trim().is_empty() && !app.is_llm_loading {
        let input = app.chat_input.trim().to_string();
        app.chat_input.clear();

        // Add user message to history
        app.chat_history.push(ChatEntry {
            role: "user".to_string(),
            content: input.clone(),
            is_error: false,
        });

        // For commit mode, prepend diff info
        let final_input = if app.chat_mode == ChatMode::Commit {
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
        } else if app.chat_mode == ChatMode::Impl {
            let mut context = String::new();
            if !app.file_lines.is_empty() {
                context.push_str(&format!("Current file: {}\n\n", app.file_path));
                // Include surrounding context around cursor
                if let Some(cursor) = app.cursor_line {
                    let start = cursor.saturating_sub(20);
                    let end = (cursor + 20).min(app.file_lines.len());
                    context.push_str("Context around cursor:\n```\n");
                    for (i, line) in app.file_lines[start..end].iter().enumerate() {
                        let line_num = start + i + 1;
                        let marker = if line_num == cursor + 1 { ">>" } else { "  " };
                        context.push_str(&format!("{} {}\n", marker, line));
                    }
                    context.push_str("```\n\n");
                }
            }
            context.push_str(&input);
            context
        } else {
            input
        };

        // Build messages from history (exclude errors, include last user message with context)
        let mut messages: Vec<ChatMessage> = app
            .chat_history
            .iter()
            .filter(|e| e.role == "user" || e.role == "assistant")
            .map(|e| ChatMessage {
                role: e.role.clone(),
                content: e.content.clone(),
            })
            .collect();

        // Replace the last user message with the context-enriched version
        if let Some(last) = messages.last_mut() {
            if last.role == "user" {
                last.content = final_input;
            }
        }

        let provider = app.current_chat_provider().clone();
        let system_prompt = app.chat_mode.system_prompt();

        app.llm_response_receiver = Some(llm::send_to_llm(provider, messages, system_prompt));
        app.is_llm_loading = true;
    }
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
            let lines = wrap_text(&entry.content, max_chars);
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
