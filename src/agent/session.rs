//! Agent session state — tracks the full conversation between the user,
//! the AI, and the tools across a single agent task.
//!
//! Also contains the [`ApprovalDecision`] type and approval-policy helpers.

use std::collections::HashSet;

use super::parser::ToolCallRequest;
use super::tools::ToolResult;

/// A single step in the agent session timeline.
#[derive(Debug, Clone)]
pub enum AgentStep {
    /// The initial task from the user.
    UserTask(String),
    /// Free-form reasoning produced by the model (text before a tag).
    Thinking(String),
    /// The model requested a tool call (awaiting or already approved).
    ToolRequest(ToolCallRequest),
    /// The result of an executed tool.
    ToolResult {
        tool_name: String,
        result: ToolResult,
    },
    /// The model declared the task complete.
    FinalAnswer,
    /// An error occurred (malformed output, inference failure, etc.).
    Error(String),
}

/// Current state of the agent session (finite state machine).
#[derive(Debug, Clone)]
pub enum AgentStatus {
    /// Model inference is running (tokens streaming).
    Thinking,
    /// Waiting for the user to approve/reject a tool call.
    AwaitingApproval(ToolCallRequest),
    /// A tool is currently executing.
    Executing,
    /// The agent finished the task.
    Completed,
    /// The user cancelled the session.
    Cancelled,
}

/// The user's decision for a pending tool-call approval.
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalDecision {
    /// Run this specific command.
    ApproveOnce,
    /// Auto-approve this tool type for the rest of the session.
    ApproveToolForSession,
    /// Reject — optionally provide an alternative message to the agent.
    Reject { user_message: Option<String> },
}

/// Check whether a tool still requires explicit approval.
pub fn needs_approval(session: &AgentSession, tool_name: &str) -> bool {
    !session.auto_approved_tools.contains(tool_name)
}

/// Token usage stats for a single inference round.
#[derive(Debug, Clone, Copy, Default)]
pub struct InferenceStats {
    pub prompt_tokens: usize,
    pub generated_tokens: usize,
}

/// Full state of one agent task execution.
pub struct AgentSession {
    /// Ordered list of steps (conversation log).
    pub steps: Vec<AgentStep>,
    /// Current status.
    pub status: AgentStatus,
    /// Tool names the user has auto-approved for this session.
    pub auto_approved_tools: HashSet<String>,
    /// Context window size of the loaded model (in tokens).
    pub context_size: u32,
    /// Cumulative token stats across all inference rounds.
    pub total_prompt_tokens: usize,
    pub total_generated_tokens: usize,
    /// Number of inference rounds completed.
    pub inference_rounds: usize,
    /// Last inference round stats.
    pub last_stats: InferenceStats,
    /// Whether a compaction (summarization) has been applied.
    pub compacted: bool,
}

impl AgentSession {
    /// Create a new session for the given task.
    pub fn new(task: String) -> Self {
        let mut session = Self {
            steps: Vec::new(),
            status: AgentStatus::Thinking,
            auto_approved_tools: HashSet::new(),
            context_size: 0,
            total_prompt_tokens: 0,
            total_generated_tokens: 0,
            inference_rounds: 0,
            last_stats: InferenceStats::default(),
            compacted: false,
        };
        session.steps.push(AgentStep::UserTask(task));
        session
    }

    /// Add a thinking step.
    pub fn push_thinking(&mut self, text: String) {
        if !text.is_empty() {
            self.steps.push(AgentStep::Thinking(text));
        }
    }

    /// Record a tool request and transition to AwaitingApproval.
    pub fn push_tool_request(&mut self, request: ToolCallRequest) {
        self.status = AgentStatus::AwaitingApproval(request.clone());
        self.steps.push(AgentStep::ToolRequest(request));
    }

    /// Record a tool result and transition back to Thinking.
    pub fn push_tool_result(&mut self, tool_name: String, result: ToolResult) {
        self.steps.push(AgentStep::ToolResult { tool_name, result });
        self.status = AgentStatus::Thinking;
    }

    /// Record a final answer and mark the session as completed.
    pub fn push_final_answer(&mut self) {
        self.steps.push(AgentStep::FinalAnswer);
        self.status = AgentStatus::Completed;
    }

    /// Record an error.
    pub fn push_error(&mut self, msg: String) {
        self.steps.push(AgentStep::Error(msg));
    }

    /// Record token stats from a completed inference round.
    pub fn record_inference_stats(&mut self, prompt_tokens: usize, generated_tokens: usize) {
        self.total_prompt_tokens += prompt_tokens;
        self.total_generated_tokens += generated_tokens;
        self.inference_rounds += 1;
        self.last_stats = InferenceStats { prompt_tokens, generated_tokens };
    }

    /// Fraction of context window used (0.0–1.0). Returns 0.0 if context_size unknown.
    pub fn context_usage_fraction(&self) -> f64 {
        if self.context_size == 0 { return 0.0; }
        let last_total = self.last_stats.prompt_tokens + self.last_stats.generated_tokens;
        last_total as f64 / self.context_size as f64
    }

    /// Returns true if context usage is at or above 75%.
    pub fn needs_compaction(&self) -> bool {
        !self.compacted && self.context_usage_fraction() >= 0.75
    }

    /// Format a human-readable token usage string.
    pub fn token_usage_display(&self) -> String {
        if self.context_size > 0 {
            let pct = (self.context_usage_fraction() * 100.0) as u32;
            format!(
                "{} tokens ({}% of {}k context)",
                self.last_stats.prompt_tokens + self.last_stats.generated_tokens,
                pct,
                self.context_size / 1024,
            )
        } else {
            format!(
                "{} prompt + {} generated tokens",
                self.last_stats.prompt_tokens, self.last_stats.generated_tokens,
            )
        }
    }

    /// Mark session as cancelled.
    pub fn cancel(&mut self) {
        self.status = AgentStatus::Cancelled;
    }

    /// Auto-approve a tool type for the rest of this session.
    pub fn auto_approve_tool(&mut self, tool_name: String) {
        self.auto_approved_tools.insert(tool_name);
    }

    /// Whether the session has finished (completed or cancelled).
    #[cfg(test)]
    pub fn is_finished(&self) -> bool {
        matches!(self.status, AgentStatus::Completed | AgentStatus::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn dummy_request() -> ToolCallRequest {
        ToolCallRequest {
            tool_name: "shell_exec".into(),
            args: HashMap::new(),
            reasoning: "testing".into(),
        }
    }

    #[test]
    fn new_session_starts_with_user_task() {
        let s = AgentSession::new("update system".into());
        assert_eq!(s.steps.len(), 1);
        assert!(matches!(&s.steps[0], AgentStep::UserTask(t) if t == "update system"));
        assert!(matches!(s.status, AgentStatus::Thinking));
    }

    #[test]
    fn push_tool_request_transitions_to_awaiting() {
        let mut s = AgentSession::new("task".into());
        s.push_tool_request(dummy_request());
        assert!(matches!(s.status, AgentStatus::AwaitingApproval(_)));
        assert_eq!(s.steps.len(), 2);
    }

    #[test]
    fn push_tool_result_transitions_back_to_thinking() {
        let mut s = AgentSession::new("task".into());
        s.push_tool_request(dummy_request());
        s.push_tool_result(
            "shell_exec".into(),
            ToolResult { output: "ok".into(), is_error: false },
        );
        assert!(matches!(s.status, AgentStatus::Thinking));
    }

    #[test]
    fn push_final_answer_completes() {
        let mut s = AgentSession::new("task".into());
        s.push_final_answer();
        assert!(s.is_finished());
        assert!(matches!(s.status, AgentStatus::Completed));
    }

    #[test]
    fn cancel_session() {
        let mut s = AgentSession::new("task".into());
        s.cancel();
        assert!(s.is_finished());
        assert!(matches!(s.status, AgentStatus::Cancelled));
    }

    #[test]
    fn auto_approve_tool() {
        let mut s = AgentSession::new("task".into());
        assert!(needs_approval(&s, "shell_exec"));
        s.auto_approve_tool("shell_exec".into());
        assert!(!needs_approval(&s, "shell_exec"));
        assert!(needs_approval(&s, "read_file"));
    }

    #[test]
    fn empty_thinking_not_pushed() {
        let mut s = AgentSession::new("task".into());
        s.push_thinking(String::new());
        assert_eq!(s.steps.len(), 1); // only UserTask
    }
}
