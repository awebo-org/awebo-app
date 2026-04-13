use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{draw_text_at, draw_text_at_bold};
use crate::usage::{Feature, UsageTracker};

use super::{draw_border, fill_rounded_rect};

const PANEL_W: f32 = 420.0;
const PANEL_PAD: f32 = 20.0;
const TITLE_H: f32 = 32.0;
const ROW_H: f32 = 28.0;
const BAR_H: f32 = 6.0;
const FOOTER_H: f32 = 36.0;
const BTN_H: f32 = 28.0;
const BTN_W: f32 = 140.0;

const BG: Rgb = (30, 30, 34);
const BORDER: Rgb = (55, 55, 62);
const TITLE_COLOR: Rgb = (220, 220, 230);
const TEXT_COLOR: Rgb = (180, 180, 190);
const TEXT_DIM: Rgb = (110, 112, 120);
const BAR_BG: Rgb = (45, 45, 52);
const BAR_FILL_OK: Rgb = (60, 140, 80);
const BAR_FILL_WARN: Rgb = (200, 170, 60);
const BAR_FILL_FULL: Rgb = (200, 80, 60);
const BTN_BG: Rgb = (50, 100, 180);
const BTN_HOVER: Rgb = (70, 120, 200);
const CLOSE_BTN_BG: Rgb = (50, 50, 56);
const CLOSE_BTN_HOVER: Rgb = (70, 70, 78);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsagePanelHit {
    Close,
    UpgradePro,
    Backdrop,
}

pub fn draw_usage_panel(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    tracker: &UsageTracker,
    hovered: Option<usize>,
    sf: f32,
) {
    let w = (PANEL_W * sf) as usize;
    let pad = (PANEL_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let row_h = (ROW_H * sf) as usize;
    let bar_h = (BAR_H * sf) as usize;
    let footer_h = (FOOTER_H * sf) as usize;

    let feature_count = Feature::all().len();
    let content_h = title_h + (row_h * feature_count) + pad + footer_h + pad * 2;

    let x = (buf.width.saturating_sub(w)) / 2;
    let y = (buf.height.saturating_sub(content_h)) / 2;

    buf.dim(0.5);

    fill_rounded_rect(buf, x, y, w, content_h, (6.0 * sf) as usize, BG);
    draw_border(buf, x, y, w, content_h, (1.0 * sf).max(1.0) as usize, BORDER);

    let title_metrics = Metrics::new(15.0 * sf, 20.0 * sf);
    let text_metrics = Metrics::new(12.0 * sf, 17.0 * sf);
    let small_metrics = Metrics::new(10.0 * sf, 14.0 * sf);

    let ty = y + pad;
    draw_text_at_bold(
        buf, font_system, swash_cache,
        x + pad, ty, buf.height,
        "Daily Usage", title_metrics, TITLE_COLOR, Family::SansSerif,
    );

    let reset_label = if tracker.is_pro() {
        "Pro — unlimited".to_string()
    } else {
        format!("Resets in {}", crate::usage::format_duration_short(tracker.time_until_reset()))
    };
    let reset_w = crate::renderer::text::measure_text_width(font_system, &reset_label, small_metrics, Family::SansSerif).ceil() as usize;
    draw_text_at(
        buf, font_system, swash_cache,
        x + w - pad - reset_w, ty + (4.0 * sf) as usize, buf.height,
        &reset_label, small_metrics, TEXT_DIM, Family::SansSerif,
    );

    let rows_y = ty + title_h;
    for (i, &feature) in Feature::all().iter().enumerate() {
        let ry = rows_y + i * row_h;
        let used = tracker.count(feature);
        let limit = feature.free_limit();
        let label = feature.label();

        draw_text_at(
            buf, font_system, swash_cache,
            x + pad, ry, buf.height,
            label, text_metrics, TEXT_COLOR, Family::SansSerif,
        );

        let count_str = if tracker.is_pro() {
            format!("{used}/∞")
        } else {
            format!("{used}/{limit}")
        };
        let count_w = crate::renderer::text::measure_text_width(font_system, &count_str, text_metrics, Family::SansSerif).ceil() as usize;
        draw_text_at(
            buf, font_system, swash_cache,
            x + w - pad - count_w, ry, buf.height,
            &count_str, text_metrics, TEXT_DIM, Family::SansSerif,
        );

        let bar_y = ry + row_h - bar_h - (2.0 * sf) as usize;
        let bar_w = w - pad * 2;
        fill_rounded_rect(buf, x + pad, bar_y, bar_w, bar_h, (2.0 * sf) as usize, BAR_BG);

        if !tracker.is_pro() && limit > 0 {
            let ratio = (used as f32 / limit as f32).min(1.0);
            let fill_w = (bar_w as f32 * ratio) as usize;
            let fill_color = if ratio >= 1.0 {
                BAR_FILL_FULL
            } else if ratio >= 0.75 {
                BAR_FILL_WARN
            } else {
                BAR_FILL_OK
            };
            if fill_w > 0 {
                fill_rounded_rect(buf, x + pad, bar_y, fill_w, bar_h, (2.0 * sf) as usize, fill_color);
            }
        }
    }

    let footer_y = rows_y + feature_count * row_h + pad;

    if !tracker.is_pro() {
        let btn_w = (BTN_W * sf) as usize;
        let btn_h = (BTN_H * sf) as usize;
        let btn_x = x + (w - btn_w) / 2;
        let btn_y = footer_y;
        let bg = if hovered == Some(0) { BTN_HOVER } else { BTN_BG };
        fill_rounded_rect(buf, btn_x, btn_y, btn_w, btn_h, (4.0 * sf) as usize, bg);
        let lbl = "Upgrade to Pro";
        let lbl_w = crate::renderer::text::measure_text_width_bold(font_system, lbl, text_metrics, Family::SansSerif).ceil() as usize;
        draw_text_at_bold(
            buf, font_system, swash_cache,
            btn_x + (btn_w - lbl_w) / 2,
            btn_y + ((btn_h as f32 - text_metrics.line_height) / 2.0) as usize,
            buf.height,
            lbl, text_metrics, (255, 255, 255), Family::SansSerif,
        );
    }

    let close_btn_w = (80.0 * sf) as usize;
    let close_btn_h = (BTN_H * sf) as usize;
    let close_x = x + (w - close_btn_w) / 2;
    let close_y = footer_y + (BTN_H * sf) as usize + (8.0 * sf) as usize;
    let close_bg = if hovered == Some(1) { CLOSE_BTN_HOVER } else { CLOSE_BTN_BG };
    fill_rounded_rect(buf, close_x, close_y, close_btn_w, close_btn_h, (4.0 * sf) as usize, close_bg);
    let close_lbl = "Close";
    let close_lbl_w = crate::renderer::text::measure_text_width_bold(font_system, close_lbl, text_metrics, Family::SansSerif).ceil() as usize;
    draw_text_at_bold(
        buf, font_system, swash_cache,
        close_x + (close_btn_w - close_lbl_w) / 2,
        close_y + ((close_btn_h as f32 - text_metrics.line_height) / 2.0) as usize,
        buf.height,
        close_lbl, text_metrics, TEXT_COLOR, Family::SansSerif,
    );
}

pub fn usage_panel_hit_test(
    buf_w: usize,
    buf_h: usize,
    tracker: &UsageTracker,
    mx: f64,
    my: f64,
    sf: f32,
) -> Option<UsagePanelHit> {
    let w = (PANEL_W * sf) as usize;
    let pad = (PANEL_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let row_h = (ROW_H * sf) as usize;
    let feature_count = Feature::all().len();
    let footer_h = (FOOTER_H * sf) as usize;
    let content_h = title_h + (row_h * feature_count) + pad + footer_h + pad * 2;

    let x = (buf_w.saturating_sub(w)) / 2;
    let y = (buf_h.saturating_sub(content_h)) / 2;

    let mx = mx as usize;
    let my = my as usize;

    if mx < x || mx >= x + w || my < y || my >= y + content_h {
        return Some(UsagePanelHit::Backdrop);
    }

    let footer_y = y + pad + title_h + feature_count * row_h + pad;
    let btn_h = (BTN_H * sf) as usize;

    if !tracker.is_pro() {
        let btn_w = (BTN_W * sf) as usize;
        let btn_x = x + (w - btn_w) / 2;
        let btn_y = footer_y;
        if mx >= btn_x && mx < btn_x + btn_w && my >= btn_y && my < btn_y + btn_h {
            return Some(UsagePanelHit::UpgradePro);
        }
    }

    let close_btn_w = (80.0 * sf) as usize;
    let close_x = x + (w - close_btn_w) / 2;
    let close_y = footer_y + btn_h + (8.0 * sf) as usize;
    if mx >= close_x && mx < close_x + close_btn_w && my >= close_y && my < close_y + btn_h {
        return Some(UsagePanelHit::Close);
    }

    None
}

pub fn usage_panel_hover_test(
    buf_w: usize,
    buf_h: usize,
    tracker: &UsageTracker,
    mx: f64,
    my: f64,
    sf: f32,
) -> Option<usize> {
    let w = (PANEL_W * sf) as usize;
    let pad = (PANEL_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let row_h = (ROW_H * sf) as usize;
    let feature_count = Feature::all().len();
    let footer_h = (FOOTER_H * sf) as usize;
    let content_h = title_h + (row_h * feature_count) + pad + footer_h + pad * 2;

    let x = (buf_w.saturating_sub(w)) / 2;
    let y = (buf_h.saturating_sub(content_h)) / 2;

    let mx = mx as usize;
    let my = my as usize;

    let footer_y = y + pad + title_h + feature_count * row_h + pad;
    let btn_h = (BTN_H * sf) as usize;

    if !tracker.is_pro() {
        let btn_w = (BTN_W * sf) as usize;
        let btn_x = x + (w - btn_w) / 2;
        let btn_y = footer_y;
        if mx >= btn_x && mx < btn_x + btn_w && my >= btn_y && my < btn_y + btn_h {
            return Some(0);
        }
    }

    let close_btn_w = (80.0 * sf) as usize;
    let close_x = x + (w - close_btn_w) / 2;
    let close_y = footer_y + btn_h + (8.0 * sf) as usize;
    if mx >= close_x && mx < close_x + close_btn_w && my >= close_y && my < close_y + btn_h {
        return Some(1);
    }

    None
}
