use cosmic_text::Family;

use crate::renderer::pixel_buffer::Rgb;
use crate::renderer::theme;
use crate::ui::{DrawCtx, Rect, Widget};

pub struct Tooltip<'a> {
    pub text: &'a str,
    pub bg: Rgb,
    pub fg: Rgb,
    pub border: Rgb,
}

impl<'a> Tooltip<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            bg: theme::BG_ELEVATED,
            fg: theme::FG_PRIMARY,
            border: theme::BORDER,
        }
    }
}

impl Widget for Tooltip<'_> {
    fn draw(&self, painter: &mut DrawCtx, rect: Rect) {
        let r = painter.px(4.0);
        painter.fill_rounded_rect(rect, r, self.bg);
        painter.stroke_rounded_rect(rect, 1.0, r, self.border);
        let pad = painter.px(6.0);
        let text_size = painter.px(11.0);
        painter.text(
            self.text,
            rect.x + pad,
            rect.y + (rect.h - text_size) / 2.0,
            text_size,
            self.fg,
            Family::Monospace,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_new_defaults() {
        let t = Tooltip::new("tip text");
        assert_eq!(t.text, "tip text");
    }
}
