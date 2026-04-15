use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use serde::{Deserialize, Serialize};
use winit::event_loop::EventLoopProxy;

use crate::terminal::TerminalEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
    pub parameter_size: String,
    pub family: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Option<Vec<TagsModel>>,
}

#[derive(Debug, Deserialize)]
struct TagsModel {
    name: String,
    size: u64,
    details: Option<TagsModelDetails>,
}

#[derive(Debug, Deserialize)]
struct TagsModelDetails {
    parameter_size: Option<String>,
    family: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMsg],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [OllamaTool]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    message: Option<ChatChunkMessage>,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct ChatChunkMessage {
    content: String,
    #[serde(default)]
    tool_calls: Vec<ChunkToolCall>,
}

#[derive(Debug, Deserialize)]
struct ChunkToolCall {
    function: ChunkToolFunction,
}

#[derive(Debug, Deserialize)]
struct ChunkToolFunction {
    name: String,
    arguments: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OllamaTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaToolFunction,
}

#[derive(Debug, Clone, Serialize)]
struct OllamaToolFunction {
    name: String,
    description: String,
    parameters: OllamaToolParams,
}

#[derive(Debug, Clone, Serialize)]
struct OllamaToolParams {
    #[serde(rename = "type")]
    param_type: String,
    properties: std::collections::HashMap<String, OllamaParamDef>,
    required: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct OllamaParamDef {
    #[serde(rename = "type")]
    param_type: String,
    description: String,
}

fn normalize_host(host: &str) -> String {
    let h = host.trim().trim_end_matches('/');
    if h.contains("://") {
        h.to_string()
    } else {
        format!("http://{h}")
    }
}

pub fn list_models(host: &str) -> Result<Vec<OllamaModel>, String> {
    let base = normalize_host(host);
    let url = format!("{base}/api/tags");

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(5)))
        .build()
        .new_agent();

    let resp = agent
        .get(&url)
        .call()
        .map_err(|e| format!("Connection failed: {e}"))?;

    let body_str = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Read error: {e}"))?;

    let body: TagsResponse =
        serde_json::from_str(&body_str).map_err(|e| format!("Invalid response: {e}"))?;

    let models = body.models.unwrap_or_default();
    Ok(models
        .into_iter()
        .map(|m| {
            let details = m.details.unwrap_or(TagsModelDetails {
                parameter_size: None,
                family: None,
            });
            OllamaModel {
                name: m.name,
                size: m.size,
                parameter_size: details.parameter_size.unwrap_or_default(),
                family: details.family.unwrap_or_default(),
            }
        })
        .collect())
}

pub fn stream_chat(
    host: &str,
    model: &str,
    messages: Vec<ChatMsg>,
    token_tx: mpsc::Sender<String>,
    proxy: EventLoopProxy<TerminalEvent>,
    cancel: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    let host = host.to_string();
    let model = model.to_string();

    tokio::task::spawn_blocking(move || {
        do_stream_chat(&host, &model, &messages, None, token_tx, proxy, cancel);
    })
}

pub fn stream_chat_with_tools(
    host: &str,
    model: &str,
    messages: Vec<ChatMsg>,
    tools: Vec<OllamaTool>,
    token_tx: mpsc::Sender<String>,
    proxy: EventLoopProxy<TerminalEvent>,
    cancel: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    let host = host.to_string();
    let model = model.to_string();

    tokio::task::spawn_blocking(move || {
        do_stream_chat(
            &host,
            &model,
            &messages,
            Some(&tools),
            token_tx,
            proxy,
            cancel,
        );
    })
}

pub fn do_stream_chat_pub(
    host: &str,
    model: &str,
    messages: &[ChatMsg],
    token_tx: mpsc::Sender<String>,
    proxy: EventLoopProxy<TerminalEvent>,
    cancel: Arc<AtomicBool>,
) {
    do_stream_chat(host, model, messages, None, token_tx, proxy, cancel);
}

fn do_stream_chat(
    host: &str,
    model: &str,
    messages: &[ChatMsg],
    tools: Option<&[OllamaTool]>,
    token_tx: mpsc::Sender<String>,
    proxy: EventLoopProxy<TerminalEvent>,
    cancel: Arc<AtomicBool>,
) {
    let base = normalize_host(host);
    let url = format!("{base}/api/chat");

    let body = ChatRequest {
        model,
        messages,
        stream: true,
        tools,
    };

    let body_json = match serde_json::to_string(&body) {
        Ok(j) => j,
        Err(e) => {
            log::error!("Ollama: failed to serialize request: {e}");
            let _ = proxy.send_event(TerminalEvent::AiError(format!("Serialize error: {e}")));
            let _ = token_tx.send(String::new());
            return;
        }
    };

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .build()
        .new_agent();

    let resp = match agent
        .post(&url)
        .header("Content-Type", "application/json")
        .send(body_json.as_bytes())
    {
        Ok(r) => r,
        Err(e) => {
            log::error!("Ollama: request failed: {e}");
            let _ = proxy.send_event(TerminalEvent::AiError(format!("Ollama error: {e}")));
            let _ = token_tx.send(String::new());
            return;
        }
    };

    let reader = std::io::BufReader::new(resp.into_body().into_reader());
    let mut generated = 0usize;
    let mut in_think = false;
    let mut buf = String::new();
    let mut tool_calls_collected: Vec<ChunkToolCall> = Vec::new();
    let mut full_content = String::new();

    for line_result in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            log::info!("Ollama: inference cancelled after {generated} tokens");
            break;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                log::error!("Ollama: read error: {e}");
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let chunk: ChatChunk = match serde_json::from_str(&line) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(msg) = &chunk.message {
            if !msg.tool_calls.is_empty() {
                for tc in &msg.tool_calls {
                    tool_calls_collected.push(ChunkToolCall {
                        function: ChunkToolFunction {
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                        },
                    });
                }
            }

            let piece = &msg.content;
            if !piece.is_empty() {
                generated += 1;
                buf.push_str(piece);

                loop {
                    if in_think {
                        if let Some(end) = buf.find("</think>") {
                            buf = buf[end + 8..].to_string();
                            in_think = false;
                            continue;
                        }
                        buf.clear();
                        break;
                    }

                    if let Some(start) = buf.find("<think>") {
                        let before = &buf[..start];
                        if !before.is_empty() {
                            full_content.push_str(before);
                            if token_tx.send(before.to_string()).is_err() {
                                let _ = token_tx.send(String::new());
                                let _ = proxy.send_event(TerminalEvent::Wakeup);
                                return;
                            }
                            let _ = proxy.send_event(TerminalEvent::Wakeup);
                        }
                        buf = buf[start + 7..].to_string();
                        in_think = true;
                        continue;
                    }

                    if buf.len() > 10 {
                        let target = buf.len() - 10;
                        let safe = buf.floor_char_boundary(target);
                        if safe > 0 {
                            let to_send = buf[..safe].to_string();
                            buf = buf[safe..].to_string();
                            full_content.push_str(&to_send);
                            if token_tx.send(to_send).is_err() {
                                let _ = token_tx.send(String::new());
                                let _ = proxy.send_event(TerminalEvent::Wakeup);
                                return;
                            }
                            let _ = proxy.send_event(TerminalEvent::Wakeup);
                        }
                    }
                    break;
                }
            }
        }

        if chunk.done {
            break;
        }
    }

    if !tool_calls_collected.is_empty() {
        let reasoning = if !in_think {
            std::mem::take(&mut buf)
        } else {
            String::new()
        };
        let tc = &tool_calls_collected[0];
        let args_json =
            serde_json::to_string(&tc.function.arguments).unwrap_or_else(|_| "{}".into());
        let xml = format!(
            "{reasoning}\n<tool_call>\n<name>{}</name>\n<args>\n{args_json}\n</args>\n</tool_call>",
            tc.function.name,
        );
        let _ = token_tx.send(xml);
    } else if !in_think && !buf.is_empty() {
        full_content.push_str(&buf);
        let has_xml_tags =
            full_content.contains("<tool_call>") || full_content.contains("<final_answer>");
        if tools.is_some() && !has_xml_tags {
            let xml = format!("<final_answer>\n{buf}\n</final_answer>");
            let _ = token_tx.send(xml);
        } else {
            let _ = token_tx.send(buf);
        }
    } else if tools.is_some()
        && buf.is_empty()
        && tool_calls_collected.is_empty()
        && !full_content.contains("<tool_call>")
        && !full_content.contains("<final_answer>")
    {
        let _ = token_tx.send("<final_answer>\nNo response from model.\n</final_answer>".into());
    }

    let _ = token_tx.send(String::new());
    let _ = proxy.send_event(TerminalEvent::Wakeup);
}

pub fn build_chat_messages(
    system_prompt: &str,
    messages: &[crate::ai::ChatMessage],
) -> Vec<ChatMsg> {
    let mut out = Vec::with_capacity(messages.len() + 1);
    out.push(ChatMsg {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    });
    for msg in messages {
        let role = match msg.role {
            crate::ai::MessageRole::User => "user",
            crate::ai::MessageRole::Assistant => "assistant",
        };
        if !msg.content.is_empty() {
            out.push(ChatMsg {
                role: role.to_string(),
                content: msg.content.clone(),
            });
        }
    }
    out
}

pub fn build_agent_messages(system_prompt: &str, messages: &[(&str, &str)]) -> Vec<ChatMsg> {
    let mut out = Vec::with_capacity(messages.len() + 1);
    out.push(ChatMsg {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    });
    for (role, content) in messages {
        out.push(ChatMsg {
            role: role.to_string(),
            content: content.to_string(),
        });
    }
    out
}

pub fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.0} MB", bytes as f64 / 1_000_000.0)
    } else {
        format!("{} B", bytes)
    }
}

pub fn tools_from_registry(registry: &crate::agent::tools::ToolRegistry) -> Vec<OllamaTool> {
    registry
        .specs()
        .into_iter()
        .map(|spec| {
            let mut properties = std::collections::HashMap::new();
            let mut required = Vec::new();
            for p in &spec.parameters {
                properties.insert(
                    p.name.to_string(),
                    OllamaParamDef {
                        param_type: "string".into(),
                        description: p.description.to_string(),
                    },
                );
                if p.required {
                    required.push(p.name.to_string());
                }
            }
            OllamaTool {
                tool_type: "function".into(),
                function: OllamaToolFunction {
                    name: spec.name.to_string(),
                    description: spec.description.to_string(),
                    parameters: OllamaToolParams {
                        param_type: "object".into(),
                        properties,
                        required,
                    },
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_host_adds_scheme() {
        assert_eq!(normalize_host("localhost:11434"), "http://localhost:11434");
        assert_eq!(
            normalize_host("my.server.com:11434"),
            "http://my.server.com:11434"
        );
    }

    #[test]
    fn normalize_host_keeps_existing_scheme() {
        assert_eq!(
            normalize_host("http://localhost:11434"),
            "http://localhost:11434"
        );
        assert_eq!(
            normalize_host("https://remote.host:443"),
            "https://remote.host:443"
        );
    }

    #[test]
    fn normalize_host_strips_trailing_slash() {
        assert_eq!(
            normalize_host("http://localhost:11434/"),
            "http://localhost:11434"
        );
    }

    #[test]
    fn normalize_host_trims_whitespace() {
        assert_eq!(
            normalize_host("  http://localhost:11434  "),
            "http://localhost:11434"
        );
    }

    #[test]
    fn parse_tags_response_empty() {
        let json = r#"{"models":[]}"#;
        let resp: TagsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.models.unwrap().is_empty());
    }

    #[test]
    fn parse_tags_response_with_models() {
        let json = r#"{"models":[{"name":"llama2:latest","size":3825819519,"details":{"parameter_size":"7B","family":"llama"}}]}"#;
        let resp: TagsResponse = serde_json::from_str(json).unwrap();
        let models = resp.models.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "llama2:latest");
        assert_eq!(models[0].size, 3825819519);
    }

    #[test]
    fn parse_tags_response_missing_details() {
        let json = r#"{"models":[{"name":"test:latest","size":100}]}"#;
        let resp: TagsResponse = serde_json::from_str(json).unwrap();
        let models = resp.models.unwrap();
        assert_eq!(models[0].name, "test:latest");
        assert!(models[0].details.is_none());
    }

    #[test]
    fn parse_chat_chunk_streaming() {
        let json = r#"{"message":{"role":"assistant","content":"Hello"},"done":false}"#;
        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        assert!(!chunk.done);
        assert_eq!(chunk.message.unwrap().content, "Hello");
    }

    #[test]
    fn parse_chat_chunk_done() {
        let json = r#"{"message":{"role":"assistant","content":""},"done":true}"#;
        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.done);
    }

    #[test]
    fn build_chat_messages_basic() {
        let msgs = vec![crate::ai::ChatMessage {
            role: crate::ai::MessageRole::User,
            content: "hello".to_string(),
            streaming: false,
        }];
        let result = build_chat_messages("You are helpful", &msgs);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "system");
        assert_eq!(result[0].content, "You are helpful");
        assert_eq!(result[1].role, "user");
        assert_eq!(result[1].content, "hello");
    }

    #[test]
    fn build_chat_messages_skips_empty() {
        let msgs = vec![
            crate::ai::ChatMessage {
                role: crate::ai::MessageRole::User,
                content: "hi".to_string(),
                streaming: false,
            },
            crate::ai::ChatMessage {
                role: crate::ai::MessageRole::Assistant,
                content: String::new(),
                streaming: false,
            },
        ];
        let result = build_chat_messages("sys", &msgs);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn build_agent_messages_basic() {
        let msgs = vec![("user", "do something"), ("assistant", "ok")];
        let result = build_agent_messages("You are an agent", &msgs);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].role, "system");
        assert_eq!(result[1].role, "user");
        assert_eq!(result[2].role, "assistant");
    }

    #[test]
    fn format_size_gb() {
        assert_eq!(format_size(3_825_819_519), "3.8 GB");
    }

    #[test]
    fn format_size_mb() {
        assert_eq!(format_size(500_000_000), "500 MB");
    }

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(1024), "1024 B");
    }

    #[test]
    fn tags_response_null_models_field() {
        let json = r#"{}"#;
        let resp: TagsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.models.is_none());
    }

    #[test]
    fn chat_msg_serialize() {
        let msg = ChatMsg {
            role: "user".to_string(),
            content: "hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"hello\""));
    }

    #[test]
    fn tools_from_registry_produces_valid_tools() {
        let reg = crate::agent::tools::ToolRegistry::with_defaults();
        let tools = tools_from_registry(&reg);
        assert!(tools.len() >= 4);
        for tool in &tools {
            assert_eq!(tool.tool_type, "function");
            assert!(!tool.function.name.is_empty());
            assert!(!tool.function.description.is_empty());
            assert_eq!(tool.function.parameters.param_type, "object");
        }
    }

    #[test]
    fn tools_from_registry_shell_exec_has_required_command() {
        let reg = crate::agent::tools::ToolRegistry::with_defaults();
        let tools = tools_from_registry(&reg);
        let shell = tools
            .iter()
            .find(|t| t.function.name == "shell_exec")
            .unwrap();
        assert!(
            shell
                .function
                .parameters
                .required
                .contains(&"command".to_string())
        );
        assert!(shell.function.parameters.properties.contains_key("command"));
    }

    #[test]
    fn tools_from_registry_list_dir_optional_path() {
        let reg = crate::agent::tools::ToolRegistry::with_defaults();
        let tools = tools_from_registry(&reg);
        let ld = tools
            .iter()
            .find(|t| t.function.name == "list_dir")
            .unwrap();
        assert!(
            !ld.function
                .parameters
                .required
                .contains(&"path".to_string())
        );
        assert!(ld.function.parameters.properties.contains_key("path"));
    }

    #[test]
    fn ollama_tool_serializes_correctly() {
        let reg = crate::agent::tools::ToolRegistry::with_defaults();
        let tools = tools_from_registry(&reg);
        let json = serde_json::to_string(&tools[0]).unwrap();
        assert!(json.contains("\"type\":\"function\""));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"parameters\""));
    }

    #[test]
    fn chat_request_with_tools_serializes() {
        let msgs = vec![ChatMsg {
            role: "user".to_string(),
            content: "hi".to_string(),
        }];
        let reg = crate::agent::tools::ToolRegistry::with_defaults();
        let tools = tools_from_registry(&reg);
        let req = ChatRequest {
            model: "test",
            messages: &msgs,
            stream: true,
            tools: Some(&tools),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"tools\""));
        assert!(json.contains("\"function\""));
    }

    #[test]
    fn chat_request_without_tools_omits_field() {
        let msgs = vec![ChatMsg {
            role: "user".to_string(),
            content: "hi".to_string(),
        }];
        let req = ChatRequest {
            model: "test",
            messages: &msgs,
            stream: true,
            tools: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"tools\""));
    }

    #[test]
    fn parse_chat_chunk_with_tool_calls() {
        let json = r#"{"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"shell_exec","arguments":{"command":"ls -la"}}}]},"done":true}"#;
        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.done);
        let msg = chunk.message.unwrap();
        assert_eq!(msg.tool_calls.len(), 1);
        assert_eq!(msg.tool_calls[0].function.name, "shell_exec");
        assert_eq!(
            msg.tool_calls[0].function.arguments.get("command").unwrap(),
            "ls -la"
        );
    }

    #[test]
    fn parse_chat_chunk_no_tool_calls() {
        let json = r#"{"message":{"role":"assistant","content":"Hello"},"done":false}"#;
        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        let msg = chunk.message.unwrap();
        assert!(msg.tool_calls.is_empty());
        assert_eq!(msg.content, "Hello");
    }
}
