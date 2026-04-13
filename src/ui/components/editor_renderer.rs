//! Editor rendering: text, hex, and image viewers.

use cosmic_text::{Buffer, Family, FontSystem, Metrics, SwashCache};

use crate::renderer::glyph_atlas::GlyphAtlas;
use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at_buffered;
use crate::renderer::theme;
use crate::ui::editor::{
    DiffLineKind, EditorMode, EditorState,
    HEX_FONT_SIZE, HEX_LINE_HEIGHT, HEX_PAD_X, HEX_PAD_Y,
    TEXT_FONT_SIZE, TEXT_LINE_HEIGHT, TEXT_PAD_X, TEXT_PAD_Y, GUTTER_PAD_RIGHT,
};


const GUTTER_BG: (u8, u8, u8) = theme::BG;
const GUTTER_TEXT: (u8, u8, u8) = theme::FG_MUTED;
const GUTTER_ACTIVE_TEXT: (u8, u8, u8) = theme::EDITOR_GUTTER_ACTIVE;
const LINE_TEXT: (u8, u8, u8) = theme::FG_PRIMARY;
const CURRENT_LINE_BG: (u8, u8, u8) = theme::EDITOR_CURRENT_LINE_BG;
const SELECTION_BG: (u8, u8, u8) = theme::BG_SELECTION;
const CURSOR_COLOR: (u8, u8, u8) = theme::EDITOR_CURSOR;

const DIFF_ADDED_BG: (u8, u8, u8) = (30, 60, 30);
const DIFF_REMOVED_BG: (u8, u8, u8) = (60, 25, 25);
const DIFF_ADDED_GUTTER: (u8, u8, u8) = (50, 160, 60);
const DIFF_REMOVED_GUTTER: (u8, u8, u8) = (200, 60, 60);

const HEX_ADDR_COLOR: (u8, u8, u8) = theme::FG_MUTED;
const HEX_BYTE_COLOR: (u8, u8, u8) = theme::FG_PRIMARY;
const HEX_ASCII_COLOR: (u8, u8, u8) = theme::FG_SECONDARY;
const HEX_NULL_COLOR: (u8, u8, u8) = theme::FG_DIM;

const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_MIN_THUMB: f32 = 20.0;
const SCROLLBAR_COLOR: (u8, u8, u8) = theme::SCROLLBAR_THUMB;


/// Convert a column (byte offset) to a pixel X offset using monospace char width.
fn col_to_pixel_x(line: &str, col: usize, char_w: usize) -> usize {
    let safe_col = col.min(line.len());
    let chars_before = line[..safe_col].chars().count();
    chars_before * char_w
}


/// Computed viewport rectangle in physical pixels.
struct Viewport {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}


/// Draw the editor content into the pixel buffer.
pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    glyph_atlas: &mut GlyphAtlas,
    state: &EditorState,
    y_start: usize,
    content_h: usize,
    x_start: usize,
    content_w: usize,
    sf: f32,
    scrollbar_state: ScrollbarHit,
    cursor_visible: bool,
) {
    let vp = Viewport { x: x_start, y: y_start, w: content_w, h: content_h };

    buf.fill_rect(vp.x, vp.y, vp.w, vp.h, theme::BG);

    match state.mode {
        EditorMode::Text => draw_text_mode(buf, font_system, swash_cache, glyph_atlas, state, &vp, sf, cursor_visible),
        EditorMode::Hex  => draw_hex_mode(buf, font_system, swash_cache, state, &vp, sf),
        EditorMode::Image => draw_image_mode(buf, state, &vp, sf),
    }

    draw_scrollbar(buf, state, &vp, sf, scrollbar_state);
}

/// Hit-test a physical (x, y) click to a (line, col) in the editor.
/// Returns None if click is outside the text area.
pub fn hit_test_cursor(
    state: &EditorState,
    phys_x: usize,
    phys_y: usize,
    x_start: usize,
    y_start: usize,
    sf: f32,
) -> Option<(usize, usize)> {
    if state.mode != EditorMode::Text { return None; }

    let line_h = (TEXT_LINE_HEIGHT * sf) as usize;
    if line_h == 0 { return None; }

    let pad_y = (TEXT_PAD_Y * sf) as usize;
    let gutter_w = gutter_width(state.lines.len(), sf);
    let pad_x = (TEXT_PAD_X * sf) as usize;
    let divider_w = (1.0 * sf).max(1.0) as usize;
    let code_x = x_start + gutter_w + divider_w + pad_x;
    let char_w = (TEXT_FONT_SIZE * sf * 0.6) as usize;

    let click_in_gutter = phys_x < code_x && phys_x >= x_start;

    let scroll = state.scroll_offset.max(0.0) as usize;
    let y_in_content = (phys_y + scroll).saturating_sub(y_start + pad_y);
    let line = (y_in_content / line_h).min(state.lines.len().saturating_sub(1));

    if click_in_gutter {
        return Some((line, 0));
    }

    if phys_x < code_x { return None; }

    let x_in_line = phys_x.saturating_sub(code_x);
    let line_str = &state.lines[line];
    let char_idx = if char_w > 0 { (x_in_line + char_w / 2) / char_w } else { 0 };
    let mut byte_off = 0;
    for (i, ch) in line_str.chars().enumerate() {
        if i >= char_idx { break; }
        byte_off += ch.len_utf8();
    }
    let col = byte_off.min(line_str.len());

    Some((line, col))
}

/// Total content height in physical pixels — used by scroll clamping in event handler.
pub fn content_height_px(state: &EditorState, sf: f32) -> usize {
    state.content_height(sf) as usize
}

/// Scrollbar hit-test result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarHit {
    /// Vertical scrollbar thumb.
    Vertical,
    /// Horizontal scrollbar thumb.
    Horizontal,
    /// Not on any scrollbar.
    None,
}

/// Hit-test whether a physical (px, py) is on the vertical scrollbar thumb.
pub fn scrollbar_hit_test(
    state: &EditorState,
    px: usize, py: usize,
    x_start: usize, y_start: usize,
    content_w: usize, content_h: usize,
    sf: f32,
) -> ScrollbarHit {
    let vp = Viewport { x: x_start, y: y_start, w: content_w, h: content_h };

    if let Some((tx, ty, tw, th)) = vertical_thumb_rect(state, &vp, sf) {
        let margin = (4.0 * sf) as usize;
        if px + margin >= tx && px < tx + tw + margin && py >= ty && py < ty + th {
            return ScrollbarHit::Vertical;
        }
    }

    if let Some((tx, ty, tw, th)) = horizontal_thumb_rect(state, &vp, sf) {
        let margin = (4.0 * sf) as usize;
        if px >= tx && px < tx + tw && py + margin >= ty && py < ty + th + margin {
            return ScrollbarHit::Horizontal;
        }
    }

    ScrollbarHit::None
}

/// Compute vertical thumb rect: (x, y, w, h). Returns None if no scrollbar needed.
fn vertical_thumb_rect(state: &EditorState, vp: &Viewport, sf: f32) -> Option<(usize, usize, usize, usize)> {
    let total_h = content_height_px(state, sf);
    if total_h <= vp.h || vp.h == 0 { return None; }

    let sb_w = (SCROLLBAR_WIDTH * sf).max(4.0) as usize;
    let track_x = vp.x + vp.w - sb_w - (2.0 * sf) as usize;
    let track_h = vp.h;
    let thumb_h = ((vp.h as f64 / total_h as f64) * track_h as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let max_scroll = total_h.saturating_sub(vp.h);
    let scroll = state.scroll_offset.max(0.0) as usize;
    let frac = if max_scroll > 0 { scroll.min(max_scroll) as f64 / max_scroll as f64 } else { 0.0 };
    let thumb_y = vp.y + (frac * (track_h.saturating_sub(thumb_h)) as f64) as usize;

    Some((track_x, thumb_y, sb_w, thumb_h))
}

/// Compute horizontal thumb rect: (x, y, w, h). Returns None if no scrollbar needed.
fn horizontal_thumb_rect(state: &EditorState, vp: &Viewport, sf: f32) -> Option<(usize, usize, usize, usize)> {
    let font_size = TEXT_FONT_SIZE * sf;
    let char_w = (font_size * 0.6) as usize;
    let gutter_w = gutter_width(state.lines.len(), sf);
    let divider_w = (1.0_f32 * sf).max(1.0) as usize;
    let pad_x = (TEXT_PAD_X * sf) as usize;
    let code_x = vp.x + gutter_w + divider_w + pad_x;
    let code_w = vp.w.saturating_sub(gutter_w + divider_w + pad_x);
    let max_w = max_line_width_px(state, char_w);
    if max_w <= code_w || code_w == 0 { return None; }

    let sb_h = (SCROLLBAR_WIDTH * sf).max(4.0) as usize;
    let track_y = vp.y + vp.h - sb_h - (2.0 * sf) as usize;
    let track_w = code_w;
    let thumb_w = ((code_w as f64 / max_w as f64) * track_w as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let max_scroll = max_w.saturating_sub(code_w);
    let scroll_x = state.scroll_x().max(0.0) as usize;
    let frac = if max_scroll > 0 { scroll_x.min(max_scroll) as f64 / max_scroll as f64 } else { 0.0 };
    let thumb_x = code_x + (frac * (track_w.saturating_sub(thumb_w)) as f64) as usize;

    Some((thumb_x, track_y, thumb_w, sb_h))
}

/// Map a vertical pixel position to a scroll offset (for drag).
pub fn vertical_drag_to_scroll(
    state: &EditorState,
    py: f64,
    y_start: usize, content_h: usize,
    sf: f32,
) -> f32 {
    let total_h = content_height_px(state, sf);
    if total_h <= content_h || content_h == 0 { return 0.0; }

    let sb_w_unused = (SCROLLBAR_WIDTH * sf).max(4.0) as usize;
    let _ = sb_w_unused;
    let track_h = content_h;
    let thumb_h = ((content_h as f64 / total_h as f64) * track_h as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let usable = track_h.saturating_sub(thumb_h) as f64;
    if usable <= 0.0 { return 0.0; }

    let max_scroll = total_h.saturating_sub(content_h) as f64;
    let rel = (py - y_start as f64 - thumb_h as f64 / 2.0).clamp(0.0, usable);
    (rel / usable * max_scroll) as f32
}

/// Map a horizontal pixel position to a scroll_x offset (for drag).
pub fn horizontal_drag_to_scroll(
    state: &EditorState,
    px: f64,
    x_start: usize, content_w: usize,
    sf: f32,
) -> f32 {
    let font_size = TEXT_FONT_SIZE * sf;
    let char_w = (font_size * 0.6) as usize;
    let gutter_w = gutter_width(state.lines.len(), sf);
    let divider_w = (1.0_f32 * sf).max(1.0) as usize;
    let pad_x = (TEXT_PAD_X * sf) as usize;
    let code_x = x_start + gutter_w + divider_w + pad_x;
    let code_w = content_w.saturating_sub(gutter_w + divider_w + pad_x);
    let max_w = max_line_width_px(state, char_w);
    if max_w <= code_w || code_w == 0 { return 0.0; }

    let track_w = code_w;
    let thumb_w = ((code_w as f64 / max_w as f64) * track_w as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let usable = track_w.saturating_sub(thumb_w) as f64;
    if usable <= 0.0 { return 0.0; }

    let max_scroll = max_w.saturating_sub(code_w) as f64;
    let rel = (px - code_x as f64 - thumb_w as f64 / 2.0).clamp(0.0, usable);
    (rel / usable * max_scroll) as f32
}


fn draw_text_mode(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    glyph_atlas: &mut GlyphAtlas,
    state: &EditorState,
    vp: &Viewport,
    sf: f32,
    cursor_visible: bool,
) {
    let line_h = (TEXT_LINE_HEIGHT * sf) as usize;
    if line_h == 0 { return; }

    let font_size = TEXT_FONT_SIZE * sf;
    let line_height = TEXT_LINE_HEIGHT * sf;
    let pad_y = (TEXT_PAD_Y * sf) as usize;
    let pad_x = (TEXT_PAD_X * sf) as usize;
    let gutter_pad = (GUTTER_PAD_RIGHT * sf) as usize;
    let char_w = (font_size * 0.6) as usize;

    let gutter_w = gutter_width(state.lines.len(), sf);
    let scroll_y = state.scroll_offset.max(0.0) as usize;
    let scroll_x = state.scroll_x().max(0.0) as usize;
    let divider_w = (1.0 * sf).max(1.0) as usize;

    buf.fill_rect(vp.x, vp.y, gutter_w, vp.h, GUTTER_BG);

    let divider_x = vp.x + gutter_w;
    buf.fill_rect(divider_x, vp.y, divider_w, vp.h, theme::BORDER);

    let content_top = vp.y;
    let content_h = vp.h;

    let first_line = scroll_y / line_h.max(1);
    let last_line = ((scroll_y + content_h) / line_h.max(1) + 1).min(state.lines.len());

    let code_x = vp.x + gutter_w + divider_w + pad_x;
    let clip_bottom = content_top + content_h;
    let clip_right = vp.x + vp.w;

    let sel = state.selection_range();

    for idx in first_line..last_line {
        let y_logical = pad_y + idx * line_h;
        let y_px = content_top + y_logical.saturating_sub(scroll_y);

        if y_px + line_h <= content_top || y_px >= clip_bottom {
            continue;
        }

        if idx == state.cursor_line() {
            buf.fill_rect(
                vp.x + gutter_w + divider_w, y_px,
                vp.w.saturating_sub(gutter_w + divider_w), line_h,
                CURRENT_LINE_BG,
            );
        }

        let diff_kind = state.diff_lines.get(idx).copied();
        if let Some(kind) = diff_kind {
            let content_area_x = vp.x + gutter_w + divider_w;
            let content_area_w = vp.w.saturating_sub(gutter_w + divider_w);
            match kind {
                DiffLineKind::Added => {
                    buf.fill_rect(content_area_x, y_px, content_area_w, line_h, DIFF_ADDED_BG);
                }
                DiffLineKind::Removed => {
                    buf.fill_rect(content_area_x, y_px, content_area_w, line_h, DIFF_REMOVED_BG);
                }
                DiffLineKind::Context => {}
            }
        }

        if let Some((sl, sc, el, ec)) = sel {
            if idx >= sl && idx <= el {
                let line = &state.lines[idx];
                let sel_start_col = if idx == sl { sc } else { 0 };
                let sel_end_col = if idx == el { ec } else { line.len() };

                let x_start_sel = col_to_pixel_x(line, sel_start_col, char_w);
                let x_end_sel = col_to_pixel_x(line, sel_end_col, char_w);

                let sel_px_x = (code_x + x_start_sel).saturating_sub(scroll_x).max(code_x);
                if sel_px_x < clip_right {
                    let sel_right = ((code_x + x_end_sel).saturating_sub(scroll_x)).min(clip_right);
                    let sel_draw_w = sel_right.saturating_sub(sel_px_x).max(char_w);
                    buf.fill_rect(sel_px_x, y_px, sel_draw_w, line_h, SELECTION_BG);
                }
            }
        }

        let line_num = format!("{}", idx + 1);
        let num_text_w = line_num.len() * char_w.max(1);
        let gutter_x = vp.x + gutter_w.saturating_sub(num_text_w + gutter_pad);
        let gutter_color = if idx == state.cursor_line() { GUTTER_ACTIVE_TEXT } else { GUTTER_TEXT };

        blit_str_atlas(
            buf, glyph_atlas, font_system, swash_cache,
            gutter_x, y_px, y_px, y_px + line_h, vp.x, vp.x + gutter_w,
            &line_num, font_size, line_height, gutter_color,
        );

        if let Some(kind) = diff_kind {
            let stripe_w = (2.0 * sf).max(1.0) as usize;
            let stripe_color = match kind {
                DiffLineKind::Added => Some(DIFF_ADDED_GUTTER),
                DiffLineKind::Removed => Some(DIFF_REMOVED_GUTTER),
                DiffLineKind::Context => None,
            };
            if let Some(color) = stripe_color {
                buf.fill_rect(vp.x, y_px, stripe_w, line_h, color);
            }
        }

        let line = &state.lines[idx];
        if !line.is_empty() {
            let colors = if state.has_syntax() {
                let line_byte_start: usize = state.lines[..idx].iter().map(|l| l.len() + 1).sum();
                let line_byte_end = line_byte_start + line.len();
                let tokens = state.tokens_for_line(line_byte_start, line_byte_end);
                build_char_colors(line, line_byte_start, tokens)
            } else {
                Vec::new()
            };

            let mut char_idx = 0usize;
            for (byte_idx, ch) in line.char_indices() {
                let px_x = code_x + char_idx * char_w;
                let screen_x = px_x.saturating_sub(scroll_x);
                if screen_x + char_w <= code_x {
                    char_idx += 1;
                    continue;
                }
                if screen_x >= clip_right {
                    break;
                }

                let color = if !colors.is_empty() && byte_idx < colors.len() {
                    colors[byte_idx]
                } else {
                    LINE_TEXT
                };

                if let Some(glyph) = glyph_atlas.get_or_rasterize(
                    ch, font_size, line_height, false, false, font_system, swash_cache,
                ) {
                    blit_glyph(buf, glyph, screen_x, y_px, color, y_px, y_px + line_h, code_x, clip_right);
                }
                char_idx += 1;
            }
        }

        if cursor_visible && idx == state.cursor_line() {
            let cursor_char_idx = line[..state.cursor_col().min(line.len())].chars().count();
            let cursor_px = code_x + cursor_char_idx * char_w;
            let cursor_x = cursor_px.saturating_sub(scroll_x);
            if cursor_x >= code_x && cursor_x < clip_right {
                let caret_w = (2.0 * sf).max(1.0) as usize;
                let caret_top = y_px + (1.0 * sf) as usize;
                let caret_h = line_h.saturating_sub((2.0 * sf) as usize);
                buf.fill_rect(cursor_x, caret_top, caret_w, caret_h, CURSOR_COLOR);
            }
        }
    }
}

/// Blit a pre-rasterized glyph to the pixel buffer.
#[inline]
fn blit_glyph(
    buf: &mut PixelBuffer,
    glyph: &crate::renderer::glyph_atlas::RasterizedGlyph,
    x: usize,
    y: usize,
    color: (u8, u8, u8),
    clip_top: usize,
    clip_bottom: usize,
    clip_left: usize,
    clip_right: usize,
) {
    let gx = (x as i32 + glyph.bearing_x) as usize;
    let gy = (y as i32 + glyph.bearing_y) as usize;
    let buf_w = buf.width;
    let buf_h = buf.height.min(clip_bottom);

    for row in 0..glyph.height {
        let py = gy + row;
        if py < clip_top { continue; }
        if py >= buf_h { break; }
        let row_off = row * glyph.width;
        for col in 0..glyph.width {
            let px = gx + col;
            if px < clip_left { continue; }
            if px >= buf_w || px >= clip_right { break; }
            let a = glyph.alphas[row_off + col];
            if a > 0 {
                buf.blend_pixel(px, py, color, a as f32 / 255.0);
            }
        }
    }
}

/// Blit a string using the glyph atlas (for gutter numbers etc.)
fn blit_str_atlas(
    buf: &mut PixelBuffer,
    atlas: &mut GlyphAtlas,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x: usize,
    y: usize,
    clip_top: usize,
    clip_bottom: usize,
    clip_left: usize,
    clip_right: usize,
    text: &str,
    font_size: f32,
    line_height: f32,
    color: (u8, u8, u8),
) {
    let char_w = (font_size * 0.6) as usize;
    for (i, ch) in text.chars().enumerate() {
        let px = x + i * char_w;
        if px >= clip_right { break; }
        if let Some(glyph) = atlas.get_or_rasterize(
            ch, font_size, line_height, false, false, font_system, swash_cache,
        ) {
            blit_glyph(buf, glyph, px, y, color, clip_top, clip_bottom, clip_left, clip_right);
        }
    }
}

/// Build a per-byte color map for a syntax-highlighted line.
/// Returns empty Vec if no tokens (caller falls back to LINE_TEXT).
fn build_char_colors(
    line: &str,
    line_byte_start: usize,
    tokens: &[crate::ui::syntax::Token],
) -> Vec<(u8, u8, u8)> {
    if tokens.is_empty() { return Vec::new(); }

    let mut colors = vec![LINE_TEXT; line.len()];
    for tok in tokens {
        let start = tok.start.saturating_sub(line_byte_start);
        let end = tok.end.saturating_sub(line_byte_start).min(line.len());
        let c = tok.kind.to_color();
        for byte in start..end {
            if byte < colors.len() {
                colors[byte] = c;
            }
        }
    }
    colors
}

/// Maximum line width in pixels — used for horizontal scrollbar sizing.
/// Maximum horizontal scroll in pixels for the editor's text content.
pub fn max_scroll_x(state: &EditorState, sf: f32) -> f32 {
    let font_size = TEXT_FONT_SIZE * sf;
    let char_w = (font_size * 0.6) as usize;
    let gutter_w = gutter_width(state.lines.len(), sf);
    let divider_w = (1.0_f32 * sf).max(1.0) as usize;
    let pad_x = (TEXT_PAD_X * sf) as usize;
    let overhead = gutter_w + divider_w + pad_x;
    let max_w = max_line_width_px(state, char_w);
    max_w.saturating_sub(overhead) as f32
}

fn max_line_width_px(state: &EditorState, char_w: usize) -> usize {
    state.lines.iter()
        .map(|l| l.chars().count() * char_w)
        .max()
        .unwrap_or(0)
}

/// Gutter width in physical pixels — depends on line count digit width.
fn gutter_width(line_count: usize, sf: f32) -> usize {
    let digits = if line_count == 0 { 1 } else { (line_count as f64).log10().floor() as usize + 1 };
    let char_w = (TEXT_FONT_SIZE * sf * 0.6) as usize;
    let pad = (GUTTER_PAD_RIGHT * sf) as usize;
    let pad_left = (14.0 * sf) as usize;
    pad_left + digits * char_w.max(1) + pad
}

/// Thin info bar showing file path — drawn at the top of the editor area.
fn draw_file_info_bar(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    state: &EditorState,
    vp: &Viewport,
    sf: f32,
) {
    let bar_h = (24.0 * sf) as usize;
    buf.fill_rect(vp.x, vp.y, vp.w, bar_h, theme::BG_SURFACE);

    let border_h = (1.0 * sf).max(1.0) as usize;
    buf.fill_rect(vp.x, vp.y + bar_h, vp.w, border_h, theme::BORDER);

    let pad_x = (8.0 * sf) as usize;
    let metrics = Metrics::new(11.0 * sf, 24.0 * sf);
    let path_str = state.path.to_string_lossy();

    let mode_label = match state.mode {
        EditorMode::Text => format!("{} — {} lines", path_str, state.lines.len()),
        EditorMode::Hex => format!("{} — {} bytes", path_str, state.raw_bytes.len()),
        EditorMode::Image => format!("{} — {}×{}", path_str, state.image_width, state.image_height),
    };

    let mut info_buf = Buffer::new(font_system, metrics);
    draw_text_at_buffered(
        buf, font_system, swash_cache, &mut info_buf,
        vp.x + pad_x, vp.y, vp.y + bar_h,
        &mode_label, metrics, theme::FG_SECONDARY, Family::SansSerif,
    );
}


fn draw_hex_mode(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    state: &EditorState,
    vp: &Viewport,
    sf: f32,
) {
    let line_h = (HEX_LINE_HEIGHT * sf) as usize;
    if line_h == 0 { return; }

    let pad_y = (HEX_PAD_Y * sf) as usize;
    let pad_x = (HEX_PAD_X * sf) as usize;
    let metrics = Metrics::new(HEX_FONT_SIZE * sf, HEX_LINE_HEIGHT * sf);
    let scroll = state.scroll_offset.max(0.0) as usize;

    let bytes_per_row = 16usize;
    let total_rows = (state.raw_bytes.len() + bytes_per_row - 1) / bytes_per_row;

    draw_file_info_bar(buf, font_system, swash_cache, state, vp, sf);
    let info_bar_h = (24.0 * sf) as usize + (1.0 * sf).max(1.0) as usize;

    let first_row = scroll / line_h.max(1);
    let last_row = ((scroll + vp.h) / line_h.max(1) + 1).min(total_rows);

    let mut addr_buf = Buffer::new(font_system, metrics);
    let mut hex_buf = Buffer::new(font_system, metrics);
    let mut ascii_buf = Buffer::new(font_system, metrics);

    let char_w = (HEX_FONT_SIZE * sf * 0.62) as usize;
    let addr_w = 8 * char_w.max(1) + pad_x;
    let hex_col_w = 3 * char_w.max(1);
    let hex_block_w = bytes_per_row * hex_col_w + pad_x;
    let clip_bottom = vp.y + vp.h;

    for row in first_row..last_row {
        let y_logical = info_bar_h + pad_y + row * line_h;
        let y_px = vp.y + y_logical.saturating_sub(scroll);

        if y_px + line_h <= vp.y || y_px >= clip_bottom {
            continue;
        }

        let offset = row * bytes_per_row;

        let addr_str = format!("{:08X}", offset);
        draw_text_at_buffered(
            buf, font_system, swash_cache, &mut addr_buf,
            vp.x + pad_x, y_px, clip_bottom,
            &addr_str, metrics, HEX_ADDR_COLOR, Family::Monospace,
        );

        let end = (offset + bytes_per_row).min(state.raw_bytes.len());
        let chunk = &state.raw_bytes[offset..end];
        let hex_str: String = chunk.iter()
            .map(|b| format!("{:02X} ", b))
            .collect();
        let padded = if chunk.len() < bytes_per_row {
            format!("{:<width$}", hex_str, width = bytes_per_row * 3)
        } else {
            hex_str
        };

        let hex_x = vp.x + pad_x + addr_w;
        draw_text_at_buffered(
            buf, font_system, swash_cache, &mut hex_buf,
            hex_x, y_px, clip_bottom,
            &padded, metrics, HEX_BYTE_COLOR, Family::Monospace,
        );

        let ascii_str: String = chunk.iter().map(|&b| {
            if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }
        }).collect();

        let ascii_x = hex_x + hex_block_w;
        let ascii_color = if chunk.iter().all(|&b| b == 0) { HEX_NULL_COLOR } else { HEX_ASCII_COLOR };
        draw_text_at_buffered(
            buf, font_system, swash_cache, &mut ascii_buf,
            ascii_x, y_px, clip_bottom,
            &ascii_str, metrics, ascii_color, Family::Monospace,
        );
    }
}


fn draw_image_mode(
    buf: &mut PixelBuffer,
    state: &EditorState,
    vp: &Viewport,
    sf: f32,
) {
    let info_bar_h = (24.0 * sf) as usize + (1.0 * sf).max(1.0) as usize;

    if state.image_width == 0 || state.image_height == 0 || state.image_rgba.is_empty() {
        return;
    }

    let iw = state.image_width as usize;
    let ih = state.image_height as usize;

    let available_w = vp.w;
    let available_h = vp.h.saturating_sub(info_bar_h);

    let scale_x = available_w as f64 / iw as f64;
    let scale_y = available_h as f64 / ih as f64;
    let scale = scale_x.min(scale_y).min(1.0);

    let draw_w = (iw as f64 * scale) as usize;
    let draw_h = (ih as f64 * scale) as usize;

    let offset_x = vp.x + (available_w.saturating_sub(draw_w)) / 2;
    let offset_y = vp.y + info_bar_h + (available_h.saturating_sub(draw_h)) / 2;

    let buf_w = buf.width;
    let buf_h = buf.height;

    for dy in 0..draw_h {
        let py = offset_y + dy;
        if py >= buf_h { break; }

        let src_y = (dy as f64 / scale) as usize;
        if src_y >= ih { continue; }

        for dx in 0..draw_w {
            let px = offset_x + dx;
            if px >= buf_w { continue; }

            let src_x = (dx as f64 / scale) as usize;
            if src_x >= iw { continue; }

            let si = (src_y * iw + src_x) * 4;
            if si + 3 >= state.image_rgba.len() { continue; }

            let r = state.image_rgba[si];
            let g = state.image_rgba[si + 1];
            let b = state.image_rgba[si + 2];
            let a = state.image_rgba[si + 3];

            if a == 255 {
                let di = (py * buf_w + px) * 4;
                buf.data[di] = b;
                buf.data[di + 1] = g;
                buf.data[di + 2] = r;
                buf.data[di + 3] = 255;
            } else if a > 0 {
                buf.blend_pixel(px, py, (r, g, b), a as f32 / 255.0);
            }
        }
    }

    buf.mark_dirty(offset_y, offset_y + draw_h);
}


fn draw_scrollbar(
    buf: &mut PixelBuffer,
    state: &EditorState,
    vp: &Viewport,
    sf: f32,
    active: ScrollbarHit,
) {
    let v_color = match active {
        ScrollbarHit::Vertical => theme::SCROLLBAR_THUMB_HOVER,
        _ => SCROLLBAR_COLOR,
    };
    let h_color = match active {
        ScrollbarHit::Horizontal => theme::SCROLLBAR_THUMB_HOVER,
        _ => SCROLLBAR_COLOR,
    };

    if let Some((tx, ty, tw, th)) = vertical_thumb_rect(state, vp, sf) {
        buf.fill_rect(tx, ty, tw, th, v_color);
    }

    if let Some((tx, ty, tw, th)) = horizontal_thumb_rect(state, vp, sf) {
        buf.fill_rect(tx, ty, tw, th, h_color);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gutter_width_scales_with_digits() {
        let w1 = gutter_width(9, 1.0);
        let w2 = gutter_width(99, 1.0);
        let w3 = gutter_width(999, 1.0);
        assert!(w2 > w1);
        assert!(w3 > w2);
    }

    #[test]
    fn gutter_width_scales_with_sf() {
        let w1 = gutter_width(100, 1.0);
        let w2 = gutter_width(100, 2.0);
        assert!(w2 > w1);
    }

    #[test]
    fn content_height_text() {
        let state = EditorState::test_text("test.rs", vec!["a".into(), "b".into(), "c".into()]);
        let h = content_height_px(&state, 1.0);
        assert!(h > 0);
    }

    #[test]
    fn content_height_hex() {
        let state = EditorState::test_hex("data.bin", vec![0u8; 256]);
        let h = content_height_px(&state, 1.0);
        assert!(h > 0);
    }
}
