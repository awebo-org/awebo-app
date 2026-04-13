//! Command block list for Smart prompt mode.
//!
//! Each executed command becomes a `CommandBlock` containing:
//! - Prompt snapshot (CWD, git, user segments + duration)
//! - The command text as typed by the user
//! - Captured output lines from the terminal grid (with colors)
//!
//! The `BlockList` manages the list of blocks and tracks a grid‐line
//! checkpoint so it can attribute new PTY output to the active block.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::prompt::PromptInfo;
use crate::renderer::pixel_buffer::Rgb;

/// A colored span of text within an output line.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StyledSpan {
    pub text: String,
    pub fg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    /// Inline code — rendered with a subtle background.
    pub code: bool,
    /// Heading level (0 = normal text, 1–6 = h1–h6).
    pub heading_level: u8,
    /// This span is a horizontal rule — rendered as a visual line.
    pub horizontal_rule: bool,
}

impl StyledSpan {
    /// Shorthand for a plain (unstyled) span.
    pub fn plain(text: String, fg: Rgb) -> Self {
        Self { text, fg, bold: false, italic: false, underline: false, strikethrough: false, code: false, heading_level: 0, horizontal_rule: false }
    }
}

/// A single output line made of one or more styled spans.
pub type StyledLine = Vec<StyledSpan>;

/// Returns the plain text content of a styled line.
pub fn styled_line_text(line: &StyledLine) -> String {
    let mut s = String::new();
    for span in line {
        s.push_str(&span.text);
    }
    s
}

/// Create a single-span line with a default color.
pub fn plain_line(text: String, fg: Rgb) -> StyledLine {
    vec![StyledSpan::plain(text, fg)]
}

/// Describes the visual kind of an agent step rendered inline in the
/// block view.  Plain blocks (non-agent) have `agent_step == None`.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentStepKind {
    /// Agent is thinking / streaming tokens.
    Thinking,
    /// Awaiting user approval for a tool call.
    ToolApproval {
        tool_name: String,
        command_preview: String,
        /// 0 = Approve, 1 = Always approve, 2 = Reject
        selected_option: usize,
    },
    /// Result of an executed tool.
    ToolResult {
        tool_name: String,
        is_error: bool,
    },
    /// Agent produced a final answer.
    FinalAnswer,
}

/// A single command block in the Smart mode timeline.
#[derive(Clone)]
pub struct CommandBlock {
    /// Prompt info snapshot at the time the command was submitted.
    pub prompt: PromptInfo,
    /// The command string the user typed.
    pub command: String,
    /// Captured output lines with color information.
    pub output: Vec<StyledLine>,
    /// When the command was submitted.
    pub started: Instant,
    /// How long the command took (set when the command finishes).
    /// `None` means still running.
    pub duration: Option<Duration>,
    /// Whether this block is currently selected (clicked).
    pub selected: bool,
    /// Whether AI is in the thinking phase (processing before first token).
    pub thinking: bool,
    /// Whether this block represents an error (non-zero exit code or AI error).
    pub is_error: bool,
    /// Process exit code reported by the shell integration hook.
    /// `None` until the shell reports it.
    pub exit_code: Option<i32>,
    /// Transient cursor line shown while command is running (e.g. "Password:").
    /// Not persisted — cleared when the block finishes.
    pub pending_line: Option<StyledLine>,
    /// Grid checkpoint when this block was created. Used to detect whether
    /// any PTY output has flowed since the command was submitted. Without
    /// this, stale Wakeup events can make `cursor_on_prompt()` return true
    /// on the OLD prompt before the command has been echoed.
    pub checkpoint_at_start: i32,
    /// `true` for blocks restored from a saved session. These must not be
    /// re-recorded by `record_last_block` to avoid duplicate entries.
    pub restored: bool,
    /// If `Some`, this block is part of an agent session and should be
    /// rendered with agent-specific visuals.
    pub agent_step: Option<AgentStepKind>,
}

impl CommandBlock {
    /// Returns the elapsed time — frozen `duration` if finished,
    /// live `started.elapsed()` if still running.
    pub fn elapsed(&self) -> Duration {
        self.duration.unwrap_or_else(|| self.started.elapsed())
    }

    /// Whether the command is still running (no duration recorded yet).
    pub fn is_running(&self) -> bool {
        self.duration.is_none()
    }
}

/// Manages the list of command blocks and tracks the PTY grid checkpoint.
pub struct BlockList {
    pub blocks: Vec<CommandBlock>,
    /// Absolute line index in the terminal grid from which to read new output.
    /// Updated after each `capture_output()` call.
    grid_checkpoint: i32,
    /// Vertical scroll offset in logical pixels (0 = bottom, grows upward).
    pub scroll_offset: f32,
    /// Monotonically increasing counter bumped whenever block output changes.
    /// Used by the renderer to detect when cached heights need recomputation.
    pub generation: u64,
}

/// Default foreground for AI / plain text.
pub const DEFAULT_FG: Rgb = (170, 172, 182);

/// Position within block output text, used for text selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockTextPos {
    pub block_idx: usize,
    pub line_idx: usize,
    pub char_idx: usize,
}

impl PartialOrd for BlockTextPos {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BlockTextPos {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.block_idx
            .cmp(&other.block_idx)
            .then(self.line_idx.cmp(&other.line_idx))
            .then(self.char_idx.cmp(&other.char_idx))
    }
}

/// Text selection across block output.
#[derive(Clone, Debug)]
pub struct BlockSelection {
    pub anchor: BlockTextPos,
    pub head: BlockTextPos,
}

impl BlockSelection {
    pub fn new(pos: BlockTextPos) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn start(&self) -> BlockTextPos {
        self.anchor.min(self.head)
    }

    pub fn end(&self) -> BlockTextPos {
        self.anchor.max(self.head)
    }

    /// Extract selected text from block list.
    pub fn extract_text(&self, blocks: &[CommandBlock], max_chars: usize) -> String {
        let start = self.start();
        let end = self.end();
        let mut result = String::new();

        for bi in start.block_idx..=end.block_idx.min(blocks.len().saturating_sub(1)) {
            let block = &blocks[bi];
            let flat = flatten_output(&block.output, max_chars);

            let line_start = if bi == start.block_idx {
                start.line_idx
            } else {
                0
            };
            let line_end = if bi == end.block_idx {
                end.line_idx
            } else {
                flat.len().saturating_sub(1)
            };

            for li in line_start..=line_end.min(flat.len().saturating_sub(1)) {
                let text = &flat[li];
                let cs = if bi == start.block_idx && li == start.line_idx {
                    start.char_idx
                } else {
                    0
                };
                let ce = if bi == end.block_idx && li == end.line_idx {
                    end.char_idx
                } else {
                    text.len()
                };
                let slice: String = text.chars().skip(cs).take(ce.saturating_sub(cs)).collect();
                result.push_str(&slice);
                if li < line_end && li < flat.len().saturating_sub(1) {
                    result.push('\n');
                }
            }
            if bi < end.block_idx {
                result.push('\n');
            }
        }
        result
    }
}

/// Flatten block output into plain-text lines with word wrapping applied.
fn flatten_output(output: &[StyledLine], max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for styled_line in output {
        let text = styled_line_text(styled_line);
        if max_chars > 0 && text.len() > max_chars {
            let wrapped = crate::ui::components::block_renderer::word_wrap(&text, max_chars);
            for w in wrapped {
                lines.push(w);
            }
        } else {
            lines.push(text);
        }
    }
    lines
}

impl BlockList {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            grid_checkpoint: 0,
            scroll_offset: 0.0,
            generation: 0,
        }
    }

    /// Reset checkpoint to current terminal cursor position after clear.
    pub fn sync_checkpoint(&mut self, terminal: &crate::terminal::Terminal) {
        let (_, new_cp) = terminal.read_styled_lines_from(self.grid_checkpoint);
        self.grid_checkpoint = new_cp;
    }

    /// Increment the generation counter (signals renderer cache invalidation).
    pub fn bump_generation(&mut self) {
        self.generation += 1;
    }

    /// Start a new command block. Finishes the duration of the previous block.
    pub fn push_command(&mut self, prompt: PromptInfo, command: String) {
        if let Some(prev) = self.blocks.last() {
            log::info!(
                "push_command: finishing prev block#{} cmd={:?} was_running={} output_len={}",
                self.blocks.len() - 1,
                prev.command,
                prev.is_running(),
                prev.output.len(),
            );
        }
        self.finish_last();
        self.generation += 1;

        let cp = self.grid_checkpoint;
        log::info!(
            "push_command: creating block#{} cmd={:?} checkpoint_at_start={}",
            self.blocks.len(),
            command,
            cp,
        );
        self.blocks.push(CommandBlock {
            prompt,
            command,
            output: Vec::new(),
            started: Instant::now(),
            duration: None,
            selected: false,
            thinking: false,
            is_error: false,
            exit_code: None,
            pending_line: None,
            checkpoint_at_start: cp,
            restored: false,
            agent_step: None,
        });
    }

    /// Append plain text to the last block's output (used by AI streaming).
    ///
    /// The text may contain newlines — it is split into lines and merged
    /// with any partial (trailing) line from a previous call.
    pub fn append_output_text(&mut self, text: &str) {
        let block = match self.blocks.last_mut() {
            Some(b) => b,
            None => return,
        };
        self.generation += 1;

        let mut parts = text.split('\n');

        if let Some(first) = parts.next() {
            if let Some(last_line) = block.output.last_mut() {
                if let Some(last_span) = last_line.last_mut() {
                    last_span.text.push_str(first);
                } else {
                    last_line.push(StyledSpan::plain(first.to_string(), DEFAULT_FG));
                }
            } else {
                block.output.push(plain_line(first.to_string(), DEFAULT_FG));
            }
        }

        for part in parts {
            block.output.push(plain_line(part.to_string(), DEFAULT_FG));
        }
    }

    /// Mark the last block's duration as finished.
    pub fn finish_last(&mut self) {
        if let Some(prev) = self.blocks.last_mut() {
            if prev.duration.is_none() {
                prev.duration = Some(prev.started.elapsed());
                prev.pending_line = None;
                strip_trailing_prompt_lines(&mut prev.output);
                if let Some(code) = prev.exit_code {
                    prev.is_error = code != 0;
                }
            }
        }
    }

    /// Replace the last block's output with markdown-parsed content from `full_text`.
    ///
    /// Used during live AI streaming so each token batch triggers a full
    /// markdown re-parse (cheap on small texts, gives instant styled output).
    pub fn set_output_markdown(&mut self, full_text: &str) {
        let block = match self.blocks.last_mut() {
            Some(b) => b,
            None => return,
        };
        block.output = crate::ui::markdown::parse(full_text);
        self.generation += 1;
    }

    /// Tag the last block with an agent step kind for visual differentiation.
    pub fn set_last_agent_step(&mut self, step: AgentStepKind) {
        if let Some(block) = self.blocks.last_mut() {
            block.agent_step = Some(step);
            self.generation += 1;
        }
    }

    /// Read new lines from the terminal grid since the last checkpoint
    /// and append them to the current (last) block's output.
    ///
    /// Only reads lines above the cursor (settled output). The cursor line
    /// itself is shown live in the raw grid area while a command is running.
    ///
    /// Returns `true` if new lines were captured.
    pub fn capture_output(&mut self, terminal: &crate::terminal::Terminal) -> bool {
        let (lines, new_checkpoint) = terminal.read_styled_lines_from(self.grid_checkpoint);
        let block_count = self.blocks.len();
        let old_cp = self.grid_checkpoint;

        let block = match self.blocks.last_mut() {
            Some(b) => b,
            None => {
                self.grid_checkpoint = new_checkpoint;
                return false;
            }
        };

        let is_finished = block.duration.is_some();

        if !lines.is_empty() || new_checkpoint != old_cp {
            log::debug!(
                "capture_output: block#{} cmd={:?} finished={} lines={} old_cp={} new_cp={} output_len={}",
                block_count.saturating_sub(1),
                &block.command,
                is_finished,
                lines.len(),
                old_cp,
                new_checkpoint,
                block.output.len(),
            );
        }

        if !is_finished {
            let had_pending = block.pending_line.is_some();
            block.pending_line = terminal.read_cursor_line_if_not_prompt();
            if had_pending != block.pending_line.is_some() {
                self.generation += 1;
            }
        }

        if lines.is_empty() {
            self.grid_checkpoint = new_checkpoint;
            return !is_finished && block.pending_line.is_some();
        }

        if is_finished {
            log::warn!(
                "capture_output: DISCARDING {} lines for finished block#{} cmd={:?}",
                lines.len(),
                block_count.saturating_sub(1),
                &block.command,
            );
            self.grid_checkpoint = new_checkpoint;
            return false;
        }

        let cmd_trimmed = block.command.trim();
        let mut added = false;
        let mut skipped_noise = 0;

        let echo_idx = if block.output.is_empty() && !cmd_trimmed.is_empty() {
            lines.iter().position(|line| {
                let text = styled_line_text(line);
                let trimmed = text.trim();
                trimmed == cmd_trimmed || trimmed.ends_with(cmd_trimmed)
            })
        } else {
            None
        };

        let skip_count = echo_idx.map(|i| i + 1).unwrap_or(0);

        for (idx, line) in lines.into_iter().enumerate() {
            if idx < skip_count {
                continue;
            }
            if is_shell_integration_noise(&line) {
                skipped_noise += 1;
                continue;
            }
            block.output.push(line);
            added = true;
        }
        if added {
            self.generation += 1;
        }

        log::debug!(
            "capture_output: block#{} added={} echo_at={:?} skipped_noise={} total_output={}",
            block_count.saturating_sub(1),
            added,
            echo_idx,
            skipped_noise,
            block.output.len(),
        );

        self.grid_checkpoint = new_checkpoint;
        true
    }

    /// Called when an app-controlled process exits (alt screen or TUI).
    /// Finishes the block with a simple exit notice instead of trying to
    /// snapshot the shared grid (which is unreliable).
    /// Also resets the grid checkpoint to the current cursor position
    /// so we don't capture stale TUI output into the next block.
    pub fn finish_app_block(&mut self, terminal: &crate::terminal::Terminal) {
        if let Some(block) = self.blocks.last_mut() {
            if block.duration.is_none() {
                if block.output.is_empty() {
                    block
                        .output
                        .push(plain_line("Process exited.".to_string(), (100, 102, 112)));
                }
                block.duration = Some(block.started.elapsed());
            }
        }
        let (_, new_checkpoint) = terminal.read_styled_lines_from(self.grid_checkpoint);
        self.grid_checkpoint = new_checkpoint;
    }

    /// Finish the last block IF we have a hard confirmation that the
    /// command completed.
    ///
    /// Hard signals (in priority order):
    ///  1. `exit_code` is set — shell integration hook confirmed it.
    ///  2. Real PTY activity was observed for this block (output lines
    ///     were captured, or a pending cursor line was seen), AND the
    ///     caller already verified `cursor_on_prompt()`.
    ///  3. The caller confirmed the cursor is on a prompt (`prompt_visible`).
    ///
    /// If none of these conditions is met, the block stays running — a stale
    /// Wakeup cannot trick us into closing a freshly-created block.
    pub fn finish_block_if_confirmed(&mut self) {
        self.finish_block_if_confirmed_inner(false);
    }

    /// Like `finish_block_if_confirmed` but the caller has already verified
    /// that the terminal cursor sits on a prompt line.
    pub fn finish_block_if_confirmed_prompt(&mut self) {
        self.finish_block_if_confirmed_inner(true);
    }

    fn finish_block_if_confirmed_inner(&mut self, prompt_visible: bool) {
        let block_count = self.blocks.len();
        let cp = self.grid_checkpoint;
        if let Some(block) = self.blocks.last_mut() {
            if block.duration.is_some() {
                return;
            }

            let has_exit_code = block.exit_code.is_some();
            let had_pty_activity = !block.output.is_empty()
                || block.pending_line.is_some()
                || cp != block.checkpoint_at_start;

            log::info!(
                "finish_block_if_confirmed: block#{} cmd={:?} exit_code={:?} output_len={} pending={} cp={} cp_start={} activity={} prompt={} → {}",
                block_count.saturating_sub(1),
                block.command,
                block.exit_code,
                block.output.len(),
                block.pending_line.is_some(),
                cp,
                block.checkpoint_at_start,
                had_pty_activity,
                prompt_visible,
                if has_exit_code || had_pty_activity || prompt_visible { "FINISHING" } else { "BLOCKED" },
            );

            if !has_exit_code && !had_pty_activity && !prompt_visible {
                return;
            }

            block.duration = Some(block.started.elapsed());
            block.pending_line = None;
            strip_trailing_prompt_lines(&mut block.output);
            if let Some(code) = block.exit_code {
                block.is_error = code != 0;
            }
        }
    }

    /// Total number of blocks.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Whether the last command is still running (waiting for output / input).
    pub fn last_is_running(&self) -> bool {
        self.blocks.last().map_or(false, |b| b.is_running())
    }

    /// Whether the last block finished with a non-zero exit code.
    pub fn last_is_error(&self) -> bool {
        self.blocks.last().map_or(false, |b| b.is_error)
    }
}

/// Whether a command string is an internal slash command that should be
/// excluded from AI context and shell history.
fn is_slash_command_block(cmd: &str) -> bool {
    cmd.starts_with("/agent")
        || cmd.starts_with("/ask")
        || cmd.starts_with("/summarize")
        || cmd.starts_with("/models")
        || cmd.starts_with("/help")
        || cmd.starts_with("/clear")
}

/// Characters commonly used as prompt suffixes by shells (zsh `%`,
/// bash `$`, root `#`, starship `❯`, generic `>`).
const PROMPT_SUFFIXES: &[char] = &['%', '$', '#', '❯', '>'];

/// Returns `true` when `text` (trimmed) ends with a typical shell prompt character.
pub fn text_looks_like_prompt(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty() && trimmed.ends_with(PROMPT_SUFFIXES)
}

/// Check if a styled line looks like a shell prompt.
fn looks_like_prompt(line: &StyledLine) -> bool {
    text_looks_like_prompt(&styled_line_text(line))
}

/// Check if a line contains our shell integration hook code.
///
/// When we inject `__term_report_ec` into the PTY, the shell echoes
/// the typed command before executing it.  These echo lines should
/// never appear in block output.
fn is_shell_integration_noise(line: &StyledLine) -> bool {
    let text = styled_line_text(line);
    text.contains("__term_report_ec") || text.contains("__TERM_EC:")
}

/// Remove trailing lines from output that look like shell prompts
/// or contain shell integration hook noise.
/// These can sneak in when the shell reprints its prompt after Ctrl+C
/// or when commands finish and the prompt is captured before the
/// `cursor_on_prompt` check runs.
fn strip_trailing_prompt_lines(output: &mut Vec<StyledLine>) {
    while let Some(last) = output.last() {
        if looks_like_prompt(last) || is_shell_integration_noise(last) {
            output.pop();
        } else {
            break;
        }
    }
}

/// Information about a clickable link detected in block output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoveredLink {
    /// Block index containing the link.
    pub block_idx: usize,
    /// Visual (wrapped) line index within the block output.
    pub visual_line_idx: usize,
    /// Character start offset within the visual line.
    pub char_start: usize,
    /// Character end offset (exclusive) within the visual line.
    pub char_end: usize,
    /// The resolved absolute path on disk.
    pub path: String,
}

/// Try to find a file-path token at character position `char_idx` within `text`.
///
/// Returns `(start, end, token)` if a path-like token is found at that position.
/// This does NOT verify existence — the caller should do that.
pub fn path_token_at(text: &str, char_idx: usize) -> Option<(usize, usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    if char_idx >= chars.len() {
        return None;
    }

    let is_delimiter = |c: char| -> bool {
        c.is_whitespace()
            || c == '"'
            || c == '\''
            || c == '('
            || c == ')'
            || c == '['
            || c == ']'
            || c == '{'
            || c == '}'
            || c == '<'
            || c == '>'
            || c == '|'
            || c == ';'
            || c == ','
    };

    let mut start = char_idx;
    while start > 0 && !is_delimiter(chars[start - 1]) {
        start -= 1;
    }

    let mut end = char_idx;
    while end < chars.len() && !is_delimiter(chars[end]) {
        end += 1;
    }

    if start == end {
        return None;
    }

    let token: String = chars[start..end].iter().collect();

    let clean = strip_line_col_suffix(&token);

    if clean.contains("://") {
        return None;
    }

    if clean.len() <= 1 || clean.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    Some((start, end, clean.to_string()))
}

/// Strip trailing `:line` or `:line:col` suffixes from a path string.
/// e.g. `src/main.rs:42:10` -> `src/main.rs`
fn strip_line_col_suffix(s: &str) -> &str {
    let mut result = s;
    for _ in 0..2 {
        if let Some(colon_pos) = result.rfind(':') {
            let suffix = &result[colon_pos + 1..];
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                result = &result[..colon_pos];
            } else {
                break;
            }
        } else {
            break;
        }
    }
    result
}

/// Resolve a path token to an absolute path, checking existence.
///
/// `cwd` is the working directory to resolve relative paths against.
/// Returns `Some(absolute_path)` if the file exists.
pub fn resolve_path(token: &str, cwd: &str) -> Option<String> {
    let expanded = if token.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            let rest = token.strip_prefix('~').unwrap_or(token);
            let rest = rest.strip_prefix('/').unwrap_or(rest);
            home.join(rest).to_string_lossy().to_string()
        } else {
            return None;
        }
    } else if token.starts_with('/') {
        token.to_string()
    } else {
        let base = Path::new(cwd);
        base.join(token).to_string_lossy().to_string()
    };

    let path = Path::new(&expanded);
    if path.exists() {
        Some(expanded)
    } else {
        None
    }
}

impl BlockList {
    ///
    /// Returns lines representing recent blocks (command + output) in
    /// **reverse chronological order** (newest first), capped at
    /// `max_lines` total.  This ensures the AI always sees the most
    /// recent terminal activity even if the context window is small.
    pub fn context_for_ai(&self, max_lines: usize) -> Vec<String> {
        let mut lines = Vec::new();
        for block in self.blocks.iter().rev() {
            if is_slash_command_block(&block.command) {
                continue;
            }

            let mut block_lines = Vec::new();
            block_lines.push(format!("$ {}", block.command));
            for styled in &block.output {
                block_lines.push(styled_line_text(styled));
            }

            if lines.len() + block_lines.len() > max_lines {
                let remaining = max_lines.saturating_sub(lines.len());
                let start = block_lines.len().saturating_sub(remaining);
                lines.extend_from_slice(&block_lines[start..]);
                break;
            }

            lines.extend(block_lines);

            if lines.len() >= max_lines {
                break;
            }
        }

        lines.reverse();
        lines
    }

    /// Return the list of user-typed commands (for history navigation).
    /// Returns them in chronological order (oldest first).
    pub fn command_history(&self) -> Vec<&str> {
        self.blocks
            .iter()
            .filter(|b| !b.command.is_empty() && !is_slash_command_block(&b.command))
            .map(|b| b.command.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::{PromptInfo, PromptSegment};

    fn test_prompt() -> PromptInfo {
        PromptInfo {
            segments: vec![PromptSegment {
                kind: crate::prompt::SegmentKind::Cwd,
                text: "~/test".to_string(),
                fg: (100, 200, 255),
            }],
            diff_additions: 0,
            diff_deletions: 0,
        }
    }

    #[test]
    fn styled_line_text_single_span() {
        let line = vec![StyledSpan::plain("hello".to_string(), (255, 255, 255))];
        assert_eq!(styled_line_text(&line), "hello");
    }

    #[test]
    fn styled_line_text_multiple_spans() {
        let line = vec![
            StyledSpan::plain("foo".to_string(), (255, 0, 0)),
            StyledSpan::plain(" bar".to_string(), (0, 255, 0)),
            StyledSpan::plain(" baz".to_string(), (0, 0, 255)),
        ];
        assert_eq!(styled_line_text(&line), "foo bar baz");
    }

    #[test]
    fn styled_line_text_empty() {
        let line: StyledLine = vec![];
        assert_eq!(styled_line_text(&line), "");
    }

    #[test]
    fn plain_line_creates_single_span() {
        let line = plain_line("hello".to_string(), (10, 20, 30));
        assert_eq!(line.len(), 1);
        assert_eq!(line[0].text, "hello");
        assert_eq!(line[0].fg, (10, 20, 30));
    }

    #[test]
    fn block_list_new_empty() {
        let bl = BlockList::new();
        assert_eq!(bl.len(), 0);
        assert!(!bl.last_is_running());
    }

    #[test]
    fn push_command_adds_block() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "ls -la".to_string());
        assert_eq!(bl.len(), 1);
        assert!(bl.last_is_running());
        assert_eq!(bl.blocks[0].command, "ls -la");
    }

    #[test]
    fn push_command_finishes_previous() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "first".to_string());
        assert!(bl.blocks[0].is_running());

        bl.push_command(test_prompt(), "second".to_string());
        assert!(!bl.blocks[0].is_running());
        assert!(bl.blocks[1].is_running());
    }

    #[test]
    fn finish_last_sets_duration() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "test".to_string());
        assert!(bl.blocks[0].duration.is_none());
        bl.finish_last();
        assert!(bl.blocks[0].duration.is_some());
    }

    /// Simulate grid activity by advancing the checkpoint past the block's
    /// creation checkpoint. This represents real PTY output having flowed.
    fn simulate_grid_activity(bl: &mut BlockList) {
        bl.grid_checkpoint += 1;
    }

    #[test]
    fn finish_block_if_confirmed_with_exit_code() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "test".to_string());
        bl.blocks.last_mut().unwrap().exit_code = Some(0);
        bl.finish_block_if_confirmed();
        assert!(!bl.last_is_running());
    }

    #[test]
    fn finish_block_if_confirmed_with_grid_activity() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "test".to_string());
        simulate_grid_activity(&mut bl);
        bl.finish_block_if_confirmed();
        assert!(!bl.last_is_running());
    }

    #[test]
    fn finish_block_if_confirmed_blocks_without_evidence() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "test".to_string());
        bl.finish_block_if_confirmed();
        assert!(bl.last_is_running(), "block without evidence must stay running");
    }

    #[test]
    fn append_output_text_single_line() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "echo hi".to_string());
        bl.append_output_text("hello world");
        assert_eq!(bl.blocks[0].output.len(), 1);
        assert_eq!(styled_line_text(&bl.blocks[0].output[0]), "hello world");
    }

    #[test]
    fn append_output_text_multiline() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "ls".to_string());
        bl.append_output_text("line1\nline2\nline3");
        assert_eq!(bl.blocks[0].output.len(), 3);
        assert_eq!(styled_line_text(&bl.blocks[0].output[0]), "line1");
        assert_eq!(styled_line_text(&bl.blocks[0].output[1]), "line2");
        assert_eq!(styled_line_text(&bl.blocks[0].output[2]), "line3");
    }

    #[test]
    fn append_output_text_incremental() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "cat".to_string());
        bl.append_output_text("hel");
        bl.append_output_text("lo");
        assert_eq!(bl.blocks[0].output.len(), 1);
        assert_eq!(styled_line_text(&bl.blocks[0].output[0]), "hello");
    }

    #[test]
    fn append_output_text_empty_no_blocks() {
        let mut bl = BlockList::new();
        bl.append_output_text("stray text");
        assert_eq!(bl.len(), 0);
    }

    #[test]
    fn context_for_ai_empty() {
        let bl = BlockList::new();
        let ctx = bl.context_for_ai(100);
        assert!(ctx.is_empty());
    }

    #[test]
    fn context_for_ai_includes_commands() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "ls".to_string());
        bl.append_output_text("file.txt");
        bl.finish_last();

        let ctx = bl.context_for_ai(100);
        assert!(ctx.iter().any(|l| l == "$ ls"));
        assert!(ctx.iter().any(|l| l == "file.txt"));
    }

    #[test]
    fn context_for_ai_skips_slash_commands() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "/ask what is rust".to_string());
        bl.finish_last();
        bl.push_command(test_prompt(), "/models".to_string());
        bl.finish_last();
        bl.push_command(test_prompt(), "/help".to_string());
        bl.finish_last();
        bl.push_command(test_prompt(), "/clear".to_string());
        bl.finish_last();
        bl.push_command(test_prompt(), "echo hello".to_string());
        bl.finish_last();

        let ctx = bl.context_for_ai(100);
        assert!(ctx.iter().any(|l| l == "$ echo hello"));
        assert!(!ctx.iter().any(|l| l.contains("/ask")));
        assert!(!ctx.iter().any(|l| l.contains("/models")));
    }

    #[test]
    fn context_for_ai_respects_max_lines() {
        let mut bl = BlockList::new();
        for i in 0..10 {
            bl.push_command(test_prompt(), format!("cmd{i}"));
            bl.append_output_text(&format!("output{i}"));
            bl.finish_last();
        }

        let ctx = bl.context_for_ai(5);
        assert!(ctx.len() <= 5);
    }

    #[test]
    fn command_history_filters_slash() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "ls".to_string());
        bl.push_command(test_prompt(), "/ask test".to_string());
        bl.push_command(test_prompt(), "pwd".to_string());
        bl.push_command(test_prompt(), "/clear".to_string());
        bl.push_command(test_prompt(), "".to_string());

        let hist = bl.command_history();
        assert_eq!(hist, vec!["ls", "pwd"]);
    }

    #[test]
    fn block_elapsed_running() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "sleep 10".to_string());
        let elapsed = bl.blocks[0].elapsed();
        assert!(elapsed.as_millis() < 1000);
    }

    #[test]
    fn block_elapsed_finished() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "echo".to_string());
        bl.finish_last();
        let d = bl.blocks[0].duration.unwrap();
        assert_eq!(bl.blocks[0].elapsed(), d);
    }

    #[test]
    fn block_text_pos_ordering() {
        let a = BlockTextPos {
            block_idx: 0,
            line_idx: 0,
            char_idx: 5,
        };
        let b = BlockTextPos {
            block_idx: 0,
            line_idx: 1,
            char_idx: 0,
        };
        let c = BlockTextPos {
            block_idx: 1,
            line_idx: 0,
            char_idx: 0,
        };
        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
    }

    #[test]
    fn block_selection_start_end() {
        let a = BlockTextPos {
            block_idx: 0,
            line_idx: 2,
            char_idx: 5,
        };
        let b = BlockTextPos {
            block_idx: 0,
            line_idx: 0,
            char_idx: 3,
        };
        let sel = BlockSelection { anchor: a, head: b };
        assert_eq!(sel.start(), b);
        assert_eq!(sel.end(), a);
    }

    #[test]
    fn block_selection_extract_single_line() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "echo".to_string());
        bl.append_output_text("hello world");
        bl.finish_last();

        let sel = BlockSelection {
            anchor: BlockTextPos {
                block_idx: 0,
                line_idx: 0,
                char_idx: 0,
            },
            head: BlockTextPos {
                block_idx: 0,
                line_idx: 0,
                char_idx: 5,
            },
        };
        let text = sel.extract_text(&bl.blocks, 0);
        assert_eq!(text, "hello");
    }

    #[test]
    fn block_selection_extract_multi_line() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "ls".to_string());
        bl.append_output_text("alpha\nbeta\ngamma");
        bl.finish_last();

        let sel = BlockSelection {
            anchor: BlockTextPos {
                block_idx: 0,
                line_idx: 0,
                char_idx: 2,
            },
            head: BlockTextPos {
                block_idx: 0,
                line_idx: 2,
                char_idx: 3,
            },
        };
        let text = sel.extract_text(&bl.blocks, 0);
        assert_eq!(text, "pha\nbeta\ngam");
    }

    #[test]
    fn exit_code_sets_is_error_on_nonzero() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "bad_cmd".to_string());
        bl.blocks[0].exit_code = Some(127);
        simulate_grid_activity(&mut bl);
        bl.finish_block_if_confirmed();
        assert!(bl.blocks[0].is_error);
        assert_eq!(bl.blocks[0].exit_code, Some(127));
    }

    #[test]
    fn exit_code_zero_no_error() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "ls".to_string());
        bl.blocks[0].exit_code = Some(0);
        simulate_grid_activity(&mut bl);
        bl.finish_block_if_confirmed();
        assert!(!bl.blocks[0].is_error);
    }

    #[test]
    fn no_exit_code_no_error_by_default() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "ls".to_string());
        simulate_grid_activity(&mut bl);
        bl.finish_block_if_confirmed();
        assert!(!bl.blocks[0].is_error);
        assert!(bl.blocks[0].exit_code.is_none());
    }

    #[test]
    fn finish_confirmed_clears_pending_line() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "sudo test".to_string());
        bl.blocks[0].pending_line = Some(vec![StyledSpan::plain(
            "Password:".to_string(),
            (200, 200, 200),
        )]);
        simulate_grid_activity(&mut bl);
        bl.finish_block_if_confirmed();
        assert!(bl.blocks[0].pending_line.is_none());
    }

    #[test]
    fn finish_confirmed_uses_exit_code() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "echo".to_string());
        bl.append_output_text("error: this is just text");
        bl.blocks[0].exit_code = Some(0);
        simulate_grid_activity(&mut bl);
        bl.finish_block_if_confirmed();
        assert!(!bl.blocks[0].is_error);
    }

    #[test]
    fn strip_trailing_prompt_removes_prompt_lines() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "test".to_string());
        bl.blocks[0]
            .output
            .push(plain_line("real output".to_string(), (200, 200, 200)));
        bl.blocks[0]
            .output
            .push(plain_line("user@host ~ %".to_string(), (200, 200, 200)));
        bl.blocks[0]
            .output
            .push(plain_line("user@host ~ $".to_string(), (200, 200, 200)));
        simulate_grid_activity(&mut bl);
        bl.finish_block_if_confirmed();
        assert_eq!(bl.blocks[0].output.len(), 1);
        assert_eq!(styled_line_text(&bl.blocks[0].output[0]), "real output");
    }

    #[test]
    fn strip_trailing_removes_shell_integration_noise() {
        let mut bl = BlockList::new();
        bl.push_command(test_prompt(), "test".to_string());
        bl.blocks[0]
            .output
            .push(plain_line("real output".to_string(), (200, 200, 200)));
        bl.blocks[0]
            .output
            .push(plain_line(
                "__term_report_ec() { printf ... }; precmd_functions=(__term_report_ec $precmd_functions)".to_string(),
                (200, 200, 200),
            ));
        bl.blocks[0]
            .output
            .push(plain_line("user@host ~ %".to_string(), (200, 200, 200)));
        simulate_grid_activity(&mut bl);
        bl.finish_block_if_confirmed();
        assert_eq!(bl.blocks[0].output.len(), 1);
        assert_eq!(styled_line_text(&bl.blocks[0].output[0]), "real output");
    }

    #[test]
    fn is_shell_integration_noise_detects_hook() {
        let line = plain_line(
            "__term_report_ec() { printf '\\e]0;__TERM_EC:%d__\\a' $? }".to_string(),
            (200, 200, 200),
        );
        assert!(is_shell_integration_noise(&line));
    }

    #[test]
    fn is_shell_integration_noise_passes_normal_text() {
        let line = plain_line("hello world".to_string(), (200, 200, 200));
        assert!(!is_shell_integration_noise(&line));
    }

    #[test]
    fn path_token_at_absolute_path() {
        let text = "error in /usr/local/bin/foo.rs:42 found";
        let result = path_token_at(text, 12);
        assert!(result.is_some());
        let (start, _end, token) = result.unwrap();
        assert_eq!(start, 9);
        assert_eq!(token, "/usr/local/bin/foo.rs");
    }

    #[test]
    fn path_token_at_relative_path() {
        let text = "warning: src/main.rs:10:5 unused variable";
        let result = path_token_at(text, 12);
        assert!(result.is_some());
        let (_start, _end, token) = result.unwrap();
        assert_eq!(token, "src/main.rs");
    }

    #[test]
    fn path_token_at_tilde_path() {
        let text = "reading ~/Documents/test.txt done";
        let result = path_token_at(text, 10);
        assert!(result.is_some());
        let (_start, _end, token) = result.unwrap();
        assert_eq!(token, "~/Documents/test.txt");
    }

    #[test]
    fn path_token_at_returns_token_for_any_word() {
        let text = "hello world no paths here";
        let result = path_token_at(text, 6);
        assert!(result.is_some());
        let (_start, _end, token) = result.unwrap();
        assert_eq!(token, "world");
    }

    #[test]
    fn path_token_at_rejects_single_char() {
        let text = "a b c";
        assert!(path_token_at(text, 0).is_none());
    }

    #[test]
    fn path_token_at_rejects_pure_number() {
        let text = "total 680";
        let result = path_token_at(text, 7);
        assert!(result.is_none());
    }

    #[test]
    fn path_token_at_bare_filename() {
        let text = "-rw-r--r--  1 patryk staff 199872 Apr  9 09:08 terminal.log";
        let result = path_token_at(text, 52);
        assert!(result.is_some());
        let (_start, _end, token) = result.unwrap();
        assert_eq!(token, "terminal.log");
    }

    #[test]
    fn path_token_at_url_rejected() {
        let text = "visit https://example.com/path for info";
        let result = path_token_at(text, 10);
        assert!(result.is_none());
    }

    #[test]
    fn strip_line_col_suffix_basic() {
        assert_eq!(strip_line_col_suffix("src/main.rs:42:10"), "src/main.rs");
        assert_eq!(strip_line_col_suffix("src/main.rs:42"), "src/main.rs");
        assert_eq!(strip_line_col_suffix("src/main.rs"), "src/main.rs");
        assert_eq!(strip_line_col_suffix("/foo/bar:99"), "/foo/bar");
    }
}
