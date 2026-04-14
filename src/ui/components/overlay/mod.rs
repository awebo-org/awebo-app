//! Overlay rendering: command palette, debug panel, shell picker, and settings.
//!
//! Each overlay component lives in its own sub-module, following single-responsibility:
//! - `palette`      — command palette overlay
//! - `debug_panel`  — debug info panel
//! - `shell_picker` — shell picker dropdown
//! - `settings/`    — full-screen settings panel (sidebar, appearance, about, font picker)

mod confirm_close;
mod debug_panel;
mod model_picker;
pub(crate) mod models_view;
mod palette;
pub(crate) mod pro_panel;
pub mod settings;
mod shell_picker;
pub(crate) mod usage_panel;

use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};

pub use confirm_close::{
    ConfirmCloseHit, confirm_close_hit_test, confirm_close_hover_test, draw_confirm_close,
};
pub use debug_panel::draw_debug;
pub use model_picker::{ModelPickerItem, ModelPickerState, ModelStatus, draw_model_picker};
pub use palette::{PaletteState, draw_palette};
pub use settings::{
    AiModelsHit, InputType, SandboxSettingsState, SettingsCategory, SettingsState,
    detect_monospace_fonts, draw_font_picker, draw_settings, font_picker_hit_test,
    settings_ai_models_hit_test, settings_panel_contains, settings_panel_rect,
    settings_sidebar_hit_test,
};
pub use shell_picker::{
    SandboxImageInfo, ShellInfo, ShellPickerChoice, ShellPickerState, draw_shell_picker,
    shell_picker_hit_test, shell_picker_hover_test,
};

pub(crate) fn draw_border(
    buf: &mut PixelBuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    bw: usize,
    color: Rgb,
) {
    buf.fill_rect(x, y, w, bw, color);
    buf.fill_rect(x, y + h.saturating_sub(bw), w, bw, color);
    buf.fill_rect(x, y, bw, h, color);
    buf.fill_rect(x + w.saturating_sub(bw), y, bw, h, color);
}

pub(crate) fn fill_rounded_rect(
    buf: &mut PixelBuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    r: usize,
    color: Rgb,
) {
    if r == 0 || w <= r * 2 || h <= r * 2 {
        buf.fill_rect(x, y, w, h, color);
        return;
    }

    buf.fill_rect(x, y + r, w, h - r * 2, color);
    buf.fill_rect(x + r, y, w - r * 2, r, color);
    buf.fill_rect(x + r, y + h - r, w - r * 2, r, color);

    for dy in 0..r {
        let rf = r as f32;
        let inset = rf - (rf * rf - (rf - dy as f32 - 0.5).powi(2)).sqrt().max(0.0);
        let inset = inset.ceil() as usize;

        buf.fill_rect(x + inset, y + dy, r - inset, 1, color);
        buf.fill_rect(x + w - r, y + dy, r - inset, 1, color);
        buf.fill_rect(x + inset, y + h - 1 - dy, r - inset, 1, color);
        buf.fill_rect(x + w - r, y + h - 1 - dy, r - inset, 1, color);
    }
}

pub(crate) fn draw_border_rounded(
    buf: &mut PixelBuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    bw: usize,
    corner_r: usize,
    color: Rgb,
) {
    if corner_r == 0 || w <= corner_r * 2 || h <= corner_r * 2 {
        draw_border(buf, x, y, w, h, bw, color);
        return;
    }
    buf.fill_rect(x + corner_r, y, w - corner_r * 2, bw, color);
    buf.fill_rect(x + corner_r, y + h - bw, w - corner_r * 2, bw, color);
    buf.fill_rect(x, y + corner_r, bw, h - corner_r * 2, color);
    buf.fill_rect(x + w - bw, y + corner_r, bw, h - corner_r * 2, color);
    for dy in 0..corner_r {
        let rf = corner_r as f32;
        let outer = rf - (rf * rf - (rf - dy as f32 - 0.5).powi(2)).sqrt().max(0.0);
        let inner = rf
            - ((rf - bw as f32).max(0.0).powi(2) - (rf - dy as f32 - 0.5).powi(2))
                .sqrt()
                .max(0.0);
        let o = outer.ceil() as usize;
        let i = inner.ceil() as usize;
        let span = i.saturating_sub(o).max(bw);
        buf.fill_rect(x + o, y + dy, span, 1, color);
        buf.fill_rect(x + w - o - span, y + dy, span, 1, color);
        buf.fill_rect(x + o, y + h - 1 - dy, span, 1, color);
        buf.fill_rect(x + w - o - span, y + h - 1 - dy, span, 1, color);
    }
}
