use cosmic_text::Family;

use crate::renderer::pixel_buffer::Rgb;
use crate::renderer::theme;
use crate::ui::{DrawCtx, Rect, Widget};

pub struct DropdownItem<'a> {
    pub label: &'a str,
    pub detail: &'a str,
    pub active: bool,
}

pub struct Dropdown<'a> {
    pub items: &'a [DropdownItem<'a>],
    pub hovered: Option<usize>,
    pub bg: Rgb,
    pub border: Rgb,
    pub item_fg: Rgb,
    pub detail_fg: Rgb,
    pub hover_bg: Rgb,
    pub active_fg: Rgb,
}

impl<'a> Dropdown<'a> {
    pub fn new(items: &'a [DropdownItem<'a>]) -> Self {
        Self {
            items,
            hovered: None,
            bg: theme::SHELL_PICKER_BG,
            border: theme::SHELL_PICKER_BORDER,
            item_fg: theme::SHELL_PICKER_TEXT,
            detail_fg: theme::PALETTE_DIM_TEXT,
            hover_bg: theme::SHELL_PICKER_HOVER,
            active_fg: theme::TAB_INDICATOR,
        }
    }

    pub fn item_height(&self, sf: f32) -> f32 {
        (28.0 * sf).round()
    }
}

impl Widget for Dropdown<'_> {
    fn draw(&self, painter: &mut DrawCtx, rect: Rect) {
        let r = painter.px(6.0);
        let border_w = (1.0 * painter.sf).max(1.0);
        painter.fill_rounded_rect(rect, r, self.bg);
        painter.stroke_rounded_rect(rect, border_w, r, self.border);

        let item_h = self.item_height(painter.sf);
        let pad_x = painter.px(10.0);
        let text_size = painter.px(12.0);
        let detail_size = painter.px(10.0);

        for (i, item) in self.items.iter().enumerate() {
            let iy = rect.y + i as f32 * item_h;
            let item_rect = Rect::new(rect.x, iy, rect.w, item_h);

            if self.hovered == Some(i) {
                let inset = Rect::new(
                    item_rect.x + border_w,
                    item_rect.y,
                    item_rect.w - border_w * 2.0,
                    item_rect.h,
                );
                painter.fill_rect(inset, self.hover_bg);
            }

            let fg = if item.active {
                self.active_fg
            } else {
                self.item_fg
            };
            painter.text(
                item.label,
                item_rect.x + pad_x,
                item_rect.y + (item_h - text_size) / 2.0,
                text_size,
                fg,
                Family::Monospace,
            );

            if !item.detail.is_empty() {
                let label_w = painter.text_width(item.label, text_size, Family::Monospace);
                painter.text(
                    item.detail,
                    item_rect.x + pad_x + label_w + painter.px(8.0),
                    item_rect.y + (item_h - detail_size) / 2.0 + painter.px(1.0),
                    detail_size,
                    self.detail_fg,
                    Family::Monospace,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropdown_new_defaults() {
        let items = [];
        let d = Dropdown::new(&items);
        assert!(d.hovered.is_none());
        assert_eq!(d.items.len(), 0);
    }

    #[test]
    fn dropdown_item_height_scales() {
        let items = [];
        let d = Dropdown::new(&items);
        let h1 = d.item_height(1.0);
        let h2 = d.item_height(2.0);
        assert!(h2 > h1);
    }
}
