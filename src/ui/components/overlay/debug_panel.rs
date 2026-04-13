//! Debug information overlay panel.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::draw_border;

pub fn draw_debug(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    info: &str,
    sf: f32,
) {
    let w = buf.width;
    let h = buf.height;

    let font_size = 11.0 * sf;
    let line_height = 16.0 * sf;
    let metrics = Metrics::new(font_size, line_height);

    let pad_x = (8.0 * sf) as usize;
    let pad_y = (6.0 * sf) as usize;
    let char_w = 7.0 * sf;
    let panel_w = (info.len() as f32 * char_w) as usize + pad_x * 2;
    let panel_h = line_height as usize + pad_y * 2;

    let panel_x = w.saturating_sub(panel_w + (10.0 * sf) as usize);
    let panel_y = h.saturating_sub(panel_h + (10.0 * sf) as usize);
    let bw = (1.0 * sf).max(1.0) as usize;

    buf.fill_rect(panel_x, panel_y, panel_w, panel_h, theme::DEBUG_BG);
    draw_border(
        buf,
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        bw,
        theme::PALETTE_BORDER,
    );

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        panel_x + pad_x,
        panel_y + pad_y,
        h,
        info,
        metrics,
        theme::DEBUG_TEXT,
        Family::Monospace,
    );
}
