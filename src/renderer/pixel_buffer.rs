//! CPU pixel buffer abstraction for software-rasterized rendering.
//!
//! Encapsulates raw pixel data with BGRA/RGBA awareness and provides
//! primitive drawing operations used by all sub-renderers.

/// A CPU-side pixel buffer that knows its own dimensions and byte order.
pub struct PixelBuffer {
    pub data: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub is_bgra: bool,
    clear_pixel: [u8; 4],
    dirty_min_y: usize,
    dirty_max_y: usize,
}

/// RGB color tuple used throughout the renderer.
pub type Rgb = (u8, u8, u8);

impl PixelBuffer {
    pub fn new(width: usize, height: usize, is_bgra: bool, bg: Rgb) -> Self {
        let (r, g, b) = bg;
        let clear_pixel = if is_bgra {
            [b, g, r, 255]
        } else {
            [r, g, b, 255]
        };
        let mut data = vec![0u8; width * height * 4];
        let row_bytes = width * 4;
        for i in 0..width {
            let off = i * 4;
            data[off] = clear_pixel[0];
            data[off + 1] = clear_pixel[1];
            data[off + 2] = clear_pixel[2];
            data[off + 3] = clear_pixel[3];
        }
        for row in 1..height {
            data.copy_within(0..row_bytes, row * row_bytes);
        }

        Self {
            data,
            width,
            height,
            is_bgra,
            clear_pixel,
            dirty_min_y: 0,
            dirty_max_y: height.saturating_sub(1),
        }
    }

    pub fn clear(&mut self, bg: Rgb) {
        let (r, g, b) = bg;
        self.clear_pixel = if self.is_bgra {
            [b, g, r, 255]
        } else {
            [r, g, b, 255]
        };
        let pixel_u32 = u32::from_ne_bytes(self.clear_pixel);
        let buf: &mut [u32] = unsafe {
            std::slice::from_raw_parts_mut(self.data.as_mut_ptr() as *mut u32, self.data.len() / 4)
        };
        buf.fill(pixel_u32);
        self.dirty_min_y = 0;
        self.dirty_max_y = self.height.saturating_sub(1);
    }

    pub fn ensure_size(&mut self, width: usize, height: usize, bg: Rgb) {
        let resized = self.width != width || self.height != height;
        if resized {
            self.width = width;
            self.height = height;
            self.data.resize(width * height * 4, 0);
            self.clear(bg);
        } else {
            self.reset_dirty();
        }
    }

    /// Reset dirty region tracking. Call at the start of a frame.
    pub fn reset_dirty(&mut self) {
        self.dirty_min_y = usize::MAX;
        self.dirty_max_y = 0;
    }

    /// Returns the dirty pixel row range `(min_y, max_y)` inclusive,
    /// or `None` if no pixels were touched since the last `reset_dirty()`.
    pub fn dirty_range(&self) -> Option<(usize, usize)> {
        if self.dirty_min_y <= self.dirty_max_y && self.dirty_max_y < self.height {
            Some((self.dirty_min_y, self.dirty_max_y))
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn mark_dirty(&mut self, y_start: usize, y_end: usize) {
        if y_start < self.dirty_min_y {
            self.dirty_min_y = y_start;
        }
        if y_end > self.dirty_max_y {
            self.dirty_max_y = y_end;
        }
    }
    #[inline]
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Rgb) {
        let (r, g, b) = color;
        let buf_w = self.width;
        let is_bgra = self.is_bgra;

        let x_end = (x + w).min(buf_w);
        let y_end = (y + h).min(self.height);

        if x >= x_end || y >= y_end {
            return;
        }

        self.mark_dirty(y, y_end.saturating_sub(1));
        let pixels = &mut self.data;

        let span = x_end - x;

        let first_row_start = y * buf_w * 4 + x * 4;
        if is_bgra {
            for col in 0..span {
                let idx = first_row_start + col * 4;
                pixels[idx] = b;
                pixels[idx + 1] = g;
                pixels[idx + 2] = r;
                pixels[idx + 3] = 255;
            }
        } else {
            for col in 0..span {
                let idx = first_row_start + col * 4;
                pixels[idx] = r;
                pixels[idx + 1] = g;
                pixels[idx + 2] = b;
                pixels[idx + 3] = 255;
            }
        }

        let span_bytes = span * 4;
        for row in (y + 1)..y_end {
            let dst = row * buf_w * 4 + x * 4;
            pixels.copy_within(first_row_start..first_row_start + span_bytes, dst);
        }
    }

    #[inline]
    pub fn blend_pixel(&mut self, px: usize, py: usize, color: Rgb, alpha: f32) {
        if px >= self.width || py >= self.height {
            return;
        }
        self.mark_dirty(py, py);
        let idx = (py * self.width + px) * 4;
        let (cr, cg, cb) = color;
        let inv = 1.0 - alpha;

        if self.is_bgra {
            self.data[idx] = (cb as f32 * alpha + self.data[idx] as f32 * inv) as u8;
            self.data[idx + 1] = (cg as f32 * alpha + self.data[idx + 1] as f32 * inv) as u8;
            self.data[idx + 2] = (cr as f32 * alpha + self.data[idx + 2] as f32 * inv) as u8;
        } else {
            self.data[idx] = (cr as f32 * alpha + self.data[idx] as f32 * inv) as u8;
            self.data[idx + 1] = (cg as f32 * alpha + self.data[idx + 1] as f32 * inv) as u8;
            self.data[idx + 2] = (cb as f32 * alpha + self.data[idx + 2] as f32 * inv) as u8;
        }
        self.data[idx + 3] = 255;
    }

    pub fn dim(&mut self, factor: f32) {
        self.mark_dirty(0, self.height.saturating_sub(1));
        let keep = 1.0 - factor;
        for chunk in self.data.chunks_exact_mut(4) {
            chunk[0] = (chunk[0] as f32 * keep) as u8;
            chunk[1] = (chunk[1] as f32 * keep) as u8;
            chunk[2] = (chunk[2] as f32 * keep) as u8;
        }
    }

    pub fn draw_line_aa(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, color: Rgb) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return;
        }

        let nx = -dy / len;
        let ny = dx / len;

        let half = thickness / 2.0;
        let min_x = x0.min(x1) - half - 1.0;
        let max_x = x0.max(x1) + half + 1.0;
        let min_y = y0.min(y1) - half - 1.0;
        let max_y = y0.max(y1) + half + 1.0;

        let px_min = (min_x.floor() as isize).max(0) as usize;
        let px_max = (max_x.ceil() as usize).min(self.width);
        let py_min = (min_y.floor() as isize).max(0) as usize;
        let py_max = (max_y.ceil() as usize).min(self.height);

        for py in py_min..py_max {
            for px in px_min..px_max {
                let cx = px as f32 + 0.5;
                let cy = py as f32 + 0.5;

                let rel_x = cx - x0;
                let rel_y = cy - y0;
                let along = rel_x * (dx / len) + rel_y * (dy / len);
                let perp = (rel_x * nx + rel_y * ny).abs();

                if along < -half && along > len + half {
                    continue;
                }

                let dist_perp = (perp - half).max(0.0);
                let dist_along = if along < 0.0 {
                    -along
                } else if along > len {
                    along - len
                } else {
                    0.0
                };
                let dist = (dist_perp * dist_perp + dist_along * dist_along).sqrt();
                let alpha = (1.0 - dist).clamp(0.0, 1.0);

                if alpha > 0.0 {
                    self.blend_pixel(px, py, color, alpha);
                }
            }
        }
    }

    pub fn fill_circle(&mut self, cx: f32, cy: f32, radius: f32, color: Rgb) {
        let min_x = ((cx - radius - 1.0).floor() as isize).max(0) as usize;
        let max_x = ((cx + radius + 1.0).ceil() as usize).min(self.width);
        let min_y = ((cy - radius - 1.0).floor() as isize).max(0) as usize;
        let max_y = ((cy + radius + 1.0).ceil() as usize).min(self.height);

        for py in min_y..max_y {
            for px in min_x..max_x {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let alpha = (radius + 0.5 - dist).clamp(0.0, 1.0);
                if alpha > 0.0 {
                    self.blend_pixel(px, py, color, alpha);
                }
            }
        }
    }

    #[cfg(test)]
    pub fn stroke_circle(&mut self, cx: f32, cy: f32, radius: f32, thickness: f32, color: Rgb) {
        let half = thickness / 2.0;
        let min_x = ((cx - radius - half - 1.0).floor() as isize).max(0) as usize;
        let max_x = ((cx + radius + half + 1.0).ceil() as usize).min(self.width);
        let min_y = ((cy - radius - half - 1.0).floor() as isize).max(0) as usize;
        let max_y = ((cy + radius + half + 1.0).ceil() as usize).min(self.height);

        for py in min_y..max_y {
            for px in min_x..max_x {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let ring_dist = (dist - radius).abs();
                let alpha = (half + 0.5 - ring_dist).clamp(0.0, 1.0);
                if alpha > 0.0 {
                    self.blend_pixel(px, py, color, alpha);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_correct_dimensions() {
        let buf = PixelBuffer::new(10, 5, false, (0, 0, 0));
        assert_eq!(buf.width, 10);
        assert_eq!(buf.height, 5);
        assert_eq!(buf.data.len(), 10 * 5 * 4);
    }

    #[test]
    fn new_fills_with_bg_color_rgba() {
        let buf = PixelBuffer::new(2, 2, false, (255, 128, 0));
        assert_eq!(buf.data[0], 255);
        assert_eq!(buf.data[1], 128);
        assert_eq!(buf.data[2], 0);
        assert_eq!(buf.data[3], 255);
    }

    #[test]
    fn new_fills_with_bg_color_bgra() {
        let buf = PixelBuffer::new(2, 2, true, (255, 128, 0));
        assert_eq!(buf.data[0], 0);
        assert_eq!(buf.data[1], 128);
        assert_eq!(buf.data[2], 255);
        assert_eq!(buf.data[3], 255);
    }

    #[test]
    fn clear_overwrites_all_pixels() {
        let mut buf = PixelBuffer::new(3, 3, false, (0, 0, 0));
        buf.clear((100, 200, 50));
        for y in 0..3 {
            for x in 0..3 {
                let idx = (y * 3 + x) * 4;
                assert_eq!(buf.data[idx], 100);
                assert_eq!(buf.data[idx + 1], 200);
                assert_eq!(buf.data[idx + 2], 50);
                assert_eq!(buf.data[idx + 3], 255);
            }
        }
    }

    #[test]
    fn ensure_size_resizes() {
        let mut buf = PixelBuffer::new(2, 2, false, (0, 0, 0));
        buf.ensure_size(4, 4, (10, 20, 30));
        assert_eq!(buf.width, 4);
        assert_eq!(buf.height, 4);
        assert_eq!(buf.data.len(), 4 * 4 * 4);
    }

    #[test]
    fn fill_rect_basic() {
        let mut buf = PixelBuffer::new(10, 10, false, (0, 0, 0));
        buf.fill_rect(2, 2, 3, 3, (255, 0, 0));
        let idx = (2 * 10 + 2) * 4;
        assert_eq!(buf.data[idx], 255);
        assert_eq!(buf.data[idx + 1], 0);
        assert_eq!(buf.data[idx + 2], 0);

        let outside = (0 * 10 + 0) * 4;
        assert_eq!(buf.data[outside], 0);
    }

    #[test]
    fn fill_rect_clamps_to_bounds() {
        let mut buf = PixelBuffer::new(5, 5, false, (0, 0, 0));
        buf.fill_rect(3, 3, 10, 10, (255, 255, 255));
        assert_eq!(buf.data.len(), 5 * 5 * 4);
    }

    #[test]
    fn fill_rect_out_of_bounds_noop() {
        let mut buf = PixelBuffer::new(5, 5, false, (0, 0, 0));
        buf.fill_rect(10, 10, 5, 5, (255, 0, 0));
        assert!(buf.data.iter().all(|&b| b == 0 || b == 255));
    }

    #[test]
    fn blend_pixel_full_alpha() {
        let mut buf = PixelBuffer::new(2, 2, false, (0, 0, 0));
        buf.blend_pixel(0, 0, (200, 100, 50), 1.0);
        assert_eq!(buf.data[0], 200);
        assert_eq!(buf.data[1], 100);
        assert_eq!(buf.data[2], 50);
    }

    #[test]
    fn blend_pixel_zero_alpha() {
        let mut buf = PixelBuffer::new(2, 2, false, (100, 100, 100));
        buf.blend_pixel(0, 0, (255, 0, 0), 0.0);
        assert_eq!(buf.data[0], 100);
        assert_eq!(buf.data[1], 100);
        assert_eq!(buf.data[2], 100);
    }

    #[test]
    fn blend_pixel_half_alpha() {
        let mut buf = PixelBuffer::new(2, 2, false, (0, 0, 0));
        buf.blend_pixel(0, 0, (200, 100, 50), 0.5);
        assert_eq!(buf.data[0], 100);
        assert_eq!(buf.data[1], 50);
        assert_eq!(buf.data[2], 25);
    }

    #[test]
    fn blend_pixel_out_of_bounds_noop() {
        let mut buf = PixelBuffer::new(2, 2, false, (0, 0, 0));
        buf.blend_pixel(5, 5, (255, 0, 0), 1.0);
    }

    #[test]
    fn dim_reduces_values() {
        let mut buf = PixelBuffer::new(1, 1, false, (200, 100, 50));
        buf.dim(0.5);
        assert_eq!(buf.data[0], 100);
        assert_eq!(buf.data[1], 50);
        assert_eq!(buf.data[2], 25);
    }

    #[test]
    fn fill_circle_center_pixel() {
        let mut buf = PixelBuffer::new(10, 10, false, (0, 0, 0));
        buf.fill_circle(5.0, 5.0, 2.0, (255, 0, 0));
        let center_idx = (5 * 10 + 5) * 4;
        assert!(buf.data[center_idx] > 200);
    }

    #[test]
    fn stroke_circle_does_not_fill_center() {
        let mut buf = PixelBuffer::new(20, 20, false, (0, 0, 0));
        buf.stroke_circle(10.0, 10.0, 6.0, 1.0, (255, 0, 0));
        let center_idx = (10 * 20 + 10) * 4;
        assert_eq!(buf.data[center_idx], 0);
    }

    #[test]
    fn draw_line_aa_horizontal() {
        let mut buf = PixelBuffer::new(20, 5, false, (0, 0, 0));
        buf.draw_line_aa(2.0, 2.5, 18.0, 2.5, 1.5, (255, 255, 255));
        let mid = (2 * 20 + 10) * 4;
        assert!(buf.data[mid] > 0);
    }

    #[test]
    fn dirty_range_after_new_covers_all() {
        let buf = PixelBuffer::new(10, 10, false, (0, 0, 0));
        assert_eq!(buf.dirty_range(), Some((0, 9)));
    }

    #[test]
    fn dirty_range_after_reset_is_none() {
        let mut buf = PixelBuffer::new(10, 10, false, (0, 0, 0));
        buf.reset_dirty();
        assert_eq!(buf.dirty_range(), None);
    }

    #[test]
    fn dirty_range_tracks_fill_rect() {
        let mut buf = PixelBuffer::new(10, 10, false, (0, 0, 0));
        buf.reset_dirty();
        buf.fill_rect(0, 3, 5, 2, (255, 0, 0));
        assert_eq!(buf.dirty_range(), Some((3, 4)));
    }

    #[test]
    fn dirty_range_tracks_blend_pixel() {
        let mut buf = PixelBuffer::new(10, 10, false, (0, 0, 0));
        buf.reset_dirty();
        buf.blend_pixel(5, 7, (255, 0, 0), 0.5);
        assert_eq!(buf.dirty_range(), Some((7, 7)));
    }

    #[test]
    fn dirty_range_expands_with_multiple_draws() {
        let mut buf = PixelBuffer::new(20, 20, false, (0, 0, 0));
        buf.reset_dirty();
        buf.fill_rect(0, 2, 5, 1, (255, 0, 0));
        buf.fill_rect(0, 15, 5, 3, (0, 255, 0));
        assert_eq!(buf.dirty_range(), Some((2, 17)));
    }

    #[test]
    fn ensure_size_no_resize_resets_dirty() {
        let mut buf = PixelBuffer::new(10, 10, false, (0, 0, 0));
        buf.ensure_size(10, 10, (0, 0, 0));
        assert_eq!(buf.dirty_range(), None);
    }

    #[test]
    fn ensure_size_resize_marks_all_dirty() {
        let mut buf = PixelBuffer::new(10, 10, false, (0, 0, 0));
        buf.ensure_size(20, 15, (0, 0, 0));
        assert_eq!(buf.dirty_range(), Some((0, 14)));
    }
}
