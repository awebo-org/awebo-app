//! Hint banner — sticky informational panel rendered above the smart input.
//!
//! Two variants:
//! - **Welcome** — shown on first terminal open with keyboard shortcuts.
//! - **AgentMode** — shown when the user enters `/agent` mode.
//!
//! The banner is dismissable ("Don't show again") and positioned between
//! the block view content area and the prompt bar.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::{Icon, IconRenderer};
use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{draw_text_at, draw_text_at_bold, measure_text_width};
use crate::renderer::theme;

use super::overlay::fill_rounded_rect;

const BANNER_PAD_X: f32 = 16.0;
const BANNER_PAD_Y: f32 = 10.0;
const BANNER_HINT_GAP_Y: f32 = 6.0;
const BANNER_ICON_SIZE: f32 = 18.0;
const BANNER_TITLE_GAP: f32 = 8.0;

const BANNER_BG: Rgb = theme::BG;
const BANNER_SEPARATOR: Rgb = theme::BORDER;
const BANNER_TITLE: Rgb = theme::FG_BRIGHT;
const BANNER_HINT_TEXT: Rgb = theme::FG_SECONDARY;
const BANNER_KEY_BG: Rgb = (30, 30, 36);
const BANNER_KEY_TEXT: Rgb = theme::FG_DIM;
const BANNER_DISMISS_TEXT: Rgb = theme::FG_MUTED;
const BANNER_DISMISS_HOVER: Rgb = theme::FG_PRIMARY;


/// Which banner variant to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintBannerKind {
    Welcome,
    AgentMode,
}

/// Persistent state for the hint banner, owned by `App`.
pub struct HintBannerState {
    /// Currently displayed banner kind (if any).
    pub kind: Option<HintBannerKind>,
    /// Whether the dismiss button is hovered.
    pub dismiss_hovered: bool,
    /// Remembered dismissal for Welcome banner (user clicked "Don't show again").
    pub welcome_dismissed: bool,
}

impl Default for HintBannerState {
    fn default() -> Self {
        Self {
            kind: Some(HintBannerKind::Welcome),
            dismiss_hovered: false,
            welcome_dismissed: false,
        }
    }
}

impl HintBannerState {

    /// Dismiss the currently visible banner.
    pub fn dismiss(&mut self) {
        if self.kind == Some(HintBannerKind::Welcome) {
            self.welcome_dismissed = true;
        }
        self.kind = None;
        self.dismiss_hovered = false;
    }

    /// Show the agent-mode banner.
    pub fn show_agent_mode(&mut self) {
        self.kind = Some(HintBannerKind::AgentMode);
        self.dismiss_hovered = false;
    }

    /// Restore to welcome (if not permanently dismissed) or hide.
    pub fn exit_agent_mode(&mut self) {
        if !self.welcome_dismissed {
            self.kind = Some(HintBannerKind::Welcome);
        } else {
            self.kind = None;
        }
        self.dismiss_hovered = false;
    }


    pub fn is_visible(&self) -> bool {
        self.kind.is_some()
    }
}


struct HintLine {
    keys: &'static str,
    description: &'static str,
}

fn welcome_hints() -> &'static [HintLine] {
    &[
        HintLine { keys: "/agent <task>", description: "start an agent conversation" },
        HintLine { keys: "/ask <question>", description: "ask AI about your terminal" },
        HintLine { keys: "↑ ↓", description: "cycle past commands" },
        HintLine { keys: "/help", description: "show all available commands" },
    ]
}

fn agent_mode_hints() -> &'static [HintLine] {
    &[
        HintLine { keys: "Enter", description: "send task to agent" },
        HintLine { keys: "/close", description: "exit agent mode" },
        HintLine { keys: "Esc", description: "cancel running inference" },
    ]
}

fn banner_title(kind: HintBannerKind) -> &'static str {
    match kind {
        HintBannerKind::Welcome => "New terminal session",
        HintBannerKind::AgentMode => "Agent mode",
    }
}

fn banner_hints(kind: HintBannerKind) -> &'static [HintLine] {
    match kind {
        HintBannerKind::Welcome => welcome_hints(),
        HintBannerKind::AgentMode => agent_mode_hints(),
    }
}


/// Total height of the banner in physical pixels.
/// Returns 0 if the banner is not visible.
pub fn banner_height(state: &HintBannerState, sf: f32) -> usize {
    let kind = match state.kind {
        Some(k) => k,
        None => return 0,
    };
    let hints = banner_hints(kind);
    let sep_h = (1.0 * sf).max(1.0) as usize;
    let pad_y = (BANNER_PAD_Y * sf) as usize;
    let title_h = (20.0 * sf) as usize;
    let title_gap = (BANNER_TITLE_GAP * sf) as usize;
    let line_h = (18.0 * sf) as usize;
    let hint_gap = (BANNER_HINT_GAP_Y * sf) as usize;
    let hints_total = if hints.is_empty() {
        0
    } else {
        hints.len() * line_h + (hints.len() - 1) * hint_gap
    };
    sep_h + pad_y + title_h + title_gap + hints_total + pad_y
}



/// Check if mouse position is over the dismiss button area.
pub fn hit_test_dismiss(
    mx: f64,
    my: f64,
    state: &HintBannerState,
    banner_y: usize,
    banner_w: usize,
    sf: f32,
) -> bool {
    if state.kind.is_none() {
        return false;
    }
    let h = banner_height(state, sf);
    if h == 0 {
        return false;
    }
    let pad_x = (BANNER_PAD_X * sf) as usize;
    let pad_y = (BANNER_PAD_Y * sf) as usize;
    let sep_h = (1.0 * sf).max(1.0) as usize;
    let dismiss_w = (120.0 * sf) as usize;
    let title_h = (20.0 * sf) as usize;
    let dismiss_x = banner_w.saturating_sub(pad_x + dismiss_w);
    let dismiss_row_y = banner_y + sep_h + pad_y;

    let px = mx as usize;
    let py = my as usize;
    px >= dismiss_x && px < dismiss_x + dismiss_w
        && py >= dismiss_row_y && py < dismiss_row_y + title_h
}


/// Draw the hint banner. Call this from the renderer between block view and prompt bar.
/// `x_edge` is the left edge for the full-width separator (accounts for sidebar).
pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &HintBannerState,
    x_start: usize,
    y_start: usize,
    max_w: usize,
    x_edge: usize,
    full_width: usize,
    sf: f32,
) {
    let kind = match state.kind {
        Some(k) => k,
        None => return,
    };

    let hints = banner_hints(kind);
    let title = banner_title(kind);

    let pad_x = (BANNER_PAD_X * sf) as usize;
    let pad_y = (BANNER_PAD_Y * sf) as usize;
    let icon_sz = (BANNER_ICON_SIZE * sf) as u32;
    let title_gap = (BANNER_TITLE_GAP * sf) as usize;
    let hint_gap = (BANNER_HINT_GAP_Y * sf) as usize;
    let total_h = banner_height(state, sf);

    let edge_w = full_width.saturating_sub(x_edge);
    buf.fill_rect(x_edge, y_start, edge_w, total_h, BANNER_BG);

    let sep_h = (1.0 * sf).max(1.0) as usize;
    buf.fill_rect(x_edge, y_start, edge_w, sep_h, BANNER_SEPARATOR);

    let title_font_size = 14.0 * sf;
    let title_line_height = 20.0 * sf;
    let title_metrics = Metrics::new(title_font_size, title_line_height);

    let row_y = y_start + sep_h + pad_y;
    let icon_x = x_start + pad_x;
    let icon_y = row_y + ((title_line_height - icon_sz as f32) / 2.0).max(0.0) as usize;
    icon_renderer.draw(buf, Icon::Awebo, icon_x, icon_y, icon_sz, theme::FG_PRIMARY);

    let title_x = icon_x + icon_sz as usize + (8.0 * sf) as usize;
    draw_text_at_bold(
        buf, font_system, swash_cache,
        title_x, row_y, buf.height,
        title, title_metrics, BANNER_TITLE, Family::SansSerif,
    );

    let dismiss_text = "Don't show again";
    let dismiss_font_size = 11.0 * sf;
    let dismiss_line_height = 16.0 * sf;
    let dismiss_metrics = Metrics::new(dismiss_font_size, dismiss_line_height);
    let dismiss_text_w = measure_text_width(font_system, dismiss_text, dismiss_metrics, Family::SansSerif) as usize;
    let dismiss_pad = (8.0 * sf) as usize;
    let dismiss_btn_w = dismiss_text_w + dismiss_pad * 2;
    let dismiss_btn_h = (22.0 * sf) as usize;
    let dismiss_btn_x = (x_start + max_w).saturating_sub(pad_x + dismiss_btn_w);
    let dismiss_btn_y = row_y + ((title_line_height - dismiss_btn_h as f32) / 2.0).max(0.0) as usize;
    let dismiss_r = (4.0 * sf) as usize;

    if state.dismiss_hovered {
        fill_rounded_rect(buf, dismiss_btn_x, dismiss_btn_y, dismiss_btn_w, dismiss_btn_h, dismiss_r, theme::BG_HOVER);
    }
    let dismiss_color = if state.dismiss_hovered { BANNER_DISMISS_HOVER } else { BANNER_DISMISS_TEXT };
    let dismiss_text_y = dismiss_btn_y + ((dismiss_btn_h as f32 - dismiss_line_height) / 2.0) as usize;
    draw_text_at(
        buf, font_system, swash_cache,
        dismiss_btn_x + dismiss_pad, dismiss_text_y, buf.height,
        dismiss_text, dismiss_metrics, dismiss_color, Family::SansSerif,
    );

    let hint_font_size = 12.0 * sf;
    let hint_line_height = 18.0 * sf;
    let hint_metrics = Metrics::new(hint_font_size, hint_line_height);
    let key_font_size = 11.0 * sf;
    let key_line_height = 16.0 * sf;
    let key_metrics = Metrics::new(key_font_size, key_line_height);

    let title_h = title_line_height as usize;
    let hints_start_y = row_y + title_h + title_gap;
    let line_h = (18.0 * sf) as usize;

    for (i, hint) in hints.iter().enumerate() {
        let ly = hints_start_y + i * (line_h + hint_gap);
        let lx = x_start + pad_x;

        let key_w = measure_text_width(font_system, hint.keys, key_metrics, Family::Monospace) as usize;
        let badge_pad = (4.0 * sf) as usize;
        let badge_h = (key_line_height + 4.0 * sf) as usize;
        let badge_y = ly + ((line_h as f32 - badge_h as f32) / 2.0).max(0.0) as usize;
        let badge_r = (3.0 * sf) as usize;
        fill_rounded_rect(buf, lx, badge_y, key_w + badge_pad * 2, badge_h, badge_r, BANNER_KEY_BG);
        let key_text_y = badge_y + ((badge_h as f32 - key_line_height) / 2.0) as usize;
        draw_text_at(
            buf, font_system, swash_cache,
            lx + badge_pad, key_text_y, buf.height,
            hint.keys, key_metrics, BANNER_KEY_TEXT, Family::Monospace,
        );

        let desc_x = lx + key_w + badge_pad * 2 + (10.0 * sf) as usize;
        let desc_y = ly + ((line_h as f32 - hint_line_height) / 2.0) as usize;
        draw_text_at(
            buf, font_system, swash_cache,
            desc_x, desc_y, buf.height,
            hint.description, hint_metrics, BANNER_HINT_TEXT, Family::SansSerif,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_shows_welcome() {
        let state = HintBannerState::default();
        assert_eq!(state.kind, Some(HintBannerKind::Welcome));
        assert!(!state.dismiss_hovered);
        assert!(!state.welcome_dismissed);
        assert!(state.is_visible());
    }

    #[test]
    fn dismiss_hides_banner() {
        let mut state = HintBannerState::default();
        state.dismiss();
        assert!(!state.is_visible());
        assert!(state.welcome_dismissed);
    }

    #[test]
    fn agent_mode_banner() {
        let mut state = HintBannerState::default();
        state.show_agent_mode();
        assert_eq!(state.kind, Some(HintBannerKind::AgentMode));
    }

    #[test]
    fn exit_agent_restores_welcome_if_not_dismissed() {
        let mut state = HintBannerState::default();
        state.show_agent_mode();
        state.exit_agent_mode();
        assert_eq!(state.kind, Some(HintBannerKind::Welcome));
    }

    #[test]
    fn exit_agent_hides_if_welcome_dismissed() {
        let mut state = HintBannerState::default();
        state.dismiss();
        state.show_agent_mode();
        state.exit_agent_mode();
        assert!(!state.is_visible());
    }

    #[test]
    fn banner_height_zero_when_hidden() {
        let state = HintBannerState { kind: None, dismiss_hovered: false, welcome_dismissed: true };
        assert_eq!(banner_height(&state, 2.0), 0);
    }

    #[test]
    fn banner_height_positive_when_visible() {
        let state = HintBannerState::default();
        assert!(banner_height(&state, 2.0) > 0);
    }

    #[test]
    fn banner_height_scales() {
        let state = HintBannerState::default();
        let h1 = banner_height(&state, 1.0);
        let h2 = banner_height(&state, 2.0);
        assert!(h2 > h1);
    }
}
