/// color theme and ANSI color resolution.
///
/// All UI colors derive from a single accent constant — `PRIMARY`
/// — plus a pure-black background.  Change it here to re-skin the entire app.
///
/// Provides the default terminal color palette and resolves ANSI colors
/// against TUI app overrides from `alacritty_terminal::term::color::Colors`.
use alacritty_terminal::term::color::Colors as TermColors;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};

use super::pixel_buffer::Rgb;

/// Primary accent — used for active indicators, focused elements, interactive highlights.
pub const PRIMARY: Rgb = (219, 39, 119);

pub const BG: Rgb = (0, 0, 0);
/// The color `NamedColor::Background` resolves to (used for bg_override remapping).
pub const fn named_bg() -> Rgb { (13, 13, 15) }
/// Slightly lifted surface (status bar, panels, popups).
pub const BG_SURFACE: Rgb = (12, 12, 14);
/// Even more lifted (active tabs, input fields).
pub const BG_ELEVATED: Rgb = (22, 22, 26);
/// Selection / hover highlight.
pub const BG_SELECTION: Rgb = (36, 20, 28);
/// Hover on interactive elements.
pub const BG_HOVER: Rgb = (28, 28, 34);

pub const FG_PRIMARY: Rgb = (200, 200, 210);
pub const FG_BRIGHT: Rgb = (228, 228, 233);
pub const FG_SECONDARY: Rgb = (120, 120, 135);
pub const FG_MUTED: Rgb = (70, 72, 80);
pub const FG_DIM: Rgb = (90, 92, 100);

pub const ERROR: Rgb = (200, 60, 60);
pub const ERROR_BG: Rgb = (40, 18, 18);
pub const ERROR_TEXT: Rgb = (200, 100, 100);
pub const WARNING: Rgb = (210, 160, 60);
pub const SUCCESS: Rgb = (90, 182, 90);

pub const BORDER: Rgb = (38, 38, 44);
pub const DIVIDER: Rgb = (32, 32, 38);

pub const TAB_BAR_BG: Rgb = BG;
pub const TAB_ACTIVE_BG: Rgb = BG_ELEVATED;
pub const TAB_ACTIVE_TEXT: Rgb = FG_BRIGHT;
pub const TAB_INACTIVE_TEXT: Rgb = FG_DIM;
pub const TAB_SEPARATOR: Rgb = BORDER;
pub const TAB_INDICATOR: Rgb = PRIMARY;
pub const TAB_CLOSE_NORMAL: Rgb = FG_DIM;
pub const TAB_CLOSE_HOVER: Rgb = ERROR;
pub const TAB_CLOSE_HOVER_BG: Rgb = ERROR_BG;
pub const PLUS_TEXT: Rgb = FG_DIM;

pub const PALETTE_BG: Rgb = BG_SURFACE;
pub const PALETTE_BORDER: Rgb = BORDER;
pub const PALETTE_INPUT_BG: Rgb = BG;
pub const PALETTE_SELECTED_BG: Rgb = BG_SELECTION;
pub const PALETTE_TEXT: Rgb = FG_PRIMARY;
pub const PALETTE_DIM_TEXT: Rgb = FG_SECONDARY;

pub const DEBUG_BG: Rgb = BG_SURFACE;
pub const DEBUG_TEXT: Rgb = (130, 180, 130);

pub const SHELL_PICKER_BG: Rgb = BG_SURFACE;
pub const SHELL_PICKER_HOVER: Rgb = BG_HOVER;
pub const SHELL_PICKER_TEXT: Rgb = FG_PRIMARY;
pub const SHELL_PICKER_BORDER: Rgb = BORDER;

pub const SETTINGS_SIDEBAR_BG: Rgb = BG;
pub const SETTINGS_SIDEBAR_TEXT: Rgb = FG_SECONDARY;
pub const SETTINGS_SIDEBAR_ACTIVE_BG: Rgb = PRIMARY;
pub const SETTINGS_SIDEBAR_ACTIVE_TEXT: Rgb = (255, 255, 255);
pub const SETTINGS_SIDEBAR_HOVER_BG: Rgb = BG_HOVER;
pub const SETTINGS_CONTENT_BG: Rgb = BG;
pub const SETTINGS_HEADER_TEXT: Rgb = FG_PRIMARY;
pub const SETTINGS_BODY_TEXT: Rgb = FG_SECONDARY;
pub const SETTINGS_DIVIDER: Rgb = DIVIDER;
pub const SETTINGS_SECTION_TITLE: Rgb = FG_BRIGHT;
pub const SETTINGS_LABEL: Rgb = FG_SECONDARY;
pub const SETTINGS_INPUT_BG: Rgb = BG_SURFACE;
pub const SETTINGS_INPUT_BORDER: Rgb = BORDER;
pub const SETTINGS_INPUT_TEXT: Rgb = FG_PRIMARY;

pub const EDITOR_CURRENT_LINE_BG: Rgb = (18, 18, 22);
pub const EDITOR_CURSOR: Rgb = PRIMARY;
pub const EDITOR_GUTTER_ACTIVE: Rgb = FG_SECONDARY;
pub const SCROLLBAR_THUMB: Rgb = (80, 84, 96);
pub const SCROLLBAR_THUMB_HOVER: Rgb = (120, 122, 132);

pub const AVATAR_ICON_HOVER: Rgb = FG_PRIMARY;

pub const TOAST_BG: Rgb = (18, 18, 22);
pub const TOAST_TEXT: Rgb = FG_PRIMARY;
pub const TOAST_INFO_ACCENT: Rgb = (97, 175, 239);
pub const TOAST_SUCCESS_ACCENT: Rgb = SUCCESS;
pub const TOAST_WARNING_ACCENT: Rgb = WARNING;
pub const TOAST_ERROR_ACCENT: Rgb = ERROR;

/// Accent color for agent blocks (a saturated blue-purple).
pub const AGENT_ACCENT: Rgb = (100, 120, 230);
/// Subtle background tint for agent blocks.
pub const AGENT_BG: Rgb = (0, 0, 0);
/// Header/label color in agent blocks.
pub const AGENT_TEXT: Rgb = (130, 145, 220);
/// Approval box button highlight.
pub const AGENT_BUTTON_BG: Rgb = (30, 34, 55);

/// Resolve an ANSI color to RGB, checking TUI app color overrides first.
pub fn resolve_color(color: &AnsiColor, colors: &TermColors) -> Rgb {
    match color {
        AnsiColor::Named(named) => {
            if let Some(rgb) = colors[*named] {
                return (rgb.r, rgb.g, rgb.b);
            }
            named_color_to_rgb(*named)
        }
        AnsiColor::Spec(rgb) => (rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(idx) => {
            if let Some(rgb) = colors[*idx as usize] {
                return (rgb.r, rgb.g, rgb.b);
            }
            indexed_color_to_rgb(*idx)
        }
    }
}

fn named_color_to_rgb(c: NamedColor) -> Rgb {
    match c {
        NamedColor::Black => (13, 13, 15),
        NamedColor::Red => (224, 108, 117),
        NamedColor::Green => (152, 195, 121),
        NamedColor::Yellow => (229, 192, 123),
        NamedColor::Blue => (97, 175, 239),
        NamedColor::Magenta => (198, 120, 221),
        NamedColor::Cyan => (86, 182, 194),
        NamedColor::White => (171, 178, 191),
        NamedColor::BrightBlack => (92, 99, 112),
        NamedColor::BrightRed => (232, 131, 136),
        NamedColor::BrightGreen => (168, 212, 143),
        NamedColor::BrightYellow => (235, 203, 139),
        NamedColor::BrightBlue => (127, 195, 245),
        NamedColor::BrightMagenta => (210, 144, 228),
        NamedColor::BrightCyan => (115, 199, 210),
        NamedColor::BrightWhite => (220, 220, 225),
        NamedColor::Foreground => (255, 255, 255),
        NamedColor::Background => (13, 13, 15),
        _ => (171, 178, 191),
    }
}

fn indexed_color_to_rgb(idx: u8) -> Rgb {
    if idx < 16 {
        return named_color_to_rgb(match idx {
            0 => NamedColor::Black,
            1 => NamedColor::Red,
            2 => NamedColor::Green,
            3 => NamedColor::Yellow,
            4 => NamedColor::Blue,
            5 => NamedColor::Magenta,
            6 => NamedColor::Cyan,
            7 => NamedColor::White,
            8 => NamedColor::BrightBlack,
            9 => NamedColor::BrightRed,
            10 => NamedColor::BrightGreen,
            11 => NamedColor::BrightYellow,
            12 => NamedColor::BrightBlue,
            13 => NamedColor::BrightMagenta,
            14 => NamedColor::BrightCyan,
            15 => NamedColor::BrightWhite,
            _ => NamedColor::Foreground,
        });
    }

    if idx < 232 {
        let idx = idx - 16;
        let r = (idx / 36) % 6;
        let g = (idx / 6) % 6;
        let b = idx % 6;
        let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
        return (to_val(r), to_val(g), to_val(b));
    }

    let v = 8 + 10 * (idx - 232);
    (v, v, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bg_is_dark() {
        let (r, g, b) = BG;
        assert!(r < 30 && g < 30 && b < 30);
    }

    #[test]
    fn named_colors_cover_all_basic() {
        use alacritty_terminal::vte::ansi::NamedColor;
        let colors = [
            NamedColor::Black,
            NamedColor::Red,
            NamedColor::Green,
            NamedColor::Yellow,
            NamedColor::Blue,
            NamedColor::Magenta,
            NamedColor::Cyan,
            NamedColor::White,
            NamedColor::BrightBlack,
            NamedColor::BrightRed,
            NamedColor::BrightGreen,
            NamedColor::BrightYellow,
            NamedColor::BrightBlue,
            NamedColor::BrightMagenta,
            NamedColor::BrightCyan,
            NamedColor::BrightWhite,
            NamedColor::Foreground,
            NamedColor::Background,
        ];
        for c in &colors {
            let _ = named_color_to_rgb(*c);
        }
    }

    #[test]
    fn indexed_color_first_16_match_named() {
        for idx in 0u8..16 {
            let _ = indexed_color_to_rgb(idx);
        }
    }

    #[test]
    fn indexed_color_cube_range() {
        for idx in 16u8..232 {
            let _ = indexed_color_to_rgb(idx);
        }
    }

    #[test]
    fn indexed_color_grayscale_range() {
        for idx in 232u8..=255 {
            let (r, g, b) = indexed_color_to_rgb(idx);
            assert_eq!(r, g);
            assert_eq!(g, b);
        }
    }

    #[test]
    fn indexed_color_cube_black() {
        let (r, g, b) = indexed_color_to_rgb(16);
        assert_eq!((r, g, b), (0, 0, 0));
    }

    #[test]
    fn indexed_color_cube_white() {
        let (r, g, b) = indexed_color_to_rgb(231);
        assert_eq!((r, g, b), (255, 255, 255));
    }

    #[test]
    fn grayscale_ramp_monotonic() {
        let mut prev = 0u8;
        for idx in 232u8..=255 {
            let (v, _, _) = indexed_color_to_rgb(idx);
            assert!(v >= prev);
            prev = v;
        }
    }

    #[test]
    fn resolve_color_spec() {
        use alacritty_terminal::vte::ansi::Color as AnsiColor;
        let rgb_spec = alacritty_terminal::vte::ansi::Rgb {
            r: 42,
            g: 84,
            b: 126,
        };
        let colors = TermColors::default();
        let result = resolve_color(&AnsiColor::Spec(rgb_spec), &colors);
        assert_eq!(result, (42, 84, 126));
    }

    #[test]
    fn tab_indicator_is_primary() {
        let (r, _g, _b) = TAB_INDICATOR;
        assert!(r > 100, "TAB_INDICATOR should have a strong primary channel");
    }
}
