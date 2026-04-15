use crate::ai;
use crate::blocks;
use crate::terminal::TerminalEvent;

impl super::App {
    /// Auto-load the last-used model from config on startup.
    pub(crate) fn auto_load_model(&mut self) {
        if !self.config.ai.auto_load {
            return;
        }
        let last = &self.config.ai.last_model;
        if last.is_empty() {
            return;
        }
        let models = ai::registry::MODELS;
        if let Some(idx) = models.iter().position(|m| m.name == last) {
            self.load_model_by_index(idx);
        }
    }

    /// Load a model by its registry index.
    pub(crate) fn load_model_by_index(&mut self, idx: usize) {
        let models = ai::registry::MODELS;
        if idx >= models.len() {
            return;
        }
        let model = &models[idx];
        let models_dir = ai::model_manager::models_dir();
        let path = models_dir.join(model.filename);

        if !path.exists() {
            return;
        }

        if self.ai_ctrl.state.loaded_model_name.as_deref() == Some(model.name) {
            return;
        }

        self.overlay.close_model_picker();

        self.ai_ctrl.state.loaded_model = None;
        self.ai_ctrl.state.handle_rx = None;

        let model_name = model.name.to_string();

        let path_clone = path.clone();
        let proxy = self.proxy.clone();
        let error_proxy = self.proxy.clone();

        let (tx, rx) = std::sync::mpsc::channel();
        self.ai_ctrl.state.handle_rx = Some(rx);
        self.ai_ctrl.state.loaded_model_name = Some(model_name);
        self.ai_ctrl.state.context_size = model.context_size;
        self.save_config();

        tokio::task::spawn_blocking(move || {
            log::info!("Loading model from {:?}", path_clone);
            let backend = match llama_cpp_2::llama_backend::LlamaBackend::init() {
                Ok(b) => b,
                Err(e) => {
                    log::error!("Failed to init llama backend: {e:?}");
                    let _ = error_proxy.send_event(TerminalEvent::AiError(format!(
                        "Failed to init AI backend: {e}"
                    )));
                    return;
                }
            };
            let params =
                llama_cpp_2::model::params::LlamaModelParams::default().with_n_gpu_layers(999);
            let model = match llama_cpp_2::model::LlamaModel::load_from_file(
                &backend,
                &path_clone,
                &params,
            ) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Failed to load model: {e:?}");
                    let _ = error_proxy
                        .send_event(TerminalEvent::AiError(format!("Failed to load model: {e}")));
                    return;
                }
            };
            let model = Box::new(model);

            let n_ctx = 4096u32;
            let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
                .with_n_ctx(std::num::NonZero::new(n_ctx));

            // SAFETY: context borrows model. We ensure model outlives context
            let model_ref: &'static llama_cpp_2::model::LlamaModel =
                unsafe { &*(&*model as *const llama_cpp_2::model::LlamaModel) };

            let context = match model_ref.new_context(&backend, ctx_params) {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed to create context: {e:?}");
                    let _ = error_proxy.send_event(TerminalEvent::AiError(format!(
                        "Failed to create context: {e}"
                    )));
                    return;
                }
            };

            let handle = ai::model_manager::LoadedModelHandle {
                _backend: backend,
                model,
                context,
                n_ctx,
            };

            let _ = tx.send(handle);
            let _ = proxy.send_event(crate::terminal::TerminalEvent::Wakeup);
        });
    }

    /// Find the smallest downloaded model and load it automatically.
    /// Returns `true` if a model was found and loading was initiated.
    pub(crate) fn auto_load_best_efficient(&mut self) -> bool {
        if self.ai_ctrl.state.loaded_model.is_some()
            || self.ai_ctrl.state.loaded_model_name.is_some()
        {
            return true;
        }

        let models_dir = ai::model_manager::models_dir();
        let best = ai::registry::MODELS
            .iter()
            .enumerate()
            .filter(|(_, m)| models_dir.join(m.filename).exists())
            .min_by_key(|(_, m)| m.size_bytes);

        if let Some((idx, _)) = best {
            self.load_model_by_index(idx);
            true
        } else {
            false
        }
    }

    /// Default model name to auto-download when no models exist.
    const DEFAULT_MODEL_NAME: &'static str = "Gemma 4 E2B";

    /// Start downloading the default model (Gemma 4 E2B) and show a progress block.
    ///
    /// After download completes, the model will be auto-loaded on the next
    /// `poll_model_downloads` cycle, which in turn fires `pending_agent_task`.
    pub(crate) fn auto_download_default_model(&mut self, task_hint: &str) {
        let idx = match ai::registry::MODELS
            .iter()
            .position(|m| m.name == Self::DEFAULT_MODEL_NAME)
        {
            Some(i) => i,
            None => {
                self.push_agent_block("No default model found in registry.");
                return;
            }
        };

        if self.models_view.is_downloading(Self::DEFAULT_MODEL_NAME) {
            return;
        }

        let model = &ai::registry::MODELS[idx];
        let dest_dir = std::path::PathBuf::from(&self.settings_state.models_path);
        let (tx, rx) = std::sync::mpsc::channel();

        ai::model_manager::download_model(
            model.hf_repo,
            model.hf_filename,
            model.name,
            &dest_dir,
            tx,
            self.proxy.clone(),
        );

        self.models_view
            .download_receivers
            .push((model.name.to_string(), rx));

        self.auto_download_model_idx = Some(idx);

        let cmd = format!("/agent {task_hint}");
        if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
            && let super::TabKind::Terminal {
                terminal,
                block_list,
                ..
            } = &mut tab.kind
        {
            let prompt_info = terminal.prompt_info();
            block_list.push_command(prompt_info, cmd);
            block_list.append_output_text(&format!("Downloading {} …", model.name));
        }
    }

    /// Start an AI inference from a `/ask <query>` command.
    ///
    /// Creates an AI command block and starts streaming inference.
    /// Tokens are polled in the `Wakeup` handler and appended directly
    /// to the block's output (no PTY round-trip).
    pub(crate) fn start_ai_query(&mut self, query: &str) {
        let result = self.usage_tracker.can_use(crate::usage::Feature::Ask);
        if let crate::usage::UsageResult::Denied { used, limit } = result {
            self.show_limit_reached(crate::usage::Feature::Ask, used, limit);
            return;
        }

        let ollama_enabled = self.config.ai.ollama_enabled;
        let ollama_model_set = !self.config.ai.ollama_model.is_empty();

        if ollama_enabled && ollama_model_set {
            self.start_ai_query_ollama(query);
            return;
        }

        if self.ai_ctrl.state.loaded_model.is_none()
            && self.ai_ctrl.state.loaded_model_name.is_none()
            && !self.auto_load_best_efficient()
        {
            self.auto_download_default_model(&format!("/ask {query}"));
            self.pending_ai_query = Some(query.to_string());
            return;
        }

        if self.ai_ctrl.state.loaded_model.is_none() {
            let model_label = self
                .ai_ctrl
                .state
                .loaded_model_name
                .clone()
                .unwrap_or_else(|| "model".into());
            if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                && let super::TabKind::Terminal {
                    terminal,
                    block_list,
                    ..
                } = &mut tab.kind
            {
                let prompt_info = terminal.prompt_info();
                block_list.push_command(prompt_info, format!("/ask {}", query));
                block_list.append_output_text(&format!("Loading {model_label}…"));
            }
            self.pending_ai_query = Some(query.to_string());
            return;
        }

        let context_lines = self
            .active_block_list()
            .map(|bl| bl.context_for_ai(self.ai_ctrl.state.context_lines))
            .unwrap_or_default();

        let model_handle = self.ai_ctrl.state.loaded_model.take().unwrap();

        let tab_messages = if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
            && let super::TabKind::Terminal {
                terminal,
                block_list,
                ai_messages,
                ..
            } = &mut tab.kind
        {
            let prompt_info = terminal.prompt_info();
            block_list.push_command(prompt_info, format!("/ask {}", query));
            if let Some(block) = block_list.blocks.last_mut() {
                block.thinking = true;
            }
            ai_messages.push(crate::ai::ChatMessage {
                role: crate::ai::MessageRole::User,
                content: query.to_string(),
                streaming: false,
            });
            ai_messages.clone()
        } else {
            vec![crate::ai::ChatMessage {
                role: crate::ai::MessageRole::User,
                content: query.to_string(),
                streaming: false,
            }]
        };

        self.maybe_show_first_use_hint(crate::usage::Feature::Ask);
        self.usage_tracker.record_use(crate::usage::Feature::Ask);

        let fallback_template = self.ai_ctrl.fallback_template();
        let prompt = self.ai_ctrl.state.build_prompt(
            &tab_messages,
            &context_lines,
            &model_handle.model,
            fallback_template,
        );

        let (token_tx, token_rx) = std::sync::mpsc::channel();
        let cancel = self.ai_ctrl.arm_cancel();
        let jh = ai::inference::run_inference(
            model_handle,
            prompt,
            token_tx,
            self.proxy.clone(),
            cancel,
        );
        self.ai_ctrl.state.inference_rx = Some(token_rx);
        self.ai_ctrl.state.inference_handle = Some(jh);
        self.ai_ctrl.state.begin_assistant_message();
    }

    fn start_ai_query_ollama(&mut self, query: &str) {
        let context_lines = self
            .active_block_list()
            .map(|bl| bl.context_for_ai(self.ai_ctrl.state.context_lines))
            .unwrap_or_default();

        let tab_messages = if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
            && let super::TabKind::Terminal {
                terminal,
                block_list,
                ai_messages,
                ..
            } = &mut tab.kind
        {
            let prompt_info = terminal.prompt_info();
            block_list.push_command(prompt_info, format!("/ask {}", query));
            if let Some(block) = block_list.blocks.last_mut() {
                block.thinking = true;
            }
            ai_messages.push(crate::ai::ChatMessage {
                role: crate::ai::MessageRole::User,
                content: query.to_string(),
                streaming: false,
            });
            ai_messages.clone()
        } else {
            vec![crate::ai::ChatMessage {
                role: crate::ai::MessageRole::User,
                content: query.to_string(),
                streaming: false,
            }]
        };

        self.maybe_show_first_use_hint(crate::usage::Feature::Ask);
        self.usage_tracker.record_use(crate::usage::Feature::Ask);

        let system = format!(
            "You are a helpful terminal assistant. \
             Answer concisely. When suggesting commands, use code blocks. \
             You have access to the user's recent terminal output below.\n\n{}",
            context_lines.join("\n")
        );
        let messages = ai::ollama::build_chat_messages(&system, &tab_messages);

        let (token_tx, token_rx) = std::sync::mpsc::channel();
        let cancel = self.ai_ctrl.arm_cancel();
        let host = self.config.ai.ollama_host.clone();
        let model = self.config.ai.ollama_model.clone();
        let _jh = ai::ollama::stream_chat(
            &host,
            &model,
            messages,
            token_tx,
            self.proxy.clone(),
            cancel,
        );

        self.ai_ctrl.state.inference_rx = Some(token_rx);
        self.ai_ctrl.state.begin_assistant_message();
    }

    /// Poll inference tokens from the AI thread and stream them into the
    /// current command block's output.
    pub(crate) fn poll_ai_tokens(&mut self) {
        let got_token = self.ai_ctrl.state.poll_inference();

        if self.git_panel.generating_commit_msg {
            if got_token && let Some(msg) = self.ai_ctrl.state.messages.last() {
                let raw = msg.content.trim().to_string();
                let cleaned = strip_commit_artifacts(&raw);
                self.git_panel.commit_message = cleaned;
                self.git_panel.cursor = self.git_panel.commit_message.len();
                self.git_panel.selection_anchor = None;
            }
            if self.ai_ctrl.state.inference_rx.is_none() {
                self.git_panel.generating_commit_msg = false;
                self.ai_ctrl.block_written = 0;
                self.ai_ctrl.state.messages.clear();
                self.request_redraw();
            }
            return;
        }

        if got_token {
            if self.agent.is_some() {
                let buf = &self.ai_ctrl.state.agent_response_buf;
                if !buf.is_empty() {
                    self.ai_ctrl.block_written = buf.len();
                }
            } else {
                let full_content: Option<String> = self
                    .ai_ctrl
                    .state
                    .messages
                    .last()
                    .filter(|msg| !msg.content.is_empty())
                    .map(|msg| msg.content.clone());

                if let Some(full_text) = full_content {
                    let display_text = ai::strip_channel_tokens(&full_text);
                    if let Some(bl) = self.active_block_list_mut() {
                        if let Some(block) = bl.blocks.last_mut()
                            && block.thinking
                        {
                            block.thinking = false;
                        }
                        bl.set_output_markdown(&display_text);
                    }
                    self.ai_ctrl.block_written = full_text.len();
                }
            }
        }

        if self.ai_ctrl.state.inference_rx.is_none() && self.ai_ctrl.block_written > 0 {
            if self.agent.is_some() {
                let full_response = self.ai_ctrl.state.agent_response_buf.clone();
                self.ai_ctrl.state.agent_response_buf.clear();

                if let Some(bl) = self.active_block_list_mut()
                    && let Some(last) = bl.blocks.last()
                    && (last.thinking
                        || matches!(
                            last.agent_step,
                            Some(crate::blocks::AgentStepKind::Thinking)
                        ))
                {
                    bl.blocks.pop();
                    bl.bump_generation();
                }
                self.ai_ctrl.block_written = 0;
                self.on_agent_inference_complete(full_response);
                return;
            }

            let full_response = self
                .ai_ctrl
                .state
                .messages
                .last()
                .map(|m| m.content.clone())
                .unwrap_or_default();

            if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                && let super::TabKind::Terminal { ai_messages, .. } = &mut tab.kind
            {
                ai_messages.push(crate::ai::ChatMessage {
                    role: crate::ai::MessageRole::Assistant,
                    content: full_response,
                    streaming: false,
                });
            }

            self.ai_ctrl.state.messages.clear();

            if let Some(bl) = self.active_block_list_mut() {
                bl.finish_last();
            }
            self.record_last_block();
            self.ai_ctrl.block_written = 0;
        }
    }

    /// Start a `/summarize` inference — uses the same streaming path as `/ask`
    /// but with a summarization-focused system prompt.
    pub(crate) fn start_summarize(&mut self) {
        if self.config.ai.ollama_enabled && !self.config.ai.ollama_model.is_empty() {
            self.start_summarize_ollama();
            return;
        }

        let context_lines = self
            .active_block_list()
            .map(|bl| bl.context_for_ai(self.ai_ctrl.state.context_lines))
            .unwrap_or_default();

        let model_handle = match self.ai_ctrl.state.loaded_model.take() {
            Some(h) => h,
            None => {
                if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                    && let super::TabKind::Terminal {
                        terminal,
                        block_list,
                        ..
                    } = &mut tab.kind
                {
                    let prompt_info = terminal.prompt_info();
                    block_list.push_command(prompt_info, "/summarize".to_string());

                    let models_dir = ai::model_manager::models_dir();
                    let available: Vec<&str> = ai::registry::MODELS
                        .iter()
                        .filter(|m| models_dir.join(m.filename).exists())
                        .map(|m| m.name)
                        .collect();

                    let msg = if available.is_empty() {
                        "[AI] No model loaded. Use /models to browse and download one.".to_string()
                    } else {
                        format!(
                            "[AI] No model loaded. Available: {}. Use /models to manage.",
                            available.join(", ")
                        )
                    };
                    block_list.append_output_text(&msg);
                    if let Some(block) = block_list.blocks.last_mut() {
                        block.is_error = true;
                    }
                    block_list.finish_last();
                }
                self.record_last_block();
                self.open_models_view();
                return;
            }
        };

        if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
            && let super::TabKind::Terminal {
                terminal,
                block_list,
                ..
            } = &mut tab.kind
        {
            let prompt_info = terminal.prompt_info();
            block_list.push_command(prompt_info, "/summarize".to_string());
            if let Some(block) = block_list.blocks.last_mut() {
                block.thinking = true;
            }
        }

        self.ai_ctrl
            .state
            .add_user_message("Summarize my recent terminal session.".to_string());

        let fallback_template = self.ai_ctrl.fallback_template();
        let prompt = self.ai_ctrl.state.build_summarize_prompt(
            &context_lines,
            &model_handle.model,
            fallback_template,
        );

        let (token_tx, token_rx) = std::sync::mpsc::channel();
        let cancel = self.ai_ctrl.arm_cancel();
        let jh = ai::inference::run_inference(
            model_handle,
            prompt,
            token_tx,
            self.proxy.clone(),
            cancel,
        );
        self.ai_ctrl.state.inference_rx = Some(token_rx);
        self.ai_ctrl.state.inference_handle = Some(jh);
        self.ai_ctrl.state.begin_assistant_message();
    }

    fn start_summarize_ollama(&mut self) {
        let context_lines = self
            .active_block_list()
            .map(|bl| bl.context_for_ai(self.ai_ctrl.state.context_lines))
            .unwrap_or_default();

        if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
            && let super::TabKind::Terminal {
                terminal,
                block_list,
                ..
            } = &mut tab.kind
        {
            let prompt_info = terminal.prompt_info();
            block_list.push_command(prompt_info, "/summarize".to_string());
            if let Some(block) = block_list.blocks.last_mut() {
                block.thinking = true;
            }
        }

        self.ai_ctrl
            .state
            .add_user_message("Summarize my recent terminal session.".to_string());

        let system = format!(
            "You are a terminal output summarizer. \
             The user's recent terminal session is provided below. \
             Produce a clear, concise summary of what happened: \
             which commands were run, whether they succeeded or failed, \
             and any important output. Use markdown formatting.\n\n{}",
            context_lines.join("\n")
        );
        let messages = ai::ollama::build_chat_messages(&system, &self.ai_ctrl.state.messages);

        let (token_tx, token_rx) = std::sync::mpsc::channel();
        let cancel = self.ai_ctrl.arm_cancel();
        let host = self.config.ai.ollama_host.clone();
        let model = self.config.ai.ollama_model.clone();
        let _jh = ai::ollama::stream_chat(
            &host,
            &model,
            messages,
            token_tx,
            self.proxy.clone(),
            cancel,
        );

        self.ai_ctrl.state.inference_rx = Some(token_rx);
        self.ai_ctrl.state.begin_assistant_message();
    }

    /// Check the last finished block for error patterns and, if a model is
    /// loaded (and not busy with a user query), start a background inference
    /// to suggest a fix command shown as ghost text in the input field.
    pub(crate) fn request_ai_hint_if_eligible(&mut self) {
        if self.ai_ctrl.state.inference_rx.is_some() || self.ai_ctrl.state.hint_rx.is_some() {
            return;
        }

        let (command, output_text) = {
            let block = match self.active_block_list().and_then(|bl| bl.blocks.last()) {
                Some(b) => b,
                None => return,
            };

            if block.command.starts_with('/') {
                return;
            }

            if block.output.is_empty() {
                return;
            }

            let output_text: String = block
                .output
                .iter()
                .take(20)
                .map(blocks::styled_line_text)
                .collect::<Vec<_>>()
                .join("\n");

            (block.command.clone(), output_text)
        };

        let web_search_enabled = self.settings_state.web_search_enabled;
        let (token_tx, token_rx) = std::sync::mpsc::channel();
        let proxy = self.proxy.clone();
        let cancel = self.ai_ctrl.arm_cancel();

        self.ai_ctrl.state.hint_rx = Some(token_rx);
        self.ai_ctrl.state.hint_buffer.clear();

        if self.config.ai.ollama_enabled && !self.config.ai.ollama_model.is_empty() {
            let host = self.config.ai.ollama_host.clone();
            let model_name = self.config.ai.ollama_model.clone();

            tokio::task::spawn_blocking(move || {
                let web_context = if web_search_enabled {
                    let first_line = output_text.lines().next().unwrap_or(&output_text);
                    let program = command.split_whitespace().next().unwrap_or(&command);
                    let query = format!("{program} {first_line}");
                    ai::web_search::search(&query, 3)
                } else {
                    None
                };

                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = token_tx.send(String::new());
                    let _ = proxy.send_event(TerminalEvent::Wakeup);
                    return;
                }

                let os_info = ai::detect_os_info();
                let system = ai::hint_system_prompt(&os_info);
                let mut user_text = format!("Command: {command}\nOutput:\n{output_text}");
                if let Some(ctx) = web_context.as_deref() {
                    user_text.push_str("\n\nWeb search results:\n");
                    user_text.push_str(ctx);
                }
                user_text.push_str("\nAnswer:");

                let msgs = vec![
                    ai::ollama::ChatMsg {
                        role: "system".into(),
                        content: system,
                    },
                    ai::ollama::ChatMsg {
                        role: "user".into(),
                        content: user_text,
                    },
                ];
                ai::ollama::do_stream_chat_pub(&host, &model_name, &msgs, token_tx, proxy, cancel);
            });
        } else {
            let model_handle = match self.ai_ctrl.state.loaded_model.take() {
                Some(h) => h,
                None => {
                    self.ai_ctrl.state.hint_rx = None;
                    return;
                }
            };

            let fallback_template = self.ai_ctrl.fallback_template().to_string();

            let jh = tokio::task::spawn_blocking(move || {
                let web_context = if web_search_enabled {
                    let first_line = output_text.lines().next().unwrap_or(&output_text);
                    let program = command.split_whitespace().next().unwrap_or(&command);
                    let query = format!("{program} {first_line}");
                    log::info!("web search query: {:?}", query);
                    let result = ai::web_search::search(&query, 3);
                    if let Some(ref ctx) = result {
                        log::info!("web search result: {:?}", ctx);
                    } else {
                        log::info!("web search: no results");
                    }
                    result
                } else {
                    None
                };

                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = token_tx.send(String::new());
                    let _ = proxy.send_event(TerminalEvent::Wakeup);
                    return None;
                }

                let prompt = ai::AiState::build_hint_prompt_static(
                    &command,
                    &output_text,
                    web_context.as_deref(),
                    &model_handle.model,
                    &fallback_template,
                );

                log::info!("command: {:?}", command);
                log::info!("prompt sent to model:\n{}", prompt);

                ai::inference::do_inference_pub(model_handle, prompt, token_tx, proxy, cancel)
            });

            self.ai_ctrl.state.hint_handle = Some(jh);
        }

        log::info!("hint inference started (web search + inference in background)");
    }
}

/// Clean AI output for commit messages: strip code blocks, backticks, quotes.
fn strip_commit_artifacts(raw: &str) -> String {
    let mut result = String::new();
    let mut in_code_block = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        let cleaned = trimmed
            .trim_start_matches('>')
            .trim_start_matches('\"')
            .trim_end_matches('\"')
            .trim_start_matches('`')
            .trim_end_matches('`')
            .trim();
        if !cleaned.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(cleaned);
        }
    }
    result.lines().next().unwrap_or("").to_string()
}
