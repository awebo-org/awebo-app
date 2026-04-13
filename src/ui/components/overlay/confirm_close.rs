//! Confirm-close dialog overlay — shown when closing an unsaved editor tab.
//!
//! Rendering + hit-testing for the "unsaved changes" confirmation dialog.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{draw_text_at, draw_text_at_bold};
use crate::renderer::theme;

use super::draw_border;

const DIALOG_W: f32 = 400.0;
const DIALOG_PAD: f32 = 20.0;
const TITLE_H: f32 = 28.0;
const MSG_H: f32 = 20.0;
const BTN_ROW_H: f32 = 36.0;
const BTN_W: f32 = 100.0;
const BTN_GAP: f32 = 10.0;
const BTN_H: f32 = 28.0;
const BORDER_W: f32 = 1.0;

const BG: Rgb = (30, 30, 34);
const BTN_BG: Rgb = (50, 50, 56);
const BTN_HOVER: Rgb = (70, 70, 78);
const BTN_SAVE_BG: Rgb = (40, 80, 160);
const BTN_SAVE_HOVER: Rgb = (55, 100, 190);
const TEXT_COLOR: Rgb = (200, 200, 210);
const TEXT_DIM: Rgb = (140, 142, 150);

/// Hit-test result for the confirm-close dialog buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmCloseHit {
    Save,
    DontSave,
    Cancel,
    Backdrop,
}

/// Draw the confirm-close overlay dialog.
///
/// `file_name` — the name shown in the dialog title.
/// `hovered` — which button (0=Save, 1=Don't Save, 2=Cancel) is hovered, if any.
pub fn draw_confirm_close(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    file_name: &str,
    hovered: Option<usize>,
    sf: f32,
) {
    let w = buf.width;
    let h = buf.height;

    buf.dim(0.45);

    let dw = (DIALOG_W * sf) as usize;
    let pad = (DIALOG_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let msg_h = (MSG_H * sf) as usize;
    let btn_row_h = (BTN_ROW_H * sf) as usize;
    let bw = (BORDER_W * sf).max(1.0) as usize;

    let dh = pad + title_h + pad / 2 + msg_h + pad + btn_row_h + pad;
    let dx = w.saturating_sub(dw) / 2;
    let dy = h.saturating_sub(dh) / 3;

    buf.fill_rect(dx, dy, dw, dh, BG);
    draw_border(buf, dx, dy, dw, dh, bw, theme::PALETTE_BORDER);

    let font_size = 14.0 * sf;
    let small_size = 13.0 * sf;

    let title_clip_y = dy + pad + title_h;
    draw_text_at_bold(
        buf,
        font_system,
        swash_cache,
        dx + pad,
        dy + pad,
        title_clip_y,
        "Unsaved Changes",
        Metrics::new(font_size, font_size * 1.3),
        TEXT_COLOR,
        Family::SansSerif,
    );

    let msg = format!(
        "\"{}\" has unsaved changes. Save before closing?",
        file_name,
    );
    let msg_y = dy + pad + title_h + pad / 2;
    let msg_clip_y = msg_y + msg_h;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        dx + pad,
        msg_y,
        msg_clip_y,
        &msg,
        Metrics::new(small_size, small_size * 1.3),
        TEXT_DIM,
        Family::SansSerif,
    );

    let btn_w = (BTN_W * sf) as usize;
    let btn_h = (BTN_H * sf) as usize;
    let btn_gap = (BTN_GAP * sf) as usize;

    let btns_total_w = btn_w * 3 + btn_gap * 2;
    let btns_x = dx + dw.saturating_sub(pad + btns_total_w);
    let btns_y = dy + dh - pad - btn_row_h + (btn_row_h.saturating_sub(btn_h)) / 2;

    let labels = ["Don't Save", "Cancel", "Save"];
    for (i, label) in labels.iter().enumerate() {
        let bx = btns_x + i * (btn_w + btn_gap);
        let is_hovered = hovered == Some(i);

        let bg = if i == 2 {
            if is_hovered {
                BTN_SAVE_HOVER
            } else {
                BTN_SAVE_BG
            }
        } else if is_hovered {
            BTN_HOVER
        } else {
            BTN_BG
        };

        buf.fill_rect(bx, btns_y, btn_w, btn_h, bg);

        let label_px_w = label.len() as f32 * small_size * 0.48;
        let label_x = bx + (btn_w as f32 / 2.0 - label_px_w / 2.0).max(0.0) as usize;
        let btn_clip_y = btns_y + btn_h;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            label_x,
            btns_y,
            btn_clip_y,
            label,
            Metrics::new(small_size, small_size * 1.3),
            TEXT_COLOR,
            Family::SansSerif,
        );
    }
}

/// Hit-test a click against the confirm-close dialog.
///
/// Returns which button was hit, or `Backdrop` if clicked outside the dialog.
pub fn confirm_close_hit_test(
    mx: usize,
    my: usize,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> ConfirmCloseHit {
    let dw = (DIALOG_W * sf) as usize;
    let pad = (DIALOG_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let msg_h = (MSG_H * sf) as usize;
    let btn_row_h = (BTN_ROW_H * sf) as usize;

    let dh = pad + title_h + pad / 2 + msg_h + pad + btn_row_h + pad;
    let dx = buf_w.saturating_sub(dw) / 2;
    let dy = buf_h.saturating_sub(dh) / 3;

    if mx < dx || mx >= dx + dw || my < dy || my >= dy + dh {
        return ConfirmCloseHit::Backdrop;
    }

    let btn_w = (BTN_W * sf) as usize;
    let btn_h = (BTN_H * sf) as usize;
    let btn_gap = (BTN_GAP * sf) as usize;
    let btns_total_w = btn_w * 3 + btn_gap * 2;
    let btns_x = dx + dw.saturating_sub(pad + btns_total_w);
    let btns_y = dy + dh - pad - btn_row_h + (btn_row_h.saturating_sub(btn_h)) / 2;

    if my >= btns_y && my < btns_y + btn_h {
        for i in 0..3 {
            let bx = btns_x + i * (btn_w + btn_gap);
            if mx >= bx && mx < bx + btn_w {
                return match i {
                    0 => ConfirmCloseHit::DontSave,
                    1 => ConfirmCloseHit::Cancel,
                    2 => ConfirmCloseHit::Save,
                    _ => ConfirmCloseHit::Backdrop,
                };
            }
        }
    }

    ConfirmCloseHit::Backdrop
}

/// Hover-test: returns which button index (0..3) the mouse is over, or None.
pub fn confirm_close_hover_test(
    mx: usize,
    my: usize,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> Option<usize> {
    let dw = (DIALOG_W * sf) as usize;
    let pad = (DIALOG_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let msg_h = (MSG_H * sf) as usize;
    let btn_row_h = (BTN_ROW_H * sf) as usize;

    let dh = pad + title_h + pad / 2 + msg_h + pad + btn_row_h + pad;
    let dx = buf_w.saturating_sub(dw) / 2;
    let dy = buf_h.saturating_sub(dh) / 3;

    let btn_w = (BTN_W * sf) as usize;
    let btn_h = (BTN_H * sf) as usize;
    let btn_gap = (BTN_GAP * sf) as usize;
    let btns_total_w = btn_w * 3 + btn_gap * 2;
    let btns_x = dx + dw.saturating_sub(pad + btns_total_w);
    let btns_y = dy + dh - pad - btn_row_h + (btn_row_h.saturating_sub(btn_h)) / 2;

    if my >= btns_y && my < btns_y + btn_h {
        for i in 0..3 {
            let bx = btns_x + i * (btn_w + btn_gap);
            if mx >= bx && mx < bx + btn_w {
                return Some(i);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_backdrop_outside() {
        let hit = confirm_close_hit_test(0, 0, 1920, 1080, 1.0);
        assert_eq!(hit, ConfirmCloseHit::Backdrop);
    }

    #[test]
    fn hover_outside_returns_none() {
        assert!(confirm_close_hover_test(0, 0, 1920, 1080, 1.0).is_none());
    }
}
