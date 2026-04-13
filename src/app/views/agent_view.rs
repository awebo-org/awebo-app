//! Agent integration — connects the agent orchestrator to the App.
//!
//! Provides `start_agent`, `handle_agent_approval`, `cancel_agent`,
//! and `poll_agent_tokens` methods on `App`.

use crate::agent::orchestrator::{AgentNext, AgentOrchestrator};
use crate::agent::session::ApprovalDecision;
use crate::agent::tools::ToolRegistry;

impl super::super::App {
    /// Start an agent session from `/agent <task>`.
    pub(crate) fn start_agent(&mut self, task: String) {
        if self.ai_ctrl.state.loaded_model.is_none() && self.ai_ctrl.state.loaded_model_name.is_none() {
            if !self.auto_load_best_efficient() {
                self.auto_download_default_model(&task);
                self.pending_agent_task = Some(task);
                return;
            }
        }

        if self.ai_ctrl.state.loaded_model.is_none() {
            let model_label = self.ai_ctrl.state.loaded_model_name
                .clone()
                .unwrap_or_else(|| "model".into());
            self.push_agent_loading_block(&task, &model_label);
            self.pending_agent_task = Some(task);
            return;
        }

        let cwd = self.current_cwd().unwrap_or_else(|| "/tmp".into());
        let os_info = std::env::consts::OS.to_string();
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());

        let tools = ToolRegistry::with_defaults();
        let agent_cmd = format!("/agent {task}");
        let (mut orch, next) = AgentOrchestrator::start(task.clone(), tools, os_info, shell, cwd);

        let ctx_size = self.ai_ctrl.state.context_size;
        orch.set_context_size(ctx_size);

        self.agent = Some(orch);
        self.agent_command = Some(agent_cmd);

        self.process_agent_next(next);
    }

    /// Handle user approval/rejection of a tool call.
    pub(crate) fn handle_agent_approval(&mut self, decision: ApprovalDecision) {
        let next = if let Some(orch) = &mut self.agent {
            orch.on_approval(decision)
        } else {
            return;
        };
        self.process_agent_next(next);
    }

    /// Cancel the active agent session.
    pub(crate) fn cancel_agent(&mut self) {
        if let Some(orch) = &mut self.agent {
            orch.cancel();
        }
        self.ai_ctrl.cancel_inference();
        self.push_agent_block("Agent session cancelled.");
        self.agent = None;
        self.agent_command = None;
    }

    /// Called from `poll_ai_tokens` when an agent inference completes.
    pub(crate) fn on_agent_inference_complete(&mut self, full_response: String) {
        let prompt_tokens = self.ai_ctrl.state.last_prompt_tokens;
        let generated_tokens = self.ai_ctrl.state.last_generated_tokens;

        if let Some(orch) = &mut self.agent {
            orch.record_stats(prompt_tokens, generated_tokens);

            if orch.needs_compaction() {
                log::info!(
                    "Agent context at {:.0}% — compacting conversation",
                    orch.session.context_usage_fraction() * 100.0,
                );
                orch.compact_conversation();
                self.push_agent_block("[context compacted — older steps summarized to free space]");
            }
        }

        let clean = Self::strip_agent_control_tokens(&full_response);
        log::info!("Agent inference complete (clean {} bytes): {:?}",
            clean.len(), &clean[..clean.len().min(200)]);

        let next = if let Some(orch) = &mut self.agent {
            orch.on_inference_complete(&clean)
        } else {
            return;
        };
        self.process_agent_next(next);
    }

    /// Strip model control tokens from agent responses.
    ///
    /// Removes patterns like `<|channel|>...<|message|>`, `<|end|>`,
    /// `<|start|>`, `<|im_start|>assistant`, `<|im_end|>` etc.
    fn strip_agent_control_tokens(text: &str) -> String {
        let mut result = text.to_string();

        while let Some(ch_start) = result.find("<|channel|>") {
            if let Some(msg_pos) = result[ch_start..].find("<|message|>") {
                let end = ch_start + msg_pos + "<|message|>".len();
                result.replace_range(ch_start..end, "");
            } else {
                result.truncate(ch_start);
                break;
            }
        }

        for token in &[
            "<|end|>", "<|start|>",
            "<|im_start|>assistant", "<|im_start|>", "<|im_end|>",
            "<|eot_id|>", "<|start_header_id|>assistant<|end_header_id|>",
        ] {
            while result.contains(token) {
                result = result.replace(token, "");
            }
        }

        result.trim().to_string()
    }

    /// Route an `AgentNext` instruction to the appropriate side effect.
    fn process_agent_next(&mut self, next: AgentNext) {
        match next {
            AgentNext::RunInference { system_prompt, user_messages } => {
                self.start_agent_inference(system_prompt, user_messages);
            }
            AgentNext::RequestApproval(request) => {
                let preview = match request.args.get("command") {
                    Some(serde_json::Value::String(cmd)) => cmd.clone(),
                    _ => {
                        serde_json::to_string_pretty(&request.args).unwrap_or_default()
                    }
                };
                let tool_name = request.tool_name.clone();
                self.push_agent_block_with_step(
                    &format!(
                        "Agent wants to call `{}`:\n```\n{}\n```",
                        tool_name, preview,
                    ),
                    Some(crate::blocks::AgentStepKind::ToolApproval {
                        tool_name: tool_name.clone(),
                        command_preview: preview.clone(),
                        selected_option: 0,
                    }),
                );
            }
            AgentNext::ExecuteTool(request) => {
                let cwd = self.current_cwd().unwrap_or_else(|| "/tmp".into());

                let tool_exists = self.agent.as_ref()
                    .map(|o| o.tool_registry.get(&request.tool_name).is_some())
                    .unwrap_or(false);

                if !tool_exists {
                    let result = crate::agent::tools::ToolResult {
                        output: format!("Unknown tool: {}", request.tool_name),
                        is_error: true,
                    };
                    self.on_tool_complete(request, result);
                    return;
                }

                self.push_agent_block(&format!(
                    "Running `{}`…", request.tool_name,
                ));

                let proxy = self.proxy.clone();
                let args = request.args.clone();
                let tool_name = request.tool_name.clone();
                let req_clone = request.clone();
                tokio::task::spawn_blocking(move || {
                    let registry = crate::agent::tools::ToolRegistry::with_defaults();
                    let result = if let Some(tool) = registry.get(&tool_name) {
                        tool.execute(&args, &cwd)
                    } else {
                        crate::agent::tools::ToolResult {
                            output: format!("Unknown tool: {tool_name}"),
                            is_error: true,
                        }
                    };
                    let _ = proxy.send_event(crate::terminal::TerminalEvent::ToolComplete {
                        request: req_clone,
                        result,
                    });
                });
            }
            AgentNext::Done(answer) => {
                let token_info = self.agent.as_ref()
                    .map(|o| o.token_usage_display())
                    .unwrap_or_default();
                let rounds = self.agent.as_ref()
                    .map(|o| o.session.inference_rounds)
                    .unwrap_or(0);
                self.push_agent_block_with_step(
                    &format!(
                        "Agent complete:\n\n{answer}\n\n---\n{rounds} rounds, {token_info}",
                    ),
                    Some(crate::blocks::AgentStepKind::FinalAnswer),
                );
                self.agent = None;
                self.agent_command = None;
            }
            AgentNext::Error(msg) => {
                self.push_agent_error(&msg);
                self.agent = None;
                self.agent_command = None;
            }
        }
    }

    /// Handle the result of a tool that executed on a background thread.
    ///
    /// Called from `user_event(TerminalEvent::ToolComplete {.. })`.
    pub(crate) fn on_tool_complete(
        &mut self,
        request: crate::agent::parser::ToolCallRequest,
        result: crate::agent::tools::ToolResult,
    ) {
        let is_error = result.is_error;
        let output_preview = if result.output.len() > 500 {
            format!("{}...\n(truncated)", &result.output[..500])
        } else {
            result.output.clone()
        };

        let icon = if is_error { "[error]" } else { "[ok]" };
        let tool_name = request.tool_name.clone();
        let token_info = self.agent.as_ref()
            .map(|o| o.token_usage_display())
            .unwrap_or_default();
        self.push_agent_block_with_step(
            &format!(
                "{icon} `{}` result:\n```\n{}\n```\n{token_info}",
                tool_name, output_preview,
            ),
            Some(crate::blocks::AgentStepKind::ToolResult {
                tool_name: tool_name.clone(),
                is_error,
            }),
        );

        let next = if let Some(orch) = &mut self.agent {
            orch.on_tool_result(tool_name, result)
        } else {
            return;
        };
        self.process_agent_next(next);
    }

    /// Start an inference round for the agent (reuses existing AI pipeline).
    fn start_agent_inference(
        &mut self,
        system_prompt: String,
        messages: Vec<crate::agent::orchestrator::AgentConvMessage>,
    ) {
        use crate::agent::orchestrator::AgentConvRole;

        let handle = match self.ai_ctrl.state.loaded_model.take() {
            Some(h) => h,
            None => {
                self.push_agent_error("Model was unloaded during agent session.");
                self.agent = None;
                self.agent_command = None;
                return;
            }
        };

        let fallback_template = self.ai_ctrl.fallback_template();

        let msg_tuples: Vec<(&str, String)> = messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    AgentConvRole::User => "user",
                    AgentConvRole::Assistant => "assistant",
                    AgentConvRole::Tool => "user",
                };
                (role, m.content.clone())
            })
            .collect();
        let msg_refs: Vec<(&str, &str)> = msg_tuples
            .iter()
            .map(|(r, c)| (*r, c.as_str()))
            .collect();

        let prompt = self.ai_ctrl.state.build_custom_prompt(
            &system_prompt,
            &msg_refs,
            &handle.model,
            &fallback_template,
        );

        self.push_agent_thinking_block();

        self.ai_ctrl.state.begin_assistant_message();

        let proxy = self.proxy.clone();
        let cancel = self.ai_ctrl.arm_cancel();
        let (tx, rx) = std::sync::mpsc::channel();

        let inference_handle = crate::ai::inference::run_inference(handle, prompt, tx, proxy, cancel);

        self.ai_ctrl.state.inference_rx = Some(rx);
        self.ai_ctrl.state.inference_handle = Some(inference_handle);
        self.ai_ctrl.state.thinking_since = Some(std::time::Instant::now());
        self.ai_ctrl.block_written = 0;
    }

    fn agent_cmd_label(&self) -> String {
        self.agent_command.clone().unwrap_or_else(|| "/agent".to_string())
    }

    pub(crate) fn push_agent_block(&mut self, text: &str) {
        self.push_agent_block_with_step(text, None);
    }

    fn push_agent_block_with_step(&mut self, text: &str, step: Option<crate::blocks::AgentStepKind>) {
        let cmd = self.agent_cmd_label();
        let clean = Self::strip_agent_control_tokens(text);
        if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index()) {
            if let super::super::TabKind::Terminal { terminal, block_list, .. } = &mut tab.kind {
                let prompt_info = terminal.prompt_info();
                block_list.push_command(prompt_info, cmd);
                block_list.set_output_markdown(&clean);
                if let Some(s) = step {
                    block_list.set_last_agent_step(s);
                }
                block_list.finish_last();
            }
        }
        self.record_last_block();
    }

    fn push_agent_thinking_block(&mut self) {
        let cmd = self.agent_cmd_label();
        if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index()) {
            if let super::super::TabKind::Terminal { terminal, block_list, .. } = &mut tab.kind {
                let prompt_info = terminal.prompt_info();
                block_list.push_command(prompt_info, cmd);
                if let Some(block) = block_list.blocks.last_mut() {
                    block.thinking = true;
                }
                block_list.set_last_agent_step(crate::blocks::AgentStepKind::Thinking);
            }
        }
    }

    pub(crate) fn push_agent_error(&mut self, msg: &str) {
        self.push_agent_block(msg);
    }

    fn push_agent_loading_block(&mut self, task: &str, model_name: &str) {
        let cmd = format!("/agent {task}");
        if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index()) {
            if let super::super::TabKind::Terminal { terminal, block_list, .. } = &mut tab.kind {
                let prompt_info = terminal.prompt_info();
                block_list.push_command(prompt_info, cmd);
                block_list.append_output_text(&format!("Loading {model_name}…"));
            }
        }
    }

    /// Get the currently selected approval option index (0–2).
    pub(crate) fn agent_approval_selected(&self) -> usize {
        let tab = match self.tab_mgr.get(self.tab_mgr.active_index()) {
            Some(t) => t,
            None => return 0,
        };
        if let super::super::TabKind::Terminal { block_list, .. } = &tab.kind {
            if let Some(block) = block_list.blocks.last() {
                if let Some(crate::blocks::AgentStepKind::ToolApproval { selected_option, .. }) = &block.agent_step {
                    return *selected_option;
                }
            }
        }
        0
    }

    /// Cycle the approval selection by `delta` (positive = right, negative = left).
    pub(crate) fn cycle_agent_approval_selection(&mut self, delta: i32) {
        let tab = match self.tab_mgr.get_mut(self.tab_mgr.active_index()) {
            Some(t) => t,
            None => return,
        };
        if let super::super::TabKind::Terminal { block_list, .. } = &mut tab.kind {
            if let Some(block) = block_list.blocks.last_mut() {
                if let Some(crate::blocks::AgentStepKind::ToolApproval { selected_option, .. }) = &mut block.agent_step {
                    let cur = *selected_option as i32;
                    *selected_option = (cur + delta).rem_euclid(3) as usize;
                }
            }
            block_list.bump_generation();
        }
    }

    /// Set the approval selection to a specific index (0 = Approve, 1 = Always, 2 = Reject).
    pub(crate) fn set_agent_approval_selection(&mut self, index: usize) {
        let tab = match self.tab_mgr.get_mut(self.tab_mgr.active_index()) {
            Some(t) => t,
            None => return,
        };
        if let super::super::TabKind::Terminal { block_list, .. } = &mut tab.kind {
            if let Some(block) = block_list.blocks.last_mut() {
                if let Some(crate::blocks::AgentStepKind::ToolApproval { selected_option, .. }) = &mut block.agent_step {
                    *selected_option = index.min(2);
                }
            }
            block_list.bump_generation();
        }
    }

    fn current_cwd(&self) -> Option<String> {
        let tab = self.tab_mgr.get(self.tab_mgr.active_index())?;
        if let super::super::TabKind::Terminal { terminal, .. } = &tab.kind {
            let cwd = terminal.prompt_info().cwd()?;
            if cwd.starts_with('~') {
                if let Some(home) = dirs::home_dir() {
                    return Some(cwd.replacen('~', &home.to_string_lossy(), 1));
                }
            }
            Some(cwd)
        } else {
            None
        }
    }
}
