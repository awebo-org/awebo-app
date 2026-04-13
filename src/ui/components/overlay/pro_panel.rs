use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{draw_text_at, draw_text_at_bold, measure_text_width};
use crate::license::LicenseManager;
use crate::renderer::theme;

use super::{draw_border, fill_rounded_rect};

const PANEL_W: f32 = 440.0;
const PANEL_PAD: f32 = 24.0;
const TITLE_H: f32 = 36.0;
const FEATURE_ROW_H: f32 = 22.0;
const SECTION_GAP: f32 = 16.0;
const INPUT_H: f32 = 30.0;
const BTN_H: f32 = 30.0;
const BTN_W: f32 = 160.0;

const BG: Rgb = (28, 28, 32);
const BORDER: Rgb = (55, 55, 62);
const TITLE_COLOR: Rgb = (230, 230, 240);
const TEXT_COLOR: Rgb = (190, 190, 200);
const TEXT_DIM: Rgb = (120, 122, 130);
const INPUT_BG: Rgb = (40, 40, 46);
const INPUT_BORDER: Rgb = (65, 65, 72);
const BTN_PRIMARY_BG: Rgb = theme::PRIMARY;
const BTN_PRIMARY_HOVER: Rgb = (239, 59, 139);
const BTN_CLOSE_BG: Rgb = (50, 50, 56);
const BTN_CLOSE_HOVER: Rgb = (70, 70, 78);
const BTN_DEACTIVATE_BG: Rgb = (150, 60, 50);
const BTN_DEACTIVATE_HOVER: Rgb = (170, 75, 65);
const PRO_BADGE: Rgb = (255, 200, 60);
const CURSOR_COLOR: Rgb = theme::PRIMARY;

const FEATURES: &[&str] = &[
    "∞  Unlimited /ask queries",
    "∞  Unlimited /agent sessions",
    "∞  Unlimited sandbox environments",
    "∞  Unlimited git operations",
    "∞  Unlimited editor tabs",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProPanelHit {
    Close,
    BuyPro,
    ActivateKey,
    FocusInput,
    Deactivate,
    Backdrop,
}

pub fn draw_pro_panel(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    license_mgr: &LicenseManager,
    license_input: &str,
    cursor_pos: usize,
    input_focused: bool,
    hovered: Option<usize>,
    sf: f32,
) {
    let w = (PANEL_W * sf) as usize;
    let pad = (PANEL_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let feature_row_h = (FEATURE_ROW_H * sf) as usize;
    let section_gap = (SECTION_GAP * sf) as usize;
    let input_h = (INPUT_H * sf) as usize;
    let btn_h = (BTN_H * sf) as usize;

    let is_pro = license_mgr.is_pro();

    let content_h = if is_pro {
        pad + title_h + section_gap + (feature_row_h * 3) + section_gap + btn_h + section_gap + btn_h + pad
    } else {
        pad + title_h + (feature_row_h * FEATURES.len()) + section_gap * 2 + input_h + section_gap + btn_h + section_gap + btn_h + section_gap + btn_h + pad
    };

    let x = (buf.width.saturating_sub(w)) / 2;
    let y = (buf.height.saturating_sub(content_h)) / 2;

    buf.dim(0.5);

    fill_rounded_rect(buf, x, y, w, content_h, (8.0 * sf) as usize, BG);
    draw_border(buf, x, y, w, content_h, (1.0 * sf).max(1.0) as usize, BORDER);

    let title_metrics = Metrics::new(16.0 * sf, 22.0 * sf);
    let text_metrics = Metrics::new(12.0 * sf, 17.0 * sf);
    let input_metrics = Metrics::new(13.0 * sf, 18.0 * sf);

    let mut cy = y + pad;

    if is_pro {
        draw_text_at_bold(
            buf, font_system, swash_cache,
            x + pad, cy, buf.height,
            "Awebo Pro ✓", title_metrics, PRO_BADGE, Family::SansSerif,
        );
        cy += title_h;

        if let crate::license::LicenseStatus::Pro(data) = license_mgr.status() {
            let info_lines = [
                format!("Email: {}", data.email),
                format!("Activated: {}", data.activated_at),
                format!("Devices: {}/{}", 1, data.max_devices),
            ];
            for line in &info_lines {
                draw_text_at(
                    buf, font_system, swash_cache,
                    x + pad, cy, buf.height,
                    line, text_metrics, TEXT_COLOR, Family::SansSerif,
                );
                cy += feature_row_h;
            }
        }

        cy += section_gap;

        let deact_w = (BTN_W * sf) as usize;
        let deact_x = x + (w - deact_w) / 2;
        let bg = if hovered == Some(2) { BTN_DEACTIVATE_HOVER } else { BTN_DEACTIVATE_BG };
        fill_rounded_rect(buf, deact_x, cy, deact_w, btn_h, (4.0 * sf) as usize, bg);
        let lbl = "Deactivate License";
        let lbl_w = crate::renderer::text::measure_text_width_bold(font_system, lbl, text_metrics, Family::SansSerif).ceil() as usize;
        draw_text_at_bold(
            buf, font_system, swash_cache,
            deact_x + (deact_w - lbl_w) / 2,
            cy + ((btn_h as f32 - text_metrics.line_height) / 2.0) as usize,
            buf.height, lbl, text_metrics, (255, 255, 255), Family::SansSerif,
        );
        cy += btn_h + section_gap;
    } else {
        draw_text_at_bold(
            buf, font_system, swash_cache,
            x + pad, cy, buf.height,
            "Upgrade to Awebo Pro", title_metrics, TITLE_COLOR, Family::SansSerif,
        );
        cy += title_h;

        for &feature in FEATURES {
            draw_text_at(
                buf, font_system, swash_cache,
                x + pad, cy, buf.height,
                feature, text_metrics, TEXT_COLOR, Family::SansSerif,
            );
            cy += feature_row_h;
        }

        cy += section_gap;

        let buy_w = (BTN_W * sf) as usize;
        let buy_x = x + (w - buy_w) / 2;
        let bg = if hovered == Some(0) { BTN_PRIMARY_HOVER } else { BTN_PRIMARY_BG };
        fill_rounded_rect(buf, buy_x, cy, buy_w, btn_h, (4.0 * sf) as usize, bg);
        let lbl = "Upgrade to Pro";
        let lbl_w = crate::renderer::text::measure_text_width_bold(font_system, lbl, text_metrics, Family::SansSerif).ceil() as usize;
        draw_text_at_bold(
            buf, font_system, swash_cache,
            buy_x + (buy_w - lbl_w) / 2,
            cy + ((btn_h as f32 - text_metrics.line_height) / 2.0) as usize,
            buf.height, lbl, text_metrics, (255, 255, 255), Family::SansSerif,
        );
        cy += btn_h + section_gap;

        draw_text_at(
            buf, font_system, swash_cache,
            x + pad, cy, buf.height,
            "Already have a license key?", text_metrics, TEXT_DIM, Family::SansSerif,
        );
        cy += (18.0 * sf) as usize;

        let input_w = w - pad * 2;
        let input_border = if input_focused { CURSOR_COLOR } else { INPUT_BORDER };
        fill_rounded_rect(buf, x + pad, cy, input_w, input_h, (4.0 * sf) as usize, INPUT_BG);
        draw_border(buf, x + pad, cy, input_w, input_h, (1.0 * sf).max(1.0) as usize, input_border);

        let text_pad = (8.0 * sf) as usize;
        let text_y = cy + ((input_h as f32 - input_metrics.line_height) / 2.0) as usize;
        if license_input.is_empty() {
            draw_text_at(
                buf, font_system, swash_cache,
                x + pad + text_pad, text_y,
                buf.height, "XXXX-XXXX-XXXX-XXXX", input_metrics, TEXT_DIM, Family::Monospace,
            );
        } else {
            draw_text_at(
                buf, font_system, swash_cache,
                x + pad + text_pad, text_y,
                buf.height, license_input, input_metrics, TEXT_COLOR, Family::Monospace,
            );
        }

        if input_focused {
            let clamped = cursor_pos.min(license_input.len());
            let before_cursor = &license_input[..clamped];
            let cursor_x_offset = measure_text_width(font_system, before_cursor, input_metrics, Family::Monospace).ceil() as usize;
            let cursor_x = x + pad + text_pad + cursor_x_offset;
            let cursor_h = (input_metrics.line_height * 0.75) as usize;
            let cursor_y = cy + (input_h.saturating_sub(cursor_h)) / 2;
            let cursor_w = (1.5 * sf).max(1.0) as usize;
            buf.fill_rect(cursor_x, cursor_y, cursor_w, cursor_h, CURSOR_COLOR);
        }

        cy += input_h + section_gap;

        let act_w = (BTN_W * sf) as usize;
        let act_x = x + (w - act_w) / 2;
        let bg = if hovered == Some(1) { BTN_PRIMARY_HOVER } else { BTN_PRIMARY_BG };
        fill_rounded_rect(buf, act_x, cy, act_w, btn_h, (4.0 * sf) as usize, bg);
        let lbl = "Activate Key";
        let lbl_w = crate::renderer::text::measure_text_width_bold(font_system, lbl, text_metrics, Family::SansSerif).ceil() as usize;
        draw_text_at_bold(
            buf, font_system, swash_cache,
            act_x + (act_w - lbl_w) / 2,
            cy + ((btn_h as f32 - text_metrics.line_height) / 2.0) as usize,
            buf.height, lbl, text_metrics, (255, 255, 255), Family::SansSerif,
        );
        cy += btn_h + section_gap;
    }

    let close_w = (80.0 * sf) as usize;
    let close_x = x + (w - close_w) / 2;
    let close_bg = if hovered == Some(3) { BTN_CLOSE_HOVER } else { BTN_CLOSE_BG };
    fill_rounded_rect(buf, close_x, cy, close_w, btn_h, (4.0 * sf) as usize, close_bg);
    let close_lbl = "Close";
    let close_lbl_w = crate::renderer::text::measure_text_width_bold(font_system, close_lbl, text_metrics, Family::SansSerif).ceil() as usize;
    draw_text_at_bold(
        buf, font_system, swash_cache,
        close_x + (close_w - close_lbl_w) / 2,
        cy + ((btn_h as f32 - text_metrics.line_height) / 2.0) as usize,
        buf.height, close_lbl, text_metrics, TEXT_COLOR, Family::SansSerif,
    );
}

pub fn pro_panel_hit_test(
    buf_w: usize,
    buf_h: usize,
    is_pro: bool,
    mx: f64,
    my: f64,
    sf: f32,
) -> Option<ProPanelHit> {
    let w = (PANEL_W * sf) as usize;
    let pad = (PANEL_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let feature_row_h = (FEATURE_ROW_H * sf) as usize;
    let section_gap = (SECTION_GAP * sf) as usize;
    let input_h = (INPUT_H * sf) as usize;
    let btn_h = (BTN_H * sf) as usize;

    let content_h = if is_pro {
        pad + title_h + section_gap + (feature_row_h * 3) + section_gap + btn_h + section_gap + btn_h + pad
    } else {
        pad + title_h + (feature_row_h * FEATURES.len()) + section_gap * 2 + input_h + section_gap + btn_h + section_gap + btn_h + section_gap + btn_h + pad
    };

    let x = (buf_w.saturating_sub(w)) / 2;
    let y = (buf_h.saturating_sub(content_h)) / 2;

    let mx = mx as usize;
    let my = my as usize;

    if mx < x || mx >= x + w || my < y || my >= y + content_h {
        return Some(ProPanelHit::Backdrop);
    }

    let mut cy = y + pad;

    if is_pro {
        cy += title_h + section_gap + feature_row_h * 3 + section_gap;
        let deact_w = (BTN_W * sf) as usize;
        let deact_x = x + (w - deact_w) / 2;
        if mx >= deact_x && mx < deact_x + deact_w && my >= cy && my < cy + btn_h {
            return Some(ProPanelHit::Deactivate);
        }
        cy += btn_h + section_gap;
    } else {
        cy += title_h + feature_row_h * FEATURES.len() + section_gap;
        let buy_w = (BTN_W * sf) as usize;
        let buy_x = x + (w - buy_w) / 2;
        if mx >= buy_x && mx < buy_x + buy_w && my >= cy && my < cy + btn_h {
            return Some(ProPanelHit::BuyPro);
        }
        cy += btn_h + section_gap + (18.0 * sf) as usize;
        let input_w = w - pad * 2;
        if mx >= x + pad && mx < x + pad + input_w && my >= cy && my < cy + input_h {
            return Some(ProPanelHit::FocusInput);
        }
        cy += input_h + section_gap;
        let act_w = (BTN_W * sf) as usize;
        let act_x = x + (w - act_w) / 2;
        if mx >= act_x && mx < act_x + act_w && my >= cy && my < cy + btn_h {
            return Some(ProPanelHit::ActivateKey);
        }
        cy += btn_h + section_gap;
    }

    let close_w = (80.0 * sf) as usize;
    let close_x = x + (w - close_w) / 2;
    if mx >= close_x && mx < close_x + close_w && my >= cy && my < cy + btn_h {
        return Some(ProPanelHit::Close);
    }

    None
}

pub fn pro_panel_hover_test(
    buf_w: usize,
    buf_h: usize,
    is_pro: bool,
    mx: f64,
    my: f64,
    sf: f32,
) -> Option<usize> {
    let w = (PANEL_W * sf) as usize;
    let pad = (PANEL_PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let feature_row_h = (FEATURE_ROW_H * sf) as usize;
    let section_gap = (SECTION_GAP * sf) as usize;
    let input_h = (INPUT_H * sf) as usize;
    let btn_h = (BTN_H * sf) as usize;

    let content_h = if is_pro {
        pad + title_h + section_gap + (feature_row_h * 3) + section_gap + btn_h + section_gap + btn_h + pad
    } else {
        pad + title_h + (feature_row_h * FEATURES.len()) + section_gap * 2 + input_h + section_gap + btn_h + section_gap + btn_h + section_gap + btn_h + pad
    };

    let x = (buf_w.saturating_sub(w)) / 2;
    let y = (buf_h.saturating_sub(content_h)) / 2;

    let mx = mx as usize;
    let my = my as usize;

    let mut cy = y + pad;

    if is_pro {
        cy += title_h + section_gap + feature_row_h * 3 + section_gap;
        let deact_w = (BTN_W * sf) as usize;
        let deact_x = x + (w - deact_w) / 2;
        if mx >= deact_x && mx < deact_x + deact_w && my >= cy && my < cy + btn_h {
            return Some(2);
        }
        cy += btn_h + section_gap;
    } else {
        cy += title_h + feature_row_h * FEATURES.len() + section_gap;
        let buy_w = (BTN_W * sf) as usize;
        let buy_x = x + (w - buy_w) / 2;
        if mx >= buy_x && mx < buy_x + buy_w && my >= cy && my < cy + btn_h {
            return Some(0);
        }
        cy += btn_h + section_gap + (18.0 * sf) as usize + input_h + section_gap;
        let act_w = (BTN_W * sf) as usize;
        let act_x = x + (w - act_w) / 2;
        if mx >= act_x && mx < act_x + act_w && my >= cy && my < cy + btn_h {
            return Some(1);
        }
        cy += btn_h + section_gap;
    }

    let close_w = (80.0 * sf) as usize;
    let close_x = x + (w - close_w) / 2;
    if mx >= close_x && mx < close_x + close_w && my >= cy && my < cy + btn_h {
        return Some(3);
    }

    None
}
