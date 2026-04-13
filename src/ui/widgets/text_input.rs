use cosmic_text::Family;

use crate::renderer::pixel_buffer::Rgb;
use crate::renderer::theme;
use crate::ui::{DrawCtx, Rect, Widget};

pub struct TextInput<'a> {
    pub text: &'a str,
    pub placeholder: &'a str,
    pub cursor: usize,
    pub focused: bool,
    pub bg: Rgb,
    pub border: Rgb,
    pub focus_border: Rgb,
    pub text_color: Rgb,
    pub placeholder_color: Rgb,
    pub cursor_color: Rgb,
}

impl<'a> TextInput<'a> {
    pub fn new(text: &'a str, placeholder: &'a str) -> Self {
        Self {
            text,
            placeholder,
            cursor: 0,
            focused: false,
            bg: theme::SETTINGS_INPUT_BG,
            border: theme::SETTINGS_INPUT_BORDER,
            focus_border: theme::TAB_INDICATOR,
            text_color: theme::SETTINGS_INPUT_TEXT,
            placeholder_color: theme::PALETTE_DIM_TEXT,
            cursor_color: theme::TAB_INDICATOR,
        }
    }
}

impl Widget for TextInput<'_> {
    fn draw(&self, painter: &mut DrawCtx, rect: Rect) {
        let r = painter.px(4.0);
        let border_w = (1.0 * painter.sf).max(1.0);
        let border_color = if self.focused {
            self.focus_border
        } else {
            self.border
        };

        painter.fill_rounded_rect(rect, r, self.bg);
        painter.stroke_rounded_rect(rect, border_w, r, border_color);

        let pad_x = painter.px(8.0);
        let text_size = painter.px(13.0);
        let text_y = rect.y + (rect.h - text_size) / 2.0;

        if self.text.is_empty() && !self.focused {
            painter.text(
                self.placeholder,
                rect.x + pad_x,
                text_y,
                text_size,
                self.placeholder_color,
                Family::Monospace,
            );
        } else {
            if !self.text.is_empty() {
                painter.text(
                    self.text,
                    rect.x + pad_x,
                    text_y,
                    text_size,
                    self.text_color,
                    Family::Monospace,
                );
            }

            if self.focused {
                let before_cursor = &self.text[..self.cursor.min(self.text.len())];
                let cursor_x = if before_cursor.is_empty() {
                    rect.x + pad_x
                } else {
                    let w = painter.text_width(before_cursor, text_size, Family::Monospace);
                    rect.x + pad_x + w
                };
                let cursor_h = text_size * 1.1;
                let cursor_y = rect.y + (rect.h - cursor_h) / 2.0;
                painter.fill_rect(
                    Rect::new(cursor_x, cursor_y, (1.5 * painter.sf).max(1.0), cursor_h),
                    self.cursor_color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_new_defaults() {
        let ti = TextInput::new("hello", "placeholder");
        assert_eq!(ti.text, "hello");
        assert_eq!(ti.placeholder, "placeholder");
        assert_eq!(ti.cursor, 0);
        assert!(!ti.focused);
    }
}
