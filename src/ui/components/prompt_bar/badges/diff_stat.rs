//! Git diff stat badge — shows lines added / removed in the working tree.

use crate::renderer::pixel_buffer::Rgb;
use crate::renderer::theme;

use super::{BadgeCtx, BadgeResult};
use crate::renderer::text::{draw_text_at_bold, measure_text_width_bold};
use cosmic_text::Family;

/// GitHub-style git green for additions.
const ADD_FG: Rgb = (63, 185, 80);
/// GitHub-style git red for deletions.
const DEL_FG: Rgb = (248, 81, 73);
const DIFF_BORDER: Rgb = theme::BORDER;

/// Draw the diff stat badge right-aligned at `right_x`.
/// Shows `+N -M` with additions in green and deletions in red.
pub fn draw(
    ctx: &mut BadgeCtx<'_>,
    right_x: usize,
    additions: usize,
    deletions: usize,
) -> BadgeResult {
    if additions == 0 && deletions == 0 {
        return BadgeResult {
            consumed: 0,
            hit_rect: None,
        };
    }

    let add_label = format!("+{additions}");
    let del_label = format!("-{deletions}");
    let inner_gap = (4.0 * ctx.sf) as usize;
    let add_w = measure_text_width_bold(ctx.font_system, &add_label, ctx.seg_metrics, Family::Monospace).ceil() as usize;
    let del_w = measure_text_width_bold(ctx.font_system, &del_label, ctx.seg_metrics, Family::Monospace).ceil() as usize;
    let content_w = add_w + inner_gap + del_w;
    let seg_w = content_w + ctx.pad_x * 2;
    let x = right_x.saturating_sub(seg_w);

    super::super::stroke_rounded_rect(
        ctx.buf, x, ctx.seg_y, seg_w, ctx.seg_h, ctx.radius, ctx.sf, DIFF_BORDER,
    );

    let text_y = ctx.seg_y + ((ctx.seg_h as f32 - ctx.seg_metrics.line_height) / 2.0) as usize;

    draw_text_at_bold(
        ctx.buf,
        ctx.font_system,
        ctx.swash_cache,
        x + ctx.pad_x,
        text_y,
        ctx.buf.height,
        &add_label,
        ctx.seg_metrics,
        ADD_FG,
        Family::Monospace,
    );

    draw_text_at_bold(
        ctx.buf,
        ctx.font_system,
        ctx.swash_cache,
        x + ctx.pad_x + add_w + inner_gap,
        text_y,
        ctx.buf.height,
        &del_label,
        ctx.seg_metrics,
        DEL_FG,
        Family::Monospace,
    );

    BadgeResult {
        consumed: seg_w + ctx.gap,
        hit_rect: None,
    }
}
