//! Models repository view — full-tab model browser rendered via pixel buffer.
//!
//! Displays all registry models as cards with status, download progress,
//! and action buttons. Supports search filtering, load/unload, auto-load toggle.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::ai;
use crate::app::views::models_view::ModelsViewState;
use crate::renderer::icons::{Icon, IconRenderer};
use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::draw_text_at;
use crate::renderer::theme;
use crate::ui::widgets::search_input::{SearchInput, SearchInputStyle};
use crate::ui::{DrawCtx, Rect, Widget};

use super::fill_rounded_rect;

const HEADER_BG: Rgb = (0, 0, 0);
const CARD_BG: Rgb = theme::BG_ELEVATED;
const CARD_HOVER: Rgb = theme::BG_SELECTION;
const TITLE_TEXT: Rgb = theme::FG_PRIMARY;
const NAME_TEXT: Rgb = theme::FG_PRIMARY;
const META_TEXT: Rgb = theme::FG_SECONDARY;
const DIM_TEXT: Rgb = theme::FG_DIM;
const BADGE_DOWNLOADING: Rgb = theme::PRIMARY;
const PROGRESS_TRACK: Rgb = theme::BG_SURFACE;
const PROGRESS_FILL: Rgb = theme::PRIMARY;
const BTN_BG: Rgb = theme::BG_ELEVATED;
const BTN_TEXT: Rgb = theme::FG_PRIMARY;
const BTN_ACTIVE_TEXT: Rgb = (255, 255, 255);
const DELETE_TEXT: Rgb = theme::ERROR;
const LOADED_BADGE_BG: Rgb = theme::PRIMARY;
const LOADED_BADGE_TEXT: Rgb = (255, 255, 255);
const TOGGLE_ON: Rgb = theme::PRIMARY;
const TOGGLE_OFF: Rgb = theme::BORDER;
const TOGGLE_KNOB: Rgb = theme::FG_BRIGHT;
const DIVIDER: Rgb = theme::SETTINGS_DIVIDER;

const HEADER_H: f32 = 64.0;
const SEARCH_H: f32 = 36.0;
const CARD_H: f32 = 80.0;
const CARD_GAP: f32 = 4.0;
const PAD_X: f32 = 24.0;
const CARD_PAD_X: f32 = 16.0;
const CARD_RADIUS: f32 = 6.0;
const SCROLLBAR_COLOR: Rgb = (80, 84, 96);
const SCROLLBAR_HOVER: Rgb = (255, 255, 255);

const SEARCH_PLACEHOLDER: &str = "Search models by name, family, or size...";

/// The style used for the models-view search bar.
fn search_style() -> SearchInputStyle {
    SearchInputStyle {
        bg: theme::BG_ELEVATED,
        border: theme::BORDER,
        focus_border: theme::PRIMARY,
        text_color: theme::FG_PRIMARY,
        placeholder_color: theme::FG_DIM,
        cursor_color: theme::FG_PRIMARY,
        radius: 6.0,
        font_size: 13.0,
        padding_x: 12.0,
    }
}

/// Draw the search bar widget at the given rect.
fn draw_search_bar(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    query: &str,
    cursor_visible: bool,
    focused: bool,
    rect: Rect,
    sf: f32,
) {
    let widget = SearchInput::new(query, SEARCH_PLACEHOLDER)
        .cursor_visible(cursor_visible)
        .focused(focused)
        .style(search_style());
    let mut painter = DrawCtx::new(buf, font_system, swash_cache, sf);
    widget.draw(&mut painter, rect);
}

/// Main draw entry point for the Models view.
pub fn draw_models_view(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &ModelsViewState,
    loaded_model_name: Option<&str>,
    auto_load: bool,
    models_path: &str,
    y_offset: usize,
    x_offset: usize,
    sf: f32,
    cursor_visible: bool,
    scrollbar_hovered: bool,
) {
    let full_w = buf.width;
    let w = full_w;
    let h = buf.height;
    let content_h = h.saturating_sub(y_offset);
    let content_w = w.saturating_sub(x_offset);
    if content_h == 0 || content_w == 0 {
        return;
    }

    buf.fill_rect(x_offset, y_offset, content_w, content_h, theme::BG);

    let pad = (PAD_X * sf) as usize;
    let lpad = x_offset + pad;

    let header_h = (HEADER_H * sf) as usize;
    buf.fill_rect(x_offset, y_offset, content_w, header_h, HEADER_BG);

    let title_metrics = Metrics::new(18.0 * sf, 24.0 * sf);
    let title_y = y_offset + ((header_h as f32 - 24.0 * sf) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        lpad,
        title_y,
        h,
        "Local Models Repository",
        title_metrics,
        TITLE_TEXT,
        Family::SansSerif,
    );

    let small_metrics = Metrics::new(11.0 * sf, 16.0 * sf);
    let label_metrics = Metrics::new(12.0 * sf, 17.0 * sf);

    if let Some(name) = loaded_model_name {
        let unload_label = "Unload Model";
        let unload_w = (unload_label.len() as f32 * 7.0 * sf) as usize + (20.0 * sf) as usize;
        let unload_h = (26.0 * sf) as usize;
        let unload_x = w.saturating_sub(pad + unload_w);
        let unload_y = y_offset + (header_h - unload_h) / 2;
        let unload_r = (4.0 * sf) as usize;
        fill_rounded_rect(
            buf, unload_x, unload_y, unload_w, unload_h, unload_r, BTN_BG,
        );
        let unload_text_y = unload_y + ((unload_h as f32 - 17.0 * sf) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            unload_x + (10.0 * sf) as usize,
            unload_text_y,
            h,
            unload_label,
            label_metrics,
            BTN_TEXT,
            Family::SansSerif,
        );

        let badge_text = name.to_string();
        let badge_w = (badge_text.len() as f32 * 7.0 * sf) as usize + (20.0 * sf) as usize;
        let badge_h = (24.0 * sf) as usize;
        let badge_x = unload_x.saturating_sub(badge_w + (8.0 * sf) as usize);
        let badge_y = y_offset + (header_h - badge_h) / 2;
        let badge_r = badge_h / 2;
        fill_rounded_rect(
            buf,
            badge_x,
            badge_y,
            badge_w,
            badge_h,
            badge_r,
            LOADED_BADGE_BG,
        );
        let text_y = badge_y + ((badge_h as f32 - 17.0 * sf) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            badge_x + (10.0 * sf) as usize,
            text_y,
            h,
            &badge_text,
            label_metrics,
            LOADED_BADGE_TEXT,
            Family::SansSerif,
        );
    }

    let toggle_w = (36.0 * sf) as usize;
    let toggle_h = (20.0 * sf) as usize;
    let toggle_label = "Auto-load";
    let toggle_label_w = (toggle_label.len() as f32 * 7.0 * sf) as usize;
    let auto_load_block_w = toggle_label_w + (8.0 * sf) as usize + toggle_w;

    let auto_load_x = if let Some(name) = &loaded_model_name {
        let unload_label = "Unload Model";
        let unload_w = (unload_label.len() as f32 * 7.0 * sf) as usize + (20.0 * sf) as usize;
        let badge_text = name.to_string();
        let badge_w = (badge_text.len() as f32 * 7.0 * sf) as usize + (20.0 * sf) as usize;
        let used_right = pad + unload_w + (8.0 * sf) as usize + badge_w + (16.0 * sf) as usize;
        w.saturating_sub(used_right + auto_load_block_w)
    } else {
        w.saturating_sub(pad + auto_load_block_w)
    };

    let toggle_label_y = y_offset + ((header_h as f32 - 17.0 * sf) / 2.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        auto_load_x,
        toggle_label_y,
        h,
        toggle_label,
        label_metrics,
        META_TEXT,
        Family::SansSerif,
    );

    let toggle_x = auto_load_x + toggle_label_w + (8.0 * sf) as usize;
    let toggle_y = y_offset + (header_h - toggle_h) / 2;
    let toggle_r = toggle_h / 2;
    let toggle_bg = if auto_load { TOGGLE_ON } else { TOGGLE_OFF };
    fill_rounded_rect(
        buf, toggle_x, toggle_y, toggle_w, toggle_h, toggle_r, toggle_bg,
    );

    let knob_d = toggle_h.saturating_sub((4.0 * sf) as usize);
    let knob_r = knob_d / 2;
    let knob_y = toggle_y + (toggle_h - knob_d) / 2;
    let knob_x = if auto_load {
        toggle_x + toggle_w - knob_d - (2.0 * sf) as usize
    } else {
        toggle_x + (2.0 * sf) as usize
    };
    fill_rounded_rect(buf, knob_x, knob_y, knob_d, knob_d, knob_r, TOGGLE_KNOB);

    let div_h = (1.0 * sf).max(1.0) as usize;
    buf.fill_rect(
        x_offset,
        y_offset + header_h - div_h,
        content_w,
        div_h,
        DIVIDER,
    );

    let search_y = y_offset + header_h + (8.0 * sf) as usize;
    let search_h = (SEARCH_H * sf) as usize;
    let search_w = content_w.saturating_sub(pad * 2);
    let search_rect = Rect::new(
        lpad as f32,
        search_y as f32,
        search_w as f32,
        search_h as f32,
    );
    draw_search_bar(
        buf,
        font_system,
        swash_cache,
        &state.search_query,
        cursor_visible,
        state.search_focused,
        search_rect,
        sf,
    );

    let scroll_area_top = search_y + search_h + (8.0 * sf) as usize;
    let card_h = (CARD_H * sf) as usize;
    let _card_gap = (CARD_GAP * sf) as usize;
    let card_r = (CARD_RADIUS * sf) as usize;
    let card_pad = (CARD_PAD_X * sf) as usize;

    let filtered = state.filtered_indices(models_path);
    let models_dir = std::path::PathBuf::from(models_path);

    let name_metrics = Metrics::new(14.0 * sf, 20.0 * sf);
    let meta_metrics = Metrics::new(11.0 * sf, 16.0 * sf);

    let count_text = format!("{} models found", filtered.len());
    let count_y = scroll_area_top + (4.0 * sf) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        lpad,
        count_y,
        h,
        &count_text,
        small_metrics,
        DIM_TEXT,
        Family::SansSerif,
    );

    let cards_y_start = count_y + (20.0 * sf) as usize;
    let clip_top = scroll_area_top;

    for (vi, &idx) in filtered.iter().enumerate() {
        let model = &ai::registry::MODELS[idx];
        let card_y_raw =
            cards_y_start as f32 + vi as f32 * (CARD_H + CARD_GAP) * sf - state.scroll_offset;
        let card_y = card_y_raw as isize;

        if card_y + card_h as isize <= clip_top as isize {
            continue;
        }
        if card_y >= h as isize {
            break;
        }
        let cy = (card_y.max(clip_top as isize)) as usize;
        let card_draw_h = if card_y < clip_top as isize {
            card_h.saturating_sub((clip_top as isize - card_y) as usize)
        } else {
            card_h.min(h.saturating_sub(cy))
        };

        let is_downloaded = models_dir.join(model.filename).exists();
        let is_downloading = state.is_downloading(model.name);
        let is_loaded = loaded_model_name == Some(model.name);
        let is_hovered = state.hovered_action == Some(vi) || state.hovered_delete == Some(vi);

        let card_bg = if is_hovered { CARD_HOVER } else { CARD_BG };
        fill_rounded_rect(
            buf,
            lpad,
            cy,
            w.saturating_sub(lpad + pad),
            card_draw_h,
            card_r,
            card_bg,
        );

        if is_loaded && card_draw_h > card_r * 2 {
            let accent_w = (3.0 * sf) as usize;
            buf.fill_rect(
                lpad,
                cy + card_r,
                accent_w,
                card_draw_h.saturating_sub(card_r * 2),
                LOADED_BADGE_BG,
            );
        }

        let real_cy = card_y.max(0) as usize;
        let name_y = real_cy + (12.0 * sf) as usize;

        if name_y >= clip_top && name_y < h {
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                lpad + card_pad,
                name_y,
                h,
                model.name,
                name_metrics,
                NAME_TEXT,
                Family::SansSerif,
            );

            let name_w = (model.name.len() as f32 * 8.5 * sf) as usize;
            let family_x = lpad + card_pad + name_w + (8.0 * sf) as usize;
            let family_badge_w =
                (model.family.len() as f32 * 6.5 * sf) as usize + (10.0 * sf) as usize;
            let family_badge_h = (16.0 * sf) as usize;
            let family_badge_y = name_y + ((20.0 * sf - 16.0 * sf) / 2.0) as usize;
            fill_rounded_rect(
                buf,
                family_x,
                family_badge_y,
                family_badge_w,
                family_badge_h,
                (3.0 * sf) as usize,
                PROGRESS_TRACK,
            );
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                family_x + (5.0 * sf) as usize,
                family_badge_y + (1.0 * sf) as usize,
                h,
                model.family,
                meta_metrics,
                META_TEXT,
                Family::SansSerif,
            );

            let meta_y = cy + (38.0 * sf) as usize;
            let size_str = format_size(model.size_bytes);
            let meta_line = format!(
                "{}  ·  {}  ·  {}  ·  ctx {}k",
                model.params,
                model.quant_label,
                size_str,
                model.context_size / 1024
            );
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                lpad + card_pad,
                meta_y,
                h,
                &meta_line,
                meta_metrics,
                META_TEXT,
                Family::SansSerif,
            );

            let repo_y = meta_y + (16.0 * sf) as usize;
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                lpad + card_pad,
                repo_y,
                h,
                model.hf_repo,
                meta_metrics,
                DIM_TEXT,
                Family::SansSerif,
            );

            let right_x = w.saturating_sub(pad + card_pad);

            if is_downloading {
                if let Some(progress) = state.active_downloads.get(model.name) {
                    if let Some(err) = &progress.error {
                        let err_text = format!("Error: {}", err);
                        draw_text_at(
                            buf,
                            font_system,
                            swash_cache,
                            right_x - (err_text.len() as f32 * 6.0 * sf) as usize,
                            name_y + (2.0 * sf) as usize,
                            h,
                            &err_text,
                            meta_metrics,
                            theme::ERROR,
                            Family::SansSerif,
                        );
                    } else {
                        let pct = progress.percent();
                        let pct_text = format!("{}%", pct);
                        let pct_w = (pct_text.len() as f32 * 7.0 * sf) as usize;
                        draw_text_at(
                            buf,
                            font_system,
                            swash_cache,
                            right_x - pct_w,
                            name_y + (2.0 * sf) as usize,
                            h,
                            &pct_text,
                            label_metrics,
                            BADGE_DOWNLOADING,
                            Family::SansSerif,
                        );

                        let bar_w = (120.0 * sf) as usize;
                        let bar_h_px = (6.0 * sf) as usize;
                        let bar_x = right_x - bar_w;
                        let bar_y = meta_y + (4.0 * sf) as usize;
                        let bar_r = bar_h_px / 2;
                        fill_rounded_rect(
                            buf,
                            bar_x,
                            bar_y,
                            bar_w,
                            bar_h_px,
                            bar_r,
                            PROGRESS_TRACK,
                        );
                        let fill_w = ((bar_w as f64) * progress.fraction()) as usize;
                        if fill_w > 0 {
                            fill_rounded_rect(
                                buf,
                                bar_x,
                                bar_y,
                                fill_w.min(bar_w),
                                bar_h_px,
                                bar_r,
                                PROGRESS_FILL,
                            );
                        }

                        let dl_text = format!(
                            "{} / {}",
                            format_size(progress.bytes_downloaded),
                            format_size(progress.bytes_total)
                        );
                        draw_text_at(
                            buf,
                            font_system,
                            swash_cache,
                            bar_x,
                            bar_y + bar_h_px + (2.0 * sf) as usize,
                            h,
                            &dl_text,
                            meta_metrics,
                            DIM_TEXT,
                            Family::SansSerif,
                        );
                    }
                }
            } else if is_downloaded {
                let icon_sz = (18.0 * sf) as u32;
                let icon_pad = (8.0 * sf) as usize;
                let icon_btn_sz = icon_sz as usize + icon_pad * 2;
                let btns_y = name_y;

                let del_x = right_x - icon_btn_sz;
                let del_hover = state.hovered_delete == Some(vi);
                let del_color = if del_hover { DELETE_TEXT } else { META_TEXT };
                icon_renderer.draw(
                    buf,
                    Icon::Trash,
                    del_x + icon_pad,
                    btns_y + icon_pad / 2,
                    icon_sz,
                    del_color,
                );

                if !is_loaded {
                    let load_x = del_x - icon_btn_sz - (4.0 * sf) as usize;
                    let load_hover = state.hovered_action == Some(vi);
                    let load_color = if load_hover {
                        BTN_ACTIVE_TEXT
                    } else {
                        META_TEXT
                    };
                    icon_renderer.draw(
                        buf,
                        Icon::Play,
                        load_x + icon_pad,
                        btns_y + icon_pad / 2,
                        icon_sz,
                        load_color,
                    );
                }
            } else {
                let icon_sz = (18.0 * sf) as u32;
                let icon_pad = (8.0 * sf) as usize;
                let icon_btn_sz = icon_sz as usize + icon_pad * 2;
                let dl_x = right_x - icon_btn_sz;
                let dl_hover = state.hovered_action == Some(vi);
                let dl_color = if dl_hover { BTN_ACTIVE_TEXT } else { META_TEXT };
                icon_renderer.draw(
                    buf,
                    Icon::Download,
                    dl_x + icon_pad,
                    name_y + icon_pad / 2,
                    icon_sz,
                    dl_color,
                );
            }
        } // end clip guard
    }

    buf.fill_rect(x_offset, y_offset, content_w, header_h, HEADER_BG);

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        lpad,
        title_y,
        h,
        "Local Models Repository",
        title_metrics,
        TITLE_TEXT,
        Family::SansSerif,
    );

    if let Some(name) = loaded_model_name {
        let unload_label = "Unload Model";
        let unload_w = (unload_label.len() as f32 * 7.0 * sf) as usize + (20.0 * sf) as usize;
        let unload_h = (26.0 * sf) as usize;
        let unload_x = w.saturating_sub(pad + unload_w);
        let unload_y = y_offset + (header_h - unload_h) / 2;
        let unload_r = (4.0 * sf) as usize;
        fill_rounded_rect(
            buf, unload_x, unload_y, unload_w, unload_h, unload_r, BTN_BG,
        );
        let unload_text_y = unload_y + ((unload_h as f32 - 17.0 * sf) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            unload_x + (10.0 * sf) as usize,
            unload_text_y,
            h,
            unload_label,
            label_metrics,
            BTN_TEXT,
            Family::SansSerif,
        );

        let badge_text = name.to_string();
        let badge_w = (badge_text.len() as f32 * 7.0 * sf) as usize + (20.0 * sf) as usize;
        let badge_h = (24.0 * sf) as usize;
        let badge_x = unload_x.saturating_sub(badge_w + (8.0 * sf) as usize);
        let badge_y = y_offset + (header_h - badge_h) / 2;
        let badge_r = badge_h / 2;
        fill_rounded_rect(
            buf,
            badge_x,
            badge_y,
            badge_w,
            badge_h,
            badge_r,
            LOADED_BADGE_BG,
        );
        let badge_text_y = badge_y + ((badge_h as f32 - 17.0 * sf) / 2.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            badge_x + (10.0 * sf) as usize,
            badge_text_y,
            h,
            &badge_text,
            label_metrics,
            LOADED_BADGE_TEXT,
            Family::SansSerif,
        );
    }

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        auto_load_x,
        toggle_label_y,
        h,
        toggle_label,
        label_metrics,
        META_TEXT,
        Family::SansSerif,
    );
    fill_rounded_rect(
        buf, toggle_x, toggle_y, toggle_w, toggle_h, toggle_r, toggle_bg,
    );
    fill_rounded_rect(buf, knob_x, knob_y, knob_d, knob_d, knob_r, TOGGLE_KNOB);

    buf.fill_rect(
        x_offset,
        y_offset + header_h - div_h,
        content_w,
        div_h,
        DIVIDER,
    );

    let search_area_h = search_y + search_h + (8.0 * sf) as usize - (y_offset + header_h);
    buf.fill_rect(
        x_offset,
        y_offset + header_h,
        content_w,
        search_area_h,
        theme::BG,
    );
    draw_search_bar(
        buf,
        font_system,
        swash_cache,
        &state.search_query,
        cursor_visible,
        state.search_focused,
        search_rect,
        sf,
    );

    draw_text_at(
        buf,
        font_system,
        swash_cache,
        lpad,
        count_y,
        h,
        &count_text,
        small_metrics,
        DIM_TEXT,
        Family::SansSerif,
    );

    if filtered.is_empty() {
        let empty_y = cards_y_start + (20.0 * sf) as usize;
        let empty_text = if state.search_query.is_empty() {
            "No models available."
        } else {
            "No models match your search."
        };
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            lpad,
            empty_y,
            h,
            empty_text,
            Metrics::new(14.0 * sf, 20.0 * sf),
            META_TEXT,
            Family::SansSerif,
        );
    }

    let total_list_h = filtered.len() as f32 * (CARD_H + CARD_GAP) * sf + 60.0 * sf;
    let available_list_h = h.saturating_sub(cards_y_start);
    if total_list_h as usize > available_list_h {
        let sb_w = (6.0 * sf).max(4.0) as usize;
        let sb_margin = (2.0 * sf) as usize;
        let track_x = w.saturating_sub(sb_w + sb_margin);
        let track_h = available_list_h;

        let thumb_h = ((available_list_h as f64 / total_list_h as f64) * track_h as f64)
            .max(20.0 * sf as f64) as usize;

        let max_scroll = (total_list_h as usize).saturating_sub(available_list_h);
        let scroll_frac = if max_scroll > 0 {
            state.scroll_offset as f64 / max_scroll as f64
        } else {
            0.0
        };
        let thumb_y =
            cards_y_start + (scroll_frac * (track_h.saturating_sub(thumb_h)) as f64) as usize;

        let sc = if scrollbar_hovered {
            SCROLLBAR_HOVER
        } else {
            SCROLLBAR_COLOR
        };
        buf.fill_rect(track_x, thumb_y, sb_w, thumb_h, sc);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelsViewHit {
    SearchBar,
    AutoLoadToggle,
    Download(usize),
    Load(usize),
    Unload(usize),
    Delete(usize),
}

/// Hit-test a click at physical coordinates against the models view layout.
pub fn models_view_hit_test(
    phys_x: f64,
    phys_y: f64,
    state: &ModelsViewState,
    loaded_model_name: Option<&str>,
    models_path: &str,
    y_offset: usize,
    buf_w: usize,
    x_offset: usize,
    sf: f32,
) -> Option<ModelsViewHit> {
    if phys_x < x_offset as f64 {
        return None;
    }
    let pad = (PAD_X * sf) as f64;
    let header_h = (HEADER_H * sf) as f64;

    if loaded_model_name.is_some() {
        let unload_label = "Unload Model";
        let unload_w = (unload_label.len() as f64) * 7.0 * sf as f64 + 20.0 * sf as f64;
        let unload_h = 26.0 * sf as f64;
        let unload_x = buf_w as f64 - pad - unload_w;
        let unload_y = y_offset as f64 + (header_h - unload_h) / 2.0;

        if phys_x >= unload_x
            && phys_x < unload_x + unload_w
            && phys_y >= unload_y
            && phys_y < unload_y + unload_h
        {
            return Some(ModelsViewHit::Unload(0));
        }
    }

    let toggle_w = 36.0 * sf as f64;
    let toggle_h = 20.0 * sf as f64;
    let toggle_label_w = ("Auto-load".len() as f64) * 7.0 * sf as f64;
    let auto_load_block_w = toggle_label_w + 8.0 * sf as f64 + toggle_w;

    let auto_load_x = if let Some(name) = &loaded_model_name {
        let unload_label = "Unload Model";
        let unload_w_ht = (unload_label.len() as f64) * 7.0 * sf as f64 + 20.0 * sf as f64;
        let badge_text = name.to_string();
        let badge_w = badge_text.len() as f64 * 7.0 * sf as f64 + 20.0 * sf as f64;
        let used_right = pad + unload_w_ht + 8.0 * sf as f64 + badge_w + 16.0 * sf as f64;
        buf_w as f64 - used_right - auto_load_block_w
    } else {
        buf_w as f64 - pad - auto_load_block_w
    };

    let toggle_x = auto_load_x + toggle_label_w + 8.0 * sf as f64;
    let toggle_y = y_offset as f64 + (header_h - toggle_h) / 2.0;

    if phys_x >= toggle_x
        && phys_x < toggle_x + toggle_w
        && phys_y >= toggle_y
        && phys_y < toggle_y + toggle_h
    {
        return Some(ModelsViewHit::AutoLoadToggle);
    }

    let search_y = y_offset as f64 + header_h + 8.0 * sf as f64;
    let search_h = SEARCH_H as f64 * sf as f64;
    if phys_y >= search_y
        && phys_y < search_y + search_h
        && phys_x >= pad
        && phys_x < buf_w as f64 - pad
    {
        return Some(ModelsViewHit::SearchBar);
    }

    let scroll_area_top = search_y + search_h + 8.0 * sf as f64;
    let count_h = 24.0 * sf as f64;
    let cards_y_start = scroll_area_top + count_h;

    let card_h = CARD_H as f64 * sf as f64;
    let card_gap = CARD_GAP as f64 * sf as f64;
    let card_pad = CARD_PAD_X as f64 * sf as f64;

    let filtered = state.filtered_indices(models_path);
    let models_dir = std::path::PathBuf::from(models_path);

    if phys_y < scroll_area_top {
        return None;
    }

    for (vi, &idx) in filtered.iter().enumerate() {
        let model = &ai::registry::MODELS[idx];
        let card_y = cards_y_start + vi as f64 * (card_h + card_gap) - state.scroll_offset as f64;

        if phys_y < card_y || phys_y >= card_y + card_h {
            continue;
        }

        let is_downloaded = models_dir.join(model.filename).exists();
        let is_downloading = state.is_downloading(model.name);
        let is_loaded = loaded_model_name == Some(model.name);

        let right_x = buf_w as f64 - pad - card_pad;
        let name_y = card_y + 12.0 * sf as f64;

        if is_downloading {
            return None;
        } else if is_downloaded {
            let icon_sz = 18.0 * sf as f64;
            let icon_pad = 8.0 * sf as f64;
            let icon_btn_sz = icon_sz + icon_pad * 2.0;

            let del_x = right_x - icon_btn_sz;
            if phys_x >= del_x
                && phys_x < del_x + icon_btn_sz
                && phys_y >= name_y
                && phys_y < name_y + icon_btn_sz
            {
                return Some(ModelsViewHit::Delete(idx));
            }

            if !is_loaded {
                let load_x = del_x - icon_btn_sz - 4.0 * sf as f64;
                if phys_x >= load_x
                    && phys_x < load_x + icon_btn_sz
                    && phys_y >= name_y
                    && phys_y < name_y + icon_btn_sz
                {
                    return Some(ModelsViewHit::Load(idx));
                }
            }
        } else {
            let icon_sz = 18.0 * sf as f64;
            let icon_pad = 8.0 * sf as f64;
            let icon_btn_sz = icon_sz + icon_pad * 2.0;
            let dl_x = right_x - icon_btn_sz;
            if phys_x >= dl_x
                && phys_x < dl_x + icon_btn_sz
                && phys_y >= name_y
                && phys_y < name_y + icon_btn_sz
            {
                return Some(ModelsViewHit::Download(idx));
            }
        }

        return None;
    }

    None
}

/// Max scroll offset for the model list.
pub fn max_scroll(state: &ModelsViewState, sf: f32, content_h: usize, models_path: &str) -> f32 {
    let count = state.filtered_indices(models_path).len();
    let total = count as f32 * (CARD_H + CARD_GAP) * sf + 60.0 * sf;
    (total - content_h as f32).max(0.0)
}

fn format_size(bytes: u64) -> String {
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
    fn format_size_gb() {
        assert_eq!(format_size(4_000_000_000), "3.7 GB");
    }

    #[test]
    fn format_size_mb() {
        assert_eq!(format_size(500_000_000), "477 MB");
    }

    #[test]
    fn format_size_zero() {
        assert_eq!(format_size(0), "0 MB");
    }

    #[test]
    fn hit_test_returns_none_outside() {
        let state = ModelsViewState::new();
        let result = models_view_hit_test(0.0, 0.0, &state, None, "/tmp", 40, 1000, 0, 2.0);
        assert!(result.is_none());
    }
}
