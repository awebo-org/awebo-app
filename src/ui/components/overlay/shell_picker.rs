//! Shell picker dropdown overlay — local shells + sandbox environments.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::draw_border;

const PICKER_WIDTH: f32 = 280.0;
const ITEM_HEIGHT: f32 = 28.0;
const SANDBOX_ITEM_HEIGHT: f32 = 42.0;
const SECTION_HEADER_HEIGHT: f32 = 24.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const PADDING: f32 = 6.0;
const TEXT_SIZE: f32 = 12.0;
const LINE_HEIGHT: f32 = 17.0;
const DESC_SIZE: f32 = 10.0;
const DESC_LINE_HEIGHT: f32 = 14.0;

/// What the user chose from the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellPickerChoice {
    /// A local shell by index in the `local_shells` list.
    LocalShell(usize),
    /// A sandbox image by index in the `sandbox_images` list.
    Sandbox(usize),
}

/// State for the shell picker dropdown.
pub struct ShellPickerState {
    pub local_shells: Vec<ShellInfo>,
    pub sandbox_images: Vec<SandboxImageInfo>,
    pub sandbox_available: bool,
    pub hovered: Option<usize>,
}

/// Info about an available local shell.
pub struct ShellInfo {
    pub name: String,
}

/// Info about a sandbox image for the picker.
pub struct SandboxImageInfo {
    pub name: String,
    pub description: String,
    pub category: String,
}

/// Description of a single visual row.
enum RowKind {
    SectionHeader(&'static str),
    Separator,
    LocalShell(usize),
    SandboxImage(usize),
    InstallHint,
}

/// Query: build the list of visual rows.
fn build_rows(picker: &ShellPickerState) -> Vec<RowKind> {
    let mut rows = Vec::new();
    if !picker.local_shells.is_empty() {
        rows.push(RowKind::SectionHeader("Local Shells"));
        for i in 0..picker.local_shells.len() {
            rows.push(RowKind::LocalShell(i));
        }
    }
    if !picker.sandbox_images.is_empty() {
        if !rows.is_empty() {
            rows.push(RowKind::Separator);
        }
        rows.push(RowKind::SectionHeader("Sandbox Environments"));
        for i in 0..picker.sandbox_images.len() {
            rows.push(RowKind::SandboxImage(i));
        }
    } else if !picker.sandbox_available {
        if !rows.is_empty() {
            rows.push(RowKind::Separator);
        }
        rows.push(RowKind::InstallHint);
    }
    rows
}

/// Query: height of a row in physical pixels.
fn row_height(row: &RowKind, sf: f32) -> usize {
    match row {
        RowKind::SectionHeader(_) => (SECTION_HEADER_HEIGHT * sf) as usize,
        RowKind::Separator => (SEPARATOR_HEIGHT * sf) as usize,
        RowKind::SandboxImage(_) => (SANDBOX_ITEM_HEIGHT * sf) as usize,
        _ => (ITEM_HEIGHT * sf) as usize,
    }
}

pub fn draw_shell_picker(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    picker: &ShellPickerState,
    anchor_x: usize,
    anchor_y: usize,
    sf: f32,
) {
    let rows = build_rows(picker);
    let pad = (PADDING * sf) as usize;
    let bw = (1.0 * sf).max(1.0) as usize;
    let pw = (PICKER_WIDTH * sf) as usize;
    let ph: usize = rows.iter().map(|r| row_height(r, sf)).sum::<usize>() + pad * 2;

    let px = anchor_x;
    let py = anchor_y;

    buf.fill_rect(px, py, pw, ph, theme::SHELL_PICKER_BG);
    draw_border(buf, px, py, pw, ph, bw, theme::SHELL_PICKER_BORDER);

    let metrics = Metrics::new(TEXT_SIZE * sf, LINE_HEIGHT * sf);
    let desc_metrics = Metrics::new(DESC_SIZE * sf, DESC_LINE_HEIGHT * sf);
    let header_color = theme::FG_DIM;
    let text_color = theme::SHELL_PICKER_TEXT;
    let desc_color = theme::FG_DIM;

    let mut y = py + pad;

    for (ri, row) in rows.iter().enumerate() {
        let rh = row_height(row, sf);

        match row {
            RowKind::SectionHeader(label) => {
                let tx = px + pad + (8.0 * sf) as usize;
                let ty = y + ((rh as f32 - LINE_HEIGHT * sf) / 2.0) as usize;
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    tx,
                    ty,
                    buf.height,
                    label,
                    desc_metrics,
                    header_color,
                    Family::SansSerif,
                );
            }
            RowKind::Separator => {
                let sep_y = y + rh / 2;
                let sep_x0 = px + pad;
                let sep_x1 = px + pw - pad;
                for x in sep_x0..sep_x1 {
                    buf.blend_pixel(x, sep_y, theme::SHELL_PICKER_BORDER, 1.0);
                }
            }
            RowKind::LocalShell(idx) => {
                if picker.hovered == Some(ri) {
                    buf.fill_rect(px + bw, y, pw - bw * 2, rh, theme::SHELL_PICKER_HOVER);
                }
                let tx = px + pad + (8.0 * sf) as usize;
                let ty = y + ((rh as f32 - LINE_HEIGHT * sf) / 2.0) as usize;
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    tx,
                    ty,
                    buf.height,
                    &picker.local_shells[*idx].name,
                    metrics,
                    text_color,
                    Family::Monospace,
                );
            }
            RowKind::SandboxImage(idx) => {
                if picker.hovered == Some(ri) {
                    buf.fill_rect(px + bw, y, pw - bw * 2, rh, theme::SHELL_PICKER_HOVER);
                }
                let img = &picker.sandbox_images[*idx];
                let tx = px + pad + (8.0 * sf) as usize;
                let ty = y + (4.0 * sf) as usize;
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    tx,
                    ty,
                    buf.height,
                    &img.name,
                    metrics,
                    text_color,
                    Family::Monospace,
                );
                let desc_text = format!("{} · {}", img.category, img.description);
                let desc_y = ty + (LINE_HEIGHT * sf) as usize;
                let avail_w = pw.saturating_sub(pad * 2 + (16.0 * sf) as usize);
                let char_w = (DESC_SIZE * sf * 0.55) as usize;
                let max_chars = if char_w > 0 {
                    avail_w / char_w
                } else {
                    desc_text.len()
                };
                let truncated = if desc_text.len() > max_chars && max_chars > 3 {
                    format!("{}…", &desc_text[..max_chars.saturating_sub(1)])
                } else {
                    desc_text
                };
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    tx,
                    desc_y,
                    buf.height,
                    &truncated,
                    desc_metrics,
                    desc_color,
                    Family::SansSerif,
                );
            }
            RowKind::InstallHint => {
                if picker.hovered == Some(ri) {
                    buf.fill_rect(px + bw, y, pw - bw * 2, rh, theme::SHELL_PICKER_HOVER);
                }
                let tx = px + pad + (8.0 * sf) as usize;
                let ty = y + ((rh as f32 - LINE_HEIGHT * sf) / 2.0) as usize;
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    tx,
                    ty,
                    buf.height,
                    "Install microsandbox…",
                    metrics,
                    desc_color,
                    Family::Monospace,
                );
            }
        }

        y += rh;
    }
}

/// Hit-test for the shell picker dropdown. Returns choice if a clickable row is hit.
pub fn shell_picker_hit_test(
    phys_x: f64,
    phys_y: f64,
    anchor_x: usize,
    anchor_y: usize,
    picker: &ShellPickerState,
    sf: f32,
) -> Option<ShellPickerChoice> {
    let rows = build_rows(picker);
    let pad = (PADDING * sf) as f64;
    let pw = (PICKER_WIDTH * sf) as f64;
    let ph: f64 = rows.iter().map(|r| row_height(r, sf) as f64).sum::<f64>() + pad * 2.0;

    let px = anchor_x as f64;
    let py = anchor_y as f64;

    if phys_x < px || phys_x >= px + pw || phys_y < py || phys_y >= py + ph {
        return None;
    }

    let mut y = py + pad;
    for row in rows.iter() {
        let rh = row_height(row, sf) as f64;
        if phys_y >= y && phys_y < y + rh {
            return match row {
                RowKind::LocalShell(idx) => Some(ShellPickerChoice::LocalShell(*idx)),
                RowKind::SandboxImage(idx) => Some(ShellPickerChoice::Sandbox(*idx)),
                _ => None, // headers, separators, install hint: not clickable
            };
        }
        y += rh;
    }
    None
}

/// Hover-test: returns the visual row index if the cursor is over an interactive item.
pub fn shell_picker_hover_test(
    phys_x: f64,
    phys_y: f64,
    anchor_x: usize,
    anchor_y: usize,
    picker: &ShellPickerState,
    sf: f32,
) -> Option<usize> {
    let rows = build_rows(picker);
    let pad = (PADDING * sf) as f64;
    let pw = (PICKER_WIDTH * sf) as f64;
    let ph: f64 = rows.iter().map(|r| row_height(r, sf) as f64).sum::<f64>() + pad * 2.0;

    let px = anchor_x as f64;
    let py = anchor_y as f64;

    if phys_x < px || phys_x >= px + pw || phys_y < py || phys_y >= py + ph {
        return None;
    }

    let mut y = py + pad;
    for (ri, row) in rows.iter().enumerate() {
        let rh = row_height(row, sf) as f64;
        if phys_y >= y && phys_y < y + rh {
            return match row {
                RowKind::LocalShell(_) | RowKind::SandboxImage(_) | RowKind::InstallHint => {
                    Some(ri)
                }
                _ => None,
            };
        }
        y += rh;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_picker() -> ShellPickerState {
        ShellPickerState {
            local_shells: vec![
                ShellInfo { name: "zsh".into() },
                ShellInfo {
                    name: "bash".into(),
                },
            ],
            sandbox_images: vec![SandboxImageInfo {
                name: "Alpine Linux".into(),
                description: "Minimal 5MB Linux".into(),
                category: "Base".into(),
            }],
            sandbox_available: true,
            hovered: None,
        }
    }

    #[test]
    fn hit_test_outside_returns_none() {
        let p = make_picker();
        assert!(shell_picker_hit_test(0.0, 0.0, 100, 50, &p, 1.0).is_none());
    }

    #[test]
    fn hit_test_local_shell() {
        let p = make_picker();
        let result = shell_picker_hit_test(150.0, 83.0, 100, 50, &p, 1.0);
        assert_eq!(result, Some(ShellPickerChoice::LocalShell(0)));
    }

    #[test]
    fn hit_test_sandbox_image() {
        let p = make_picker();
        let result = shell_picker_hit_test(150.0, 172.0, 100, 50, &p, 1.0);
        assert_eq!(result, Some(ShellPickerChoice::Sandbox(0)));
    }

    #[test]
    fn hit_test_section_header_not_clickable() {
        let p = make_picker();
        let result = shell_picker_hit_test(150.0, 59.0, 100, 50, &p, 1.0);
        assert!(result.is_none());
    }

    #[test]
    fn hover_test_returns_row_index() {
        let p = make_picker();
        let result = shell_picker_hover_test(150.0, 83.0, 100, 50, &p, 1.0);
        assert!(result.is_some());
    }
}
