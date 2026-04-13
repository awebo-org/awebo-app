//! Animated thinking indicator — subtle bouncing dots.
//!
//! Three tiny dots that gently bounce up/down in sequence, like a
//! modern agent "thinking" animation. Drawn directly to the pixel buffer.

use crate::renderer::pixel_buffer::PixelBuffer;

/// Number of dots.
const DOT_COUNT: usize = 3;
/// Dot radius in logical pixels.
const DOT_R: f32 = 1.5;
/// Horizontal gap between dot centres in logical pixels.
const DOT_GAP: f32 = 4.0;
/// Peak vertical bounce offset in logical pixels.
const BOUNCE_PX: f32 = 2.5;
/// Full cycle duration in ms.
const CYCLE_MS: f32 = 1400.0;
/// Phase delay between consecutive dots (fraction of cycle).
const PHASE_STEP: f32 = 0.18;

/// Total width of the indicator in physical pixels.
pub fn width(sf: f32) -> usize {
    let d = DOT_R * 2.0 * sf;
    let g = DOT_GAP * sf;
    (d * DOT_COUNT as f32 + g * (DOT_COUNT - 1) as f32).ceil() as usize
}

/// Total height (includes bounce room) in physical pixels.
pub fn height(sf: f32) -> usize {
    ((DOT_R * 2.0 + BOUNCE_PX) * sf).ceil() as usize
}

/// Draw the bouncing dots at `(x, y)`. `y` is the vertical centre line.
pub fn draw(
    buf: &mut PixelBuffer,
    x: usize,
    y: usize,
    sf: f32,
    elapsed_ms: u128,
    color: (u8, u8, u8),
) {
    let r = (DOT_R * sf).round().max(1.0);
    let gap = (DOT_GAP * sf).round();
    let bounce = BOUNCE_PX * sf;
    let t_global = (elapsed_ms as f32 % CYCLE_MS) / CYCLE_MS;

    for i in 0..DOT_COUNT {
        let t = (t_global - PHASE_STEP * i as f32).rem_euclid(1.0);

        let (dy, alpha) = if t < 0.4 {
            let u = t / 0.4;
            let ease = (u * std::f32::consts::PI).sin(); // 0→1→0
            (-bounce * ease, 0.45 + 0.55 * ease)
        } else {
            (0.0, 0.45)
        };

        let cx = x as f32 + r + i as f32 * (r * 2.0 + gap);
        let cy = y as f32 + r + bounce + dy;
        draw_dot(buf, cx, cy, r, color, alpha);
    }
}

/// Rasterise one anti-aliased filled circle blended onto the buffer.
fn draw_dot(
    buf: &mut PixelBuffer,
    cx: f32,
    cy: f32,
    radius: f32,
    color: (u8, u8, u8),
    base_alpha: f32,
) {
    let x0 = (cx - radius - 1.0).max(0.0) as usize;
    let y0 = (cy - radius - 1.0).max(0.0) as usize;
    let x1 = ((cx + radius + 1.0) as usize + 1).min(buf.width);
    let y1 = ((cy + radius + 1.0) as usize + 1).min(buf.height);

    let buf_w = buf.width;
    let is_bgra = buf.is_bgra;
    buf.mark_dirty(y0, y1.saturating_sub(1));

    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > radius + 0.5 {
                continue;
            }
            let edge = (radius + 0.5 - dist).min(1.0);
            let alpha = base_alpha * edge;
            let inv = 1.0 - alpha;

            let idx = (py * buf_w + px) * 4;
            if is_bgra {
                buf.data[idx]     = (color.2 as f32 * alpha + buf.data[idx]     as f32 * inv) as u8;
                buf.data[idx + 1] = (color.1 as f32 * alpha + buf.data[idx + 1] as f32 * inv) as u8;
                buf.data[idx + 2] = (color.0 as f32 * alpha + buf.data[idx + 2] as f32 * inv) as u8;
            } else {
                buf.data[idx]     = (color.0 as f32 * alpha + buf.data[idx]     as f32 * inv) as u8;
                buf.data[idx + 1] = (color.1 as f32 * alpha + buf.data[idx + 1] as f32 * inv) as u8;
                buf.data[idx + 2] = (color.2 as f32 * alpha + buf.data[idx + 2] as f32 * inv) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_height_positive() {
        assert!(width(1.0) > 0);
        assert!(height(1.0) > 0);
        assert!(width(2.0) > width(1.0));
    }
}
