//! Shared infrastructure for prompt bar badges.
//!
//! Each badge is a small, self-contained drawing function that renders
//! a right-aligned segment in the prompt bar.  All badges share a
//! `BadgeCtx` for pixel buffer, font system, scale factor, and metrics.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::{Icon, IconRenderer};
use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{draw_text_at_bold, measure_text_width_bold};

pub mod diff_stat;
pub mod duration;
pub mod model;
pub mod stop;

/// Shared drawing context passed to every badge.
pub struct BadgeCtx<'a> {
    pub buf: &'a mut PixelBuffer,
    pub font_system: &'a mut FontSystem,
    pub swash_cache: &'a mut SwashCache,
    pub icon_renderer: &'a mut IconRenderer,
    pub sf: f32,
    /// Segment height in physical pixels.
    pub seg_h: usize,
    /// Horizontal padding inside each badge.
    pub pad_x: usize,
    /// Gap between badges.
    pub gap: usize,
    /// Corner radius for badge outlines.
    pub radius: usize,
    /// Font metrics for badge text.
    pub seg_metrics: Metrics,
    /// Y coordinate of the segment row.
    pub seg_y: usize,
}

/// Result of drawing a badge — how much horizontal space it consumed
/// and an optional hit-test rectangle.
pub struct BadgeResult {
    /// Total width consumed (badge width + gap).
    pub consumed: usize,
    /// Optional hit-test rect (x, y, w, h) in physical pixels.
    pub hit_rect: Option<(usize, usize, usize, usize)>,
}

/// Draw a badge with an SVG icon + bold label, right-aligned.
/// Returns the badge width.
pub fn draw_badge_with_icon(
    ctx: &mut BadgeCtx<'_>,
    right_x: usize,
    icon: Icon,
    label: &str,
    fg: Rgb,
    border: Rgb,
) -> usize {
    let icon_sz = (ctx.seg_h as f32 * 0.6).round() as u32;
    let icon_gap = (3.0 * ctx.sf) as usize;
    let text_w = measure_text_width_bold(ctx.font_system, label, ctx.seg_metrics, Family::Monospace).ceil() as usize;
    let seg_w = ctx.pad_x + icon_sz as usize + icon_gap + text_w + ctx.pad_x;
    let x = right_x.saturating_sub(seg_w);

    super::stroke_rounded_rect(ctx.buf, x, ctx.seg_y, seg_w, ctx.seg_h, ctx.radius, ctx.sf, border);

    let icon_y = ctx.seg_y + (ctx.seg_h.saturating_sub(icon_sz as usize)) / 2;
    ctx.icon_renderer.draw(ctx.buf, icon, x + ctx.pad_x, icon_y, icon_sz, fg);

    let text_x = x + ctx.pad_x + icon_sz as usize + icon_gap;
    let text_y = ctx.seg_y + ((ctx.seg_h as f32 - ctx.seg_metrics.line_height) / 2.0) as usize;
    draw_text_at_bold(
        ctx.buf,
        ctx.font_system,
        ctx.swash_cache,
        text_x,
        text_y,
        ctx.buf.height,
        label,
        ctx.seg_metrics,
        fg,
        Family::Monospace,
    );

    seg_w
}

/// Draw a text-only badge (no icon) right-aligned at `right_x`.
pub fn draw_badge_text(
    ctx: &mut BadgeCtx<'_>,
    right_x: usize,
    label: &str,
    fg: Rgb,
    border: Rgb,
) -> usize {
    let text_w = measure_text_width_bold(ctx.font_system, label, ctx.seg_metrics, Family::Monospace).ceil() as usize;
    let seg_w = ctx.pad_x + text_w + ctx.pad_x;
    let x = right_x.saturating_sub(seg_w);

    super::stroke_rounded_rect(ctx.buf, x, ctx.seg_y, seg_w, ctx.seg_h, ctx.radius, ctx.sf, border);

    let text_x = x + ctx.pad_x;
    let text_y = ctx.seg_y + ((ctx.seg_h as f32 - ctx.seg_metrics.line_height) / 2.0) as usize;
    draw_text_at_bold(
        ctx.buf,
        ctx.font_system,
        ctx.swash_cache,
        text_x,
        text_y,
        ctx.buf.height,
        label,
        ctx.seg_metrics,
        fg,
        Family::Monospace,
    );

    seg_w
}
