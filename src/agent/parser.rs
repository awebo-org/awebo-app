//! Streaming XML parser for agent model responses.
//!
//! The model is instructed to emit tool calls inside `<tool_call>` tags
//! and final answers inside `<final_answer>` tags.  Any text *before*
//! these tags is treated as "thinking" (visible to the user in real-time).
//!
//! The parser is designed for **incremental** use: tokens are appended
//! one-by-one and the parser yields structured results once complete
//! tags are detected.

use std::collections::HashMap;

/// A parsed tool-call request extracted from the model output.
#[derive(Debug, Clone)]
pub struct ToolCallRequest {
    /// Name of the tool to invoke (must match a registered tool).
    pub tool_name: String,
    /// Arguments as key-value pairs (parsed from JSON inside `<args>`).
    pub args: HashMap<String, serde_json::Value>,
    /// Free-form text the model produced *before* the `<tool_call>` tag.
    pub reasoning: String,
}

/// Structured representation of a parsed model response.
#[derive(Debug, Clone)]
pub enum ParsedResponse {
    /// The model wants to call a tool.
    ToolCall(ToolCallRequest),
    /// The model produced a final answer (task complete).
    FinalAnswer { reasoning: String, answer: String },
    /// The model is still "thinking" — no structured tag detected yet.
    Thinking(String),
}

const TAG_TOOL_CALL_OPEN: &str = "<tool_call>";
const TAG_TOOL_CALL_CLOSE: &str = "</tool_call>";
const TAG_NAME_OPEN: &str = "<name>";
const TAG_NAME_CLOSE: &str = "</name>";
const TAG_ARGS_OPEN: &str = "<args>";
const TAG_ARGS_CLOSE: &str = "</args>";
const TAG_FINAL_OPEN: &str = "<final_answer>";
const TAG_FINAL_CLOSE: &str = "</final_answer>";

/// Parse a complete model response into a structured result.
///
/// Call this once the full response is available (after inference
/// finishes or after the closing tag is detected in the token stream).
pub fn parse_response(text: &str) -> ParsedResponse {
    if let Some(tc) = try_parse_tool_call(text) {
        return ParsedResponse::ToolCall(tc);
    }

    if let Some((reasoning, answer)) = try_parse_final_answer(text) {
        return ParsedResponse::FinalAnswer { reasoning, answer };
    }

    ParsedResponse::Thinking(text.to_string())
}

/// Check whether the accumulated text contains a complete `<tool_call>` block.
#[cfg(test)]
pub fn has_complete_tool_call(text: &str) -> bool {
    text.contains(TAG_TOOL_CALL_OPEN) && text.contains(TAG_TOOL_CALL_CLOSE)
}

/// Check whether the accumulated text contains a complete `<final_answer>` block.
#[cfg(test)]
pub fn has_complete_final_answer(text: &str) -> bool {
    text.contains(TAG_FINAL_OPEN) && text.contains(TAG_FINAL_CLOSE)
}

/// Check whether a structured block (tool_call or final_answer) is complete.
#[cfg(test)]
pub fn has_complete_block(text: &str) -> bool {
    has_complete_tool_call(text) || has_complete_final_answer(text)
}

fn try_parse_tool_call(text: &str) -> Option<ToolCallRequest> {
    let tc_start = text.find(TAG_TOOL_CALL_OPEN)?;
    let tc_end = text.find(TAG_TOOL_CALL_CLOSE)?;
    if tc_end <= tc_start {
        return None;
    }

    let reasoning = text[..tc_start].trim().to_string();
    let inner = &text[tc_start + TAG_TOOL_CALL_OPEN.len()..tc_end];

    let tool_name = extract_tag(inner, TAG_NAME_OPEN, TAG_NAME_CLOSE)?
        .trim()
        .to_string();

    let args_str = extract_tag(inner, TAG_ARGS_OPEN, TAG_ARGS_CLOSE).unwrap_or_default();
    let args = parse_json_args(args_str.trim());

    Some(ToolCallRequest {
        tool_name,
        args,
        reasoning,
    })
}

fn try_parse_final_answer(text: &str) -> Option<(String, String)> {
    let fa_start = text.find(TAG_FINAL_OPEN)?;
    let fa_end = text.find(TAG_FINAL_CLOSE)?;
    if fa_end <= fa_start {
        return None;
    }

    let reasoning = text[..fa_start].trim().to_string();
    let answer = text[fa_start + TAG_FINAL_OPEN.len()..fa_end]
        .trim()
        .to_string();

    Some((reasoning, answer))
}

fn extract_tag<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text.find(close)?;
    if end <= start {
        return None;
    }
    Some(&text[start..end])
}

/// Best-effort JSON → HashMap parse.  Falls back to a single
/// `"input"` key if the JSON is a bare string.
fn parse_json_args(s: &str) -> HashMap<String, serde_json::Value> {
    if s.is_empty() {
        return HashMap::new();
    }
    if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(s) {
        return map.into_iter().collect();
    }
    if let Ok(serde_json::Value::String(v)) = serde_json::from_str(s) {
        let mut m = HashMap::new();
        m.insert("input".into(), serde_json::Value::String(v));
        return m;
    }
    let mut m = HashMap::new();
    m.insert("input".into(), serde_json::Value::String(s.to_string()));
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tool_call_basic() {
        let input = r#"I need to check the hostname.

<tool_call>
<name>shell_exec</name>
<args>
{"command": "hostname"}
</args>
</tool_call>"#;

        match parse_response(input) {
            ParsedResponse::ToolCall(tc) => {
                assert_eq!(tc.tool_name, "shell_exec");
                assert_eq!(tc.args.get("command").unwrap(), "hostname");
                assert!(tc.reasoning.contains("hostname"));
            }
            other => panic!("expected ToolCall, got {:?}", other),
        }
    }

    #[test]
    fn parse_final_answer() {
        let input = r#"The task is done.

<final_answer>
System updated successfully. Nginx restarted.
</final_answer>"#;

        match parse_response(input) {
            ParsedResponse::FinalAnswer { reasoning, answer } => {
                assert!(reasoning.contains("done"));
                assert!(answer.contains("Nginx"));
            }
            other => panic!("expected FinalAnswer, got {:?}", other),
        }
    }

    #[test]
    fn parse_thinking_only() {
        let input = "Let me think about this...";
        match parse_response(input) {
            ParsedResponse::Thinking(t) => assert!(t.contains("think")),
            other => panic!("expected Thinking, got {:?}", other),
        }
    }

    #[test]
    fn has_complete_block_checks() {
        assert!(!has_complete_block("just some text"));
        assert!(!has_complete_block("<tool_call>partial"));
        assert!(has_complete_block(
            "<tool_call><name>x</name><args>{}</args></tool_call>"
        ));
        assert!(has_complete_block("<final_answer>done</final_answer>"));
    }

    #[test]
    fn parse_tool_call_with_multiple_args() {
        let input = r#"
<tool_call>
<name>write_file</name>
<args>
{"path": "/tmp/test.txt", "content": "hello world"}
</args>
</tool_call>"#;

        match parse_response(input) {
            ParsedResponse::ToolCall(tc) => {
                assert_eq!(tc.tool_name, "write_file");
                assert_eq!(tc.args.get("path").unwrap(), "/tmp/test.txt");
                assert_eq!(tc.args.get("content").unwrap(), "hello world");
            }
            other => panic!("expected ToolCall, got {:?}", other),
        }
    }

    #[test]
    fn parse_empty_args() {
        let input = "<tool_call><name>list_dir</name><args></args></tool_call>";
        match parse_response(input) {
            ParsedResponse::ToolCall(tc) => {
                assert_eq!(tc.tool_name, "list_dir");
                assert!(tc.args.is_empty());
            }
            other => panic!("expected ToolCall, got {:?}", other),
        }
    }

    #[test]
    fn reasoning_is_extracted_before_tags() {
        let input = "First I'll check the disk.\nThen plan cleanup.\n<tool_call><name>shell_exec</name><args>{\"command\": \"df -h\"}</args></tool_call>";
        match parse_response(input) {
            ParsedResponse::ToolCall(tc) => {
                assert!(tc.reasoning.contains("disk"));
                assert!(tc.reasoning.contains("cleanup"));
            }
            other => panic!("expected ToolCall, got {:?}", other),
        }
    }

    #[test]
    fn tool_call_takes_precedence_over_final_answer() {
        let input = "<tool_call><name>shell_exec</name><args>{\"command\":\"ls\"}</args></tool_call><final_answer>done</final_answer>";
        assert!(matches!(parse_response(input), ParsedResponse::ToolCall(_)));
    }

    #[test]
    fn non_xml_response_is_thinking() {
        let input = r#"{"cmd":["bash","-lc","ls"]}"#;
        assert!(matches!(parse_response(input), ParsedResponse::Thinking(_)));
    }
}
