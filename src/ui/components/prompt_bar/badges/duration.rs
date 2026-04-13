//! Duration badge — shows elapsed time for the current/last command.

use crate::renderer::icons::Icon;
use crate::renderer::pixel_buffer::Rgb;

use super::{BadgeCtx, BadgeResult, draw_badge_with_icon};

const DURATION_TEXT: Rgb = (180, 180, 130);
const DURATION_BORDER: Rgb = (50, 50, 38);

/// Draw the duration badge right-aligned at `right_x`.
pub fn draw(ctx: &mut BadgeCtx<'_>, right_x: usize, label: &str) -> BadgeResult {
    let w = draw_badge_with_icon(
        ctx,
        right_x,
        Icon::Timer,
        label,
        DURATION_TEXT,
        DURATION_BORDER,
    );
    BadgeResult {
        consumed: w + ctx.gap,
        hit_rect: None,
    }
}
