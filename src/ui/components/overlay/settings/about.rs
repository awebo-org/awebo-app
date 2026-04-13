//! Settings "About" tab rendering.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::AvatarRenderer;
use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{draw_text_at, draw_text_at_bold, measure_text_width};
use crate::renderer::theme;

const BTN_H: f32 = 28.0;
const BTN_R: f32 = 4.0;
const BTN_PAD: f32 = 16.0;
const BTN_GAP: f32 = 8.0;
const SECTION_GAP: f32 = 36.0;

const DEACTIVATE_BG: Rgb = (60, 40, 40);
const DEACTIVATE_HOVER_BG: Rgb = (90, 50, 50);
const NEUTRAL_BG: Rgb = (40, 40, 46);
const NEUTRAL_HOVER_BG: Rgb = (60, 60, 68);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AboutHit {
    ResetHints,
    ResetSettings,
    UpgradeToPro,
    DeactivateLicense,
}

fn layout_y(area_y: usize, sf: f32) -> (usize, usize, usize) {
    let avatar_size = (64.0 * sf).round() as usize;
    let avatar_y = area_y + (30.0 * sf) as usize;
    let title_y = avatar_y + avatar_size + (16.0 * sf) as usize;
    let version_y = title_y + (40.0 * sf) as usize;
    (avatar_y, title_y, version_y)
}

fn approx_btn_width(label: &str, sf: f32) -> usize {
    let tw = (label.len() as f32 * 7.2 * sf) as usize;
    tw + 2 * (BTN_PAD * sf) as usize
}

struct BtnLayout {
    x: usize,
    y: usize,
    w: usize,
}

fn compute_row(labels: &[&str], cx: usize, y: usize, sf: f32) -> Vec<BtnLayout> {
    let gap = (BTN_GAP * sf) as usize;
    let widths: Vec<usize> = labels.iter().map(|l| approx_btn_width(l, sf)).collect();
    let total_w: usize = widths.iter().sum::<usize>() + gap * widths.len().saturating_sub(1);
    let start_x = cx.saturating_sub(total_w / 2);

    let mut result = Vec::with_capacity(labels.len());
    let mut x = start_x;
    for w in widths {
        result.push(BtnLayout { x, y, w });
        x += w + gap;
    }
    result
}

fn draw_btn(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    layout: &BtnLayout,
    label: &str,
    bg: Rgb,
    text_color: Rgb,
    bold: bool,
    sf: f32,
    clip_h: usize,
) {
    let btn_h = (BTN_H * sf) as usize;
    let btn_r = (BTN_R * sf) as usize;
    super::super::fill_rounded_rect(buf, layout.x, layout.y, layout.w, btn_h, btn_r, bg);
    let m = Metrics::new(12.0 * sf, 17.0 * sf);
    let tw = measure_text_width(font_system, label, m, Family::Monospace) as usize;
    let lx = layout.x + (layout.w.saturating_sub(tw)) / 2;
    let ly = layout.y + ((btn_h as f32 - 17.0 * sf) / 2.0) as usize;
    if bold {
        draw_text_at_bold(
            buf,
            font_system,
            swash_cache,
            lx,
            ly,
            clip_h,
            label,
            m,
            text_color,
            Family::Monospace,
        );
    } else {
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            lx,
            ly,
            clip_h,
            label,
            m,
            text_color,
            Family::Monospace,
        );
    }
}

pub fn draw_settings_about(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    avatar_renderer: &mut AvatarRenderer,
    area_x: usize,
    area_y: usize,
    area_w: usize,
    clip_h: usize,
    sf: f32,
    hovered: Option<AboutHit>,
    is_pro: bool,
) {
    let cx = area_x + area_w / 2;
    let (avatar_y, title_y, version_y) = layout_y(area_y, sf);

    let avatar_size = (64.0 * sf).round() as u32;
    let avatar_x = cx.saturating_sub(avatar_size as usize / 2);
    avatar_renderer.draw(buf, avatar_x, avatar_y, avatar_size);

    let title_metrics = Metrics::new(22.0 * sf, 30.0 * sf);
    let title_tw =
        measure_text_width(font_system, "Awebo", title_metrics, Family::Monospace) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        cx.saturating_sub(title_tw / 2),
        title_y,
        clip_h,
        "Awebo",
        title_metrics,
        theme::SETTINGS_HEADER_TEXT,
        Family::Monospace,
    );

    let version = if is_pro { "v0.1.0 Pro" } else { "v0.1.0" };
    let version_metrics = Metrics::new(12.0 * sf, 17.0 * sf);
    let version_tw =
        measure_text_width(font_system, version, version_metrics, Family::Monospace) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        cx.saturating_sub(version_tw / 2),
        version_y,
        clip_h,
        version,
        version_metrics,
        theme::SETTINGS_BODY_TEXT,
        Family::Monospace,
    );

    let row_y = version_y + (SECTION_GAP * sf) as usize;

    if is_pro {
        let labels = ["Deactivate License", "Reset hints", "Reset settings"];
        let layouts = compute_row(&labels, cx, row_y, sf);

        let deact_bg = if hovered == Some(AboutHit::DeactivateLicense) {
            DEACTIVATE_HOVER_BG
        } else {
            DEACTIVATE_BG
        };
        draw_btn(
            buf,
            font_system,
            swash_cache,
            &layouts[0],
            labels[0],
            deact_bg,
            (220, 80, 80),
            false,
            sf,
            clip_h,
        );

        let rh_bg = if hovered == Some(AboutHit::ResetHints) {
            NEUTRAL_HOVER_BG
        } else {
            NEUTRAL_BG
        };
        draw_btn(
            buf,
            font_system,
            swash_cache,
            &layouts[1],
            labels[1],
            rh_bg,
            theme::SETTINGS_BODY_TEXT,
            false,
            sf,
            clip_h,
        );

        let rs_bg = if hovered == Some(AboutHit::ResetSettings) {
            NEUTRAL_HOVER_BG
        } else {
            NEUTRAL_BG
        };
        draw_btn(
            buf,
            font_system,
            swash_cache,
            &layouts[2],
            labels[2],
            rs_bg,
            theme::SETTINGS_BODY_TEXT,
            false,
            sf,
            clip_h,
        );
    } else {
        let labels = ["Upgrade to Pro", "Reset hints", "Reset settings"];
        let layouts = compute_row(&labels, cx, row_y, sf);

        let upgrade_bg = if hovered == Some(AboutHit::UpgradeToPro) {
            (239, 59, 139)
        } else {
            theme::PRIMARY
        };
        draw_btn(
            buf,
            font_system,
            swash_cache,
            &layouts[0],
            labels[0],
            upgrade_bg,
            (255, 255, 255),
            true,
            sf,
            clip_h,
        );

        let rh_bg = if hovered == Some(AboutHit::ResetHints) {
            NEUTRAL_HOVER_BG
        } else {
            NEUTRAL_BG
        };
        draw_btn(
            buf,
            font_system,
            swash_cache,
            &layouts[1],
            labels[1],
            rh_bg,
            theme::SETTINGS_BODY_TEXT,
            false,
            sf,
            clip_h,
        );

        let rs_bg = if hovered == Some(AboutHit::ResetSettings) {
            NEUTRAL_HOVER_BG
        } else {
            NEUTRAL_BG
        };
        draw_btn(
            buf,
            font_system,
            swash_cache,
            &layouts[2],
            labels[2],
            rs_bg,
            theme::SETTINGS_BODY_TEXT,
            false,
            sf,
            clip_h,
        );
    }
}

pub fn about_hit_test(
    mx: usize,
    my: usize,
    area_x: usize,
    area_y: usize,
    area_w: usize,
    sf: f32,
    is_pro: bool,
) -> Option<AboutHit> {
    let cx = area_x + area_w / 2;
    let (_avatar_y, _title_y, version_y) = layout_y(area_y, sf);
    let btn_h = (BTN_H * sf) as usize;

    let row_y = version_y + (SECTION_GAP * sf) as usize;

    if is_pro {
        let layouts = compute_row(
            &["Deactivate License", "Reset hints", "Reset settings"],
            cx,
            row_y,
            sf,
        );
        if hit_btn(mx, my, &layouts[0], btn_h) {
            return Some(AboutHit::DeactivateLicense);
        }
        if hit_btn(mx, my, &layouts[1], btn_h) {
            return Some(AboutHit::ResetHints);
        }
        if hit_btn(mx, my, &layouts[2], btn_h) {
            return Some(AboutHit::ResetSettings);
        }
    } else {
        let layouts = compute_row(
            &["Upgrade to Pro", "Reset hints", "Reset settings"],
            cx,
            row_y,
            sf,
        );
        if hit_btn(mx, my, &layouts[0], btn_h) {
            return Some(AboutHit::UpgradeToPro);
        }
        if hit_btn(mx, my, &layouts[1], btn_h) {
            return Some(AboutHit::ResetHints);
        }
        if hit_btn(mx, my, &layouts[2], btn_h) {
            return Some(AboutHit::ResetSettings);
        }
    }

    None
}

fn hit_btn(mx: usize, my: usize, layout: &BtnLayout, h: usize) -> bool {
    mx >= layout.x && mx < layout.x + layout.w && my >= layout.y && my < layout.y + h
}
