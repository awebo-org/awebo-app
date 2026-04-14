pub(crate) mod actions;
mod inference;
pub mod router;
mod settings;
mod state;
mod tabs;
pub(crate) mod views;

pub(crate) use state::OverlayState;

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowAttributes, WindowId};

#[cfg(target_os = "macos")]
use winit::platform::macos::{
    ActiveEventLoopExtMacOS, EventLoopBuilderExtMacOS, WindowAttributesExtMacOS,
};

use crate::ai;
use crate::blocks;
use crate::menu::AppMenu;
use crate::renderer::{ModelPickerItem, ModelPickerState, ModelStatus, PaletteState, Renderer};
use crate::terminal::TerminalEvent;
use crate::ui::components::overlay::{
    InputType, SandboxImageInfo, SandboxSettingsState, SettingsState, ShellInfo, ShellPickerState,
};
use crate::ui::components::tab_bar::DragState;

pub(crate) use tabs::{Tab, TabKind, TabManager};

pub(crate) struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    config: crate::config::AppConfig,
    tab_mgr: TabManager,
    proxy: winit::event_loop::EventLoopProxy<TerminalEvent>,
    modifiers: ModifiersState,
    cursor_pos: (f64, f64),

    overlay: OverlayState,

    frame_times: Vec<Instant>,
    last_fps: f32,
    last_frame_ms: f64,

    drag: DragState,

    available_shells: Vec<(String, String)>,

    settings_state: SettingsState,

    ai_ctrl: crate::ai_controller::AiController,
    smart_input: crate::renderer::SmartInputState,
    agent: Option<crate::agent::orchestrator::AgentOrchestrator>,
    agent_command: Option<String>,
    pending_agent_task: Option<String>,
    pending_ai_query: Option<String>,
    app_menu: Option<AppMenu>,
    widget_debug: crate::ui::debug_viewer::DebugViewerState,
    cursor_blink_at: Instant,
    cursor_blink_on: bool,
    pending_redraw: bool,
    selecting: bool,
    block_selection: Option<crate::blocks::BlockSelection>,
    hovered_link: Option<crate::blocks::HoveredLink>,
    models_view: crate::app::views::models_view::ModelsViewState,
    /// Registry index of a model being auto-downloaded for agent use.
    auto_download_model_idx: Option<usize>,
    session_mgr: crate::session::SessionManager,
    side_panel: crate::ui::components::side_panel::SidePanelState,
    git_panel: crate::ui::components::git_panel::GitPanelState,
    panel_layout: crate::ui::panel_layout::PanelLayout,
    file_tree: crate::ui::file_tree::FileTreeState,
    hint_banner: crate::ui::components::hint_banner::HintBannerState,
    scrollbar_hovered: bool,
    scrollbar_dragging: bool,
    scrollbar_drag_start_y: f64,
    scrollbar_drag_start_scroll: f32,
    editor_scrollbar: crate::ui::components::editor_renderer::ScrollbarHit,
    editor_scrollbar_dragging: crate::ui::components::editor_renderer::ScrollbarHit,
    editor_selecting: bool,
    diff_split_dragging: bool,
    diff_split_hovered: bool,
    mouse_forwarding: bool,
    is_fullscreen: bool,
    syntax: crate::ui::syntax::SyntaxRegistry,
    context_menu: Option<crate::ui::components::context_menu::ContextMenuState>,
    context_menu_target_path: Option<std::path::PathBuf>,
    context_menu_target_tab: Option<usize>,
    sandbox_mgr: crate::sandbox::manager::SandboxManager,
    toast_mgr: crate::ui::components::toast::ToastManager,
    git_poll_at: Instant,
    file_tree_poll_at: Instant,
    usage_tracker: crate::usage::UsageTracker,
    license_mgr: crate::license::LicenseManager,
    usage_limit_banner: crate::ui::components::usage_limit_banner::UsageLimitBannerState,
}

impl App {
    fn new(proxy: winit::event_loop::EventLoopProxy<TerminalEvent>) -> Self {
        let available_shells = crate::terminal::detect_shells();
        let config = crate::config::AppConfig::load();

        let input_type = match config.appearance.input_type.as_str() {
            "shell_ps1" => InputType::ShellPS1,
            _ => InputType::Smart,
        };

        let settings_state = SettingsState {
            input_type,
            font_family: config.appearance.font_family.clone(),
            font_size_px: config.appearance.font_size,
            line_height_px: config.appearance.line_height,
            models_path: config.ai.models_path.clone(),
            web_search_enabled: config.ai.web_search,
            sandbox: SandboxSettingsState {
                cpus: config.sandbox.default_cpus,
                memory_mib: config.sandbox.default_memory_mib,
                ..Default::default()
            },
            ..Default::default()
        };

        let hint_banner_dismissed = config.general.hint_banner_dismissed;

        Self {
            window: None,
            renderer: None,
            config,
            tab_mgr: TabManager::new(),
            proxy,
            modifiers: ModifiersState::empty(),
            cursor_pos: (0.0, 0.0),
            overlay: OverlayState::default(),
            frame_times: Vec::new(),
            last_fps: 0.0,
            last_frame_ms: 0.0,
            drag: DragState::default(),
            available_shells,
            settings_state,
            ai_ctrl: crate::ai_controller::AiController::new(),
            smart_input: crate::renderer::SmartInputState::new(),
            agent: None,
            agent_command: None,
            pending_agent_task: None,
            pending_ai_query: None,
            app_menu: None,
            widget_debug: crate::ui::debug_viewer::DebugViewerState::new(),
            cursor_blink_at: Instant::now(),
            cursor_blink_on: true,
            pending_redraw: false,
            selecting: false,
            block_selection: None,
            hovered_link: None,
            models_view: crate::app::views::models_view::ModelsViewState::new(),
            auto_download_model_idx: None,
            session_mgr: crate::session::SessionManager::new(),
            side_panel: crate::ui::components::side_panel::SidePanelState::default(),
            git_panel: crate::ui::components::git_panel::GitPanelState::default(),
            panel_layout: crate::ui::panel_layout::PanelLayout::default(),
            file_tree: {
                let mut ft = crate::ui::file_tree::FileTreeState::default();
                ft.load(std::path::Path::new("."));
                ft
            },
            hint_banner: {
                let mut hb = crate::ui::components::hint_banner::HintBannerState::default();
                if hint_banner_dismissed {
                    hb.dismiss();
                }
                hb
            },
            scrollbar_hovered: false,
            scrollbar_dragging: false,
            scrollbar_drag_start_y: 0.0,
            scrollbar_drag_start_scroll: 0.0,
            editor_scrollbar: crate::ui::components::editor_renderer::ScrollbarHit::None,
            editor_scrollbar_dragging: crate::ui::components::editor_renderer::ScrollbarHit::None,
            editor_selecting: false,
            diff_split_dragging: false,
            diff_split_hovered: false,
            mouse_forwarding: false,
            is_fullscreen: false,
            syntax: {
                let mut reg = crate::ui::syntax::SyntaxRegistry::new();
                reg.load_defaults();
                reg
            },
            context_menu: None,
            context_menu_target_path: None,
            context_menu_target_tab: None,
            sandbox_mgr: crate::sandbox::manager::SandboxManager::new(),
            toast_mgr: crate::ui::components::toast::ToastManager::new(),
            git_poll_at: Instant::now(),
            file_tree_poll_at: Instant::now(),
            usage_tracker: {
                let lm = crate::license::LicenseManager::load();
                let pro = lm.is_pro();
                crate::usage::UsageTracker::new(pro)
            },
            license_mgr: crate::license::LicenseManager::load(),
            usage_limit_banner:
                crate::ui::components::usage_limit_banner::UsageLimitBannerState::default(),
        }
    }

    fn update_fps(&mut self) {
        let now = Instant::now();
        self.frame_times.push(now);
        let cutoff = now - std::time::Duration::from_secs(1);
        self.frame_times.retain(|t| *t >= cutoff);
        self.last_fps = self.frame_times.len() as f32;
    }

    /// Update all hover states. Returns `true` if side panel hover changed.
    fn update_hover(&mut self) -> bool {
        let mut panel_changed = false;
        if let Some(renderer) = &self.renderer {
            let fs = self.is_fullscreen;
            self.overlay.hovered_close = crate::ui::components::tab_bar::hovered_close_tab(
                self.cursor_pos.0,
                self.cursor_pos.1,
                self.tab_mgr.len(),
                renderer.tab_bar_height as f64,
                renderer.width as f64,
                renderer.scale_factor,
                fs,
            );
            self.overlay.avatar_hovered = crate::ui::components::tab_bar::is_avatar_hovered(
                self.cursor_pos.0,
                self.cursor_pos.1,
                renderer.tab_bar_height as f64,
                renderer.width as f64,
                renderer.scale_factor,
            );
            self.overlay.new_tab_hovered = crate::ui::components::tab_bar::is_new_tab_hovered(
                self.cursor_pos.0,
                self.cursor_pos.1,
                self.tab_mgr.len(),
                renderer.tab_bar_height as f64,
                renderer.width as f64,
                renderer.scale_factor,
                fs,
            );
            self.overlay.shell_picker_btn_hovered =
                crate::ui::components::tab_bar::is_shell_picker_hovered(
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    self.tab_mgr.len(),
                    renderer.tab_bar_height as f64,
                    renderer.width as f64,
                    renderer.scale_factor,
                    fs,
                );
            self.overlay.sidebar_hovered = crate::ui::components::tab_bar::is_sidebar_hovered(
                self.cursor_pos.0,
                self.cursor_pos.1,
                renderer.tab_bar_height as f64,
                renderer.scale_factor,
                fs,
            );
            self.overlay.git_panel_hovered = crate::ui::components::tab_bar::is_git_panel_hovered(
                self.cursor_pos.0,
                self.cursor_pos.1,
                renderer.tab_bar_height as f64,
                renderer.width as f64,
                renderer.scale_factor,
            );

            if self.overlay.sidebar_open {
                let session_count = self.session_mgr.count();
                let sandbox_hover_info = self.build_sandbox_info();
                let sp_changed = crate::ui::components::side_panel::update_hover(
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    session_count,
                    renderer.tab_bar_height as f64,
                    renderer.scale_factor,
                    self.side_panel.scroll_offset,
                    &mut self.side_panel,
                    &self.panel_layout,
                    sandbox_hover_info.as_ref(),
                );
                panel_changed |= sp_changed;
                if self.panel_layout.active_tab == crate::ui::panel_layout::SidePanelTab::Files {
                    let header_h = 40.0 * renderer.scale_factor;
                    let border_w = (1.0 * renderer.scale_factor).max(1.0);
                    let content_y = (renderer.tab_bar_height as f64 + header_h + border_w) as usize;
                    let row_count = self.file_tree.row_count();
                    let panel_w = self
                        .panel_layout
                        .left_physical_width(renderer.scale_factor as f32);
                    let ft_changed = crate::ui::file_tree::update_hover(
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                        panel_w,
                        content_y,
                        self.file_tree.scroll_offset,
                        row_count,
                        &mut self.file_tree,
                        renderer.scale_factor,
                    );
                    panel_changed |= ft_changed;

                    let item_h = (crate::ui::file_tree::ITEM_HEIGHT_PX
                        * renderer.scale_factor as f32) as usize;
                    let pad_y =
                        (crate::ui::file_tree::PAD_Y_PX * renderer.scale_factor as f32) as usize;
                    let visible_h = (renderer.height as usize).saturating_sub(content_y);
                    let total_h = row_count * item_h + pad_y * 2;
                    let prev_sb = self.file_tree.scrollbar_hovered;
                    self.file_tree.scrollbar_hovered = !self.file_tree.scrollbar_dragging
                        && crate::ui::components::side_panel::panel_scrollbar_hit_test(
                            self.cursor_pos.0 as usize,
                            self.cursor_pos.1 as usize,
                            panel_w,
                            content_y,
                            visible_h,
                            total_h,
                            self.file_tree.scroll_offset as usize,
                            renderer.scale_factor as f32,
                        );
                    if self.file_tree.scrollbar_hovered != prev_sb {
                        panel_changed = true;
                    }
                } else {
                    if self.file_tree.hovered_idx.is_some() {
                        self.file_tree.hovered_idx = None;
                        panel_changed = true;
                    }
                }
            } else {
                if self.side_panel.hovered_item.is_some()
                    || self.side_panel.hovered_clear.is_some()
                    || self.side_panel.hovered_toolbar_btn.is_some()
                    || self.file_tree.hovered_idx.is_some()
                {
                    panel_changed = true;
                }
                self.side_panel.hovered_item = None;
                self.side_panel.hovered_clear = None;
                self.side_panel.hovered_toolbar_btn = None;
                self.file_tree.hovered_idx = None;
                self.file_tree.scrollbar_hovered = false;
            }

            if self.overlay.git_panel_open {
                let gp_changed = crate::ui::components::git_panel::update_hover(
                    &mut self.git_panel,
                    &self.panel_layout,
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    renderer.tab_bar_height as f64,
                    renderer.width as usize,
                    renderer.scale_factor,
                );
                panel_changed |= gp_changed;

                let sf = renderer.scale_factor as f32;
                let header_h = (40.0 * sf) as usize;
                let border_w = (1.0 * sf).max(1.0) as usize;
                let panel_w = self.panel_layout.right_physical_width(sf);
                let panel_x = (renderer.width as usize).saturating_sub(panel_w);
                let content_y = renderer.tab_bar_height as usize + header_h + border_w;
                let visible_h = (renderer.height as usize).saturating_sub(content_y);
                let total_h = crate::ui::components::git_panel::content_height(
                    &self.git_panel,
                    self.panel_layout.git_tab,
                    sf,
                ) as usize;
                let prev_sb = self.git_panel.scrollbar_hovered;
                self.git_panel.scrollbar_hovered = !self.git_panel.scrollbar_dragging
                    && crate::ui::components::git_panel::git_scrollbar_hit_test(
                        self.cursor_pos.0 as usize,
                        self.cursor_pos.1 as usize,
                        panel_x,
                        panel_w,
                        content_y,
                        visible_h,
                        total_h,
                        self.git_panel.scroll_offset as usize,
                        sf,
                    );
                if self.git_panel.scrollbar_hovered != prev_sb {
                    panel_changed = true;
                }
            } else if self.git_panel.hovered_item.is_some()
                || self.git_panel.hovered_toolbar_btn.is_some()
                || self.git_panel.hovered_commit_btn
                || self.git_panel.hovered_generate_btn
                || self.git_panel.hovered_stage_all
                || self.git_panel.hovered_unstage_all
            {
                self.git_panel.hovered_item = None;
                self.git_panel.hovered_toolbar_btn = None;
                self.git_panel.hovered_commit_btn = false;
                self.git_panel.hovered_generate_btn = false;
                self.git_panel.hovered_stage_all = false;
                self.git_panel.hovered_unstage_all = false;
                panel_changed = true;
            }

            if self.overlay.user_menu_open {
                self.overlay.user_menu_hovered = crate::ui::components::tab_bar::user_menu_hovered(
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    renderer.tab_bar_height as f64,
                    renderer.width as f64,
                    renderer.scale_factor,
                    self.license_mgr.is_pro(),
                );
            }

            let had_tooltip = self.overlay.tooltip.is_some();
            self.overlay.tooltip = if self.overlay.user_menu_open {
                None
            } else if self.overlay.sidebar_hovered {
                Some(("Sessions".into(), self.cursor_pos.0, self.cursor_pos.1))
            } else if self.overlay.new_tab_hovered {
                Some(("New Tab".into(), self.cursor_pos.0, self.cursor_pos.1))
            } else if self.overlay.shell_picker_btn_hovered {
                Some(("New Session".into(), self.cursor_pos.0, self.cursor_pos.1))
            } else if self.overlay.avatar_hovered {
                Some(("Awebo".into(), self.cursor_pos.0, self.cursor_pos.1))
            } else if self.overlay.git_panel_hovered {
                Some((
                    "Source Control".into(),
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                ))
            } else if self.overlay.hovered_close.is_some() {
                Some(("Close Tab".into(), self.cursor_pos.0, self.cursor_pos.1))
            } else if let Some((rx, ry, rw, rh)) = self.overlay.ctx_bar_rect {
                let cx = self.cursor_pos.0 as usize;
                let cy = self.cursor_pos.1 as usize;
                if cx >= rx && cx < rx + rw && cy >= ry && cy < ry + rh {
                    let used = self.ai_ctrl.state.last_prompt_tokens
                        + self.ai_ctrl.state.last_generated_tokens;
                    let total = self.ai_ctrl.state.context_size;
                    Some((
                        format!("{} / {} tokens", used, total),
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            if self.overlay.tooltip.is_some() || had_tooltip {
                self.request_redraw();
            }
        }
        panel_changed
    }

    fn shell_picker_anchor(&self) -> (usize, usize) {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return (0, 0),
        };
        let sf = renderer.scale_factor as f32;
        let tab_count = self.tab_mgr.len();
        let w = renderer.width as f64;
        let traffic_left = crate::ui::components::tab_bar::left_padding(self.is_fullscreen) as f64;
        let sidebar_w: f64 = 8.0 + 18.0 + 8.0;
        let left_pad: f64 = traffic_left + sidebar_w;
        let plus_w: f64 = 36.0;
        let btn_gap: f64 = 4.0;
        let picker_w: f64 = 32.0;
        let gear_total: f64 = 28.0 + 6.0 + 12.0;
        let tab_w_logical = if tab_count == 0 {
            220.0
        } else {
            let avail = (w / renderer.scale_factor)
                - plus_w
                - btn_gap
                - picker_w
                - left_pad
                - btn_gap
                - gear_total;
            (avail / tab_count as f64).clamp(120.0, 220.0)
        };
        let tab_w_phys = (tab_w_logical * renderer.scale_factor) as usize;
        let left_pad_phys = (left_pad * sf as f64) as usize;
        let anchor_x = left_pad_phys
            + tab_count * tab_w_phys
            + (plus_w * sf as f64) as usize
            + (btn_gap * sf as f64) as usize;
        let anchor_y = renderer.tab_bar_height as usize;
        (anchor_x, anchor_y)
    }

    /// Query: build the current shell picker state for hit-testing / rendering.
    fn build_shell_picker_state(&self) -> ShellPickerState {
        let mut sandbox_images: Vec<SandboxImageInfo> = crate::sandbox::images::IMAGES
            .iter()
            .map(|img| SandboxImageInfo {
                name: img.display_name.to_string(),
                description: img.description.to_string(),
                category: img.category.to_string(),
            })
            .collect();

        for ci in &self.config.sandbox.custom_images {
            sandbox_images.push(SandboxImageInfo {
                name: ci.display_name.clone(),
                description: ci.oci_ref.clone(),
                category: "Custom".to_string(),
            });
        }

        ShellPickerState {
            local_shells: self
                .available_shells
                .iter()
                .map(|(name, _)| ShellInfo { name: name.clone() })
                .collect(),
            sandbox_images,
            sandbox_available: self.sandbox_mgr.is_available(),
            hovered: self.overlay.shell_picker_hovered,
        }
    }

    /// Query: build sandbox info from the active tab (if it's a sandbox).
    fn build_sandbox_info(&self) -> Option<crate::ui::components::side_panel::SandboxInfo> {
        self.tab_mgr
            .get(self.tab_mgr.active_index())
            .and_then(|t| match &t.kind {
                TabKind::Sandbox { bridge, .. } => {
                    Some(crate::ui::components::side_panel::SandboxInfo {
                        display_name: bridge.display_name.clone(),
                        cpus: bridge.cpus,
                        memory_mib: bridge.memory_mib,
                        is_alive: bridge.is_alive(),
                        is_initializing: bridge.is_initializing(),
                        pull_state: bridge.pull_progress(),
                        volumes: bridge
                            .volumes
                            .iter()
                            .map(|v| (v.guest_path.clone(), v.host_path.clone()))
                            .collect(),
                    })
                }
                _ => None,
            })
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler<TerminalEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        #[cfg(target_os = "macos")]
        {
            event_loop.set_allows_automatic_window_tabbing(true);
        }

        let mut attrs = WindowAttributes::default()
            .with_title("Awebo")
            .with_inner_size(winit::dpi::LogicalSize::new(1000, 650))
            .with_min_inner_size(winit::dpi::LogicalSize::new(1000, 650));

        #[cfg(target_os = "macos")]
        {
            attrs = attrs
                .with_tabbing_identifier("awebo-main")
                .with_fullsize_content_view(true)
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_movable_by_window_background(false);
        }

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );

        #[cfg(target_os = "macos")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(RawWindowHandle::AppKit(handle)) = window.window_handle().map(|h| h.as_raw())
            {
                unsafe {
                    let ns_view: objc2::rc::Retained<objc2_app_kit::NSView> =
                        objc2::rc::Retained::retain(
                            handle.ns_view.as_ptr() as *mut objc2_app_kit::NSView
                        )
                        .unwrap();
                    if let Some(ns_window) = ns_view.window() {
                        ns_window.setMovable(false);
                    }
                }
            }
            reposition_traffic_lights(&window);
        }

        let size = window.inner_size();
        let scale_factor = window.scale_factor();

        let renderer = Renderer::new(window.clone(), size.width, size.height, scale_factor);

        log::info!("Render backend: {}", renderer.backend);

        self.window = Some(window);
        self.renderer = Some(renderer);

        if self.app_menu.is_none() {
            let app_menu = crate::menu::build_menu();

            #[cfg(target_os = "macos")]
            {
                app_menu.menu.init_for_nsapp();
                app_menu.window_submenu.set_as_windows_menu_for_nsapp();
            }

            self.app_menu = Some(app_menu);
        }

        self.create_tab(None);
        self.auto_load_model();

        if self.config.updates.auto_check {
            crate::updater::spawn_update_check(self.proxy.clone());
        }

        self.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if crate::dialog::confirm_quit() {
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(w) = &self.window {
                    self.is_fullscreen = w.fullscreen().is_some();
                }

                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);

                    let sf = renderer.scale_factor as f32;
                    renderer.panel_inset_left = if self.overlay.sidebar_open {
                        self.panel_layout.left_physical_width(sf) as u32
                    } else {
                        0
                    };
                    renderer.panel_inset_right = if self.overlay.git_panel_open {
                        self.panel_layout.right_physical_width(sf) as u32
                    } else {
                        0
                    };

                    for tab in self.tab_mgr.iter() {
                        match &tab.kind {
                            TabKind::Terminal {
                                terminal, is_alt, ..
                            } => {
                                let ws = Self::compute_window_size(renderer, *is_alt);
                                terminal.resize(ws);
                            }
                            TabKind::Sandbox { bridge, .. } => {
                                let ws = Self::compute_window_size(renderer, false);
                                let term_size = crate::terminal::TermSize(ws);
                                let mut t = bridge.term.lock();
                                t.resize(term_size);
                            }
                            _ => {}
                        }
                    }
                }

                self.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                self.update_fps();

                #[cfg(target_os = "macos")]
                if !self.is_fullscreen
                    && let Some(w) = &self.window
                {
                    reposition_traffic_lights(w);
                }

                let cursor_visible = self.cursor_blink_on;

                let debug_info = if self.overlay.debug_panel {
                    if let Some(r) = &self.renderer {
                        Some(format!(
                            "{} | FPS: {:.0} | {:.1}ms | {}x{} | sf={:.1} | tabs={}",
                            r.backend,
                            self.last_fps,
                            self.last_frame_ms,
                            (r.width as f64 / r.scale_factor).round() as u32,
                            (r.height as f64 / r.scale_factor).round() as u32,
                            r.scale_factor,
                            self.tab_mgr.len(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                };

                let palette_state = if self.overlay.palette_open {
                    let cmds = self.filtered_commands();
                    Some(PaletteState {
                        query: self.overlay.palette_query.clone(),
                        commands: cmds.iter().map(|c| c.label().to_string()).collect(),
                        shortcuts: cmds.iter().map(|c| c.shortcut().to_string()).collect(),
                        selected: self.overlay.palette_selected,
                    })
                } else {
                    None
                };

                let model_picker_state = if self.overlay.model_picker_open {
                    let models_dir = ai::model_manager::models_dir();
                    let loaded_name = self.ai_ctrl.state.loaded_model_name.as_deref();
                    let items: Vec<ModelPickerItem> = ai::registry::MODELS
                        .iter()
                        .map(|m| {
                            let path = models_dir.join(m.filename);
                            let status = if loaded_name == Some(m.name) {
                                ModelStatus::Loaded
                            } else if path.exists() {
                                ModelStatus::Downloaded
                            } else {
                                ModelStatus::NotDownloaded
                            };
                            ModelPickerItem {
                                name: m.name.to_string(),
                                quant_label: m.quant_label.to_string(),
                                status,
                            }
                        })
                        .collect();
                    Some(ModelPickerState {
                        items,
                        selected: self.overlay.model_picker_selected,
                    })
                } else {
                    None
                };

                let tab_infos = self.build_tab_infos();
                let hovered_close = self.overlay.hovered_close;

                let shell_picker_state = if self.overlay.shell_picker_open {
                    Some(self.build_shell_picker_state())
                } else {
                    None
                };

                let input_type = self.settings_state.input_type;
                let prompt_info = if input_type == InputType::Smart {
                    self.active_terminal().and_then(|t| {
                        if t.is_app_controlled() {
                            None
                        } else {
                            Some(t.prompt_info())
                        }
                    })
                } else {
                    None
                };

                let ai_thinking = self.ai_ctrl.state.inference_rx.is_some()
                    || self.ai_ctrl.state.hint_rx.is_some()
                    || self.ai_ctrl.state.thinking_since.is_some();

                let active_route = self
                    .tab_mgr
                    .get(self.tab_mgr.active_index())
                    .map(|t| t.route())
                    .unwrap_or(router::Route::Terminal);
                let settings_view = if active_route == router::Route::Settings {
                    Some(&self.settings_state)
                } else {
                    None
                };

                let models_view_data = if active_route == router::Route::Models {
                    Some((
                        &self.models_view,
                        self.ai_ctrl.state.loaded_model_name.as_deref(),
                        self.config.ai.auto_load,
                        self.settings_state.models_path.as_str(),
                    ))
                } else {
                    None
                };

                if active_route == router::Route::Editor
                    && let Some(state) = self
                        .tab_mgr
                        .active_tab_mut()
                        .and_then(|t| t.editor_state_mut())
                {
                    state.refresh_highlights(&self.syntax);
                }

                let editor_state = if active_route == router::Route::Editor {
                    self.tab_mgr.active_tab().and_then(|t| t.editor_state())
                } else {
                    None
                };

                let active_tab = &self.tab_mgr.get(self.tab_mgr.active_index());
                let active_terminal = active_tab.and_then(|t| t.terminal());
                let term_handle = active_tab.and_then(|t| t.term_handle());
                let is_app_controlled = active_tab.map(|t| t.is_app_controlled()).unwrap_or(false);

                if self.overlay.sidebar_open
                    && let Some(term) = active_terminal
                    && let Some(shell_cwd) = term.cwd()
                {
                    let cwd_path = std::path::PathBuf::from(&shell_cwd);
                    self.file_tree.load(&cwd_path);
                }

                let empty_bl = blocks::BlockList::new();
                let block_list_ref = active_tab.and_then(|t| t.block_list()).unwrap_or(&empty_bl);

                let sessions_refs: Vec<&crate::session::Session> =
                    self.session_mgr.sessions().collect();

                let active_sid = active_tab.and_then(|t| t.session_id);
                self.side_panel.active_session_visual_idx =
                    active_sid.and_then(|sid| sessions_refs.iter().position(|s| s.id == sid));

                let sandbox_info = self.build_sandbox_info();

                let update_visible = self.overlay.update_available.is_some()
                    || self.overlay.update_downloading
                    || self.overlay.update_downloaded.is_some();

                let update_new_version: Option<String> = self
                    .overlay
                    .update_available
                    .as_ref()
                    .map(|info| info.version.to_string());
                let update_downloading = self.overlay.update_downloading;
                let update_dropdown_open = self.overlay.update_dropdown_open;
                let update_dd_hovered = self.overlay.update_dropdown_hovered;
                let update_badge_hovered_val = self.overlay.update_badge_hovered;

                if let Some(renderer) = &mut self.renderer {
                    let frame_start = Instant::now();

                    let update_badge_w: Option<f32> = if update_visible {
                        Some(crate::ui::components::tab_bar::update_badge_logical_width(
                            &mut renderer.font_system,
                            renderer.scale_factor as f32,
                        ))
                    } else {
                        None
                    };
                    self.overlay.update_badge_w = update_badge_w;

                    let update_dropdown_data: Option<(&str, bool, Option<usize>)> =
                        if update_dropdown_open {
                            update_new_version
                                .as_deref()
                                .map(|v| (v, update_downloading, update_dd_hovered))
                        } else {
                            None
                        };

                    let confirm_close_data: Option<(String, Option<usize>)> =
                        self.overlay.confirm_close_tab.and_then(|idx| {
                            self.tab_mgr
                                .get(idx)
                                .and_then(|t| t.editor_state())
                                .map(|es| {
                                    let raw_name = es
                                        .path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "untitled".to_string());
                                    (raw_name, self.overlay.confirm_close_hovered)
                                })
                        });
                    let confirm_close = confirm_close_data
                        .as_ref()
                        .map(|(name, h)| (name.as_str(), *h));

                    let hit_rects = renderer.render(
                        term_handle.as_ref(),
                        is_app_controlled,
                        sandbox_info.is_some(),
                        &tab_infos,
                        hovered_close,
                        &self.drag,
                        debug_info.as_deref(),
                        palette_state.as_ref(),
                        shell_picker_state.as_ref(),
                        self.overlay.new_tab_hovered,
                        self.overlay.shell_picker_btn_hovered,
                        self.overlay.sidebar_hovered,
                        self.overlay.sidebar_open,
                        self.overlay.git_panel_hovered,
                        self.overlay.git_panel_open,
                        self.overlay.user_menu_open,
                        self.overlay.user_menu_hovered,
                        self.license_mgr.is_pro(),
                        update_badge_w,
                        update_badge_hovered_val,
                        update_dropdown_data,
                        input_type,
                        prompt_info.as_ref(),
                        &self.smart_input,
                        block_list_ref,
                        self.overlay
                            .tooltip
                            .as_ref()
                            .map(|(s, x, y)| (s.as_str(), *x, *y)),
                        self.widget_debug.open,
                        cursor_visible,
                        self.ai_ctrl.state.loaded_model_name.as_deref(),
                        ai_thinking,
                        model_picker_state.as_ref(),
                        self.block_selection.as_ref(),
                        self.hovered_link.as_ref(),
                        self.scrollbar_hovered || self.scrollbar_dragging,
                        if self.editor_scrollbar_dragging
                            != crate::ui::components::editor_renderer::ScrollbarHit::None
                        {
                            self.editor_scrollbar_dragging
                        } else {
                            self.editor_scrollbar
                        },
                        settings_view,
                        models_view_data,
                        &sessions_refs,
                        &self.side_panel,
                        &self.file_tree,
                        &self.panel_layout,
                        &self.hint_banner,
                        editor_state,
                        self.is_fullscreen,
                        confirm_close,
                        self.context_menu.as_ref(),
                        sandbox_info.as_ref(),
                        &self.toast_mgr,
                        &self.git_panel,
                        if self.overlay.usage_panel_open {
                            Some((&self.usage_tracker, self.overlay.pro_panel_hovered))
                        } else {
                            None
                        },
                        if self.overlay.pro_panel_open {
                            Some((
                                &self.license_mgr,
                                &self.overlay.pro_license_input,
                                self.overlay.pro_license_cursor,
                                self.overlay.pro_license_focused,
                                self.overlay.pro_panel_hovered,
                            ))
                        } else {
                            None
                        },
                        if self.usage_limit_banner.is_visible() {
                            Some((&self.usage_limit_banner, &self.usage_tracker))
                        } else {
                            None
                        },
                    );
                    if let Some(rects) = hit_rects {
                        self.overlay.ctx_bar_rect = rects.ctx_bar;
                        self.overlay.stop_button_rect = rects.stop_button;
                    }
                    let frame_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
                    self.last_frame_ms = frame_ms;
                    if frame_ms > 16.0 {
                        log::debug!("Frame budget exceeded: {frame_ms:.1}ms (target: 16ms)");
                    }
                }

                let any_thinking = self
                    .active_block_list()
                    .is_some_and(|bl| bl.blocks.last().is_some_and(|b| b.thinking));
                let about_visible = active_route == router::Route::Settings
                    && self.settings_state.active
                        == crate::ui::components::overlay::SettingsCategory::About;
                if any_thinking || ai_thinking || about_visible {
                    self.request_redraw();
                }
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }

            WindowEvent::Occluded(_) | WindowEvent::Focused(_) => {
                if let Some(w) = &self.window {
                    self.is_fullscreen = w.fullscreen().is_some();
                }
                self.request_redraw();
            }

            _ => {}
        }

        self.handle_mouse_event(&event, event_loop);
        self.handle_keyboard_input(&event, event_loop);
        if self.is_sandbox_active() {
            self.handle_sandbox_keyboard(&event);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TerminalEvent) {
        match event {
            TerminalEvent::Wakeup => {
                if self.sync_app_state()
                    && let Some(r) = self.renderer.as_mut()
                {
                    r.invalidate_grid_cache();
                }

                let model_just_loaded = self.ai_ctrl.state.poll_model_events();

                if model_just_loaded {
                    if let Some(task) = self.pending_agent_task.take() {
                        self.start_agent(task);
                    }
                    if let Some(query) = self.pending_ai_query.take() {
                        self.start_ai_query(&query);
                    }
                    if self.git_panel.pending_generate_commit_msg {
                        self.git_panel.pending_generate_commit_msg = false;
                        self.dispatch(
                            crate::app::actions::AppAction::GitGenerateCommitMessage,
                            event_loop,
                        );
                    }
                }

                self.poll_model_downloads();

                let ai_streaming = self.ai_ctrl.state.inference_rx.is_some();
                let mut should_hint = false;
                if self.settings_state.input_type == InputType::Smart
                    && !ai_streaming
                    && let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                    && let TabKind::Terminal {
                        terminal,
                        block_list,
                        ..
                    } = &mut tab.kind
                    && !terminal.is_app_controlled()
                {
                    block_list.capture_output(terminal);
                    if terminal.cursor_on_prompt() {
                        let was_running = block_list.last_is_running();
                        block_list.finish_block_if_confirmed_prompt();
                        if was_running && !block_list.last_is_running() {
                            should_hint = true;
                            if let Some(started) = self.smart_input.command_started.take() {
                                self.smart_input.last_command_duration = Some(started.elapsed());
                            }
                            self.smart_input.pending_command = None;
                        }
                    }
                }
                if should_hint {
                    self.request_ai_hint_if_eligible();
                }

                if self.ai_ctrl.state.inference_rx.is_some() {
                    self.poll_ai_tokens();
                }

                if self.ai_ctrl.state.hint_rx.is_some()
                    && let Some(cmd) = self.ai_ctrl.state.poll_hint()
                    && self.smart_input.text.is_empty()
                {
                    self.smart_input.ai_suggestion = Some(cmd);
                }

                self.pending_redraw = true;
            }
            TerminalEvent::Title(_title) => {
                self.pending_redraw = true;
            }
            TerminalEvent::Exit => {
                log::info!("Terminal process exited");
                let idx = self.tab_mgr.active_index();
                self.close_tab(idx, event_loop);
                self.request_redraw();
            }
            TerminalEvent::MenuAction(menu_event) => {
                use actions::AppAction;
                let action = if let Some(app_menu) = &self.app_menu {
                    if menu_event.id == app_menu.new_tab_id {
                        Some(AppAction::CreateTab { shell_path: None })
                    } else if menu_event.id == app_menu.close_tab_id {
                        Some(AppAction::CloseTab {
                            index: self.tab_mgr.active_index(),
                        })
                    } else if menu_event.id == app_menu.toggle_debug_id {
                        Some(AppAction::ToggleDebugPanel)
                    } else if menu_event.id == app_menu.toggle_settings_id {
                        Some(AppAction::OpenSettings)
                    } else if menu_event.id == app_menu.copy_id {
                        Some(AppAction::Copy)
                    } else if menu_event.id == app_menu.paste_id {
                        Some(AppAction::Paste)
                    } else if menu_event.id == app_menu.cut_id {
                        Some(AppAction::Cut)
                    } else if menu_event.id == app_menu.select_all_id {
                        Some(AppAction::SelectAll)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(action) = action {
                    self.dispatch(action, event_loop);
                }
            }
            TerminalEvent::AiError(msg) => {
                log::error!("AI error shown to user: {msg}");
                if let Some(bl) = self.active_block_list_mut() {
                    if let Some(block) = bl.blocks.last_mut() {
                        if block.thinking {
                            block.thinking = false;
                        }
                        block.is_error = true;
                    }
                    bl.append_output_text(&format!("[AI Error] {msg}"));
                    bl.finish_last();
                }
                self.record_last_block();
                self.ai_ctrl.state.loaded_model_name = None;
                self.pending_redraw = true;
            }
            TerminalEvent::CommandExitCode(code) => {
                log::info!("Shell reported exit code: {code}");
                let mut should_hint = false;
                if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                    && let TabKind::Terminal {
                        terminal,
                        block_list,
                        ..
                    } = &mut tab.kind
                {
                    block_list.capture_output(terminal);
                    let was_running = block_list.last_is_running();
                    if let Some(block) = block_list.blocks.last_mut() {
                        block.exit_code = Some(code);
                        block.is_error = code != 0;
                    }
                    block_list.finish_block_if_confirmed();
                    should_hint = was_running && !block_list.last_is_running();
                    if was_running && !block_list.last_is_running() {
                        if let Some(started) = self.smart_input.command_started.take() {
                            self.smart_input.last_command_duration = Some(started.elapsed());
                        }
                        self.smart_input.pending_command = None;
                    }
                }
                self.record_last_block();
                if should_hint {
                    self.request_ai_hint_if_eligible();
                }
                self.pending_redraw = true;
            }
            TerminalEvent::ModelDeleted(idx) => {
                log::info!("Model deletion complete: index {idx}");
                self.settings_state.deleting_model = None;
                self.pending_redraw = true;
            }
            TerminalEvent::ToolComplete { request, result } => {
                self.on_tool_complete(request, result);
                self.pending_redraw = true;
            }
            TerminalEvent::SandboxExit { name, code } => {
                log::info!("[sandbox] '{}' exited with code {}", name, code);
                let idx = self.tab_mgr.active_index();
                if let Some(tab) = self.tab_mgr.get(idx)
                    && matches!(&tab.kind, TabKind::Sandbox { .. })
                {
                    self.close_tab(idx, event_loop);
                }
                self.pending_redraw = true;
            }
            TerminalEvent::SandboxError(msg) => {
                log::error!("[sandbox] Error: {}", msg);
                self.toast_mgr.push(
                    format!("Sandbox: {}", msg),
                    crate::ui::components::toast::ToastLevel::Error,
                );
                self.pending_redraw = true;
            }
            TerminalEvent::Toast(msg) => {
                self.toast_mgr
                    .push(msg, crate::ui::components::toast::ToastLevel::Info);
                self.pending_redraw = true;
            }
            TerminalEvent::ToastLevel(msg, level) => {
                self.toast_mgr.push(msg, level);
                self.pending_redraw = true;
            }
            TerminalEvent::UpdateAvailable(info) => {
                let version_label = format!("v{}", info.version);
                self.overlay.update_available = Some(info);
                self.toast_mgr.push(
                    format!("Update available: {version_label}"),
                    crate::ui::components::toast::ToastLevel::Info,
                );
                self.pending_redraw = true;
            }
            TerminalEvent::UpdateDownloaded(path) => {
                self.overlay.update_downloading = false;
                match crate::updater::stage_update(&path) {
                    Ok(()) => {
                        crate::updater::spawn_relaunch();
                        event_loop.exit();
                    }
                    Err(e) => {
                        self.overlay.update_downloaded = Some(path);
                        self.toast_mgr.push(
                            format!("Update staging failed: {e}"),
                            crate::ui::components::toast::ToastLevel::Error,
                        );
                    }
                }
                self.pending_redraw = true;
            }
            TerminalEvent::UpdateFailed(msg) => {
                self.overlay.update_downloading = false;
                self.toast_mgr.push(
                    format!("Update error: {msg}"),
                    crate::ui::components::toast::ToastLevel::Error,
                );
                self.pending_redraw = true;
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now.duration_since(self.cursor_blink_at).as_millis() >= 500 {
            self.cursor_blink_on = !self.cursor_blink_on;
            self.cursor_blink_at = now;
            self.pending_redraw = true;
        }

        if self.toast_mgr.tick() {
            self.pending_redraw = true;
        }
        if self.toast_mgr.has_active() {
            self.pending_redraw = true;
        }

        if self.overlay.git_panel_open && now >= self.git_poll_at {
            self.git_poll_at = now + std::time::Duration::from_secs(2);
            let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
            self.git_panel.refresh(&cwd);
            self.pending_redraw = true;
        }

        if self.overlay.sidebar_open && now >= self.file_tree_poll_at {
            self.file_tree_poll_at = now + std::time::Duration::from_secs(2);
            self.refresh_file_tree();
            self.pending_redraw = true;
        }

        if self.pending_redraw {
            self.pending_redraw = false;
            self.request_redraw();
        }

        let mut next_wake = self.cursor_blink_at + std::time::Duration::from_millis(500);
        if self.overlay.git_panel_open && self.git_poll_at < next_wake {
            next_wake = self.git_poll_at;
        }
        if self.overlay.sidebar_open && self.file_tree_poll_at < next_wake {
            next_wake = self.file_tree_poll_at;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(next_wake));
    }
}

/// Application entry point — creates the event loop and runs the app.
pub fn run() {
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("awebo");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("awebo.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("Failed to open log file");
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let mut builder = EventLoop::<TerminalEvent>::with_user_event();

    #[cfg(target_os = "macos")]
    builder.with_default_menu(false);

    let event_loop = builder.build().expect("Failed to create event loop");

    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = event_loop.create_proxy();

    crate::menu::setup_event_handler(proxy.clone(), TerminalEvent::MenuAction);

    let mut app = App::new(proxy);
    event_loop.run_app(&mut app).expect("Event loop error");
}

/// Reposition the macOS traffic light buttons (close/minimize/zoom) to be
/// vertically centered in our custom tab bar instead of the default titlebar.
/// We move the buttons' shared superview so that both visuals AND hover
/// tracking areas stay aligned.
#[cfg(target_os = "macos")]
fn reposition_traffic_lights(window: &Window) {
    use objc2_app_kit::NSWindowButton;
    use objc2_foundation::NSPoint;
    use std::sync::OnceLock;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    static ORIGINAL_CONTAINER_ORIGIN: OnceLock<NSPoint> = OnceLock::new();

    let ns_window = match window.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::AppKit(handle)) => unsafe {
            let ns_view: objc2::rc::Retained<objc2_app_kit::NSView> =
                objc2::rc::Retained::retain(handle.ns_view.as_ptr() as *mut objc2_app_kit::NSView)
                    .unwrap();
            ns_view.window()
        },
        _ => None,
    };
    let Some(ns_window) = ns_window else { return };

    let Some(close_btn) = ns_window.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    let Some(container) = (unsafe { close_btn.superview() }) else {
        return;
    };

    let original = *ORIGINAL_CONTAINER_ORIGIN.get_or_init(|| container.frame().origin);

    let bar_h = crate::ui::components::tab_bar::TAB_BAR_LOGICAL_HEIGHT as f64;
    let titlebar_h = 30.0;
    let y_offset = (bar_h - titlebar_h) / 2.0;
    let extra_left = 4.0;

    let target = NSPoint::new(original.x + extra_left, original.y - y_offset);
    container.setFrameOrigin(target);
}
