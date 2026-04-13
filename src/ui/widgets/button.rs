use crate::renderer::pixel_buffer::Rgb;
use crate::ui::layout;
use crate::ui::{DrawCtx, Rect, Widget};

pub struct IconButton {
    pub hovered: bool,
    pub icon: IconKind,
    pub color: Rgb,
    pub hover_color: Rgb,
    pub hover_bg: Rgb,
}

pub enum IconKind {
    Close,
    Chevron,
}

impl Widget for IconButton {
    fn draw(&self, painter: &mut DrawCtx, rect: Rect) {
        let center = rect.center();
        let radius = rect.w.min(rect.h) / 2.0;

        if self.hovered {
            painter.fill_circle(center, radius, self.hover_bg);
        }

        let color = if self.hovered {
            self.hover_color
        } else {
            self.color
        };

        match self.icon {
            IconKind::Close => {
                let arm = radius * 0.45;
                let thickness = (1.4 * painter.sf).max(1.0);
                painter.draw_line(
                    layout::Point::new(center.x - arm, center.y - arm),
                    layout::Point::new(center.x + arm, center.y + arm),
                    thickness,
                    color,
                );
                painter.draw_line(
                    layout::Point::new(center.x + arm, center.y - arm),
                    layout::Point::new(center.x - arm, center.y + arm),
                    thickness,
                    color,
                );
            }
            IconKind::Chevron => {
                let arm = radius * 0.35;
                let thickness = (1.4 * painter.sf).max(1.0);
                painter.draw_line(
                    layout::Point::new(center.x - arm, center.y - arm * 0.5),
                    layout::Point::new(center.x, center.y + arm * 0.5),
                    thickness,
                    color,
                );
                painter.draw_line(
                    layout::Point::new(center.x, center.y + arm * 0.5),
                    layout::Point::new(center.x + arm, center.y - arm * 0.5),
                    thickness,
                    color,
                );
            }
        }
    }
}
