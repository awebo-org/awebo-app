//! Agent orchestrator — stateless coordination of the agent loop.
//!
//! The orchestrator does **not** perform side effects itself.  Instead
//! it receives events (inference complete, user approval, tool result)
//! and returns an [`AgentNext`] value describing what the application
//! should do next.

use super::parser::{self, ParsedResponse, ToolCallRequest};
use super::prompt;
use super::session::{self, AgentSession, AgentStatus, ApprovalDecision};
use super::tools::{ToolRegistry, ToolResult};

/// Instruction returned by the orchestrator after each event.
#[derive(Debug)]
pub enum AgentNext {
    /// Build this prompt and start a new inference round.
    RunInference {
        system_prompt: String,
        user_messages: Vec<AgentConvMessage>,
    },
    /// Show the approval box to the user for this tool call.
    RequestApproval(ToolCallRequest),
    /// Execute this tool (user already approved).
    ExecuteTool(ToolCallRequest),
    /// Agent completed — display the final answer.
    Done(String),
    /// An error occurred.
    Error(String),
}

/// A message in the agent conversation (for prompt building).
#[derive(Debug, Clone)]
pub struct AgentConvMessage {
    pub role: AgentConvRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentConvRole {
    User,
    Assistant,
    Tool,
}

/// Coordinates the agent loop without owning I/O.
pub struct AgentOrchestrator {
    pub session: AgentSession,
    pub tool_registry: ToolRegistry,
    os_info: String,
    shell: String,
    cwd: String,
    /// Consecutive parsing failures (no valid tag detected).
    consecutive_parse_failures: u32,
}

impl AgentOrchestrator {
    /// Start a new agent task.  Returns the first `AgentNext` to kick
    /// off inference.
    pub fn start(
        task: String,
        tool_registry: ToolRegistry,
        os_info: String,
        shell: String,
        cwd: String,
    ) -> (Self, AgentNext) {
        let session = AgentSession::new(task);

        let orch = Self {
            session,
            tool_registry,
            os_info,
            shell,
            cwd,
            consecutive_parse_failures: 0,
        };

        let next = orch.build_inference_request();
        (orch, next)
    }

    /// Called when an inference round finishes with the full model response.
    pub fn on_inference_complete(&mut self, response: &str) -> AgentNext {
        let parsed = parser::parse_response(response);

        match parsed {
            ParsedResponse::ToolCall(request) => {
                self.consecutive_parse_failures = 0;
                if !request.reasoning.is_empty() {
                    self.session.push_thinking(request.reasoning.clone());
                }

                if let Some(err) = self.validate_tool_args(&request) {
                    log::warn!("Agent: tool call validation failed: {err}");
                    self.session.push_tool_request(request.clone());
                    self.session.push_tool_result(
                        request.tool_name,
                        ToolResult {
                            output: err,
                            is_error: true,
                        },
                    );
                    return self.build_inference_request();
                }

                self.session.push_tool_request(request.clone());

                if session::needs_approval(&self.session, &request.tool_name) {
                    AgentNext::RequestApproval(request)
                } else {
                    self.session.status = AgentStatus::Executing;
                    AgentNext::ExecuteTool(request)
                }
            }
            ParsedResponse::FinalAnswer { reasoning, answer } => {
                self.consecutive_parse_failures = 0;
                if !reasoning.is_empty() {
                    self.session.push_thinking(reasoning);
                }
                self.session.push_final_answer();
                AgentNext::Done(answer)
            }
            ParsedResponse::Thinking(text) => {
                self.consecutive_parse_failures += 1;

                if self.consecutive_parse_failures >= 2 {
                    log::warn!(
                        "Agent: {} consecutive parse failures — treating response as final answer",
                        self.consecutive_parse_failures,
                    );
                    let answer = text.trim().to_string();
                    self.session.push_final_answer();
                    return AgentNext::Done(answer);
                }

                self.session.push_thinking(text.clone());
                self.session.push_error(
                    "Model did not produce a <tool_call> or <final_answer> tag.".into(),
                );
                self.build_inference_request()
            }
        }
    }

    /// Record token stats from a completed inference round.
    pub fn record_stats(&mut self, prompt_tokens: usize, generated_tokens: usize) {
        self.session
            .record_inference_stats(prompt_tokens, generated_tokens);
    }

    /// Set the model's context window size.
    pub fn set_context_size(&mut self, ctx: u32) {
        self.session.context_size = ctx;
    }

    /// Token usage display string for the current session.
    pub fn token_usage_display(&self) -> String {
        self.session.token_usage_display()
    }

    /// Whether the session needs compaction (context >= 75%).
    pub fn needs_compaction(&self) -> bool {
        self.session.needs_compaction()
    }

    /// Called when the user makes an approval decision.
    pub fn on_approval(&mut self, decision: ApprovalDecision) -> AgentNext {
        let request = match &self.session.status {
            AgentStatus::AwaitingApproval(req) => req.clone(),
            _ => {
                return AgentNext::Error("on_approval called but not awaiting approval".into());
            }
        };

        match decision {
            ApprovalDecision::ApproveOnce => {
                self.session.status = AgentStatus::Executing;
                AgentNext::ExecuteTool(request)
            }
            ApprovalDecision::ApproveToolForSession => {
                self.session.auto_approve_tool(request.tool_name.clone());
                self.session.status = AgentStatus::Executing;
                AgentNext::ExecuteTool(request)
            }
            ApprovalDecision::Reject { user_message } => {
                let msg = user_message.unwrap_or_else(|| {
                    "The user rejected this tool call. Try a different approach.".into()
                });
                self.session.push_tool_result(
                    request.tool_name,
                    ToolResult {
                        output: msg,
                        is_error: true,
                    },
                );
                self.build_inference_request()
            }
        }
    }

    /// Called after a tool has been executed.
    pub fn on_tool_result(&mut self, tool_name: String, result: ToolResult) -> AgentNext {
        self.session.push_tool_result(tool_name, result);
        self.build_inference_request()
    }

    /// Cancel the agent session.
    pub fn cancel(&mut self) {
        self.session.cancel();
    }

    /// Validate that all required parameters are present in a tool call.
    /// Returns `Some(error_message)` if validation fails, `None` if OK.
    fn validate_tool_args(&self, request: &parser::ToolCallRequest) -> Option<String> {
        let tool = self.tool_registry.get(&request.tool_name)?;
        let spec = tool.spec();
        let missing: Vec<&str> = spec
            .parameters
            .iter()
            .filter(|p| p.required)
            .filter(|p| {
                request
                    .args
                    .get(p.name)
                    .is_none_or(|v| v.as_str().is_some_and(str::is_empty))
            })
            .map(|p| p.name)
            .collect();

        if missing.is_empty() {
            None
        } else {
            Some(format!(
                "Error: missing required parameter(s): {}. Provide all required args.",
                missing.join(", "),
            ))
        }
    }

    fn build_inference_request(&self) -> AgentNext {
        let system_prompt = prompt::build_agent_system_prompt(
            &self.tool_registry,
            &self.os_info,
            &self.shell,
            &self.cwd,
        );

        let messages = self.build_conversation_messages();

        AgentNext::RunInference {
            system_prompt,
            user_messages: messages,
        }
    }

    /// Compact older conversation steps into a summary to free context.
    /// Keeps the UserTask and the last N steps, replacing everything in
    /// between with a single summary message.
    pub fn compact_conversation(&mut self) {
        use super::session::AgentStep;

        if self.session.steps.len() <= 4 {
            return;
        }

        let keep_tail = 3;
        let boundary = self.session.steps.len().saturating_sub(keep_tail);
        let old_len = self.session.steps.len();

        let mut summary_parts = Vec::new();
        for step in &self.session.steps[1..boundary] {
            match step {
                AgentStep::Thinking(text) => {
                    let brief = if text.len() > 100 {
                        &text[..100]
                    } else {
                        text.as_str()
                    };
                    summary_parts.push(format!("- Thought: {brief}..."));
                }
                AgentStep::ToolRequest(req) => {
                    let args_str = serde_json::to_string(&req.args).unwrap_or_default();
                    let brief = if args_str.len() > 80 {
                        &args_str[..80]
                    } else {
                        args_str.as_str()
                    };
                    summary_parts.push(format!("- Called {}: {brief}", req.tool_name));
                }
                AgentStep::ToolResult { tool_name, result } => {
                    let status = if result.is_error { "FAILED" } else { "OK" };
                    let brief = if result.output.len() > 120 {
                        &result.output[..120]
                    } else {
                        result.output.as_str()
                    };
                    summary_parts.push(format!("- {tool_name} {status}: {brief}"));
                }
                AgentStep::Error(msg) => {
                    summary_parts.push(format!("- Error: {msg}"));
                }
                AgentStep::UserTask(_) | AgentStep::FinalAnswer => {}
            }
        }

        if summary_parts.is_empty() {
            return;
        }

        let summary_text = format!(
            "[Context compacted — {} prior steps summarized]\n{}",
            boundary - 1,
            summary_parts.join("\n"),
        );

        let first = self.session.steps[0].clone();
        let tail: Vec<AgentStep> = self.session.steps[boundary..].to_vec();

        self.session.steps.clear();
        self.session.steps.push(first);
        self.session.steps.push(AgentStep::Thinking(summary_text));
        self.session.steps.extend(tail);
        self.session.compacted = true;

        log::info!(
            "Agent conversation compacted: {} steps -> {} steps",
            old_len,
            self.session.steps.len(),
        );
    }

    /// Convert session steps into a flat message list suitable for
    /// the chat template.
    fn build_conversation_messages(&self) -> Vec<AgentConvMessage> {
        use super::session::AgentStep;

        let mut msgs = Vec::new();

        for step in &self.session.steps {
            match step {
                AgentStep::UserTask(task) => {
                    msgs.push(AgentConvMessage {
                        role: AgentConvRole::User,
                        content: task.clone(),
                    });
                }
                AgentStep::Thinking(text) => {
                    if let Some(last) = msgs.last_mut()
                        && last.role == AgentConvRole::Assistant
                    {
                        last.content.push('\n');
                        last.content.push_str(text);
                        continue;
                    }
                    msgs.push(AgentConvMessage {
                        role: AgentConvRole::Assistant,
                        content: text.clone(),
                    });
                }
                AgentStep::ToolRequest(req) => {
                    let xml = format!(
                        "<tool_call>\n<name>{}</name>\n<args>\n{}\n</args>\n</tool_call>",
                        req.tool_name,
                        serde_json::to_string(&req.args).unwrap_or_default(),
                    );
                    if let Some(last) = msgs.last_mut()
                        && last.role == AgentConvRole::Assistant
                    {
                        last.content.push('\n');
                        last.content.push_str(&xml);
                        continue;
                    }
                    msgs.push(AgentConvMessage {
                        role: AgentConvRole::Assistant,
                        content: xml,
                    });
                }
                AgentStep::ToolResult { tool_name, result } => {
                    let formatted =
                        prompt::format_tool_result(tool_name, &result.output, result.is_error);
                    msgs.push(AgentConvMessage {
                        role: AgentConvRole::Tool,
                        content: formatted,
                    });
                }
                AgentStep::FinalAnswer => {}
                AgentStep::Error(msg) => {
                    msgs.push(AgentConvMessage {
                        role: AgentConvRole::Tool,
                        content: format!("[Error] {msg}"),
                    });
                }
            }
        }

        msgs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_orch() -> (AgentOrchestrator, AgentNext) {
        AgentOrchestrator::start(
            "update the system".into(),
            ToolRegistry::with_defaults(),
            "Linux".into(),
            "bash".into(),
            "/home/user".into(),
        )
    }

    #[test]
    fn start_returns_run_inference() {
        let (_orch, next) = make_orch();
        assert!(matches!(next, AgentNext::RunInference { .. }));
    }

    #[test]
    fn inference_with_tool_call_requests_approval() {
        let (mut orch, _) = make_orch();
        let response = r#"I'll check the hostname.

<tool_call>
<name>shell_exec</name>
<args>
{"command": "hostname"}
</args>
</tool_call>"#;

        let next = orch.on_inference_complete(response);
        assert!(matches!(next, AgentNext::RequestApproval(_)));
        assert!(matches!(
            orch.session.status,
            AgentStatus::AwaitingApproval(_)
        ));
    }

    #[test]
    fn inference_with_final_answer_completes() {
        let (mut orch, _) = make_orch();
        let response = "All done.\n\n<final_answer>\nSystem updated.\n</final_answer>";

        let next = orch.on_inference_complete(response);
        assert!(matches!(next, AgentNext::Done(_)));
        assert!(orch.session.is_finished());
    }

    #[test]
    fn approve_once_executes_tool() {
        let (mut orch, _) = make_orch();
        let response =
            "<tool_call><name>shell_exec</name><args>{\"command\":\"ls\"}</args></tool_call>";
        let _ = orch.on_inference_complete(response);

        let next = orch.on_approval(ApprovalDecision::ApproveOnce);
        assert!(matches!(next, AgentNext::ExecuteTool(_)));
    }

    #[test]
    fn approve_for_session_auto_approves_next() {
        let (mut orch, _) = make_orch();
        let response =
            "<tool_call><name>shell_exec</name><args>{\"command\":\"ls\"}</args></tool_call>";
        let next = orch.on_inference_complete(response);
        assert!(matches!(next, AgentNext::RequestApproval(_)));

        let next = orch.on_approval(ApprovalDecision::ApproveToolForSession);
        assert!(matches!(next, AgentNext::ExecuteTool(_)));

        let next = orch.on_tool_result(
            "shell_exec".into(),
            ToolResult {
                output: "file.txt".into(),
                is_error: false,
            },
        );
        assert!(matches!(next, AgentNext::RunInference { .. }));

        let response2 =
            "<tool_call><name>shell_exec</name><args>{\"command\":\"pwd\"}</args></tool_call>";
        let next = orch.on_inference_complete(response2);
        assert!(matches!(next, AgentNext::ExecuteTool(_)));
    }

    #[test]
    fn reject_sends_rejection_message() {
        let (mut orch, _) = make_orch();
        let response =
            "<tool_call><name>shell_exec</name><args>{\"command\":\"rm -rf /\"}</args></tool_call>";
        let _ = orch.on_inference_complete(response);

        let next = orch.on_approval(ApprovalDecision::Reject {
            user_message: Some("Don't do that!".into()),
        });
        assert!(matches!(next, AgentNext::RunInference { .. }));
    }

    #[test]
    fn cancel_marks_session_finished() {
        let (mut orch, _) = make_orch();
        orch.cancel();
        assert!(orch.session.is_finished());
    }

    #[test]
    fn no_tag_retries_inference() {
        let (mut orch, _) = make_orch();
        let response = "I'm not sure what to do...";
        let next = orch.on_inference_complete(response);
        assert!(matches!(next, AgentNext::RunInference { .. }));
    }

    #[test]
    fn conversation_messages_roundtrip() {
        let (mut orch, _) = make_orch();

        let r1 = "<tool_call><name>shell_exec</name><args>{\"command\":\"ls\"}</args></tool_call>";
        let _ = orch.on_inference_complete(r1);
        let _ = orch.on_approval(ApprovalDecision::ApproveOnce);
        let _ = orch.on_tool_result(
            "shell_exec".into(),
            ToolResult {
                output: "a.txt b.txt".into(),
                is_error: false,
            },
        );

        let msgs = orch.build_conversation_messages();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, AgentConvRole::User);
        assert_eq!(msgs[1].role, AgentConvRole::Assistant);
        assert_eq!(msgs[2].role, AgentConvRole::Tool);
    }
}
