use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::{Icon, IconRenderer};
use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{draw_text_at, draw_text_at_bold, measure_text_width};
use crate::renderer::theme;
use crate::usage::{Feature, UsageTracker};

use super::overlay::{draw_border_rounded, fill_rounded_rect};

const PANEL_W: f32 = 380.0;
const PAD: f32 = 20.0;
const TITLE_H: f32 = 28.0;
const TITLE_GAP: f32 = 10.0;
const ROW_H: f32 = 22.0;
const BAR_H: f32 = 6.0;
const BAR_W: f32 = 130.0;
const LABEL_W: f32 = 70.0;
const COUNT_GAP: f32 = 10.0;
const UPGRADE_GAP: f32 = 12.0;
const BTN_H: f32 = 30.0;
const CORNER_R: f32 = 10.0;

const PANEL_BG: Rgb = (22, 22, 26);
const PANEL_BORDER: Rgb = (55, 55, 62);
const TITLE_COLOR: Rgb = theme::FG_BRIGHT;
const RESET_COLOR: Rgb = theme::FG_DIM;
const LABEL_COLOR: Rgb = theme::FG_SECONDARY;
const COUNT_COLOR: Rgb = theme::FG_DIM;
const BAR_TRACK: Rgb = (38, 38, 44);
const BAR_NORMAL: Rgb = (90, 182, 90);
const BAR_WARNING: Rgb = (210, 160, 60);
const BAR_EXHAUSTED: Rgb = theme::PRIMARY;
const UPGRADE_BG: Rgb = theme::PRIMARY;
const UPGRADE_HOVER_BG: Rgb = theme::PRIMARY_HOVER;
const UPGRADE_TEXT: Rgb = (255, 255, 255);
const CLOSE_BG: Rgb = (50, 50, 56);
const CLOSE_HOVER_BG: Rgb = (70, 70, 78);
const CLOSE_TEXT: Rgb = theme::FG_SECONDARY;

const TRACKED: &[Feature] = &[Feature::Ask, Feature::Agent, Feature::Sandbox, Feature::Git];

#[derive(Default)]
pub struct UsageLimitBannerState {
    pub visible: bool,
    pub hovered: Option<usize>,
}

impl UsageLimitBannerState {
    pub fn show(&mut self) {
        self.visible = true;
        self.hovered = None;
    }

    pub fn dismiss(&mut self) {
        self.visible = false;
        self.hovered = None;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageLimitBannerHit {
    Dismiss,
    Upgrade,
    Backdrop,
}

fn panel_height(sf: f32) -> usize {
    let pad = (PAD * sf) as usize;
    let title_h = (TITLE_H * sf) as usize;
    let tg = (TITLE_GAP * sf) as usize;
    let row_h = (ROW_H * sf) as usize;
    let rows = TRACKED.len();
    let features_h = rows * row_h;
    let ug = (UPGRADE_GAP * sf) as usize;
    let btn_h = (BTN_H * sf) as usize;
    pad + title_h + tg + features_h + ug + btn_h + pad
}

fn bar_color(used: u32, limit: u32) -> Rgb {
    if used >= limit {
        BAR_EXHAUSTED
    } else if limit > 0 && (limit - used) <= (limit / 4).max(1) {
        BAR_WARNING
    } else {
        BAR_NORMAL
    }
}

fn panel_rect(buf_w: usize, buf_h: usize, sf: f32) -> (usize, usize, usize, usize) {
    let pw = (PANEL_W * sf) as usize;
    let ph = panel_height(sf);
    let px = buf_w.saturating_sub(pw) / 2;
    let py = buf_h.saturating_sub(ph) / 2;
    (px, py, pw, ph)
}

pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &UsageLimitBannerState,
    tracker: &UsageTracker,
    sf: f32,
) {
    if !state.visible {
        return;
    }

    let bw = buf.width;
    let bh = buf.height;
    let (px, py, pw, ph) = panel_rect(bw, bh, sf);

    buf.dim(0.45);

    let corner = (CORNER_R * sf) as usize;
    fill_rounded_rect(buf, px, py, pw, ph, corner, PANEL_BG);
    let bw_px = (1.0_f32 * sf).max(1.0) as usize;
    draw_border_rounded(buf, px, py, pw, ph, bw_px, corner, PANEL_BORDER);

    let pad = (PAD * sf) as usize;
    let content_x = px + pad;

    let title_fs = 14.0 * sf;
    let title_lh = TITLE_H * sf;
    let title_m = Metrics::new(title_fs, title_lh);

    let title_y = py + pad;
    let icon_sz = (18.0 * sf) as u32;
    let icon_y = title_y + ((title_lh - icon_sz as f32) / 2.0).max(0.0) as usize;
    icon_renderer.draw(
        buf,
        Icon::Sparkle,
        content_x,
        icon_y,
        icon_sz,
        BAR_EXHAUSTED,
    );

    let title_text_x = content_x + icon_sz as usize + (8.0 * sf) as usize;
    draw_text_at_bold(
        buf,
        font_system,
        swash_cache,
        title_text_x,
        title_y,
        buf.height,
        "Limit reached",
        title_m,
        TITLE_COLOR,
        Family::SansSerif,
    );

    let reset_fs = 11.0 * sf;
    let reset_lh = 16.0 * sf;
    let reset_m = Metrics::new(reset_fs, reset_lh);
    let reset_text = format!(
        "Resets in {}",
        crate::usage::format_duration_short(tracker.time_until_reset())
    );
    let reset_tw =
        measure_text_width(font_system, &reset_text, reset_m, Family::SansSerif) as usize;
    let reset_x = (px + pw).saturating_sub(pad + reset_tw);
    let reset_y = title_y + ((title_lh - reset_lh) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        reset_x,
        reset_y,
        buf.height,
        &reset_text,
        reset_m,
        RESET_COLOR,
        Family::SansSerif,
    );

    let label_fs = 12.0 * sf;
    let label_lh = ROW_H * sf;
    let label_m = Metrics::new(label_fs, label_lh);
    let count_fs = 11.0 * sf;
    let count_lh = label_lh;
    let count_m = Metrics::new(count_fs, count_lh);

    let tg = (TITLE_GAP * sf) as usize;
    let features_y = title_y + title_lh as usize + tg;
    let row_h = (ROW_H * sf) as usize;
    let bar_h = (BAR_H * sf).max(2.0) as usize;
    let bar_w = (BAR_W * sf) as usize;
    let bar_r = (bar_h as f32 / 2.0) as usize;
    let label_w = (LABEL_W * sf) as usize;
    let count_gap = (COUNT_GAP * sf) as usize;

    for (i, &feature) in TRACKED.iter().enumerate() {
        let fy = features_y + i * row_h;

        draw_text_at(
            buf,
            font_system,
            swash_cache,
            content_x,
            fy,
            buf.height,
            feature.label(),
            label_m,
            LABEL_COLOR,
            Family::SansSerif,
        );

        let bar_x = content_x + label_w;
        let bar_y = fy + ((row_h as f32 - bar_h as f32) / 2.0).max(0.0) as usize;
        fill_rounded_rect(buf, bar_x, bar_y, bar_w, bar_h, bar_r, BAR_TRACK);

        let used = tracker.count(feature);
        let limit = feature.free_limit();
        if used > 0 && limit > 0 {
            let frac = (used as f32 / limit as f32).min(1.0);
            let fill_w = ((bar_w as f32 * frac) as usize).max(bar_h);
            let color = bar_color(used, limit);
            fill_rounded_rect(buf, bar_x, bar_y, fill_w, bar_h, bar_r, color);
        }

        let count_text = format!("{}/{}", used, limit);
        let count_x = bar_x + bar_w + count_gap;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            count_x,
            fy,
            buf.height,
            &count_text,
            count_m,
            COUNT_COLOR,
            Family::Monospace,
        );
    }

    let ug = (UPGRADE_GAP * sf) as usize;
    let btn_y = features_y + TRACKED.len() * row_h + ug;
    let btn_h = (BTN_H * sf) as usize;
    let btn_r = (5.0 * sf) as usize;
    let btn_gap = (10.0 * sf) as usize;

    let upgrade_text = "Upgrade to Pro";
    let upgrade_tw =
        measure_text_width(font_system, upgrade_text, label_m, Family::SansSerif) as usize;
    let upgrade_pad = (16.0 * sf) as usize;
    let upgrade_w = upgrade_tw + upgrade_pad * 2;
    let upgrade_bg = if state.hovered == Some(0) {
        UPGRADE_HOVER_BG
    } else {
        UPGRADE_BG
    };
    fill_rounded_rect(buf, content_x, btn_y, upgrade_w, btn_h, btn_r, upgrade_bg);
    let upgrade_text_y = btn_y + ((btn_h as f32 - label_lh) / 2.0) as usize;
    draw_text_at_bold(
        buf,
        font_system,
        swash_cache,
        content_x + upgrade_pad,
        upgrade_text_y,
        buf.height,
        upgrade_text,
        label_m,
        UPGRADE_TEXT,
        Family::SansSerif,
    );

    let close_text = "Close";
    let close_tw = measure_text_width(font_system, close_text, label_m, Family::SansSerif) as usize;
    let close_pad = (16.0 * sf) as usize;
    let close_w = close_tw + close_pad * 2;
    let close_x = content_x + upgrade_w + btn_gap;
    let close_bg = if state.hovered == Some(1) {
        CLOSE_HOVER_BG
    } else {
        CLOSE_BG
    };
    fill_rounded_rect(buf, close_x, btn_y, close_w, btn_h, btn_r, close_bg);
    let close_text_y = btn_y + ((btn_h as f32 - label_lh) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        close_x + close_pad,
        close_text_y,
        buf.height,
        close_text,
        label_m,
        CLOSE_TEXT,
        Family::SansSerif,
    );
}

pub fn hit_test(
    state: &UsageLimitBannerState,
    mx: f64,
    my: f64,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> Option<UsageLimitBannerHit> {
    if !state.visible {
        return None;
    }
    let (px, py, pw, ph) = panel_rect(buf_w, buf_h, sf);
    let cx = mx as usize;
    let cy = my as usize;

    if cx < px || cx >= px + pw || cy < py || cy >= py + ph {
        return Some(UsageLimitBannerHit::Backdrop);
    }

    let pad = (PAD * sf) as usize;
    let content_x = px + pad;
    let title_lh = (TITLE_H * sf) as usize;
    let tg = (TITLE_GAP * sf) as usize;
    let row_h = (ROW_H * sf) as usize;
    let ug = (UPGRADE_GAP * sf) as usize;
    let btn_y = py + pad + title_lh + tg + TRACKED.len() * row_h + ug;
    let btn_h = (BTN_H * sf) as usize;

    if cy >= btn_y && cy < btn_y + btn_h {
        let upgrade_pad = (16.0 * sf) as usize;
        let btn_gap = (10.0 * sf) as usize;

        let upgrade_w = upgrade_pad * 2 + (100.0 * sf) as usize;
        if cx >= content_x && cx < content_x + upgrade_w {
            return Some(UsageLimitBannerHit::Upgrade);
        }

        let close_x = content_x + upgrade_w + btn_gap;
        let close_w = (16.0 * sf) as usize * 2 + (50.0 * sf) as usize;
        if cx >= close_x && cx < close_x + close_w {
            return Some(UsageLimitBannerHit::Dismiss);
        }
    }

    None
}

pub fn hover_test(
    state: &UsageLimitBannerState,
    mx: f64,
    my: f64,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> Option<usize> {
    if !state.visible {
        return None;
    }
    let (px, py, pw, ph) = panel_rect(buf_w, buf_h, sf);
    let cx = mx as usize;
    let cy = my as usize;

    if cx < px || cx >= px + pw || cy < py || cy >= py + ph {
        return None;
    }

    let pad = (PAD * sf) as usize;
    let content_x = px + pad;
    let title_lh = (TITLE_H * sf) as usize;
    let tg = (TITLE_GAP * sf) as usize;
    let row_h = (ROW_H * sf) as usize;
    let ug = (UPGRADE_GAP * sf) as usize;
    let btn_y = py + pad + title_lh + tg + TRACKED.len() * row_h + ug;
    let btn_h = (BTN_H * sf) as usize;

    if cy >= btn_y && cy < btn_y + btn_h {
        let upgrade_pad = (16.0 * sf) as usize;
        let btn_gap = (10.0 * sf) as usize;

        let upgrade_w = upgrade_pad * 2 + (100.0 * sf) as usize;
        if cx >= content_x && cx < content_x + upgrade_w {
            return Some(0);
        }

        let close_x = content_x + upgrade_w + btn_gap;
        let close_w = (16.0 * sf) as usize * 2 + (50.0 * sf) as usize;
        if cx >= close_x && cx < close_x + close_w {
            return Some(1);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_hidden() {
        let s = UsageLimitBannerState::default();
        assert!(!s.visible);
        assert!(s.hovered.is_none());
        assert!(!s.is_visible());
    }

    #[test]
    fn show_and_dismiss() {
        let mut s = UsageLimitBannerState::default();
        s.show();
        assert!(s.is_visible());
        s.dismiss();
        assert!(!s.is_visible());
    }

    #[test]
    fn panel_height_positive() {
        assert!(panel_height(1.0) > 0);
        assert!(panel_height(2.0) > panel_height(1.0));
    }

    #[test]
    fn panel_rect_centered() {
        let (px, py, pw, ph) = panel_rect(1000, 800, 1.0);
        assert!(px > 0);
        assert!(py > 0);
        assert_eq!(px, (1000 - pw) / 2);
        assert_eq!(py, (800 - ph) / 2);
    }

    #[test]
    fn bar_color_normal() {
        assert_eq!(bar_color(0, 20), BAR_NORMAL);
        assert_eq!(bar_color(10, 20), BAR_NORMAL);
    }

    #[test]
    fn bar_color_warning_near_limit() {
        assert_eq!(bar_color(18, 20), BAR_WARNING);
    }

    #[test]
    fn bar_color_exhausted_at_limit() {
        assert_eq!(bar_color(20, 20), BAR_EXHAUSTED);
        assert_eq!(bar_color(25, 20), BAR_EXHAUSTED);
    }

    #[test]
    fn hit_test_backdrop_outside() {
        let mut s = UsageLimitBannerState::default();
        s.show();
        let hit = hit_test(&s, 0.0, 0.0, 1000, 800, 1.0);
        assert_eq!(hit, Some(UsageLimitBannerHit::Backdrop));
    }
}
