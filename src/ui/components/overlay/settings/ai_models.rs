use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::{Icon, IconRenderer};
use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;

use super::super::fill_rounded_rect;
use super::SettingsState;

const BTN_BG: (u8, u8, u8) = theme::BG_ELEVATED;
const BTN_HOVER_BG: (u8, u8, u8) = theme::BORDER;
const BTN_TEXT: (u8, u8, u8) = theme::FG_PRIMARY;
const PATH_TEXT: (u8, u8, u8) = theme::FG_SECONDARY;
const SIZE_TEXT: (u8, u8, u8) = theme::FG_DIM;
const TOGGLE_ON_BG: (u8, u8, u8) = theme::PRIMARY;
const TOGGLE_OFF_BG: (u8, u8, u8) = theme::BORDER;
const TOGGLE_KNOB: (u8, u8, u8) = theme::FG_BRIGHT;
const DELETE_TEXT: (u8, u8, u8) = theme::ERROR;

const MODEL_ROW_H: f32 = 32.0;

pub fn draw_settings_ai_models(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &SettingsState,
    area_x: usize,
    area_y: usize,
    area_w: usize,
    clip_h: usize,
    sf: f32,
) {
    let pad = (24.0 * sf) as usize;
    let section_metrics = Metrics::new(15.0 * sf, 21.0 * sf);
    let label_metrics = Metrics::new(13.0 * sf, 18.0 * sf);
    let small_metrics = Metrics::new(11.0 * sf, 16.0 * sf);
    let btn_metrics = Metrics::new(12.0 * sf, 17.0 * sf);
    let row_h = (40.0 * sf) as usize;
    let mut y = area_y;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y,
        clip_h,
        "Models Storage",
        section_metrics,
        theme::SETTINGS_SECTION_TITLE,
        Family::Monospace,
    );
    y += (32.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y + (row_h as f32 / 2.0 - 9.0 * sf) as usize,
        clip_h,
        "Storage path",
        label_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );
    y += row_h;

    let path_display = truncate_path_front(&state.models_path, area_w.saturating_sub(pad * 2), sf);
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y,
        clip_h,
        &path_display,
        small_metrics,
        PATH_TEXT,
        Family::Monospace,
    );
    y += (24.0 * sf) as usize;

    let disk_usage = compute_disk_usage(&state.models_path);
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y,
        clip_h,
        &disk_usage,
        small_metrics,
        SIZE_TEXT,
        Family::Monospace,
    );
    y += (28.0 * sf) as usize;

    let btn_h = (28.0 * sf) as usize;
    let btn_corner = (4.0 * sf) as usize;
    let btn_gap = (12.0 * sf) as usize;

    let open_label = "Open in Finder";
    let open_w = (open_label.len() as f32 * 7.5 * sf) as usize + (24.0 * sf) as usize;
    let open_x = area_x + pad;
    let open_bg = if state.hovered_btn == Some(AiModelsHit::OpenInFinder) {
        BTN_HOVER_BG
    } else {
        BTN_BG
    };
    fill_rounded_rect(buf, open_x, y, open_w, btn_h, btn_corner, open_bg);
    let text_y = y + ((btn_h as f32 - 17.0 * sf) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        open_x + (12.0 * sf) as usize,
        text_y,
        clip_h,
        open_label,
        btn_metrics,
        BTN_TEXT,
        Family::Monospace,
    );

    let change_label = "Change Path";
    let change_w = (change_label.len() as f32 * 7.5 * sf) as usize + (24.0 * sf) as usize;
    let change_x = open_x + open_w + btn_gap;
    let change_bg = if state.hovered_btn == Some(AiModelsHit::ChangePath) {
        BTN_HOVER_BG
    } else {
        BTN_BG
    };
    fill_rounded_rect(buf, change_x, y, change_w, btn_h, btn_corner, change_bg);
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        change_x + (12.0 * sf) as usize,
        text_y,
        clip_h,
        change_label,
        btn_metrics,
        BTN_TEXT,
        Family::Monospace,
    );
    y += btn_h + (24.0 * sf) as usize;

    let bw = (1.0 * sf).max(1.0) as usize;
    let line_pad = (20.0 * sf) as usize;
    buf.fill_rect(
        area_x + line_pad,
        y,
        area_w.saturating_sub(line_pad * 2),
        bw,
        theme::SETTINGS_DIVIDER,
    );
    y += (24.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y,
        clip_h,
        "Downloaded Models",
        section_metrics,
        theme::SETTINGS_SECTION_TITLE,
        Family::Monospace,
    );
    y += (32.0 * sf) as usize;

    let models_dir = std::path::PathBuf::from(&state.models_path);
    let models = crate::ai::registry::MODELS;
    let mut any_found = false;

    let model_row_h = (MODEL_ROW_H * sf) as usize;
    let icon_btn_sz = (28.0 * sf) as usize;
    let icon_sz = (16.0 * sf) as usize;
    let icon_pad = (icon_btn_sz - icon_sz) / 2;

    for (mi, model) in models.iter().enumerate() {
        let path = models_dir.join(&model.filename);
        if !path.exists() {
            continue;
        }
        any_found = true;

        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
        let size_str = format_bytes(size);
        let line = format!("{}  ({}  {})", model.name, model.quant_label, size_str);
        let text_y = y + ((model_row_h as f32 - 18.0 * sf) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            area_x + pad,
            text_y,
            clip_h,
            &line,
            label_metrics,
            theme::SETTINGS_LABEL,
            Family::Monospace,
        );

        let is_deleting = state.deleting_model == Some(mi);
        if is_deleting {
            let del_label = "Deleting…";
            let del_metrics = Metrics::new(11.0 * sf, 16.0 * sf);
            let del_text_x = area_x + area_w.saturating_sub(pad + (del_label.len() as f32 * 6.5 * sf) as usize);
            let del_text_y = y + ((model_row_h as f32 - 16.0 * sf) / 2.0) as usize;
            draw_text_at(
                buf, font_system, swash_cache,
                del_text_x, del_text_y, clip_h,
                del_label, del_metrics, theme::FG_DIM, Family::Monospace,
            );
        } else {
            let del_x = area_x + area_w.saturating_sub(pad + icon_btn_sz);
            let del_y = y + (model_row_h - icon_btn_sz) / 2;
            let is_del_hovered = state.hovered_btn == Some(AiModelsHit::DeleteModel(mi));
            let del_color = if is_del_hovered { DELETE_TEXT } else { SIZE_TEXT };
            icon_renderer.draw(buf, Icon::Trash, del_x + icon_pad, del_y + icon_pad, icon_sz as u32, del_color);
        }

        y += model_row_h;
    }

    if !any_found {
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            area_x + pad,
            y,
            clip_h,
            "No models downloaded yet. Use the AI panel to download models.",
            small_metrics,
            SIZE_TEXT,
            Family::Monospace,
        );
        y += (26.0 * sf) as usize;
    }

    y += (12.0 * sf) as usize;
    let bw2 = (1.0 * sf).max(1.0) as usize;
    let line_pad2 = (20.0 * sf) as usize;
    buf.fill_rect(
        area_x + line_pad2,
        y,
        area_w.saturating_sub(line_pad2 * 2),
        bw2,
        theme::SETTINGS_DIVIDER,
    );
    y += (24.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y,
        clip_h,
        "AI Hints",
        section_metrics,
        theme::SETTINGS_SECTION_TITLE,
        Family::Monospace,
    );
    y += (32.0 * sf) as usize;

    let toggle_label = "Enrich hints with web search";
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y + (row_h as f32 / 2.0 - 9.0 * sf) as usize,
        clip_h,
        toggle_label,
        label_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );

    let toggle_w = (36.0 * sf) as usize;
    let toggle_h = (20.0 * sf) as usize;
    let toggle_x = area_x + area_w.saturating_sub(pad + toggle_w);
    let toggle_y = y + (row_h - toggle_h) / 2;
    let toggle_r = toggle_h / 2;
    let toggle_bg = if state.web_search_enabled {
        TOGGLE_ON_BG
    } else {
        TOGGLE_OFF_BG
    };
    fill_rounded_rect(
        buf, toggle_x, toggle_y, toggle_w, toggle_h, toggle_r, toggle_bg,
    );

    let knob_d = toggle_h.saturating_sub((4.0 * sf) as usize);
    let knob_r = knob_d / 2;
    let knob_y = toggle_y + (toggle_h - knob_d) / 2;
    let knob_x = if state.web_search_enabled {
        toggle_x + toggle_w - knob_d - (2.0 * sf) as usize
    } else {
        toggle_x + (2.0 * sf) as usize
    };
    fill_rounded_rect(buf, knob_x, knob_y, knob_d, knob_d, knob_r, TOGGLE_KNOB);

    y += row_h;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y,
        clip_h,
        "When enabled, searches the web for install instructions on \"command not found\" errors.",
        small_metrics,
        SIZE_TEXT,
        Family::Monospace,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiModelsHit {
    OpenInFinder,
    ChangePath,
    ToggleWebSearch,
    DeleteModel(usize),
}

pub fn settings_ai_models_hit_test(
    phys_x: f64,
    phys_y: f64,
    y_offset: usize,
    content_area_x: usize,
    content_area_w: usize,
    sf: f32,
) -> Option<AiModelsHit> {
    let pad = (24.0 * sf) as f64;
    let base_y = y_offset as f64 + (16.0 * sf) as f64;

    let btn_y = base_y + (32.0 + 40.0 + 24.0 + 28.0) * sf as f64;
    let btn_h = (28.0 * sf) as f64;
    let btn_gap = (12.0 * sf) as f64;

    if phys_y >= btn_y && phys_y <= btn_y + btn_h {
        let open_label = "Open in Finder";
        let open_w = open_label.len() as f64 * 7.5 * sf as f64 + 24.0 * sf as f64;
        let open_x = content_area_x as f64 + pad;

        if phys_x >= open_x && phys_x < open_x + open_w {
            return Some(AiModelsHit::OpenInFinder);
        }

        let change_label = "Change Path";
        let change_w = change_label.len() as f64 * 7.5 * sf as f64 + 24.0 * sf as f64;
        let change_x = open_x + open_w + btn_gap;

        if phys_x >= change_x && phys_x < change_x + change_w {
            return Some(AiModelsHit::ChangePath);
        }
    }

    let after_btn = btn_y + btn_h + 24.0 * sf as f64;
    let models_start_y = after_btn + 12.0 * sf as f64 + 1.0 + 24.0 * sf as f64 + 32.0 * sf as f64;

    let models_dir = std::path::PathBuf::from(
        crate::ai::model_manager::models_dir()
            .to_string_lossy()
            .to_string(),
    );
    let model_row_h = MODEL_ROW_H as f64 * sf as f64;
    let icon_btn_sz = 28.0 * sf as f64;

    let mut row_idx = 0usize;
    let mut downloaded_rows = 0usize;
    for (i, model) in crate::ai::registry::MODELS.iter().enumerate() {
        let path = models_dir.join(model.filename);
        if !path.exists() {
            continue;
        }

        let row_y = models_start_y + row_idx as f64 * model_row_h;
        let del_x = content_area_x as f64 + content_area_w as f64 - pad - icon_btn_sz;
        let del_y = row_y + (model_row_h - icon_btn_sz) / 2.0;

        if phys_x >= del_x
            && phys_x < del_x + icon_btn_sz
            && phys_y >= del_y
            && phys_y < del_y + icon_btn_sz
        {
            return Some(AiModelsHit::DeleteModel(i));
        }

        row_idx += 1;
        downloaded_rows += 1;
    }

    let model_rows_h = if downloaded_rows > 0 {
        downloaded_rows as f64 * model_row_h
    } else {
        26.0 * sf as f64
    };
    let toggle_section_y = models_start_y + model_rows_h
        + 12.0 * sf as f64  // gap before divider
        + 1.0               // divider
        + 24.0 * sf as f64  // gap after divider
        + 32.0 * sf as f64; // "AI Hints" title + spacing

    let row_h = 40.0 * sf as f64;
    let toggle_w = 36.0 * sf as f64;
    let toggle_h = 20.0 * sf as f64;
    let toggle_x = content_area_x as f64 + content_area_w as f64 - pad - toggle_w;
    let toggle_y = toggle_section_y + (row_h - toggle_h) / 2.0;

    if phys_x >= toggle_x
        && phys_x < toggle_x + toggle_w
        && phys_y >= toggle_y
        && phys_y < toggle_y + toggle_h
    {
        return Some(AiModelsHit::ToggleWebSearch);
    }

    None
}

fn truncate_path_front(path: &str, max_px: usize, sf: f32) -> String {
    let max_chars = (max_px as f32 / (7.0 * sf)) as usize;
    if path.len() <= max_chars {
        path.to_string()
    } else {
        let skip = path.len() - max_chars + 3;
        format!("...{}", &path[skip..])
    }
}

fn compute_disk_usage(path: &str) -> String {
    let dir = std::path::Path::new(path);
    if !dir.exists() {
        return "Directory does not exist".to_string();
    }
    let mut total: u64 = 0;
    let mut count: usize = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().map(|e| e == "gguf").unwrap_or(false) {
                if let Ok(meta) = p.metadata() {
                    total += meta.len();
                    count += 1;
                }
            }
        }
    }
    format!("{} model file(s), {} total", count, format_bytes(total))
}

fn format_bytes(bytes: u64) -> String {
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb >= 1.0 {
        format!("{:.1} GB", gb)
    } else {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        format!("{:.0} MB", mb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_gb() {
        let result = format_bytes(2 * 1024 * 1024 * 1024);
        assert_eq!(result, "2.0 GB");
    }

    #[test]
    fn format_bytes_mb() {
        let result = format_bytes(500 * 1024 * 1024);
        assert_eq!(result, "500 MB");
    }

    #[test]
    fn format_bytes_zero() {
        let result = format_bytes(0);
        assert_eq!(result, "0 MB");
    }

    #[test]
    fn truncate_path_short() {
        let result = truncate_path_front("/usr/local", 200, 1.0);
        assert_eq!(result, "/usr/local");
    }

    #[test]
    fn truncate_path_long() {
        let long = "/very/long/path/that/should/be/truncated/for/display";
        let result = truncate_path_front(long, 100, 1.0);
        assert!(result.starts_with("..."));
        assert!(result.len() < long.len());
    }
}
