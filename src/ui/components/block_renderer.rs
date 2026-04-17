//! Renders the command block list for Smart prompt mode.
//!
//! Each `CommandBlock` is rendered as:
//! 1. Header line — abbreviated CWD + duration (plain text, no pills)
//! 2. Command text — `$ command` in bright color
//! 3. Output lines — plain monospace text
//! 4. Separator line (omitted for the last/bottom-most block)
//!
//! Blocks are rendered bottom-up (newest at the bottom, just above the
//! prompt bar), scrolling upward as history grows.

use cosmic_text::{Buffer, Family, FontSystem, Metrics, SwashCache};

use crate::blocks::{BlockList, StyledLine, StyledSpan};
use crate::renderer::gpu_grid::CellGlyph;
use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::draw_text_at_buffered;
use crate::renderer::theme;

use super::prompt_bar::format_duration;

/// Measure the actual monospace character width (in pixels) for the
/// output font metrics used in block view.  Falls back to
/// `CHAR_W_FALLBACK * sf` if measurement fails.
fn measure_char_width(font_system: &mut FontSystem, sf: f32) -> f32 {
    let out_font_size = 13.0 * sf;
    let out_line_height = LINE_H * sf;
    let metrics = Metrics::new(out_font_size, out_line_height);
    let w = crate::renderer::text::measure_text_width(font_system, "M", metrics, Family::Monospace);
    if w > 0.0 { w } else { CHAR_W_FALLBACK * sf }
}

/// Public wrapper: measure the monospace character width used for block
/// output text, so callers (e.g. hit_test, max_chars computation) can
/// use the same value that `draw()` uses.
pub fn output_char_width(font_system: &mut FontSystem, sf: f32) -> f32 {
    measure_char_width(font_system, sf)
}

/// Height of the header line (CWD + duration).
const HEADER_H: f32 = 18.0;
/// Height per output line.
const LINE_H: f32 = 18.0;
/// Command line height.
const CMD_H: f32 = 20.0;
/// Vertical gap between blocks (contains the separator).
const BLOCK_GAP: f32 = 10.0;
/// Padding inside block area.
const BLOCK_PAD_X: f32 = 10.0;
/// Internal vertical padding inside each block (top and bottom).
const BLOCK_PAD_Y: f32 = 10.0;
/// Fallback monospace character width (used only when font_system is
/// unavailable, e.g. in tests).  Actual rendering measures the real width.
const CHAR_W_FALLBACK: f32 = 7.0;

const HEADER_COLOR: Rgb = theme::FG_DIM;
const CMD_PREFIX_COLOR: Rgb = theme::PRIMARY;
const CMD_COLOR: Rgb = theme::FG_BRIGHT;
const SELECTED_BG: Rgb = theme::BG_ELEVATED;
const SELECTION_BG: Rgb = theme::BG_SELECTION;
const ERROR_ACCENT: Rgb = theme::ERROR;
const ERROR_BG: Rgb = theme::ERROR_BG;
const ERROR_HEADER_COLOR: Rgb = theme::ERROR_TEXT;
const SEPARATOR_COLOR: Rgb = theme::DIVIDER;
const LINK_COLOR: Rgb = theme::PRIMARY;
/// Subtle background for inline `code` spans.
const CODE_BG: Rgb = (24, 30, 28);
/// Color for horizontal rule lines.
const HR_COLOR: Rgb = (55, 57, 65);

const AGENT_ACCENT: Rgb = theme::AGENT_ACCENT;
const AGENT_BG: Rgb = theme::AGENT_BG;
const AGENT_HEADER_COLOR: Rgb = theme::AGENT_TEXT;
const AGENT_PREFIX_COLOR: Rgb = theme::AGENT_ACCENT;
/// Approval buttons use the app PRIMARY accent.
const APPROVAL_ACTIVE_BG: Rgb = theme::PRIMARY;

/// Font-size multiplier for heading levels (indexed by heading_level − 1).
const HEADING_SCALE: [f32; 3] = [1.45, 1.25, 1.1];
/// Extra vertical padding above a heading line (fraction of line height).
const HEADING_PAD_TOP: f32 = 0.35;

/// Height of the horizontal rule element (actual line is 1px inside this).
const HR_HEIGHT: f32 = 12.0;

/// Extra height reserved for the approval button row in agent tool-approval blocks.
const APPROVAL_ROW_H: f32 = 28.0;

const THINKING_PHRASES: &[&str] = &[
    "Warming up neurons",
    "Parsing context",
    "Unrolling thoughts",
    "Chaining tokens",
    "Sampling latent space",
    "Traversing embeddings",
    "Weighting attention",
    "Decoding intent",
    "Aligning vectors",
    "Compressing meaning",
    "Crystallizing response",
    "Braiding logic",
    "Sifting probabilities",
    "Fusing representations",
    "Distilling answer",
    "Mapping tensors",
    "Hydrating weights",
    "Calibrating layers",
    "Indexing vocabulary",
    "Wiring attention heads",
];

const THINKING_TEXT_COLOR: Rgb = theme::PRIMARY;

/// Height of the thinking indicator — one output line.
const THINKING_H: f32 = LINE_H;

/// Detect heading level from the first span of a styled line (0 = normal).
fn line_heading_level(line: &StyledLine) -> u8 {
    line.first().map(|s| s.heading_level).unwrap_or(0)
}

/// Whether this styled line is a horizontal rule.
fn line_is_hr(line: &StyledLine) -> bool {
    line.first().map(|s| s.horizontal_rule).unwrap_or(false)
}

/// Pixel height of a single output line, accounting for headings and HR.
fn output_line_height(line: &StyledLine, base_h: f32) -> f32 {
    if line_is_hr(line) {
        return HR_HEIGHT;
    }
    let hl = line_heading_level(line);
    if (1..=3).contains(&hl) {
        let scale = HEADING_SCALE[(hl - 1) as usize];
        base_h * scale + base_h * HEADING_PAD_TOP
    } else {
        base_h
    }
}

/// Wrap a styled line to fit within `max_chars` columns.
///
/// Splits at word boundaries when possible, otherwise hard-breaks.
/// Returns one or more visual lines, each a `Vec<StyledSpan>`.
fn wrap_styled_line(line: &StyledLine, max_chars: usize) -> Vec<StyledLine> {
    if max_chars == 0 {
        return vec![line.clone()];
    }

    let total: usize = line.iter().map(|s| s.text.chars().count()).sum();
    if total <= max_chars {
        return vec![line.clone()];
    }

    let full_text: String = line.iter().map(|s| s.text.as_str()).collect();

    let visual_lines = word_wrap(&full_text, max_chars);

    let mut result = Vec::with_capacity(visual_lines.len());
    let mut global_offset: usize = 0;

    for vline in &visual_lines {
        let vline_chars: usize = vline.chars().count();
        let vline_start = global_offset;
        let vline_end = global_offset + vline_chars;

        let mut spans_for_line: Vec<StyledSpan> = Vec::new();
        let mut span_offset: usize = 0;

        for span in line {
            let span_chars: usize = span.text.chars().count();
            let span_start = span_offset;
            let span_end = span_offset + span_chars;

            if span_end <= vline_start || span_start >= vline_end {
                span_offset = span_end;
                continue;
            }

            let slice_start = vline_start.max(span_start) - span_start;
            let slice_end = vline_end.min(span_end) - span_start;

            let sliced: String = span
                .text
                .chars()
                .skip(slice_start)
                .take(slice_end - slice_start)
                .collect();
            if !sliced.is_empty() {
                spans_for_line.push(StyledSpan {
                    text: sliced,
                    fg: span.fg,
                    bold: span.bold,
                    italic: span.italic,
                    underline: span.underline,
                    strikethrough: span.strikethrough,
                    code: span.code,
                    heading_level: span.heading_level,
                    horizontal_rule: span.horizontal_rule,
                });
            }

            span_offset = span_end;
        }

        if !spans_for_line.is_empty() {
            result.push(spans_for_line);
        }
        global_offset = vline_end;
    }

    if result.is_empty() {
        vec![line.clone()]
    } else {
        result
    }
}

/// Simple word-wrap: split `text` into lines of at most `max` characters.
/// Breaks at the last space before max, or hard-breaks if no space found.
pub fn word_wrap(text: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if len - i <= max {
            lines.push(chars[i..].iter().collect());
            break;
        }

        let window_end = i + max;
        let mut break_pos = None;
        for j in (i + 1..=window_end.min(len)).rev() {
            if j < len && chars[j] == ' ' {
                break_pos = Some(j);
                break;
            }
        }

        if let Some(bp) = break_pos {
            lines.push(chars[i..bp].iter().collect());
            i = bp + 1;
        } else {
            lines.push(chars[i..window_end].iter().collect());
            i = window_end;
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Count word-wrapped lines without allocating any Strings.
/// Same logic as `word_wrap` but only counts.
fn word_wrap_count(text: &str, max: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut count = 0usize;
    let mut i = 0;

    while i < len {
        count += 1;
        if len - i <= max {
            break;
        }

        let window_end = i + max;
        let mut break_pos = None;
        for j in (i + 1..=window_end.min(len)).rev() {
            if j < len && chars[j] == ' ' {
                break_pos = Some(j);
                break;
            }
        }

        if let Some(bp) = break_pos {
            i = bp + 1;
        } else {
            i = window_end;
        }
    }

    count.max(1)
}

/// Count the number of visual lines an output produces after word-wrapping.
/// Uses non-allocating `word_wrap_count` for performance on large outputs.
fn wrapped_output_lines(output: &[StyledLine], max_chars: usize) -> usize {
    if max_chars == 0 {
        return output.len();
    }
    output
        .iter()
        .map(|line| {
            let total: usize = line.iter().map(|s| s.text.chars().count()).sum();
            if total <= max_chars {
                1
            } else {
                let full_text: String = line.iter().map(|s| s.text.as_str()).collect();
                word_wrap_count(&full_text, max_chars)
            }
        })
        .sum()
}

/// Pixel height of block output content, accounting for headings and HR.
fn output_content_height(output: &[StyledLine], sf: f32, max_chars: usize) -> f32 {
    let base_h = LINE_H * sf;
    output
        .iter()
        .map(|line| {
            let visual_lines = if max_chars == 0 {
                1
            } else {
                let total: usize = line.iter().map(|s| s.text.chars().count()).sum();
                if total <= max_chars {
                    1
                } else {
                    let full_text: String = line.iter().map(|s| s.text.as_str()).collect();
                    word_wrap_count(&full_text, max_chars)
                }
            };
            let first = output_line_height(line, base_h);
            first + (visual_lines.saturating_sub(1)) as f32 * base_h
        })
        .sum()
}

/// Public version of `wrapped_output_lines` for use in select-all.
pub fn wrapped_output_line_count(output: &[StyledLine], max_chars: usize) -> usize {
    wrapped_output_lines(output, max_chars)
}

/// Get the plain text of a specific visual (wrapped) line within a block's output.
///
/// `visual_idx` is the zero-based index into the flattened, word-wrapped lines.
/// Returns `None` if the index is out of range.
pub fn visual_line_text(
    output: &[StyledLine],
    max_chars: usize,
    visual_idx: usize,
) -> Option<String> {
    let mut idx = 0usize;
    for styled_line in output {
        let wrapped = wrap_styled_line(styled_line, max_chars);
        for visual_line in &wrapped {
            if idx == visual_idx {
                let text: String = visual_line.iter().map(|s| s.text.as_str()).collect();
                return Some(text);
            }
            idx += 1;
        }
    }
    None
}

/// Split a span into sub-parts for link highlighting.
///
/// Given a span at character range `[span_start, span_end)` and a link at
/// `[link_cs, link_ce)`, return up to 3 (text, color) pieces:
/// before the link (original color), the link (LINK_COLOR), after the link (original color).
fn split_span_for_link(
    text: &str,
    original_fg: Rgb,
    span_start: usize,
    span_end: usize,
    link_cs: usize,
    link_ce: usize,
) -> Vec<(String, Rgb)> {
    if link_ce <= span_start || link_cs >= span_end {
        return vec![(text.to_string(), original_fg)];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut parts = Vec::new();

    let before_end = link_cs.saturating_sub(span_start);
    if before_end > 0 {
        let s: String = chars[..before_end].iter().collect();
        parts.push((s, original_fg));
    }

    let link_local_start = link_cs.max(span_start) - span_start;
    let link_local_end = link_ce.min(span_end) - span_start;
    if link_local_end > link_local_start {
        let s: String = chars[link_local_start..link_local_end].iter().collect();
        parts.push((s, LINK_COLOR));
    }

    let after_start = link_ce.saturating_sub(span_start).min(chars.len());
    if after_start < chars.len() {
        let s: String = chars[after_start..].iter().collect();
        parts.push((s, original_fg));
    }

    if parts.is_empty() {
        vec![(text.to_string(), original_fg)]
    } else {
        parts
    }
}

/// Compute the pixel height of a single block.
fn block_height(
    block: &crate::blocks::CommandBlock,
    sf: f32,
    is_last: bool,
    max_chars: usize,
) -> f32 {
    let pad_y = BLOCK_PAD_Y * sf * 2.0;
    let header = HEADER_H * sf;
    let cmd = CMD_H * sf;
    let content = if block.thinking {
        THINKING_H * sf
    } else {
        output_content_height(&block.output, sf, max_chars)
    };
    let pending = if block.pending_line.is_some() {
        LINE_H * sf
    } else {
        0.0
    };
    let approval = if matches!(
        &block.agent_step,
        Some(crate::blocks::AgentStepKind::ToolApproval { .. })
    ) {
        APPROVAL_ROW_H * sf
    } else {
        0.0
    };
    let gap = if is_last { 0.0 } else { BLOCK_GAP * sf };
    pad_y + header + cmd + content + pending + approval + gap
}

/// Total pixel height of all blocks.
pub fn total_height(blocks: &BlockList, sf: f32, max_chars: usize) -> f32 {
    let count = blocks.blocks.len();
    blocks
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| block_height(b, sf, i == count - 1, max_chars))
        .sum()
}

/// Build the header text for a block: abbreviated CWD.
/// Extracts the CWD from the first prompt segment (which has the 📂 prefix).
fn header_text(block: &crate::blocks::CommandBlock) -> String {
    if let Some(seg) = block.prompt.segments.first() {
        return seg.text.clone();
    }
    "~".to_string()
}

/// Convert a pixel position to a `BlockTextPos` within the block output.
///
/// Snaps to the closest valid position so selection keeps updating even
/// when the mouse is on headers, gaps, or outside block output areas.
pub fn hit_test(
    blocks: &BlockList,
    mx: f64,
    my: f64,
    y_start: usize,
    y_end: usize,
    pad: usize,
    x_offset: usize,
    buf_width: usize,
    sf: f32,
    char_w: f32,
) -> Option<crate::blocks::BlockTextPos> {
    if blocks.blocks.is_empty() {
        return None;
    }

    let available_h = y_end.saturating_sub(y_start);
    if available_h == 0 {
        return None;
    }

    let block_pad = (BLOCK_PAD_X * sf) as usize;
    let pad = pad + x_offset;
    let content_width = buf_width.saturating_sub(pad + block_pad + (pad - x_offset) + block_pad);
    let max_chars = if char_w > 0.0 {
        (content_width as f32 / char_w).floor() as usize
    } else {
        0
    };

    let total_h = total_height(blocks, sf, max_chars) as usize;
    let scroll = blocks.scroll_offset.max(0.0) as usize;

    let content_top = if total_h <= available_h {
        y_start as i32
    } else {
        y_start as i32 - (total_h - available_h) as i32 + scroll as i32
    };

    let block_pad_y = (BLOCK_PAD_Y * sf) as i32;
    let block_count = blocks.blocks.len();
    let line_h = (LINE_H * sf) as i32;

    let char_idx_from_mx = |mx: f64| -> usize {
        let rel_x = mx - (pad + block_pad) as f64;
        if rel_x < 0.0 {
            0
        } else if char_w > 0.0 {
            (rel_x / char_w as f64) as usize
        } else {
            0
        }
    };

    let mut best: Option<crate::blocks::BlockTextPos> = None;
    let mut y = content_top;

    for (bi, block) in blocks.blocks.iter().enumerate() {
        let is_last = bi == block_count - 1;
        let bh = block_height(block, sf, is_last, max_chars) as i32;
        let block_bottom = y + bh;

        if my < y as f64 && best.is_none() && !block.output.is_empty() && !block.thinking {
            return Some(crate::blocks::BlockTextPos {
                block_idx: bi,
                line_idx: 0,
                char_idx: 0,
            });
        }

        let header_cmd_top = y + block_pad_y;
        let output_top = header_cmd_top + (HEADER_H * sf) as i32 + (CMD_H * sf) as i32;

        if my >= header_cmd_top as f64
            && my < output_top as f64
            && !block.output.is_empty()
            && !block.thinking
        {
            return Some(crate::blocks::BlockTextPos {
                block_idx: bi,
                line_idx: 0,
                char_idx: char_idx_from_mx(mx),
            });
        }

        y = output_top;

        if !block.thinking && !block.output.is_empty() {
            let mut visual_line_idx = 0usize;
            let mut last_line_chars = 0usize;
            for styled_line in &block.output {
                let wrapped = wrap_styled_line(styled_line, max_chars);
                for visual_line in &wrapped {
                    let line_text: String = visual_line.iter().map(|s| s.text.as_str()).collect();
                    last_line_chars = line_text.chars().count();

                    if my >= y as f64 && my < (y + line_h) as f64 {
                        return Some(crate::blocks::BlockTextPos {
                            block_idx: bi,
                            line_idx: visual_line_idx,
                            char_idx: char_idx_from_mx(mx),
                        });
                    }

                    if my >= (y + line_h) as f64 {
                        best = Some(crate::blocks::BlockTextPos {
                            block_idx: bi,
                            line_idx: visual_line_idx,
                            char_idx: last_line_chars,
                        });
                    }

                    y += line_h;
                    visual_line_idx += 1;
                }
            }

            if my >= y as f64 && my < block_bottom as f64 {
                let total_vis = visual_line_idx;
                return Some(crate::blocks::BlockTextPos {
                    block_idx: bi,
                    line_idx: total_vis.saturating_sub(1),
                    char_idx: last_line_chars,
                });
            }
        }

        y = block_bottom;
    }

    best
}

/// Draw the block list into the pixel buffer.
///
/// `y_start` — first pixel row (below tab bar).
/// `y_end`   — last pixel row (above prompt bar).
/// `pad`     — horizontal padding in physical pixels.
/// `char_w`  — pre-measured monospace character width (avoids measuring each frame).
/// `height_cache` — cached block heights + cumulative line offsets for O(log n) culling.
///
/// Performance: uses binary search to find the first visible output line within each
/// block, and early-breaks when past the viewport. Only visible lines are processed
/// per frame regardless of total output size. Dirty tracking skips the entire draw
/// when nothing has changed since the previous frame.
pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    blocks: &BlockList,
    y_start: usize,
    y_end: usize,
    pad: usize,
    x_offset: usize,
    sf: f32,
    char_w: f32,
    selection: Option<&crate::blocks::BlockSelection>,
    hovered_link: Option<&crate::blocks::HoveredLink>,
    height_cache: &mut crate::renderer::BlockHeightCache,
    scrollbar_hovered: bool,
    overlay_active: bool,
    right_inset: usize,
    hide_last_running: bool,
) -> Vec<CellGlyph> {
    let content_area_w = buf.width.saturating_sub(right_inset);
    if blocks.blocks.is_empty() {
        let available_h = y_end.saturating_sub(y_start);
        if available_h > 0 {
            buf.fill_rect(
                x_offset,
                y_start,
                content_area_w.saturating_sub(x_offset),
                available_h,
                crate::renderer::theme::BG,
            );
        }
        return Vec::new();
    }

    let available_h = y_end.saturating_sub(y_start);
    if available_h == 0 {
        return Vec::new();
    }

    let block_pad = (BLOCK_PAD_X * sf) as usize;
    let right_pad = pad;
    let pad = pad + x_offset;
    let content_width = content_area_w.saturating_sub(pad + block_pad + right_pad + block_pad);
    let max_chars = if char_w > 0.0 {
        (content_width as f32 / char_w).floor() as usize
    } else {
        0
    };

    if height_cache.generation != blocks.generation || height_cache.max_chars != max_chars {
        let count = blocks.blocks.len();
        height_cache.block_heights.clear();
        height_cache.block_heights.reserve(count);
        height_cache.block_cum_lines.clear();
        height_cache.block_cum_lines.reserve(count);
        let mut total_h = 0.0f32;

        for (idx, block) in blocks.blocks.iter().enumerate() {
            let mut cum = Vec::with_capacity(block.output.len());
            let mut cum_total = 0u32;
            for line in &block.output {
                let wc = if max_chars == 0 || line.is_empty() {
                    1u32
                } else {
                    let char_count: usize = line.iter().map(|s| s.text.chars().count()).sum();
                    if char_count <= max_chars {
                        1
                    } else {
                        let full_text: String = line.iter().map(|s| s.text.as_str()).collect();
                        word_wrap_count(&full_text, max_chars) as u32
                    }
                };
                cum_total += wc;
                cum.push(cum_total);
            }

            let is_last = idx == count - 1;
            let pad_y = BLOCK_PAD_Y * sf * 2.0;
            let header = HEADER_H * sf;
            let cmd = CMD_H * sf;
            let content = if block.thinking {
                THINKING_H * sf
            } else {
                output_content_height(&block.output, sf, max_chars)
            };
            let pending = if block.pending_line.is_some() {
                LINE_H * sf
            } else {
                0.0
            };
            let approval = if matches!(
                &block.agent_step,
                Some(crate::blocks::AgentStepKind::ToolApproval { .. })
            ) {
                APPROVAL_ROW_H * sf
            } else {
                0.0
            };
            let gap = if is_last { 0.0 } else { BLOCK_GAP * sf };
            let h = pad_y + header + cmd + content + pending + approval + gap;

            height_cache.block_heights.push(h);
            height_cache.block_cum_lines.push(cum);
            total_h += h;
        }

        height_cache.total_height = total_h;
        height_cache.generation = blocks.generation;
        height_cache.max_chars = max_chars;
        height_cache.pixels_valid = false;
    }

    let sel_gen = selection.map_or(0u64, |s| {
        let a = s.anchor;
        let b = s.head;
        (a.block_idx as u64 * 1000000 + a.line_idx as u64 * 1000 + a.char_idx as u64)
            ^ (b.block_idx as u64 * 1000000 + b.line_idx as u64 * 1000 + b.char_idx as u64)
                .wrapping_mul(0x9e3779b97f4a7c15)
    });
    let link_gen = hovered_link.map_or(0u64, |l| {
        l.block_idx as u64 * 1000000 + l.visual_line_idx as u64 * 1000 + l.char_start as u64
    });
    let any_pending = blocks
        .blocks
        .iter()
        .any(|b| b.pending_line.is_some() || b.thinking);
    let any_running = blocks.blocks.iter().any(|b| b.is_running());

    let dirty = !height_cache.pixels_valid
        || blocks.scroll_offset != height_cache.last_scroll
        || sel_gen != height_cache.last_selection_gen
        || link_gen != height_cache.last_link_gen
        || scrollbar_hovered != height_cache.last_scrollbar_hovered
        || content_area_w != height_cache.last_buf_width
        || available_h != height_cache.last_available_h
        || any_pending != height_cache.last_any_pending
        || any_pending
        || any_running
        || overlay_active != height_cache.last_overlay_active
        || overlay_active
        || hide_last_running != height_cache.last_hide_last;

    if !dirty {
        return height_cache.cached_glyphs.clone();
    }

    height_cache.last_scroll = blocks.scroll_offset;
    height_cache.last_selection_gen = sel_gen;
    height_cache.last_link_gen = link_gen;
    height_cache.last_scrollbar_hovered = scrollbar_hovered;
    height_cache.last_buf_width = content_area_w;
    height_cache.last_available_h = available_h;
    height_cache.last_any_pending = any_pending;
    height_cache.last_overlay_active = overlay_active;
    height_cache.last_hide_last = hide_last_running;

    buf.fill_rect(
        x_offset,
        y_start,
        content_area_w.saturating_sub(x_offset),
        available_h,
        crate::renderer::theme::BG,
    );

    let total_h = {
        let mut th = height_cache.total_height;
        if hide_last_running
            && let Some(last_h) = height_cache.block_heights.last()
            && blocks.blocks.last().is_some_and(|b| b.is_running())
        {
            th -= last_h;
        }
        th as usize
    };
    let scroll = blocks.scroll_offset.max(0.0) as usize;

    let content_top = if total_h <= available_h {
        y_start as i32
    } else {
        y_start as i32 - (total_h - available_h) as i32 + scroll as i32
    };

    let mut y = content_top;

    let header_font_size = 11.0 * sf;
    let header_line_height = HEADER_H * sf;
    let header_metrics = Metrics::new(header_font_size, header_line_height);

    let cmd_font_size = 14.0 * sf;
    let cmd_line_height = CMD_H * sf;

    let out_font_size = 13.0 * sf;
    let out_line_height = LINE_H * sf;
    let out_metrics = Metrics::new(out_font_size, out_line_height);

    let block_pad_y = (BLOCK_PAD_Y * sf) as i32;
    let gap = (BLOCK_GAP * sf) as usize;
    let block_count = blocks.blocks.len();
    let visible_block_count =
        if hide_last_running && blocks.blocks.last().is_some_and(|b| b.is_running()) {
            block_count.saturating_sub(1)
        } else {
            block_count
        };
    let line_h_i32 = (LINE_H * sf) as i32;

    let mut text_buf = Buffer::new(font_system, out_metrics);

    let mut glyphs: Vec<CellGlyph> = Vec::with_capacity(2048);

    /// Collect characters from text into GPU glyph list (monospace).
    /// Uses f32 x-tracking to avoid fractional truncation drift.
    #[inline]
    fn collect_glyphs(
        glyphs: &mut Vec<CellGlyph>,
        text: &str,
        x: &mut f32,
        py: usize,
        fg: Rgb,
        char_w: f32,
        font_size: f32,
        line_height: f32,
        y_start: usize,
        y_end: usize,
        bold: bool,
        italic: bool,
    ) {
        if py < y_start || py >= y_end {
            *x += text.chars().count() as f32 * char_w;
            return;
        }
        for ch in text.chars() {
            if !ch.is_whitespace() && !ch.is_control() {
                glyphs.push(CellGlyph {
                    px: *x as usize,
                    py,
                    ch,
                    fg,
                    font_size,
                    line_height,
                    bold,
                    italic,
                });
            }
            *x += char_w;
        }
    }

    for (i, block) in blocks.blocks.iter().enumerate() {
        if i >= visible_block_count {
            break;
        }
        let is_last = i == visible_block_count - 1;
        if i >= height_cache.block_heights.len() {
            break; // Cache stale after session switch — skip until next revalidation
        }
        let bh = height_cache.block_heights[i] as i32;
        let block_bottom = y + bh;

        if y >= y_end as i32 {
            break;
        }
        if block_bottom <= y_start as i32 {
            y = block_bottom;
            continue;
        }

        let is_agent = block.command.starts_with("/agent");

        if is_agent && !block.is_error {
            let bg_y = y.max(y_start as i32) as usize;
            let bg_bottom = if is_last {
                block_bottom.min(y_end as i32) as usize
            } else {
                (block_bottom - gap as i32 / 2).min(y_end as i32).max(0) as usize
            };
            let bg_h = bg_bottom.saturating_sub(bg_y);
            buf.fill_rect(
                pad,
                bg_y,
                content_area_w.saturating_sub(pad + right_pad),
                bg_h,
                AGENT_BG,
            );
            buf.fill_rect(pad, bg_y, (3.0 * sf).max(2.0) as usize, bg_h, AGENT_ACCENT);
        } else if block.is_error {
            let bg_y = y.max(y_start as i32) as usize;
            let bg_bottom = if is_last {
                block_bottom.min(y_end as i32) as usize
            } else {
                (block_bottom - gap as i32 / 2).min(y_end as i32).max(0) as usize
            };
            let bg_h = bg_bottom.saturating_sub(bg_y);
            buf.fill_rect(
                pad,
                bg_y,
                content_area_w.saturating_sub(pad + right_pad),
                bg_h,
                ERROR_BG,
            );
            buf.fill_rect(pad, bg_y, (3.0 * sf).max(2.0) as usize, bg_h, ERROR_ACCENT);
        } else if block.selected {
            let bg_y = y.max(y_start as i32) as usize;
            let bg_bottom = if is_last {
                block_bottom.min(y_end as i32) as usize
            } else {
                (block_bottom - gap as i32 / 2).min(y_end as i32).max(0) as usize
            };
            buf.fill_rect(
                x_offset,
                bg_y,
                content_area_w.saturating_sub(x_offset),
                bg_bottom.saturating_sub(bg_y),
                SELECTED_BG,
            );
        }

        let mut line_y = y + block_pad_y;

        if line_y >= y_start as i32 && line_y < y_end as i32 {
            let hdr = header_text(block);
            let elapsed = block.elapsed();
            let label = format!("{} ({})", hdr, format_duration(elapsed));
            let hdr_color = if block.is_error {
                ERROR_HEADER_COLOR
            } else if is_agent {
                AGENT_HEADER_COLOR
            } else {
                HEADER_COLOR
            };
            draw_text_at_buffered(
                buf,
                font_system,
                swash_cache,
                &mut text_buf,
                pad + block_pad,
                line_y as usize,
                buf.height,
                &label,
                header_metrics,
                hdr_color,
                Family::Monospace,
            );
        }
        line_y += (HEADER_H * sf) as i32;

        if line_y >= y_start as i32 && line_y < y_end as i32 {
            let (prefix, prefix_color) = if block.is_error {
                ("$ ", ERROR_ACCENT)
            } else if is_agent {
                ("$ ", AGENT_PREFIX_COLOR)
            } else {
                ("$ ", CMD_PREFIX_COLOR)
            };
            let mut x = (pad + block_pad) as f32;
            collect_glyphs(
                &mut glyphs,
                prefix,
                &mut x,
                line_y as usize,
                prefix_color,
                char_w,
                cmd_font_size,
                cmd_line_height,
                y_start,
                y_end,
                true,
                false,
            );
            collect_glyphs(
                &mut glyphs,
                &block.command,
                &mut x,
                line_y as usize,
                CMD_COLOR,
                char_w,
                cmd_font_size,
                cmd_line_height,
                y_start,
                y_end,
                false,
                false,
            );
        }
        line_y += (CMD_H * sf) as i32;

        if block.thinking {
            if line_y >= y_start as i32 && line_y < y_end as i32 {
                let elapsed_ms = block.started.elapsed().as_millis();
                let area_h = (THINKING_H * sf) as usize;

                let dots_x = pad + block_pad;
                let dots_h = super::awebo::height(sf);
                let dots_y = line_y as usize + area_h.saturating_sub(dots_h) / 2;
                super::awebo::draw(buf, dots_x, dots_y, sf, elapsed_ms, THINKING_TEXT_COLOR);

                let dots_w = super::awebo::width(sf);
                let offset = block
                    .command
                    .as_bytes()
                    .iter()
                    .fold(0usize, |h, &b| h.wrapping_mul(31).wrapping_add(b as usize));
                let phrase_idx = (offset + elapsed_ms as usize / 2500) % THINKING_PHRASES.len();
                let phrase = THINKING_PHRASES[phrase_idx];
                let label = format!("  {phrase}");
                draw_text_at_buffered(
                    buf,
                    font_system,
                    swash_cache,
                    &mut text_buf,
                    dots_x + dots_w + (2.0 * sf) as usize,
                    line_y as usize,
                    buf.height,
                    &label,
                    out_metrics,
                    THINKING_TEXT_COLOR,
                    Family::Monospace,
                );
            }
        } else {
            let cum = &height_cache.block_cum_lines[i];
            if !cum.is_empty() {
                let output_start_y = line_y;

                let skip_visual = if output_start_y < y_start as i32 {
                    ((y_start as i32 - output_start_y) / line_h_i32) as u32
                } else {
                    0
                };

                let first_line = if skip_visual > 0 {
                    cum.partition_point(|&c| c <= skip_visual)
                } else {
                    0
                };

                let mut visual_line_idx = if first_line > 0 {
                    let jumped = cum[first_line - 1];
                    line_y = output_start_y + jumped as i32 * line_h_i32;
                    jumped as usize
                } else {
                    0usize
                };

                for styled_line in block.output[first_line..].iter() {
                    if line_y >= y_end as i32 {
                        break;
                    }

                    let hl = line_heading_level(styled_line);
                    let is_hr = line_is_hr(styled_line);

                    if is_hr {
                        let hr_h = (HR_HEIGHT * sf) as i32;
                        if line_y >= y_start as i32 && line_y < y_end as i32 {
                            let rule_y = (line_y + hr_h / 2) as usize;
                            let rule_x = pad + block_pad;
                            let rule_w = content_width;
                            buf.fill_rect(
                                rule_x,
                                rule_y,
                                rule_w,
                                (1.0 * sf).max(1.0) as usize,
                                HR_COLOR,
                            );
                        }
                        line_y += hr_h;
                        visual_line_idx += 1;
                        continue;
                    }

                    let (span_font_size, span_line_height, extra_top) = if (1..=3).contains(&hl) {
                        let scale = HEADING_SCALE[(hl - 1) as usize];
                        let fs = out_font_size * scale;
                        let lh = out_line_height * scale;
                        let top = (out_line_height * HEADING_PAD_TOP) as i32;
                        (fs, lh, top)
                    } else {
                        (out_font_size, out_line_height, 0i32)
                    };
                    let cur_line_h = if (1..=3).contains(&hl) {
                        (span_line_height as i32) + extra_top
                    } else {
                        line_h_i32
                    };

                    line_y += extra_top;

                    let wrapped = wrap_styled_line(styled_line, max_chars);
                    for (wrap_idx, visual_line) in wrapped.iter().enumerate() {
                        if line_y >= y_end as i32 {
                            break;
                        }
                        let (vl_font_size, vl_line_height, vl_h) = if wrap_idx == 0 {
                            (span_font_size, span_line_height, cur_line_h)
                        } else {
                            (out_font_size, out_line_height, line_h_i32)
                        };

                        if line_y >= y_start as i32 && !visual_line.is_empty() {
                            let line_text: String =
                                visual_line.iter().map(|s| s.text.as_str()).collect();
                            let line_char_count = line_text.chars().count();
                            let base_x = (pad + block_pad) as f32;

                            if let Some(sel) = selection {
                                let ss = sel.start();
                                let se = sel.end();
                                let on_line = (i > ss.block_idx
                                    || (i == ss.block_idx && visual_line_idx >= ss.line_idx))
                                    && (i < se.block_idx
                                        || (i == se.block_idx && visual_line_idx <= se.line_idx));
                                if on_line {
                                    let is_first =
                                        i == ss.block_idx && visual_line_idx == ss.line_idx;
                                    let is_last_s =
                                        i == se.block_idx && visual_line_idx == se.line_idx;
                                    let cs =
                                        if is_first { ss.char_idx } else { 0 }.min(line_char_count);
                                    let ce = if is_last_s {
                                        se.char_idx
                                    } else {
                                        line_char_count
                                    }
                                    .min(line_char_count);
                                    let extend = !is_last_s;
                                    if ce > cs || extend {
                                        let sx = (base_x + cs as f32 * char_w) as usize;
                                        let sw = if extend {
                                            content_width
                                                .saturating_sub((cs as f32 * char_w) as usize)
                                        } else {
                                            ((ce - cs) as f32 * char_w) as usize
                                        };
                                        buf.fill_rect(
                                            sx,
                                            line_y as usize,
                                            sw,
                                            vl_h as usize,
                                            SELECTION_BG,
                                        );
                                    }
                                }
                            }

                            let mut x = base_x;
                            let link_range: Option<(usize, usize)> = hovered_link.and_then(|hl| {
                                if hl.block_idx == i && hl.visual_line_idx == visual_line_idx {
                                    Some((hl.char_start, hl.char_end))
                                } else {
                                    None
                                }
                            });

                            let mut char_offset = 0usize;
                            for span in visual_line {
                                if span.text.is_empty() {
                                    continue;
                                }
                                let span_chars = span.text.chars().count();
                                let span_start = char_offset;
                                let span_end = char_offset + span_chars;

                                if span.code && line_y >= y_start as i32 && line_y < y_end as i32 {
                                    let code_x = x as usize;
                                    let code_w = (span_chars as f32 * char_w) as usize;
                                    buf.fill_rect(
                                        code_x,
                                        line_y as usize,
                                        code_w,
                                        vl_h as usize,
                                        CODE_BG,
                                    );
                                }

                                if let Some((link_cs, link_ce)) = link_range {
                                    for (pt, pf) in &split_span_for_link(
                                        &span.text, span.fg, span_start, span_end, link_cs, link_ce,
                                    ) {
                                        if pt.is_empty() {
                                            continue;
                                        }
                                        collect_glyphs(
                                            &mut glyphs,
                                            pt,
                                            &mut x,
                                            line_y as usize,
                                            *pf,
                                            char_w,
                                            vl_font_size,
                                            vl_line_height,
                                            y_start,
                                            y_end,
                                            span.bold,
                                            span.italic,
                                        );
                                    }
                                } else {
                                    collect_glyphs(
                                        &mut glyphs,
                                        &span.text,
                                        &mut x,
                                        line_y as usize,
                                        span.fg,
                                        char_w,
                                        vl_font_size,
                                        vl_line_height,
                                        y_start,
                                        y_end,
                                        span.bold,
                                        span.italic,
                                    );
                                }

                                if span.underline
                                    && line_y >= y_start as i32
                                    && line_y < y_end as i32
                                {
                                    let ux = (base_x + char_offset as f32 * char_w) as usize;
                                    let uw = (span_chars as f32 * char_w) as usize;
                                    let uy = line_y as usize + vl_h as usize
                                        - (1.5 * sf).max(1.0) as usize;
                                    buf.fill_rect(
                                        ux,
                                        uy,
                                        uw,
                                        (1.0 * sf).max(1.0) as usize,
                                        span.fg,
                                    );
                                }

                                if span.strikethrough
                                    && line_y >= y_start as i32
                                    && line_y < y_end as i32
                                {
                                    let sx = (base_x + char_offset as f32 * char_w) as usize;
                                    let sw = (span_chars as f32 * char_w) as usize;
                                    let sy = line_y as usize + (vl_h as usize / 2);
                                    buf.fill_rect(
                                        sx,
                                        sy,
                                        sw,
                                        (1.0 * sf).max(1.0) as usize,
                                        span.fg,
                                    );
                                }

                                char_offset = span_end;
                            }

                            if let Some((lcs, lce)) = link_range {
                                let ux = (base_x + lcs as f32 * char_w) as usize;
                                let uw = ((lce - lcs) as f32 * char_w) as usize;
                                let uy =
                                    line_y as usize + vl_h as usize - (1.5 * sf).max(1.0) as usize;
                                buf.fill_rect(ux, uy, uw, (1.0 * sf).max(1.0) as usize, LINK_COLOR);
                            }
                        }
                        line_y += vl_h;
                        visual_line_idx += 1;
                    }
                }
            }
        }

        if let Some(pending) = &block.pending_line
            && line_y >= y_start as i32
            && line_y < y_end as i32
            && !pending.is_empty()
        {
            let mut x = (pad + block_pad) as f32;
            for span in pending {
                if span.text.is_empty() {
                    continue;
                }
                collect_glyphs(
                    &mut glyphs,
                    &span.text,
                    &mut x,
                    line_y as usize,
                    span.fg,
                    char_w,
                    out_font_size,
                    out_line_height,
                    y_start,
                    y_end,
                    span.bold,
                    span.italic,
                );
            }
        }

        if let Some(crate::blocks::AgentStepKind::ToolApproval {
            selected_option, ..
        }) = &block.agent_step
        {
            let row_h = (APPROVAL_ROW_H * sf) as i32;
            let btn_y = line_y + (4.0 * sf) as i32;
            let btn_h = (row_h - (8.0 * sf) as i32).max(1);
            let sel = *selected_option;

            if btn_y >= y_start as i32 && btn_y < y_end as i32 {
                let labels = [
                    "  [Enter] Approve  ",
                    "  [A] Always approve  ",
                    "  [Esc] Reject  ",
                ];
                let mut bx = (pad + block_pad) as i32;
                let btn_gap = (6.0 * sf) as i32;

                for (idx, label) in labels.iter().enumerate() {
                    let w = (label.chars().count() as f32 * char_w) as i32;
                    let is_sel = idx == sel;

                    let bg_color = if is_sel {
                        APPROVAL_ACTIVE_BG
                    } else {
                        theme::AGENT_BUTTON_BG
                    };
                    buf.fill_rect(
                        bx as usize,
                        btn_y as usize,
                        w as usize,
                        btn_h as usize,
                        bg_color,
                    );

                    if !is_sel {
                        let bw = (1.0 * sf).max(1.0) as usize;
                        let border_c = theme::BORDER;
                        buf.fill_rect(bx as usize, btn_y as usize, w as usize, bw, border_c);
                        buf.fill_rect(
                            bx as usize,
                            (btn_y + btn_h - bw as i32).max(0) as usize,
                            w as usize,
                            bw,
                            border_c,
                        );
                        buf.fill_rect(bx as usize, btn_y as usize, bw, btn_h as usize, border_c);
                        buf.fill_rect(
                            (bx + w - bw as i32).max(0) as usize,
                            btn_y as usize,
                            bw,
                            btn_h as usize,
                            border_c,
                        );
                    }

                    let text_color = if is_sel {
                        (255, 255, 255)
                    } else {
                        theme::FG_SECONDARY
                    };
                    let text_y = btn_y + (btn_h - (out_line_height as i32)) / 2;
                    let mut gx = bx as f32;
                    collect_glyphs(
                        &mut glyphs,
                        label,
                        &mut gx,
                        text_y as usize,
                        text_color,
                        char_w,
                        out_font_size,
                        out_line_height,
                        y_start,
                        y_end,
                        is_sel,
                        false,
                    );

                    bx += w + btn_gap;
                }
            }
            line_y += row_h;
            let _ = line_y; // consumed by block_bottom arithmetic
        }

        if !is_last {
            let sep_y = (block_bottom - gap as i32 / 2).max(0) as usize;
            if sep_y > y_start && sep_y < y_end {
                buf.fill_rect(
                    pad,
                    sep_y,
                    content_area_w.saturating_sub(pad + right_pad),
                    (1.0 * sf).max(1.0) as usize,
                    SEPARATOR_COLOR,
                );
            }
        }

        y = block_bottom;
    }

    if total_h > available_h {
        let geom = scrollbar_geometry(content_area_w, y_start, available_h, total_h, scroll, sf);
        let sc: Rgb = if scrollbar_hovered {
            (255, 255, 255)
        } else {
            (80, 84, 96)
        };
        buf.fill_rect(geom.track_x, geom.thumb_y, geom.width, geom.thumb_h, sc);
    }

    height_cache.pixels_valid = true;
    height_cache.cached_glyphs = glyphs.clone();
    glyphs
}

/// Scrollbar geometry for hit-testing and rendering.
pub struct ScrollbarGeometry {
    pub track_x: usize,
    pub width: usize,
    pub track_h: usize,
    pub thumb_y: usize,
    pub thumb_h: usize,
    pub max_scroll: usize,
}

/// Compute scrollbar geometry given viewport and content metrics.
pub fn scrollbar_geometry(
    buf_width: usize,
    y_start: usize,
    available_h: usize,
    total_h: usize,
    scroll: usize,
    sf: f32,
) -> ScrollbarGeometry {
    let scrollbar_w = (6.0 * sf).max(4.0) as usize;
    let scrollbar_margin = (2.0 * sf) as usize;
    let track_x = buf_width.saturating_sub(scrollbar_w + scrollbar_margin);
    let track_h = available_h;

    let thumb_h =
        ((available_h as f64 / total_h as f64) * track_h as f64).max(20.0 * sf as f64) as usize;

    let max_scroll = total_h.saturating_sub(available_h);
    let scroll_frac = if max_scroll > 0 {
        scroll as f64 / max_scroll as f64
    } else {
        0.0
    };
    let thumb_y =
        y_start + ((1.0 - scroll_frac) * (track_h.saturating_sub(thumb_h)) as f64) as usize;

    ScrollbarGeometry {
        track_x,
        width: scrollbar_w,
        track_h,
        thumb_y,
        thumb_h,
        max_scroll,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::BlockList;
    use crate::prompt::{PromptInfo, PromptSegment};
    use std::time::Instant;

    fn make_block(output_lines: usize, thinking: bool) -> crate::blocks::CommandBlock {
        crate::blocks::CommandBlock {
            prompt: PromptInfo::default(),
            command: String::new(),
            output: vec![vec![]; output_lines],
            started: Instant::now(),
            duration: Some(std::time::Duration::from_secs(0)),
            selected: false,
            thinking,
            is_error: false,
            exit_code: None,
            pending_line: None,
            checkpoint_at_start: 0,
            restored: false,
            agent_step: None,
        }
    }

    #[test]
    fn block_height_no_output() {
        let b = make_block(0, false);
        let h = block_height(&b, 1.0, true, 0);
        assert!(h > 0.0);
    }

    #[test]
    fn block_height_with_output() {
        let b0 = make_block(0, false);
        let b5 = make_block(5, false);
        let h0 = block_height(&b0, 1.0, true, 0);
        let h5 = block_height(&b5, 1.0, true, 0);
        assert!(h5 > h0);
    }

    #[test]
    fn block_height_thinking() {
        let b = make_block(0, true);
        let h = block_height(&b, 1.0, true, 0);
        assert!(h > 0.0);
    }

    #[test]
    fn block_height_not_last_adds_gap() {
        let b = make_block(1, false);
        let last = block_height(&b, 1.0, true, 0);
        let not_last = block_height(&b, 1.0, false, 0);
        assert!(not_last > last);
    }

    #[test]
    fn total_height_empty() {
        let bl = BlockList::new();
        assert_eq!(total_height(&bl, 1.0, 0), 0.0);
    }

    #[test]
    fn total_height_multiple_blocks() {
        let mut bl = BlockList::new();
        bl.push_command(PromptInfo::default(), "ls".into());
        bl.push_command(PromptInfo::default(), "pwd".into());
        let h = total_height(&bl, 1.0, 0);
        assert!(h > 0.0);
    }

    #[test]
    fn header_text_with_cwd_segment() {
        let mut b = make_block(0, false);
        b.prompt.segments.push(PromptSegment {
            kind: crate::prompt::SegmentKind::Cwd,
            text: "~/projects".to_string(),
            fg: (0, 0, 0),
        });
        assert_eq!(header_text(&b), "~/projects");
    }

    #[test]
    fn header_text_empty_prompt() {
        let b = make_block(0, false);
        assert_eq!(header_text(&b), "~");
    }

    #[test]
    fn error_block_has_red_prefix_color() {
        assert_eq!(ERROR_ACCENT, (200, 60, 60));
    }

    #[test]
    fn word_wrap_short_line_unchanged() {
        let result = word_wrap("hello world", 80);
        assert_eq!(result, vec!["hello world"]);
    }

    #[test]
    fn word_wrap_breaks_at_space() {
        let result = word_wrap("hello world foo", 11);
        assert_eq!(result, vec!["hello world", "foo"]);
    }

    #[test]
    fn word_wrap_hard_break_no_space() {
        let result = word_wrap("abcdefghij", 5);
        assert_eq!(result, vec!["abcde", "fghij"]);
    }

    #[test]
    fn wrap_styled_line_no_wrap_needed() {
        let line = vec![StyledSpan::plain("short".into(), (255, 255, 255))];
        let wrapped = wrap_styled_line(&line, 80);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0][0].text, "short");
    }

    #[test]
    fn wrap_styled_line_splits_long_line() {
        let line = vec![StyledSpan::plain(
            "this is a long line that should be wrapped".into(),
            (200, 200, 200),
        )];
        let wrapped = wrap_styled_line(&line, 20);
        assert!(wrapped.len() >= 2);
        for vline in &wrapped {
            let len: usize = vline.iter().map(|s| s.text.chars().count()).sum();
            assert!(len <= 20);
        }
    }

    #[test]
    fn wrap_styled_line_preserves_colors() {
        let line = vec![
            StyledSpan::plain("red text ".into(), (255, 0, 0)),
            StyledSpan::plain("blue text".into(), (0, 0, 255)),
        ];
        let wrapped = wrap_styled_line(&line, 12);
        assert!(wrapped.len() >= 2);
        assert_eq!(wrapped[0].last().unwrap().fg, (255, 0, 0));
    }

    #[test]
    fn wrapped_output_lines_counts_correctly() {
        let output = vec![
            vec![StyledSpan::plain("short".into(), (255, 255, 255))],
            vec![StyledSpan::plain(
                "this is a very long line indeed".into(),
                (200, 200, 200),
            )],
        ];
        let count = wrapped_output_lines(&output, 15);
        assert!(count > 2);
    }
}
