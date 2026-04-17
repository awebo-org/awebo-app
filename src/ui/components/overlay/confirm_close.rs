//! Confirm-close dialog overlay — shown when closing an unsaved editor tab.
//!
//! Rendering + hit-testing for the "unsaved changes" confirmation dialog.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{draw_text_at, draw_text_at_bold, measure_text_width};
use crate::renderer::theme;

use super::{draw_border_rounded, fill_rounded_rect};

const DIALOG_W: f32 = 440.0;
const DIALOG_PAD: f32 = 24.0;
const TITLE_SIZE: f32 = 14.0;
const TITLE_H: f32 = 20.0;
const MSG_SIZE: f32 = 12.0;
const MSG_LINE_H: f32 = 17.0;
const MSG_GAP: f32 = 8.0;
const BTN_GAP: f32 = 8.0;
const BTN_H: f32 = 30.0;
const BTN_W: f32 = 100.0;
const BTN_W_WIDE: f32 = 110.0;
const BTN_CORNER: f32 = 6.0;
const CORNER_R: f32 = 10.0;
const BORDER_W: f32 = 1.0;
const SEPARATOR_GAP: f32 = 16.0;

const BG: Rgb = theme::BG_SURFACE;
const BORDER: Rgb = (55, 55, 62);
const BTN_BG: Rgb = (50, 50, 56);
const BTN_HOVER: Rgb = (70, 70, 78);
const BTN_SAVE_BG: Rgb = theme::PRIMARY;
const BTN_SAVE_HOVER: Rgb = theme::PRIMARY_HOVER;
const TITLE_COLOR: Rgb = (230, 230, 240);
const MSG_COLOR: Rgb = (140, 142, 150);
const BTN_TEXT: Rgb = (200, 200, 210);
const BTN_SAVE_TEXT: Rgb = (255, 255, 255);
const SEPARATOR: Rgb = (40, 40, 46);

const BTN_WIDTHS: [f32; 3] = [BTN_W_WIDE, BTN_W, BTN_W];

/// Hit-test result for the confirm-close dialog buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmCloseHit {
    Save,
    DontSave,
    Cancel,
    Backdrop,
}

fn dialog_geometry(buf_w: usize, buf_h: usize, sf: f32) -> (usize, usize, usize, usize) {
    let pad = (DIALOG_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let msg_gap = (MSG_GAP * sf) as usize;
    let msg_line_h = (MSG_LINE_H * sf) as usize;
    let sep_gap = (SEPARATOR_GAP * sf) as usize;
    let btn_h = (BTN_H * sf) as usize;

    let dw = (DIALOG_W * sf) as usize;
    let dh = pad + title_h + msg_gap + msg_line_h + sep_gap + btn_h + pad;
    let dx = buf_w.saturating_sub(dw) / 2;
    let dy = buf_h.saturating_sub(dh) / 3;

    (dx, dy, dw, dh)
}

fn button_rects(dx: usize, dy: usize, dw: usize, sf: f32) -> [(usize, usize, usize, usize); 3] {
    let pad = (DIALOG_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let msg_gap = (MSG_GAP * sf) as usize;
    let msg_line_h = (MSG_LINE_H * sf) as usize;
    let sep_gap = (SEPARATOR_GAP * sf) as usize;
    let btn_h = (BTN_H * sf) as usize;
    let btn_gap = (BTN_GAP * sf) as usize;

    let widths: [usize; 3] = [
        (BTN_WIDTHS[0] * sf) as usize,
        (BTN_WIDTHS[1] * sf) as usize,
        (BTN_WIDTHS[2] * sf) as usize,
    ];
    let total_w: usize = widths.iter().sum::<usize>() + btn_gap * 2;
    let start_x = dx + dw.saturating_sub(pad + total_w);
    let btns_y = dy + pad + title_h + msg_gap + msg_line_h + sep_gap;

    let mut rects = [(0usize, 0usize, 0usize, 0usize); 3];
    let mut cx = start_x;
    for (i, w) in widths.iter().enumerate() {
        rects[i] = (cx, btns_y, *w, btn_h);
        cx += w + btn_gap;
    }
    rects
}

/// Draw the confirm-close overlay dialog.
///
/// `file_name` — the name shown in the dialog title.
/// `hovered` — which button (0=Don't Save, 1=Cancel, 2=Save) is hovered, if any.
pub fn draw_confirm_close(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    file_name: &str,
    hovered: Option<usize>,
    sf: f32,
) {
    buf.dim(0.45);

    let (dx, dy, dw, dh) = dialog_geometry(buf.width, buf.height, sf);
    let corner = (CORNER_R * sf) as usize;
    let bw = (BORDER_W * sf).max(1.0) as usize;
    let btn_corner = (BTN_CORNER * sf) as usize;

    fill_rounded_rect(buf, dx, dy, dw, dh, corner, BG);
    draw_border_rounded(buf, dx, dy, dw, dh, bw, corner, BORDER);

    let pad = (DIALOG_PAD * sf) as usize;
    let title_size = TITLE_SIZE * sf;
    let msg_size = MSG_SIZE * sf;
    let title_h = (TITLE_H * sf) as usize;
    let msg_gap = (MSG_GAP * sf) as usize;
    let msg_line_h = (MSG_LINE_H * sf) as usize;
    let sep_gap = (SEPARATOR_GAP * sf) as usize;

    let title_y = dy + pad;
    draw_text_at_bold(
        buf,
        font_system,
        swash_cache,
        dx + pad,
        title_y,
        title_y + title_h,
        "Unsaved Changes",
        Metrics::new(title_size, title_size * 1.3),
        TITLE_COLOR,
        Family::SansSerif,
    );

    let msg = format!(
        "\u{201c}{}\u{201d} has unsaved changes. Save before closing?",
        file_name,
    );
    let msg_y = title_y + title_h + msg_gap;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        dx + pad,
        msg_y,
        msg_y + msg_line_h,
        &msg,
        Metrics::new(msg_size, msg_size * 1.3),
        MSG_COLOR,
        Family::SansSerif,
    );

    let sep_y = msg_y + msg_line_h + sep_gap / 2;
    buf.fill_rect(dx + pad, sep_y, dw.saturating_sub(pad * 2), 1, SEPARATOR);

    let rects = button_rects(dx, dy, dw, sf);
    let labels = ["Don't Save", "Cancel", "Save"];

    for (i, label) in labels.iter().enumerate() {
        let (bx, by, bw_btn, bh) = rects[i];
        let is_hovered = hovered == Some(i);

        let (bg, fg) = if i == 2 {
            if is_hovered {
                (BTN_SAVE_HOVER, BTN_SAVE_TEXT)
            } else {
                (BTN_SAVE_BG, BTN_SAVE_TEXT)
            }
        } else if is_hovered {
            (BTN_HOVER, BTN_TEXT)
        } else {
            (BTN_BG, BTN_TEXT)
        };

        fill_rounded_rect(buf, bx, by, bw_btn, bh, btn_corner, bg);

        let metrics = Metrics::new(msg_size, msg_size * 1.3);
        let tw = measure_text_width(font_system, label, metrics, Family::SansSerif) as usize;
        let label_x = bx + (bw_btn.saturating_sub(tw)) / 2;
        let label_y = by + (bh.saturating_sub((msg_size * 1.3) as usize)) / 2;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            label_x,
            label_y,
            by + bh,
            label,
            metrics,
            fg,
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
    let (dx, dy, dw, dh) = dialog_geometry(buf_w, buf_h, sf);

    if mx < dx || mx >= dx + dw || my < dy || my >= dy + dh {
        return ConfirmCloseHit::Backdrop;
    }

    let rects = button_rects(dx, dy, dw, sf);
    for (i, &(bx, by, bw_btn, bh)) in rects.iter().enumerate() {
        if mx >= bx && mx < bx + bw_btn && my >= by && my < by + bh {
            return match i {
                0 => ConfirmCloseHit::DontSave,
                1 => ConfirmCloseHit::Cancel,
                2 => ConfirmCloseHit::Save,
                _ => ConfirmCloseHit::Backdrop,
            };
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
    let (dx, dy, dw, _dh) = dialog_geometry(buf_w, buf_h, sf);
    let rects = button_rects(dx, dy, dw, sf);

    for (i, &(bx, by, bw_btn, bh)) in rects.iter().enumerate() {
        if mx >= bx && mx < bx + bw_btn && my >= by && my < by + bh {
            return Some(i);
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

    #[test]
    fn dialog_centered_horizontally() {
        let (dx, _, dw, _) = dialog_geometry(1920, 1080, 1.0);
        assert_eq!(dx, (1920 - dw) / 2);
    }

    #[test]
    fn buttons_inside_dialog() {
        let (dx, dy, dw, dh) = dialog_geometry(1920, 1080, 2.0);
        let rects = button_rects(dx, dy, dw, 2.0);
        for (bx, by, bw_btn, bh) in &rects {
            assert!(*bx >= dx);
            assert!(*bx + *bw_btn <= dx + dw);
            assert!(*by >= dy);
            assert!(*by + *bh <= dy + dh);
        }
    }
}
