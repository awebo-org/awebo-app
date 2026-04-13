//! Font picker dropdown for the settings appearance tab.

use std::collections::BTreeSet;

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::SettingsState;
use crate::ui::components::overlay::draw_border;

/// Scan the font system for monospace families.
pub fn detect_monospace_fonts(font_system: &FontSystem) -> Vec<String> {
    let db = font_system.db();
    let mut families = BTreeSet::new();
    families.insert("Monospace".to_string());
    for face in db.faces() {
        if !face.monospaced {
            continue;
        }
        if let Some((name, _)) = face.families.first() {
            if name.is_empty() || name.starts_with('.') {
                continue;
            }
            let lower = name.to_lowercase();
            if lower.contains("bitmap") || lower.contains("lastresort") {
                continue;
            }
            families.insert(name.clone());
        }
    }
    families.into_iter().collect()
}

pub fn draw_font_picker(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    state: &SettingsState,
    y_offset: usize,
    content_area_x: usize,
    content_area_w: usize,
    sf: f32,
) {
    let (anchor_x, anchor_y) = font_picker_anchor(y_offset, content_area_x, content_area_w, sf);
    let item_h = (32.0 * sf) as usize;
    let pad = (6.0 * sf) as usize;
    let bw = (1.0 * sf).max(1.0) as usize;
    let pw = (220.0 * sf) as usize;
    let ph = state.font_options.len() * item_h + pad * 2;
    let corner_r = (6.0 * sf) as usize;

    let px = anchor_x;
    let py = anchor_y + (4.0 * sf) as usize;

    crate::ui::components::overlay::fill_rounded_rect(
        buf,
        px,
        py,
        pw,
        ph,
        corner_r,
        theme::SHELL_PICKER_BG,
    );
    draw_border(buf, px, py, pw, ph, bw, theme::SHELL_PICKER_BORDER);

    let text_metrics = Metrics::new(13.0 * sf, 18.0 * sf);

    for (i, font_name) in state.font_options.iter().enumerate() {
        let iy = py + pad + i * item_h;
        let is_active = font_name == &state.font_family;
        let is_hovered = state.font_picker_hovered == Some(i);

        if is_hovered {
            buf.fill_rect(px + bw, iy, pw - bw * 2, item_h, theme::SHELL_PICKER_HOVER);
        }

        let text_color = if is_active {
            theme::TAB_INDICATOR
        } else {
            theme::SHELL_PICKER_TEXT
        };

        let text_x = px + pad + (10.0 * sf) as usize;
        let text_y = iy + ((item_h as f32 - 18.0 * sf) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            text_x,
            text_y,
            buf.height,
            font_name,
            text_metrics,
            text_color,
            Family::Monospace,
        );

        if is_active {
            let check_x = px + pw - pad - (18.0 * sf) as usize;
            let check_y = iy + ((item_h as f32 - 18.0 * sf) / 2.0) as usize;
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                check_x,
                check_y,
                buf.height,
                "\u{2713}",
                text_metrics,
                theme::TAB_INDICATOR,
                Family::Monospace,
            );
        }
    }
}

pub fn font_picker_hit_test(
    phys_x: f64,
    phys_y: f64,
    y_offset: usize,
    content_area_x: usize,
    content_area_w: usize,
    sf: f32,
    font_count: usize,
) -> Option<usize> {
    let (anchor_x, anchor_y) = font_picker_anchor(y_offset, content_area_x, content_area_w, sf);
    let item_h = (32.0 * sf) as f64;
    let pad = (6.0 * sf) as f64;
    let pw = (220.0 * sf) as f64;
    let ph = font_count as f64 * item_h + pad * 2.0;

    let px = anchor_x as f64;
    let py = anchor_y as f64 + (4.0 * sf) as f64;

    if phys_x < px || phys_x >= px + pw || phys_y < py || phys_y >= py + ph {
        return None;
    }

    let rel_y = phys_y - py - pad;
    if rel_y < 0.0 {
        return None;
    }

    let idx = (rel_y / item_h) as usize;
    if idx < font_count { Some(idx) } else { None }
}

fn font_picker_anchor(
    y_offset: usize,
    content_area_x: usize,
    content_area_w: usize,
    sf: f32,
) -> (usize, usize) {
    let pad = (24.0 * sf) as usize;
    let row_h = (40.0 * sf) as usize;
    let header_y = y_offset + (8.0 * sf) as usize;
    let divider_y = header_y + (26.0 * sf) as usize;
    let base_y = divider_y + (20.0 * sf) as usize;
    let input_row_y = base_y + (32.0 * sf) as usize;
    let text_base =
        input_row_y + row_h + (24.0 * sf) as usize + (24.0 * sf) as usize + (32.0 * sf) as usize;
    let input_w = (180.0 * sf) as usize;
    let input_h = (30.0 * sf) as usize;
    let input_x = content_area_x + content_area_w - pad - input_w;
    let anchor_y = text_base + (row_h - input_h) / 2 + input_h;
    (input_x, anchor_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_picker_anchor_reasonable_position() {
        let (ax, ay) = font_picker_anchor(42, 200, 800, 1.0);
        assert!(ax > 0);
        assert!(ay > 42);
    }

    #[test]
    fn font_picker_hit_test_outside_returns_none() {
        assert!(font_picker_hit_test(0.0, 0.0, 42, 200, 800, 1.0, 5).is_none());
    }
}
