//! AI model status badge.

use crate::renderer::pixel_buffer::Rgb;
use crate::renderer::theme;

use super::{BadgeCtx, BadgeResult, draw_badge_text};

const MODEL_LOADED_FG: Rgb = theme::FG_SECONDARY;
const MODEL_NONE_FG: Rgb = theme::FG_DIM;
const MODEL_BORDER: Rgb = theme::BORDER;
const MODEL_THINKING_FG: Rgb = theme::PRIMARY;
const MODEL_THINKING_BORDER: Rgb = (80, 55, 100);

/// Draw the AI model badge right-aligned at `right_x`.
pub fn draw(
    ctx: &mut BadgeCtx<'_>,
    right_x: usize,
    model_name: Option<&str>,
    ai_thinking: bool,
) -> BadgeResult {
    let (label, fg, border) = if ai_thinking {
        let elapsed_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let dots = match (elapsed_ms / 400) % 3 {
            0 => ".",
            1 => "..",
            _ => "...",
        };
        let name = model_name.unwrap_or("AI");
        (
            format!("{name} {dots}"),
            MODEL_THINKING_FG,
            MODEL_THINKING_BORDER,
        )
    } else {
        match model_name {
            Some(name) => (name.to_string(), MODEL_LOADED_FG, MODEL_BORDER),
            None => (
                "auto (best-efficient)".to_string(),
                MODEL_NONE_FG,
                MODEL_BORDER,
            ),
        }
    };

    let w = draw_badge_text(ctx, right_x, &label, fg, border);
    BadgeResult {
        consumed: w + ctx.gap,
        hit_rect: None,
    }
}
