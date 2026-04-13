use cosmic_text::Family;

use crate::renderer::pixel_buffer::Rgb;
use crate::renderer::theme;
use crate::ui::widgets::button::{IconButton, IconKind};
use crate::ui::widgets::dropdown::{Dropdown, DropdownItem};
use crate::ui::widgets::text_input::TextInput;
use crate::ui::widgets::tooltip::Tooltip;
use crate::ui::Widget;
use crate::ui::{DrawCtx, Rect};

const VIEWER_BG: Rgb = (18, 18, 22);
const CARD_BG: Rgb = (25, 27, 33);
const CARD_BORDER: Rgb = (50, 52, 58);
const LABEL_FG: Rgb = (140, 142, 150);
const TITLE_FG: Rgb = (220, 220, 228);

pub struct DebugViewerState {
    pub open: bool,
}

impl DebugViewerState {
    pub fn new() -> Self {
        Self { open: false }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

pub fn draw(ctx: &mut DrawCtx, backend_label: &str) {
    let w = ctx.buf.width as f32;
    let h = ctx.buf.height as f32;
    ctx.fill_rect(Rect::new(0.0, 0.0, w, h), VIEWER_BG);

    let title_size = ctx.px(16.0);
    let label_size = ctx.px(11.0);
    let pad = ctx.px(20.0);
    let card_pad = ctx.px(12.0);
    let header_h = ctx.px(24.0);

    ctx.text(
        "Widget Debug Viewer",
        pad,
        pad,
        title_size,
        TITLE_FG,
        Family::Monospace,
    );
    let info_y = pad + title_size + ctx.px(4.0);
    ctx.text(
        "Ctrl+Shift+D to close",
        pad,
        info_y,
        label_size,
        LABEL_FG,
        Family::Monospace,
    );

    let backend_text = format!("Render backend: {backend_label}");
    let badge_y = info_y + label_size + ctx.px(8.0);
    let badge_h = ctx.px(22.0);
    let badge_pad_x = ctx.px(10.0);
    let badge_text_w = ctx.text_width(&backend_text, label_size, Family::Monospace);
    let badge_w = badge_text_w + badge_pad_x * 2.0;
    let badge_color: Rgb = if backend_label.contains("GPU") {
        (40, 167, 69)
    } else {
        (200, 150, 50)
    };
    let badge_rect = Rect::new(pad, badge_y, badge_w, badge_h);
    let badge_r = ctx.px(4.0);
    ctx.fill_rounded_rect(badge_rect, badge_r, badge_color);
    ctx.text(
        &backend_text,
        pad + badge_pad_x,
        badge_y + (badge_h - label_size) / 2.0,
        label_size,
        (255, 255, 255),
        Family::Monospace,
    );

    let start_y = badge_y + badge_h + ctx.px(16.0);
    let col_w = ((w - pad * 3.0) / 2.0).max(200.0);
    let mut x = pad;
    let mut y = start_y;
    let row_h = ctx.px(160.0);

    draw_card(
        ctx,
        x,
        y,
        col_w,
        row_h,
        card_pad,
        header_h,
        label_size,
        "IconButton (Close)",
        |ctx, rect| {
            let btn_size = ctx.px(24.0);
            let btn_rect = Rect::new(
                rect.center_x() - btn_size / 2.0 - ctx.px(30.0),
                rect.center_y() - btn_size / 2.0,
                btn_size,
                btn_size,
            );
            let normal = IconButton {
                hovered: false,
                icon: IconKind::Close,
                color: theme::TAB_CLOSE_NORMAL,
                hover_color: theme::TAB_CLOSE_HOVER,
                hover_bg: theme::TAB_CLOSE_HOVER_BG,
            };
            normal.draw(ctx, btn_rect);

            let hovered_rect = Rect::new(
                rect.center_x() - btn_size / 2.0 + ctx.px(30.0),
                rect.center_y() - btn_size / 2.0,
                btn_size,
                btn_size,
            );
            let hovered = IconButton {
                hovered: true,
                icon: IconKind::Close,
                color: theme::TAB_CLOSE_NORMAL,
                hover_color: theme::TAB_CLOSE_HOVER,
                hover_bg: theme::TAB_CLOSE_HOVER_BG,
            };
            hovered.draw(ctx, hovered_rect);

            let lbl_y = hovered_rect.bottom() + ctx.px(8.0);
            let s = ctx.px(9.0);
            ctx.text("normal", btn_rect.x, lbl_y, s, LABEL_FG, Family::Monospace);
            ctx.text(
                "hovered",
                hovered_rect.x,
                lbl_y,
                s,
                LABEL_FG,
                Family::Monospace,
            );
        },
    );

    x += col_w + pad;

    draw_card(
        ctx,
        x,
        y,
        col_w,
        row_h,
        card_pad,
        header_h,
        label_size,
        "IconButton (Chevron)",
        |ctx, rect| {
            let btn_size = ctx.px(24.0);
            let btn_rect = Rect::new(
                rect.center_x() - btn_size / 2.0 - ctx.px(30.0),
                rect.center_y() - btn_size / 2.0,
                btn_size,
                btn_size,
            );
            let normal = IconButton {
                hovered: false,
                icon: IconKind::Chevron,
                color: theme::TAB_CLOSE_NORMAL,
                hover_color: theme::AVATAR_ICON_HOVER,
                hover_bg: (40, 42, 48),
            };
            normal.draw(ctx, btn_rect);

            let hovered_rect = Rect::new(
                rect.center_x() - btn_size / 2.0 + ctx.px(30.0),
                rect.center_y() - btn_size / 2.0,
                btn_size,
                btn_size,
            );
            let hovered = IconButton {
                hovered: true,
                icon: IconKind::Chevron,
                color: theme::TAB_CLOSE_NORMAL,
                hover_color: theme::AVATAR_ICON_HOVER,
                hover_bg: (40, 42, 48),
            };
            hovered.draw(ctx, hovered_rect);

            let lbl_y = hovered_rect.bottom() + ctx.px(8.0);
            let s = ctx.px(9.0);
            ctx.text("normal", btn_rect.x, lbl_y, s, LABEL_FG, Family::Monospace);
            ctx.text(
                "hovered",
                hovered_rect.x,
                lbl_y,
                s,
                LABEL_FG,
                Family::Monospace,
            );
        },
    );

    x = pad;
    y += row_h + pad;

    draw_card(
        ctx,
        x,
        y,
        col_w,
        row_h,
        card_pad,
        header_h,
        label_size,
        "Tooltip",
        |ctx, rect| {
            let tip = Tooltip::new("Settings");
            let tw = ctx.text_width("Settings", ctx.px(11.0), Family::Monospace) + ctx.px(12.0);
            let th = ctx.px(24.0);
            let tip_rect = Rect::new(
                rect.center_x() - tw / 2.0,
                rect.center_y() - th / 2.0,
                tw,
                th,
            );
            tip.draw(ctx, tip_rect);
        },
    );

    x += col_w + pad;

    draw_card(
        ctx,
        x,
        y,
        col_w,
        row_h,
        card_pad,
        header_h,
        label_size,
        "TextInput",
        |ctx, rect| {
            let input_h = ctx.px(32.0);
            let input_w = rect.w - ctx.px(20.0);

            let unfocused_rect = Rect::new(
                rect.center_x() - input_w / 2.0,
                rect.y + ctx.px(8.0),
                input_w,
                input_h,
            );
            let unfocused = TextInput::new("", "Ask something...");
            unfocused.draw(ctx, unfocused_rect);

            let focused_rect = Rect::new(
                rect.center_x() - input_w / 2.0,
                unfocused_rect.bottom() + ctx.px(8.0),
                input_w,
                input_h,
            );
            let mut focused = TextInput::new("Hello world", "");
            focused.focused = true;
            focused.cursor = 5;
            focused.draw(ctx, focused_rect);

            let s = ctx.px(9.0);
            ctx.text(
                "unfocused",
                unfocused_rect.x,
                unfocused_rect.bottom() + ctx.px(2.0),
                s,
                LABEL_FG,
                Family::Monospace,
            );
            ctx.text(
                "focused",
                focused_rect.x,
                focused_rect.bottom() + ctx.px(2.0),
                s,
                LABEL_FG,
                Family::Monospace,
            );
        },
    );

    x = pad;
    y += row_h + pad;
    let dropdown_row_h = ctx.px(220.0);

    draw_card(
        ctx,
        x,
        y,
        col_w,
        dropdown_row_h,
        card_pad,
        header_h,
        label_size,
        "Dropdown",
        |ctx, rect| {
            let items = [
                DropdownItem {
                    label: "SmolLM3 3B",
                    detail: "Q4_K_M",
                    active: false,
                },
                DropdownItem {
                    label: "Phi-4 Mini",
                    detail: "Q4_K_M",
                    active: true,
                },
                DropdownItem {
                    label: "Qwen3 4B",
                    detail: "Q4_K_M",
                    active: false,
                },
                DropdownItem {
                    label: "Gemma 4 E2B",
                    detail: "Q8_0",
                    active: false,
                },
            ];
            let mut dd = Dropdown::new(&items);
            dd.hovered = Some(2);
            let dd_w = rect.w - ctx.px(16.0);
            let item_h = dd.item_height(ctx.sf);
            let dd_h = items.len() as f32 * item_h;
            let dd_rect = Rect::new(
                rect.center_x() - dd_w / 2.0,
                rect.y + ctx.px(4.0),
                dd_w,
                dd_h,
            );
            dd.draw(ctx, dd_rect);
        },
    );
}

fn draw_card(
    ctx: &mut DrawCtx,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    card_pad: f32,
    header_h: f32,
    label_size: f32,
    title: &str,
    content_fn: impl FnOnce(&mut DrawCtx, Rect),
) {
    let card = Rect::new(x, y, w, h);
    let r = ctx.px(6.0);
    let border_w = (1.0 * ctx.sf).max(1.0);

    ctx.fill_rounded_rect(card, r, CARD_BG);
    ctx.stroke_rounded_rect(card, border_w, r, CARD_BORDER);

    ctx.text(
        title,
        x + card_pad,
        y + (header_h - label_size) / 2.0,
        label_size,
        LABEL_FG,
        Family::Monospace,
    );

    let divider_y = y + header_h;
    ctx.fill_rect(Rect::new(x, divider_y, w, border_w), CARD_BORDER);

    let content_rect = Rect::new(
        x + card_pad,
        divider_y + border_w + card_pad,
        w - card_pad * 2.0,
        h - header_h - border_w - card_pad * 2.0,
    );
    content_fn(ctx, content_rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_viewer_new_closed() {
        let s = DebugViewerState::new();
        assert!(!s.open);
    }

    #[test]
    fn debug_viewer_toggle() {
        let mut s = DebugViewerState::new();
        s.toggle();
        assert!(s.open);
        s.toggle();
        assert!(!s.open);
    }
}
