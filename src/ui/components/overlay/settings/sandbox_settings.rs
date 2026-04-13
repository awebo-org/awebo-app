//! Settings "Sandbox" tab — resource sliders, volumes, custom images.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::{Icon, IconRenderer};
use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::{SandboxSlider, SettingsState};

const SLIDER_HEIGHT: f32 = 6.0;
const SLIDER_THUMB_R: f32 = 8.0;
const SLIDER_TRACK_COLOR: (u8, u8, u8) = (40, 40, 46);
const SLIDER_FILL_COLOR: (u8, u8, u8) = crate::renderer::theme::PRIMARY;

/// Computed pixel positions for slider interaction.
struct SliderLayout {
    /// CPU slider: (track_x, track_y, track_w)
    cpu: (usize, usize, usize),
    /// Memory slider: (track_x, track_y, track_w)
    mem: (usize, usize, usize),
}

/// Compute the slider pixel positions matching `draw_settings_sandbox` layout.
fn compute_slider_layout(area_x: usize, area_y: usize, area_w: usize, sf: f32, scroll: usize) -> SliderLayout {
    let pad = (24.0 * sf) as usize;
    let row_h = (40.0 * sf) as usize;
    let section_gap = (24.0 * sf) as usize;
    let slider_w = area_w.saturating_sub(pad * 2);

    let mut y = area_y + scroll;
    y += (32.0 * sf) as usize;
    y += row_h;
    y += row_h + section_gap;
    y += section_gap;
    y += (32.0 * sf) as usize;
    y += (22.0 * sf) as usize;
    let cpu_y = y.saturating_sub(scroll);
    y += (28.0 * sf) as usize;
    y += (22.0 * sf) as usize;
    let mem_y = y.saturating_sub(scroll);

    SliderLayout {
        cpu: (area_x + pad, cpu_y, slider_w),
        mem: (area_x + pad, mem_y, slider_w),
    }
}

/// Hit-test sliders. Returns which slider and the fraction [0..1].
pub fn sandbox_slider_hit_test(
    mx: f64,
    my: f64,
    bar_h: usize,
    content_x: usize,
    content_w: usize,
    sf: f32,
    scroll: f32,
) -> Option<(SandboxSlider, f32)> {
    let area_x = content_x;
    let area_y = bar_h;
    let layout = compute_slider_layout(area_x, area_y, content_w, sf, scroll as usize);
    let grab_tolerance = (SLIDER_THUMB_R * sf * 1.5) as usize;

    for &(slider_kind, (sx, sy, sw)) in &[
        (SandboxSlider::Cpu, layout.cpu),
        (SandboxSlider::Memory, layout.mem),
    ] {
        let track_h = (SLIDER_HEIGHT * sf).max(2.0) as usize;
        let cy = sy + track_h / 2;
        if (my as usize) >= cy.saturating_sub(grab_tolerance)
            && (my as usize) <= cy + grab_tolerance
            && (mx as usize) >= sx.saturating_sub(grab_tolerance)
            && (mx as usize) <= sx + sw + grab_tolerance
        {
            let frac = ((mx as f32 - sx as f32) / sw as f32).clamp(0.0, 1.0);
            return Some((slider_kind, frac));
        }
    }
    None
}

/// Convert a slider fraction to a value in [min, max].
pub fn fraction_to_value(frac: f32, min: u32, max: u32) -> u32 {
    if max <= min { return min; }
    let val = min as f32 + frac * (max - min) as f32;
    (val.round() as u32).clamp(min, max)
}

/// Compute total content height for sandbox settings (used for max scroll).
pub fn sandbox_settings_content_height(sf: f32) -> f32 {
    let row_h = 40.0 * sf;
    let section_gap = 24.0 * sf;

    let mut h: f32 = 0.0;
    h += 32.0 * sf + row_h + row_h + section_gap;
    h += section_gap;
    h += 32.0 * sf + 22.0 * sf + 28.0 * sf + 22.0 * sf + 28.0 * sf + section_gap;
    h += section_gap;
    h += 32.0 * sf;
    let builtin_count = crate::sandbox::images::IMAGES.len() as f32;
    h += row_h * builtin_count;
    h += 8.0 * sf + 22.0 * sf;
    let config = crate::config::AppConfig::load();
    let custom_count = config.sandbox.custom_images.len().max(1) as f32;
    h += row_h * custom_count;
    h += 8.0 * sf + 30.0 * sf + section_gap;
    h += section_gap;
    h += 32.0 * sf;
    if config.sandbox.volumes.is_empty() {
        h += row_h;
    } else {
        h += 48.0 * sf * config.sandbox.volumes.len() as f32;
    }
    h += section_gap;
    h
}

/// Check if the mouse is over the scrollbar thumb.
/// Returns `true` if mouse is within the thumb area — caller should start drag.
/// Layout must match the scrollbar drawn in `draw_settings_sandbox`.
pub fn scrollbar_thumb_hit_test(
    mx: f64,
    my: f64,
    area_y: usize,
    content_x: usize,
    content_w: usize,
    viewport_h: f32,
    sf: f32,
    scroll: f32,
) -> bool {
    let total_content = sandbox_settings_content_height(sf);
    if total_content <= viewport_h {
        return false;
    }
    let track_w = (4.0 * sf).max(2.0) as usize;
    let hit_w = (12.0 * sf).max(6.0) as usize;
    let track_x = content_x + content_w - track_w - (2.0 * sf) as usize;
    let hit_x = track_x.saturating_sub((hit_w - track_w) / 2);

    let thumb_ratio = viewport_h / total_content;
    let thumb_h = ((viewport_h * thumb_ratio) as usize).max((12.0 * sf) as usize);
    let max_scroll = total_content - viewport_h;
    let scroll_ratio = (scroll / max_scroll).clamp(0.0, 1.0);
    let track_space = viewport_h as usize - thumb_h;
    let thumb_y = area_y + (track_space as f32 * scroll_ratio) as usize;

    mx >= hit_x as f64
        && mx < (hit_x + hit_w) as f64
        && my >= thumb_y as f64
        && my < (thumb_y + thumb_h) as f64
}

/// Hit-test for interactive elements (delete buttons, add-image).
/// Layout must match `draw_settings_sandbox`.
pub fn sandbox_settings_hit_test(
    mx: f64,
    my: f64,
    bar_h: usize,
    content_x: usize,
    content_w: usize,
    sf: f32,
    scroll: f32,
) -> Option<super::SandboxSettingsHit> {
    use super::SandboxSettingsHit;

    let content_my = my + scroll as f64;

    let area_x = content_x;
    let area_y = bar_h;
    let area_w = content_w;
    let pad = (24.0 * sf) as usize;
    let row_h = (40.0 * sf) as usize;
    let section_gap = (24.0 * sf) as usize;
    let btn_icon = (14.0 * sf) as usize;
    let btn_pad = (5.0 * sf) as usize;
    let full_btn = btn_icon + btn_pad * 2;
    let btn_gap_x = (10.0 * sf) as usize;

    let mut y = area_y;

    y += (32.0 * sf) as usize;
    y += row_h;
    y += row_h + section_gap;
    y += section_gap;

    y += (32.0 * sf) as usize;
    y += (22.0 * sf) as usize + (28.0 * sf) as usize;
    y += (22.0 * sf) as usize + (28.0 * sf) as usize + section_gap;

    y += section_gap;

    y += (32.0 * sf) as usize;

    let builtin_count = crate::sandbox::images::IMAGES.len();
    for bi in 0..builtin_count {
        let del_bx = area_x + area_w - pad - full_btn;
        let del_by = y + (row_h - full_btn) / 2;
        if mx >= del_bx as f64 && mx < (del_bx + full_btn) as f64
            && content_my >= del_by as f64 && content_my < (del_by + full_btn) as f64
        {
            return Some(SandboxSettingsHit::DeleteTrustedImage(bi));
        }
        let upd_bx = del_bx - btn_gap_x - full_btn;
        let upd_by = del_by;
        if mx >= upd_bx as f64 && mx < (upd_bx + full_btn) as f64
            && content_my >= upd_by as f64 && content_my < (upd_by + full_btn) as f64
        {
            return Some(SandboxSettingsHit::UpdateTrustedImage(bi));
        }
        y += row_h;
    }

    y += (8.0 * sf) as usize;
    y += (22.0 * sf) as usize;

    let config = crate::config::AppConfig::load();
    for ci_idx in 0..config.sandbox.custom_images.len() {
        let del_bx = area_x + area_w - pad - full_btn;
        let del_by = y + (row_h - full_btn) / 2;
        if mx >= del_bx as f64 && mx < (del_bx + full_btn) as f64
            && content_my >= del_by as f64 && content_my < (del_by + full_btn) as f64
        {
            return Some(SandboxSettingsHit::DeleteCustomImage(ci_idx));
        }
        y += row_h;
    }

    if config.sandbox.custom_images.is_empty() {
        y += row_h; // "No custom images" placeholder
    }

    y += (8.0 * sf) as usize;
    let input_h = (30.0 * sf) as usize;
    let btn_gap = (8.0 * sf) as usize;
    let add_btn_w = (60.0 * sf) as usize;
    let input_w = area_w.saturating_sub(pad * 2 + btn_gap + add_btn_w);
    let input_x = area_x + pad;

    if mx >= input_x as f64 && mx < (input_x + input_w) as f64
        && content_my >= y as f64 && content_my < (y + input_h) as f64
    {
        return Some(SandboxSettingsHit::AddImageInput);
    }

    let add_x = input_x + input_w + btn_gap;
    if mx >= add_x as f64 && mx < (add_x + add_btn_w) as f64
        && content_my >= y as f64 && content_my < (y + input_h) as f64
    {
        return Some(SandboxSettingsHit::AddImage);
    }

    y += input_h + section_gap;
    y += section_gap;
    y += (32.0 * sf) as usize;

    let vol_row_h = (48.0 * sf) as usize;
    for vi in 0..config.sandbox.volumes.len() {
        let del_bx = area_x + area_w - pad - full_btn;
        let del_by = y + (vol_row_h - full_btn) / 2;
        if mx >= del_bx as f64 && mx < (del_bx + full_btn) as f64
            && content_my >= del_by as f64 && content_my < (del_by + full_btn) as f64
        {
            return Some(SandboxSettingsHit::DeleteVolume(vi));
        }
        y += vol_row_h;
    }

    None
}

pub fn draw_settings_sandbox(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &SettingsState,
    area_x: usize,
    area_y: usize,
    area_w: usize,
    clip_h: usize,
    sf: f32,
) {
    let pad = (24.0 * sf) as usize;
    let section_metrics = Metrics::new(15.0 * sf, 21.0 * sf);
    let label_metrics = Metrics::new(13.0 * sf, 18.0 * sf);
    let desc_metrics = Metrics::new(11.5 * sf, 16.0 * sf);
    let small_metrics = Metrics::new(10.5 * sf, 14.0 * sf);
    let row_h = (40.0 * sf) as usize;
    let section_gap = (24.0 * sf) as usize;
    let line_pad = (20.0 * sf) as usize;
    let divider_h = (1.0 * sf).max(1.0) as usize;

    let sb = &state.sandbox;
    let scroll = sb.scroll_offset as i32;

    let mut vy: i32 = 0;

    let vis = |vy: i32, h: usize| -> Option<usize> {
        let sy = vy - scroll + area_y as i32;
        if sy + h as i32 <= area_y as i32 || sy >= clip_h as i32 {
            None
        } else {
            Some(sy.max(0) as usize)
        }
    };

    if let Some(sy) = vis(vy, 32) {
        draw_text_at(
            buf, font_system, swash_cache,
            area_x + pad, sy, clip_h,
            "Runtime", section_metrics,
            theme::SETTINGS_SECTION_TITLE, Family::Monospace,
        );
    }
    vy += (32.0 * sf) as i32;

    let available = crate::sandbox::manager::SandboxManager::new().is_available();
    let (status_text, status_color) = if available {
        ("microsandbox available", (80, 200, 120))
    } else {
        ("microsandbox not available", (200, 80, 80))
    };

    if let Some(sy) = vis(vy, row_h) {
        let dot_r = (4.0 * sf).max(1.0);
        let dot_x = (area_x + pad) as f32 + dot_r;
        let dot_cy = sy as f32 + row_h as f32 / 2.0;
        buf.fill_circle(dot_x, dot_cy, dot_r, status_color);

        let text_x = area_x + pad + (dot_r * 2.0 + 8.0 * sf) as usize;
        draw_text_at(
            buf, font_system, swash_cache,
            text_x, sy + (row_h as f32 / 2.0 - 9.0 * sf) as usize, clip_h,
            status_text, label_metrics,
            theme::SETTINGS_BODY_TEXT, Family::Monospace,
        );
    }
    vy += row_h as i32;

    let platform = if cfg!(target_os = "macos") {
        "macOS (Apple Virtualization)"
    } else if cfg!(target_os = "linux") {
        "Linux (KVM)"
    } else {
        "Unsupported platform"
    };
    if let Some(sy) = vis(vy, row_h) {
        draw_text_at(
            buf, font_system, swash_cache,
            area_x + pad, sy + (row_h as f32 / 2.0 - 9.0 * sf) as usize, clip_h,
            "Platform", label_metrics,
            theme::SETTINGS_LABEL, Family::Monospace,
        );
        let val_x = area_x + area_w / 2;
        draw_text_at(
            buf, font_system, swash_cache,
            val_x, sy + (row_h as f32 / 2.0 - 9.0 * sf) as usize, clip_h,
            platform, desc_metrics,
            theme::SETTINGS_BODY_TEXT, Family::Monospace,
        );
    }
    vy += row_h as i32 + section_gap as i32;

    if let Some(sy) = vis(vy, divider_h) {
        buf.fill_rect(area_x + line_pad, sy, area_w.saturating_sub(line_pad * 2), divider_h, theme::SETTINGS_DIVIDER);
    }
    vy += section_gap as i32;

    if let Some(sy) = vis(vy, 32) {
        draw_text_at(
            buf, font_system, swash_cache,
            area_x + pad, sy, clip_h,
            "Default Resources", section_metrics,
            theme::SETTINGS_SECTION_TITLE, Family::Monospace,
        );
    }
    vy += (32.0 * sf) as i32;

    let slider_w = area_w.saturating_sub(pad * 2);
    let cpu_label = format!("vCPU: {} / {}", sb.cpus, sb.system_cpus);
    if let Some(sy) = vis(vy, 22) {
        draw_text_at(
            buf, font_system, swash_cache,
            area_x + pad, sy, clip_h,
            &cpu_label, label_metrics,
            theme::SETTINGS_BODY_TEXT, Family::Monospace,
        );
    }
    vy += (22.0 * sf) as i32;

    if let Some(sy) = vis(vy, 28) {
        draw_slider(buf, area_x + pad, sy, slider_w, sf, sb.cpus, 1, sb.system_cpus);
    }
    vy += (28.0 * sf) as i32;

    let mem_label = format!("Memory: {} MiB / {} MiB", sb.memory_mib, sb.system_memory_mib);
    if let Some(sy) = vis(vy, 22) {
        draw_text_at(
            buf, font_system, swash_cache,
            area_x + pad, sy, clip_h,
            &mem_label, label_metrics,
            theme::SETTINGS_BODY_TEXT, Family::Monospace,
        );
    }
    vy += (22.0 * sf) as i32;

    let mem_max = sb.system_memory_mib;
    let mem_min = 128_u32;
    if let Some(sy) = vis(vy, 28) {
        draw_slider(buf, area_x + pad, sy, slider_w, sf, sb.memory_mib, mem_min, mem_max);
    }
    vy += (28.0 * sf) as i32 + section_gap as i32;

    if let Some(sy) = vis(vy, divider_h) {
        buf.fill_rect(area_x + line_pad, sy, area_w.saturating_sub(line_pad * 2), divider_h, theme::SETTINGS_DIVIDER);
    }
    vy += section_gap as i32;

    if let Some(sy) = vis(vy, 32) {
        draw_text_at(
            buf, font_system, swash_cache,
            area_x + pad, sy, clip_h,
            "Trusted Images", section_metrics,
            theme::SETTINGS_SECTION_TITLE, Family::Monospace,
        );
    }
    vy += (32.0 * sf) as i32;

    let icon_sz = (14.0 * sf).round() as u32;
    let btn_icon = (14.0 * sf).round() as usize;
    let btn_pad = (5.0 * sf) as usize;
    let full_btn = btn_icon + btn_pad * 2;
    let btn_gap_x = (10.0 * sf) as usize;
    let btn_r = (4.0 * sf) as usize;
    let images = crate::sandbox::images::IMAGES;

    for (bi, img) in images.iter().enumerate() {
        if let Some(sy) = vis(vy, row_h) {
            draw_image_row(buf, font_system, swash_cache, icon_renderer,
                area_x + pad, sy, area_w, row_h, sf,
                icon_sz, img.display_name, img.oci_ref, None,
                label_metrics, desc_metrics, small_metrics);

            let del_bx = area_x + area_w - pad - full_btn;
            let del_by = sy + (row_h - full_btn) / 2;
            let is_del_hovered = sb.hovered_hit == Some(super::SandboxSettingsHit::DeleteTrustedImage(bi));
            let del_bg = if is_del_hovered { theme::BG_HOVER } else { theme::BG_ELEVATED };
            super::super::fill_rounded_rect(buf, del_bx, del_by, full_btn, full_btn, btn_r, del_bg);
            let del_color = if is_del_hovered { theme::ERROR } else { theme::FG_MUTED };
            icon_renderer.draw(buf, Icon::Trash, del_bx + btn_pad, del_by + btn_pad, btn_icon as u32, del_color);

            let upd_bx = del_bx - btn_gap_x - full_btn;
            let upd_by = del_by;
            let is_upd_hovered = sb.hovered_hit == Some(super::SandboxSettingsHit::UpdateTrustedImage(bi));
            let upd_bg = if is_upd_hovered { theme::BG_HOVER } else { theme::BG_ELEVATED };
            super::super::fill_rounded_rect(buf, upd_bx, upd_by, full_btn, full_btn, btn_r, upd_bg);
            let upd_color = if is_upd_hovered { theme::TOAST_INFO_ACCENT } else { theme::FG_MUTED };
            icon_renderer.draw(buf, Icon::Refresh, upd_bx + btn_pad, upd_by + btn_pad, btn_icon as u32, upd_color);
        }
        vy += row_h as i32;
    }

    let config = crate::config::AppConfig::load();
    vy += (8.0 * sf) as i32;
    if let Some(sy) = vis(vy, 22) {
        draw_text_at(
            buf, font_system, swash_cache,
            area_x + pad, sy, clip_h,
            "Custom Images", Metrics::new(12.0 * sf, 17.0 * sf),
            theme::FG_MUTED, Family::Monospace,
        );
    }
    vy += (22.0 * sf) as i32;

    for (ci_idx, ci) in config.sandbox.custom_images.iter().enumerate() {
        if let Some(sy) = vis(vy, row_h) {
            let pulled = if ci.last_pulled.is_empty() { None } else { Some(ci.last_pulled.as_str()) };
            draw_image_row(buf, font_system, swash_cache, icon_renderer,
                area_x + pad, sy, area_w, row_h, sf,
                icon_sz, &ci.display_name, &ci.oci_ref, pulled,
                label_metrics, desc_metrics, small_metrics);

            let del_bx = area_x + area_w - pad - full_btn;
            let del_by = sy + (row_h - full_btn) / 2;
            let is_hovered = sb.hovered_hit == Some(super::SandboxSettingsHit::DeleteCustomImage(ci_idx));
            let del_bg = if is_hovered { theme::BG_HOVER } else { theme::BG_ELEVATED };
            super::super::fill_rounded_rect(buf, del_bx, del_by, full_btn, full_btn, btn_r, del_bg);
            let del_color = if is_hovered { theme::ERROR } else { theme::FG_MUTED };
            icon_renderer.draw(buf, Icon::Trash, del_bx + btn_pad, del_by + btn_pad, btn_icon as u32, del_color);
        }
        vy += row_h as i32;
    }

    if config.sandbox.custom_images.is_empty() {
        if let Some(sy) = vis(vy, row_h) {
            draw_text_at(
                buf, font_system, swash_cache,
                area_x + pad, sy, clip_h,
                "No custom images", desc_metrics,
                theme::FG_MUTED, Family::Monospace,
            );
        }
        vy += row_h as i32;
    }

    vy += (8.0 * sf) as i32;
    let input_h = (30.0 * sf) as usize;
    let btn_gap = (8.0 * sf) as usize;
    let add_btn_w = (60.0 * sf) as usize;
    let input_w = area_w.saturating_sub(pad * 2 + btn_gap + add_btn_w);
    let input_x = area_x + pad;

    if let Some(sy) = vis(vy, input_h) {
        let input_border = if sb.add_image_focused { theme::PRIMARY } else { theme::SETTINGS_DIVIDER };
        super::super::draw_border(buf, input_x, sy, input_w, input_h, (1.0 * sf).max(1.0) as usize, input_border);
        let input_text = if sb.add_image_input.is_empty() && !sb.add_image_focused {
            "oci-ref (e.g. docker.io/library/ubuntu)"
        } else {
            &sb.add_image_input
        };
        let input_color = if sb.add_image_input.is_empty() && !sb.add_image_focused {
            theme::FG_MUTED
        } else {
            theme::SETTINGS_INPUT_TEXT
        };
        draw_text_at(
            buf, font_system, swash_cache,
            input_x + (8.0 * sf) as usize,
            sy + (input_h as f32 / 2.0 - 7.0 * sf) as usize,
            clip_h,
            input_text, desc_metrics,
            input_color, Family::Monospace,
        );

        let add_x = input_x + input_w + btn_gap;
        let is_add_hovered = sb.hovered_hit == Some(super::SandboxSettingsHit::AddImage);
        let add_bg = if is_add_hovered { theme::PRIMARY } else { theme::BG_ELEVATED };
        let add_r = (4.0 * sf) as usize;
        super::super::fill_rounded_rect(buf, add_x, sy, add_btn_w, input_h, add_r, add_bg);
        let add_text_color = if is_add_hovered { (255, 255, 255) } else { theme::FG_SECONDARY };
        let add_text_x = add_x + (add_btn_w as f32 / 2.0 - 10.0 * sf) as usize;
        draw_text_at(
            buf, font_system, swash_cache,
            add_text_x,
            sy + (input_h as f32 / 2.0 - 7.0 * sf) as usize,
            clip_h,
            "Add", desc_metrics,
            add_text_color, Family::Monospace,
        );
    }
    vy += input_h as i32 + section_gap as i32;

    if let Some(sy) = vis(vy, divider_h) {
        buf.fill_rect(area_x + line_pad, sy, area_w.saturating_sub(line_pad * 2), divider_h, theme::SETTINGS_DIVIDER);
    }
    vy += section_gap as i32;

    if let Some(sy) = vis(vy, 32) {
        draw_text_at(
            buf, font_system, swash_cache,
            area_x + pad, sy, clip_h,
            "Default Volumes", section_metrics,
            theme::SETTINGS_SECTION_TITLE, Family::Monospace,
        );
    }
    vy += (32.0 * sf) as i32;

    if config.sandbox.volumes.is_empty() {
        if let Some(sy) = vis(vy, row_h) {
            draw_text_at(
                buf, font_system, swash_cache,
                area_x + pad, sy, clip_h,
                "No default volumes configured", desc_metrics,
                theme::FG_MUTED, Family::Monospace,
            );
        }
        vy += row_h as i32;
    } else {
        let vol_row_h = (48.0 * sf) as usize;
        for (vi, vol) in config.sandbox.volumes.iter().enumerate() {
            if let Some(sy) = vis(vy, vol_row_h) {
                draw_text_at(
                    buf, font_system, swash_cache,
                    area_x + pad, sy + (4.0 * sf) as usize, clip_h,
                    &vol.label, label_metrics,
                    theme::SETTINGS_BODY_TEXT, Family::Monospace,
                );

                let del_bx = area_x + area_w - pad - full_btn;
                let del_by = sy + (vol_row_h - full_btn) / 2;
                let is_hovered = sb.hovered_hit == Some(super::SandboxSettingsHit::DeleteVolume(vi));
                let del_bg = if is_hovered { theme::BG_HOVER } else { theme::BG_ELEVATED };
                super::super::fill_rounded_rect(buf, del_bx, del_by, full_btn, full_btn, btn_r, del_bg);
                let del_color = if is_hovered { theme::ERROR } else { theme::FG_MUTED };
                icon_renderer.draw(buf, Icon::Trash, del_bx + btn_pad, del_by + btn_pad, btn_icon as u32, del_color);

                let mapping = format!("{} → {}", vol.guest_path, vol.host_path);
                draw_text_at(
                    buf, font_system, swash_cache,
                    area_x + pad, sy + (24.0 * sf) as usize, clip_h,
                    &mapping, small_metrics,
                    theme::FG_MUTED, Family::Monospace,
                );
            }
            vy += vol_row_h as i32;
        }
    }
    let total_content = vy.max(0) as f32;
    let viewport = (clip_h - area_y) as f32;
    if total_content > viewport {
        let track_w = (4.0 * sf).max(2.0) as usize;
        let track_x = area_x + area_w - track_w - (2.0 * sf) as usize;
        let thumb_ratio = viewport / total_content;
        let thumb_h = ((viewport * thumb_ratio) as usize).max((12.0 * sf) as usize);
        let max_scroll = total_content - viewport;
        let scroll_ratio = (scroll as f32 / max_scroll).clamp(0.0, 1.0);
        let track_space = viewport as usize - thumb_h;
        let thumb_y = area_y + (track_space as f32 * scroll_ratio) as usize;
        let r = track_w / 2;
        super::super::fill_rounded_rect(buf, track_x, thumb_y, track_w, thumb_h, r, theme::FG_MUTED);
    }
}

/// Draw a horizontal slider track + filled portion + thumb.
fn draw_slider(
    buf: &mut PixelBuffer,
    x: usize,
    y: usize,
    w: usize,
    sf: f32,
    value: u32,
    min: u32,
    max: u32,
) {
    if max <= min { return; }

    let track_h = (SLIDER_HEIGHT * sf).max(2.0) as usize;
    let track_r = track_h / 2;
    let thumb_r = (SLIDER_THUMB_R * sf).max(3.0);

    let track_y = y;

    super::super::fill_rounded_rect(buf, x, track_y, w, track_h, track_r, SLIDER_TRACK_COLOR);

    let ratio = ((value.saturating_sub(min)) as f32) / ((max - min) as f32);
    let fill_w = (w as f32 * ratio.clamp(0.0, 1.0)) as usize;
    if fill_w > 0 {
        super::super::fill_rounded_rect(buf, x, track_y, fill_w, track_h, track_r, SLIDER_FILL_COLOR);
    }

    let thumb_cx = x as f32 + fill_w as f32;
    let thumb_cy = track_y as f32 + track_h as f32 / 2.0;
    buf.fill_circle(thumb_cx, thumb_cy, thumb_r, SLIDER_FILL_COLOR);
    buf.fill_circle(thumb_cx, thumb_cy, thumb_r * 0.4, (255, 255, 255));
}

/// Draw a single image row (shared between trusted and custom).
fn draw_image_row(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    x: usize,
    y: usize,
    area_w: usize,
    row_h: usize,
    sf: f32,
    icon_sz: u32,
    name: &str,
    oci_ref: &str,
    last_pulled: Option<&str>,
    label_metrics: Metrics,
    desc_metrics: Metrics,
    small_metrics: Metrics,
) {
    let clip_h = buf.height;
    let icon_y = y + (row_h as f32 / 2.0 - icon_sz as f32 / 2.0) as usize;
    icon_renderer.draw(buf, Icon::CodeSandbox, x, icon_y, icon_sz, theme::FG_MUTED);

    let name_x = x + icon_sz as usize + (8.0 * sf) as usize;
    draw_text_at(
        buf, font_system, swash_cache,
        name_x, y + (row_h as f32 / 2.0 - 9.0 * sf) as usize, clip_h,
        name, label_metrics,
        theme::SETTINGS_BODY_TEXT, Family::Monospace,
    );

    let val_x = x + area_w / 2;
    draw_text_at(
        buf, font_system, swash_cache,
        val_x, y + (row_h as f32 / 2.0 - 9.0 * sf) as usize, clip_h,
        oci_ref, desc_metrics,
        theme::FG_MUTED, Family::Monospace,
    );

    if let Some(pulled) = last_pulled {
        let tag_text = format!("pulled: {}", pulled);
        draw_text_at(
            buf, font_system, swash_cache,
            name_x, y + (row_h as f32 / 2.0 + 5.0 * sf) as usize, clip_h,
            &tag_text, small_metrics,
            theme::FG_MUTED, Family::Monospace,
        );
    }
}
