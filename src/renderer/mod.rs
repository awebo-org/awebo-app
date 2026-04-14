//! Renderer — orchestrates UI component drawing with backend abstraction.
//!
//! Architecture:
//! - `backend`      — enum-based GPU (wgpu) / CPU (softbuffer) selection
//! - `pixel_buffer` — raw pixel data with BGRA awareness
//! - `gpu_grid`     — GPU instanced glyph rendering (GPU backend only)
//! - `theme`        — One Dark color palette + ANSI color resolution
//! - `text`         — shared cosmic-text drawing helper
//!
//! All UI components (tab bar, grid, block view, prompt bar, overlays)
//! live under `ui::components` and are invoked from `Renderer::render()`.

pub mod backend;
pub mod glyph_atlas;
pub mod gpu_grid;
pub mod icons;
pub mod pixel_buffer;
pub mod text;
pub mod theme;

use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap};

use crate::blocks::BlockList;
use crate::prompt::PromptInfo;
use crate::terminal::JsonEventProxy;
use crate::ui::components::overlay::InputType;
use crate::ui::components::prompt_bar::InputFieldState;

pub use crate::ui::components::overlay::PaletteState;
pub use crate::ui::components::overlay::{ModelPickerItem, ModelPickerState, ModelStatus};
pub use crate::ui::components::prompt_bar::InputFieldState as SmartInputState;
pub use crate::ui::components::tab_bar::{DragState, TabInfo};

const GRID_PADDING_LOGICAL: f32 = 8.0;
const SANDBOX_PADDING_LOGICAL: f32 = 4.0;
const APP_LEFT_PAD_LOGICAL: f32 = 4.0;

/// Renderer: owns the render backend and cosmic-text font system.
///
/// Backend is either GPU (wgpu + instanced glyph rendering) or
/// CPU (softbuffer). Selection happens automatically at startup
/// with GPU preferred, or can be forced via `TERMINAL_BACKEND` env var.
pub struct Renderer {
    pub backend: backend::RenderBackend,
    pub width: u32,
    pub height: u32,

    pub font_system: FontSystem,
    pub(crate) swash_cache: SwashCache,

    pub cell_width: f32,
    pub cell_height: f32,
    pub font_size: f32,
    pub scale_factor: f64,
    pub font_family: String,

    pub tab_bar_height: u32,
    prompt_bar_height_px: u32,
    pub smart_prompt: bool,
    /// Measured monospace character width for block view output text.
    pub block_char_width: f32,

    pub(crate) pixel_buf: pixel_buffer::PixelBuffer,
    grid_cache: crate::ui::components::grid::GridCache,
    pub glyph_atlas: glyph_atlas::GlyphAtlas,

    /// Cached block view height data to avoid recomputing every frame.
    block_height_cache: BlockHeightCache,

    /// SVG icon rasterizer with per-size caching.
    pub icon_renderer: icons::IconRenderer,

    /// Avatar image renderer (PNG with rounded corners).
    pub avatar_renderer: icons::AvatarRenderer,

    /// Whether system fonts have been loaded into the font database.
    pub system_fonts_loaded: bool,

    /// Pixel widths consumed by the left and right side panels.
    /// Updated when panels open/close/resize so that `terminal_width()`
    /// returns the actual content area and PTY columns stay correct.
    pub panel_inset_left: u32,
    pub panel_inset_right: u32,
}

/// Cached total height and per-block heights for block_renderer rendering.
/// Invalidated when BlockList::generation or max_chars changes.
/// Also stores per-block cumulative visual line counts for O(log n) viewport culling,
/// and dirty-tracking state to skip re-rendering unchanged frames entirely.
pub struct BlockHeightCache {
    pub generation: u64,
    pub max_chars: usize,
    pub block_heights: Vec<f32>,
    pub total_height: f32,
    /// Per-block cumulative visual line counts (for binary search in draw).
    /// `block_cum_lines[i][j]` = total wrapped visual lines for block `i` output lines 0..=j.
    pub block_cum_lines: Vec<Vec<u32>>,

    pub last_scroll: f32,
    pub last_selection_gen: u64,
    pub last_link_gen: u64,
    pub last_scrollbar_hovered: bool,
    pub last_buf_width: usize,
    pub last_available_h: usize,
    pub last_any_pending: bool,
    pub last_overlay_active: bool,
    pub pixels_valid: bool,
    /// Cached glyphs from last dirty render — returned on non-dirty frames
    /// so GPU swap chain (which doesn't preserve content) still gets text.
    pub cached_glyphs: Vec<crate::renderer::gpu_grid::CellGlyph>,
}

impl BlockHeightCache {
    fn new() -> Self {
        Self {
            generation: u64::MAX,
            max_chars: 0,
            block_heights: Vec::new(),
            total_height: 0.0,
            block_cum_lines: Vec::new(),
            last_scroll: f32::NAN,
            last_selection_gen: u64::MAX,
            last_link_gen: u64::MAX,
            last_scrollbar_hovered: false,
            last_buf_width: 0,
            last_available_h: 0,
            last_any_pending: false,
            last_overlay_active: false,
            pixels_valid: false,
            cached_glyphs: Vec::new(),
        }
    }
}

impl Renderer {
    pub fn new(
        window: Arc<winit::window::Window>,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Self {
        let back = backend::RenderBackend::new(window, width, height);
        let is_bgra = back.is_bgra();

        let mut db = fontdb::Database::new();
        const JBM_REGULAR: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");
        const JBM_BOLD: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMono-Bold.ttf");
        const JBM_ITALIC: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMono-Italic.ttf");
        const JBM_BOLD_ITALIC: &[u8] =
            include_bytes!("../../assets/fonts/JetBrainsMono-BoldItalic.ttf");
        db.load_font_data(JBM_REGULAR.to_vec());
        db.load_font_data(JBM_BOLD.to_vec());
        db.load_font_data(JBM_ITALIC.to_vec());
        db.load_font_data(JBM_BOLD_ITALIC.to_vec());
        db.set_monospace_family("JetBrains Mono");
        db.set_sans_serif_family("JetBrains Mono");
        db.set_serif_family("JetBrains Mono");
        let mut font_system = FontSystem::new_with_locale_and_db(
            sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string()),
            db,
        );

        let swash_cache = SwashCache::new();

        let (cell_width, cell_height, font_size) = measure_cell(&mut font_system, scale_factor);

        let block_char_width = crate::ui::components::block_renderer::output_char_width(
            &mut font_system,
            scale_factor as f32,
        );

        let tab_bar_height =
            (crate::ui::components::tab_bar::TAB_BAR_LOGICAL_HEIGHT * scale_factor as f32) as u32;
        let prompt_bar_height_px =
            crate::ui::components::prompt_bar::prompt_bar_height(scale_factor as f32) as u32;

        let pixel_buf =
            pixel_buffer::PixelBuffer::new(width as usize, height as usize, is_bgra, theme::BG);

        Self {
            backend: back,
            width,
            height,
            font_system,
            swash_cache,
            cell_width,
            cell_height,
            font_size,
            scale_factor,
            font_family: "JetBrains Mono".to_string(),
            tab_bar_height,
            prompt_bar_height_px,
            smart_prompt: true,
            block_char_width,
            pixel_buf,
            grid_cache: crate::ui::components::grid::GridCache::new(),
            glyph_atlas: glyph_atlas::GlyphAtlas::new(cosmic_text::Family::Monospace),
            block_height_cache: BlockHeightCache::new(),
            icon_renderer: icons::IconRenderer::new(),
            avatar_renderer: icons::AvatarRenderer::new(),
            system_fonts_loaded: false,
            panel_inset_left: 0,
            panel_inset_right: 0,
        }
    }

    /// Command: load system fonts into the database (one-time, deferred from startup).
    pub fn ensure_system_fonts_loaded(&mut self) {
        if self.system_fonts_loaded {
            return;
        }
        self.system_fonts_loaded = true;
        self.font_system.db_mut().load_system_fonts();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.grid_cache = crate::ui::components::grid::GridCache::new();
        self.backend.resize(width, height);
    }

    /// Padding around the grid in physical pixels for the given mode.
    ///
    /// Normal terminals get breathing-room padding, sandbox gets a smaller
    /// padding, and TUI apps (is_app_controlled without sandbox) get zero.
    pub fn grid_padding_for(&self, is_app_controlled: bool, is_sandbox: bool) -> u32 {
        if is_sandbox {
            (SANDBOX_PADDING_LOGICAL * self.scale_factor as f32) as u32
        } else if is_app_controlled {
            0
        } else {
            (GRID_PADDING_LOGICAL * self.scale_factor as f32) as u32
        }
    }

    /// Force a full grid redraw on next frame (e.g. after tab switch).
    pub fn invalidate_grid_cache(&mut self) {
        self.grid_cache = crate::ui::components::grid::GridCache::new();
        self.block_height_cache = BlockHeightCache::new();
        self.pixel_buf.clear(theme::BG);
        if let backend::RenderBackend::Gpu(g) = &mut self.backend {
            g.gpu_grid.clear_atlas();
        }
    }

    /// Width available for the terminal grid (total minus padding and panel insets).
    pub fn terminal_width(&self, is_app_controlled: bool) -> u32 {
        let app_lpad = if is_app_controlled {
            (APP_LEFT_PAD_LOGICAL * self.scale_factor as f32) as u32
        } else {
            0
        };
        self.width
            .saturating_sub(self.grid_padding_for(is_app_controlled, false) * 2)
            .saturating_sub(app_lpad)
            .saturating_sub(self.panel_inset_left)
            .saturating_sub(self.panel_inset_right)
    }

    /// Height available for the terminal grid (total minus tab bar, prompt bar, and padding).
    ///
    /// When a TUI app has taken control the prompt bar is hidden, so only
    /// the tab bar and padding are subtracted.  When smart prompt is
    /// disabled (ShellPS1 mode) the prompt bar height is also zero.
    pub fn terminal_height(&self, is_app_controlled: bool) -> u32 {
        let prompt = if is_app_controlled || !self.smart_prompt {
            0
        } else {
            self.prompt_bar_height_px
        };
        self.height
            .saturating_sub(self.tab_bar_height)
            .saturating_sub(prompt)
            .saturating_sub(self.grid_padding_for(is_app_controlled, false) * 2)
    }

    /// Main render entry point — composes all layers into a frame.
    /// Returns prompt bar hit-test rects for mouse interaction.
    pub fn render(
        &mut self,
        term_handle: Option<
            &Arc<alacritty_terminal::sync::FairMutex<alacritty_terminal::Term<JsonEventProxy>>>,
        >,
        is_app_controlled: bool,
        is_sandbox: bool,
        tab_infos: &[TabInfo],
        hovered_close: Option<usize>,
        drag: &DragState,
        debug_info: Option<&str>,
        palette: Option<&PaletteState>,
        shell_picker: Option<&crate::ui::components::overlay::ShellPickerState>,
        new_tab_hovered: bool,
        shell_picker_btn_hovered: bool,
        sidebar_hovered: bool,
        sidebar_open: bool,
        git_panel_hovered: bool,
        git_panel_open: bool,
        user_menu_open: bool,
        user_menu_hovered: Option<usize>,
        is_pro: bool,
        input_type: InputType,
        prompt_info: Option<&PromptInfo>,
        input_field: &InputFieldState,
        block_list: &BlockList,
        tooltip: Option<(&str, f64, f64)>,
        widget_debug: bool,
        cursor_visible: bool,
        model_name: Option<&str>,
        ai_thinking: bool,
        model_picker: Option<&ModelPickerState>,
        block_selection: Option<&crate::blocks::BlockSelection>,
        hovered_link: Option<&crate::blocks::HoveredLink>,
        scrollbar_hovered: bool,
        editor_scrollbar: crate::ui::components::editor_renderer::ScrollbarHit,
        settings: Option<&crate::ui::components::overlay::SettingsState>,
        models_view: Option<(
            &crate::app::views::models_view::ModelsViewState,
            Option<&str>,
            bool,
            &str,
        )>,
        sessions: &[&crate::session::Session],
        side_panel_state: &crate::ui::components::side_panel::SidePanelState,
        file_tree_state: &crate::ui::file_tree::FileTreeState,
        panel_layout: &crate::ui::panel_layout::PanelLayout,
        hint_banner: &crate::ui::components::hint_banner::HintBannerState,
        editor_state: Option<&crate::ui::editor::EditorState>,
        is_fullscreen: bool,
        confirm_close: Option<(&str, Option<usize>)>,
        context_menu: Option<&crate::ui::components::context_menu::ContextMenuState>,
        sandbox_info: Option<&crate::ui::components::side_panel::SandboxInfo>,
        toast_mgr: &crate::ui::components::toast::ToastManager,
        git_panel_state: &crate::ui::components::git_panel::GitPanelState,
        usage_panel: Option<(&crate::usage::UsageTracker, Option<usize>)>,
        pro_panel: Option<(
            &crate::license::LicenseManager,
            &str,
            usize,
            bool,
            Option<usize>,
        )>,
        usage_limit_banner: Option<(
            &crate::ui::components::usage_limit_banner::UsageLimitBannerState,
            &crate::usage::UsageTracker,
        )>,
    ) -> Option<crate::ui::components::prompt_bar::PromptBarHitRects> {
        let w = self.width as usize;
        let h = self.height as usize;
        if w == 0 || h == 0 {
            return None;
        }

        let sf = self.scale_factor as f32;
        let mut prompt_hit_rects = crate::ui::components::prompt_bar::PromptBarHitRects {
            ctx_bar: None,
            stop_button: None,
        };

        self.pixel_buf.ensure_size(w, h, theme::BG);
        let mut pending_cell_glyphs: Option<Vec<crate::renderer::gpu_grid::CellGlyph>> = None;
        let mut glyph_scissor: Option<(u32, u32, u32, u32)> = None;

        let bar_h = self.tab_bar_height as usize;
        if bar_h > 0 && bar_h < h {
            crate::ui::components::tab_bar::draw(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                &mut self.icon_renderer,
                &mut self.avatar_renderer,
                bar_h,
                tab_infos,
                sf,
                hovered_close,
                drag,
                new_tab_hovered,
                shell_picker_btn_hovered,
                sidebar_hovered,
                sidebar_open,
                git_panel_hovered,
                git_panel_open,
                is_fullscreen,
            );
        }

        let mut slash_pad = 0usize;
        let mut slash_prompt_h = 0usize;

        let active_file_path = editor_state.map(|es| es.path.as_path());
        let _side_panel_w = if sidebar_open {
            crate::ui::components::side_panel::draw(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                &mut self.icon_renderer,
                sessions,
                side_panel_state,
                file_tree_state,
                active_file_path,
                panel_layout,
                bar_h,
                sf,
                sandbox_info,
            )
        } else {
            0
        };
        let content_x_offset = _side_panel_w;

        let git_panel_w = if git_panel_open {
            panel_layout.right_physical_width(sf)
        } else {
            0
        };

        let content_right_edge = w.saturating_sub(git_panel_w);

        let has_overlay = debug_info.is_some()
            || palette.is_some()
            || model_picker.is_some()
            || shell_picker.is_some()
            || settings.is_some()
            || models_view.is_some()
            || tooltip.is_some()
            || input_field.slash_menu_open
            || context_menu.is_some()
            || pro_panel.is_some()
            || usage_limit_banner.is_some();

        if let Some(settings_state) = settings {
            crate::ui::components::overlay::draw_settings(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                &mut self.icon_renderer,
                &mut self.avatar_renderer,
                settings_state,
                bar_h,
                content_x_offset,
                sf,
                is_pro,
            );

            if settings_state.font_picker_open {
                let sidebar_w = (200.0 * sf) as usize;
                let content_w = content_right_edge.saturating_sub(content_x_offset + sidebar_w);
                crate::ui::components::overlay::draw_font_picker(
                    &mut self.pixel_buf,
                    &mut self.font_system,
                    &mut self.swash_cache,
                    settings_state,
                    bar_h,
                    content_x_offset + sidebar_w,
                    content_w,
                    sf,
                );
            }
        } else if let Some((mv_state, mv_loaded, mv_auto, mv_path)) = models_view {
            crate::ui::components::overlay::models_view::draw_models_view(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                &mut self.icon_renderer,
                mv_state,
                mv_loaded,
                mv_auto,
                mv_path,
                bar_h,
                content_x_offset,
                sf,
                cursor_visible,
                scrollbar_hovered,
            );
        } else if let Some(ed) = editor_state {
            let content_h = h.saturating_sub(bar_h);
            let content_w = content_right_edge.saturating_sub(content_x_offset);
            if content_h > 0 && content_w > 0 {
                crate::ui::components::editor_renderer::draw(
                    &mut self.pixel_buf,
                    &mut self.font_system,
                    &mut self.swash_cache,
                    &mut self.glyph_atlas,
                    ed,
                    bar_h,
                    content_h,
                    content_x_offset,
                    content_w,
                    sf,
                    editor_scrollbar,
                    cursor_visible,
                );
            }
        } else if let Some(term_handle) = term_handle {
            let is_app = is_app_controlled;
            let pad = self.grid_padding_for(is_app, is_sandbox) as usize;
            let app_lpad = if is_app {
                (APP_LEFT_PAD_LOGICAL * sf) as usize
            } else {
                0
            };

            let prompt_h = if input_type == InputType::Smart && !is_app {
                if prompt_info.is_some() {
                    crate::ui::components::prompt_bar::prompt_bar_height(sf)
                } else {
                    0
                }
            } else {
                0
            };

            slash_pad = pad + content_x_offset;
            slash_prompt_h = prompt_h;

            let smart_mode = input_type == InputType::Smart && !is_app;
            let use_block_renderer = smart_mode;

            let banner_h = if smart_mode {
                crate::ui::components::hint_banner::banner_height(hint_banner, sf)
            } else {
                0
            };
            let banner_gap = if smart_mode && banner_h > 0 {
                (10.0 * sf) as usize
            } else {
                0
            };

            let grid_y = bar_h;
            let grid_h = h
                .saturating_sub(grid_y)
                .saturating_sub(prompt_h)
                .saturating_sub(banner_h)
                .saturating_sub(banner_gap);

            if use_block_renderer {
                if grid_h > 0 {
                    let y_end = grid_y + grid_h;
                    let block_glyphs = crate::ui::components::block_renderer::draw(
                        &mut self.pixel_buf,
                        &mut self.font_system,
                        &mut self.swash_cache,
                        block_list,
                        grid_y,
                        y_end,
                        pad,
                        content_x_offset,
                        sf,
                        self.block_char_width,
                        block_selection,
                        hovered_link,
                        &mut self.block_height_cache,
                        scrollbar_hovered,
                        has_overlay,
                    );
                    if !block_glyphs.is_empty() {
                        glyph_scissor = Some((
                            content_x_offset as u32,
                            grid_y as u32,
                            (content_right_edge - content_x_offset) as u32,
                            grid_h as u32,
                        ));
                        pending_cell_glyphs = Some(block_glyphs);
                    }
                }
            } else {
                if grid_h > 0 {
                    let grid_bg = if is_sandbox {
                        Some((0, 0, 0))
                    } else if is_app {
                        let bg = {
                            use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
                            let term = term_handle.lock();
                            let colors = term.renderable_content().colors;
                            theme::resolve_color(&AnsiColor::Named(NamedColor::Background), colors)
                        };
                        Some(bg)
                    } else {
                        None
                    };
                    if is_sandbox {
                        if let Some(bg) = grid_bg {
                            self.pixel_buf.fill_rect(
                                content_x_offset,
                                grid_y,
                                content_right_edge.saturating_sub(content_x_offset),
                                grid_h,
                                bg,
                            );
                        }
                    }

                    let sandbox_initializing =
                        sandbox_info.map(|s| s.is_initializing).unwrap_or(false);

                    if is_sandbox && sandbox_initializing {
                        self.draw_sandbox_pull_progress(
                            sandbox_info,
                            content_x_offset,
                            grid_y,
                            content_right_edge.saturating_sub(content_x_offset),
                            grid_h,
                            sf,
                        );
                    } else {
                        let grid_x = content_x_offset + app_lpad;
                        let cell_glyphs = crate::ui::components::grid::draw(
                            &mut self.pixel_buf,
                            term_handle,
                            grid_y,
                            pad,
                            grid_x,
                            self.cell_width,
                            self.cell_height,
                            self.scale_factor,
                            &mut self.grid_cache,
                            has_overlay,
                            self.font_size,
                            grid_bg,
                            grid_h,
                        );
                        if app_lpad > 0 {
                            if let Some(bg) = grid_bg {
                                self.pixel_buf.fill_rect(
                                    content_x_offset,
                                    grid_y,
                                    app_lpad,
                                    grid_h,
                                    bg,
                                );
                            }
                        }
                        pending_cell_glyphs = Some(cell_glyphs);
                        glyph_scissor = Some((
                            content_x_offset as u32,
                            grid_y as u32,
                            (content_right_edge - content_x_offset) as u32,
                            grid_h as u32,
                        ));
                    }
                }
            }

            if prompt_h > 0
                && let Some(info) = prompt_info
            {
                let prompt_y = h.saturating_sub(prompt_h);
                let command_running = block_list.last_is_running();
                prompt_hit_rects = crate::ui::components::prompt_bar::draw(
                    &mut self.pixel_buf,
                    &mut self.font_system,
                    &mut self.swash_cache,
                    &mut self.icon_renderer,
                    info,
                    input_field,
                    pad + content_x_offset,
                    pad + git_panel_w,
                    prompt_y,
                    sf,
                    cursor_visible,
                    command_running,
                    model_name,
                    ai_thinking,
                );
            }

            if banner_h > 0 {
                let banner_y = h.saturating_sub(prompt_h + banner_h);
                if banner_gap > 0 {
                    let gap_y = banner_y.saturating_sub(banner_gap);
                    self.pixel_buf.fill_rect(
                        content_x_offset,
                        gap_y,
                        content_right_edge.saturating_sub(content_x_offset),
                        banner_gap,
                        crate::renderer::theme::BG,
                    );
                }
                let banner_w = content_right_edge.saturating_sub(content_x_offset);
                crate::ui::components::hint_banner::draw(
                    &mut self.pixel_buf,
                    &mut self.font_system,
                    &mut self.swash_cache,
                    &mut self.icon_renderer,
                    hint_banner,
                    content_x_offset + pad,
                    banner_y,
                    banner_w.saturating_sub(pad * 2),
                    content_x_offset,
                    w,
                    sf,
                );
            }
        }

        if git_panel_open {
            crate::ui::components::git_panel::draw(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                &mut self.icon_renderer,
                git_panel_state,
                panel_layout,
                bar_h,
                sf,
                cursor_visible,
            );
        }

        if has_overlay && let Some(glyphs) = &pending_cell_glyphs {
            self.blit_glyphs_cpu(glyphs);
        }

        if input_field.slash_menu_open && slash_prompt_h > 0 {
            let prompt_y = h.saturating_sub(slash_prompt_h);
            crate::ui::components::prompt_bar::draw_slash_menu(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                input_field,
                slash_pad,
                prompt_y,
                sf,
            );
        }

        if let Some(info) = debug_info {
            crate::ui::components::overlay::draw_debug(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                info,
                sf,
            );
        }

        if let Some(palette) = palette {
            crate::ui::components::overlay::draw_palette(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                palette,
                sf,
            );
        }

        if let Some(picker) = model_picker {
            crate::ui::components::overlay::draw_model_picker(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                picker,
                sf,
            );
        }

        if let Some(picker) = shell_picker {
            let tab_count = tab_infos.len();
            let sidebar_logical = 8.0 + 18.0 + 8.0;
            let left_logical = crate::ui::components::tab_bar::left_padding(is_fullscreen) as f64
                + sidebar_logical;
            let tab_w_logical = if tab_count == 0 {
                220.0
            } else {
                let avail = (w as f64 / self.scale_factor)
                    - 36.0
                    - 4.0
                    - 32.0
                    - left_logical
                    - 28.0
                    - 6.0
                    - 12.0;
                (avail / tab_count as f64).clamp(120.0, 220.0)
            };
            let tab_w_phys = (tab_w_logical * self.scale_factor) as usize;
            let left_pad = (left_logical * sf as f64) as usize;
            let anchor_x = left_pad + tab_count * tab_w_phys + (36.0 * sf) as usize;
            let anchor_y = bar_h;

            crate::ui::components::overlay::draw_shell_picker(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                picker,
                anchor_x,
                anchor_y,
                sf,
            );
        }

        if let Some((text, tx, ty)) = tooltip {
            crate::ui::components::tab_bar::draw_tooltip(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                text,
                tx,
                ty,
                sf,
            );
        }

        if user_menu_open {
            crate::ui::components::tab_bar::draw_user_menu(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                bar_h,
                w,
                sf,
                user_menu_hovered,
                is_pro,
            );
        }

        if let Some(cm) = context_menu {
            crate::ui::components::context_menu::draw_context_menu(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                cm,
                sf,
            );
        }

        if let Some((file_name, hovered_btn)) = confirm_close {
            crate::ui::components::overlay::draw_confirm_close(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                file_name,
                hovered_btn,
                sf,
            );
        }

        if let Some((tracker, hovered)) = usage_panel {
            crate::ui::components::overlay::usage_panel::draw_usage_panel(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                tracker,
                hovered,
                sf,
            );
        }

        if let Some((license_mgr, license_input, cursor_pos, input_focused, hovered)) = pro_panel {
            crate::ui::components::overlay::pro_panel::draw_pro_panel(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                license_mgr,
                license_input,
                cursor_pos,
                input_focused,
                hovered,
                sf,
            );
        }

        if let Some((banner_state, tracker)) = usage_limit_banner {
            crate::ui::components::usage_limit_banner::draw(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                &mut self.icon_renderer,
                banner_state,
                tracker,
                sf,
            );
        }

        crate::ui::components::toast::draw_toasts(
            &mut self.pixel_buf,
            &mut self.font_system,
            &mut self.swash_cache,
            toast_mgr,
            sf,
        );

        if widget_debug {
            let backend_label = format!("{}", self.backend);
            let mut ctx = crate::ui::DrawCtx::new(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                sf,
            );
            crate::ui::debug_viewer::draw(&mut ctx, &backend_label);
        }

        if !self.backend.is_gpu()
            && !has_overlay
            && let Some(glyphs) = &pending_cell_glyphs
        {
            self.blit_glyphs_cpu(glyphs);
        }

        let dirty = self.pixel_buf.dirty_range();
        let frame = self
            .backend
            .begin_frame(&self.pixel_buf.data, w as u32, h as u32, dirty);

        if let Some(frame) = frame {
            if !has_overlay
                && let backend::RenderBackend::Gpu(g) = &mut self.backend
                && let Some(glyphs) = &pending_cell_glyphs
                && !glyphs.is_empty()
            {
                g.gpu_grid.render(
                    glyphs,
                    &mut self.glyph_atlas,
                    &mut self.font_system,
                    &mut self.swash_cache,
                    &g.device,
                    &g.queue,
                    &frame.view,
                    w as f32,
                    h as f32,
                    glyph_scissor,
                );
            }
            self.backend.end_frame(frame);
        }

        Some(prompt_hit_rects)
    }

    /// Draw Docker-style per-layer pull progress on the sandbox init screen.
    fn draw_sandbox_pull_progress(
        &mut self,
        sandbox_info: Option<&crate::ui::components::side_panel::SandboxInfo>,
        area_x: usize,
        area_y: usize,
        area_w: usize,
        area_h: usize,
        sf: f32,
    ) {
        use crate::sandbox::bridge::{LayerPhase, PullPhase};

        let ps = match sandbox_info {
            Some(info) => &info.pull_state,
            None => return,
        };

        let pad = (24.0 * sf) as usize;
        let line_h = (18.0 * sf) as usize;
        let bar_h = (6.0 * sf).max(2.0) as usize;
        let bar_r = bar_h / 2;
        let label_metrics = cosmic_text::Metrics::new(12.0 * sf, 16.0 * sf);
        let title_metrics = cosmic_text::Metrics::new(13.0 * sf, 18.0 * sf);
        let muted: (u8, u8, u8) = (100, 100, 100);
        let text_color: (u8, u8, u8) = (160, 160, 160);
        let bar_bg: (u8, u8, u8) = (40, 40, 40);
        let bar_fill: (u8, u8, u8) = (80, 180, 80);
        let clip_h = area_y + area_h;

        let max_w = area_w.min((420.0 * sf) as usize);
        let cx = area_x + pad;
        let mut y = area_y + pad;

        let title = match ps.phase {
            PullPhase::Resolving => "Resolving image...",
            PullPhase::Pulling => "Pulling image layers",
            PullPhase::Complete => "Starting sandbox...",
        };
        crate::renderer::text::draw_text_at(
            &mut self.pixel_buf,
            &mut self.font_system,
            &mut self.swash_cache,
            cx,
            y,
            clip_h,
            title,
            title_metrics,
            text_color,
            cosmic_text::Family::Monospace,
        );
        y += line_h + (8.0 * sf) as usize;

        if ps.layers.is_empty() && ps.phase == PullPhase::Resolving {
            crate::renderer::text::draw_text_at(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                cx,
                y,
                clip_h,
                "Contacting registry...",
                label_metrics,
                muted,
                cosmic_text::Family::Monospace,
            );
            return;
        }

        let bar_w = max_w - pad * 2 - (100.0 * sf) as usize;
        for (i, layer) in ps.layers.iter().enumerate() {
            if y + line_h + bar_h > clip_h {
                break;
            }

            let (status, pct) = match layer.phase {
                LayerPhase::Waiting => ("waiting".to_string(), 0.0_f32),
                LayerPhase::Downloading => {
                    let p = match layer.total {
                        Some(t) if t > 0 => layer.downloaded as f32 / t as f32,
                        _ => 0.0,
                    };
                    (format_bytes(layer.downloaded, layer.total), p)
                }
                LayerPhase::Downloaded => ("downloaded".to_string(), 1.0),
                LayerPhase::Extracting => {
                    let p = if layer.extract_total > 0 {
                        layer.extracted as f32 / layer.extract_total as f32
                    } else {
                        0.5
                    };
                    (format!("extracting {:.0}%", p * 100.0), p)
                }
                LayerPhase::Done => ("done ✓".to_string(), 1.0),
            };

            let phase_color = match layer.phase {
                LayerPhase::Done => (80, 180, 80),
                LayerPhase::Downloading | LayerPhase::Extracting => text_color,
                _ => muted,
            };

            let label = format!("Layer {}: {}", i + 1, status);
            crate::renderer::text::draw_text_at(
                &mut self.pixel_buf,
                &mut self.font_system,
                &mut self.swash_cache,
                cx,
                y,
                clip_h,
                &label,
                label_metrics,
                phase_color,
                cosmic_text::Family::Monospace,
            );
            y += line_h;

            if layer.phase != LayerPhase::Waiting && layer.phase != LayerPhase::Done {
                let bar_x = cx;
                crate::ui::components::overlay::fill_rounded_rect(
                    &mut self.pixel_buf,
                    bar_x,
                    y,
                    bar_w,
                    bar_h,
                    bar_r,
                    bar_bg,
                );
                let fill_w = ((bar_w as f32 * pct.clamp(0.0, 1.0)) as usize).max(1);
                crate::ui::components::overlay::fill_rounded_rect(
                    &mut self.pixel_buf,
                    bar_x,
                    y,
                    fill_w,
                    bar_h,
                    bar_r,
                    bar_fill,
                );
            }
            y += bar_h + (6.0 * sf) as usize;
        }
    }

    /// Blit CellGlyphs into the pixel buffer using the CPU glyph atlas.
    /// Used as a fallback when the GPU backend is not available.
    fn blit_glyphs_cpu(&mut self, glyphs: &[gpu_grid::CellGlyph]) {
        for g in glyphs {
            let raster = self.glyph_atlas.get_or_rasterize(
                g.ch,
                g.font_size,
                g.line_height,
                g.bold,
                g.italic,
                &mut self.font_system,
                &mut self.swash_cache,
            );
            if let Some(r) = raster {
                let dx = g.px as i32 + r.bearing_x;
                let dy = g.py as i32 + r.bearing_y;
                for row in 0..r.height {
                    for col in 0..r.width {
                        let alpha = r.alphas[row * r.width + col];
                        if alpha == 0 {
                            continue;
                        }
                        let px = dx + col as i32;
                        let py = dy + row as i32;
                        if px >= 0 && py >= 0 {
                            self.pixel_buf.blend_pixel(
                                px as usize,
                                py as usize,
                                g.fg,
                                alpha as f32 / 255.0,
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Format byte counts for pull progress display (e.g. "1.2 MB / 4.5 MB").
fn format_bytes(downloaded: u64, total: Option<u64>) -> String {
    fn human(b: u64) -> String {
        if b >= 1_000_000 {
            format!("{:.1} MB", b as f64 / 1_000_000.0)
        } else if b >= 1_000 {
            format!("{:.0} KB", b as f64 / 1_000.0)
        } else {
            format!("{} B", b)
        }
    }
    match total {
        Some(t) if t > 0 => format!("{} / {}", human(downloaded), human(t)),
        _ => human(downloaded),
    }
}

/// Measure monospace cell dimensions in physical pixels.
fn measure_cell(font_system: &mut FontSystem, scale_factor: f64) -> (f32, f32, f32) {
    let base_font_size = 14.0;
    let base_line_height = 19.0;
    let font_size = base_font_size * scale_factor as f32;
    let line_height = base_line_height * scale_factor as f32;
    let metrics = Metrics::new(font_size, line_height);

    let mut buf = Buffer::new(font_system, metrics);
    buf.set_size(font_system, Some(500.0), Some(line_height));
    buf.set_wrap(font_system, Wrap::None);
    buf.set_text(
        font_system,
        "M",
        &Attrs::new().family(Family::Monospace),
        Shaping::Advanced,
        None,
    );
    buf.shape_until_scroll(font_system, true);

    let cell_width = buf
        .layout_runs()
        .next()
        .and_then(|run| run.glyphs.first().map(|g| g.w))
        .unwrap_or(8.4);

    (cell_width, line_height, font_size)
}

pub fn measure_cell_width(
    font_system: &mut FontSystem,
    font_size: f32,
    line_height: f32,
    font_family: &str,
) -> f32 {
    let family = resolve_family(font_family);
    let metrics = Metrics::new(font_size, line_height);
    let mut buf = Buffer::new(font_system, metrics);
    buf.set_size(font_system, Some(500.0), Some(line_height));
    buf.set_wrap(font_system, Wrap::None);
    buf.set_text(
        font_system,
        "M",
        &Attrs::new().family(family),
        Shaping::Advanced,
        None,
    );
    buf.shape_until_scroll(font_system, true);
    buf.layout_runs()
        .next()
        .and_then(|run| run.glyphs.first().map(|g| g.w))
        .unwrap_or(8.4)
}

pub fn resolve_family(name: &str) -> Family<'_> {
    match name {
        "Monospace" => Family::Monospace,
        "SansSerif" | "Sans Serif" => Family::Monospace,
        "Serif" => Family::Serif,
        other => Family::Name(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::Family;

    #[test]
    fn resolve_family_monospace() {
        assert!(matches!(resolve_family("Monospace"), Family::Monospace));
    }

    #[test]
    fn resolve_family_sans_serif() {
        assert!(matches!(resolve_family("SansSerif"), Family::Monospace));
        assert!(matches!(resolve_family("Sans Serif"), Family::Monospace));
    }

    #[test]
    fn resolve_family_serif() {
        assert!(matches!(resolve_family("Serif"), Family::Serif));
    }

    #[test]
    fn resolve_family_custom_name() {
        match resolve_family("JetBrains Mono") {
            Family::Name(n) => assert_eq!(n, "JetBrains Mono"),
            _ => panic!("expected Name variant"),
        }
    }
}
