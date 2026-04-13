use crate::renderer::pixel_buffer::Rgb;
use cosmic_text::{
    Attrs, Buffer, Color as CColor, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap,
};

use crate::renderer::pixel_buffer::PixelBuffer;

use super::layout::{Point, Rect};

pub struct DrawCtx<'a> {
    pub buf: &'a mut PixelBuffer,
    pub font: &'a mut FontSystem,
    pub cache: &'a mut SwashCache,
    pub sf: f32,
}

impl<'a> DrawCtx<'a> {
    pub fn new(
        buf: &'a mut PixelBuffer,
        font: &'a mut FontSystem,
        cache: &'a mut SwashCache,
        sf: f32,
    ) -> Self {
        Self {
            buf,
            font,
            cache,
            sf,
        }
    }

    pub fn px(&self, logical: f32) -> f32 {
        logical * self.sf
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Rgb) {
        self.buf.fill_rect(
            rect.x as usize,
            rect.y as usize,
            rect.w as usize,
            rect.h as usize,
            color,
        );
    }

    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Rgb) {
        crate::ui::components::overlay::fill_rounded_rect(
            self.buf,
            rect.x as usize,
            rect.y as usize,
            rect.w as usize,
            rect.h as usize,
            radius as usize,
            color,
        );
    }

    pub fn stroke_rounded_rect(&mut self, rect: Rect, border: f32, radius: f32, color: Rgb) {
        crate::ui::components::overlay::draw_border_rounded(
            self.buf,
            rect.x as usize,
            rect.y as usize,
            rect.w as usize,
            rect.h as usize,
            border as usize,
            radius as usize,
            color,
        );
    }

    pub fn fill_circle(&mut self, center: Point, radius: f32, color: Rgb) {
        self.buf.fill_circle(center.x, center.y, radius, color);
    }

    pub fn draw_line(&mut self, from: Point, to: Point, thickness: f32, color: Rgb) {
        self.buf
            .draw_line_aa(from.x, from.y, to.x, to.y, thickness, color);
    }

    pub fn text(&mut self, text: &str, x: f32, y: f32, size: f32, color: Rgb, family: Family<'_>) {
        let metrics = Metrics::new(size, size * 1.2);
        let mut buffer = Buffer::new(self.font, metrics);
        buffer.set_size(self.font, Some(800.0), Some(metrics.line_height));
        buffer.set_wrap(self.font, Wrap::None);
        buffer.set_text(
            self.font,
            text,
            &Attrs::new().family(family),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(self.font, true);

        let (cr, cg, cb) = color;
        let buf_w = self.buf.width;
        let buf_h = self.buf.height;
        let x_off = x as usize;
        let y_off = y as usize;

        buffer.draw(
            self.font,
            self.cache,
            CColor::rgba(cr, cg, cb, 255),
            |gx, gy, _gw, _gh, c| {
                let px = gx + x_off as i32;
                let py = gy + y_off as i32;
                if px < 0 || py < 0 {
                    return;
                }
                let px = px as usize;
                let py = py as usize;
                if px >= buf_w || py >= buf_h {
                    return;
                }
                let a = c.a();
                if a == 0 {
                    return;
                }
                self.buf.blend_pixel(px, py, color, a as f32 / 255.0);
            },
        );
    }

    pub fn text_width(&mut self, text: &str, size: f32, family: Family<'_>) -> f32 {
        let metrics = Metrics::new(size, size * 1.2);
        let mut buffer = Buffer::new(self.font, metrics);
        buffer.set_size(self.font, Some(4096.0), Some(metrics.line_height));
        buffer.set_wrap(self.font, Wrap::None);
        buffer.set_text(
            self.font,
            text,
            &Attrs::new().family(family),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(self.font, true);
        buffer.layout_runs().map(|r| r.line_w).next().unwrap_or(0.0)
    }
}
