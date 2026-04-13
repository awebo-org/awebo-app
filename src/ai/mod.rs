pub mod inference;
pub mod model_manager;
pub mod registry;
pub mod web_search;

use std::sync::mpsc;
use std::time::Instant;

use crate::ai::inference::InferenceResult;
use crate::ai::model_manager::LoadedModelHandle;

/// Synchronously collect the result of a completed tokio blocking task.
/// Only call when you know the task has finished (e.g. its channel disconnected).
fn collect_blocking<T: Send + 'static>(
    jh: tokio::task::JoinHandle<T>,
) -> Option<T> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(jh).ok()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub streaming: bool,
}

pub struct AiState {
    pub messages: Vec<ChatMessage>,
    pub context_lines: usize,

    pub inference_rx: Option<mpsc::Receiver<String>>,
    pub inference_handle: Option<tokio::task::JoinHandle<Option<InferenceResult>>>,

    pub loaded_model: Option<LoadedModelHandle>,
    pub handle_rx: Option<mpsc::Receiver<LoadedModelHandle>>,
    /// Display name of the currently loaded model (from registry).
    pub loaded_model_name: Option<String>,

    pub auto_scroll: bool,
    pub thinking_since: Option<Instant>,

    /// Background hint inference — produces a single fix command suggestion.
    pub hint_rx: Option<mpsc::Receiver<String>>,
    pub hint_handle: Option<tokio::task::JoinHandle<Option<InferenceResult>>>,
    /// Accumulated hint text (collected from tokens).
    pub hint_buffer: String,

    /// Last inference token counts for context usage display.
    pub last_prompt_tokens: usize,
    pub last_generated_tokens: usize,
    /// Context window size of the loaded model.
    pub context_size: u32,
}

impl AiState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            context_lines: 50,
            inference_rx: None,
            inference_handle: None,
            loaded_model: None,
            handle_rx: None,
            loaded_model_name: None,
            auto_scroll: true,
            thinking_since: None,
            hint_rx: None,
            hint_handle: None,
            hint_buffer: String::new(),
            last_prompt_tokens: 0,
            last_generated_tokens: 0,
            context_size: 0,
        }
    }

    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content,
            streaming: false,
        });
        self.auto_scroll = true;
    }

    pub fn begin_assistant_message(&mut self) {
        self.thinking_since = Some(Instant::now());
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            streaming: true,
        });
        self.auto_scroll = true;
    }

    pub fn append_token(&mut self, token: &str) {
        if self.thinking_since.is_some() {
            self.thinking_since = None;
        }
        if let Some(msg) = self.messages.last_mut() {
            if msg.role == MessageRole::Assistant && msg.streaming {
                msg.content.push_str(token);
            }
        }
        self.auto_scroll = true;
    }

    pub fn finish_streaming(&mut self) {
        self.thinking_since = None;
        if let Some(msg) = self.messages.last_mut() {
            msg.streaming = false;
        }
    }

    /// Check if a model handle has been delivered asynchronously.
    /// Returns `true` if a model was just received.
    pub fn poll_model_events(&mut self) -> bool {
        if let Some(rx) = &self.handle_rx {
            if let Ok(handle) = rx.try_recv() {
                self.loaded_model = Some(handle);
                return true;
            }
        }
        false
    }

    pub fn poll_inference(&mut self) -> bool {
        let rx = match self.inference_rx.take() {
            Some(rx) => rx,
            None => return false,
        };
        let mut got_token = false;
        let mut done = false;
        loop {
            match rx.try_recv() {
                Ok(token) => {
                    if token.is_empty() {
                        self.finish_streaming();
                        done = true;
                        got_token = true;
                        break;
                    }
                    self.append_token(&token);
                    got_token = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.finish_streaming();
                    done = true;
                    break;
                }
            }
        }
        if done {
            if let Some(jh) = self.inference_handle.take() {
                if let Some(Some(result)) = collect_blocking(jh) {
                    self.last_prompt_tokens = result.prompt_tokens;
                    self.last_generated_tokens = result.generated_tokens;
                    self.loaded_model = Some(result.handle);
                }
            }
        } else {
            self.inference_rx = Some(rx);
        }
        got_token
    }

    /// Poll the background hint inference channel.
    /// Returns `Some(command)` when the hint is complete,
    /// `None` if still running or not active.
    pub fn poll_hint(&mut self) -> Option<String> {
        let rx = match self.hint_rx.take() {
            Some(rx) => rx,
            None => return None,
        };
        let mut done = false;
        loop {
            match rx.try_recv() {
                Ok(token) => {
                    if token.is_empty() {
                        done = true;
                        break;
                    }
                    self.hint_buffer.push_str(&token);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }
        if done {
            if let Some(jh) = self.hint_handle.take() {
                if let Some(Some(result)) = collect_blocking(jh) {
                    self.loaded_model = Some(result.handle);
                }
            }
            let result = std::mem::take(&mut self.hint_buffer);
            log::info!("raw model response: {:?}", result);
            let result = strip_channel_tokens(&result);
            let cmd = result
                .trim()
                .trim_start_matches("```")
                .trim_start_matches("bash")
                .trim_start_matches("sh")
                .trim_start_matches('\n')
                .trim_end_matches("```")
                .trim()
                .trim_start_matches("$ ")
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if cmd.is_empty() || cmd.eq_ignore_ascii_case("none") {
                log::info!("model responded NONE (no suggestion)");
                None
            } else {
                log::info!("model suggested: {:?}", cmd);
                Some(cmd)
            }
        } else {
            self.hint_rx = Some(rx);
            None
        }
    }

    /// Build a prompt that asks the model for a single fix command.
    /// Build the prompt for command-fix hints.
    pub fn build_hint_prompt_static(
        command: &str,
        output: &str,
        web_context: Option<&str>,
        model: &llama_cpp_2::model::LlamaModel,
        fallback_template: &str,
    ) -> String {
        use llama_cpp_2::model::{LlamaChatMessage, LlamaChatTemplate};

        let os_info = detect_os_info();

        let system_text = format!("\
You are a terminal assistant running on {os_info}. \
The user just ran a command in their terminal. Analyze the command and its output.

Rules:
- If the command succeeded with normal output and there is nothing to improve, respond with exactly: NONE
- If the command failed or could be corrected, respond with ONLY the corrected command.
- No explanation, no markdown, no backticks, no quotes. Just the raw command or NONE.
- Think about what the user was TRYING to do and suggest the correct way to achieve it:
  * \"command not found\" -> suggest install command (brew install X, apt install X, etc.)
  * Typo in command name -> fix the typo
  * Permission denied -> add sudo
  * Wrong flags or syntax -> fix them
  * Wrong usage (e.g. passing a string as a filename) -> suggest the correct invocation (e.g. piping with echo)
- Only respond NONE when the output is clearly successful and intentional.
- NEVER suggest a command from a different OS. You are on {os_info}. Use only commands available on this OS.
- Prefer the system's native package manager.

Examples:
Command: ls -l
Output: total 0\\n-rw-r--r-- 1 user staff 0 Jan 1 00:00 file.txt
Answer: NONE

Command: gti status
Output: zsh: command not found: gti
Answer: git status

Command: md5 hello
Output: md5: hello: No such file or directory
Answer: echo \"hello\" | md5

Command: cat nonexistent.txt
Output: cat: nonexistent.txt: No such file or directory
Answer: NONE");

        let mut user_text = format!("Command: {command}\nOutput:\n{output}");

        if let Some(ctx) = web_context {
            user_text.push_str("\n\nWeb search results:\n");
            user_text.push_str(ctx);
        }

        user_text.push_str("\nAnswer:");

        if fallback_template == "gemma" {
            return format!(
                "<bos><|turn>system\n{system_text}<turn|>\n<|turn>user\n{user_text}<turn|>\n<|turn>model\n"
            );
        }

        let template = model
            .chat_template(None)
            .ok()
            .or_else(|| LlamaChatTemplate::new(fallback_template).ok())
            .unwrap_or_else(|| LlamaChatTemplate::new("chatml").unwrap());

        let messages = vec![
            LlamaChatMessage::new("system".into(), system_text.clone()).unwrap(),
            LlamaChatMessage::new("user".into(), user_text.clone()).unwrap(),
        ];

        match model.apply_chat_template(&template, &messages, true) {
            Ok(prompt) => prompt,
            Err(_) => {
                format!(
                    "<|im_start|>system\n{system_text}<|im_end|>\n<|im_start|>user\n{user_text}<|im_end|>\n<|im_start|>assistant\n"
                )
            }
        }
    }

    pub fn build_prompt(
        &self,
        terminal_lines: &[String],
        model: &llama_cpp_2::model::LlamaModel,
        fallback_template: &str,
    ) -> String {
        let base = "You are a helpful terminal assistant. \
                     Answer concisely. When suggesting commands, use code blocks. \
                     You have access to the user's recent terminal output below.";
        self.build_prompt_with_system(base, terminal_lines, model, fallback_template)
    }

    /// Build a prompt from a custom system message and explicit chat messages.
    ///
    /// Used by the agent mode which manages its own conversation history.
    /// Each `(role, content)` tuple represents one message — roles are
    /// "user", "assistant", or "system".
    pub fn build_custom_prompt(
        &self,
        system_prompt: &str,
        messages: &[(&str, &str)],
        model: &llama_cpp_2::model::LlamaModel,
        fallback_template: &str,
    ) -> String {
        use llama_cpp_2::model::{LlamaChatMessage, LlamaChatTemplate};

        if fallback_template == "gemma" {
            let mut prompt = String::new();
            prompt.push_str("<bos><|turn>system\n");
            prompt.push_str(system_prompt);
            prompt.push_str("<turn|>\n");
            for (role, content) in messages {
                let tag = if *role == "assistant" { "model" } else { role };
                prompt.push_str(&format!("<|turn>{tag}\n{content}<turn|>\n"));
            }
            prompt.push_str("<|turn>model\n");
            return prompt;
        }

        let template = model
            .chat_template(None)
            .ok()
            .or_else(|| LlamaChatTemplate::new(fallback_template).ok())
            .unwrap_or_else(|| LlamaChatTemplate::new("chatml").unwrap());

        let mut chat_msgs = Vec::new();
        chat_msgs.push(LlamaChatMessage::new("system".into(), system_prompt.to_string()).unwrap());
        for (role, content) in messages {
            if let Ok(m) = LlamaChatMessage::new(role.to_string(), content.to_string()) {
                chat_msgs.push(m);
            }
        }

        match model.apply_chat_template(&template, &chat_msgs, true) {
            Ok(prompt) => {
                log::info!("Agent chat template applied ({} bytes)", prompt.len());
                prompt
            }
            Err(e) => {
                log::error!("Agent apply_chat_template failed: {e:?}, using chatml fallback");
                let mut p = String::new();
                p.push_str("<|im_start|>system\n");
                p.push_str(system_prompt);
                p.push_str("<|im_end|>\n");
                for (role, content) in messages {
                    p.push_str(&format!("<|im_start|>{role}\n{content}<|im_end|>\n"));
                }
                p.push_str("<|im_start|>assistant\n");
                p
            }
        }
    }

    /// Build an inference prompt with a summarization-focused system message.
    pub fn build_summarize_prompt(
        &self,
        terminal_lines: &[String],
        model: &llama_cpp_2::model::LlamaModel,
        fallback_template: &str,
    ) -> String {
        let base = "You are a terminal output summarizer. \
                     The user's recent terminal session is provided below. \
                     Produce a clear, concise summary of what happened: \
                     which commands were run, whether they succeeded or failed, \
                     and any important output. Use markdown formatting.";
        self.build_prompt_with_system(base, terminal_lines, model, fallback_template)
    }

    fn build_prompt_with_system(
        &self,
        system_base: &str,
        terminal_lines: &[String],
        model: &llama_cpp_2::model::LlamaModel,
        fallback_template: &str,
    ) -> String {
        use llama_cpp_2::model::{LlamaChatMessage, LlamaChatTemplate};

        let system_text = {
            if terminal_lines.is_empty() {
                system_base.to_string()
            } else {
                let joined = terminal_lines.join("\n");
                format!("{system_base}\n\n<terminal_context>\n{joined}\n</terminal_context>")
            }
        };

        if fallback_template == "gemma" {
            return self.build_gemma4_prompt(&system_text);
        }

        let template = model
            .chat_template(None)
            .ok()
            .or_else(|| LlamaChatTemplate::new(fallback_template).ok())
            .unwrap_or_else(|| LlamaChatTemplate::new("chatml").unwrap());

        let mut messages = Vec::new();
        messages.push(LlamaChatMessage::new("system".into(), system_text).unwrap());
        for msg in &self.messages {
            match msg.role {
                MessageRole::User => {
                    messages
                        .push(LlamaChatMessage::new("user".into(), msg.content.clone()).unwrap());
                }
                MessageRole::Assistant => {
                    if !msg.content.is_empty() {
                        messages.push(
                            LlamaChatMessage::new("assistant".into(), msg.content.clone()).unwrap(),
                        );
                    }
                }
            }
        }

        match model.apply_chat_template(&template, &messages, true) {
            Ok(prompt) => {
                log::info!(
                    "Chat template '{}' applied ({} bytes)",
                    fallback_template,
                    prompt.len()
                );
                prompt
            }
            Err(e) => {
                log::error!("apply_chat_template '{}' failed: {e:?}", fallback_template);
                self.build_prompt_fallback(terminal_lines)
            }
        }
    }

    /// Build a prompt in Gemma 4 native format.
    ///
    /// Format:
    ///   <bos><|turn>system\n{system}<turn|>\n
    ///   <|turn>user\n{msg}<turn|>\n
    ///   <|turn>model\n{msg}<turn|>\n
    ///   ...
    ///   <|turn>model\n
    fn build_gemma4_prompt(&self, system_text: &str) -> String {
        let mut prompt = String::new();
        prompt.push_str("<bos>");
        prompt.push_str("<|turn>system\n");
        prompt.push_str(system_text);
        prompt.push_str("<turn|>\n");
        for msg in &self.messages {
            match msg.role {
                MessageRole::User => {
                    prompt.push_str("<|turn>user\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("<turn|>\n");
                }
                MessageRole::Assistant => {
                    if !msg.content.is_empty() {
                        prompt.push_str("<|turn>model\n");
                        prompt.push_str(&msg.content);
                        prompt.push_str("<turn|>\n");
                    }
                }
            }
        }
        prompt.push_str("<|turn>model\n");
        prompt
    }

    fn build_prompt_fallback(&self, terminal_lines: &[String]) -> String {
        let system_text = {
            let base = "You are a helpful terminal assistant. \
                         Answer concisely. When suggesting commands, use code blocks.";
            if terminal_lines.is_empty() {
                base.to_string()
            } else {
                let joined = terminal_lines.join("\n");
                format!("{base}\n\n<terminal_context>\n{joined}\n</terminal_context>")
            }
        };

        let mut prompt = String::new();
        prompt.push_str("<|im_start|>system\n");
        prompt.push_str(&system_text);
        prompt.push_str("<|im_end|>\n");
        for msg in &self.messages {
            match msg.role {
                MessageRole::User => {
                    prompt.push_str("<|im_start|>user\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("<|im_end|>\n");
                }
                MessageRole::Assistant => {
                    prompt.push_str("<|im_start|>assistant\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("<|im_end|>\n");
                }
            }
        }
        prompt.push_str("<|im_start|>assistant\n");
        prompt
    }
}

/// Detect OS, architecture and shell for the hint system prompt.
fn detect_os_info() -> String {
    let os = if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Unknown OS"
    };

    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        ""
    };

    let shell = std::env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(String::from))
        .unwrap_or_default();

    let pkg_mgr = if cfg!(target_os = "macos") {
        "brew"
    } else if cfg!(target_os = "linux") {
        if std::path::Path::new("/usr/bin/apt").exists() {
            "apt"
        } else if std::path::Path::new("/usr/bin/dnf").exists() {
            "dnf"
        } else if std::path::Path::new("/usr/bin/pacman").exists() {
            "pacman"
        } else {
            "unknown"
        }
    } else {
        "unknown"
    };

    let mut info = format!("{os} {arch}");
    if !shell.is_empty() {
        info.push_str(&format!(", shell: {shell}"));
    }
    info.push_str(&format!(", package manager: {pkg_mgr}"));
    info
}

/// Strip GPT-oss style channel/message tokens from model output.
///
/// GPT-oss models emit structured output like:
///   `<|channel|>analysis<|message|>...<|end|><|start|>assistant<|channel|>final<|message|>actual response`
///
/// Extracts only the `final` channel content. If no channel tokens, returns unchanged.
pub(crate) fn strip_channel_tokens(text: &str) -> String {
    const CHANNEL: &str = "<|channel|>";
    const MESSAGE: &str = "<|message|>";

    if !text.contains(CHANNEL) {
        return text.to_string();
    }

    if let Some(final_pos) = text.find("<|channel|>final") {
        let after_channel = &text[final_pos + "<|channel|>final".len()..];
        if let Some(msg_pos) = after_channel.find(MESSAGE) {
            let content = &after_channel[msg_pos + MESSAGE.len()..];
            let content = content
                .trim_end_matches("<|end|>")
                .trim_end_matches("<|start|>");
            return content.trim().to_string();
        }
    }

    if let Some(last_msg) = text.rfind(MESSAGE) {
        let content = &text[last_msg + MESSAGE.len()..];
        let content = content
            .trim_end_matches("<|end|>")
            .trim_end_matches("<|start|>");
        return content.trim().to_string();
    }

    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_state_new_defaults() {
        let state = AiState::new();
        assert!(state.messages.is_empty());
        assert_eq!(state.context_lines, 50);
        assert!(state.loaded_model.is_none());
        assert!(state.thinking_since.is_none());
        assert!(state.auto_scroll);
        assert_eq!(state.last_prompt_tokens, 0);
        assert_eq!(state.context_size, 0);
    }

    #[test]
    fn add_user_message() {
        let mut state = AiState::new();
        state.add_user_message("hello".to_string());
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, MessageRole::User);
        assert_eq!(state.messages[0].content, "hello");
        assert!(!state.messages[0].streaming);
        assert!(state.auto_scroll);
    }

    #[test]
    fn begin_assistant_message() {
        let mut state = AiState::new();
        state.begin_assistant_message();
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, MessageRole::Assistant);
        assert!(state.messages[0].streaming);
        assert!(state.messages[0].content.is_empty());
        assert!(state.thinking_since.is_some());
    }

    #[test]
    fn append_token_clears_thinking() {
        let mut state = AiState::new();
        state.begin_assistant_message();
        assert!(state.thinking_since.is_some());
        state.append_token("hello");
        assert!(state.thinking_since.is_none());
        assert_eq!(state.messages[0].content, "hello");
    }

    #[test]
    fn append_token_concatenates() {
        let mut state = AiState::new();
        state.begin_assistant_message();
        state.append_token("hel");
        state.append_token("lo");
        assert_eq!(state.messages[0].content, "hello");
    }

    #[test]
    fn finish_streaming() {
        let mut state = AiState::new();
        state.begin_assistant_message();
        state.append_token("done");
        state.finish_streaming();
        assert!(!state.messages[0].streaming);
        assert!(state.thinking_since.is_none());
    }

    #[test]
    fn poll_model_events_no_receiver() {
        let mut state = AiState::new();
        assert!(!state.poll_model_events());
    }

    #[test]
    fn poll_inference_no_receiver() {
        let mut state = AiState::new();
        assert!(!state.poll_inference());
    }

    #[test]
    fn poll_hint_no_receiver() {
        let mut state = AiState::new();
        assert!(state.poll_hint().is_none());
    }

    #[test]
    fn poll_inference_receives_tokens() {
        let mut state = AiState::new();
        state.begin_assistant_message();

        let (tx, rx) = std::sync::mpsc::channel();
        state.inference_rx = Some(rx);

        tx.send("hello ".to_string()).unwrap();
        tx.send("world".to_string()).unwrap();
        tx.send(String::new()).unwrap();
        drop(tx);

        let got = state.poll_inference();
        assert!(got);
        assert_eq!(state.messages.last().unwrap().content, "hello world");
        assert!(!state.messages.last().unwrap().streaming);
    }

    #[test]
    fn message_role_eq() {
        assert_eq!(MessageRole::User, MessageRole::User);
        assert_eq!(MessageRole::Assistant, MessageRole::Assistant);
        assert_ne!(MessageRole::User, MessageRole::Assistant);
    }

    #[test]
    fn detect_os_info_not_empty() {
        let info = detect_os_info();
        assert!(!info.is_empty());
        assert!(info.contains("macOS") || info.contains("Linux") || info.contains("Windows") || info.contains("Unknown"));
    }

    #[test]
    fn build_prompt_fallback_has_structure() {
        let state = AiState::new();
        let prompt = state.build_prompt_fallback(&["$ ls".to_string(), "file.txt".to_string()]);
        assert!(prompt.contains("<|im_start|>system"));
        assert!(prompt.contains("terminal_context"));
        assert!(prompt.contains("<|im_start|>assistant"));
    }

    #[test]
    fn build_prompt_fallback_empty_context() {
        let state = AiState::new();
        let prompt = state.build_prompt_fallback(&[]);
        assert!(prompt.contains("<|im_start|>system"));
        assert!(!prompt.contains("terminal_context"));
    }

    #[test]
    fn build_gemma4_prompt_format() {
        let mut state = AiState::new();
        state.add_user_message("hello".to_string());
        let prompt = state.build_gemma4_prompt("You are helpful.");
        assert!(prompt.starts_with("<bos>"));
        assert!(prompt.contains("<|turn>system\n"));
        assert!(prompt.contains("<|turn>user\n"));
        assert!(prompt.ends_with("<|turn>model\n"));
    }

    #[test]
    fn strip_channel_no_tokens() {
        assert_eq!(strip_channel_tokens("Hello world"), "Hello world");
    }

    #[test]
    fn strip_channel_final() {
        let input = "<|channel|>analysis<|message|>thinking...<|end|><|start|>assistant<|channel|>final<|message|>Hello! How can I help?";
        assert_eq!(strip_channel_tokens(input), "Hello! How can I help?");
    }

    #[test]
    fn strip_channel_final_with_end() {
        let input = "<|channel|>analysis<|message|>blah<|end|><|start|>assistant<|channel|>final<|message|>Result here<|end|>";
        assert_eq!(strip_channel_tokens(input), "Result here");
    }

    #[test]
    fn strip_channel_partial_stream() {
        let input = "<|channel|>analysis<|message|>thinking<|end|><|start|>assistant<|channel|>final<|message|>Partial resp";
        assert_eq!(strip_channel_tokens(input), "Partial resp");
    }

    #[test]
    fn strip_channel_fallback_last_message() {
        let input = "<|channel|>unknown<|message|>some content";
        assert_eq!(strip_channel_tokens(input), "some content");
    }
}
