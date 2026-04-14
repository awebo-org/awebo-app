//! Terminal grid renderer using `Term::renderable_content()`.
//!
//! Properly handles TUI app color overrides, cursor rendering,
//! and selection highlighting. Uses integer-snapped cell positions
//! to avoid subpixel gaps.
//!
//! Text characters are NOT blitted to the CPU pixel buffer — instead
//! they are collected as [`CellGlyph`] descriptors and returned to the
//! caller for GPU instanced rendering. Backgrounds, cursors, and
//! box-drawing primitives remain CPU-drawn.

use std::sync::Arc;

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor};

use crate::renderer::gpu_grid::CellGlyph;
use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::theme;
use crate::terminal::JsonEventProxy;

type CellEntry = (char, Rgb, Rgb, CellFlags, bool);

/// Cached grid state from previous frame for dirty-row optimization.
pub struct GridCache {
    cells: Vec<Vec<CellEntry>>,
    cursor_row: usize,
    cursor_col: usize,
    cursor_shape: CursorShape,
    had_overlay: bool,
    /// Reusable grid buffer to avoid per-frame allocation.
    grid_buf: Vec<Vec<CellEntry>>,
}

impl GridCache {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            cursor_row: usize::MAX,
            cursor_col: usize::MAX,
            cursor_shape: CursorShape::Hidden,
            had_overlay: false,
            grid_buf: Vec::new(),
        }
    }
}

impl Default for GridCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Draw the terminal grid into the pixel buffer.
///
/// `y_offset` is the first pixel row where the grid starts (below tab bar).
/// `pad` is the padding in physical pixels applied on all four sides of the grid.
///
/// Returns a list of [`CellGlyph`] descriptors for every regular text
/// character that should be rendered via the GPU instanced pipeline.
/// Box-drawing / block-element characters are drawn directly into the
/// CPU pixel buffer (they need precise geometric primitives).
pub fn draw(
    buf: &mut PixelBuffer,
    term_handle: &Arc<FairMutex<Term<JsonEventProxy>>>,
    y_offset: usize,
    pad: usize,
    x_offset: usize,
    cell_width: f32,
    cell_height: f32,
    scale_factor: f64,
    cache: &mut GridCache,
    overlay_active: bool,
    font_size: f32,
    bg_override: Option<Rgb>,
    avail_h: usize,
) -> Vec<CellGlyph> {
    let force_dirty = overlay_active || cache.had_overlay;
    cache.had_overlay = overlay_active;

    let term = term_handle.lock();
    let content = term.renderable_content();

    let colors = content.colors;
    let cursor = content.cursor;
    let display_offset = content.display_offset;
    let selection = content.selection;
    let screen_lines = term.grid().screen_lines();
    let cols = term.grid().columns();

    let cell_w = cell_width.round() as usize;
    let cell_h = cell_height.round() as usize;

    let x_pad = pad + x_offset;
    let y_pad = pad;

    let w = buf.width;

    let default_fg = theme::resolve_color(&AnsiColor::Named(NamedColor::Foreground), colors);
    let default_bg = bg_override.unwrap_or(theme::BG);
    let default_cell: CellEntry = (' ', default_fg, default_bg, CellFlags::empty(), false);

    cache
        .grid_buf
        .resize_with(screen_lines, || vec![default_cell; cols]);
    for row in cache.grid_buf.iter_mut() {
        row.resize(cols, default_cell);
        row.fill(default_cell);
    }

    for indexed in content.display_iter {
        let point = indexed.point;
        let viewport_row = (point.line.0 + display_offset as i32) as usize;
        let col = point.column.0;

        if viewport_row >= screen_lines || col >= cols {
            continue;
        }

        let cell = &indexed.cell;
        let flags = cell.flags;
        let inverse = flags.contains(CellFlags::INVERSE);
        let hidden = flags.contains(CellFlags::HIDDEN);

        let is_selected = selection
            .as_ref()
            .map(|sel| sel.contains(point))
            .unwrap_or(false);

        let (raw_fg, raw_bg) = if inverse {
            (&cell.bg, &cell.fg)
        } else {
            (&cell.fg, &cell.bg)
        };

        let mut fg = theme::resolve_color(raw_fg, colors);
        let bg = theme::resolve_color(raw_bg, colors);
        let bg = if bg_override.is_some() && bg == theme::named_bg() {
            default_bg
        } else {
            bg
        };

        if flags.contains(CellFlags::DIM) {
            fg = dim_color(fg);
        }

        let (fg, bg) = if is_selected { (bg, fg) } else { (fg, bg) };
        let c = if hidden { ' ' } else { cell.c };

        cache.grid_buf[viewport_row][col] = (c, fg, bg, flags, is_selected);
    }

    let crow = if cursor.shape != CursorShape::Hidden {
        let r = (cursor.point.line.0 + display_offset as i32) as usize;
        if r < screen_lines { r } else { usize::MAX }
    } else {
        usize::MAX
    };
    let ccol = cursor.point.column.0;

    let cursor_moved =
        crow != cache.cursor_row || ccol != cache.cursor_col || cursor.shape != cache.cursor_shape;

    let mut dirty = vec![false; screen_lines];
    for (row_idx, row) in cache.grid_buf.iter().enumerate() {
        let is_dirty = force_dirty
            || row_idx >= cache.cells.len()
            || cache.cells[row_idx].len() != row.len()
            || cache.cells[row_idx] != *row;
        if is_dirty {
            dirty[row_idx] = true;
        }
    }
    if cursor_moved {
        if cache.cursor_row < screen_lines {
            dirty[cache.cursor_row] = true;
        }
        if crow < screen_lines {
            dirty[crow] = true;
        }
    }

    let sf = scale_factor as f32;

    if cursor.shape == CursorShape::Block && crow < screen_lines && ccol < cols {
        cache.grid_buf[crow][ccol].1 = default_bg;
    }

    let mut cell_glyphs = Vec::with_capacity(screen_lines * cols);

    for (row_idx, row) in cache.grid_buf.iter().enumerate() {
        let base_y = y_offset + y_pad + row_idx * cell_h;
        let is_dirty = dirty[row_idx];

        if is_dirty {
            buf.fill_rect(
                x_pad,
                base_y,
                w.saturating_sub(x_pad + pad),
                cell_h,
                default_bg,
            );

            for (col_idx, &(_, _, bg, _, is_sel)) in row.iter().enumerate() {
                if bg == default_bg && !is_sel {
                    continue;
                }
                let cell_x = x_pad + col_idx * cell_w;
                buf.fill_rect(cell_x, base_y, cell_w, cell_h, bg);
            }
        }

        for (col_idx, &(c, fg, _bg, flags, _)) in row.iter().enumerate() {
            if c <= ' ' || flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                continue;
            }
            if is_box_or_block(c) {
                if is_dirty {
                    draw_box_or_block(
                        buf,
                        c,
                        x_pad + col_idx * cell_w,
                        base_y,
                        cell_w,
                        cell_h,
                        fg,
                        _bg,
                        sf,
                    );
                }
                continue;
            }
            cell_glyphs.push(CellGlyph {
                px: x_pad + col_idx * cell_w,
                py: base_y,
                ch: c,
                fg,
                font_size,
                line_height: cell_height,
                bold: false,
                italic: false,
            });
        }
    }

    if cursor.shape != CursorShape::Hidden && crow < screen_lines && ccol < cols {
        let cx = x_pad + ccol * cell_w;
        let cy = y_offset + y_pad + crow * cell_h;

        let cursor_color = colors[NamedColor::Cursor]
            .map(|rgb| (rgb.r, rgb.g, rgb.b))
            .unwrap_or((171, 178, 191));

        match cursor.shape {
            CursorShape::Block => {
                buf.fill_rect(cx, cy, cell_w, cell_h, cursor_color);
            }
            CursorShape::Beam => {
                let beam_w = (2.0 * sf).max(1.0) as usize;
                buf.fill_rect(cx, cy, beam_w, cell_h, cursor_color);
            }
            CursorShape::Underline => {
                let uh = (2.0 * sf).max(1.0) as usize;
                buf.fill_rect(cx, cy + cell_h.saturating_sub(uh), cell_w, uh, cursor_color);
            }
            CursorShape::HollowBlock => {
                let bw = (1.0 * sf).max(1.0) as usize;
                buf.fill_rect(cx, cy, cell_w, bw, cursor_color);
                buf.fill_rect(cx, cy + cell_h.saturating_sub(bw), cell_w, bw, cursor_color);
                buf.fill_rect(cx, cy, bw, cell_h, cursor_color);
                buf.fill_rect(cx + cell_w.saturating_sub(bw), cy, bw, cell_h, cursor_color);
            }
            CursorShape::Hidden => {}
        }
    }

    let rows_end_y = y_offset + y_pad + screen_lines * cell_h;
    let grid_end_y = y_offset + avail_h;
    if grid_end_y > rows_end_y {
        buf.fill_rect(
            x_pad,
            rows_end_y,
            w.saturating_sub(x_pad + pad),
            grid_end_y - rows_end_y,
            default_bg,
        );
    }

    std::mem::swap(&mut cache.cells, &mut cache.grid_buf);
    cache.cursor_row = crow;
    cache.cursor_col = ccol;
    cache.cursor_shape = cursor.shape;

    cell_glyphs
}

#[inline]
fn dim_color(c: Rgb) -> Rgb {
    (
        (c.0 as f32 * 0.66) as u8,
        (c.1 as f32 * 0.66) as u8,
        (c.2 as f32 * 0.66) as u8,
    )
}

/// Returns true if `c` is a box-drawing (U+2500–U+257F) or block element
/// (U+2580–U+259F) character that we render as primitives rather than via
/// the font. Font glyphs for these characters typically don't fill the full
/// cell vertically, which causes visible gaps between stacked characters
/// (e.g. `│` columns in TUI borders, or block-element "pixel" logos).
#[inline]
fn is_box_or_block(c: char) -> bool {
    let cp = c as u32;
    (0x2500..=0x259F).contains(&cp)
}

/// Render a box-drawing or block element character directly as rectangles,
/// sized to fill the entire cell. This is how mature terminals (Ghostty,
/// WezTerm, Alacritty) avoid gaps in TUI borders and make block-element art
/// look crisp.
fn draw_box_or_block(
    buf: &mut PixelBuffer,
    c: char,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    fg: Rgb,
    bg: Rgb,
    sf: f32,
) {
    let cp = c as u32;

    if (0x2580..=0x259F).contains(&cp) {
        draw_block_element(buf, c, x, y, w, h, fg, bg);
        return;
    }

    let light = ((sf).round() as usize).max(1);
    let heavy = ((sf * 2.0).round() as usize).max(light + 1);

    let (up, down, left, right): (u8, u8, u8, u8) = match c {
        '─' => (0, 0, 1, 1),
        '━' => (0, 0, 2, 2),
        '│' => (1, 1, 0, 0),
        '┃' => (2, 2, 0, 0),
        '┌' | '╭' => (0, 1, 0, 1),
        '┍' => (0, 1, 0, 2),
        '┎' => (0, 2, 0, 1),
        '┏' => (0, 2, 0, 2),
        '┐' | '╮' => (0, 1, 1, 0),
        '┑' => (0, 1, 2, 0),
        '┒' => (0, 2, 1, 0),
        '┓' => (0, 2, 2, 0),
        '└' | '╰' => (1, 0, 0, 1),
        '┕' => (1, 0, 0, 2),
        '┖' => (2, 0, 0, 1),
        '┗' => (2, 0, 0, 2),
        '┘' | '╯' => (1, 0, 1, 0),
        '┙' => (1, 0, 2, 0),
        '┚' => (2, 0, 1, 0),
        '┛' => (2, 0, 2, 0),
        '├' => (1, 1, 0, 1),
        '┝' => (1, 1, 0, 2),
        '┞' => (2, 1, 0, 1),
        '┟' => (1, 2, 0, 1),
        '┠' => (2, 2, 0, 1),
        '┡' => (2, 1, 0, 2),
        '┢' => (1, 2, 0, 2),
        '┣' => (2, 2, 0, 2),
        '┤' => (1, 1, 1, 0),
        '┥' => (1, 1, 2, 0),
        '┦' => (2, 1, 1, 0),
        '┧' => (1, 2, 1, 0),
        '┨' => (2, 2, 1, 0),
        '┩' => (2, 1, 2, 0),
        '┪' => (1, 2, 2, 0),
        '┫' => (2, 2, 2, 0),
        '┬' => (0, 1, 1, 1),
        '┭' => (0, 1, 2, 1),
        '┮' => (0, 1, 1, 2),
        '┯' => (0, 1, 2, 2),
        '┰' => (0, 2, 1, 1),
        '┱' => (0, 2, 2, 1),
        '┲' => (0, 2, 1, 2),
        '┳' => (0, 2, 2, 2),
        '┴' => (1, 0, 1, 1),
        '┵' => (1, 0, 2, 1),
        '┶' => (1, 0, 1, 2),
        '┷' => (1, 0, 2, 2),
        '┸' => (2, 0, 1, 1),
        '┹' => (2, 0, 2, 1),
        '┺' => (2, 0, 1, 2),
        '┻' => (2, 0, 2, 2),
        '┼' => (1, 1, 1, 1),
        '╋' => (2, 2, 2, 2),
        _ => return,
    };

    let thick = |weight: u8| -> usize {
        match weight {
            1 => light,
            2 => heavy,
            _ => 0,
        }
    };

    let cx = w / 2;
    let cy = h / 2;

    if left > 0 {
        let t = thick(left);
        let y0 = cy.saturating_sub(t / 2);
        buf.fill_rect(x, y + y0, cx + t / 2 + (t % 2), t, fg);
    }
    if right > 0 {
        let t = thick(right);
        let y0 = cy.saturating_sub(t / 2);
        let start = cx.saturating_sub(t / 2);
        buf.fill_rect(x + start, y + y0, w - start, t, fg);
    }
    if up > 0 {
        let t = thick(up);
        let x0 = cx.saturating_sub(t / 2);
        buf.fill_rect(x + x0, y, t, cy + t / 2 + (t % 2), fg);
    }
    if down > 0 {
        let t = thick(down);
        let x0 = cx.saturating_sub(t / 2);
        let start = cy.saturating_sub(t / 2);
        buf.fill_rect(x + x0, y + start, t, h - start, fg);
    }
}

/// Render a U+2580..U+259F block element. Uses primitive fills so rows and
/// columns of blocks tile seamlessly, which is how ASCII-art pixel logos
/// (e.g. the opencode splash) are supposed to look.
fn draw_block_element(
    buf: &mut PixelBuffer,
    c: char,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    fg: Rgb,
    bg: Rgb,
) {
    let fill = |buf: &mut PixelBuffer, rx: usize, ry: usize, rw: usize, rh: usize, color: Rgb| {
        buf.fill_rect(
            x + rx.min(w),
            y + ry.min(h),
            rw.min(w - rx.min(w)),
            rh.min(h - ry.min(h)),
            color,
        );
    };
    let shade = |buf: &mut PixelBuffer, alpha: f32| {
        let mix = |f: u8, b: u8| -> u8 {
            (f as f32 * alpha + b as f32 * (1.0 - alpha))
                .round()
                .clamp(0.0, 255.0) as u8
        };
        let color = (mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2));
        buf.fill_rect(x, y, w, h, color);
    };

    let hh = |n: usize| (h * n + 4) / 8;
    let wh = |n: usize| (w * n + 4) / 8;

    match c {
        '▀' => fill(buf, 0, 0, w, h / 2, fg), // upper half
        '▁' => {
            let t = hh(1);
            fill(buf, 0, h - t, w, t, fg);
        }
        '▂' => {
            let t = hh(2);
            fill(buf, 0, h - t, w, t, fg);
        }
        '▃' => {
            let t = hh(3);
            fill(buf, 0, h - t, w, t, fg);
        }
        '▄' => {
            let t = hh(4);
            fill(buf, 0, h - t, w, t, fg);
        }
        '▅' => {
            let t = hh(5);
            fill(buf, 0, h - t, w, t, fg);
        }
        '▆' => {
            let t = hh(6);
            fill(buf, 0, h - t, w, t, fg);
        }
        '▇' => {
            let t = hh(7);
            fill(buf, 0, h - t, w, t, fg);
        }
        '█' => fill(buf, 0, 0, w, h, fg), // full block
        '▉' => {
            let t = wh(7);
            fill(buf, 0, 0, t, h, fg);
        }
        '▊' => {
            let t = wh(6);
            fill(buf, 0, 0, t, h, fg);
        }
        '▋' => {
            let t = wh(5);
            fill(buf, 0, 0, t, h, fg);
        }
        '▌' => fill(buf, 0, 0, w / 2, h, fg), // left half
        '▍' => {
            let t = wh(3);
            fill(buf, 0, 0, t, h, fg);
        }
        '▎' => {
            let t = wh(2);
            fill(buf, 0, 0, t, h, fg);
        }
        '▏' => {
            let t = wh(1);
            fill(buf, 0, 0, t, h, fg);
        }
        '▐' => fill(buf, w / 2, 0, w - w / 2, h, fg), // right half
        '░' => shade(buf, 0.25),
        '▒' => shade(buf, 0.50),
        '▓' => shade(buf, 0.75),
        '▔' => {
            let t = hh(1);
            fill(buf, 0, 0, w, t, fg);
        }
        '▕' => {
            let t = wh(1);
            fill(buf, w - t, 0, t, h, fg);
        }
        '▖' => fill(buf, 0, h / 2, w / 2, h - h / 2, fg),
        '▗' => fill(buf, w / 2, h / 2, w - w / 2, h - h / 2, fg),
        '▘' => fill(buf, 0, 0, w / 2, h / 2, fg),
        '▙' => {
            fill(buf, 0, 0, w / 2, h / 2, fg);
            fill(buf, 0, h / 2, w, h - h / 2, fg);
        }
        '▚' => {
            fill(buf, 0, 0, w / 2, h / 2, fg);
            fill(buf, w / 2, h / 2, w - w / 2, h - h / 2, fg);
        }
        '▛' => {
            fill(buf, 0, 0, w, h / 2, fg);
            fill(buf, 0, h / 2, w / 2, h - h / 2, fg);
        }
        '▜' => {
            fill(buf, 0, 0, w, h / 2, fg);
            fill(buf, w / 2, h / 2, w - w / 2, h - h / 2, fg);
        }
        '▝' => fill(buf, w / 2, 0, w - w / 2, h / 2, fg),
        '▞' => {
            fill(buf, w / 2, 0, w - w / 2, h / 2, fg);
            fill(buf, 0, h / 2, w / 2, h - h / 2, fg);
        }
        '▟' => {
            fill(buf, w / 2, 0, w - w / 2, h / 2, fg);
            fill(buf, 0, h / 2, w, h - h / 2, fg);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dim_color_reduces_brightness() {
        let c = dim_color((100, 200, 50));
        assert!(c.0 < 100);
        assert!(c.1 < 200);
        assert!(c.2 < 50);
    }

    #[test]
    fn dim_color_black_stays_black() {
        assert_eq!(dim_color((0, 0, 0)), (0, 0, 0));
    }

    #[test]
    fn is_box_or_block_box_drawing() {
        assert!(is_box_or_block('─'));
        assert!(is_box_or_block('│'));
        assert!(is_box_or_block('┌'));
        assert!(is_box_or_block('▀'));
        assert!(is_box_or_block('█'));
    }

    #[test]
    fn is_box_or_block_normal_chars() {
        assert!(!is_box_or_block('A'));
        assert!(!is_box_or_block(' '));
        assert!(!is_box_or_block('€'));
    }

    #[test]
    fn grid_cache_new_is_empty() {
        let cache = GridCache::new();
        assert!(cache.cells.is_empty());
        assert_eq!(cache.cursor_row, usize::MAX);
        assert_eq!(cache.cursor_col, usize::MAX);
        assert!(!cache.had_overlay);
    }

    #[test]
    fn grid_cache_default_matches_new() {
        let cache = GridCache::default();
        assert!(cache.cells.is_empty());
    }
}
