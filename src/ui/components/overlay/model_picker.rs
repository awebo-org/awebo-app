//! Model picker overlay — select and load AI models.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::draw_border;

/// Per-item status passed from App.
#[derive(Clone)]
pub struct ModelPickerItem {
    pub name: String,
    pub quant_label: String,
    pub status: ModelStatus,
}

/// Download/load status of an AI model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelStatus {
    Loaded,
    Downloaded,
    NotDownloaded,
}

/// State passed from App to renderer for the model picker overlay.
pub struct ModelPickerState {
    pub items: Vec<ModelPickerItem>,
    pub selected: usize,
}

pub fn draw_model_picker(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    state: &ModelPickerState,
    sf: f32,
) {
    let w = buf.width;
    let h = buf.height;

    buf.dim(0.4);

    let pw = (420.0 * sf) as usize;
    let item_h = (36.0 * sf) as usize;
    let header_h = (40.0 * sf) as usize;
    let pad = (8.0 * sf) as usize;
    let bw = (1.0 * sf).max(1.0) as usize;

    let num_items = state.items.len().min(8);
    let ph = header_h + num_items * item_h + pad * 2;

    let px = w.saturating_sub(pw) / 2;
    let py = (h / 5).min(h.saturating_sub(ph));

    buf.fill_rect(px, py, pw, ph, theme::PALETTE_BG);
    draw_border(buf, px, py, pw, ph, bw, theme::PALETTE_BORDER);

    let font_size = 14.0 * sf;
    let line_height = 20.0 * sf;
    let header_metrics = Metrics::new(font_size, line_height);

    let title_x = px + pad + (8.0 * sf) as usize;
    let title_y = py + ((header_h as f32 - line_height) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        title_x,
        title_y,
        h,
        "Select Model",
        header_metrics,
        theme::PALETTE_TEXT,
        Family::Monospace,
    );

    let sep_y = py + header_h;
    buf.fill_rect(px + pad, sep_y, pw - pad * 2, bw, theme::PALETTE_BORDER);

    let list_y = sep_y + pad;
    let item_metrics = Metrics::new(13.0 * sf, 18.0 * sf);
    let status_metrics = Metrics::new(11.0 * sf, 16.0 * sf);

    for (i, item) in state.items.iter().take(8).enumerate() {
        let iy = list_y + i * item_h;

        if i == state.selected {
            buf.fill_rect(
                px + pad,
                iy,
                pw - pad * 2,
                item_h,
                theme::PALETTE_SELECTED_BG,
            );
        }

        let name_color = if i == state.selected {
            theme::PALETTE_TEXT
        } else {
            theme::PALETTE_DIM_TEXT
        };

        let label = format!("{} [{}]", item.name, item.quant_label);
        let item_x = px + pad + (12.0 * sf) as usize;
        let item_y = iy + ((item_h as f32 - 18.0 * sf) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            item_x,
            item_y,
            h,
            &label,
            item_metrics,
            name_color,
            Family::Monospace,
        );

        let (status_text, status_color) = match item.status {
            ModelStatus::Loaded => ("loaded", theme::SUCCESS),
            ModelStatus::Downloaded => ("downloaded", theme::PALETTE_DIM_TEXT),
            ModelStatus::NotDownloaded => ("not downloaded", theme::FG_DIM),
        };
        let sc_w = status_text.len() as f32 * 7.0 * sf;
        let sc_x = (px + pw - pad - (12.0 * sf) as usize).saturating_sub(sc_w as usize);
        let sc_y = iy + ((item_h as f32 - 16.0 * sf) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            sc_x,
            sc_y,
            h,
            status_text,
            status_metrics,
            status_color,
            Family::Monospace,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_status_equality() {
        assert_eq!(ModelStatus::Loaded, ModelStatus::Loaded);
        assert_ne!(ModelStatus::Loaded, ModelStatus::Downloaded);
        assert_ne!(ModelStatus::Downloaded, ModelStatus::NotDownloaded);
    }

    #[test]
    fn model_picker_state_construction() {
        let state = ModelPickerState {
            items: vec![ModelPickerItem {
                name: "test".into(),
                quant_label: "Q4_K_M".into(),
                status: ModelStatus::NotDownloaded,
            }],
            selected: 0,
        };
        assert_eq!(state.items.len(), 1);
        assert_eq!(state.selected, 0);
    }
}
