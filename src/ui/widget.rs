use super::draw_ctx::DrawCtx;
use super::layout::Rect;

/// Rendering trait for UI components.
///
/// Any struct implementing `Widget` can draw itself into a `DrawCtx`
/// within a given `Rect` bounding box.
pub trait Widget {
    fn draw(&self, painter: &mut DrawCtx, rect: Rect);
}
