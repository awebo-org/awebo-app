pub mod badges;

use std::time::{Duration, Instant};

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::prompt::{PromptInfo, SegmentKind};
use crate::renderer::icons::{Icon, IconRenderer};

use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{
    draw_text_at, draw_text_at_bold, measure_text_width, measure_text_width_bold,
};
use crate::renderer::theme;

use super::overlay::fill_rounded_rect;

/// Hit-test rects returned by `draw()` for mouse interaction.
pub struct PromptBarHitRects {
    pub ctx_bar: Option<(usize, usize, usize, usize)>,
    pub stop_button: Option<(usize, usize, usize, usize)>,
}

const SEGMENT_H_LOGICAL: f32 = 20.0;
const SEGMENT_PAD_X: f32 = 8.0;
const SEGMENT_GAP: f32 = 6.0;
const SEGMENT_RADIUS: f32 = 4.0;

const INPUT_H_LOGICAL: f32 = 32.0;

/// Shared horizontal content padding for both the segment row and input row,
/// so badges and the text cursor are left-aligned consistently.
const PROMPT_PAD_X: f32 = 12.0;
/// Space above the separator line (between terminal content and prompt bar).
const PROMPT_MARGIN_TOP: f32 = 6.0;
const PROMPT_PAD_TOP: f32 = 10.0;
const PROMPT_GAP: f32 = 8.0;
const PROMPT_PAD_BOTTOM: f32 = 10.0;

const CHAR_WIDTH_EST: f32 = 7.0;

const BORDER_COLOR: Rgb = theme::BORDER;
const SEPARATOR_COLOR: Rgb = (28, 28, 32);
const INPUT_TEXT: Rgb = theme::FG_PRIMARY;
/// Slash menu popup colors.
const SLASH_MENU_BG: Rgb = theme::BG_SURFACE;
const SLASH_MENU_BORDER: Rgb = theme::BORDER;
const SLASH_MENU_SELECTED_BG: Rgb = theme::BG_SELECTION;
const SLASH_MENU_NAME: Rgb = theme::PRIMARY;
const SLASH_MENU_DESC: Rgb = theme::SETTINGS_HEADER_TEXT;
const SUGGESTION_COLOR: Rgb = theme::FG_MUTED;
const CURSOR_COLOR: Rgb = theme::PRIMARY;
const PASSTHROUGH_HINT: Rgb = theme::WARNING;
/// AI fix suggestion — uses primary accent.
const AI_SUGGESTION_COLOR: Rgb = theme::PRIMARY;

/// Definition of a slash command for the prompt input autocomplete menu.
pub struct SlashCommandDef {
    pub name: &'static str,
    pub description: &'static str,
    /// When `true` the command only appears in agent mode.
    pub agent_only: bool,
}

pub const SLASH_COMMANDS: &[SlashCommandDef] = &[
    SlashCommandDef {
        name: "/agent",
        description: "Start agentic mode — delegate tasks to AI",
        agent_only: false,
    },
    SlashCommandDef {
        name: "/ask",
        description: "Ask AI a question about your terminal",
        agent_only: false,
    },
    SlashCommandDef {
        name: "/summarize",
        description: "Summarize recent terminal output",
        agent_only: false,
    },
    SlashCommandDef {
        name: "/close",
        description: "Exit agent mode",
        agent_only: true,
    },
    SlashCommandDef {
        name: "/clear",
        description: "Clear terminal screen",
        agent_only: false,
    },
    SlashCommandDef {
        name: "/models",
        description: "Open model repository",
        agent_only: false,
    },
    SlashCommandDef {
        name: "/help",
        description: "Show available commands",
        agent_only: false,
    },
];

/// The current input mode of the smart terminal.
///
/// `Normal` — standard shell commands + slash commands.
/// `Agent`  — every Enter is dispatched as an agent task; `/close` exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Agent,
}

/// State of the smart prompt input field, owned by `App` and passed to draw.
pub struct InputFieldState {
    pub text: String,
    pub cursor: usize,
    /// When the last command was submitted (Enter pressed).
    pub command_started: Option<Instant>,
    /// Duration of the last completed command.
    pub last_command_duration: Option<Duration>,
    /// Whether the slash command menu is visible.
    pub slash_menu_open: bool,
    /// Currently highlighted slash command index (within filtered list).
    pub slash_selected: usize,
    /// Ghost text suggestion (PATH-based autocomplete).
    pub suggestion: Option<String>,
    /// History navigation index: `None` = editing new text,
    /// `Some(i)` = browsing history at position `i` (0 = most recent).
    pub history_index: Option<usize>,
    /// Stashed text that was being typed before history browsing started.
    pub history_stash: String,
    /// AI-generated fix suggestion (shown as ghost text after an error).
    pub ai_suggestion: Option<String>,
    /// Current input mode (Normal vs Agent).
    pub input_mode: InputMode,
    /// Command text that was submitted and is currently executing.
    /// While set, the prompt bar shows the command inline instead of
    /// showing the empty pass-through indicator. Cleared when the
    /// command finishes.
    pub pending_command: Option<String>,
}

impl InputFieldState {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            command_started: None,
            last_command_duration: None,
            slash_menu_open: false,
            slash_selected: 0,
            suggestion: None,
            history_index: None,
            history_stash: String::new(),
            ai_suggestion: None,
            input_mode: InputMode::Normal,
            pending_command: None,
        }
    }

    /// Returns the filtered slash commands based on current input text.
    pub fn filtered_slash_commands(&self) -> Vec<&'static SlashCommandDef> {
        if !self.text.starts_with('/') {
            return Vec::new();
        }
        let query = &self.text;
        let in_agent = self.input_mode == InputMode::Agent;
        SLASH_COMMANDS
            .iter()
            .filter(|cmd| cmd.name.starts_with(query))
            .filter(|cmd| !cmd.agent_only || in_agent)
            .collect()
    }

    /// Update slash menu state based on current input.
    pub fn update_slash_menu(&mut self) {
        if self.text.starts_with('/') {
            let filtered = self.filtered_slash_commands();
            self.slash_menu_open = !filtered.is_empty();
            if self.slash_selected >= filtered.len() {
                self.slash_selected = 0;
            }
        } else {
            self.slash_menu_open = false;
            self.slash_selected = 0;
        }
    }

    /// Update autosuggestion for the current input.
    /// For single-word inputs: finds executables in PATH.
    /// For multi-word inputs: completes file/directory paths relative to `cwd`.
    pub fn update_suggestion(&mut self, cwd: Option<&str>) {
        let text = self.text.trim();
        if text.is_empty() || text.starts_with('/') {
            self.suggestion = None;
            return;
        }
        if text.contains(' ') {
            self.suggestion = find_file_suggestion(text, cwd);
        } else {
            self.suggestion = find_path_suggestion(text);
        }
    }

    /// Query: placeholder text for the current input mode.
    pub fn placeholder(&self) -> &'static str {
        match self.input_mode {
            InputMode::Normal => "Type a command or /slash…",
            InputMode::Agent => "Describe a task for the agent…",
        }
    }

    /// Switch to agent mode.
    pub fn enter_agent_mode(&mut self) {
        self.input_mode = InputMode::Agent;
        self.text.clear();
        self.cursor = 0;
    }

    /// Switch back to normal mode.
    pub fn exit_agent_mode(&mut self) {
        self.input_mode = InputMode::Normal;
        self.text.clear();
        self.cursor = 0;
    }
}

/// Find the first executable in PATH whose name starts with `prefix`.
fn find_path_suggestion(prefix: &str) -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;
    let mut best: Option<String> = None;
    for dir in path_var.split(':') {
        let dir_path = std::path::Path::new(dir);
        let entries = match std::fs::read_dir(dir_path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str()
                && name.starts_with(prefix)
                && name != prefix
                && best.as_ref().is_none_or(|b| name.len() < b.len())
            {
                best = Some(name.to_string());
            }
        }
    }
    best
}

/// Complete file/directory paths for the last argument in `full_text`.
/// Resolves relative paths against `cwd` (the shell's working directory).
fn find_file_suggestion(full_text: &str, cwd: Option<&str>) -> Option<String> {
    let last_space = full_text.rfind(' ')?;
    let prefix_cmd = &full_text[..=last_space];
    let arg_partial = &full_text[last_space + 1..];

    let base_dir = cwd.map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    });

    let (search_dir, name_prefix) = if let Some(sep) = arg_partial.rfind('/') {
        let dir_part = &arg_partial[..=sep];
        let name_part = &arg_partial[sep + 1..];
        let resolved = if std::path::Path::new(dir_part).is_absolute() {
            std::path::PathBuf::from(dir_part)
        } else {
            base_dir.join(dir_part)
        };
        (resolved, name_part)
    } else {
        (base_dir, arg_partial)
    };

    if name_prefix.is_empty() {
        return None;
    }

    let entries = std::fs::read_dir(&search_dir).ok()?;
    let mut best: Option<String> = None;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = match name.to_str() {
            Some(s) => s,
            None => continue,
        };
        if !name_str.starts_with(name_prefix) || name_str == name_prefix {
            continue;
        }
        if name_str.starts_with('.') && !name_prefix.starts_with('.') {
            continue;
        }
        if best.as_ref().is_none_or(|b| name_str.len() < b.len()) {
            let is_dir = entry.file_type().is_ok_and(|ft| ft.is_dir());
            let completed_arg = if let Some(sep) = arg_partial.rfind('/') {
                let dir_part = &arg_partial[..=sep];
                if is_dir {
                    format!("{}{}/", dir_part, name_str)
                } else {
                    format!("{}{}", dir_part, name_str)
                }
            } else if is_dir {
                format!("{}/", name_str)
            } else {
                name_str.to_string()
            };
            best = Some(format!("{}{}", prefix_cmd, completed_arg));
        }
    }
    best
}

/// Total prompt bar height in physical pixels:
/// margin_top + pad_top + segments_row + gap + input_row + pad_bottom.
pub fn prompt_bar_height(sf: f32) -> usize {
    ((PROMPT_MARGIN_TOP
        + PROMPT_PAD_TOP
        + SEGMENT_H_LOGICAL
        + PROMPT_GAP
        + INPUT_H_LOGICAL
        + PROMPT_PAD_BOTTOM)
        * sf)
        .ceil() as usize
}

/// Height of a single pending output line in physical pixels.
const PENDING_LINE_H: f32 = 22.0;

/// Prompt bar height when a pending command is active.
/// Adds one row for the submitted command text and one row for
/// the terminal cursor line (e.g. "Password:").
pub fn prompt_bar_height_with_pending(sf: f32, has_pending_line: bool) -> usize {
    let extra = PENDING_LINE_H
        + if has_pending_line {
            PENDING_LINE_H
        } else {
            0.0
        };
    ((PROMPT_MARGIN_TOP
        + PROMPT_PAD_TOP
        + SEGMENT_H_LOGICAL
        + PROMPT_GAP
        + INPUT_H_LOGICAL
        + extra
        + PROMPT_PAD_BOTTOM)
        * sf)
        .ceil() as usize
}

/// Returns hit-test rects for the context usage bar and stop button.
pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    info: &PromptInfo,
    input: &InputFieldState,
    x_start: usize,
    right_margin: usize,
    y_start: usize,
    sf: f32,
    cursor_visible: bool,
    command_running: bool,
    model_name: Option<&str>,
    ai_thinking: bool,
    pending_line_text: Option<&str>,
) -> PromptBarHitRects {
    let has_pending = input.pending_command.is_some();
    let total_h = if has_pending {
        prompt_bar_height_with_pending(sf, pending_line_text.is_some())
    } else {
        prompt_bar_height(sf)
    };

    buf.fill_rect(
        x_start,
        y_start,
        buf.width.saturating_sub(x_start),
        total_h,
        theme::BG,
    );

    let content_pad = (PROMPT_PAD_X * sf) as usize;

    let margin_top = (PROMPT_MARGIN_TOP * sf) as usize;
    let sep_y = y_start + margin_top;
    let sep_h = (1.0 * sf).max(1.0) as usize;
    buf.fill_rect(
        x_start,
        sep_y,
        buf.width.saturating_sub(x_start),
        sep_h,
        SEPARATOR_COLOR,
    );

    let seg_h = (SEGMENT_H_LOGICAL * sf) as usize;
    let pad_x = (SEGMENT_PAD_X * sf) as usize;
    let gap = (SEGMENT_GAP * sf) as usize;
    let radius = (SEGMENT_RADIUS * sf) as usize;
    let seg_font_size = 12.0 * sf;
    let seg_line_height = 17.0 * sf;
    let seg_metrics = Metrics::new(seg_font_size, seg_line_height);

    let seg_y = sep_y + sep_h + (PROMPT_PAD_TOP * sf) as usize;
    let mut x = x_start + content_pad;

    let icon_sz = (seg_h as f32 * 0.6).round() as u32;
    let icon_gap = (3.0 * sf) as usize;

    for seg in &info.segments {
        let text = &seg.text;
        let text_w = measure_text_width_bold(font_system, text, seg_metrics, Family::Monospace)
            .ceil() as usize;

        let icon = match seg.kind {
            SegmentKind::Cwd => Some(Icon::Folder),
            SegmentKind::GitBranch => Some(Icon::GitBranch),
            SegmentKind::Shell => None,
        };
        let icon_space = if icon.is_some() {
            icon_sz as usize + icon_gap
        } else {
            0
        };
        let seg_w = pad_x + icon_space + text_w + pad_x;

        stroke_rounded_rect(buf, x, seg_y, seg_w, seg_h, radius, sf, BORDER_COLOR);

        if let Some(ic) = icon {
            let icon_y = seg_y + (seg_h.saturating_sub(icon_sz as usize)) / 2;
            icon_renderer.draw(buf, ic, x + pad_x, icon_y, icon_sz, seg.fg);
        }

        let text_x = x + pad_x + icon_space;
        let text_y = seg_y + ((seg_h as f32 - seg_line_height) / 2.0) as usize;
        draw_text_at_bold(
            buf,
            font_system,
            swash_cache,
            text_x,
            text_y,
            buf.height,
            text,
            seg_metrics,
            seg.fg,
            Family::Monospace,
        );

        x += seg_w + gap;
    }

    let mut right_x = buf.width.saturating_sub(right_margin + content_pad);

    let mut badge_ctx = badges::BadgeCtx {
        buf,
        font_system,
        swash_cache,
        icon_renderer,
        sf,
        seg_h,
        pad_x,
        gap,
        radius,
        seg_metrics,
        seg_y,
    };

    let duration_label = if input.command_started.is_some() {
        let elapsed = input
            .command_started
            .as_ref()
            .map(|t| t.elapsed())
            .unwrap_or_default();
        Some(format_duration(elapsed))
    } else {
        input.last_command_duration.map(format_duration)
    };
    if let Some(ref label) = duration_label {
        let res = badges::duration::draw(&mut badge_ctx, right_x, label);
        right_x = right_x.saturating_sub(res.consumed);
    }

    {
        let res = badges::diff_stat::draw(
            &mut badge_ctx,
            right_x,
            info.diff_additions,
            info.diff_deletions,
        );
        right_x = right_x.saturating_sub(res.consumed);
    }

    {
        let res = badges::model::draw(&mut badge_ctx, right_x, model_name, ai_thinking);
        right_x = right_x.saturating_sub(res.consumed);
    }

    let stop_button_rect = {
        let res = badges::stop::draw(&mut badge_ctx, right_x, ai_thinking);
        res.hit_rect
    };

    let buf = badge_ctx.buf;
    let font_system = badge_ctx.font_system;
    let swash_cache = badge_ctx.swash_cache;

    let input_h = (INPUT_H_LOGICAL * sf) as usize;
    let input_y = seg_y + seg_h + (PROMPT_GAP * sf) as usize;

    let input_font_size = 14.0 * sf;
    let input_line_height = 20.0 * sf;
    let input_metrics = Metrics::new(input_font_size, input_line_height);
    let text_y = input_y + ((input_h as f32 - input_line_height) / 2.0) as usize;

    let text_x = x_start + content_pad;
    let cursor_x = text_x;

    if command_running && has_pending {
        let cmd_text = input.pending_command.as_deref().unwrap_or("");
        let cmd_metrics = Metrics::new(13.0 * sf, 18.0 * sf);
        let prefix = "$ ";
        let prefix_w =
            measure_text_width(font_system, prefix, cmd_metrics, Family::Monospace) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            text_x,
            text_y,
            buf.height,
            prefix,
            cmd_metrics,
            theme::FG_DIM,
            Family::Monospace,
        );
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            text_x + prefix_w,
            text_y,
            buf.height,
            cmd_text,
            cmd_metrics,
            theme::FG_PRIMARY,
            Family::Monospace,
        );

        let pending_row_h = (PENDING_LINE_H * sf) as usize;
        let pending_y = input_y + input_h;

        if let Some(line_text) = pending_line_text {
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                text_x,
                pending_y + ((pending_row_h as f32 - 18.0 * sf) / 2.0) as usize,
                buf.height,
                line_text,
                cmd_metrics,
                theme::FG_BRIGHT,
                Family::Monospace,
            );

            let line_w =
                measure_text_width(font_system, line_text, cmd_metrics, Family::Monospace) as usize;
            if cursor_visible {
                let beam_w = (2.0 * sf).max(1.0) as usize;
                let cursor_h = (18.0 * sf * 0.8) as usize;
                let cy = pending_y + ((pending_row_h as f32 - cursor_h as f32) / 2.0) as usize;
                buf.fill_rect(text_x + line_w, cy, beam_w, cursor_h, PASSTHROUGH_HINT);
            }
        } else {
            if cursor_visible {
                let beam_w = (2.0 * sf).max(1.0) as usize;
                let cursor_h = (18.0 * sf * 0.8) as usize;
                let cy = pending_y + ((pending_row_h as f32 - cursor_h as f32) / 2.0) as usize;
                buf.fill_rect(text_x, cy, beam_w, cursor_h, PASSTHROUGH_HINT);
            }
        }
    } else if command_running {
        if cursor_visible {
            let beam_w = (3.0 * sf).max(1.0) as usize;
            let cursor_h = (input_line_height * 0.75) as usize;
            let cursor_y = input_y + ((input_h as f32 - cursor_h as f32) / 2.0) as usize;
            let r = beam_w / 2;
            fill_rounded_rect(
                buf,
                cursor_x,
                cursor_y,
                beam_w,
                cursor_h,
                r,
                PASSTHROUGH_HINT,
            );
        }
    } else if !input.text.is_empty() {
        let slash_cmd_len = if input.text.starts_with('/') {
            let cmd_end = input.text.find(' ').unwrap_or(input.text.len());
            let cmd = &input.text[..cmd_end];
            if SLASH_COMMANDS.iter().any(|sc| sc.name == cmd)
                || SLASH_COMMANDS
                    .iter()
                    .any(|sc| cmd.len() < sc.name.len() && sc.name.starts_with(cmd))
            {
                cmd_end
            } else {
                0
            }
        } else {
            0
        };

        if slash_cmd_len > 0 {
            let cmd_text = &input.text[..slash_cmd_len];
            let rest_text = &input.text[slash_cmd_len..];

            let cmd_w = measure_text_width(font_system, cmd_text, input_metrics, Family::Monospace)
                as usize;
            let pill_pad = (3.0 * sf) as usize;
            let pill_h = (input_line_height * 0.85) as usize;
            let pill_y = text_y + ((input_line_height - pill_h as f32) / 2.0) as usize;
            let pill_r = (3.0 * sf) as usize;
            const SLASH_CMD_BG: Rgb = (55, 20, 45);
            fill_rounded_rect(
                buf,
                text_x.saturating_sub(pill_pad),
                pill_y,
                cmd_w + pill_pad * 2,
                pill_h,
                pill_r,
                SLASH_CMD_BG,
            );

            draw_text_at(
                buf,
                font_system,
                swash_cache,
                text_x,
                text_y,
                buf.height,
                cmd_text,
                input_metrics,
                theme::PRIMARY,
                Family::Monospace,
            );

            if !rest_text.is_empty() {
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    text_x + cmd_w,
                    text_y,
                    buf.height,
                    rest_text,
                    input_metrics,
                    INPUT_TEXT,
                    Family::Monospace,
                );
            }
        } else {
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                text_x,
                text_y,
                buf.height,
                &input.text,
                input_metrics,
                INPUT_TEXT,
                Family::Monospace,
            );
        }

        if cursor_visible {
            let before_cursor = &input.text[..input.cursor.min(input.text.len())];
            let cursor_offset =
                measure_text_width(font_system, before_cursor, input_metrics, Family::Monospace)
                    as usize;
            let beam_w = (1.5 * sf).max(1.0) as usize;
            let cursor_h = (input_line_height * 0.85) as usize;
            let cursor_y = input_y + ((input_h as f32 - cursor_h as f32) / 2.0) as usize;
            let r = beam_w / 2;
            fill_rounded_rect(
                buf,
                cursor_x + cursor_offset,
                cursor_y,
                beam_w,
                cursor_h,
                r,
                CURSOR_COLOR,
            );
        }

        if let Some(ref suggestion) = input.suggestion
            && suggestion.len() > input.text.len()
        {
            let ghost = &suggestion[input.text.len()..];
            let text_w =
                measure_text_width(font_system, &input.text, input_metrics, Family::Monospace)
                    as usize;
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                text_x + text_w,
                text_y,
                buf.height,
                ghost,
                input_metrics,
                SUGGESTION_COLOR,
                Family::Monospace,
            );
        }
    } else {
        let placeholder_x = text_x;

        if let Some(ref ai_cmd) = input.ai_suggestion {
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                text_x,
                text_y,
                buf.height,
                ai_cmd,
                input_metrics,
                AI_SUGGESTION_COLOR,
                Family::Monospace,
            );
            let cmd_w =
                measure_text_width(font_system, ai_cmd, input_metrics, Family::Monospace) as usize;
            let hint_metrics = Metrics::new(10.0 * sf, 14.0 * sf);
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                text_x + cmd_w + (8.0 * sf) as usize,
                text_y + (3.0 * sf) as usize,
                buf.height,
                "Tab",
                hint_metrics,
                SUGGESTION_COLOR,
                Family::Monospace,
            );
        } else {
            let placeholder = input.placeholder();
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                placeholder_x,
                text_y,
                buf.height,
                placeholder,
                input_metrics,
                SUGGESTION_COLOR,
                Family::Monospace,
            );
            if cursor_visible {
                let beam_w = (1.5 * sf).max(1.0) as usize;
                let cursor_h = (input_line_height * 0.85) as usize;
                let cursor_y = input_y + ((input_h as f32 - cursor_h as f32) / 2.0) as usize;
                let r = beam_w / 2;
                fill_rounded_rect(
                    buf,
                    placeholder_x,
                    cursor_y,
                    beam_w,
                    cursor_h,
                    r,
                    CURSOR_COLOR,
                );
            }
        }
    }

    PromptBarHitRects {
        ctx_bar: None,
        stop_button: stop_button_rect,
    }
}

/// Draw the slash command popup ABOVE the prompt bar.
/// Returns the height consumed so the caller knows the popup bounds.
pub fn draw_slash_menu(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    input: &InputFieldState,
    x_start: usize,
    prompt_y: usize,
    sf: f32,
) {
    if !input.slash_menu_open {
        return;
    }
    let filtered = input.filtered_slash_commands();
    if filtered.is_empty() {
        return;
    }

    let row_h = (34.0 * sf) as usize;
    let pad_x = (12.0 * sf) as usize;
    let pad_y = (6.0 * sf) as usize;
    let radius = (6.0 * sf) as usize;
    let menu_w = buf.width.saturating_sub(x_start * 2);
    let menu_h = pad_y * 2 + filtered.len() * row_h;

    let menu_y = prompt_y.saturating_sub(menu_h + (4.0 * sf) as usize);

    fill_rounded_rect(buf, x_start, menu_y, menu_w, menu_h, radius, SLASH_MENU_BG);
    stroke_rounded_rect(
        buf,
        x_start,
        menu_y,
        menu_w,
        menu_h,
        radius,
        sf,
        SLASH_MENU_BORDER,
    );

    let name_font_size = 13.0 * sf;
    let name_line_height = 18.0 * sf;
    let name_metrics = Metrics::new(name_font_size, name_line_height);

    let desc_font_size = 11.0 * sf;
    let desc_line_height = 16.0 * sf;
    let desc_metrics = Metrics::new(desc_font_size, desc_line_height);

    for (i, cmd) in filtered.iter().enumerate() {
        let item_y = menu_y + pad_y + i * row_h;

        if i == input.slash_selected {
            let hl_radius = (4.0 * sf) as usize;
            let hl_x = x_start + (4.0 * sf) as usize;
            let hl_w = menu_w.saturating_sub((8.0 * sf) as usize);
            fill_rounded_rect(
                buf,
                hl_x,
                item_y,
                hl_w,
                row_h,
                hl_radius,
                SLASH_MENU_SELECTED_BG,
            );
        }

        let text_y = item_y + ((row_h as f32 - name_line_height) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            x_start + pad_x,
            text_y,
            buf.height,
            cmd.name,
            name_metrics,
            SLASH_MENU_NAME,
            Family::Monospace,
        );

        let name_w = (cmd.name.chars().count() as f32 * CHAR_WIDTH_EST * sf) as usize;
        let desc_x = x_start + pad_x + name_w + (24.0 * sf) as usize;
        let desc_y = item_y + ((row_h as f32 - desc_line_height) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            desc_x,
            desc_y,
            buf.height,
            cmd.description,
            desc_metrics,
            SLASH_MENU_DESC,
            Family::Monospace,
        );
    }
}

pub fn stroke_rounded_rect(
    buf: &mut PixelBuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    r: usize,
    sf: f32,
    color: Rgb,
) {
    let bw = (1.0 * sf).max(1.0) as usize;
    if r == 0 || w <= r * 2 || h <= r * 2 {
        buf.fill_rect(x, y, w, bw, color);
        buf.fill_rect(x, y + h - bw, w, bw, color);
        buf.fill_rect(x, y, bw, h, color);
        buf.fill_rect(x + w - bw, y, bw, h, color);
        return;
    }
    buf.fill_rect(x + r, y, w - r * 2, bw, color);
    buf.fill_rect(x + r, y + h - bw, w - r * 2, bw, color);
    buf.fill_rect(x, y + r, bw, h - r * 2, color);
    buf.fill_rect(x + w - bw, y + r, bw, h - r * 2, color);
    for dy in 0..r {
        let rf = r as f32;
        let outer = rf - (rf * rf - (rf - dy as f32 - 0.5).powi(2)).sqrt().max(0.0);
        let o = outer.ceil() as usize;
        buf.fill_rect(x + o, y + dy, bw, 1, color);
        buf.fill_rect(x + w - 1 - o, y + dy, bw, 1, color);
        buf.fill_rect(x + o, y + h - 1 - dy, bw, 1, color);
        buf.fill_rect(x + w - 1 - o, y + h - 1 - dy, bw, 1, color);
    }
}

/// Format a `Duration` into a human-friendly string:
/// - `< 60s` → `12.34s`
/// - `< 1h`  → `3m 12.34s`
/// - `≥ 1h`  → `1h 18m 20.96s`
pub fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs_f64();
    if total_secs < 60.0 {
        format!("{:.2}s", total_secs)
    } else if total_secs < 3600.0 {
        let mins = (total_secs / 60.0).floor() as u64;
        let secs = total_secs - (mins as f64 * 60.0);
        format!("{}m {:.2}s", mins, secs)
    } else {
        let hours = (total_secs / 3600.0).floor() as u64;
        let remaining = total_secs - (hours as f64 * 3600.0);
        let mins = (remaining / 60.0).floor() as u64;
        let secs = remaining - (mins as f64 * 60.0);
        format!("{}h {}m {:.2}s", hours, mins, secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_seconds() {
        let d = Duration::from_secs_f64(3.14);
        assert_eq!(format_duration(d), "3.14s");
    }

    #[test]
    fn format_duration_sub_second() {
        let d = Duration::from_millis(250);
        assert_eq!(format_duration(d), "0.25s");
    }

    #[test]
    fn format_duration_zero() {
        let d = Duration::from_secs(0);
        assert_eq!(format_duration(d), "0.00s");
    }

    #[test]
    fn format_duration_minutes() {
        let d = Duration::from_secs(90);
        let s = format_duration(d);
        assert!(s.starts_with("1m"));
        assert!(s.contains("30.00s"));
    }

    #[test]
    fn format_duration_hours() {
        let d = Duration::from_secs(3661);
        let s = format_duration(d);
        assert!(s.starts_with("1h"));
        assert!(s.contains("1m"));
    }

    #[test]
    fn slash_commands_not_empty() {
        assert!(!SLASH_COMMANDS.is_empty());
    }

    #[test]
    fn all_slash_commands_start_with_slash() {
        for cmd in SLASH_COMMANDS {
            assert!(
                cmd.name.starts_with('/'),
                "Command '{}' missing leading /",
                cmd.name
            );
        }
    }

    #[test]
    fn all_slash_commands_have_descriptions() {
        for cmd in SLASH_COMMANDS {
            assert!(!cmd.description.is_empty());
        }
    }

    #[test]
    fn prompt_bar_height_positive() {
        let h = prompt_bar_height(2.0);
        assert!(h > 0);
    }

    #[test]
    fn prompt_bar_height_scales() {
        let h1 = prompt_bar_height(1.0);
        let h2 = prompt_bar_height(2.0);
        assert!(h2 > h1);
    }

    #[test]
    fn input_field_state_default() {
        let state = InputFieldState::new();
        assert!(state.text.is_empty());
        assert_eq!(state.cursor, 0);
        assert!(!state.slash_menu_open);
        assert!(state.suggestion.is_none());
        assert!(state.history_index.is_none());
        assert_eq!(state.input_mode, InputMode::Normal);
    }

    #[test]
    fn prompt_bar_hit_rects_default() {
        let rects = PromptBarHitRects {
            ctx_bar: None,
            stop_button: None,
        };
        assert!(rects.ctx_bar.is_none());
        assert!(rects.stop_button.is_none());
    }

    #[test]
    fn error_color_is_reddish() {
        assert!(theme::ERROR.0 > 150);
    }
}
