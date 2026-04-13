//! Stop inference button badge.

use crate::renderer::icons::Icon;
use crate::renderer::pixel_buffer::Rgb;
use crate::renderer::theme;

use super::{BadgeCtx, BadgeResult, draw_badge_with_icon};

const STOP_FG: Rgb = theme::ERROR;
const STOP_BORDER: Rgb = (120, 40, 40);

/// Draw the stop button badge right-aligned at `right_x`.
/// Only draws when `ai_thinking` is true; returns zero consumed otherwise.
pub fn draw(ctx: &mut BadgeCtx<'_>, right_x: usize, ai_thinking: bool) -> BadgeResult {
    if !ai_thinking {
        return BadgeResult {
            consumed: 0,
            hit_rect: None,
        };
    }

    let w = draw_badge_with_icon(ctx, right_x, Icon::Stop, "Stop", STOP_FG, STOP_BORDER);
    let x = right_x.saturating_sub(w);
    BadgeResult {
        consumed: w + ctx.gap,
        hit_rect: Some((x, ctx.seg_y, w, ctx.seg_h)),
    }
}
