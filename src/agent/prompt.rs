//! Agent system prompt construction.
//!
//! Builds the system prompt that instructs the AI model to behave as a
//! terminal agent with access to specific tools.  The prompt format uses
//! XML tags for tool invocation — this is more reliable with local models
//! than JSON function-calling, as XML tags are less likely to be
//! malformed during generation.

use super::tools::ToolRegistry;

/// Build the system prompt for agent mode.
///
/// The prompt describes the agent's role, lists available tools with
/// their parameter schemas, and defines the XML response format.
pub fn build_agent_system_prompt(
    tools: &ToolRegistry,
    os_info: &str,
    shell: &str,
    cwd: &str,
) -> String {
    let mut prompt = String::with_capacity(3072);

    prompt.push_str(
        "You are a terminal agent. You accomplish tasks by calling tools one at a time.\n\n",
    );

    prompt.push_str(&format!(
        "ENVIRONMENT: OS={os_info}, Shell={shell}, CWD={cwd}\n\n",
    ));

    prompt.push_str("TOOLS:\n");
    for spec in tools.specs() {
        prompt.push_str(&format!("  {} — {}\n", spec.name, spec.description));
        for p in &spec.parameters {
            let req = if p.required { "required" } else { "optional" };
            prompt.push_str(&format!("    {}: {} ({})\n", p.name, p.description, req));
        }
    }
    prompt.push('\n');

    prompt.push_str("OUTPUT FORMAT:\n\
Every response MUST contain exactly one of these two XML blocks.\n\
No other format is accepted. Do not output JSON. Do not use code fences for tool calls.\n\n\
To call a tool — write a short reason, then:\n\
<tool_call>\n\
<name>TOOL_NAME</name>\n\
<args>\n\
{\"param\": \"value\"}\n\
</args>\n\
</tool_call>\n\n\
To finish the task:\n\
<final_answer>\n\
Your answer here.\n\
</final_answer>\n\n\
RULES:\n\
1. Exactly one <tool_call> or one <final_answer> per response. Nothing else.\n\
2. You may write one short reasoning sentence before the XML block.\n\
3. After <tool_call>, STOP and wait for the tool result before continuing.\n\
4. When done, use <final_answer> with a concise summary of what happened.\n\
5. If a tool fails, analyze the error and try a different approach.\n\
6. Prefer simple, non-destructive commands.\n\
7. ALWAYS provide ALL required parameters. Never send empty args like {}.\n\
   For shell_exec you MUST include {\"command\": \"...\"}.\n\n\
EXAMPLE:\n\n\
User: What is the largest directory here?\n\n\
Assistant: I will check directory sizes.\n\
<tool_call>\n\
<name>shell_exec</name>\n\
<args>\n\
{\"command\": \"du -sh */ | sort -hr | head -5\"}\n\
</args>\n\
</tool_call>\n\n\
[Tool result: 15G target/  1.2M src/  ...]\n\n\
Assistant: The largest directory is target.\n\
<final_answer>\n\
The largest directory is target/ at 15 GB.\n\
</final_answer>\n");

    prompt
}

/// Format a tool result for injection back into the conversation.
pub fn format_tool_result(tool_name: &str, output: &str, is_error: bool) -> String {
    let status = if is_error { "ERROR" } else { "OK" };
    format!("[Tool Result: {tool_name} — {status}]\n{output}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_contains_tools() {
        let reg = ToolRegistry::with_defaults();
        let prompt = build_agent_system_prompt(&reg, "macOS 15", "zsh", "/home/user");
        assert!(prompt.contains("shell_exec"));
        assert!(prompt.contains("read_file"));
        assert!(prompt.contains("write_file"));
        assert!(prompt.contains("list_dir"));
        assert!(prompt.contains("<tool_call>"));
        assert!(prompt.contains("<final_answer>"));
    }

    #[test]
    fn system_prompt_contains_env_info() {
        let reg = ToolRegistry::with_defaults();
        let prompt = build_agent_system_prompt(&reg, "Ubuntu 24.04", "bash", "/root");
        assert!(prompt.contains("Ubuntu 24.04"));
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("/root"));
    }

    #[test]
    fn format_tool_result_ok() {
        let s = format_tool_result("shell_exec", "hello world", false);
        assert!(s.contains("OK"));
        assert!(s.contains("hello world"));
    }

    #[test]
    fn format_tool_result_error() {
        let s = format_tool_result("shell_exec", "command not found", true);
        assert!(s.contains("ERROR"));
        assert!(s.contains("command not found"));
    }
}
