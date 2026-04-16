use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::{Icon, IconRenderer};
use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::{draw_text_at, draw_text_clipped, measure_text_width};
use crate::renderer::theme;

use super::super::{draw_border, fill_rounded_rect};
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
const RUNTIME_BTN_H: f32 = 28.0;

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
    let btn_h = (RUNTIME_BTN_H * sf) as usize;
    let btn_corner = (4.0 * sf) as usize;
    let mut y = area_y;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y,
        clip_h,
        "Runtime",
        section_metrics,
        theme::SETTINGS_SECTION_TITLE,
        Family::Monospace,
    );
    y += (28.0 * sf) as usize;

    let local_label = "Local";
    let ollama_label = "Ollama";
    let local_w = (local_label.len() as f32 * 7.5 * sf) as usize + (24.0 * sf) as usize;
    let ollama_w = (ollama_label.len() as f32 * 7.5 * sf) as usize + (24.0 * sf) as usize;
    let gap = (8.0 * sf) as usize;
    let local_x = area_x + pad;
    let ollama_x = local_x + local_w + gap;

    let local_bg = if !state.ollama_enabled {
        TOGGLE_ON_BG
    } else if state.hovered_btn == Some(AiModelsHit::RuntimeLocal) {
        BTN_HOVER_BG
    } else {
        BTN_BG
    };
    let ollama_bg = if state.ollama_enabled {
        TOGGLE_ON_BG
    } else if state.hovered_btn == Some(AiModelsHit::RuntimeOllama) {
        BTN_HOVER_BG
    } else {
        BTN_BG
    };

    fill_rounded_rect(buf, local_x, y, local_w, btn_h, btn_corner, local_bg);
    let text_y = y + ((btn_h as f32 - 17.0 * sf) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        local_x + (12.0 * sf) as usize,
        text_y,
        clip_h,
        local_label,
        btn_metrics,
        BTN_TEXT,
        Family::Monospace,
    );

    fill_rounded_rect(buf, ollama_x, y, ollama_w, btn_h, btn_corner, ollama_bg);
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        ollama_x + (12.0 * sf) as usize,
        text_y,
        clip_h,
        ollama_label,
        btn_metrics,
        BTN_TEXT,
        Family::Monospace,
    );
    y += btn_h + (16.0 * sf) as usize;

    let bw = (1.0 * sf).max(1.0) as usize;
    let line_pad = (20.0 * sf) as usize;
    buf.fill_rect(
        area_x + line_pad,
        y,
        area_w.saturating_sub(line_pad * 2),
        bw,
        theme::SETTINGS_DIVIDER,
    );
    y += (20.0 * sf) as usize;

    if state.ollama_enabled {
        draw_ollama_section(
            buf,
            font_system,
            swash_cache,
            state,
            area_x,
            &mut y,
            area_w,
            clip_h,
            sf,
            pad,
            section_metrics,
            label_metrics,
            small_metrics,
            btn_metrics,
            btn_h,
            btn_corner,
        );
    } else {
        draw_local_section(
            buf,
            font_system,
            swash_cache,
            icon_renderer,
            state,
            area_x,
            &mut y,
            area_w,
            clip_h,
            sf,
            pad,
            section_metrics,
            label_metrics,
            small_metrics,
            btn_metrics,
            btn_h,
            btn_corner,
        );
    }

    buf.fill_rect(
        area_x + line_pad,
        y,
        area_w.saturating_sub(line_pad * 2),
        bw,
        theme::SETTINGS_DIVIDER,
    );
    y += (20.0 * sf) as usize;

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
    y += (28.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        y + (row_h as f32 / 2.0 - 9.0 * sf) as usize,
        clip_h,
        "Enrich hints with web search",
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
        "Searches the web for help on \"command not found\" errors.",
        small_metrics,
        SIZE_TEXT,
        Family::Monospace,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_local_section(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &SettingsState,
    area_x: usize,
    y: &mut usize,
    area_w: usize,
    clip_h: usize,
    sf: f32,
    pad: usize,
    section_metrics: Metrics,
    label_metrics: Metrics,
    small_metrics: Metrics,
    btn_metrics: Metrics,
    btn_h: usize,
    btn_corner: usize,
) {
    let row_h = (40.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        *y,
        clip_h,
        "Models Storage",
        section_metrics,
        theme::SETTINGS_SECTION_TITLE,
        Family::Monospace,
    );
    *y += (28.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        *y + (row_h as f32 / 2.0 - 9.0 * sf) as usize,
        clip_h,
        "Storage path",
        label_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );
    *y += row_h;

    let path_display = truncate_path_front(&state.models_path, area_w.saturating_sub(pad * 2), sf);
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        *y,
        clip_h,
        &path_display,
        small_metrics,
        PATH_TEXT,
        Family::Monospace,
    );
    *y += (20.0 * sf) as usize;

    let disk_usage = compute_disk_usage(&state.models_path);
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        *y,
        clip_h,
        &disk_usage,
        small_metrics,
        SIZE_TEXT,
        Family::Monospace,
    );
    *y += (24.0 * sf) as usize;

    let btn_gap = (12.0 * sf) as usize;
    let open_label = "Open in Finder";
    let open_w = (open_label.len() as f32 * 7.5 * sf) as usize + (24.0 * sf) as usize;
    let open_x = area_x + pad;
    let open_bg = if state.hovered_btn == Some(AiModelsHit::OpenInFinder) {
        BTN_HOVER_BG
    } else {
        BTN_BG
    };
    fill_rounded_rect(buf, open_x, *y, open_w, btn_h, btn_corner, open_bg);
    let text_y = *y + ((btn_h as f32 - 17.0 * sf) / 2.0) as usize;
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
    fill_rounded_rect(buf, change_x, *y, change_w, btn_h, btn_corner, change_bg);
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
    *y += btn_h + (16.0 * sf) as usize;

    let bw = (1.0 * sf).max(1.0) as usize;
    let lp = (20.0 * sf) as usize;
    buf.fill_rect(
        area_x + lp,
        *y,
        area_w.saturating_sub(lp * 2),
        bw,
        theme::SETTINGS_DIVIDER,
    );
    *y += (20.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        *y,
        clip_h,
        "Available Models",
        section_metrics,
        theme::SETTINGS_SECTION_TITLE,
        Family::Monospace,
    );

    let manage_label = "Manage";
    let manage_w = (manage_label.len() as f32 * 7.5 * sf) as usize + (24.0 * sf) as usize;
    let manage_x = area_x + area_w.saturating_sub(pad + manage_w);
    let section_text_h = (15.0 * sf) as usize;
    let manage_y = (*y)
        .saturating_add(section_text_h / 2)
        .saturating_sub(btn_h / 2);
    let manage_bg = if state.hovered_btn == Some(AiModelsHit::OpenModels) {
        BTN_HOVER_BG
    } else {
        BTN_BG
    };
    fill_rounded_rect(
        buf, manage_x, manage_y, manage_w, btn_h, btn_corner, manage_bg,
    );
    let manage_text_y = manage_y + ((btn_h as f32 - 17.0 * sf) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        manage_x + (12.0 * sf) as usize,
        manage_text_y,
        clip_h,
        manage_label,
        btn_metrics,
        BTN_TEXT,
        Family::Monospace,
    );
    *y += (28.0 * sf) as usize;

    let models_dir = std::path::PathBuf::from(&state.models_path);
    let models = crate::ai::registry::MODELS;
    let mut any_found = false;

    let model_row_h = (MODEL_ROW_H * sf) as usize;
    let icon_btn_sz = (28.0 * sf) as usize;
    let icon_sz = (16.0 * sf) as usize;
    let icon_pad = (icon_btn_sz - icon_sz) / 2;
    let row_corner = (4.0 * sf) as usize;

    for (mi, model) in models.iter().enumerate() {
        let path = models_dir.join(model.filename);
        if !path.exists() {
            continue;
        }
        any_found = true;

        let is_del_hovered = state.hovered_btn == Some(AiModelsHit::DeleteModel(mi));
        let row_bg = if is_del_hovered {
            BTN_HOVER_BG
        } else {
            theme::BG_ELEVATED
        };
        fill_rounded_rect(
            buf,
            area_x + pad / 2,
            *y,
            area_w.saturating_sub(pad),
            model_row_h,
            row_corner,
            row_bg,
        );

        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
        let size_str = format_bytes(size);
        let line = format!("{}  ({}  {})", model.name, model.quant_label, size_str);
        let text_y = *y + ((model_row_h as f32 - 18.0 * sf) / 2.0) as usize;
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
            let del_text_x =
                area_x + area_w.saturating_sub(pad + (del_label.len() as f32 * 6.5 * sf) as usize);
            let del_text_y = *y + ((model_row_h as f32 - 16.0 * sf) / 2.0) as usize;
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                del_text_x,
                del_text_y,
                clip_h,
                del_label,
                del_metrics,
                theme::FG_DIM,
                Family::Monospace,
            );
        } else {
            let del_x = area_x + area_w.saturating_sub(pad + icon_btn_sz);
            let del_y = *y + (model_row_h - icon_btn_sz) / 2;
            let del_color = if is_del_hovered {
                DELETE_TEXT
            } else {
                SIZE_TEXT
            };
            icon_renderer.draw(
                buf,
                Icon::Trash,
                del_x + icon_pad,
                del_y + icon_pad,
                icon_sz as u32,
                del_color,
            );
        }

        *y += model_row_h + (2.0 * sf) as usize;
    }

    if !any_found {
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            area_x + pad,
            *y,
            clip_h,
            "No models downloaded yet.",
            small_metrics,
            SIZE_TEXT,
            Family::Monospace,
        );
        *y += (22.0 * sf) as usize;
    }

    *y += (12.0 * sf) as usize;
}

#[allow(clippy::too_many_arguments)]
fn draw_ollama_section(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    state: &SettingsState,
    area_x: usize,
    y: &mut usize,
    area_w: usize,
    clip_h: usize,
    sf: f32,
    pad: usize,
    section_metrics: Metrics,
    label_metrics: Metrics,
    small_metrics: Metrics,
    btn_metrics: Metrics,
    btn_h: usize,
    btn_corner: usize,
) {
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        *y,
        clip_h,
        "Connection",
        section_metrics,
        theme::SETTINGS_SECTION_TITLE,
        Family::Monospace,
    );
    *y += (28.0 * sf) as usize;

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        area_x + pad,
        *y,
        clip_h,
        "Host",
        label_metrics,
        theme::SETTINGS_LABEL,
        Family::Monospace,
    );
    *y += (22.0 * sf) as usize;

    let input_h = (30.0 * sf) as usize;
    let input_x = area_x + pad;
    let input_w = area_w.saturating_sub(pad * 2);
    let input_corner = (4.0 * sf) as usize;
    let bw = (1.0 * sf).max(1.0) as usize;

    fill_rounded_rect(
        buf,
        input_x,
        *y,
        input_w,
        input_h,
        input_corner,
        theme::SETTINGS_INPUT_BG,
    );
    let input_border = if state.ollama_host_focused {
        theme::PRIMARY
    } else {
        theme::SETTINGS_INPUT_BORDER
    };
    draw_border(buf, input_x, *y, input_w, input_h, bw, input_border);

    let text_pad = (8.0 * sf) as usize;
    let input_text = if state.ollama_host.is_empty() && !state.ollama_host_focused {
        "http://localhost:11434"
    } else {
        &state.ollama_host
    };
    let input_color = if state.ollama_host.is_empty() && !state.ollama_host_focused {
        theme::FG_MUTED
    } else {
        theme::SETTINGS_INPUT_TEXT
    };
    let text_y_inner = *y + (input_h as f32 / 2.0 - 7.0 * sf) as usize;
    let clip_right = input_x + input_w - text_pad;

    if state.ollama_host_focused
        && let Some(anchor) = state.ollama_host_sel_anchor
    {
        let sel_start = anchor.min(state.ollama_host_cursor);
        let sel_end = anchor.max(state.ollama_host_cursor);
        if sel_start < sel_end {
            let pre = &state.ollama_host[..sel_start];
            let sel = &state.ollama_host[sel_start..sel_end];
            let pre_w =
                measure_text_width(font_system, pre, small_metrics, Family::Monospace) as usize;
            let sel_w =
                measure_text_width(font_system, sel, small_metrics, Family::Monospace) as usize;
            let sx = input_x + text_pad + pre_w;
            let sel_h = (14.0 * sf) as usize;
            let sel_y = text_y_inner;
            buf.fill_rect(sx, sel_y, sel_w, sel_h, theme::PRIMARY);
        }
    }

    draw_text_clipped(
        buf,
        font_system,
        swash_cache,
        input_x + text_pad,
        text_y_inner,
        clip_h,
        clip_right,
        input_x + text_pad,
        input_text,
        small_metrics,
        input_color,
        Family::Monospace,
    );

    if state.ollama_host_focused {
        let before_cursor = &state.ollama_host[..state.ollama_host_cursor];
        let cursor_px =
            measure_text_width(font_system, before_cursor, small_metrics, Family::Monospace)
                as usize;
        let cx = input_x + text_pad + cursor_px;
        let cy = text_y_inner;
        let ch = (14.0 * sf) as usize;
        buf.fill_rect(cx, cy, (1.0 * sf).max(1.0) as usize, ch, theme::FG_PRIMARY);
    }

    *y += input_h + (10.0 * sf) as usize;

    let test_label = "Test Connection";
    let test_w = (test_label.len() as f32 * 7.5 * sf) as usize + (24.0 * sf) as usize;
    let test_x = area_x + pad;
    let test_bg = if state.hovered_btn == Some(AiModelsHit::OllamaTestConnection) {
        BTN_HOVER_BG
    } else {
        BTN_BG
    };
    fill_rounded_rect(buf, test_x, *y, test_w, btn_h, btn_corner, test_bg);
    let text_y = *y + ((btn_h as f32 - 17.0 * sf) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        test_x + (12.0 * sf) as usize,
        text_y,
        clip_h,
        test_label,
        btn_metrics,
        BTN_TEXT,
        Family::Monospace,
    );

    let refresh_label = "Refresh";
    let refresh_w = (refresh_label.len() as f32 * 7.5 * sf) as usize + (24.0 * sf) as usize;
    let refresh_x = test_x + test_w + (8.0 * sf) as usize;
    let refresh_bg = if state.hovered_btn == Some(AiModelsHit::OllamaRefresh) {
        BTN_HOVER_BG
    } else {
        BTN_BG
    };
    fill_rounded_rect(buf, refresh_x, *y, refresh_w, btn_h, btn_corner, refresh_bg);
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        refresh_x + (12.0 * sf) as usize,
        text_y,
        clip_h,
        refresh_label,
        btn_metrics,
        BTN_TEXT,
        Family::Monospace,
    );

    *y += btn_h + (16.0 * sf) as usize;

    if !state.ollama_models.is_empty() {
        let bw = (1.0 * sf).max(1.0) as usize;
        let lp = (20.0 * sf) as usize;
        buf.fill_rect(
            area_x + lp,
            *y,
            area_w.saturating_sub(lp * 2),
            bw,
            theme::SETTINGS_DIVIDER,
        );
        *y += (20.0 * sf) as usize;

        draw_text_at(
            buf,
            font_system,
            swash_cache,
            area_x + pad,
            *y,
            clip_h,
            "Available Models",
            section_metrics,
            theme::SETTINGS_SECTION_TITLE,
            Family::Monospace,
        );
        *y += (28.0 * sf) as usize;

        let model_row_h = (MODEL_ROW_H * sf) as usize;
        for (i, model) in state.ollama_models.iter().enumerate() {
            let is_selected = state.ollama_model == model.name;
            let is_hovered = state.hovered_btn == Some(AiModelsHit::OllamaSelectModel(i));

            if is_selected || is_hovered {
                let bg = if is_selected {
                    theme::PRIMARY_DIM
                } else {
                    BTN_HOVER_BG
                };
                fill_rounded_rect(
                    buf,
                    area_x + pad / 2,
                    *y,
                    area_w.saturating_sub(pad),
                    model_row_h,
                    (4.0 * sf) as usize,
                    bg,
                );
            }

            let text_y = *y + ((model_row_h as f32 - 18.0 * sf) / 2.0) as usize;
            let size_str = crate::ai::ollama::format_size(model.size);
            let info = if model.parameter_size.is_empty() {
                size_str
            } else {
                format!("{}  {}", model.parameter_size, size_str)
            };
            let line = format!("{}  ({})", model.name, info);
            let name_color = if is_selected {
                theme::PRIMARY
            } else {
                theme::SETTINGS_LABEL
            };
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                area_x + pad,
                text_y,
                clip_h,
                &line,
                label_metrics,
                name_color,
                Family::Monospace,
            );

            *y += model_row_h;
        }
    }

    *y += (12.0 * sf) as usize;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiModelsHit {
    RuntimeLocal,
    RuntimeOllama,
    OpenInFinder,
    ChangePath,
    OpenModels,
    DeleteModel(usize),
    OllamaHostInput,
    OllamaTestConnection,
    OllamaRefresh,
    OllamaSelectModel(usize),
    ToggleWebSearch,
}

pub fn settings_ai_models_hit_test(
    phys_x: f64,
    phys_y: f64,
    y_offset: usize,
    content_area_x: usize,
    content_area_w: usize,
    sf: f32,
    ollama_enabled: bool,
    ollama_model_count: usize,
) -> Option<AiModelsHit> {
    let pad = (24.0 * sf) as f64;
    let mut y = y_offset as f64;

    y += (28.0 * sf) as f64;

    let btn_h = RUNTIME_BTN_H as f64 * sf as f64;
    if phys_y >= y && phys_y < y + btn_h {
        let local_label = "Local";
        let ollama_label = "Ollama";
        let local_w = local_label.len() as f64 * 7.5 * sf as f64 + 24.0 * sf as f64;
        let ollama_w = ollama_label.len() as f64 * 7.5 * sf as f64 + 24.0 * sf as f64;
        let gap = 8.0 * sf as f64;
        let local_x = content_area_x as f64 + pad;
        let ollama_x = local_x + local_w + gap;

        if phys_x >= local_x && phys_x < local_x + local_w {
            return Some(AiModelsHit::RuntimeLocal);
        }
        if phys_x >= ollama_x && phys_x < ollama_x + ollama_w {
            return Some(AiModelsHit::RuntimeOllama);
        }
    }
    y += btn_h + (16.0 * sf) as f64;

    y += (20.0 * sf) as f64;

    if ollama_enabled {
        // "Connection" title
        y += (28.0 * sf) as f64;
        // "Host" label
        y += (22.0 * sf) as f64;

        // Host input field
        let input_h = 30.0 * sf as f64;
        let input_x = content_area_x as f64 + pad;
        let input_w = content_area_w as f64 - pad * 2.0;
        if phys_x >= input_x && phys_x < input_x + input_w && phys_y >= y && phys_y < y + input_h {
            return Some(AiModelsHit::OllamaHostInput);
        }
        y += input_h + (10.0 * sf) as f64;

        // Test Connection + Refresh buttons
        if phys_y >= y && phys_y < y + btn_h {
            let test_label = "Test Connection";
            let test_w = test_label.len() as f64 * 7.5 * sf as f64 + 24.0 * sf as f64;
            let test_x = content_area_x as f64 + pad;
            if phys_x >= test_x && phys_x < test_x + test_w {
                return Some(AiModelsHit::OllamaTestConnection);
            }
            let gap = 8.0 * sf as f64;
            let refresh_label = "Refresh";
            let refresh_w = refresh_label.len() as f64 * 7.5 * sf as f64 + 24.0 * sf as f64;
            let refresh_x = test_x + test_w + gap;
            if phys_x >= refresh_x && phys_x < refresh_x + refresh_w {
                return Some(AiModelsHit::OllamaRefresh);
            }
        }
        y += btn_h + (16.0 * sf) as f64;

        if ollama_model_count > 0 {
            y += (20.0 * sf) as f64;
            y += (28.0 * sf) as f64;

            let model_row_h = MODEL_ROW_H as f64 * sf as f64;
            for i in 0..ollama_model_count {
                let row_x = content_area_x as f64 + pad / 2.0;
                let row_w = content_area_w as f64 - pad;
                if phys_x >= row_x
                    && phys_x < row_x + row_w
                    && phys_y >= y
                    && phys_y < y + model_row_h
                {
                    return Some(AiModelsHit::OllamaSelectModel(i));
                }
                y += model_row_h;
            }
        }
        y += (12.0 * sf) as f64;
    } else {
        // Models Storage section
        y += (28.0 * sf) as f64;
        let row_h = 40.0 * sf as f64;
        y += row_h;
        y += (20.0 * sf) as f64;
        y += (24.0 * sf) as f64;

        if phys_y >= y && phys_y < y + btn_h {
            let open_label = "Open in Finder";
            let open_w = open_label.len() as f64 * 7.5 * sf as f64 + 24.0 * sf as f64;
            let open_x = content_area_x as f64 + pad;
            let btn_gap = 12.0 * sf as f64;
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
        y += btn_h + (16.0 * sf) as f64;

        // divider
        y += (20.0 * sf) as f64;

        // "Available Models" title row — Manage button is right-aligned
        let manage_label = "Manage";
        let manage_w = manage_label.len() as f64 * 7.5 * sf as f64 + 24.0 * sf as f64;
        let manage_x = content_area_x as f64 + content_area_w as f64 - pad - manage_w;
        if phys_y >= y && phys_y < y + btn_h && phys_x >= manage_x && phys_x < manage_x + manage_w {
            return Some(AiModelsHit::OpenModels);
        }
        y += (28.0 * sf) as f64;

        let models_dir = std::path::PathBuf::from(
            crate::ai::model_manager::models_dir()
                .to_string_lossy()
                .to_string(),
        );
        let model_row_h = MODEL_ROW_H as f64 * sf as f64;
        let row_gap = 2.0 * sf as f64;
        let mut any_found = false;

        for (i, model) in crate::ai::registry::MODELS.iter().enumerate() {
            let path = models_dir.join(model.filename);
            if !path.exists() {
                continue;
            }
            any_found = true;
            let row_x = content_area_x as f64 + pad / 2.0;
            let row_w = content_area_w as f64 - pad;
            if phys_x >= row_x && phys_x < row_x + row_w && phys_y >= y && phys_y < y + model_row_h
            {
                return Some(AiModelsHit::DeleteModel(i));
            }
            y += model_row_h + row_gap;
        }
        if !any_found {
            y += (22.0 * sf) as f64;
        }
        y += (12.0 * sf) as f64;
    }

    y += (20.0 * sf) as f64;
    y += (28.0 * sf) as f64;

    let row_h = 40.0 * sf as f64;
    let toggle_w = 36.0 * sf as f64;
    let toggle_h = 20.0 * sf as f64;
    let toggle_x = content_area_x as f64 + content_area_w as f64 - pad - toggle_w;
    let toggle_y = y + (row_h - toggle_h) / 2.0;
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
            if p.is_file()
                && p.extension().map(|e| e == "gguf").unwrap_or(false)
                && let Ok(meta) = p.metadata()
            {
                total += meta.len();
                count += 1;
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
