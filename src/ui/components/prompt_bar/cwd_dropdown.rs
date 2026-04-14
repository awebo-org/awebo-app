//! CWD dropdown — lists subdirectories of the current working directory.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::super::overlay::{draw_border, fill_rounded_rect};

const DROPDOWN_W: f32 = 280.0;
const ITEM_H: f32 = 28.0;
const PAD_Y: f32 = 6.0;
const PAD_X: f32 = 12.0;
const TEXT_SIZE: f32 = 12.0;
const LINE_HEIGHT: f32 = 17.0;
const MAX_VISIBLE: usize = 12;
const CORNER_R: f32 = 6.0;

pub fn list_subdirectories(cwd: &str) -> Vec<String> {
    let path = std::path::Path::new(cwd);
    let mut entries: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(path) {
        for entry in rd.flatten() {
            if let Ok(ft) = entry.file_type()
                && ft.is_dir()
                && let Some(name) = entry.file_name().to_str()
                && name != ".git"
            {
                entries.push(name.to_string());
            }
        }
    }
    entries.sort_unstable_by_key(|a| a.to_lowercase());
    entries
}

pub fn dropdown_rect(
    badge_x: usize,
    badge_y: usize,
    entry_count: usize,
    sf: f32,
) -> (usize, usize, usize, usize) {
    let w = (DROPDOWN_W * sf) as usize;
    let visible = entry_count.min(MAX_VISIBLE);
    let pad = (PAD_Y * sf) as usize;
    let item_h = (ITEM_H * sf) as usize;
    let h = visible * item_h + pad * 2;
    let y = badge_y.saturating_sub(h + (2.0 * sf) as usize);
    (badge_x, y, w, h)
}

pub fn draw_cwd_dropdown(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    entries: &[String],
    badge_x: usize,
    badge_y: usize,
    scroll: usize,
    hovered: Option<usize>,
    sf: f32,
) {
    if entries.is_empty() {
        return;
    }

    let (dx, dy, dw, dh) = dropdown_rect(badge_x, badge_y, entries.len(), sf);
    let r = (CORNER_R * sf) as usize;
    let bw = (1.0_f32 * sf).max(1.0) as usize;

    fill_rounded_rect(buf, dx, dy, dw, dh, r, theme::BG_SURFACE);
    draw_border(buf, dx, dy, dw, dh, bw, theme::BORDER);

    let pad_y = (PAD_Y * sf) as usize;
    let pad_x = (PAD_X * sf) as usize;
    let item_h = (ITEM_H * sf) as usize;
    let metrics = Metrics::new(TEXT_SIZE * sf, LINE_HEIGHT * sf);
    let visible = entries.len().min(MAX_VISIBLE);

    let mut y = dy + pad_y;
    for vi in 0..visible {
        let idx = scroll + vi;
        if idx >= entries.len() {
            break;
        }

        if hovered == Some(idx) {
            buf.fill_rect(dx + bw, y, dw - bw * 2, item_h, theme::BG_SELECTION);
        }

        let text_y = y + ((item_h as f32 - LINE_HEIGHT * sf) / 2.0) as usize;
        let fg = if hovered == Some(idx) {
            theme::FG_BRIGHT
        } else {
            theme::FG_PRIMARY
        };
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            dx + pad_x,
            text_y,
            buf.height,
            &entries[idx],
            metrics,
            fg,
            Family::Monospace,
        );

        y += item_h;
    }
}

pub fn hit_test(
    phys_x: f64,
    phys_y: f64,
    badge_x: usize,
    badge_y: usize,
    entry_count: usize,
    scroll: usize,
    sf: f32,
) -> Option<usize> {
    if entry_count == 0 {
        return None;
    }

    let (dx, dy, dw, dh) = dropdown_rect(badge_x, badge_y, entry_count, sf);

    if phys_x < dx as f64
        || phys_x >= (dx + dw) as f64
        || phys_y < dy as f64
        || phys_y >= (dy + dh) as f64
    {
        return None;
    }

    let pad_y = (PAD_Y * sf) as f64;
    let item_h = (ITEM_H * sf) as f64;
    let rel_y = phys_y - dy as f64 - pad_y;
    if rel_y < 0.0 {
        return None;
    }

    let vi = (rel_y / item_h) as usize;
    let idx = scroll + vi;
    if idx < entry_count { Some(idx) } else { None }
}

pub fn hover_test(
    phys_x: f64,
    phys_y: f64,
    badge_x: usize,
    badge_y: usize,
    entry_count: usize,
    scroll: usize,
    sf: f32,
) -> Option<usize> {
    hit_test(phys_x, phys_y, badge_x, badge_y, entry_count, scroll, sf)
}

pub fn contains(
    phys_x: f64,
    phys_y: f64,
    badge_x: usize,
    badge_y: usize,
    entry_count: usize,
    sf: f32,
) -> bool {
    if entry_count == 0 {
        return false;
    }
    let (dx, dy, dw, dh) = dropdown_rect(badge_x, badge_y, entry_count, sf);
    phys_x >= dx as f64
        && phys_x < (dx + dw) as f64
        && phys_y >= dy as f64
        && phys_y < (dy + dh) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropdown_rect_clamped_to_max_visible() {
        let (_, _, _, h1) = dropdown_rect(0, 500, 5, 1.0);
        let (_, _, _, h2) = dropdown_rect(0, 500, 20, 1.0);
        assert!(h2 > h1);
        let (_, _, _, h3) = dropdown_rect(0, 500, 100, 1.0);
        assert_eq!(h2, h3);
    }

    #[test]
    fn hit_test_outside_returns_none() {
        assert!(hit_test(0.0, 0.0, 100, 500, 5, 0, 1.0).is_none());
    }

    #[test]
    fn contains_empty_returns_false() {
        assert!(!contains(100.0, 400.0, 100, 500, 0, 1.0));
    }

    #[test]
    fn list_subdirectories_returns_sorted() {
        let entries = list_subdirectories(env!("CARGO_MANIFEST_DIR"));
        assert!(entries.contains(&"src".to_string()));
        let is_sorted = entries
            .windows(2)
            .all(|w| w[0].to_lowercase() <= w[1].to_lowercase());
        assert!(is_sorted);
    }
}
