/// Shared text rendering helper using cosmic-text.
use cosmic_text::{
    Attrs, Buffer, Color as CColor, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap,
};

use super::pixel_buffer::{PixelBuffer, Rgb};

/// Measure the rendered pixel width of a text string using cosmic-text layout.
pub fn measure_text_width(
    font_system: &mut FontSystem,
    text: &str,
    metrics: Metrics,
    family: Family<'_>,
) -> f32 {
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(2000.0), Some(metrics.line_height));
    buffer.set_wrap(font_system, Wrap::None);
    buffer.set_text(
        font_system,
        text,
        &Attrs::new().family(family),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(font_system, true);

    buffer
        .layout_runs()
        .next()
        .map(|run| run.glyphs.iter().map(|g| g.w).sum::<f32>())
        .unwrap_or(0.0)
}

/// Internal text rendering with configurable shaping mode and weight.
fn draw_text_impl(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text_buf: &mut Buffer,
    x_off: usize,
    y_off: usize,
    clip_h: usize,
    text: &str,
    metrics: Metrics,
    color: Rgb,
    family: Family<'_>,
    shaping: Shaping,
    weight: Weight,
) {
    text_buf.set_metrics(font_system, metrics);
    text_buf.set_size(font_system, Some(4096.0), Some(metrics.line_height));
    text_buf.set_wrap(font_system, Wrap::None);
    text_buf.set_text(
        font_system,
        text,
        &Attrs::new().family(family).weight(weight),
        shaping,
        None,
    );
    text_buf.shape_until_scroll(font_system, true);

    let (cr, cg, cb) = color;
    let buf_w = buf.width;
    let buf_h = buf.height;

    text_buf.draw(
        font_system,
        swash_cache,
        CColor::rgba(cr, cg, cb, 255),
        |x, y, _gw, _gh, c| {
            let px = x + x_off as i32;
            let py = y + y_off as i32;
            if px < 0 || py < 0 {
                return;
            }
            let px = px as usize;
            let py = py as usize;
            if px >= buf_w || py >= buf_h || py >= clip_h {
                return;
            }
            let a = c.a();
            if a == 0 {
                return;
            }
            buf.blend_pixel(px, py, color, a as f32 / 255.0);
        },
    );
}

/// Draw text reusing an existing cosmic-text `Buffer` to avoid per-call allocation.
/// Uses `Shaping::Simple` (no HarfBuzz) for maximum performance in hot loops.
///
/// Call this in tight loops (e.g. block_renderer output lines) instead of
/// `draw_text_at` which creates a fresh `Buffer` every call.
pub fn draw_text_at_buffered(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text_buf: &mut Buffer,
    x_off: usize,
    y_off: usize,
    clip_h: usize,
    text: &str,
    metrics: Metrics,
    color: Rgb,
    family: Family<'_>,
) {
    draw_text_impl(
        buf,
        font_system,
        swash_cache,
        text_buf,
        x_off,
        y_off,
        clip_h,
        text,
        metrics,
        color,
        family,
        Shaping::Basic,
        Weight::NORMAL,
    );
}

/// Draw text creating a fresh Buffer. Uses `Shaping::Advanced` (HarfBuzz).
/// Suitable for non-hot-path text (overlays, tab bar, prompts).
pub fn draw_text_at(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x_off: usize,
    y_off: usize,
    clip_h: usize,
    text: &str,
    metrics: Metrics,
    color: Rgb,
    family: Family<'_>,
) {
    let mut buffer = Buffer::new(font_system, metrics);
    draw_text_impl(
        buf,
        font_system,
        swash_cache,
        &mut buffer,
        x_off,
        y_off,
        clip_h,
        text,
        metrics,
        color,
        family,
        Shaping::Advanced,
        Weight::NORMAL,
    );
}

/// Draw bold text creating a fresh Buffer. Uses `Shaping::Advanced` (HarfBuzz).
pub fn draw_text_at_bold(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    x_off: usize,
    y_off: usize,
    clip_h: usize,
    text: &str,
    metrics: Metrics,
    color: Rgb,
    family: Family<'_>,
) {
    let mut buffer = Buffer::new(font_system, metrics);
    draw_text_impl(
        buf,
        font_system,
        swash_cache,
        &mut buffer,
        x_off,
        y_off,
        clip_h,
        text,
        metrics,
        color,
        family,
        Shaping::Advanced,
        Weight::BOLD,
    );
}

/// Draw bold text reusing an existing `Buffer`. Uses `Shaping::Basic` for speed.
pub fn draw_text_at_bold_buffered(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text_buf: &mut Buffer,
    x_off: usize,
    y_off: usize,
    clip_h: usize,
    text: &str,
    metrics: Metrics,
    color: Rgb,
    family: Family<'_>,
) {
    draw_text_impl(
        buf,
        font_system,
        swash_cache,
        text_buf,
        x_off,
        y_off,
        clip_h,
        text,
        metrics,
        color,
        family,
        Shaping::Basic,
        Weight::BOLD,
    );
}

/// Measure the rendered pixel width of bold text.
pub fn measure_text_width_bold(
    font_system: &mut FontSystem,
    text: &str,
    metrics: Metrics,
    family: Family<'_>,
) -> f32 {
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(2000.0), Some(metrics.line_height));
    buffer.set_wrap(font_system, Wrap::None);
    buffer.set_text(
        font_system,
        text,
        &Attrs::new().family(family).weight(Weight::BOLD),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(font_system, true);

    buffer
        .layout_runs()
        .next()
        .map(|run| run.glyphs.iter().map(|g| g.w).sum::<f32>())
        .unwrap_or(0.0)
}
