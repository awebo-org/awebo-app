//! Settings panel — sidebar, state, and category dispatch.

pub mod about;
pub mod font_picker;
pub mod sandbox_settings;

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::{draw_border_rounded, fill_rounded_rect};

pub use ai_models::{AiModelsHit, settings_ai_models_hit_test};
pub use font_picker::{detect_monospace_fonts, draw_font_picker, font_picker_hit_test};

/// Settings panel category tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    AiModels,
    Sandbox,
    About,
}

impl SettingsCategory {
    pub fn all() -> &'static [SettingsCategory] {
        &[
            SettingsCategory::AiModels,
            SettingsCategory::Sandbox,
            SettingsCategory::About,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            SettingsCategory::AiModels => "Local Models",
            SettingsCategory::Sandbox => "Sandbox",
            SettingsCategory::About => "About",
        }
    }
}

/// Prompt input mode: Smart (block-based with AI) or ShellPS1 (raw PTY).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Smart,
    ShellPS1,
}

/// Persistent state for the settings panel UI.
pub struct SettingsState {
    pub active: SettingsCategory,
    pub hovered: Option<usize>,
    pub hovered_btn: Option<ai_models::AiModelsHit>,
    pub input_type: InputType,
    pub font_family: String,
    pub font_size_px: f32,
    pub line_height_px: f32,
    pub font_picker_open: bool,
    pub font_picker_hovered: Option<usize>,
    pub font_options: Vec<String>,
    pub models_path: String,
    /// Whether to enrich AI hints with web search results (default: off).
    pub web_search_enabled: bool,
    /// Registry index of a model currently being deleted (shows spinner).
    pub deleting_model: Option<usize>,
    /// Sandbox settings sub-state.
    pub sandbox: SandboxSettingsState,
    /// Whether "Reset hints" button in About tab is hovered.
    pub about_hovered: Option<about::AboutHit>,
}

/// UI state for the sandbox settings tab.
pub struct SandboxSettingsState {
    /// System-detected vCPU count (upper bound for slider).
    pub system_cpus: u32,
    /// System-detected total memory in MiB (upper bound for slider).
    pub system_memory_mib: u32,
    /// Current slider value for default vCPUs.
    pub cpus: u32,
    /// Current slider value for default memory in MiB.
    pub memory_mib: u32,
    /// Which slider is being dragged (if any).
    pub dragging_slider: Option<SandboxSlider>,
    /// Currently hovered interactive element.
    pub hovered_hit: Option<SandboxSettingsHit>,
    /// Text buffer for adding a new custom image OCI ref.
    pub add_image_input: String,
    /// Whether the add-image text input is focused.
    pub add_image_focused: bool,
    /// Scroll offset for sandbox settings content (pixels).
    pub scroll_offset: f32,
    /// Whether the scrollbar thumb is being dragged.
    pub dragging_scrollbar: bool,
    /// Mouse Y at the start of a scrollbar drag (physical pixels).
    pub scrollbar_drag_anchor_y: f32,
    /// scroll_offset at the start of a scrollbar drag.
    pub scrollbar_drag_anchor_offset: f32,
}

/// Interactive elements in the sandbox settings tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxSettingsHit {
    /// Delete a trusted image at the given built-in index.
    DeleteTrustedImage(usize),
    /// Update (re-pull) a trusted image at the given built-in index.
    UpdateTrustedImage(usize),
    /// Delete the custom image at the given config index.
    DeleteCustomImage(usize),
    /// Delete the volume at the given config index.
    DeleteVolume(usize),
    /// The "Add Image" button (submits `add_image_input`).
    AddImage,
    /// The add-image text input field.
    AddImageInput,
}

/// Which slider the user is currently dragging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxSlider {
    Cpu,
    Memory,
}

impl Default for SandboxSettingsState {
    fn default() -> Self {
        let sys_cpus = crate::system_info::cpu_count();
        let sys_mem = crate::system_info::total_memory_mib();
        Self {
            system_cpus: sys_cpus,
            system_memory_mib: sys_mem,
            cpus: 1,
            memory_mib: 512,
            dragging_slider: None,
            hovered_hit: None,
            add_image_input: String::new(),
            add_image_focused: false,
            scroll_offset: 0.0,
            dragging_scrollbar: false,
            scrollbar_drag_anchor_y: 0.0,
            scrollbar_drag_anchor_offset: 0.0,
        }
    }
}

impl SettingsState {
    pub fn new(font_options: Vec<String>) -> Self {
        let models_path = crate::ai::model_manager::models_dir()
            .to_string_lossy()
            .into_owned();
        Self {
            active: SettingsCategory::AiModels,
            hovered: None,
            hovered_btn: None,
            input_type: InputType::Smart,
            font_family: "JetBrains Mono".to_string(),
            font_size_px: 16.0,
            line_height_px: 22.0,
            font_picker_open: false,
            font_picker_hovered: None,
            font_options,
            models_path,
            web_search_enabled: false,
            deleting_model: None,
            sandbox: SandboxSettingsState::default(),
            about_hovered: None,
        }
    }
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::new(vec!["Monospace".to_string()])
    }
}

pub mod ai_models;

const PANEL_W: f32 = 680.0;
const PANEL_MAX_H: f32 = 520.0;
const PANEL_CORNER_R: f32 = 10.0;
const SIDEBAR_LOGICAL_WIDTH: f32 = 180.0;
const ITEM_LOGICAL_HEIGHT: f32 = 36.0;
const SIDEBAR_TOP_PAD: f32 = 16.0;
const SIDEBAR_ITEM_PAD: f32 = 12.0;
const PANEL_BG: crate::renderer::pixel_buffer::Rgb = theme::BG_SURFACE;
const PANEL_BORDER: crate::renderer::pixel_buffer::Rgb = theme::BORDER;

/// Compute the centered settings panel rect (px, py, pw, ph) in physical pixels.
pub fn settings_panel_rect(buf_w: usize, buf_h: usize, sf: f32) -> (usize, usize, usize, usize) {
    let pw = (PANEL_W * sf) as usize;
    let max_ph = (PANEL_MAX_H * sf) as usize;
    let ph = max_ph.min(buf_h * 80 / 100);
    let px = buf_w.saturating_sub(pw) / 2;
    let py = buf_h.saturating_sub(ph) / 2;
    (px, py, pw, ph)
}

pub fn draw_settings(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut crate::renderer::icons::IconRenderer,
    avatar_renderer: &mut crate::renderer::icons::AvatarRenderer,
    state: &SettingsState,
    sf: f32,
    is_pro: bool,
) {
    buf.dim(0.45);

    let (px, py, pw, ph) = settings_panel_rect(buf.width, buf.height, sf);
    let corner = (PANEL_CORNER_R * sf) as usize;
    let border_w = (1.0_f32 * sf).max(1.0) as usize;

    fill_rounded_rect(buf, px, py, pw, ph, corner, PANEL_BG);
    draw_border_rounded(buf, px, py, pw, ph, border_w, corner, PANEL_BORDER);

    let sidebar_w = (SIDEBAR_LOGICAL_WIDTH * sf) as usize;
    let item_h = (ITEM_LOGICAL_HEIGHT * sf) as usize;
    let top_pad = (SIDEBAR_TOP_PAD * sf) as usize;
    let item_pad_x = (SIDEBAR_ITEM_PAD * sf) as usize;

    let divider_x = px + sidebar_w;
    buf.fill_rect(
        divider_x,
        py + corner,
        border_w,
        ph.saturating_sub(corner * 2),
        PANEL_BORDER,
    );

    let label_metrics = Metrics::new(13.0 * sf, 18.0 * sf);
    let categories = SettingsCategory::all();
    let corner_r = (4.0 * sf) as usize;

    for (i, cat) in categories.iter().enumerate() {
        let iy = py + top_pad + i * item_h;
        let is_active = *cat == state.active;
        let is_hovered = state.hovered == Some(i);

        if is_active {
            let rect_x = px + item_pad_x / 2;
            let rect_w = sidebar_w - item_pad_x;
            fill_rounded_rect(
                buf,
                rect_x,
                iy,
                rect_w,
                item_h,
                corner_r,
                theme::SETTINGS_SIDEBAR_ACTIVE_BG,
            );
        } else if is_hovered {
            let rect_x = px + item_pad_x / 2;
            let rect_w = sidebar_w - item_pad_x;
            fill_rounded_rect(
                buf,
                rect_x,
                iy,
                rect_w,
                item_h,
                corner_r,
                theme::SETTINGS_SIDEBAR_HOVER_BG,
            );
        }

        let text_color = if is_active {
            theme::SETTINGS_SIDEBAR_ACTIVE_TEXT
        } else {
            theme::SETTINGS_SIDEBAR_TEXT
        };

        let text_y = iy + ((item_h as f32 - 18.0 * sf) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            px + item_pad_x + (4.0 * sf) as usize,
            text_y,
            py + ph,
            cat.label(),
            label_metrics,
            text_color,
            Family::Monospace,
        );
    }

    let content_x = px + sidebar_w + border_w;
    let content_w = pw.saturating_sub(sidebar_w + border_w);
    let body_y = py + (16.0 * sf) as usize;
    let clip_h = py + ph;

    match state.active {
        SettingsCategory::AiModels => {
            ai_models::draw_settings_ai_models(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                state,
                content_x,
                body_y,
                content_w,
                clip_h,
                sf,
            );
        }
        SettingsCategory::Sandbox => {
            sandbox_settings::draw_settings_sandbox(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                state,
                content_x,
                body_y,
                content_w,
                clip_h,
                sf,
            );
        }
        SettingsCategory::About => {
            about::draw_settings_about(
                buf,
                font_system,
                swash_cache,
                avatar_renderer,
                content_x,
                body_y,
                content_w,
                clip_h,
                sf,
                state.about_hovered,
                is_pro,
            );
        }
    }
}

/// Hit-test the settings sidebar. Returns category index if hit.
pub fn settings_sidebar_hit_test(
    phys_x: f64,
    phys_y: f64,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> Option<usize> {
    let (px, py, _pw, ph) = settings_panel_rect(buf_w, buf_h, sf);
    let sidebar_w = (SIDEBAR_LOGICAL_WIDTH * sf) as f64;

    if phys_x < px as f64
        || phys_x >= px as f64 + sidebar_w
        || phys_y < py as f64
        || phys_y >= (py + ph) as f64
    {
        return None;
    }

    let item_h = (ITEM_LOGICAL_HEIGHT * sf) as f64;
    let top_pad = (SIDEBAR_TOP_PAD * sf) as f64;
    let rel_y = phys_y - py as f64 - top_pad;

    if rel_y < 0.0 {
        return None;
    }

    let idx = (rel_y / item_h) as usize;
    let count = SettingsCategory::all().len();
    if idx < count { Some(idx) } else { None }
}

/// Check whether a click is inside the settings panel area.
pub fn settings_panel_contains(
    phys_x: f64,
    phys_y: f64,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> bool {
    let (px, py, pw, ph) = settings_panel_rect(buf_w, buf_h, sf);
    phys_x >= px as f64
        && phys_x < (px + pw) as f64
        && phys_y >= py as f64
        && phys_y < (py + ph) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_category_all_returns_three() {
        assert_eq!(SettingsCategory::all().len(), 3);
    }

    #[test]
    fn settings_category_labels() {
        assert_eq!(SettingsCategory::AiModels.label(), "Local Models");
        assert_eq!(SettingsCategory::Sandbox.label(), "Sandbox");
        assert_eq!(SettingsCategory::About.label(), "About");
    }

    #[test]
    fn settings_state_default() {
        let s = SettingsState::default();
        assert_eq!(s.active, SettingsCategory::AiModels);
        assert!(!s.web_search_enabled);
        assert!(!s.font_picker_open);
    }

    #[test]
    fn settings_sidebar_hit_test_outside() {
        assert!(settings_sidebar_hit_test(0.0, 0.0, 1000, 800, 1.0).is_none());
    }

    #[test]
    fn settings_sidebar_hit_test_first_item() {
        let (px, py, _, _) = settings_panel_rect(1000, 800, 1.0);
        let result = settings_sidebar_hit_test(px as f64 + 50.0, py as f64 + 20.0, 1000, 800, 1.0);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn settings_panel_rect_centered() {
        let (px, py, pw, ph) = settings_panel_rect(1000, 800, 1.0);
        assert!(pw > 0);
        assert!(ph > 0);
        assert_eq!(px, (1000 - pw) / 2);
        assert_eq!(py, (800 - ph) / 2);
    }

    #[test]
    fn settings_panel_contains_inside() {
        let (px, py, pw, ph) = settings_panel_rect(1000, 800, 1.0);
        assert!(settings_panel_contains(
            (px + pw / 2) as f64,
            (py + ph / 2) as f64,
            1000,
            800,
            1.0
        ));
    }

    #[test]
    fn settings_panel_contains_outside() {
        assert!(!settings_panel_contains(0.0, 0.0, 1000, 800, 1.0));
    }
}
