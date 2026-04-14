//! Tab bar rendering, hit-testing, hover state, and drag-and-drop reordering.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::{AvatarRenderer, Icon, IconRenderer};
use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::{draw_text_at, draw_text_at_bold};
use crate::renderer::theme;

pub const TAB_BAR_LOGICAL_HEIGHT: f32 = 42.0;

const TAB_MAX_WIDTH: f32 = 220.0;
const PLUS_BUTTON_WIDTH: f32 = 36.0;
const SHELL_PICKER_WIDTH: f32 = 32.0;
const BUTTON_GAP: f32 = 4.0;
const BTN_LEFT_MARGIN: f32 = 8.0;
const AVATAR_ICON_SIZE: f32 = 22.0;
const RIGHT_MARGIN: f32 = 12.0;
const RIGHT_GAP: f32 = 6.0;
const SIDEBAR_ICON_SIZE: f32 = 18.0;
const SIDEBAR_ICON_LEFT_MARGIN: f32 = 8.0;
const GIT_PANEL_ICON_SIZE: f32 = 15.0;

#[cfg(target_os = "macos")]
const LEFT_PADDING_MACOS: f32 = 84.0;

pub fn left_padding(is_fullscreen: bool) -> f32 {
    #[cfg(target_os = "macos")]
    {
        if is_fullscreen {
            0.0
        } else {
            LEFT_PADDING_MACOS
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = is_fullscreen;
        0.0
    }
}

/// Result of a hit-test against the tab bar.
#[derive(Debug, Clone, PartialEq)]
pub enum TabBarHit {
    Tab(usize),
    CloseTab(usize),
    NewTab,
    ShellPicker,
    Settings,
    SidebarToggle,
    GitPanelToggle,
    EmptyBar,
    None,
}

/// Display metadata for a single tab.
pub struct TabInfo {
    pub title: String,
    pub is_active: bool,
    /// Last block in this tab finished with an error.
    pub is_error: bool,
    /// Optional icon to draw before the title.
    pub icon: Option<Icon>,
    /// Tab is inactive/stopped (dimmed rendering).
    pub is_muted: bool,
}

/// Drag-and-drop state machine for tab reordering.
#[derive(Default)]
pub struct DragState {
    pub dragging: Option<usize>,
    pub start_x: f64,
    pub current_x: f64,
    pub active: bool,
}

impl DragState {
    pub fn begin(&mut self, tab_idx: usize, x: f64) {
        self.dragging = Some(tab_idx);
        self.start_x = x;
        self.current_x = x;
        self.active = false;
    }

    pub fn update(&mut self, x: f64) {
        self.current_x = x;
        if !self.active && (self.current_x - self.start_x).abs() > 5.0 {
            self.active = true;
        }
    }

    /// Returns `Some((from, to))` if the dragged tab should be moved.
    pub fn resolve_drop(
        &mut self,
        tab_count: usize,
        sf: f64,
        buf_width: f64,
        is_fullscreen: bool,
    ) -> Option<(usize, usize)> {
        let from = self.dragging?;
        if !self.active {
            self.reset();
            return None;
        }

        let tab_w = tab_logical_width(tab_count, buf_width, sf, is_fullscreen) * sf;
        let left_pad = left_padding(is_fullscreen) as f64 * sf;
        let rel_x = self.current_x - left_pad;
        let to = ((rel_x / tab_w) as usize).min(tab_count.saturating_sub(1));

        self.reset();

        if from != to { Some((from, to)) } else { None }
    }

    pub fn reset(&mut self) {
        self.dragging = None;
        self.active = false;
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some() && self.active
    }
}

/// Logical width reserved by the sidebar toggle icon + its margins.
fn sidebar_icon_logical_width() -> f64 {
    SIDEBAR_ICON_LEFT_MARGIN as f64 + SIDEBAR_ICON_SIZE as f64 + SIDEBAR_ICON_LEFT_MARGIN as f64
}

fn tab_logical_width(tab_count: usize, buf_width_phys: f64, sf: f64, is_fullscreen: bool) -> f64 {
    if tab_count == 0 {
        return TAB_MAX_WIDTH as f64;
    }
    let avail = (buf_width_phys / sf)
        - BTN_LEFT_MARGIN as f64
        - PLUS_BUTTON_WIDTH as f64
        - BUTTON_GAP as f64
        - SHELL_PICKER_WIDTH as f64
        - left_padding(is_fullscreen) as f64
        - sidebar_icon_logical_width()
        - GIT_PANEL_ICON_SIZE as f64
        - RIGHT_GAP as f64
        - AVATAR_ICON_SIZE as f64
        - RIGHT_GAP as f64
        - RIGHT_MARGIN as f64;
    (avail / tab_count as f64)
        .max(0.0)
        .min(TAB_MAX_WIDTH as f64)
}

pub fn hit_test(
    phys_x: f64,
    phys_y: f64,
    tab_count: usize,
    bar_h_phys: f64,
    buf_width_phys: f64,
    sf: f64,
    is_fullscreen: bool,
) -> TabBarHit {
    if phys_y > bar_h_phys {
        return TabBarHit::None;
    }

    let avatar_size = AVATAR_ICON_SIZE as f64 * sf;
    let margin = RIGHT_MARGIN as f64 * sf;

    let avatar_x = buf_width_phys - margin - avatar_size;
    if phys_x >= avatar_x && phys_x < avatar_x + avatar_size {
        return TabBarHit::Settings;
    }

    let git_gap = RIGHT_GAP as f64 * sf;
    let git_size = GIT_PANEL_ICON_SIZE as f64 * sf;
    let git_x = avatar_x - git_gap - git_size;
    if phys_x >= git_x && phys_x < git_x + git_size {
        return TabBarHit::GitPanelToggle;
    }

    let traffic_pad = left_padding(is_fullscreen) as f64 * sf;
    if phys_x < traffic_pad {
        return TabBarHit::None;
    }

    let sidebar_w = sidebar_icon_logical_width() * sf;
    let sidebar_start = traffic_pad;
    let sidebar_end = sidebar_start + sidebar_w;
    if phys_x >= sidebar_start && phys_x < sidebar_end {
        return TabBarHit::SidebarToggle;
    }

    let left_pad = sidebar_end;
    let tab_w = tab_logical_width(tab_count, buf_width_phys, sf, is_fullscreen) * sf;
    let x = phys_x - left_pad;

    for i in 0..tab_count {
        let tab_start = i as f64 * tab_w;
        let tab_end = tab_start + tab_w;
        if x >= tab_start && x < tab_end {
            let close_start = tab_end - 22.0 * sf;
            if x >= close_start {
                return TabBarHit::CloseTab(i);
            }
            return TabBarHit::Tab(i);
        }
    }

    let btn_margin = BTN_LEFT_MARGIN as f64 * sf;
    let plus_start = tab_count as f64 * tab_w + btn_margin;
    let plus_end = plus_start + PLUS_BUTTON_WIDTH as f64 * sf;
    if x >= plus_start && x < plus_end {
        return TabBarHit::NewTab;
    }

    let gap = BUTTON_GAP as f64 * sf;
    let picker_start = plus_end + gap;
    let picker_end = picker_start + SHELL_PICKER_WIDTH as f64 * sf;
    if x >= picker_start && x < picker_end {
        return TabBarHit::ShellPicker;
    }

    TabBarHit::EmptyBar
}

pub fn hovered_close_tab(
    phys_x: f64,
    phys_y: f64,
    tab_count: usize,
    bar_h_phys: f64,
    buf_width_phys: f64,
    sf: f64,
    is_fullscreen: bool,
) -> Option<usize> {
    if phys_y > bar_h_phys || phys_y < 0.0 {
        return None;
    }
    let left_pad = left_padding(is_fullscreen) as f64 * sf + sidebar_icon_logical_width() * sf;
    if phys_x < left_pad {
        return None;
    }
    let tab_w = tab_logical_width(tab_count, buf_width_phys, sf, is_fullscreen) * sf;
    let x = phys_x - left_pad;

    for i in 0..tab_count {
        let tab_start = i as f64 * tab_w;
        let tab_end = tab_start + tab_w;
        if x >= tab_start && x < tab_end {
            let close_start = tab_end - 22.0 * sf;
            if x >= close_start {
                return Some(i);
            }
            return None;
        }
    }
    None
}

/// Check if the cursor is hovering over the settings gear icon.
pub fn is_avatar_hovered(
    phys_x: f64,
    phys_y: f64,
    bar_h_phys: f64,
    buf_width_phys: f64,
    sf: f64,
) -> bool {
    if phys_y > bar_h_phys || phys_y < 0.0 {
        return false;
    }
    let avatar_size = AVATAR_ICON_SIZE as f64 * sf;
    let margin = RIGHT_MARGIN as f64 * sf;
    let avatar_x = buf_width_phys - margin - avatar_size;
    phys_x >= avatar_x && phys_x < avatar_x + avatar_size
}

pub fn is_new_tab_hovered(
    phys_x: f64,
    phys_y: f64,
    tab_count: usize,
    bar_h_phys: f64,
    buf_width_phys: f64,
    sf: f64,
    is_fullscreen: bool,
) -> bool {
    if phys_y > bar_h_phys || phys_y < 0.0 {
        return false;
    }
    let left_pad = (left_padding(is_fullscreen) as f64 + sidebar_icon_logical_width()) * sf;
    let tab_w = tab_logical_width(tab_count, buf_width_phys, sf, is_fullscreen) * sf;
    let x = phys_x - left_pad;
    let btn_margin = BTN_LEFT_MARGIN as f64 * sf;
    let plus_start = tab_count as f64 * tab_w + btn_margin;
    let plus_end = plus_start + PLUS_BUTTON_WIDTH as f64 * sf;
    x >= plus_start && x < plus_end
}

pub fn is_shell_picker_hovered(
    phys_x: f64,
    phys_y: f64,
    tab_count: usize,
    bar_h_phys: f64,
    buf_width_phys: f64,
    sf: f64,
    is_fullscreen: bool,
) -> bool {
    if phys_y > bar_h_phys || phys_y < 0.0 {
        return false;
    }
    let left_pad = (left_padding(is_fullscreen) as f64 + sidebar_icon_logical_width()) * sf;
    let tab_w = tab_logical_width(tab_count, buf_width_phys, sf, is_fullscreen) * sf;
    let x = phys_x - left_pad;
    let btn_margin = BTN_LEFT_MARGIN as f64 * sf;
    let plus_end = tab_count as f64 * tab_w + btn_margin + PLUS_BUTTON_WIDTH as f64 * sf;
    let gap = BUTTON_GAP as f64 * sf;
    let picker_start = plus_end + gap;
    let picker_end = picker_start + SHELL_PICKER_WIDTH as f64 * sf;
    x >= picker_start && x < picker_end
}

pub fn is_sidebar_hovered(
    phys_x: f64,
    phys_y: f64,
    bar_h_phys: f64,
    sf: f64,
    is_fullscreen: bool,
) -> bool {
    if phys_y > bar_h_phys || phys_y < 0.0 {
        return false;
    }
    let traffic_pad = left_padding(is_fullscreen) as f64 * sf;
    let sidebar_w = sidebar_icon_logical_width() * sf;
    phys_x >= traffic_pad && phys_x < traffic_pad + sidebar_w
}

pub fn is_git_panel_hovered(
    phys_x: f64,
    phys_y: f64,
    bar_h_phys: f64,
    buf_width_phys: f64,
    sf: f64,
) -> bool {
    if phys_y > bar_h_phys || phys_y < 0.0 {
        return false;
    }
    let avatar_size = AVATAR_ICON_SIZE as f64 * sf;
    let margin = RIGHT_MARGIN as f64 * sf;
    let avatar_x = buf_width_phys - margin - avatar_size;
    let git_gap = RIGHT_GAP as f64 * sf;
    let git_size = GIT_PANEL_ICON_SIZE as f64 * sf;
    let git_x = avatar_x - git_gap - git_size;
    phys_x >= git_x && phys_x < git_x + git_size
}

pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    avatar_renderer: &mut AvatarRenderer,
    bar_h: usize,
    tabs: &[TabInfo],
    sf: f32,
    hovered_close: Option<usize>,
    drag: &DragState,
    new_tab_hovered: bool,
    shell_picker_hovered: bool,
    sidebar_hovered: bool,
    sidebar_open: bool,
    git_panel_hovered: bool,
    git_panel_open: bool,
    is_fullscreen: bool,
) {
    let w = buf.width;

    buf.fill_rect(0, 0, w, bar_h, theme::TAB_BAR_BG);

    let border_h = (1.0 * sf).max(1.0) as usize;
    let border_y = bar_h.saturating_sub(border_h);
    buf.fill_rect(0, border_y, w, border_h, theme::TAB_SEPARATOR);

    let tab_count = tabs.len();
    let tab_w_logical = tab_logical_width(tab_count, w as f64, sf as f64, is_fullscreen);
    let tab_w = (tab_w_logical * sf as f64) as usize;

    let tab_font_size = 13.0 * sf;
    let tab_line_height = 18.0 * sf;
    let tab_metrics = Metrics::new(tab_font_size, tab_line_height);

    let tab_h = bar_h;
    let tab_top = 0;
    let traffic_pad = (left_padding(is_fullscreen) * sf) as usize;

    let content_cy = (bar_h as f32 / 2.0).round();

    let sidebar_icon_sz = (SIDEBAR_ICON_SIZE * sf).round() as u32;
    let sidebar_margin = (SIDEBAR_ICON_LEFT_MARGIN * sf) as usize;
    let sidebar_icon_x = traffic_pad + sidebar_margin;
    let sidebar_icon_y = (content_cy - sidebar_icon_sz as f32 / 2.0) as usize;

    if sidebar_hovered || sidebar_open {
        let hover_pad = (4.0 * sf) as usize;
        let hover_size = sidebar_icon_sz as usize + hover_pad * 2;
        let hover_x = sidebar_icon_x.saturating_sub(hover_pad);
        let hover_y = sidebar_icon_y.saturating_sub(hover_pad);
        let r = (4.0 * sf) as usize;
        super::overlay::fill_rounded_rect(
            buf,
            hover_x,
            hover_y,
            hover_size,
            hover_size,
            r,
            theme::TAB_ACTIVE_BG,
        );
    }

    let sidebar_color = if sidebar_open || sidebar_hovered {
        theme::TAB_ACTIVE_TEXT
    } else {
        theme::PLUS_TEXT
    };
    icon_renderer.draw(
        buf,
        Icon::PanelLeft,
        sidebar_icon_x,
        sidebar_icon_y,
        sidebar_icon_sz,
        sidebar_color,
    );

    let left_pad = traffic_pad + sidebar_margin + sidebar_icon_sz as usize + sidebar_margin;

    for (i, tab) in tabs.iter().enumerate() {
        let mut tab_x = left_pad + i * tab_w;

        if drag.is_dragging() && drag.dragging == Some(i) {
            let delta = (drag.current_x - drag.start_x) as isize;
            tab_x = (tab_x as isize + delta).max(left_pad as isize) as usize;
        }

        let draw_w = tab_w.min(w.saturating_sub(tab_x));

        if tab.is_active {
            buf.fill_rect(tab_x, tab_top, draw_w, tab_h, theme::TAB_ACTIVE_BG);
        }

        if i < tab_count - 1 && !tab.is_active && !tabs.get(i + 1).is_some_and(|t| t.is_active) {
            let sep_x = left_pad + (i + 1) * tab_w;
            let sep_w = (1.0 * sf).max(1.0) as usize;
            let margin = (6.0 * sf) as usize;
            buf.fill_rect(
                sep_x.saturating_sub(sep_w),
                tab_top + margin,
                sep_w,
                tab_h.saturating_sub(2 * margin),
                theme::TAB_SEPARATOR,
            );
        }

        let has_dot = tab.is_error;
        let dot_offset = if has_dot { (10.0 * sf) as usize } else { 0 };

        let icon_offset = if let Some(icon) = tab.icon {
            let tab_icon_sz = (13.0 * sf).round() as u32;
            let tab_icon_x = tab_x + (10.0 * sf) as usize + dot_offset;
            let tab_icon_y = tab_top + ((tab_h as f32 - tab_icon_sz as f32) / 2.0) as usize;
            let icon_color = if tab.is_muted {
                theme::FG_MUTED
            } else if tab.is_active {
                theme::TAB_ACTIVE_TEXT
            } else {
                theme::TAB_INACTIVE_TEXT
            };
            icon_renderer.draw(buf, icon, tab_icon_x, tab_icon_y, tab_icon_sz, icon_color);
            tab_icon_sz as usize + (4.0 * sf) as usize
        } else {
            0
        };

        let text_avail = tab_w_logical - 42.0;
        if text_avail > 0.0 {
            let effective_avail = if has_dot {
                text_avail - 10.0
            } else {
                text_avail
            };
            let max_chars = (effective_avail / 7.5).max(1.0) as usize;
            let display: String = if tab.title.len() > max_chars && max_chars > 3 {
                format!("{}...", &tab.title[..max_chars.saturating_sub(3)])
            } else if tab.title.len() > max_chars {
                tab.title[..max_chars].to_string()
            } else {
                tab.title.clone()
            };

            let text_color = if tab.is_muted {
                theme::FG_MUTED
            } else if tab.is_active {
                theme::TAB_ACTIVE_TEXT
            } else {
                theme::TAB_INACTIVE_TEXT
            };
            let text_x = tab_x + (12.0 * sf) as usize + dot_offset + icon_offset;
            let text_y = tab_top + ((tab_h as f32 - tab_line_height) / 2.0) as usize;

            draw_text_at(
                buf,
                font_system,
                swash_cache,
                text_x,
                text_y,
                bar_h,
                &display,
                tab_metrics,
                text_color,
                Family::Monospace,
            );
        }

        if has_dot {
            let dot_r = (3.0 * sf).max(1.0) as usize;
            let dot_x = tab_x + (12.0 * sf) as usize;
            let dot_cy = bar_h / 2;
            fill_circle(
                buf,
                dot_x,
                dot_cy.saturating_sub(dot_r),
                dot_r,
                theme::ERROR,
            );
        }

        let is_close_hovered = hovered_close == Some(i);
        draw_close_button(
            buf,
            icon_renderer,
            tab_x,
            draw_w,
            tab_top,
            tab_h,
            sf,
            tab.is_active,
            is_close_hovered,
        );
    }

    let btn_margin = (BTN_LEFT_MARGIN * sf) as usize;
    let btn_area_x = left_pad + tab_count * tab_w + btn_margin;
    let plus_w = (PLUS_BUTTON_WIDTH * sf) as usize;
    let plus_cx = btn_area_x as f32 + plus_w as f32 / 2.0;

    if new_tab_hovered {
        let hover_size = (22.0 * sf) as usize;
        let hover_x = (plus_cx - 11.0 * sf) as usize;
        let hover_y = (content_cy - 11.0 * sf) as usize;
        let r = (4.0 * sf) as usize;
        super::overlay::fill_rounded_rect(
            buf,
            hover_x,
            hover_y,
            hover_size,
            hover_size,
            r,
            theme::TAB_ACTIVE_BG,
        );
    }

    let plus_metrics = Metrics::new(16.0 * sf, 22.0 * sf);
    let _ = plus_metrics; // metrics kept for layout reference
    let icon_sz = (16.0 * sf).round() as u32;
    let icon_x = (plus_cx - icon_sz as f32 / 2.0) as usize;
    let icon_y = (content_cy - icon_sz as f32 / 2.0) as usize;
    icon_renderer.draw(
        buf,
        Icon::Plus,
        icon_x,
        icon_y,
        icon_sz,
        if new_tab_hovered {
            theme::TAB_ACTIVE_TEXT
        } else {
            theme::PLUS_TEXT
        },
    );

    let gap = (BUTTON_GAP * sf) as usize;
    let picker_w = (SHELL_PICKER_WIDTH * sf) as usize;
    let picker_x = btn_area_x + plus_w + gap;
    let picker_cx = picker_x as f32 + picker_w as f32 / 2.0;

    if shell_picker_hovered {
        let hover_size = (20.0 * sf) as usize;
        let hover_x = (picker_cx - 10.0 * sf) as usize;
        let hover_y = (content_cy - 10.0 * sf) as usize;
        let r = (4.0 * sf) as usize;
        super::overlay::fill_rounded_rect(
            buf,
            hover_x,
            hover_y,
            hover_size,
            hover_size,
            r,
            theme::TAB_ACTIVE_BG,
        );
    }

    let chev_sz = (14.0 * sf).round() as u32;
    let chev_x = (picker_cx - chev_sz as f32 / 2.0) as usize;
    let chev_y = (content_cy - chev_sz as f32 / 2.0) as usize;
    icon_renderer.draw(
        buf,
        Icon::ChevronDown,
        chev_x,
        chev_y,
        chev_sz,
        if shell_picker_hovered {
            theme::TAB_ACTIVE_TEXT
        } else {
            theme::PLUS_TEXT
        },
    );

    let margin = RIGHT_MARGIN * sf;
    let avatar_size = AVATAR_ICON_SIZE * sf;
    let avatar_x_f = w as f32 - margin - avatar_size;
    let avatar_cx = avatar_x_f + avatar_size / 2.0;

    let git_icon_sz = (GIT_PANEL_ICON_SIZE * sf).round() as u32;
    let git_gap = RIGHT_GAP * sf;
    let git_icon_x = (avatar_x_f - git_gap - git_icon_sz as f32) as usize;
    let git_icon_y = (content_cy - git_icon_sz as f32 / 2.0) as usize;

    if git_panel_hovered || git_panel_open {
        let hover_pad = (4.0 * sf) as usize;
        let hover_size = git_icon_sz as usize + hover_pad * 2;
        let hover_x = git_icon_x.saturating_sub(hover_pad);
        let hover_y = git_icon_y.saturating_sub(hover_pad);
        let r = (4.0 * sf) as usize;
        super::overlay::fill_rounded_rect(
            buf,
            hover_x,
            hover_y,
            hover_size,
            hover_size,
            r,
            theme::TAB_ACTIVE_BG,
        );
    }

    let git_color = if git_panel_open || git_panel_hovered {
        theme::TAB_ACTIVE_TEXT
    } else {
        theme::PLUS_TEXT
    };
    icon_renderer.draw(
        buf,
        Icon::Diff,
        git_icon_x,
        git_icon_y,
        git_icon_sz,
        git_color,
    );

    draw_user_avatar(buf, avatar_renderer, avatar_cx, content_cy, sf);
}

fn draw_close_button(
    buf: &mut PixelBuffer,
    icon_renderer: &mut IconRenderer,
    tab_x: usize,
    tab_w: usize,
    v_pad: usize,
    inner_h: usize,
    sf: f32,
    is_active: bool,
    is_hovered: bool,
) {
    if !is_active && !is_hovered {
        return;
    }
    if tab_w < (12.0 * sf) as usize {
        return;
    }

    let size = (14.0 * sf) as usize;
    let ideal_cx = tab_x as f32 + tab_w as f32 - 18.0 * sf;
    let min_cx = tab_x as f32 + tab_w as f32 / 2.0;
    let cx = ideal_cx.max(min_cx);
    let cy = v_pad as f32 + inner_h as f32 / 2.0;

    if is_hovered {
        let hover_size = (14.0 * sf) as usize;
        let hover_x = (cx - 7.0 * sf) as usize;
        let hover_y = (cy - 7.0 * sf) as usize;
        let r = (3.0 * sf) as usize;
        super::overlay::fill_rounded_rect(
            buf,
            hover_x,
            hover_y,
            hover_size,
            hover_size,
            r,
            theme::TAB_CLOSE_HOVER_BG,
        );
    }

    let color = if is_hovered {
        theme::TAB_CLOSE_HOVER
    } else {
        theme::TAB_CLOSE_NORMAL
    };

    let icon_sz = (size as f32 * 0.85).round() as u32;
    let icon_x = (cx - icon_sz as f32 / 2.0) as usize;
    let icon_y = (cy - icon_sz as f32 / 2.0) as usize;
    icon_renderer.draw(buf, Icon::Close, icon_x, icon_y, icon_sz, color);
}

fn draw_user_avatar(
    buf: &mut PixelBuffer,
    avatar_renderer: &mut AvatarRenderer,
    cx: f32,
    cy: f32,
    sf: f32,
) {
    let icon_sz = (20.0 * sf).round() as u32;
    let icon_x = (cx - icon_sz as f32 / 2.0) as usize;
    let icon_y = (cy - icon_sz as f32 / 2.0) as usize;
    avatar_renderer.draw(buf, icon_x, icon_y, icon_sz);
}

const USER_MENU_W: f32 = 180.0;
const USER_MENU_ITEM_H: f32 = 32.0;
const USER_MENU_LABEL_H: f32 = 26.0;
const USER_MENU_SEP_H: f32 = 9.0;
const USER_MENU_PAD_X: f32 = 12.0;
const USER_MENU_GAP: f32 = 2.0;

fn user_menu_item_count(is_pro: bool) -> usize {
    if is_pro { 2 } else { 3 }
}

fn user_menu_rect(buf_width: f64, bar_h: f64, sf: f64, is_pro: bool) -> (f64, f64, f64, f64) {
    let margin = RIGHT_MARGIN as f64 * sf;
    let icon_size = AVATAR_ICON_SIZE as f64 * sf;
    let menu_w = USER_MENU_W as f64 * sf;
    let label_h = USER_MENU_LABEL_H as f64 * sf;
    let sep_h = USER_MENU_SEP_H as f64 * sf;
    let item_h = USER_MENU_ITEM_H as f64 * sf;
    let items = user_menu_item_count(is_pro) as f64;

    let menu_x =
        (buf_width - margin - icon_size / 2.0 - menu_w / 2.0).min(buf_width - menu_w - margin);
    let menu_y = bar_h + USER_MENU_GAP as f64 * sf;
    let menu_h = items * item_h + sep_h + label_h;
    (menu_x, menu_y, menu_w, menu_h)
}

fn user_menu_item_at(
    phys_x: f64,
    phys_y: f64,
    bar_h: f64,
    buf_width: f64,
    sf: f64,
    is_pro: bool,
) -> Option<usize> {
    let (menu_x, menu_y, menu_w, menu_h) = user_menu_rect(buf_width, bar_h, sf, is_pro);

    if phys_x < menu_x || phys_x >= menu_x + menu_w || phys_y < menu_y || phys_y >= menu_y + menu_h
    {
        return None;
    }

    let item_h = USER_MENU_ITEM_H as f64 * sf;
    let items = user_menu_item_count(is_pro);
    let rel_y = phys_y - menu_y;

    for i in 0..items {
        let iy = i as f64 * item_h;
        if rel_y >= iy && rel_y < iy + item_h {
            return Some(i);
        }
    }
    None
}

pub fn draw_user_menu(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    bar_h: usize,
    buf_width: usize,
    sf: f32,
    hovered: Option<usize>,
    is_pro: bool,
) {
    let (mx, my, mw, mh) = user_menu_rect(buf_width as f64, bar_h as f64, sf as f64, is_pro);
    let menu_x = mx as usize;
    let menu_y = my as usize;
    let menu_w = mw as usize;
    let menu_h = mh as usize;
    let item_h = (USER_MENU_ITEM_H * sf) as usize;
    let label_h = (USER_MENU_LABEL_H * sf) as usize;
    let sep_h = (USER_MENU_SEP_H * sf) as usize;
    let pad_x = (USER_MENU_PAD_X * sf) as usize;
    let border_w = (1.0_f32 * sf).max(1.0) as usize;
    let corner_r = (6.0 * sf) as usize;
    let inner_r = corner_r.saturating_sub(1);
    let items = user_menu_item_count(is_pro);

    super::overlay::fill_rounded_rect(
        buf,
        menu_x,
        menu_y,
        menu_w,
        menu_h,
        corner_r,
        theme::BG_ELEVATED,
    );
    super::overlay::draw_border_rounded(
        buf,
        menu_x,
        menu_y,
        menu_w,
        menu_h,
        border_w,
        corner_r,
        theme::BORDER,
    );

    let text_metrics = Metrics::new(13.0 * sf, 18.0 * sf);

    let labels: Vec<(&str, bool)> = if is_pro {
        vec![("Settings", false), ("Local Models", false)]
    } else {
        vec![
            ("Settings", false),
            ("Local Models", false),
            ("Upgrade to Pro", true),
        ]
    };

    for (i, (label, bold)) in labels.iter().enumerate() {
        let iy = menu_y + i * item_h;

        if hovered == Some(i) {
            let inner_x = menu_x + border_w;
            let inner_w = menu_w.saturating_sub(border_w * 2);
            if i == 0 {
                let h = item_h.saturating_sub(border_w);
                if items == 1 {
                    super::overlay::fill_rounded_rect(
                        buf,
                        inner_x,
                        iy + border_w,
                        inner_w,
                        h,
                        inner_r,
                        theme::BG_HOVER,
                    );
                } else {
                    buf.fill_rect(inner_x, iy + border_w, inner_w, h, theme::BG_HOVER);
                }
            } else {
                buf.fill_rect(inner_x, iy, inner_w, item_h, theme::BG_HOVER);
            }
        }

        let text_y = if i == 0 {
            menu_y + ((item_h as f32 - 18.0 * sf) / 2.0) as usize
        } else {
            iy + ((item_h as f32 - 18.0 * sf) / 2.0) as usize
        };
        let color = if hovered == Some(i) {
            theme::FG_BRIGHT
        } else {
            theme::FG_PRIMARY
        };

        if *bold {
            draw_text_at_bold(
                buf,
                font_system,
                swash_cache,
                menu_x + pad_x,
                text_y,
                buf.height,
                label,
                text_metrics,
                color,
                Family::SansSerif,
            );
        } else {
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                menu_x + pad_x,
                text_y,
                buf.height,
                label,
                text_metrics,
                color,
                Family::SansSerif,
            );
        }
    }

    let sep_y = menu_y + items * item_h + sep_h / 2;
    buf.fill_rect(
        menu_x + pad_x,
        sep_y,
        menu_w.saturating_sub(pad_x * 2),
        1,
        theme::BORDER,
    );

    let label_metrics = Metrics::new(11.0 * sf, 15.0 * sf);
    let version_label = if is_pro {
        concat!("Awebo Pro v", env!("CARGO_PKG_VERSION"))
    } else {
        concat!("Awebo v", env!("CARGO_PKG_VERSION"))
    };
    let label_y_start = menu_y + items * item_h + sep_h;
    let label_y = label_y_start + ((label_h as f32 - 15.0 * sf) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        menu_x + pad_x,
        label_y,
        buf.height,
        version_label,
        label_metrics,
        theme::FG_DIM,
        Family::SansSerif,
    );
}

pub fn user_menu_hit_test(
    phys_x: f64,
    phys_y: f64,
    bar_h: f64,
    buf_width: f64,
    sf: f64,
    is_pro: bool,
) -> Option<usize> {
    user_menu_item_at(phys_x, phys_y, bar_h, buf_width, sf, is_pro)
}

pub fn user_menu_hovered(
    phys_x: f64,
    phys_y: f64,
    bar_h: f64,
    buf_width: f64,
    sf: f64,
    is_pro: bool,
) -> Option<usize> {
    user_menu_item_at(phys_x, phys_y, bar_h, buf_width, sf, is_pro)
}

pub fn draw_tooltip(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text: &str,
    phys_x: f64,
    phys_y: f64,
    sf: f32,
) {
    let pad_x = (8.0 * sf) as usize;
    let pad_y = (4.0 * sf) as usize;
    let text_metrics = Metrics::new(11.0 * sf, 15.0 * sf);
    let approx_w = (text.len() as f32 * 6.5 * sf) as usize + pad_x * 2;
    let h = (15.0 * sf) as usize + pad_y * 2;
    let corner_r = (4.0 * sf) as usize;

    let x = (phys_x as usize)
        .saturating_sub(approx_w / 2)
        .min(buf.width.saturating_sub(approx_w));
    let y = phys_y as usize + (20.0 * sf) as usize;
    let y = if y + h > buf.height {
        (phys_y as usize).saturating_sub(h + (8.0 * sf) as usize)
    } else {
        y
    };

    super::overlay::fill_rounded_rect(buf, x, y, approx_w, h, corner_r, theme::BG_ELEVATED);
    let bw = (1.0 * sf).max(1.0) as usize;
    super::overlay::draw_border(buf, x, y, approx_w, h, bw, theme::BORDER);

    let text_x = x + pad_x;
    let text_y = y + pad_y;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        text_x,
        text_y,
        buf.height,
        text,
        text_metrics,
        theme::FG_PRIMARY,
        Family::Monospace,
    );
}

/// Tiny filled circle (used for status dots in tabs).
fn fill_circle(buf: &mut PixelBuffer, cx: usize, cy: usize, r: usize, color: (u8, u8, u8)) {
    if r == 0 {
        return;
    }
    let r_sq = (r * r) as isize;
    for dy in 0..=(r * 2) {
        let y = cy + dy;
        if y >= buf.height {
            break;
        }
        let rel_y = dy as isize - r as isize;
        let half_w_sq = r_sq - rel_y * rel_y;
        if half_w_sq < 0 {
            continue;
        }
        let half_w = (half_w_sq as f32).sqrt() as usize;
        let x0 = cx.saturating_sub(half_w);
        let w = (half_w * 2 + 1).min(buf.width.saturating_sub(x0));
        if w > 0 {
            buf.fill_rect(x0, y, w, 1, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_state_default_not_dragging() {
        let ds = DragState::default();
        assert!(!ds.is_dragging());
        assert!(ds.dragging.is_none());
    }

    #[test]
    fn drag_state_begin_and_update() {
        let mut ds = DragState::default();
        ds.begin(2, 100.0);
        assert_eq!(ds.dragging, Some(2));
        assert!(!ds.is_dragging());
        ds.update(110.0);
        assert!(ds.is_dragging());
    }

    #[test]
    fn drag_state_reset() {
        let mut ds = DragState::default();
        ds.begin(0, 50.0);
        ds.update(60.0);
        ds.reset();
        assert!(!ds.is_dragging());
        assert!(ds.dragging.is_none());
    }

    #[test]
    fn hit_test_below_bar_is_none() {
        let result = hit_test(100.0, 50.0, 2, 42.0, 1200.0, 1.0, false);
        assert_eq!(result, TabBarHit::None);
    }

    #[test]
    fn hit_test_gear_area() {
        let result = hit_test(1200.0 - 20.0, 20.0, 2, 42.0, 1200.0, 1.0, false);
        assert_eq!(result, TabBarHit::Settings);
    }

    #[test]
    fn hit_test_first_tab() {
        let sidebar_w = sidebar_icon_logical_width();
        let x = left_padding(false) as f64 + sidebar_w + 30.0;
        let result = hit_test(x, 20.0, 2, 42.0, 1200.0, 1.0, false);
        assert_eq!(result, TabBarHit::Tab(0));
    }

    #[test]
    fn is_avatar_hovered_right_edge() {
        assert!(is_avatar_hovered(1200.0 - 20.0, 20.0, 42.0, 1200.0, 1.0));
    }

    #[test]
    fn is_avatar_hovered_far_left_false() {
        assert!(!is_avatar_hovered(10.0, 20.0, 42.0, 1200.0, 1.0));
    }

    #[test]
    fn is_new_tab_hovered_after_tabs() {
        let lp = left_padding(false) as f64 + sidebar_icon_logical_width();
        let tab_w = tab_logical_width(2, 1200.0, 1.0, false);
        let x = lp + 2.0 * tab_w + BTN_LEFT_MARGIN as f64 + 10.0;
        let result = is_new_tab_hovered(x, 20.0, 2, 42.0, 1200.0, 1.0, false);
        assert!(result);
    }

    #[test]
    fn is_shell_picker_hovered_after_new_tab() {
        let lp = left_padding(false) as f64 + sidebar_icon_logical_width();
        let tab_w = tab_logical_width(2, 1200.0, 1.0, false);
        let x = lp + 2.0 * tab_w + BTN_LEFT_MARGIN as f64 + 36.0 + 4.0 + 10.0;
        let result = is_shell_picker_hovered(x, 20.0, 2, 42.0, 1200.0, 1.0, false);
        assert!(result);
    }

    #[test]
    fn hovered_close_tab_outside_bar() {
        assert!(hovered_close_tab(100.0, 50.0, 2, 42.0, 1200.0, 1.0, false).is_none());
    }
}
