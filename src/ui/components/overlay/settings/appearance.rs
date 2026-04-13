//! Settings appearance tab — draw + hit-testing + widget helpers.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::{InputType, SettingsState};

pub fn draw_settings_appearance(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
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
    let radio_metrics = Metrics::new(13.0 * sf, 18.0 * sf);
    let row_h = (40.0 * sf) as usize;
    let mut y = area_y;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y,
        clip_h,
        "Input",
        section_metrics,
        theme::SETTINGS_SECTION_TITLE,
        Family::Monospace,
    );
    y += (32.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y + (row_h as f32 / 2.0 - 9.0 * sf) as usize,
        clip_h,
        "Input type",
        label_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );

    let radio_area_x = area_x + area_w - pad - (260.0 * sf) as usize;
    draw_radio(
        buf,
        font_system,
        swash_cache,
        radio_area_x,
        y,
        row_h,
        sf,
        radio_metrics,
        "Smart",
        state.input_type == InputType::Smart,
    );
    draw_radio(
        buf,
        font_system,
        swash_cache,
        radio_area_x + (130.0 * sf) as usize,
        y,
        row_h,
        sf,
        radio_metrics,
        "Shell (PS1)",
        state.input_type == InputType::ShellPS1,
    );
    y += row_h + (24.0 * sf) as usize;

    let line_y = y;
    let line_pad = (20.0 * sf) as usize;
    buf.fill_rect(
        area_x + line_pad,
        line_y,
        area_w.saturating_sub(line_pad * 2),
        (1.0 * sf).max(1.0) as usize,
        theme::SETTINGS_DIVIDER,
    );
    y += (24.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y,
        clip_h,
        "Text",
        section_metrics,
        theme::SETTINGS_SECTION_TITLE,
        Family::Monospace,
    );
    y += (32.0 * sf) as usize;

    let input_w = (180.0 * sf) as usize;
    let input_h = (30.0 * sf) as usize;
    let input_x = area_x + area_w - pad - input_w;
    let bw = (1.0 * sf).max(1.0) as usize;
    let corner_r = (4.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y + (row_h as f32 / 2.0 - 9.0 * sf) as usize,
        clip_h,
        "Terminal font",
        label_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );
    draw_input_field(
        buf,
        font_system,
        swash_cache,
        input_x,
        y + (row_h - input_h) / 2,
        input_w,
        input_h,
        bw,
        corner_r,
        sf,
        &state.font_family,
        clip_h,
    );
    y += row_h;

    let stepper_w = (120.0 * sf) as usize;
    let stepper_h = (28.0 * sf) as usize;
    let stepper_x = area_x + area_w - pad - stepper_w;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y + (row_h as f32 / 2.0 - 9.0 * sf) as usize,
        clip_h,
        "Font size",
        label_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );
    let size_str = format!("{:.0}", state.font_size_px);
    draw_stepper(
        buf,
        font_system,
        swash_cache,
        stepper_x,
        y + (row_h - stepper_h) / 2,
        stepper_w,
        stepper_h,
        sf,
        &size_str,
    );
    y += row_h;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y + (row_h as f32 / 2.0 - 9.0 * sf) as usize,
        clip_h,
        "Line height",
        label_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );
    let lh_str = format!("{:.0}", state.line_height_px);
    draw_stepper(
        buf,
        font_system,
        swash_cache,
        stepper_x,
        y + (row_h - stepper_h) / 2,
        stepper_w,
        stepper_h,
        sf,
        &lh_str,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceHit {
    InputTypeSmart,
    InputTypeShellPS1,
    FontPickerToggle,
    FontSizeInc,
    FontSizeDec,
    LineHeightInc,
    LineHeightDec,
}

pub fn settings_appearance_hit_test(
    phys_x: f64,
    phys_y: f64,
    y_offset: usize,
    content_area_x: usize,
    content_area_w: usize,
    sf: f32,
) -> Option<AppearanceHit> {
    let pad = (24.0 * sf) as f64;
    let row_h = (40.0 * sf) as f64;
    let header_y = y_offset as f64 + (8.0 * sf) as f64;
    let divider_y = header_y + (26.0 * sf) as f64;
    let base_y = divider_y + (20.0 * sf) as f64;

    let input_row_y = base_y + (32.0 * sf) as f64;
    let radio_area_x = content_area_x as f64 + content_area_w as f64 - pad - (260.0 * sf) as f64;

    if phys_y >= input_row_y && phys_y < input_row_y + row_h {
        if phys_x >= radio_area_x && phys_x < radio_area_x + (120.0 * sf) as f64 {
            return Some(AppearanceHit::InputTypeSmart);
        }
        let ps1_x = radio_area_x + (130.0 * sf) as f64;
        if phys_x >= ps1_x && phys_x < ps1_x + (130.0 * sf) as f64 {
            return Some(AppearanceHit::InputTypeShellPS1);
        }
    }

    let text_base =
        input_row_y + row_h + (24.0 * sf) as f64 + (24.0 * sf) as f64 + (32.0 * sf) as f64;
    let input_w = (180.0 * sf) as f64;
    let input_x = content_area_x as f64 + content_area_w as f64 - pad - input_w;

    if phys_y >= text_base
        && phys_y < text_base + row_h
        && phys_x >= input_x
        && phys_x < input_x + input_w
    {
        return Some(AppearanceHit::FontPickerToggle);
    }

    let stepper_w = (120.0 * sf) as f64;
    let btn_w = (32.0 * sf) as f64;
    let stepper_x = content_area_x as f64 + content_area_w as f64 - pad - stepper_w;

    let size_row_y = text_base + row_h;
    if phys_y >= size_row_y && phys_y < size_row_y + row_h {
        if phys_x >= stepper_x && phys_x < stepper_x + btn_w {
            return Some(AppearanceHit::FontSizeDec);
        }
        if phys_x >= stepper_x + stepper_w - btn_w && phys_x < stepper_x + stepper_w {
            return Some(AppearanceHit::FontSizeInc);
        }
    }

    let lh_row_y = size_row_y + row_h;
    if phys_y >= lh_row_y && phys_y < lh_row_y + row_h {
        if phys_x >= stepper_x && phys_x < stepper_x + btn_w {
            return Some(AppearanceHit::LineHeightDec);
        }
        if phys_x >= stepper_x + stepper_w - btn_w && phys_x < stepper_x + stepper_w {
            return Some(AppearanceHit::LineHeightInc);
        }
    }

    None
}


fn draw_radio(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x: usize,
    y: usize,
    row_h: usize,
    sf: f32,
    metrics: Metrics,
    label: &str,
    active: bool,
) {
    let r = 7.0 * sf;
    let cx = x as f32 + r;
    let cy = y as f32 + row_h as f32 / 2.0;

    if active {
        buf.stroke_circle(cx, cy, r, (1.5 * sf).max(1.0), theme::SETTINGS_RADIO_ACTIVE);
        buf.fill_circle(cx, cy, 4.0 * sf, theme::SETTINGS_RADIO_DOT);
    } else {
        buf.stroke_circle(cx, cy, r, (1.0 * sf).max(1.0), theme::SETTINGS_RADIO_BORDER);
    }

    let text_x = x + (r * 2.0 + 8.0 * sf) as usize;
    let text_y = y + (row_h as f32 / 2.0 - 9.0 * sf) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        text_x,
        text_y,
        buf.height,
        label,
        metrics,
        theme::SETTINGS_RADIO_TEXT,
        Family::Monospace,
    );
}

fn draw_input_field(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    bw: usize,
    corner_r: usize,
    sf: f32,
    value: &str,
    clip_h: usize,
) {
    crate::ui::components::overlay::fill_rounded_rect(
        buf,
        x,
        y,
        w,
        h,
        corner_r,
        theme::SETTINGS_INPUT_BG,
    );
    let inner_pad = (10.0 * sf) as usize;
    let text_y = y + (h as f32 / 2.0 - 9.0 * sf) as usize;
    let text_metrics = Metrics::new(13.0 * sf, 18.0 * sf);
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        x + inner_pad,
        text_y,
        clip_h,
        value,
        text_metrics,
        theme::SETTINGS_INPUT_TEXT,
        Family::Monospace,
    );
    crate::ui::components::overlay::draw_border_rounded(
        buf,
        x,
        y,
        w,
        h,
        bw,
        corner_r,
        theme::SETTINGS_INPUT_BORDER,
    );
}

fn draw_stepper(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    sf: f32,
    value: &str,
) {
    let corner_r = (6.0 * sf) as usize;
    let bw = (1.0 * sf).max(1.0) as usize;
    let btn_w = (32.0 * sf) as usize;

    crate::ui::components::overlay::fill_rounded_rect(
        buf,
        x,
        y,
        w,
        h,
        corner_r,
        theme::SETTINGS_INPUT_BG,
    );
    crate::ui::components::overlay::draw_border_rounded(
        buf,
        x,
        y,
        w,
        h,
        bw,
        corner_r,
        theme::SETTINGS_INPUT_BORDER,
    );

    buf.fill_rect(x + btn_w, y, bw, h, theme::SETTINGS_INPUT_BORDER);
    buf.fill_rect(x + w - btn_w, y, bw, h, theme::SETTINGS_INPUT_BORDER);

    let sym_metrics = Metrics::new(15.0 * sf, 18.0 * sf);
    let val_metrics = Metrics::new(13.0 * sf, 18.0 * sf);
    let sym_y = y + (h as f32 / 2.0 - 9.0 * sf) as usize;

    let minus_x = x + (btn_w as f32 / 2.0 - 4.0 * sf) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        minus_x,
        sym_y,
        buf.height,
        "\u{2212}",
        sym_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );

    let plus_x = x + w - btn_w + (btn_w as f32 / 2.0 - 4.0 * sf) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        plus_x,
        sym_y,
        buf.height,
        "+",
        sym_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );

    let mid_w = w - btn_w * 2;
    let approx_text_w = (value.len() as f32 * 7.5 * sf) as usize;
    let val_x = x + btn_w + mid_w / 2 - approx_text_w / 2;
    let val_y = y + (h as f32 / 2.0 - 9.0 * sf) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        val_x,
        val_y,
        buf.height,
        value,
        val_metrics,
        theme::SETTINGS_INPUT_TEXT,
        Family::Monospace,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_hit_test_outside_returns_none() {
        assert!(settings_appearance_hit_test(0.0, 0.0, 42, 200, 800, 1.0).is_none());
    }
}
