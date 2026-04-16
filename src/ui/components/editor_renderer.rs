//! Editor rendering: text, hex, image, and side-by-side diff viewers.

use cosmic_text::{Buffer, Family, FontSystem, Metrics, SwashCache};

use crate::renderer::glyph_atlas::GlyphAtlas;
use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::{
    draw_text_at, draw_text_at_buffered, draw_text_clipped, measure_text_width,
};
use crate::renderer::theme;
use crate::ui::editor::{
    DiffRowKind, EditorMode, EditorState, FindBarHover, FindFocus, GUTTER_PAD_RIGHT, HEX_FONT_SIZE,
    HEX_LINE_HEIGHT, HEX_PAD_X, HEX_PAD_Y, TEXT_FONT_SIZE, TEXT_LINE_HEIGHT, TEXT_PAD_X,
    TEXT_PAD_Y,
};

const GUTTER_BG: (u8, u8, u8) = theme::BG;
const GUTTER_TEXT: (u8, u8, u8) = theme::FG_MUTED;
const GUTTER_ACTIVE_TEXT: (u8, u8, u8) = theme::EDITOR_GUTTER_ACTIVE;
const LINE_TEXT: (u8, u8, u8) = theme::FG_PRIMARY;
const CURRENT_LINE_BG: (u8, u8, u8) = theme::EDITOR_CURRENT_LINE_BG;
const SELECTION_BG: (u8, u8, u8) = theme::BG_SELECTION;
const SEARCH_HIGHLIGHT_BG: (u8, u8, u8) = (80, 60, 20);
const FIND_MATCH_BG: (u8, u8, u8) = (90, 70, 30);
const FIND_CURRENT_BG: (u8, u8, u8) = (170, 100, 40);
const CURSOR_COLOR: (u8, u8, u8) = theme::EDITOR_CURSOR;

const DIFF_ADDED_BG: (u8, u8, u8) = (30, 60, 30);
const DIFF_REMOVED_BG: (u8, u8, u8) = (60, 25, 25);
const DIFF_ADDED_GUTTER: (u8, u8, u8) = (50, 160, 60);
const DIFF_REMOVED_GUTTER: (u8, u8, u8) = (200, 60, 60);
const DIFF_EMPTY_BG: (u8, u8, u8) = (16, 16, 20);
const DIFF_SEPARATOR_BG: (u8, u8, u8) = (20, 20, 25);

const BREADCRUMB_SEPARATOR: &str = " › ";
const INFO_BAR_HEIGHT: f32 = 24.0;

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
    let vp = Viewport {
        x: x_start,
        y: y_start,
        w: content_w,
        h: content_h,
    };

    buf.fill_rect(vp.x, vp.y, vp.w, vp.h, theme::BG);

    if state.has_diff_view() {
        draw_diff_mode(buf, font_system, swash_cache, glyph_atlas, state, &vp, sf);
    } else {
        match state.mode {
            EditorMode::Text => draw_text_mode(
                buf,
                font_system,
                swash_cache,
                glyph_atlas,
                state,
                &vp,
                sf,
                cursor_visible,
            ),
            EditorMode::Hex => draw_hex_mode(buf, font_system, swash_cache, state, &vp, sf),
            EditorMode::Image => draw_image_mode(buf, state, &vp, sf),
        }
    }

    draw_scrollbar(buf, state, &vp, sf, scrollbar_state);

    if state.find_state.is_open() {
        draw_find_bar(
            buf,
            font_system,
            swash_cache,
            state,
            vp.x,
            vp.y,
            vp.w,
            sf,
            cursor_visible,
        );
    }
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
    if state.mode != EditorMode::Text {
        return None;
    }

    let line_h = (TEXT_LINE_HEIGHT * sf) as usize;
    if line_h == 0 {
        return None;
    }

    let pad_y = (TEXT_PAD_Y * sf) as usize;
    let gutter_w = gutter_width(state.lines.len(), sf);
    let pad_x = (TEXT_PAD_X * sf) as usize;
    let divider_w = (1.0 * sf).max(1.0) as usize;
    let code_x = x_start + gutter_w + divider_w + pad_x;
    let char_w = (TEXT_FONT_SIZE * sf * 0.6) as usize;
    let info_bar_h = (INFO_BAR_HEIGHT * sf) as usize + (1.0 * sf).max(1.0) as usize;

    let click_in_gutter = phys_x < code_x && phys_x >= x_start;

    let scroll = state.scroll_offset.max(0.0) as usize;
    let y_in_content = (phys_y + scroll).saturating_sub(y_start + info_bar_h + pad_y);
    let line = (y_in_content / line_h).min(state.lines.len().saturating_sub(1));

    if click_in_gutter {
        return Some((line, 0));
    }

    if phys_x < code_x {
        return None;
    }

    let x_in_line = phys_x.saturating_sub(code_x);
    let line_str = &state.lines[line];
    let char_idx = if char_w > 0 {
        (x_in_line + char_w / 2) / char_w
    } else {
        0
    };
    let mut byte_off = 0;
    for (i, ch) in line_str.chars().enumerate() {
        if i >= char_idx {
            break;
        }
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
    px: usize,
    py: usize,
    x_start: usize,
    y_start: usize,
    content_w: usize,
    content_h: usize,
    sf: f32,
) -> ScrollbarHit {
    let vp = Viewport {
        x: x_start,
        y: y_start,
        w: content_w,
        h: content_h,
    };

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
fn vertical_thumb_rect(
    state: &EditorState,
    vp: &Viewport,
    sf: f32,
) -> Option<(usize, usize, usize, usize)> {
    let total_h = content_height_px(state, sf);
    if total_h <= vp.h || vp.h == 0 {
        return None;
    }

    let sb_w = (SCROLLBAR_WIDTH * sf).max(4.0) as usize;
    let track_x = vp.x + vp.w - sb_w - (2.0 * sf) as usize;
    let track_h = vp.h;
    let thumb_h = ((vp.h as f64 / total_h as f64) * track_h as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let max_scroll = total_h.saturating_sub(vp.h);
    let scroll = state.scroll_offset.max(0.0) as usize;
    let frac = if max_scroll > 0 {
        scroll.min(max_scroll) as f64 / max_scroll as f64
    } else {
        0.0
    };
    let thumb_y = vp.y + (frac * (track_h.saturating_sub(thumb_h)) as f64) as usize;

    Some((track_x, thumb_y, sb_w, thumb_h))
}

/// Compute horizontal thumb rect: (x, y, w, h). Returns None if no scrollbar needed.
fn horizontal_thumb_rect(
    state: &EditorState,
    vp: &Viewport,
    sf: f32,
) -> Option<(usize, usize, usize, usize)> {
    let font_size = TEXT_FONT_SIZE * sf;
    let char_w = (font_size * 0.6) as usize;
    let gutter_w = gutter_width(state.lines.len(), sf);
    let divider_w = (1.0_f32 * sf).max(1.0) as usize;
    let pad_x = (TEXT_PAD_X * sf) as usize;
    let code_x = vp.x + gutter_w + divider_w + pad_x;
    let code_w = vp.w.saturating_sub(gutter_w + divider_w + pad_x);
    let max_w = max_line_width_px(state, char_w);
    if max_w <= code_w || code_w == 0 {
        return None;
    }

    let sb_h = (SCROLLBAR_WIDTH * sf).max(4.0) as usize;
    let track_y = vp.y + vp.h - sb_h - (2.0 * sf) as usize;
    let track_w = code_w;
    let thumb_w = ((code_w as f64 / max_w as f64) * track_w as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let max_scroll = max_w.saturating_sub(code_w);
    let scroll_x = state.scroll_x().max(0.0) as usize;
    let frac = if max_scroll > 0 {
        scroll_x.min(max_scroll) as f64 / max_scroll as f64
    } else {
        0.0
    };
    let thumb_x = code_x + (frac * (track_w.saturating_sub(thumb_w)) as f64) as usize;

    Some((thumb_x, track_y, thumb_w, sb_h))
}

/// Map a vertical pixel position to a scroll offset (for drag).
pub fn vertical_drag_to_scroll(
    state: &EditorState,
    py: f64,
    y_start: usize,
    content_h: usize,
    sf: f32,
) -> f32 {
    let total_h = content_height_px(state, sf);
    if total_h <= content_h || content_h == 0 {
        return 0.0;
    }

    let sb_w_unused = (SCROLLBAR_WIDTH * sf).max(4.0) as usize;
    let _ = sb_w_unused;
    let track_h = content_h;
    let thumb_h = ((content_h as f64 / total_h as f64) * track_h as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let usable = track_h.saturating_sub(thumb_h) as f64;
    if usable <= 0.0 {
        return 0.0;
    }

    let max_scroll = total_h.saturating_sub(content_h) as f64;
    let rel = (py - y_start as f64 - thumb_h as f64 / 2.0).clamp(0.0, usable);
    (rel / usable * max_scroll) as f32
}

/// Map a horizontal pixel position to a scroll_x offset (for drag).
pub fn horizontal_drag_to_scroll(
    state: &EditorState,
    px: f64,
    x_start: usize,
    content_w: usize,
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
    if max_w <= code_w || code_w == 0 {
        return 0.0;
    }

    let track_w = code_w;
    let thumb_w = ((code_w as f64 / max_w as f64) * track_w as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let usable = track_w.saturating_sub(thumb_w) as f64;
    if usable <= 0.0 {
        return 0.0;
    }

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
    if line_h == 0 {
        return;
    }

    draw_breadcrumb_bar(buf, font_system, swash_cache, state, vp, sf);
    let info_bar_h = (INFO_BAR_HEIGHT * sf) as usize + (1.0 * sf).max(1.0) as usize;

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

    buf.fill_rect(
        vp.x,
        vp.y + info_bar_h,
        gutter_w,
        vp.h.saturating_sub(info_bar_h),
        GUTTER_BG,
    );

    let divider_x = vp.x + gutter_w;
    buf.fill_rect(
        divider_x,
        vp.y + info_bar_h,
        divider_w,
        vp.h.saturating_sub(info_bar_h),
        theme::BORDER,
    );

    let content_top = vp.y + info_bar_h;
    let content_h = vp.h.saturating_sub(info_bar_h);

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
                vp.x + gutter_w + divider_w,
                y_px,
                vp.w.saturating_sub(gutter_w + divider_w),
                line_h,
                CURRENT_LINE_BG,
            );
        }

        if let Some((sl, sc, el, ec)) = sel
            && idx >= sl
            && idx <= el
        {
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

        if let Some(ref term) = state.search_highlight {
            let line = &state.lines[idx];
            let term_lower = term.to_lowercase();
            let line_lower = line.to_lowercase();
            let mut search_pos = 0;
            while let Some(found) = line_lower[search_pos..].find(&term_lower) {
                let byte_start = search_pos + found;
                let byte_end = byte_start + term_lower.len();
                let col_start = line[..byte_start].chars().count();
                let col_end = col_start + line[byte_start..byte_end].chars().count();
                let hl_x = (code_x + col_start * char_w).saturating_sub(scroll_x);
                let hl_w = (col_end - col_start) * char_w;
                if hl_x + hl_w > code_x && hl_x < clip_right {
                    let draw_x = hl_x.max(code_x);
                    let draw_w = (hl_x + hl_w).min(clip_right).saturating_sub(draw_x);
                    buf.fill_rect(draw_x, y_px, draw_w, line_h, SEARCH_HIGHLIGHT_BG);
                }
                search_pos = byte_end;
            }
        }

        if state.find_state.is_open() && !state.find_state.matches.is_empty() {
            let line = &state.lines[idx];
            for (mi, m) in state.find_state.matches.iter().enumerate() {
                if m.line != idx {
                    continue;
                }
                let safe_start = m.byte_start.min(line.len());
                let safe_end = m.byte_end.min(line.len());
                let col_start = line[..safe_start].chars().count();
                let col_end = col_start + line[safe_start..safe_end].chars().count();
                let hl_x = (code_x + col_start * char_w).saturating_sub(scroll_x);
                let hl_w = (col_end - col_start) * char_w;
                if hl_x + hl_w > code_x && hl_x < clip_right {
                    let draw_x = hl_x.max(code_x);
                    let draw_w = (hl_x + hl_w).min(clip_right).saturating_sub(draw_x);
                    let bg = if state.find_state.current == Some(mi) {
                        FIND_CURRENT_BG
                    } else {
                        FIND_MATCH_BG
                    };
                    buf.fill_rect(draw_x, y_px, draw_w, line_h, bg);
                }
            }
        }

        let line_num = format!("{}", idx + 1);
        let num_text_w = line_num.len() * char_w.max(1);
        let gutter_x = vp.x + gutter_w.saturating_sub(num_text_w + gutter_pad);
        let gutter_color = if idx == state.cursor_line() {
            GUTTER_ACTIVE_TEXT
        } else {
            GUTTER_TEXT
        };

        blit_str_atlas(
            buf,
            glyph_atlas,
            font_system,
            swash_cache,
            gutter_x,
            y_px,
            y_px,
            y_px + line_h,
            vp.x,
            vp.x + gutter_w,
            &line_num,
            font_size,
            line_height,
            gutter_color,
        );

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
                    ch,
                    font_size,
                    line_height,
                    false,
                    false,
                    font_system,
                    swash_cache,
                ) {
                    blit_glyph(
                        buf,
                        glyph,
                        screen_x,
                        y_px,
                        color,
                        y_px,
                        y_px + line_h,
                        code_x,
                        clip_right,
                    );
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
        if py < clip_top {
            continue;
        }
        if py >= buf_h {
            break;
        }
        let row_off = row * glyph.width;
        for col in 0..glyph.width {
            let px = gx + col;
            if px < clip_left {
                continue;
            }
            if px >= buf_w || px >= clip_right {
                break;
            }
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
        if px >= clip_right {
            break;
        }
        if let Some(glyph) = atlas.get_or_rasterize(
            ch,
            font_size,
            line_height,
            false,
            false,
            font_system,
            swash_cache,
        ) {
            blit_glyph(
                buf,
                glyph,
                px,
                y,
                color,
                clip_top,
                clip_bottom,
                clip_left,
                clip_right,
            );
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
    if tokens.is_empty() {
        return Vec::new();
    }

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
    state
        .lines
        .iter()
        .map(|l| l.chars().count() * char_w)
        .max()
        .unwrap_or(0)
}

/// Gutter width in physical pixels — depends on line count digit width.
fn gutter_width(line_count: usize, sf: f32) -> usize {
    let digits = if line_count == 0 {
        1
    } else {
        (line_count as f64).log10().floor() as usize + 1
    };
    let char_w = (TEXT_FONT_SIZE * sf * 0.6) as usize;
    let pad = (GUTTER_PAD_RIGHT * sf) as usize;
    let pad_left = (14.0 * sf) as usize;
    pad_left + digits * char_w.max(1) + pad
}

/// Breadcrumb bar showing path segments — drawn at the top of the editor area.
/// Renders like VS Code: `src › ui › components › editor_renderer.rs`
fn draw_breadcrumb_bar(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    state: &EditorState,
    vp: &Viewport,
    sf: f32,
) {
    let bar_h = (INFO_BAR_HEIGHT * sf) as usize;
    buf.fill_rect(vp.x, vp.y, vp.w, bar_h, theme::BG_SURFACE);

    let border_h = (1.0 * sf).max(1.0) as usize;
    buf.fill_rect(vp.x, vp.y + bar_h, vp.w, border_h, theme::BORDER);

    let pad_x = (8.0 * sf) as usize;
    let metrics = Metrics::new(11.0 * sf, INFO_BAR_HEIGHT * sf);
    let char_w = (11.0 * sf * 0.58) as usize;

    let components: Vec<&str> = state
        .path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    let start_idx = if components.len() > 6 {
        components.len() - 6
    } else {
        0
    };
    let visible = &components[start_idx..];

    let mut x = vp.x + pad_x;
    let mut info_buf = Buffer::new(font_system, metrics);
    let clip_x = vp.x + vp.w;

    for (i, segment) in visible.iter().enumerate() {
        if x >= clip_x {
            break;
        }
        let is_last = i == visible.len() - 1;
        let color = if is_last {
            theme::FG_BRIGHT
        } else {
            theme::FG_SECONDARY
        };

        draw_text_at_buffered(
            buf,
            font_system,
            swash_cache,
            &mut info_buf,
            x,
            vp.y,
            vp.y + bar_h,
            segment,
            metrics,
            color,
            Family::SansSerif,
        );
        x += segment.len() * char_w;

        if !is_last && x < clip_x {
            draw_text_at_buffered(
                buf,
                font_system,
                swash_cache,
                &mut info_buf,
                x,
                vp.y,
                vp.y + bar_h,
                BREADCRUMB_SEPARATOR,
                metrics,
                theme::FG_MUTED,
                Family::SansSerif,
            );
            x += BREADCRUMB_SEPARATOR.len() * char_w;
        }
    }
}

/// Compute gutter width for a given maximum line number.
fn gutter_width_for_num(max_num: usize, sf: f32) -> usize {
    let digits = if max_num == 0 {
        1
    } else {
        (max_num as f64).log10().floor() as usize + 1
    };
    let char_w = (TEXT_FONT_SIZE * sf * 0.6) as usize;
    let pad = (GUTTER_PAD_RIGHT * sf) as usize;
    let pad_left = (8.0 * sf) as usize;
    pad_left + digits * char_w.max(1) + pad
}

fn draw_diff_mode(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    glyph_atlas: &mut GlyphAtlas,
    state: &EditorState,
    vp: &Viewport,
    sf: f32,
) {
    let rows = match state.diff_view.as_ref() {
        Some(r) => r,
        None => return,
    };

    draw_breadcrumb_bar(buf, font_system, swash_cache, state, vp, sf);
    let info_bar_h = (INFO_BAR_HEIGHT * sf) as usize + (1.0 * sf).max(1.0) as usize;

    let line_h = (TEXT_LINE_HEIGHT * sf) as usize;
    if line_h == 0 {
        return;
    }
    let font_size = TEXT_FONT_SIZE * sf;
    let line_height = TEXT_LINE_HEIGHT * sf;
    let pad_y = (TEXT_PAD_Y * sf) as usize;
    let pad_x = (TEXT_PAD_X * sf) as usize;
    let char_w = (font_size * 0.6) as usize;
    let gutter_pad = (GUTTER_PAD_RIGHT * sf) as usize;

    let center_div_w = (1.0 * sf).max(1.0) as usize;
    let usable = vp.w.saturating_sub(center_div_w);
    let left_w = (usable as f32 * state.diff_split_frac).round() as usize;
    let right_w = usable.saturating_sub(left_w);
    let left_x = vp.x;
    let right_x = vp.x + left_w + center_div_w;

    buf.fill_rect(
        vp.x + left_w,
        vp.y + info_bar_h,
        center_div_w,
        vp.h.saturating_sub(info_bar_h),
        theme::BORDER,
    );

    let max_num = rows
        .iter()
        .flat_map(|r| [r.left_num, r.right_num].into_iter().flatten())
        .max()
        .unwrap_or(0);
    let side_gutter_w = gutter_width_for_num(max_num, sf);

    let scroll = state.scroll_offset.max(0.0) as usize;
    let first_row = scroll / line_h.max(1);
    let visible_count = vp.h.saturating_sub(info_bar_h) / line_h.max(1) + 2;
    let last_row = (first_row + visible_count).min(rows.len());

    let clip_bottom = vp.y + vp.h;

    for (i, row) in rows
        .iter()
        .enumerate()
        .skip(first_row)
        .take(last_row - first_row)
    {
        let y_logical = info_bar_h + pad_y + i * line_h;
        let y_px = vp.y + y_logical.saturating_sub(scroll);

        if y_px + line_h <= vp.y + info_bar_h || y_px >= clip_bottom {
            continue;
        }

        if row.kind == DiffRowKind::Separator {
            buf.fill_rect(left_x, y_px, left_w, line_h, DIFF_SEPARATOR_BG);
            buf.fill_rect(right_x, y_px, right_w, line_h, DIFF_SEPARATOR_BG);
            let dot_y = y_px + line_h / 2;
            let dot_h = (1.0 * sf).max(1.0) as usize;
            buf.fill_rect(left_x, dot_y, left_w, dot_h, theme::BORDER);
            buf.fill_rect(right_x, dot_y, right_w, dot_h, theme::BORDER);
            continue;
        }

        let left_changed = matches!(row.kind, DiffRowKind::Removed | DiffRowKind::Modified);
        let right_changed = matches!(row.kind, DiffRowKind::Added | DiffRowKind::Modified);

        draw_diff_side(
            buf,
            glyph_atlas,
            font_system,
            swash_cache,
            left_x,
            y_px,
            left_w,
            line_h,
            side_gutter_w,
            gutter_pad,
            pad_x,
            char_w,
            row.left_num,
            row.left_text.as_deref(),
            left_changed,
            true,
            font_size,
            line_height,
            sf,
            clip_bottom,
        );

        draw_diff_side(
            buf,
            glyph_atlas,
            font_system,
            swash_cache,
            right_x,
            y_px,
            right_w,
            line_h,
            side_gutter_w,
            gutter_pad,
            pad_x,
            char_w,
            row.right_num,
            row.right_text.as_deref(),
            right_changed,
            false,
            font_size,
            line_height,
            sf,
            clip_bottom,
        );
    }
}

/// Return the physical X coordinate of the centre divider in diff mode.
pub fn diff_divider_x(vp_x: usize, vp_w: usize, frac: f32, sf: f32) -> usize {
    let center_div_w = (1.0_f32 * sf).max(1.0) as usize;
    let usable = vp_w.saturating_sub(center_div_w);
    let left_w = (usable as f32 * frac).round() as usize;
    vp_x + left_w
}

#[allow(clippy::too_many_arguments)]
fn draw_diff_side(
    buf: &mut PixelBuffer,
    glyph_atlas: &mut GlyphAtlas,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x: usize,
    y: usize,
    width: usize,
    line_h: usize,
    gutter_w: usize,
    gutter_pad: usize,
    pad_x: usize,
    char_w: usize,
    line_num: Option<usize>,
    text: Option<&str>,
    is_changed: bool,
    is_left: bool,
    font_size: f32,
    line_height: f32,
    sf: f32,
    clip_bottom: usize,
) {
    let div_w = (1.0 * sf).max(1.0) as usize;

    buf.fill_rect(x, y, gutter_w, line_h, GUTTER_BG);
    buf.fill_rect(x + gutter_w, y, div_w, line_h, theme::BORDER);

    let content_x = x + gutter_w + div_w;
    let content_w = width.saturating_sub(gutter_w + div_w);

    if text.is_none() {
        buf.fill_rect(content_x, y, content_w, line_h, DIFF_EMPTY_BG);
        return;
    }

    if is_changed {
        let bg = if is_left {
            DIFF_REMOVED_BG
        } else {
            DIFF_ADDED_BG
        };
        buf.fill_rect(content_x, y, content_w, line_h, bg);
        let stripe_w = (2.0 * sf).max(1.0) as usize;
        let stripe_color = if is_left {
            DIFF_REMOVED_GUTTER
        } else {
            DIFF_ADDED_GUTTER
        };
        buf.fill_rect(x, y, stripe_w, line_h, stripe_color);
    }

    if let Some(num) = line_num {
        let num_str = format!("{}", num);
        let num_text_w = num_str.len() * char_w.max(1);
        let gutter_x = x + gutter_w.saturating_sub(num_text_w + gutter_pad);
        blit_str_atlas(
            buf,
            glyph_atlas,
            font_system,
            swash_cache,
            gutter_x,
            y,
            y,
            (y + line_h).min(clip_bottom),
            x,
            x + gutter_w,
            &num_str,
            font_size,
            line_height,
            GUTTER_TEXT,
        );
    }

    if let Some(text) = text {
        let code_x = content_x + pad_x;
        let clip_right = x + width;
        for (ci, ch) in text.chars().enumerate() {
            let px = code_x + ci * char_w;
            if px >= clip_right {
                break;
            }
            if let Some(glyph) = glyph_atlas.get_or_rasterize(
                ch,
                font_size,
                line_height,
                false,
                false,
                font_system,
                swash_cache,
            ) {
                blit_glyph(
                    buf,
                    glyph,
                    px,
                    y,
                    LINE_TEXT,
                    y,
                    (y + line_h).min(clip_bottom),
                    code_x,
                    clip_right,
                );
            }
        }
    }
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
    if line_h == 0 {
        return;
    }

    let pad_y = (HEX_PAD_Y * sf) as usize;
    let pad_x = (HEX_PAD_X * sf) as usize;
    let metrics = Metrics::new(HEX_FONT_SIZE * sf, HEX_LINE_HEIGHT * sf);
    let scroll = state.scroll_offset.max(0.0) as usize;

    let bytes_per_row = 16usize;
    let total_rows = state.raw_bytes.len().div_ceil(bytes_per_row);

    draw_breadcrumb_bar(buf, font_system, swash_cache, state, vp, sf);
    let info_bar_h = (INFO_BAR_HEIGHT * sf) as usize + (1.0 * sf).max(1.0) as usize;

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
            buf,
            font_system,
            swash_cache,
            &mut addr_buf,
            vp.x + pad_x,
            y_px,
            clip_bottom,
            &addr_str,
            metrics,
            HEX_ADDR_COLOR,
            Family::Monospace,
        );

        let end = (offset + bytes_per_row).min(state.raw_bytes.len());
        let chunk = &state.raw_bytes[offset..end];
        let hex_str: String = chunk.iter().map(|b| format!("{:02X} ", b)).collect();
        let padded = if chunk.len() < bytes_per_row {
            format!("{:<width$}", hex_str, width = bytes_per_row * 3)
        } else {
            hex_str
        };

        let hex_x = vp.x + pad_x + addr_w;
        draw_text_at_buffered(
            buf,
            font_system,
            swash_cache,
            &mut hex_buf,
            hex_x,
            y_px,
            clip_bottom,
            &padded,
            metrics,
            HEX_BYTE_COLOR,
            Family::Monospace,
        );

        let ascii_str: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();

        let ascii_x = hex_x + hex_block_w;
        let ascii_color = if chunk.iter().all(|&b| b == 0) {
            HEX_NULL_COLOR
        } else {
            HEX_ASCII_COLOR
        };
        draw_text_at_buffered(
            buf,
            font_system,
            swash_cache,
            &mut ascii_buf,
            ascii_x,
            y_px,
            clip_bottom,
            &ascii_str,
            metrics,
            ascii_color,
            Family::Monospace,
        );
    }
}

fn draw_image_mode(buf: &mut PixelBuffer, state: &EditorState, vp: &Viewport, sf: f32) {
    let info_bar_h = (INFO_BAR_HEIGHT * sf) as usize + (1.0 * sf).max(1.0) as usize;

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
        if py >= buf_h {
            break;
        }

        let src_y = (dy as f64 / scale) as usize;
        if src_y >= ih {
            continue;
        }

        for dx in 0..draw_w {
            let px = offset_x + dx;
            if px >= buf_w {
                continue;
            }

            let src_x = (dx as f64 / scale) as usize;
            if src_x >= iw {
                continue;
            }

            let si = (src_y * iw + src_x) * 4;
            if si + 3 >= state.image_rgba.len() {
                continue;
            }

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

/// Hit-test regions on the editor find bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindBarHit {
    ToggleExpand,
    FindInput { rel_x: usize },
    ToggleCase,
    ToggleWord,
    ToggleRegex,
    Prev,
    Next,
    Close,
    ReplaceInput { rel_x: usize },
    ReplaceOne,
    ReplaceAll,
}

const FIND_BAR_ROW_H: f32 = 30.0;
const FIND_BAR_PAD_X: f32 = 6.0;
const FIND_BAR_GAP: f32 = 4.0;
const FIND_BAR_BTN_W: f32 = 24.0;
const FIND_BAR_INPUT_W: f32 = 240.0;
const FIND_BAR_COUNT_W: f32 = 80.0;
const FIND_BAR_FONT: f32 = 12.0;

/// Compute the anchored physical rect of the find bar. Returns None when closed.
pub fn find_bar_rect(
    state: &EditorState,
    vp_x: usize,
    vp_y: usize,
    vp_w: usize,
    sf: f32,
) -> Option<(usize, usize, usize, usize)> {
    if !state.find_state.is_open() {
        return None;
    }
    let info_bar_h = (INFO_BAR_HEIGHT * sf) as usize + (1.0 * sf).max(1.0) as usize;
    let row_h = (FIND_BAR_ROW_H * sf) as usize;
    let rows = if state.find_state.show_replace { 2 } else { 1 };
    let bar_h = rows * row_h + (8.0 * sf) as usize;
    let pad = (8.0 * sf) as usize;
    let bar_w = (FIND_BAR_INPUT_W * sf
        + FIND_BAR_COUNT_W * sf
        + FIND_BAR_BTN_W * sf * 7.0
        + FIND_BAR_PAD_X * sf * 2.0
        + FIND_BAR_GAP * sf * 10.0) as usize;
    let bar_w = bar_w.min(vp_w.saturating_sub(pad * 2));
    let x = vp_x + vp_w.saturating_sub(bar_w + pad);
    let y = vp_y + info_bar_h + pad;
    Some((x, y, bar_w, bar_h))
}

fn find_bar_find_row_layout(
    state: &EditorState,
    x: usize,
    y: usize,
    w: usize,
    sf: f32,
) -> FindRowLayout {
    let pad_x = (FIND_BAR_PAD_X * sf) as usize;
    let gap = (FIND_BAR_GAP * sf) as usize;
    let btn_w = (FIND_BAR_BTN_W * sf) as usize;
    let count_w = (FIND_BAR_COUNT_W * sf) as usize;

    let mut cur = x + pad_x;
    let chevron_x = cur;
    cur += btn_w + gap;
    let input_x = cur;

    let right = x + w - pad_x;
    let close_x = right - btn_w;
    let next_x = close_x - gap - btn_w;
    let prev_x = next_x - gap - btn_w;
    let count_x = prev_x - gap - count_w;
    let regex_x = count_x - gap - btn_w;
    let word_x = regex_x - gap - btn_w;
    let case_x = word_x - gap - btn_w;
    let input_right = case_x - gap;
    let input_w = input_right.saturating_sub(input_x);

    let _ = state;
    FindRowLayout {
        row_y: y,
        btn_w,
        chevron_x,
        input_x,
        input_w,
        case_x,
        word_x,
        regex_x,
        count_x,
        prev_x,
        next_x,
        close_x,
    }
}

struct FindRowLayout {
    row_y: usize,
    btn_w: usize,
    chevron_x: usize,
    input_x: usize,
    input_w: usize,
    case_x: usize,
    word_x: usize,
    regex_x: usize,
    count_x: usize,
    prev_x: usize,
    next_x: usize,
    close_x: usize,
}

fn find_bar_replace_row_layout(
    x: usize,
    y: usize,
    w: usize,
    input_x_aligned: usize,
    input_w_aligned: usize,
    sf: f32,
) -> ReplaceRowLayout {
    let pad_x = (FIND_BAR_PAD_X * sf) as usize;
    let gap = (FIND_BAR_GAP * sf) as usize;
    let btn_w = (FIND_BAR_BTN_W * sf) as usize;

    let right = x + w - pad_x;
    let replace_all_x = right - btn_w;
    let replace_one_x = replace_all_x - gap - btn_w;
    let input_right = replace_one_x - gap;
    let input_x = input_x_aligned;
    let input_w = input_right
        .saturating_sub(input_x)
        .min(input_w_aligned + btn_w);

    ReplaceRowLayout {
        row_y: y,
        btn_w,
        input_x,
        input_w,
        replace_one_x,
        replace_all_x,
    }
}

struct ReplaceRowLayout {
    row_y: usize,
    btn_w: usize,
    input_x: usize,
    input_w: usize,
    replace_one_x: usize,
    replace_all_x: usize,
}

/// Draw the in-editor find/replace bar.
pub fn draw_find_bar(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    state: &EditorState,
    vp_x: usize,
    vp_y: usize,
    vp_w: usize,
    sf: f32,
    cursor_visible: bool,
) {
    let (x, y, w, h) = match find_bar_rect(state, vp_x, vp_y, vp_w, sf) {
        Some(r) => r,
        None => return,
    };

    buf.fill_rect(x, y, w, h, theme::BG_ELEVATED);
    let border = (1.0 * sf).max(1.0) as usize;
    buf.fill_rect(x, y, w, border, theme::BORDER);
    buf.fill_rect(x, y + h - border, w, border, theme::BORDER);
    buf.fill_rect(x, y, border, h, theme::BORDER);
    buf.fill_rect(x + w - border, y, border, h, theme::BORDER);

    let pad_v = ((FIND_BAR_ROW_H * sf - 22.0 * sf) / 2.0).max(2.0) as usize;
    let row_h = (FIND_BAR_ROW_H * sf) as usize;
    let find_row_y = y + (4.0 * sf) as usize;

    let find_layout = find_bar_find_row_layout(state, x, find_row_y, w, sf);
    let btn_h = row_h.saturating_sub(pad_v * 2);
    let btn_y = find_layout.row_y + pad_v;
    let metrics = Metrics::new(FIND_BAR_FONT * sf, (FIND_BAR_FONT + 4.0) * sf);
    let hovered = state.find_state.hovered_button;

    draw_find_bar_button(
        buf,
        font_system,
        swash_cache,
        find_layout.chevron_x,
        btn_y,
        find_layout.btn_w,
        btn_h,
        if state.find_state.show_replace {
            "▾"
        } else {
            "▸"
        },
        state.find_state.show_replace,
        hovered == Some(FindBarHover::ToggleExpand),
        metrics,
    );

    draw_find_bar_input(
        buf,
        font_system,
        swash_cache,
        find_layout.input_x,
        btn_y,
        find_layout.input_w,
        btn_h,
        &state.find_state.find_query,
        state.find_state.find_cursor,
        state.find_state.find_anchor,
        "Find",
        state.find_state.focus == FindFocus::Find,
        state.find_state.focus == FindFocus::Find && cursor_visible,
        metrics,
        sf,
    );

    draw_find_bar_toggle(
        buf,
        font_system,
        swash_cache,
        find_layout.case_x,
        btn_y,
        find_layout.btn_w,
        btn_h,
        "Aa",
        state.find_state.case_sensitive,
        hovered == Some(FindBarHover::ToggleCase),
        metrics,
        sf,
    );
    draw_find_bar_toggle(
        buf,
        font_system,
        swash_cache,
        find_layout.word_x,
        btn_y,
        find_layout.btn_w,
        btn_h,
        "ab",
        state.find_state.whole_word,
        hovered == Some(FindBarHover::ToggleWord),
        metrics,
        sf,
    );
    draw_find_bar_toggle(
        buf,
        font_system,
        swash_cache,
        find_layout.regex_x,
        btn_y,
        find_layout.btn_w,
        btn_h,
        ".*",
        state.find_state.regex,
        hovered == Some(FindBarHover::ToggleRegex),
        metrics,
        sf,
    );

    let count_text = if state.find_state.find_query.is_empty() {
        String::new()
    } else if state.find_state.matches.is_empty() {
        "No results".to_string()
    } else {
        let cur = state.find_state.current.map(|i| i + 1).unwrap_or(0);
        format!("{} of {}", cur, state.find_state.matches.len())
    };
    if !count_text.is_empty() {
        let count_text_y = btn_y + center_offset_y(btn_h, metrics);
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            find_layout.count_x,
            count_text_y,
            btn_y + btn_h,
            &count_text,
            metrics,
            if state.find_state.matches.is_empty() && !state.find_state.find_query.is_empty() {
                theme::ERROR_TEXT
            } else {
                theme::FG_SECONDARY
            },
            Family::SansSerif,
        );
    }

    draw_find_bar_button(
        buf,
        font_system,
        swash_cache,
        find_layout.prev_x,
        btn_y,
        find_layout.btn_w,
        btn_h,
        "↑",
        false,
        hovered == Some(FindBarHover::Prev),
        metrics,
    );
    draw_find_bar_button(
        buf,
        font_system,
        swash_cache,
        find_layout.next_x,
        btn_y,
        find_layout.btn_w,
        btn_h,
        "↓",
        false,
        hovered == Some(FindBarHover::Next),
        metrics,
    );
    draw_find_bar_button(
        buf,
        font_system,
        swash_cache,
        find_layout.close_x,
        btn_y,
        find_layout.btn_w,
        btn_h,
        "×",
        false,
        hovered == Some(FindBarHover::Close),
        metrics,
    );

    if state.find_state.show_replace {
        let replace_row_y = find_row_y + row_h;
        let replace_layout = find_bar_replace_row_layout(
            x,
            replace_row_y,
            w,
            find_layout.input_x,
            find_layout.input_w,
            sf,
        );
        let rbtn_y = replace_layout.row_y + pad_v;

        draw_find_bar_input(
            buf,
            font_system,
            swash_cache,
            replace_layout.input_x,
            rbtn_y,
            replace_layout.input_w,
            btn_h,
            &state.find_state.replace_query,
            state.find_state.replace_cursor,
            state.find_state.replace_anchor,
            "Replace",
            state.find_state.focus == FindFocus::Replace,
            state.find_state.focus == FindFocus::Replace && cursor_visible,
            metrics,
            sf,
        );
        draw_find_bar_button(
            buf,
            font_system,
            swash_cache,
            replace_layout.replace_one_x,
            rbtn_y,
            replace_layout.btn_w,
            btn_h,
            "→",
            false,
            hovered == Some(FindBarHover::ReplaceOne),
            metrics,
        );
        draw_find_bar_button(
            buf,
            font_system,
            swash_cache,
            replace_layout.replace_all_x,
            rbtn_y,
            replace_layout.btn_w,
            btn_h,
            "⇉",
            false,
            hovered == Some(FindBarHover::ReplaceAll),
            metrics,
        );
    }
}

/// Compute a y-offset from a button's top that vertically centers a glyph
/// drawn with `metrics` within a button of height `btn_h`.
fn center_offset_y(btn_h: usize, metrics: Metrics) -> usize {
    let lh = metrics.line_height;
    ((btn_h as f32 - lh) / 2.0).max(0.0) as usize
}

fn draw_find_bar_button(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    label: &str,
    active: bool,
    hovered: bool,
    metrics: Metrics,
) {
    if active {
        buf.fill_rect(x, y, w, h, theme::BG_SELECTION);
    } else if hovered {
        buf.fill_rect(x, y, w, h, theme::BG_HOVER);
    }
    let text_w = measure_text_width(font_system, label, metrics, Family::SansSerif) as usize;
    let tx = x + w.saturating_sub(text_w) / 2;
    let ty = y + center_offset_y(h, metrics);
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        tx,
        ty,
        y + h,
        label,
        metrics,
        theme::FG_PRIMARY,
        Family::SansSerif,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_find_bar_toggle(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    label: &str,
    active: bool,
    hovered: bool,
    metrics: Metrics,
    sf: f32,
) {
    if active {
        buf.fill_rect(x, y, w, h, theme::BG_SELECTION);
    } else if hovered {
        buf.fill_rect(x, y, w, h, theme::BG_HOVER);
    }
    let text_w = measure_text_width(font_system, label, metrics, Family::SansSerif) as usize;
    let tx = x + w.saturating_sub(text_w) / 2;
    let ty = y + center_offset_y(h, metrics);
    let color = if active {
        theme::PRIMARY
    } else {
        theme::FG_PRIMARY
    };
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        tx,
        ty,
        y + h,
        label,
        metrics,
        color,
        Family::SansSerif,
    );
    if active {
        let underline_h = (1.0 * sf).max(1.0) as usize;
        buf.fill_rect(
            x + (2.0 * sf) as usize,
            y + h - underline_h - 1,
            w.saturating_sub((4.0 * sf) as usize),
            underline_h,
            theme::PRIMARY,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_find_bar_input(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    text: &str,
    cursor: usize,
    anchor: Option<usize>,
    placeholder: &str,
    focused: bool,
    caret_visible: bool,
    metrics: Metrics,
    sf: f32,
) {
    buf.fill_rect(x, y, w, h, theme::BG);
    let border = (1.0 * sf).max(1.0) as usize;
    let border_color = theme::BORDER;
    buf.fill_rect(x, y, w, border, border_color);
    buf.fill_rect(x, y + h - border, w, border, border_color);
    buf.fill_rect(x, y, border, h, border_color);
    buf.fill_rect(x + w - border, y, border, h, border_color);

    let pad_x = (6.0 * sf) as usize;
    let text_x = x + pad_x;
    let text_y = y + center_offset_y(h, metrics);
    let clip_right = x + w - pad_x;

    if let Some(a) = anchor {
        let (s, e) = if a < cursor { (a, cursor) } else { (cursor, a) };
        if s != e {
            let s_clamped = s.min(text.len());
            let e_clamped = e.min(text.len());
            let before = &text[..s_clamped];
            let sel_text = &text[s_clamped..e_clamped];
            let sx = text_x
                + measure_text_width(font_system, before, metrics, Family::SansSerif) as usize;
            let sw = measure_text_width(font_system, sel_text, metrics, Family::SansSerif) as usize;
            let draw_x = sx.min(clip_right);
            let draw_w = (sx + sw).min(clip_right).saturating_sub(draw_x);
            if draw_w > 0 {
                buf.fill_rect(
                    draw_x,
                    y + 2,
                    draw_w,
                    h.saturating_sub(4),
                    theme::BG_SELECTION,
                );
            }
        }
    }

    let (render_text, color) = if text.is_empty() {
        (placeholder, theme::FG_MUTED)
    } else {
        (text, theme::FG_PRIMARY)
    };
    draw_text_clipped(
        buf,
        font_system,
        swash_cache,
        text_x,
        text_y,
        y + h,
        clip_right,
        text_x,
        render_text,
        metrics,
        color,
        Family::SansSerif,
    );

    if focused && caret_visible && !text.is_empty() {
        let before = &text[..cursor.min(text.len())];
        let cx =
            text_x + measure_text_width(font_system, before, metrics, Family::SansSerif) as usize;
        if cx < clip_right {
            let caret_w = (1.0 * sf).max(1.0) as usize;
            let caret_h = h.saturating_sub((4.0 * sf) as usize);
            buf.fill_rect(cx, y + 2, caret_w, caret_h, theme::FG_PRIMARY);
        }
    } else if focused && caret_visible && text.is_empty() {
        let caret_w = (1.0 * sf).max(1.0) as usize;
        let caret_h = h.saturating_sub((4.0 * sf) as usize);
        buf.fill_rect(text_x, y + 2, caret_w, caret_h, theme::FG_PRIMARY);
    }
}

/// Hit-test a physical (px, py) against the visible find bar regions.
pub fn find_bar_hit_test(
    state: &EditorState,
    px: usize,
    py: usize,
    vp_x: usize,
    vp_y: usize,
    vp_w: usize,
    sf: f32,
) -> Option<FindBarHit> {
    let (x, y, w, _h) = find_bar_rect(state, vp_x, vp_y, vp_w, sf)?;
    let row_h = (FIND_BAR_ROW_H * sf) as usize;
    let find_row_y = y + (4.0 * sf) as usize;
    let find_layout = find_bar_find_row_layout(state, x, find_row_y, w, sf);
    let pad_v = ((FIND_BAR_ROW_H * sf - 22.0 * sf) / 2.0).max(2.0) as usize;
    let btn_y = find_layout.row_y + pad_v;
    let btn_h = row_h.saturating_sub(pad_v * 2);

    let on_row = |bx: usize, bw: usize| -> bool {
        py >= btn_y && py < btn_y + btn_h && px >= bx && px < bx + bw
    };

    if on_row(find_layout.chevron_x, find_layout.btn_w) {
        return Some(FindBarHit::ToggleExpand);
    }
    if on_row(find_layout.input_x, find_layout.input_w) {
        return Some(FindBarHit::FindInput {
            rel_x: px - find_layout.input_x,
        });
    }
    if on_row(find_layout.case_x, find_layout.btn_w) {
        return Some(FindBarHit::ToggleCase);
    }
    if on_row(find_layout.word_x, find_layout.btn_w) {
        return Some(FindBarHit::ToggleWord);
    }
    if on_row(find_layout.regex_x, find_layout.btn_w) {
        return Some(FindBarHit::ToggleRegex);
    }
    if on_row(find_layout.prev_x, find_layout.btn_w) {
        return Some(FindBarHit::Prev);
    }
    if on_row(find_layout.next_x, find_layout.btn_w) {
        return Some(FindBarHit::Next);
    }
    if on_row(find_layout.close_x, find_layout.btn_w) {
        return Some(FindBarHit::Close);
    }

    if state.find_state.show_replace {
        let replace_row_y = find_row_y + row_h;
        let replace_layout = find_bar_replace_row_layout(
            x,
            replace_row_y,
            w,
            find_layout.input_x,
            find_layout.input_w,
            sf,
        );
        let rbtn_y = replace_layout.row_y + pad_v;
        let on_rrow = |bx: usize, bw: usize| -> bool {
            py >= rbtn_y && py < rbtn_y + btn_h && px >= bx && px < bx + bw
        };
        if on_rrow(replace_layout.input_x, replace_layout.input_w) {
            return Some(FindBarHit::ReplaceInput {
                rel_x: px - replace_layout.input_x,
            });
        }
        if on_rrow(replace_layout.replace_one_x, replace_layout.btn_w) {
            return Some(FindBarHit::ReplaceOne);
        }
        if on_rrow(replace_layout.replace_all_x, replace_layout.btn_w) {
            return Some(FindBarHit::ReplaceAll);
        }
    }
    None
}

/// Hit-test for hover: returns which region is under (px, py), ignoring
/// `rel_x` inside inputs (i.e. this is a coarser version of `find_bar_hit_test`
/// used for hover highlighting).
pub fn find_bar_hover_test(
    state: &EditorState,
    px: usize,
    py: usize,
    vp_x: usize,
    vp_y: usize,
    vp_w: usize,
    sf: f32,
) -> Option<FindBarHover> {
    find_bar_hit_test(state, px, py, vp_x, vp_y, vp_w, sf).map(|hit| match hit {
        FindBarHit::ToggleExpand => FindBarHover::ToggleExpand,
        FindBarHit::FindInput { .. } => FindBarHover::FindInput,
        FindBarHit::ToggleCase => FindBarHover::ToggleCase,
        FindBarHit::ToggleWord => FindBarHover::ToggleWord,
        FindBarHit::ToggleRegex => FindBarHover::ToggleRegex,
        FindBarHit::Prev => FindBarHover::Prev,
        FindBarHit::Next => FindBarHover::Next,
        FindBarHit::Close => FindBarHover::Close,
        FindBarHit::ReplaceInput { .. } => FindBarHover::ReplaceInput,
        FindBarHit::ReplaceOne => FindBarHover::ReplaceOne,
        FindBarHit::ReplaceAll => FindBarHover::ReplaceAll,
    })
}

/// Returns the absolute physical rect (x, y, w, h) of the find input box,
/// or None if the find bar is not visible.
pub fn find_bar_find_input_rect(
    state: &EditorState,
    vp_x: usize,
    vp_y: usize,
    vp_w: usize,
    sf: f32,
) -> Option<(usize, usize, usize, usize)> {
    let (x, y, w, _h) = find_bar_rect(state, vp_x, vp_y, vp_w, sf)?;
    let row_h = (FIND_BAR_ROW_H * sf) as usize;
    let find_row_y = y + (4.0 * sf) as usize;
    let find_layout = find_bar_find_row_layout(state, x, find_row_y, w, sf);
    let pad_v = ((FIND_BAR_ROW_H * sf - 22.0 * sf) / 2.0).max(2.0) as usize;
    let btn_h = row_h.saturating_sub(pad_v * 2);
    let btn_y = find_layout.row_y + pad_v;
    Some((find_layout.input_x, btn_y, find_layout.input_w, btn_h))
}

/// Returns the absolute physical rect (x, y, w, h) of the replace input box,
/// or None if the replace row isn't visible.
pub fn find_bar_replace_input_rect(
    state: &EditorState,
    vp_x: usize,
    vp_y: usize,
    vp_w: usize,
    sf: f32,
) -> Option<(usize, usize, usize, usize)> {
    if !state.find_state.show_replace {
        return None;
    }
    let (x, y, w, _h) = find_bar_rect(state, vp_x, vp_y, vp_w, sf)?;
    let row_h = (FIND_BAR_ROW_H * sf) as usize;
    let find_row_y = y + (4.0 * sf) as usize;
    let find_layout = find_bar_find_row_layout(state, x, find_row_y, w, sf);
    let pad_v = ((FIND_BAR_ROW_H * sf - 22.0 * sf) / 2.0).max(2.0) as usize;
    let btn_h = row_h.saturating_sub(pad_v * 2);
    let replace_row_y = find_row_y + row_h;
    let replace_layout = find_bar_replace_row_layout(
        x,
        replace_row_y,
        w,
        find_layout.input_x,
        find_layout.input_w,
        sf,
    );
    let rbtn_y = replace_layout.row_y + pad_v;
    Some((
        replace_layout.input_x,
        rbtn_y,
        replace_layout.input_w,
        btn_h,
    ))
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
