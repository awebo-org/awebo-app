//! Left side panel — toolbar-based routing between Sessions and Files sub-views.
//!
//! The header contains icon buttons (Rows → Sessions, Files → FileTree).
//! The active button is highlighted with PRIMARY color.
//! Below the toolbar, the appropriate sub-view renders.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::{Icon, IconRenderer};
use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::{draw_text_at, draw_text_at_bold_buffered, draw_text_at_buffered};
use crate::renderer::theme;
use crate::session::Session;
use crate::ui::file_tree::FileTreeState;
use crate::ui::panel_layout::{PanelLayout, SidePanelTab};

const HEADER_HEIGHT: f32 = 40.0;
const TOOLBAR_BTN_SIZE: f32 = 16.0;
const TOOLBAR_BTN_CONTAINER: f32 = 26.0;
const TOOLBAR_BTN_GAP: f32 = 2.0;
const TOOLBAR_BTN_RADIUS: f32 = 5.0;
const TOOLBAR_PAD_X: f32 = 8.0;
const ITEM_HEIGHT: f32 = 56.0;
const ITEM_PAD_X: f32 = 14.0;
const ITEM_PAD_Y: f32 = 8.0;
const CLEAR_BTN_SIZE: f32 = 16.0;

/// Result of a click hit-test on the side panel.
#[derive(Debug, Clone, PartialEq)]
pub enum SidePanelHit {
    OpenSession(usize),
    ClearSession(usize),
    ToolbarSessions,
    ToolbarFiles,
    ToolbarSandbox,
    ToolbarSearch,
    StopSandbox,
    None,
}

/// Snapshot of the active sandbox for the side panel.
pub struct SandboxInfo {
    pub display_name: String,
    pub cpus: u32,
    pub memory_mib: u32,
    pub is_alive: bool,
    pub is_initializing: bool,
    pub pull_state: crate::sandbox::bridge::PullState,
    pub volumes: Vec<(String, String)>,
}

/// Persistent hover state for the side panel.
#[derive(Default)]
pub struct SidePanelState {
    pub scroll_offset: f32,
    pub hovered_item: Option<usize>,
    pub hovered_clear: Option<usize>,
    pub active_session_visual_idx: Option<usize>,
    pub hovered_toolbar_btn: Option<SidePanelTab>,
    pub scrollbar_hovered: bool,
    pub scrollbar_dragging: bool,
    pub sandbox: SandboxPanelState,
}

/// State for the sandbox management panel.
#[derive(Default)]
pub struct SandboxPanelState {
    /// Whether the stop button is hovered.
    pub stop_hovered: bool,
}

/// Draw the side panel with toolbar header.
/// Returns the physical width consumed (0 if panel is closed).
pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    sessions: &[&Session],
    state: &SidePanelState,
    file_tree: &FileTreeState,
    active_file_path: Option<&std::path::Path>,
    panel_layout: &PanelLayout,
    bar_h: usize,
    sf: f32,
    sandbox_info: Option<&SandboxInfo>,
    search_panel: &crate::ui::search_panel::SearchPanelState,
    cursor_visible: bool,
) -> usize {
    let panel_w = panel_layout.left_physical_width(sf);
    let panel_h = buf.height.saturating_sub(bar_h);
    if panel_w == 0 || panel_h == 0 {
        return 0;
    }

    buf.fill_rect(0, bar_h, panel_w, panel_h, (0, 0, 0));

    let border_w = (1.0 * sf).max(1.0) as usize;
    buf.fill_rect(
        panel_w.saturating_sub(border_w),
        bar_h,
        border_w,
        panel_h,
        theme::BORDER,
    );

    let header_h = (HEADER_HEIGHT * sf) as usize;
    let icon_sz = (TOOLBAR_BTN_SIZE * sf).round() as u32;
    let container_sz = (TOOLBAR_BTN_CONTAINER * sf) as usize;
    let btn_gap = (TOOLBAR_BTN_GAP * sf) as usize;
    let btn_r = (TOOLBAR_BTN_RADIUS * sf) as usize;
    let pad_x = (TOOLBAR_PAD_X * sf) as usize;
    let icon_inset = (container_sz as f32 - icon_sz as f32) / 2.0;

    let container_y = bar_h + ((header_h - container_sz) / 2).max(0);

    let divider_y = bar_h + header_h;

    let content_y = divider_y + border_w;

    match panel_layout.active_tab {
        SidePanelTab::Sessions => {
            let visible_h = buf.height.saturating_sub(content_y);
            draw_sessions(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                sessions,
                state,
                panel_w,
                content_y,
                border_w,
                sf,
            );
            let total_h = sessions.len() * (ITEM_HEIGHT * sf) as usize;
            let sb_hover = state.scrollbar_hovered || state.scrollbar_dragging;
            draw_panel_scrollbar(
                buf,
                panel_w,
                content_y,
                visible_h,
                total_h,
                state.scroll_offset as usize,
                sb_hover,
                sf,
            );
        }
        SidePanelTab::Files => {
            let visible_h = buf.height.saturating_sub(content_y);
            crate::ui::file_tree::draw(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                file_tree,
                active_file_path,
                panel_w,
                content_y,
                sf,
            );
            let row_count = file_tree.row_count();
            let total_h = row_count * (crate::ui::file_tree::ITEM_HEIGHT_PX * sf) as usize
                + (crate::ui::file_tree::PAD_Y_PX * sf) as usize * 2;
            let sb_hover = file_tree.scrollbar_hovered || file_tree.scrollbar_dragging;
            draw_panel_scrollbar(
                buf,
                panel_w,
                content_y,
                visible_h,
                total_h,
                file_tree.scroll_offset as usize,
                sb_hover,
                sf,
            );
        }
        SidePanelTab::Sandbox => {
            draw_sandbox_panel(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                &state.sandbox,
                sandbox_info,
                panel_w,
                content_y,
                sf,
            );
        }
        SidePanelTab::Search => {
            crate::ui::search_panel::draw(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                search_panel,
                panel_w,
                content_y,
                sf,
                cursor_visible,
            );
            let visible_h = buf.height.saturating_sub(content_y);
            let total_h = search_panel.total_height(sf);
            let sb_hover = search_panel.scrollbar_hovered || search_panel.scrollbar_dragging;
            draw_panel_scrollbar(
                buf,
                panel_w,
                content_y,
                visible_h,
                total_h,
                search_panel.scroll_offset as usize,
                sb_hover,
                sf,
            );
        }
    }

    buf.fill_rect(0, bar_h, panel_w, header_h, (0, 0, 0));
    buf.fill_rect(0, divider_y, panel_w, border_w, theme::BORDER);
    buf.fill_rect(
        panel_w.saturating_sub(border_w),
        bar_h,
        border_w,
        header_h + border_w,
        theme::BORDER,
    );

    fn draw_toolbar_btn(
        buf: &mut PixelBuffer,
        icon_renderer: &mut IconRenderer,
        icon: Icon,
        cx: usize,
        cy: usize,
        container_sz: usize,
        icon_sz: u32,
        icon_inset: f32,
        btn_r: usize,
        active: bool,
        hovered: bool,
    ) {
        let bg = if active {
            Some(theme::BG_ELEVATED)
        } else if hovered {
            Some(theme::BG_HOVER)
        } else {
            None
        };
        if let Some(bg_color) = bg {
            super::overlay::fill_rounded_rect(
                buf,
                cx,
                cy,
                container_sz,
                container_sz,
                btn_r,
                bg_color,
            );
        }
        let icon_x = cx + icon_inset as usize;
        let icon_y = cy + icon_inset as usize;
        let color = if active {
            theme::FG_BRIGHT
        } else if hovered {
            theme::FG_PRIMARY
        } else {
            theme::FG_MUTED
        };
        icon_renderer.draw(buf, icon, icon_x, icon_y, icon_sz, color);
    }

    let mut cx = pad_x;
    let sessions_active = panel_layout.active_tab == SidePanelTab::Sessions;
    let sessions_hovered = state.hovered_toolbar_btn == Some(SidePanelTab::Sessions);
    draw_toolbar_btn(
        buf,
        icon_renderer,
        Icon::Rows,
        cx,
        container_y,
        container_sz,
        icon_sz,
        icon_inset,
        btn_r,
        sessions_active,
        sessions_hovered,
    );

    cx += container_sz + btn_gap;

    let files_active = panel_layout.active_tab == SidePanelTab::Files;
    let files_hovered = state.hovered_toolbar_btn == Some(SidePanelTab::Files);
    draw_toolbar_btn(
        buf,
        icon_renderer,
        Icon::Files,
        cx,
        container_y,
        container_sz,
        icon_sz,
        icon_inset,
        btn_r,
        files_active,
        files_hovered,
    );

    if sandbox_info.is_some() {
        cx += container_sz + btn_gap;
        let sandbox_active = panel_layout.active_tab == SidePanelTab::Sandbox;
        let sandbox_hovered = state.hovered_toolbar_btn == Some(SidePanelTab::Sandbox);
        draw_toolbar_btn(
            buf,
            icon_renderer,
            Icon::CodeSandbox,
            cx,
            container_y,
            container_sz,
            icon_sz,
            icon_inset,
            btn_r,
            sandbox_active,
            sandbox_hovered,
        );
    }

    cx += container_sz + btn_gap;
    let search_active = panel_layout.active_tab == SidePanelTab::Search;
    let search_hovered = state.hovered_toolbar_btn == Some(SidePanelTab::Search);
    draw_toolbar_btn(
        buf,
        icon_renderer,
        Icon::Search,
        cx,
        container_y,
        container_sz,
        icon_sz,
        icon_inset,
        btn_r,
        search_active,
        search_hovered,
    );

    panel_w
}

const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_MARGIN: f32 = 2.0;
const SCROLLBAR_MIN_THUMB: f32 = 20.0;
const SCROLLBAR_COLOR: (u8, u8, u8) = (80, 84, 96);
const SCROLLBAR_HOVER_COLOR: (u8, u8, u8) = crate::renderer::theme::SCROLLBAR_THUMB_HOVER;

/// Scrollbar thumb rectangle: (x, y, w, h). Returns `None` if content fits.
pub fn panel_scrollbar_thumb_rect(
    panel_w: usize,
    y_start: usize,
    visible_h: usize,
    total_h: usize,
    scroll: usize,
    sf: f32,
) -> Option<(usize, usize, usize, usize)> {
    if total_h <= visible_h || visible_h == 0 {
        return None;
    }
    let sb_w = (SCROLLBAR_WIDTH * sf).max(4.0) as usize;
    let sb_margin = (SCROLLBAR_MARGIN * sf) as usize;
    let border_w = (1.0 * sf).max(1.0) as usize;
    let track_x = panel_w.saturating_sub(sb_w + sb_margin + border_w);
    let track_h = visible_h;
    let thumb_h = ((visible_h as f64 / total_h as f64) * track_h as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let max_scroll = total_h.saturating_sub(visible_h);
    let frac = if max_scroll > 0 {
        scroll.min(max_scroll) as f64 / max_scroll as f64
    } else {
        0.0
    };
    let thumb_y = y_start + (frac * (track_h.saturating_sub(thumb_h)) as f64) as usize;
    Some((track_x, thumb_y, sb_w, thumb_h))
}

/// Hit-test: is (px, py) inside the scrollbar thumb?
pub fn panel_scrollbar_hit_test(
    px: usize,
    py: usize,
    panel_w: usize,
    y_start: usize,
    visible_h: usize,
    total_h: usize,
    scroll: usize,
    sf: f32,
) -> bool {
    if let Some((tx, ty, tw, th)) =
        panel_scrollbar_thumb_rect(panel_w, y_start, visible_h, total_h, scroll, sf)
    {
        let margin = (4.0 * sf) as usize;
        px + margin >= tx && px < tx + tw + margin && py >= ty && py < ty + th
    } else {
        false
    }
}

/// Map a vertical pixel drag delta to a new scroll offset.
pub fn panel_scrollbar_drag_to_scroll(
    drag_y: f64,
    drag_start_scroll: f32,
    visible_h: usize,
    total_h: usize,
    sf: f32,
) -> f32 {
    if total_h <= visible_h || visible_h == 0 {
        return 0.0;
    }
    let thumb_h = ((visible_h as f64 / total_h as f64) * visible_h as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64);
    let track_usable = visible_h as f64 - thumb_h;
    if track_usable <= 0.0 {
        return drag_start_scroll;
    }
    let max_scroll = total_h.saturating_sub(visible_h) as f64;
    let scroll_delta = (drag_y / track_usable) * max_scroll;
    (drag_start_scroll as f64 + scroll_delta).clamp(0.0, max_scroll) as f32
}

/// Draw a thin scrollbar inside the panel.
fn draw_panel_scrollbar(
    buf: &mut PixelBuffer,
    panel_w: usize,
    y_start: usize,
    visible_h: usize,
    total_h: usize,
    scroll: usize,
    hovered: bool,
    sf: f32,
) {
    if let Some((tx, ty, tw, th)) =
        panel_scrollbar_thumb_rect(panel_w, y_start, visible_h, total_h, scroll, sf)
    {
        let color = if hovered {
            SCROLLBAR_HOVER_COLOR
        } else {
            SCROLLBAR_COLOR
        };
        buf.fill_rect(tx, ty, tw, th, color);
    }
}

/// Vertical position of the stop button (stored during draw for hit-test).
/// Measured from `content_y` in logical pixels.
const STOP_BTN_HEIGHT: f32 = 30.0;

/// Draw the sandbox management sub-view.
fn draw_sandbox_panel(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &SandboxPanelState,
    info: Option<&SandboxInfo>,
    panel_w: usize,
    content_y: usize,
    sf: f32,
) {
    let pad_x = (ITEM_PAD_X * sf) as usize;
    let title_metrics = Metrics::new(11.0 * sf, 15.0 * sf);
    let label_metrics = Metrics::new(12.5 * sf, 17.0 * sf);
    let value_metrics = Metrics::new(11.5 * sf, 16.0 * sf);
    let small_metrics = Metrics::new(10.5 * sf, 14.0 * sf);
    let row_h = (36.0 * sf) as usize;
    let section_gap = (16.0 * sf) as usize;
    let clip_h = buf.height;

    let mut y = content_y + (12.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        pad_x,
        y,
        clip_h,
        "SANDBOX",
        title_metrics,
        theme::FG_MUTED,
        Family::Monospace,
    );
    y += (22.0 * sf) as usize;

    if info.is_none() {
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            pad_x,
            y + (row_h as f32 / 2.0 - 8.5 * sf) as usize,
            clip_h,
            "No active sandbox",
            label_metrics,
            theme::FG_MUTED,
            Family::Monospace,
        );
        return;
    }

    let image_val = info.map(|i| i.display_name.as_str()).unwrap_or("—");
    let cpu_val = info
        .map(|i| format!("{} vCPU", i.cpus))
        .unwrap_or_else(|| "—".into());
    let mem_val = info
        .map(|i| format!("{} MiB", i.memory_mib))
        .unwrap_or_else(|| "—".into());

    let info_items: &[(&str, &str)] = &[
        ("Image", image_val),
        ("CPU", &cpu_val),
        ("Memory", &mem_val),
    ];

    for (label, value) in info_items {
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            pad_x,
            y + (row_h as f32 / 2.0 - 8.5 * sf) as usize,
            clip_h,
            label,
            label_metrics,
            theme::FG_MUTED,
            Family::Monospace,
        );
        let val_x = panel_w / 2;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            val_x,
            y + (row_h as f32 / 2.0 - 8.0 * sf) as usize,
            clip_h,
            value,
            value_metrics,
            theme::FG_PRIMARY,
            Family::Monospace,
        );
        y += row_h;
    }

    let volumes = info.map(|i| i.volumes.as_slice()).unwrap_or(&[]);
    if !volumes.is_empty() {
        y += section_gap;

        let divider_h = (1.0 * sf).max(1.0) as usize;
        buf.fill_rect(
            pad_x,
            y,
            panel_w.saturating_sub(pad_x * 2),
            divider_h,
            theme::DIVIDER,
        );
        y += section_gap;

        draw_text_at(
            buf,
            font_system,
            swash_cache,
            pad_x,
            y,
            clip_h,
            "VOLUMES",
            title_metrics,
            theme::FG_MUTED,
            Family::Monospace,
        );
        y += (20.0 * sf) as usize;

        let vol_row_h = (44.0 * sf) as usize;
        for (guest, host) in volumes {
            if y + vol_row_h > clip_h {
                break;
            }

            draw_text_at(
                buf,
                font_system,
                swash_cache,
                pad_x,
                y + (4.0 * sf) as usize,
                clip_h,
                guest,
                label_metrics,
                theme::FG_PRIMARY,
                Family::Monospace,
            );

            draw_text_at(
                buf,
                font_system,
                swash_cache,
                pad_x,
                y + (22.0 * sf) as usize,
                clip_h,
                host,
                small_metrics,
                theme::FG_MUTED,
                Family::Monospace,
            );

            y += vol_row_h;
        }
    }

    y += section_gap;

    let has_live = info.map(|i| i.is_alive).unwrap_or(false);
    let btn_w = panel_w - pad_x * 2;
    let btn_h = (STOP_BTN_HEIGHT * sf) as usize;
    let btn_r = (4.0 * sf) as usize;
    let is_hovered = has_live && state.stop_hovered;
    let btn_bg = if is_hovered {
        theme::BG_HOVER
    } else {
        theme::BG_ELEVATED
    };
    super::overlay::fill_rounded_rect(buf, pad_x, y, btn_w, btn_h, btn_r, btn_bg);

    let icon_sz = (14.0 * sf).round() as u32;
    let icon_x = pad_x + (8.0 * sf) as usize;
    let icon_y = y + (btn_h as f32 / 2.0 - icon_sz as f32 / 2.0) as usize;
    let muted = !has_live;
    let icon_color = if muted {
        theme::FG_MUTED
    } else if is_hovered {
        theme::ERROR
    } else {
        theme::FG_SECONDARY
    };
    icon_renderer.draw(buf, Icon::Stop, icon_x, icon_y, icon_sz, icon_color);

    let label_x = icon_x + icon_sz as usize + (6.0 * sf) as usize;
    let label_y = y + (btn_h as f32 / 2.0 - 8.5 * sf) as usize;
    let stop_color = if muted {
        theme::FG_MUTED
    } else if is_hovered {
        theme::ERROR
    } else {
        theme::FG_SECONDARY
    };
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        label_x,
        label_y,
        clip_h,
        "Stop Sandbox",
        label_metrics,
        stop_color,
        Family::Monospace,
    );
}

/// Draw the sessions list sub-view.
fn draw_sessions(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    sessions: &[&Session],
    state: &SidePanelState,
    panel_w: usize,
    list_y: usize,
    border_w: usize,
    sf: f32,
) {
    let item_h = (ITEM_HEIGHT * sf) as usize;
    let title_metrics = Metrics::new(12.5 * sf, 17.0 * sf);
    let detail_metrics = Metrics::new(10.5 * sf, 14.0 * sf);
    let pad_x = (ITEM_PAD_X * sf) as usize;
    let pad_y = (ITEM_PAD_Y * sf) as usize;
    let clear_sz = (CLEAR_BTN_SIZE * sf).round() as u32;

    let mut title_buf = cosmic_text::Buffer::new(font_system, title_metrics);
    let mut detail_buf = cosmic_text::Buffer::new(font_system, detail_metrics);

    let scroll = state.scroll_offset as usize;
    let first_visible = scroll / item_h.max(1);
    let visible_count = buf.height.saturating_sub(list_y) / item_h.max(1) + 2;
    let last_visible = (first_visible + visible_count).min(sessions.len());

    for (i, session) in sessions
        .iter()
        .enumerate()
        .skip(first_visible)
        .take(last_visible - first_visible)
    {
        let y = list_y + i * item_h - scroll;
        if y + item_h <= list_y || y >= buf.height {
            continue;
        }

        let is_hovered = state.hovered_item == Some(i);
        let is_clear_hovered = state.hovered_clear == Some(i);
        let is_current = state.active_session_visual_idx == Some(i);

        if is_current && !is_hovered {
            let accent_w = (3.0 * sf).max(2.0) as usize;
            buf.fill_rect(0, y, accent_w, item_h, theme::PRIMARY);
            buf.fill_rect(
                accent_w,
                y,
                panel_w.saturating_sub(border_w + accent_w),
                item_h,
                theme::BG_SELECTION,
            );
        } else if is_hovered {
            buf.fill_rect(
                0,
                y,
                panel_w.saturating_sub(border_w),
                item_h,
                theme::BG_HOVER,
            );
            if is_current {
                let accent_w = (3.0 * sf).max(2.0) as usize;
                buf.fill_rect(0, y, accent_w, item_h, theme::PRIMARY);
            }
        }

        let title = session.display_title();
        let max_chars = ((panel_w as f32 - pad_x as f32 * 2.0 - clear_sz as f32 - 8.0 * sf)
            / (7.0 * sf))
            .max(1.0) as usize;
        let needs_truncation = title.len() > max_chars && max_chars > 3;
        let truncated_title;
        let display_title: &str = if needs_truncation {
            truncated_title = format!("{}…", &title[..max_chars.saturating_sub(1)]);
            &truncated_title
        } else {
            &title
        };

        let title_color = if is_current || is_hovered {
            theme::FG_BRIGHT
        } else {
            theme::FG_PRIMARY
        };
        draw_text_at_bold_buffered(
            buf,
            font_system,
            swash_cache,
            &mut title_buf,
            pad_x,
            y + pad_y,
            buf.height,
            display_title,
            title_metrics,
            title_color,
            Family::SansSerif,
        );

        let entry_count = session.entries.len();
        let age = session
            .entries
            .last()
            .map(|e| e.started_at.elapsed().unwrap_or_default())
            .unwrap_or_else(|| session.created_at.elapsed().unwrap_or_default());
        let age_str = if age.as_secs() < 60 { "just now" } else { "" };
        let detail = if age.as_secs() < 60 {
            if entry_count == 1 {
                format!("1 command · {}", age_str)
            } else {
                format!("{} commands · {}", entry_count, age_str)
            }
        } else if age.as_secs() < 3600 {
            let mins = age.as_secs() / 60;
            if entry_count == 1 {
                format!("1 command · {}m ago", mins)
            } else {
                format!("{} commands · {}m ago", entry_count, mins)
            }
        } else if age.as_secs() < 86400 {
            let hours = age.as_secs() / 3600;
            if entry_count == 1 {
                format!("1 command · {}h ago", hours)
            } else {
                format!("{} commands · {}h ago", entry_count, hours)
            }
        } else {
            let days = age.as_secs() / 86400;
            if entry_count == 1 {
                format!("1 command · {}d ago", days)
            } else {
                format!("{} commands · {}d ago", entry_count, days)
            }
        };

        draw_text_at_buffered(
            buf,
            font_system,
            swash_cache,
            &mut detail_buf,
            pad_x,
            y + pad_y + (18.0 * sf) as usize,
            buf.height,
            &detail,
            detail_metrics,
            theme::FG_SECONDARY,
            Family::SansSerif,
        );

        let clear_x =
            (panel_w as f32 - ITEM_PAD_X * sf - clear_sz as f32 - border_w as f32) as usize;
        let clear_y = y + (item_h.saturating_sub(clear_sz as usize)) / 2;

        if is_hovered || is_clear_hovered {
            let clear_color = if is_clear_hovered {
                theme::ERROR
            } else {
                theme::FG_MUTED
            };
            icon_renderer.draw(buf, Icon::Trash, clear_x, clear_y, clear_sz, clear_color);
        }

        let item_divider_y = y + item_h - border_w;
        buf.fill_rect(
            pad_x,
            item_divider_y,
            panel_w.saturating_sub(pad_x * 2),
            border_w,
            theme::DIVIDER,
        );
    }
}

/// Hit-test the side panel. Returns what was clicked.
pub fn hit_test(
    phys_x: f64,
    phys_y: f64,
    session_count: usize,
    bar_h_phys: f64,
    sf: f64,
    scroll_offset: f32,
    panel_layout: &PanelLayout,
    sandbox_info: Option<&SandboxInfo>,
) -> SidePanelHit {
    let panel_w = panel_layout.left_width() as f64 * sf;
    if phys_x >= panel_w || phys_y <= bar_h_phys {
        return SidePanelHit::None;
    }

    let header_h = HEADER_HEIGHT as f64 * sf;

    if phys_y < bar_h_phys + header_h {
        return toolbar_hit(phys_x, phys_y, bar_h_phys, sf, sandbox_info.is_some());
    }

    if panel_layout.active_tab == SidePanelTab::Sandbox {
        let vol_count = sandbox_info.map(|i| i.volumes.len()).unwrap_or(0);
        return sandbox_hit_test(phys_x, phys_y, bar_h_phys, sf, panel_w, vol_count);
    }

    if panel_layout.active_tab != SidePanelTab::Sessions {
        return SidePanelHit::None;
    }

    let border_w = (1.0 * sf).max(1.0);
    let list_y = bar_h_phys + header_h + border_w;

    if phys_y < list_y {
        return SidePanelHit::None;
    }

    let item_h = ITEM_HEIGHT as f64 * sf;
    let rel_y = phys_y - list_y + scroll_offset as f64;
    let idx = (rel_y / item_h) as usize;

    if idx >= session_count {
        return SidePanelHit::None;
    }

    let pad_x = ITEM_PAD_X as f64 * sf;
    let clear_sz = CLEAR_BTN_SIZE as f64 * sf;
    let clear_x = panel_w - pad_x - clear_sz - border_w;
    if phys_x >= clear_x {
        return SidePanelHit::ClearSession(idx);
    }

    SidePanelHit::OpenSession(idx)
}

/// Hit-test the toolbar buttons in the header.
fn toolbar_hit(
    phys_x: f64,
    phys_y: f64,
    bar_h_phys: f64,
    sf: f64,
    has_sandbox: bool,
) -> SidePanelHit {
    let header_h = HEADER_HEIGHT as f64 * sf;
    let csz = TOOLBAR_BTN_CONTAINER as f64 * sf;
    let btn_gap = TOOLBAR_BTN_GAP as f64 * sf;
    let pad_x = TOOLBAR_PAD_X as f64 * sf;

    let cy = bar_h_phys + ((header_h - csz) / 2.0).max(0.0);
    if phys_y < cy || phys_y > cy + csz {
        return SidePanelHit::None;
    }

    let sessions_x = pad_x;
    if phys_x >= sessions_x && phys_x <= sessions_x + csz {
        return SidePanelHit::ToolbarSessions;
    }

    let files_x = sessions_x + csz + btn_gap;
    if phys_x >= files_x && phys_x <= files_x + csz {
        return SidePanelHit::ToolbarFiles;
    }

    let mut next_x = files_x + csz + btn_gap;

    if has_sandbox {
        if phys_x >= next_x && phys_x <= next_x + csz {
            return SidePanelHit::ToolbarSandbox;
        }
        next_x += csz + btn_gap;
    }

    if phys_x >= next_x && phys_x <= next_x + csz {
        return SidePanelHit::ToolbarSearch;
    }

    SidePanelHit::None
}

/// Hit-test sandbox panel content (stop button).
fn sandbox_hit_test(
    phys_x: f64,
    phys_y: f64,
    bar_h_phys: f64,
    sf: f64,
    panel_w: f64,
    volume_count: usize,
) -> SidePanelHit {
    let header_h = HEADER_HEIGHT as f64 * sf;
    let border_w = (1.0_f64 * sf).max(1.0);
    let content_y = bar_h_phys + header_h + border_w;

    let row_h = 36.0 * sf;
    let section_gap = 16.0 * sf;
    let top_pad = 12.0 * sf;
    let title_h = 22.0 * sf;
    let pad_x = 12.0 * sf;

    let mut y = content_y + top_pad + title_h; // after section title
    y += row_h * 3.0; // after 3 info rows (Image, CPU, Memory)

    if volume_count > 0 {
        y += section_gap; // gap before divider
        y += section_gap; // divider + gap
        y += 20.0 * sf; // "VOLUMES" title
        let vol_row_h = 44.0 * sf;
        y += vol_row_h * volume_count as f64; // volume rows
    }

    y += section_gap; // gap before button

    let btn_h = STOP_BTN_HEIGHT as f64 * sf;
    let btn_x = pad_x;
    let btn_w = panel_w - pad_x * 2.0;

    if phys_x >= btn_x && phys_x <= btn_x + btn_w && phys_y >= y && phys_y <= y + btn_h {
        return SidePanelHit::StopSandbox;
    }

    SidePanelHit::None
}

/// Compute hovered item and clear button indices.
/// Returns `true` if any hover state changed (for redraw gating).
pub fn update_hover(
    phys_x: f64,
    phys_y: f64,
    session_count: usize,
    bar_h_phys: f64,
    sf: f64,
    scroll_offset: f32,
    state: &mut SidePanelState,
    panel_layout: &PanelLayout,
    sandbox_info: Option<&SandboxInfo>,
) -> bool {
    let prev_item = state.hovered_item;
    let prev_clear = state.hovered_clear;
    let prev_toolbar = state.hovered_toolbar_btn;

    let panel_w = panel_layout.left_width() as f64 * sf;
    if phys_x >= panel_w || phys_y <= bar_h_phys {
        state.hovered_item = None;
        state.hovered_clear = None;
        state.hovered_toolbar_btn = None;
        return state.hovered_item != prev_item
            || state.hovered_clear != prev_clear
            || state.hovered_toolbar_btn != prev_toolbar;
    }

    let header_h = HEADER_HEIGHT as f64 * sf;

    if phys_y < bar_h_phys + header_h {
        state.hovered_item = None;
        state.hovered_clear = None;
        let hit = toolbar_hit(phys_x, phys_y, bar_h_phys, sf, sandbox_info.is_some());
        state.hovered_toolbar_btn = match hit {
            SidePanelHit::ToolbarSessions => Some(SidePanelTab::Sessions),
            SidePanelHit::ToolbarFiles => Some(SidePanelTab::Files),
            SidePanelHit::ToolbarSandbox => Some(SidePanelTab::Sandbox),
            SidePanelHit::ToolbarSearch => Some(SidePanelTab::Search),
            _ => None,
        };
        return state.hovered_item != prev_item
            || state.hovered_clear != prev_clear
            || state.hovered_toolbar_btn != prev_toolbar;
    }

    state.hovered_toolbar_btn = None;

    if panel_layout.active_tab == SidePanelTab::Sandbox {
        let prev_stop = state.sandbox.stop_hovered;
        state.hovered_item = None;
        state.hovered_clear = None;
        let vol_count = sandbox_info.map(|i| i.volumes.len()).unwrap_or(0);
        let hit = sandbox_hit_test(phys_x, phys_y, bar_h_phys, sf, panel_w, vol_count);
        state.sandbox.stop_hovered = hit == SidePanelHit::StopSandbox;
        return state.sandbox.stop_hovered != prev_stop
            || state.hovered_item != prev_item
            || state.hovered_clear != prev_clear
            || state.hovered_toolbar_btn != prev_toolbar;
    }

    if panel_layout.active_tab != SidePanelTab::Sessions {
        state.hovered_item = None;
        state.hovered_clear = None;
        return state.hovered_item != prev_item
            || state.hovered_clear != prev_clear
            || state.hovered_toolbar_btn != prev_toolbar;
    }

    let border_w = (1.0 * sf).max(1.0);
    let list_y = bar_h_phys + header_h + border_w;

    if phys_y < list_y {
        state.hovered_item = None;
        state.hovered_clear = None;
        return state.hovered_item != prev_item
            || state.hovered_clear != prev_clear
            || state.hovered_toolbar_btn != prev_toolbar;
    }

    let item_h = ITEM_HEIGHT as f64 * sf;
    let rel_y = phys_y - list_y + scroll_offset as f64;
    let idx = (rel_y / item_h) as usize;

    if idx >= session_count {
        state.hovered_item = None;
        state.hovered_clear = None;
        return state.hovered_item != prev_item
            || state.hovered_clear != prev_clear
            || state.hovered_toolbar_btn != prev_toolbar;
    }

    state.hovered_item = Some(idx);

    let pad_x = ITEM_PAD_X as f64 * sf;
    let clear_sz = CLEAR_BTN_SIZE as f64 * sf;
    let clear_x = panel_w - pad_x - clear_sz - border_w;
    state.hovered_clear = if phys_x >= clear_x { Some(idx) } else { None };

    state.hovered_item != prev_item
        || state.hovered_clear != prev_clear
        || state.hovered_toolbar_btn != prev_toolbar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_outside_panel() {
        let pl = PanelLayout::default();
        let hit = hit_test(300.0, 100.0, 5, 42.0, 1.0, 0.0, &pl, None);
        assert_eq!(hit, SidePanelHit::None);
    }

    #[test]
    fn hit_test_in_header_toolbar() {
        let pl = PanelLayout::default();
        let csz = TOOLBAR_BTN_CONTAINER as f64;
        let cy = 42.0 + ((HEADER_HEIGHT as f64 - csz) / 2.0);
        let btn_x = TOOLBAR_PAD_X as f64;
        let hit = hit_test(btn_x + 5.0, cy + 5.0, 5, 42.0, 1.0, 0.0, &pl, None);
        assert_eq!(hit, SidePanelHit::ToolbarSessions);
    }

    #[test]
    fn hit_test_first_item() {
        let pl = PanelLayout::default();
        let header_h = HEADER_HEIGHT as f64;
        let list_y = 42.0 + header_h + 1.0;
        let hit = hit_test(50.0, list_y + 5.0, 5, 42.0, 1.0, 0.0, &pl, None);
        assert_eq!(hit, SidePanelHit::OpenSession(0));
    }
}
