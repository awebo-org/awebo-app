//! Command palette overlay.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::draw_border;

/// State passed from App to renderer for the command palette overlay.
pub struct PaletteState {
    pub query: String,
    pub commands: Vec<String>,
    pub shortcuts: Vec<String>,
    pub selected: usize,
}

pub fn draw_palette(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    palette: &PaletteState,
    sf: f32,
) {
    let w = buf.width;
    let h = buf.height;

    buf.dim(0.4);

    let pw = (400.0 * sf) as usize;
    let item_h = (32.0 * sf) as usize;
    let input_h = (36.0 * sf) as usize;
    let pad = (8.0 * sf) as usize;
    let bw = (1.0 * sf).max(1.0) as usize;

    let num_items = palette.commands.len().min(8);
    let ph = input_h + pad + num_items * item_h + pad * 2;

    let px = w.saturating_sub(pw) / 2;
    let py = (h / 5).min(h.saturating_sub(ph));

    buf.fill_rect(px, py, pw, ph, theme::PALETTE_BG);
    draw_border(buf, px, py, pw, ph, bw, theme::PALETTE_BORDER);

    let input_x = px + pad;
    let input_y = py + pad;
    let input_w = pw - pad * 2;
    buf.fill_rect(input_x, input_y, input_w, input_h, theme::PALETTE_INPUT_BG);
    draw_border(
        buf,
        input_x,
        input_y,
        input_w,
        input_h,
        bw,
        theme::PALETTE_BORDER,
    );

    let font_size = 14.0 * sf;
    let line_height = 20.0 * sf;
    let metrics = Metrics::new(font_size, line_height);

    let text_x = input_x + (8.0 * sf) as usize;
    let text_y = input_y + ((input_h as f32 - line_height) / 2.0) as usize;

    if palette.query.is_empty() {
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            text_x,
            text_y,
            h,
            "Type a command...",
            metrics,
            theme::PALETTE_DIM_TEXT,
            Family::Monospace,
        );
    } else {
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            text_x,
            text_y,
            h,
            &palette.query,
            metrics,
            theme::PALETTE_TEXT,
            Family::Monospace,
        );
    }

    let list_y = input_y + input_h + pad;
    let item_metrics = Metrics::new(13.0 * sf, 18.0 * sf);

    for (i, label) in palette.commands.iter().take(8).enumerate() {
        let iy = list_y + i * item_h;

        if i == palette.selected {
            buf.fill_rect(
                px + pad,
                iy,
                pw - pad * 2,
                item_h,
                theme::PALETTE_SELECTED_BG,
            );
        }

        let color = if i == palette.selected {
            theme::PALETTE_TEXT
        } else {
            theme::PALETTE_DIM_TEXT
        };
        let item_x = px + pad + (12.0 * sf) as usize;
        let item_y = iy + ((item_h as f32 - 18.0 * sf) / 2.0) as usize;

        draw_text_at(
            buf,
            font_system,
            swash_cache,
            item_x,
            item_y,
            h,
            label,
            item_metrics,
            color,
            Family::Monospace,
        );

        if let Some(shortcut) = palette.shortcuts.get(i)
            && !shortcut.is_empty()
        {
            let sc_metrics = Metrics::new(11.0 * sf, 16.0 * sf);
            let sc_w = shortcut.len() as f32 * 7.0 * sf;
            let sc_x = (px + pw - pad - (12.0 * sf) as usize).saturating_sub(sc_w as usize);
            let sc_y = iy + ((item_h as f32 - 16.0 * sf) / 2.0) as usize;
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                sc_x,
                sc_y,
                h,
                shortcut,
                sc_metrics,
                theme::PALETTE_DIM_TEXT,
                Family::Monospace,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_state_construction() {
        let state = PaletteState {
            query: "test".into(),
            commands: vec!["cmd1".into()],
            shortcuts: vec!["Ctrl+A".into()],
            selected: 0,
        };
        assert_eq!(state.query, "test");
        assert_eq!(state.commands.len(), 1);
    }
}
