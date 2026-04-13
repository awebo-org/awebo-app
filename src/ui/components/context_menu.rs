//! Context menu component for right-click dropdowns.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

const MENU_MIN_W: f32 = 160.0;
const ITEM_H: f32 = 28.0;
const SEPARATOR_H: f32 = 9.0;
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 4.0;
const FONT_SIZE: f32 = 12.0;
const LINE_HEIGHT: f32 = 17.0;
const BORDER_W: f32 = 1.0;
const CORNER_R: f32 = 6.0;

const BG: Rgb = (30, 30, 34);
const BORDER: Rgb = (55, 55, 62);
const HOVER_BG: Rgb = (50, 50, 58);
const TEXT_COLOR: Rgb = theme::FG_PRIMARY;
const TEXT_DISABLED: Rgb = theme::FG_DIM;
const TEXT_DESTRUCTIVE: Rgb = (220, 80, 80);
const SEPARATOR_COLOR: Rgb = (45, 45, 52);

/// A single item in a context menu.
#[derive(Debug, Clone)]
pub enum ContextMenuItem {
    /// A clickable action with a label.
    Action {
        label: String,
        id: String,
        destructive: bool,
        disabled: bool,
    },
    /// A visual separator line.
    Separator,
}

impl ContextMenuItem {
    /// Shorthand for a normal action.
    pub fn action(id: &str, label: &str) -> Self {
        Self::Action {
            label: label.into(),
            id: id.into(),
            destructive: false,
            disabled: false,
        }
    }

    /// Shorthand for a disabled (greyed-out) action.
    pub fn disabled(id: &str, label: &str) -> Self {
        Self::Action {
            label: label.into(),
            id: id.into(),
            destructive: false,
            disabled: true,
        }
    }

    /// Shorthand for a destructive (red) action.
    pub fn destructive(id: &str, label: &str) -> Self {
        Self::Action {
            label: label.into(),
            id: id.into(),
            destructive: true,
            disabled: false,
        }
    }
}

/// Transient state for an open context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    /// Items to show.
    pub items: Vec<ContextMenuItem>,
    /// Anchor position (physical pixels).
    pub anchor_x: usize,
    pub anchor_y: usize,
    /// Index of the hovered action item (skipping separators).
    pub hovered: Option<usize>,
}

impl ContextMenuState {
    pub fn new(items: Vec<ContextMenuItem>, anchor_x: usize, anchor_y: usize) -> Self {
        Self {
            items,
            anchor_x,
            anchor_y,
            hovered: None,
        }
    }
}

/// Compute the menu rect (x, y, w, h) in physical pixels,
/// clamped to fit within the buffer bounds.
fn menu_rect(
    state: &ContextMenuState,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> (usize, usize, usize, usize) {
    let pad_y = (PAD_Y * sf) as usize;
    let item_h = (ITEM_H * sf) as usize;
    let sep_h = (SEPARATOR_H * sf) as usize;
    let menu_w = (MENU_MIN_W * sf) as usize;

    let mut total_h = pad_y * 2;
    for item in &state.items {
        total_h += match item {
            ContextMenuItem::Action { .. } => item_h,
            ContextMenuItem::Separator => sep_h,
        };
    }

    let x = state.anchor_x.min(buf_w.saturating_sub(menu_w));
    let y = if state.anchor_y + total_h > buf_h {
        state.anchor_y.saturating_sub(total_h)
    } else {
        state.anchor_y
    };

    (x, y, menu_w, total_h)
}

/// Draw a context menu.
pub fn draw_context_menu(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    state: &ContextMenuState,
    sf: f32,
) {
    let (mx, my, mw, mh) = menu_rect(state, buf.width, buf.height, sf);
    let bw = (BORDER_W * sf).max(1.0) as usize;
    let cr = (CORNER_R * sf) as usize;
    let pad_y = (PAD_Y * sf) as usize;
    let pad_x = (PAD_X * sf) as usize;
    let item_h = (ITEM_H * sf) as usize;
    let sep_h = (SEPARATOR_H * sf) as usize;
    let font_size = FONT_SIZE * sf;

    crate::ui::components::overlay::fill_rounded_rect(buf, mx, my, mw, mh, cr, BG);
    crate::ui::components::overlay::draw_border_rounded(buf, mx, my, mw, mh, bw, cr, BORDER);

    let metrics = Metrics::new(font_size, LINE_HEIGHT * sf);

    let mut y = my + pad_y;
    let mut action_idx = 0usize;

    for item in &state.items {
        match item {
            ContextMenuItem::Action {
                label,
                destructive,
                disabled,
                ..
            } => {
                let is_hovered = !disabled && state.hovered == Some(action_idx);

                if is_hovered {
                    let hover_x = mx + bw;
                    let hover_w = mw.saturating_sub(bw * 2);
                    buf.fill_rect(hover_x, y, hover_w, item_h, HOVER_BG);
                }

                let text_color = if *disabled {
                    TEXT_DISABLED
                } else if *destructive {
                    TEXT_DESTRUCTIVE
                } else if is_hovered {
                    theme::FG_BRIGHT
                } else {
                    TEXT_COLOR
                };

                let text_y = y + ((item_h as f32 - LINE_HEIGHT * sf) / 2.0) as usize;
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    mx + pad_x,
                    text_y,
                    buf.height,
                    label,
                    metrics,
                    text_color,
                    Family::SansSerif,
                );

                if !*disabled {
                    action_idx += 1;
                }
                y += item_h;
            }
            ContextMenuItem::Separator => {
                let sep_y = y + sep_h / 2;
                buf.fill_rect(
                    mx + pad_x,
                    sep_y,
                    mw.saturating_sub(pad_x * 2),
                    1,
                    SEPARATOR_COLOR,
                );
                y += sep_h;
            }
        }
    }
}

/// Hit-test a click against the context menu.
/// Returns `Some(action_id)` if an enabled action was clicked, `None` if outside.
pub fn context_menu_hit_test(
    state: &ContextMenuState,
    px: usize,
    py: usize,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> Option<String> {
    let (mx, my, mw, mh) = menu_rect(state, buf_w, buf_h, sf);

    if px < mx || px >= mx + mw || py < my || py >= my + mh {
        return None;
    }

    let pad_y = (PAD_Y * sf) as usize;
    let item_h = (ITEM_H * sf) as usize;
    let sep_h = (SEPARATOR_H * sf) as usize;

    let mut y = my + pad_y;
    for item in &state.items {
        match item {
            ContextMenuItem::Action { id, disabled, .. } => {
                if py >= y && py < y + item_h && !disabled {
                    return Some(id.clone());
                }
                y += item_h;
            }
            ContextMenuItem::Separator => {
                y += sep_h;
            }
        }
    }
    None
}

/// Hover-test: returns which action index (0-based, skipping separators/disabled) the mouse is over.
pub fn context_menu_hover_test(
    state: &ContextMenuState,
    px: usize,
    py: usize,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> Option<usize> {
    let (mx, my, mw, mh) = menu_rect(state, buf_w, buf_h, sf);

    if px < mx || px >= mx + mw || py < my || py >= my + mh {
        return None;
    }

    let pad_y = (PAD_Y * sf) as usize;
    let item_h = (ITEM_H * sf) as usize;
    let sep_h = (SEPARATOR_H * sf) as usize;

    let mut y = my + pad_y;
    let mut action_idx = 0usize;
    for item in &state.items {
        match item {
            ContextMenuItem::Action { disabled, .. } => {
                if py >= y && py < y + item_h && !disabled {
                    return Some(action_idx);
                }
                if !disabled {
                    action_idx += 1;
                }
                y += item_h;
            }
            ContextMenuItem::Separator => {
                y += sep_h;
            }
        }
    }
    None
}

/// Returns true if point is inside the menu rect (for backdrop detection).
pub fn is_inside_menu(
    state: &ContextMenuState,
    px: usize,
    py: usize,
    buf_w: usize,
    buf_h: usize,
    sf: f32,
) -> bool {
    let (mx, my, mw, mh) = menu_rect(state, buf_w, buf_h, sf);
    px >= mx && px < mx + mw && py >= my && py < my + mh
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Map a flat index (0..action_count) to the actual items-array index.
    fn action_index_to_item_idx(items: &[ContextMenuItem], action_idx: usize) -> Option<usize> {
        let mut count = 0;
        for (i, item) in items.iter().enumerate() {
            if matches!(
                item,
                ContextMenuItem::Action {
                    disabled: false,
                    ..
                }
            ) {
                if count == action_idx {
                    return Some(i);
                }
                count += 1;
            }
        }
        None
    }

    fn sample_items() -> Vec<ContextMenuItem> {
        vec![
            ContextMenuItem::action("new_file", "New File"),
            ContextMenuItem::action("new_folder", "New Folder"),
            ContextMenuItem::Separator,
            ContextMenuItem::action("rename", "Rename"),
            ContextMenuItem::destructive("delete", "Delete"),
        ]
    }

    #[test]
    fn menu_rect_clamps_to_screen() {
        let state = ContextMenuState::new(sample_items(), 2000, 100);
        let (x, _y, _w, _h) = menu_rect(&state, 800, 600, 1.0);
        assert!(x + (MENU_MIN_W as usize) <= 800);
    }

    #[test]
    fn hover_test_outside_returns_none() {
        let state = ContextMenuState::new(sample_items(), 100, 100);
        assert!(context_menu_hover_test(&state, 0, 0, 800, 600, 1.0).is_none());
    }

    #[test]
    fn hit_test_outside_returns_none() {
        let state = ContextMenuState::new(sample_items(), 100, 100);
        assert!(context_menu_hit_test(&state, 0, 0, 800, 600, 1.0).is_none());
    }

    #[test]
    fn action_index_mapping() {
        let items = sample_items();
        assert_eq!(action_index_to_item_idx(&items, 0), Some(0));
        assert_eq!(action_index_to_item_idx(&items, 1), Some(1));
        assert_eq!(action_index_to_item_idx(&items, 2), Some(3));
        assert_eq!(action_index_to_item_idx(&items, 3), Some(4));
    }
}
