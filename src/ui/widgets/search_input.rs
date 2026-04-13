//! Search input widget — a styled text input with placeholder, blinking cursor,
//! and magnifying-glass prefix icon. Works with the [`DrawCtx`] / [`Widget`] system.

use cosmic_text::Family;

use crate::renderer::pixel_buffer::Rgb;
use crate::renderer::theme;
use crate::ui::{DrawCtx, Rect, Widget};

/// Visual configuration for a [`SearchInput`].
#[derive(Debug, Clone, Copy)]
pub struct SearchInputStyle {
    pub bg: Rgb,
    pub border: Rgb,
    pub focus_border: Rgb,
    pub text_color: Rgb,
    pub placeholder_color: Rgb,
    pub cursor_color: Rgb,
    pub radius: f32,
    pub font_size: f32,
    pub padding_x: f32,
}

impl Default for SearchInputStyle {
    fn default() -> Self {
        Self {
            bg: theme::BG_ELEVATED,
            border: theme::BORDER,
            focus_border: theme::PRIMARY,
            text_color: theme::FG_PRIMARY,
            placeholder_color: theme::FG_DIM,
            cursor_color: theme::FG_PRIMARY,
            radius: 6.0,
            font_size: 13.0,
            padding_x: 12.0,
        }
    }
}

/// A search-bar input widget with a "⌕" prefix icon, placeholder text,
/// and a blinking cursor.
pub struct SearchInput<'a> {
    pub text: &'a str,
    pub placeholder: &'a str,
    pub focused: bool,
    pub cursor_visible: bool,
    pub style: SearchInputStyle,
}

impl<'a> SearchInput<'a> {
    pub fn new(text: &'a str, placeholder: &'a str) -> Self {
        Self {
            text,
            placeholder,
            focused: true,
            cursor_visible: true,
            style: SearchInputStyle::default(),
        }
    }

    pub fn cursor_visible(mut self, visible: bool) -> Self {
        self.cursor_visible = visible;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn style(mut self, style: SearchInputStyle) -> Self {
        self.style = style;
        self
    }
}

impl Widget for SearchInput<'_> {
    fn draw(&self, painter: &mut DrawCtx, rect: Rect) {
        let s = &self.style;
        let r = painter.px(s.radius);
        let bw = (1.0 * painter.sf).max(1.0);

        painter.fill_rounded_rect(rect, r, s.bg);

        let border_color = if self.focused {
            s.focus_border
        } else {
            s.border
        };
        painter.stroke_rounded_rect(rect, bw, r, border_color);

        let pad_x = painter.px(s.padding_x);
        let font_sz = painter.px(s.font_size);

        let text_x = rect.x + pad_x;
        let text_y = rect.y + (rect.h - font_sz) / 2.0;

        if self.text.is_empty() {
            painter.text(
                self.placeholder,
                text_x,
                text_y,
                font_sz,
                s.placeholder_color,
                Family::SansSerif,
            );

            if self.focused && self.cursor_visible {
                let cursor_h = font_sz * 1.15;
                let cursor_w = (1.5 * painter.sf).max(1.0);
                let cursor_y = rect.y + (rect.h - cursor_h) / 2.0;
                painter.fill_rect(
                    Rect::new(text_x, cursor_y, cursor_w, cursor_h),
                    s.cursor_color,
                );
            }
        } else {
            painter.text(
                self.text,
                text_x,
                text_y,
                font_sz,
                s.text_color,
                Family::SansSerif,
            );

            if self.focused && self.cursor_visible {
                let tw = painter.text_width(self.text, font_sz, Family::SansSerif);
                let cursor_x = text_x + tw + painter.px(2.0);
                let cursor_h = font_sz * 1.15;
                let cursor_w = (1.5 * painter.sf).max(1.0);
                let cursor_y = rect.y + (rect.h - cursor_h) / 2.0;
                painter.fill_rect(
                    Rect::new(cursor_x, cursor_y, cursor_w, cursor_h),
                    s.cursor_color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_input_defaults() {
        let si = SearchInput::new("hello", "Search...");
        assert_eq!(si.text, "hello");
        assert_eq!(si.placeholder, "Search...");
        assert!(si.focused);
        assert!(si.cursor_visible);
    }

    #[test]
    fn search_input_builder_chain() {
        let si = SearchInput::new("", "Type here")
            .focused(false)
            .cursor_visible(false);
        assert!(!si.focused);
        assert!(!si.cursor_visible);
    }

    #[test]
    fn search_input_custom_style() {
        let style = SearchInputStyle {
            bg: (10, 20, 30),
            ..Default::default()
        };
        let si = SearchInput::new("test", "").style(style);
        assert_eq!(si.style.bg, (10, 20, 30));
        assert_eq!(si.style.border, theme::BORDER);
    }
}
