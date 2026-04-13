//! Toast notification manager.

use std::time::{Duration, Instant};

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

const TOAST_LOGICAL_W: f32 = 300.0;
const TOAST_LOGICAL_H: f32 = 48.0;
const TOAST_GAP: f32 = 8.0;
const TOAST_MARGIN_TOP: f32 = 50.0;
const TOAST_MARGIN_RIGHT: f32 = 16.0;
const TOAST_CORNER_R: f32 = 6.0;
const TOAST_AUTO_DISMISS: Duration = Duration::from_secs(4);
const MAX_VISIBLE: usize = 5;

/// Severity level for a toast notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    fn accent_color(self) -> (u8, u8, u8) {
        match self {
            ToastLevel::Info => theme::TOAST_INFO_ACCENT,
            ToastLevel::Success => theme::TOAST_SUCCESS_ACCENT,
            ToastLevel::Warning => theme::TOAST_WARNING_ACCENT,
            ToastLevel::Error => theme::TOAST_ERROR_ACCENT,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ToastLevel::Info => "info",
            ToastLevel::Success => "success",
            ToastLevel::Warning => "warning",
            ToastLevel::Error => "error",
        }
    }
}

/// A single toast notification.
struct Toast {
    message: String,
    level: ToastLevel,
    created_at: Instant,
}

/// Manages the toast queue — push, auto-dismiss, and render list.
pub struct ToastManager {
    toasts: Vec<Toast>,
}

impl ToastManager {
    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    /// Push a new toast notification.
    pub fn push(&mut self, message: String, level: ToastLevel) {
        self.toasts.push(Toast {
            message,
            level,
            created_at: Instant::now(),
        });
        while self.toasts.len() > MAX_VISIBLE * 2 {
            self.toasts.remove(0);
        }
    }

    /// Remove expired toasts. Returns `true` if any were removed (needs redraw).
    pub fn tick(&mut self) -> bool {
        let before = self.toasts.len();
        self.toasts
            .retain(|t| t.created_at.elapsed() < TOAST_AUTO_DISMISS);
        self.toasts.len() != before
    }

    /// Dismiss the toast at the given visible index (0 = most recent).
    pub fn dismiss_at(&mut self, visible_idx: usize) {
        let len = self.toasts.len();
        if visible_idx >= len.min(MAX_VISIBLE) {
            return;
        }
        let vec_idx = len - 1 - visible_idx;
        self.toasts.remove(vec_idx);
    }

    /// Whether there are any active toasts (used to schedule redraw).
    pub fn has_active(&self) -> bool {
        !self.toasts.is_empty()
    }

    /// Hit-test: returns the visible index of the toast under (mx, my), if any.
    pub fn hit_test(&self, mx: f64, my: f64, buf_w: usize, sf: f32) -> Option<usize> {
        if self.toasts.is_empty() {
            return None;
        }
        let toast_w = (TOAST_LOGICAL_W * sf) as usize;
        let toast_h = (TOAST_LOGICAL_H * sf) as usize;
        let gap = (TOAST_GAP * sf) as usize;
        let margin_top = (TOAST_MARGIN_TOP * sf) as usize;
        let margin_right = (TOAST_MARGIN_RIGHT * sf) as usize;
        let base_x = buf_w.saturating_sub(toast_w + margin_right);
        let count = self.toasts.len().min(MAX_VISIBLE);

        for i in 0..count {
            let y = margin_top + i * (toast_h + gap);
            if mx >= base_x as f64
                && mx < (base_x + toast_w) as f64
                && my >= y as f64
                && my < (y + toast_h) as f64
            {
                return Some(i);
            }
        }
        None
    }
}

/// Render active toasts in the top-right corner of the buffer.
pub fn draw_toasts(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    manager: &ToastManager,
    sf: f32,
) {
    if manager.toasts.is_empty() {
        return;
    }

    let w = buf.width;
    let toast_w = (TOAST_LOGICAL_W * sf) as usize;
    let toast_h = (TOAST_LOGICAL_H * sf) as usize;
    let gap = (TOAST_GAP * sf) as usize;
    let margin_top = (TOAST_MARGIN_TOP * sf) as usize;
    let margin_right = (TOAST_MARGIN_RIGHT * sf) as usize;
    let corner_r = (TOAST_CORNER_R * sf) as usize;

    let label_metrics = Metrics::new(10.0 * sf, 14.0 * sf);
    let msg_metrics = Metrics::new(12.0 * sf, 16.0 * sf);
    let pad_x = (12.0 * sf) as usize;

    let visible = manager.toasts.iter().rev().take(MAX_VISIBLE);
    let base_x = w.saturating_sub(toast_w + margin_right);

    for (i, toast) in visible.enumerate() {
        let y = margin_top + i * (toast_h + gap);
        if y + toast_h > buf.height {
            break;
        }

        crate::ui::components::overlay::fill_rounded_rect(
            buf,
            base_x,
            y,
            toast_w,
            toast_h,
            corner_r,
            theme::TOAST_BG,
        );

        let accent_color = toast.level.accent_color();
        let text_x = base_x + pad_x;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            text_x,
            y + (6.0 * sf) as usize,
            buf.height,
            toast.level.label(),
            label_metrics,
            accent_color,
            Family::Monospace,
        );

        let max_msg_chars = ((toast_w - pad_x * 2) as f32 / (7.0 * sf)) as usize;
        let display_msg = if toast.message.len() > max_msg_chars {
            format!("{}…", &toast.message[..max_msg_chars.saturating_sub(1)])
        } else {
            toast.message.clone()
        };
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            text_x,
            y + (24.0 * sf) as usize,
            buf.height,
            &display_msg,
            msg_metrics,
            theme::TOAST_TEXT,
            Family::Monospace,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_tick() {
        let mut mgr = ToastManager::new();
        assert!(!mgr.has_active());
        mgr.push("hello".into(), ToastLevel::Info);
        assert!(mgr.has_active());
        assert!(!mgr.tick());
        assert!(mgr.has_active());
    }

    #[test]
    fn level_colors_distinct() {
        let levels = [
            ToastLevel::Info,
            ToastLevel::Success,
            ToastLevel::Warning,
            ToastLevel::Error,
        ];
        for (i, a) in levels.iter().enumerate() {
            for (j, b) in levels.iter().enumerate() {
                if i != j {
                    assert_ne!(a.accent_color(), b.accent_color());
                }
            }
        }
    }

    #[test]
    fn level_labels() {
        assert_eq!(ToastLevel::Info.label(), "info");
        assert_eq!(ToastLevel::Success.label(), "success");
        assert_eq!(ToastLevel::Warning.label(), "warning");
        assert_eq!(ToastLevel::Error.label(), "error");
    }

    #[test]
    fn bounded_queue() {
        let mut mgr = ToastManager::new();
        for i in 0..20 {
            mgr.push(format!("msg {}", i), ToastLevel::Info);
        }
        assert!(mgr.toasts.len() <= MAX_VISIBLE * 2);
    }
}
