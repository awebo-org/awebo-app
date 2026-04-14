//! Settings view — renders as a dedicated tab via the Tab+Router system.

use crate::ui::components::overlay::SettingsCategory;

use super::super::router;

impl super::super::App {
    /// Open the settings view as a new tab (or focus existing one).
    pub(crate) fn open_settings_view(&mut self) {
        if let Some(idx) = self.tab_mgr.find_settings() {
            self.tab_mgr.switch_to(idx);
            self.request_redraw();
            return;
        }

        self.tab_mgr.push(super::super::Tab::new_settings());

        self.settings_state.font_picker_open = false;
        self.settings_state.font_picker_hovered = None;

        if self.settings_state.font_options.is_empty()
            && let Some(renderer) = &mut self.renderer
        {
            renderer.ensure_system_fonts_loaded();
            self.settings_state.font_options =
                crate::ui::components::overlay::detect_monospace_fonts(&renderer.font_system);
        }

        if let Some(r) = self.renderer.as_mut() {
            r.invalidate_grid_cache();
        }
        self.request_redraw();
    }

    /// Close the settings tab (if one is active or exists).
    pub(crate) fn close_settings_view(&mut self) {
        if let Some(idx) = self.tab_mgr.find_settings() {
            self.tab_mgr.remove(idx);
            self.settings_state.font_picker_open = false;
            self.settings_state.font_picker_hovered = None;
            if let Some(r) = self.renderer.as_mut() {
                r.invalidate_grid_cache();
            }
            self.request_redraw();
        }
    }

    pub(crate) fn is_settings_active(&self) -> bool {
        self.tab_mgr
            .get(self.tab_mgr.active_index())
            .map(|t| t.route() == router::Route::Settings)
            .unwrap_or(false)
    }

    /// Side panel pixel width when open (0 when closed).
    pub(crate) fn side_panel_x_offset(&self) -> usize {
        if !self.overlay.sidebar_open {
            return 0;
        }
        let sf = self
            .renderer
            .as_ref()
            .map(|r| r.scale_factor as f32)
            .unwrap_or(1.0);
        self.panel_layout.left_physical_width(sf)
    }

    /// Compute settings panel content area coordinates.
    fn settings_content_coords(&self) -> Option<(usize, usize, usize, usize, f32)> {
        let renderer = self.renderer.as_ref()?;
        let sf = renderer.scale_factor as f32;
        let bw = renderer.width as usize;
        let bh = renderer.height as usize;
        let (px, py, pw, _ph) = crate::ui::components::overlay::settings_panel_rect(bw, bh, sf);
        let sidebar_w = (180.0 * sf) as usize;
        let border_w = (1.0_f32 * sf).max(1.0) as usize;
        let content_x = px + sidebar_w + border_w;
        let content_w = pw.saturating_sub(sidebar_w + border_w);
        let body_y = py + (16.0 * sf) as usize;
        Some((content_x, content_w, body_y, bw.max(bh), sf))
    }

    /// Handle settings hover on CursorMoved (main window).
    pub(crate) fn update_settings_hover(&mut self) {
        if !self.is_settings_active() {
            return;
        }
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };
        let sf = renderer.scale_factor as f32;
        let bw = renderer.width as usize;
        let bh = renderer.height as usize;

        let prev = self.settings_state.hovered;
        self.settings_state.hovered = crate::ui::components::overlay::settings_sidebar_hit_test(
            self.cursor_pos.0,
            self.cursor_pos.1,
            bw,
            bh,
            sf,
        );

        let (content_x, content_w, body_y, _, _) = match self.settings_content_coords() {
            Some(c) => c,
            None => return,
        };

        let prev_btn = self.settings_state.hovered_btn;
        if self.settings_state.active == crate::ui::components::overlay::SettingsCategory::AiModels
        {
            self.settings_state.hovered_btn =
                crate::ui::components::overlay::settings::ai_models::settings_ai_models_hit_test(
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    body_y,
                    content_x,
                    content_w,
                    sf,
                );
        } else {
            self.settings_state.hovered_btn = None;
        }

        let prev_sandbox_hit = self.settings_state.sandbox.hovered_hit.clone();
        if self.settings_state.active == crate::ui::components::overlay::SettingsCategory::Sandbox {
            self.settings_state.sandbox.hovered_hit =
                crate::ui::components::overlay::settings::sandbox_settings::sandbox_settings_hit_test(
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    body_y,
                    content_x,
                    content_w,
                    sf,
                    self.settings_state.sandbox.scroll_offset,
                );
        } else {
            self.settings_state.sandbox.hovered_hit = None;
        }

        let prev_about = self.settings_state.about_hovered;
        if self.settings_state.active == crate::ui::components::overlay::SettingsCategory::About {
            self.settings_state.about_hovered =
                crate::ui::components::overlay::settings::about::about_hit_test(
                    self.cursor_pos.0 as usize,
                    self.cursor_pos.1 as usize,
                    content_x,
                    body_y,
                    content_w,
                    sf,
                    self.license_mgr.is_pro(),
                );
        } else {
            self.settings_state.about_hovered = None;
        }

        if self.settings_state.font_picker_open {
            let prev_fp = self.settings_state.font_picker_hovered;
            let (_, py, _, _) = crate::ui::components::overlay::settings_panel_rect(bw, bh, sf);
            self.settings_state.font_picker_hovered =
                crate::ui::components::overlay::font_picker_hit_test(
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    py,
                    content_x,
                    content_w,
                    sf,
                    self.settings_state.font_options.len(),
                );
            if self.settings_state.font_picker_hovered != prev_fp {
                self.request_redraw();
            }
        }

        if self.settings_state.hovered != prev
            || self.settings_state.hovered_btn != prev_btn
            || self.settings_state.sandbox.hovered_hit != prev_sandbox_hit
            || self.settings_state.about_hovered != prev_about
        {
            self.request_redraw();
        }
    }

    /// Handle a left-click inside the settings view.
    pub(crate) fn handle_settings_click(&mut self) {
        if !self.is_settings_active() {
            return;
        }
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };
        let sf = renderer.scale_factor as f32;
        let bw = renderer.width as usize;
        let bh = renderer.height as usize;

        // Click outside panel closes settings
        if !crate::ui::components::overlay::settings_panel_contains(
            self.cursor_pos.0,
            self.cursor_pos.1,
            bw,
            bh,
            sf,
        ) {
            self.close_settings_view();
            return;
        }

        let (content_x, content_w, body_y, _, _) = match self.settings_content_coords() {
            Some(c) => c,
            None => return,
        };

        if self.settings_state.font_picker_open {
            let (_, py, _, _) = crate::ui::components::overlay::settings_panel_rect(bw, bh, sf);
            let font_count = self.settings_state.font_options.len();
            if let Some(idx) = crate::ui::components::overlay::font_picker_hit_test(
                self.cursor_pos.0,
                self.cursor_pos.1,
                py,
                content_x,
                content_w,
                sf,
                font_count,
            ) {
                if idx < font_count {
                    self.settings_state.font_family = self.settings_state.font_options[idx].clone();
                    self.settings_state.font_picker_open = false;
                    self.settings_state.font_picker_hovered = None;
                    self.apply_font_settings();
                    self.save_config();
                }
            } else {
                self.settings_state.font_picker_open = false;
                self.settings_state.font_picker_hovered = None;
            }
        } else if let Some(idx) = crate::ui::components::overlay::settings_sidebar_hit_test(
            self.cursor_pos.0,
            self.cursor_pos.1,
            bw,
            bh,
            sf,
        ) {
            if let Some(&cat) = SettingsCategory::all().get(idx) {
                self.settings_state.active = cat;
                self.settings_state.font_picker_open = false;
                self.settings_state.font_picker_hovered = None;
            }
        } else if self.settings_state.active == SettingsCategory::AiModels {
            if let Some(hit) = crate::ui::components::overlay::settings_ai_models_hit_test(
                self.cursor_pos.0,
                self.cursor_pos.1,
                body_y,
                content_x,
                content_w,
                sf,
            ) {
                self.handle_ai_models_hit(hit);
            }
        } else if self.settings_state.active == SettingsCategory::Sandbox {
            let scroll = self.settings_state.sandbox.scroll_offset;
            let (_, py, _, ph) = crate::ui::components::overlay::settings_panel_rect(bw, bh, sf);
            let viewport_h = (py + ph).saturating_sub(body_y) as f32;

            if crate::ui::components::overlay::settings::sandbox_settings::scrollbar_thumb_hit_test(
                self.cursor_pos.0,
                self.cursor_pos.1,
                body_y,
                content_x,
                content_w,
                viewport_h,
                sf,
                scroll,
            ) {
                self.settings_state.sandbox.dragging_scrollbar = true;
                self.settings_state.sandbox.scrollbar_drag_anchor_y = self.cursor_pos.1 as f32;
                self.settings_state.sandbox.scrollbar_drag_anchor_offset = scroll;
                self.settings_state.sandbox.add_image_focused = false;
            } else if let Some((slider, frac)) =
                crate::ui::components::overlay::settings::sandbox_settings::sandbox_slider_hit_test(
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    body_y,
                    content_x,
                    content_w,
                    sf,
                    scroll,
                )
            {
                self.settings_state.sandbox.dragging_slider = Some(slider);
                self.settings_state.sandbox.add_image_focused = false;
                self.apply_sandbox_slider(slider, frac);
            } else if let Some(hit) =
                crate::ui::components::overlay::settings::sandbox_settings::sandbox_settings_hit_test(
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    body_y,
                    content_x,
                    content_w,
                    sf,
                    scroll,
                )
            {
                self.handle_sandbox_settings_hit(hit);
            } else {
                self.settings_state.sandbox.add_image_focused = false;
            }
        } else if self.settings_state.active == SettingsCategory::About {
            use crate::ui::components::overlay::settings::about::AboutHit;
            let hit = crate::ui::components::overlay::settings::about::about_hit_test(
                self.cursor_pos.0 as usize,
                self.cursor_pos.1 as usize,
                content_x,
                body_y,
                content_w,
                sf,
                self.license_mgr.is_pro(),
            );
            match hit {
                Some(AboutHit::ResetHints) => {
                    self.config.general.hint_banner_dismissed = false;
                    self.save_config();
                    self.hint_banner.welcome_dismissed = false;
                    self.hint_banner.kind =
                        Some(crate::ui::components::hint_banner::HintBannerKind::Welcome);
                    self.toast_mgr.push(
                        "Hints reset — banner will show on next session".to_string(),
                        crate::ui::components::toast::ToastLevel::Info,
                    );
                }
                Some(AboutHit::UpgradeToPro) => {
                    self.overlay.pro_panel_open = true;
                    self.overlay.usage_panel_open = false;
                    self.overlay.pro_panel_hovered = None;
                }
                Some(AboutHit::DeactivateLicense) => match self.license_mgr.deactivate() {
                    Ok(()) => {
                        self.usage_tracker.set_pro(false);
                        self.toast_mgr.push(
                            "License deactivated".to_string(),
                            crate::ui::components::toast::ToastLevel::Info,
                        );
                        self.overlay.pro_panel_open = false;
                    }
                    Err(e) => {
                        self.toast_mgr.push(
                            format!("Deactivation failed: {e}"),
                            crate::ui::components::toast::ToastLevel::Error,
                        );
                    }
                },
                Some(AboutHit::ResetSettings) => {
                    self.config = crate::config::AppConfig::default();
                    self.settings_state.input_type =
                        crate::ui::components::overlay::InputType::Smart;
                    self.settings_state.font_family = self.config.appearance.font_family.clone();
                    self.settings_state.font_size_px = self.config.appearance.font_size;
                    self.settings_state.line_height_px = self.config.appearance.line_height;
                    self.settings_state.models_path = self.config.ai.models_path.clone();
                    self.settings_state.web_search_enabled = self.config.ai.web_search;
                    self.settings_state.sandbox.cpus = self.config.sandbox.default_cpus;
                    self.settings_state.sandbox.memory_mib = self.config.sandbox.default_memory_mib;
                    self.config.save();
                    self.apply_font_settings();
                    self.toast_mgr.push(
                        "Settings reset to defaults".to_string(),
                        crate::ui::components::toast::ToastLevel::Info,
                    );
                }
                None => {}
            }
        }

        self.request_redraw();
    }

    /// Handle mouse release for sandbox slider / scrollbar drag.
    pub(crate) fn handle_settings_mouse_release(&mut self) {
        let mut changed = false;
        if self.settings_state.sandbox.dragging_slider.is_some() {
            self.settings_state.sandbox.dragging_slider = None;
            self.save_sandbox_config();
            changed = true;
        }
        if self.settings_state.sandbox.dragging_scrollbar {
            self.settings_state.sandbox.dragging_scrollbar = false;
            changed = true;
        }
        if changed {
            self.request_redraw();
        }
    }

    /// Handle mouse move for sandbox slider / scrollbar drag.
    pub(crate) fn handle_settings_mouse_move(&mut self) {
        if self.settings_state.sandbox.dragging_scrollbar {
            let (_, _, body_y, _, sf) = match self.settings_content_coords() {
                Some(c) => c,
                None => return,
            };
            let renderer = self.renderer.as_ref().unwrap();
            let bw = renderer.width as usize;
            let bh = renderer.height as usize;
            let (_, py, _, ph) = crate::ui::components::overlay::settings_panel_rect(bw, bh, sf);
            let viewport_h = (py + ph).saturating_sub(body_y) as f32;
            let total_content =
                crate::ui::components::overlay::settings::sandbox_settings::sandbox_settings_content_height(sf);
            if total_content <= viewport_h {
                return;
            }
            let max_scroll = total_content - viewport_h;

            let thumb_ratio = viewport_h / total_content;
            let thumb_h = ((viewport_h * thumb_ratio) as usize).max((12.0 * sf) as usize);
            let track_space = viewport_h - thumb_h as f32;
            if track_space <= 0.0 {
                return;
            }

            let mouse_dy =
                self.cursor_pos.1 as f32 - self.settings_state.sandbox.scrollbar_drag_anchor_y;
            let scroll_delta = mouse_dy * (max_scroll / track_space);
            self.settings_state.sandbox.scroll_offset =
                (self.settings_state.sandbox.scrollbar_drag_anchor_offset + scroll_delta)
                    .clamp(0.0, max_scroll);
            self.request_redraw();
            return;
        }

        let dragging = match self.settings_state.sandbox.dragging_slider {
            Some(s) => s,
            None => return,
        };
        let (content_x, content_w, _, _, _) = match self.settings_content_coords() {
            Some(c) => c,
            None => return,
        };

        let sf = self.renderer.as_ref().unwrap().scale_factor as f32;
        let pad = (24.0 * sf) as usize;
        let track_x = content_x + pad;
        let track_w = content_w.saturating_sub(pad * 2);
        if track_w == 0 {
            return;
        }
        let frac = ((self.cursor_pos.0 as f32 - track_x as f32) / track_w as f32).clamp(0.0, 1.0);
        self.apply_sandbox_slider(dragging, frac);
        self.request_redraw();
    }

    /// Apply a slider fraction to the sandbox settings state.
    fn apply_sandbox_slider(
        &mut self,
        slider: crate::ui::components::overlay::settings::SandboxSlider,
        frac: f32,
    ) {
        use crate::ui::components::overlay::settings::{
            SandboxSlider, sandbox_settings::fraction_to_value,
        };
        let sb = &mut self.settings_state.sandbox;
        match slider {
            SandboxSlider::Cpu => {
                sb.cpus = fraction_to_value(frac, 1, sb.system_cpus);
            }
            SandboxSlider::Memory => {
                sb.memory_mib = fraction_to_value(frac, 128, sb.system_memory_mib);
            }
        }
    }

    /// Persist sandbox settings to config file.
    fn save_sandbox_config(&mut self) {
        let sb = &self.settings_state.sandbox;
        self.config.sandbox.default_cpus = sb.cpus;
        self.config.sandbox.default_memory_mib = sb.memory_mib;
        self.config.save();
    }

    /// Handle a click on an interactive element in sandbox settings.
    pub(crate) fn handle_sandbox_settings_hit(
        &mut self,
        hit: crate::ui::components::overlay::settings::SandboxSettingsHit,
    ) {
        use crate::ui::components::overlay::settings::SandboxSettingsHit;
        match hit {
            SandboxSettingsHit::DeleteTrustedImage(idx) => {
                let images = crate::sandbox::images::IMAGES;
                if let Some(img) = images.get(idx) {
                    let oci = img.oci_ref.to_string();
                    let display = crate::sandbox::manager::sanitize_oci_ref(&oci);
                    self.toast_mgr.push(
                        format!("Removing trusted image: {}", display),
                        crate::ui::components::toast::ToastLevel::Info,
                    );
                    self.sandbox_mgr.remove_image(oci, self.proxy.clone());
                }
            }
            SandboxSettingsHit::UpdateTrustedImage(idx) => {
                let images = crate::sandbox::images::IMAGES;
                if let Some(img) = images.get(idx) {
                    let oci = img.oci_ref.to_string();
                    let display = crate::sandbox::manager::sanitize_oci_ref(&oci);
                    self.toast_mgr.push(
                        format!("Pulling latest: {}", display),
                        crate::ui::components::toast::ToastLevel::Info,
                    );
                    self.sandbox_mgr.pull_image(oci, self.proxy.clone());
                }
            }
            SandboxSettingsHit::DeleteCustomImage(idx) => {
                if idx < self.config.sandbox.custom_images.len() {
                    let oci = self.config.sandbox.custom_images[idx].oci_ref.clone();
                    self.config.sandbox.custom_images.remove(idx);
                    self.config.save();
                    self.sandbox_mgr.remove_image(oci, self.proxy.clone());
                }
            }
            SandboxSettingsHit::DeleteVolume(idx) => {
                if idx < self.config.sandbox.volumes.len() {
                    self.config.sandbox.volumes.remove(idx);
                    self.config.save();
                }
            }
            SandboxSettingsHit::AddImage => {
                let input = self
                    .settings_state
                    .sandbox
                    .add_image_input
                    .trim()
                    .to_string();
                if !input.is_empty() {
                    let display = input.rsplit('/').next().unwrap_or(&input).to_string();
                    self.config
                        .sandbox
                        .custom_images
                        .push(crate::config::CustomImageConfig {
                            oci_ref: input.clone(),
                            display_name: display,
                            tag: "latest".into(),
                            default_shell: "/bin/sh".into(),
                            default_workdir: "/root".into(),
                            last_pulled: String::new(),
                        });
                    self.config.save();
                    self.settings_state.sandbox.add_image_input.clear();
                    self.settings_state.sandbox.add_image_focused = false;
                    self.sandbox_mgr.pull_image(input, self.proxy.clone());
                }
            }
            SandboxSettingsHit::AddImageInput => {
                self.settings_state.sandbox.add_image_focused = true;
            }
        }
    }
}
