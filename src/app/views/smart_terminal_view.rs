use std::borrow::Cow;
use std::time::Instant;

use alacritty_terminal::grid::Dimensions;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

use crate::ui::components::overlay::InputType;
use crate::ui::components::tab_bar::TabBarHit;

impl super::super::App {
    pub(crate) fn handle_keyboard_input(
        &mut self,
        event: &WindowEvent,
        event_loop: &ActiveEventLoop,
    ) {
        if let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    logical_key,
                    text,
                    state: ElementState::Pressed,
                    ..
                },
            ..
        } = &event
        {
            let ctrl = self.modifiers.control_key();
            let super_key = self.modifiers.super_key();
            let shift = self.modifiers.shift_key();

            if self.overlay.is_confirm_close_open() {
                if matches!(logical_key.as_ref(), Key::Named(NamedKey::Escape)) {
                    self.dispatch(
                        crate::app::actions::AppAction::DismissConfirmClose,
                        event_loop,
                    );
                }
                self.request_redraw();
                return;
            }

            if self.file_tree.renaming_idx.is_some() {
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.file_tree.cancel_rename();
                    }
                    Key::Named(NamedKey::Enter) => {
                        if let Some((old, new)) = self.file_tree.commit_rename() {
                            if let Err(e) = std::fs::rename(&old, &new) {
                                log::error!("Rename failed: {}", e);
                            } else if let Some(parent) = new.parent() {
                                self.reload_file_tree_at(parent);
                            }
                        }
                    }
                    Key::Named(NamedKey::Backspace) => {
                        let cur = self.file_tree.rename_cursor;
                        if cur > 0 {
                            let byte_pos = self
                                .file_tree
                                .rename_text
                                .char_indices()
                                .nth(cur - 1)
                                .map(|(i, _)| i);
                            if let Some(pos) = byte_pos {
                                let next = self.file_tree.rename_text[pos..]
                                    .chars()
                                    .next()
                                    .map(|c| c.len_utf8())
                                    .unwrap_or(0);
                                self.file_tree.rename_text.drain(pos..pos + next);
                                self.file_tree.rename_cursor -= 1;
                            }
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.file_tree.rename_cursor > 0 {
                            self.file_tree.rename_cursor -= 1;
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        let len = self.file_tree.rename_text.chars().count();
                        if self.file_tree.rename_cursor < len {
                            self.file_tree.rename_cursor += 1;
                        }
                    }
                    Key::Named(NamedKey::Home) => {
                        self.file_tree.rename_cursor = 0;
                    }
                    Key::Named(NamedKey::End) => {
                        self.file_tree.rename_cursor = self.file_tree.rename_text.chars().count();
                    }
                    _ => {
                        if let Some(txt) = text {
                            let s = txt.as_str();
                            if !s.is_empty() && !ctrl && !super_key {
                                let byte_pos = self
                                    .file_tree
                                    .rename_text
                                    .char_indices()
                                    .nth(self.file_tree.rename_cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(self.file_tree.rename_text.len());
                                self.file_tree.rename_text.insert_str(byte_pos, s);
                                self.file_tree.rename_cursor += s.chars().count();
                            }
                        }
                    }
                }
                self.request_redraw();
                return;
            }

            if self.overlay.pro_panel_open {
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.dispatch(crate::app::actions::AppAction::CloseProPanel, event_loop);
                    }
                    Key::Named(NamedKey::Enter) => {
                        self.dispatch(crate::app::actions::AppAction::ActivateLicense, event_loop);
                    }
                    Key::Named(NamedKey::Backspace) => {
                        let cur = self.overlay.pro_license_cursor;
                        if cur > 0 {
                            let input = &mut self.overlay.pro_license_input;
                            let byte_pos = input.char_indices().nth(cur - 1).map(|(i, _)| i);
                            if let Some(pos) = byte_pos {
                                let next = input[pos..]
                                    .chars()
                                    .next()
                                    .map_or(pos, |c| pos + c.len_utf8());
                                input.replace_range(pos..next, "");
                                self.overlay.pro_license_cursor = cur - 1;
                            }
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.overlay.pro_license_cursor > 0 {
                            self.overlay.pro_license_cursor -= 1;
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        let len = self.overlay.pro_license_input.chars().count();
                        if self.overlay.pro_license_cursor < len {
                            self.overlay.pro_license_cursor += 1;
                        }
                    }
                    Key::Named(NamedKey::Home) => {
                        self.overlay.pro_license_cursor = 0;
                    }
                    Key::Named(NamedKey::End) => {
                        self.overlay.pro_license_cursor =
                            self.overlay.pro_license_input.chars().count();
                    }
                    _ => {
                        if (ctrl || super_key)
                            && matches!(logical_key.as_ref(), Key::Character(c) if c == "v")
                        {
                            if let Ok(mut cb) = arboard::Clipboard::new()
                                && let Ok(pasted) = cb.get_text()
                            {
                                let cleaned: String =
                                    pasted.chars().filter(|c| !c.is_control()).collect();
                                if !cleaned.is_empty() {
                                    let cur = self.overlay.pro_license_cursor;
                                    let byte_pos = self
                                        .overlay
                                        .pro_license_input
                                        .char_indices()
                                        .nth(cur)
                                        .map(|(i, _)| i)
                                        .unwrap_or(self.overlay.pro_license_input.len());
                                    self.overlay
                                        .pro_license_input
                                        .insert_str(byte_pos, &cleaned);
                                    self.overlay.pro_license_cursor += cleaned.chars().count();
                                }
                            }
                        } else if let Some(txt) = text {
                            let s = txt.to_string();
                            if !s.is_empty() && !ctrl && !super_key {
                                let cur = self.overlay.pro_license_cursor;
                                let byte_pos = self
                                    .overlay
                                    .pro_license_input
                                    .char_indices()
                                    .nth(cur)
                                    .map(|(i, _)| i)
                                    .unwrap_or(self.overlay.pro_license_input.len());
                                self.overlay.pro_license_input.insert_str(byte_pos, &s);
                                self.overlay.pro_license_cursor += s.chars().count();
                            }
                        }
                    }
                }
                self.request_redraw();
                return;
            }

            if self.overlay.usage_panel_open {
                if matches!(logical_key.as_ref(), Key::Named(NamedKey::Escape)) {
                    self.dispatch(crate::app::actions::AppAction::CloseUsagePanel, event_loop);
                }
                self.request_redraw();
                return;
            }

            if self.usage_limit_banner.is_visible()
                && matches!(logical_key.as_ref(), Key::Named(NamedKey::Escape))
            {
                self.dispatch(
                    crate::app::actions::AppAction::DismissUsageLimitBanner,
                    event_loop,
                );
            }

            if self.context_menu.is_some() {
                if matches!(logical_key.as_ref(), Key::Named(NamedKey::Escape)) {
                    self.context_menu = None;
                    self.context_menu_target_path = None;
                    self.context_menu_target_tab = None;
                    self.request_redraw();
                }
                return;
            }

            if self.overlay.cwd_dropdown_open {
                if matches!(logical_key.as_ref(), Key::Named(NamedKey::Escape)) {
                    self.overlay.close_cwd_dropdown();
                    self.request_redraw();
                }
                return;
            }

            if self.overlay.palette_open {
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.overlay.close_palette();
                    }
                    Key::Named(NamedKey::Enter) => {
                        let cmds = self.filtered_commands();
                        if let Some(&cmd) = cmds.get(self.overlay.palette_selected) {
                            self.execute_command(cmd, event_loop);
                        } else {
                            self.overlay.close_palette();
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.overlay.palette_selected > 0 {
                            self.overlay.palette_selected -= 1;
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        let count = self.filtered_commands().len();
                        if self.overlay.palette_selected + 1 < count {
                            self.overlay.palette_selected += 1;
                        }
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.overlay.palette_query.pop();
                        self.overlay.palette_selected = 0;
                    }
                    _ => {
                        if let Some(txt) = text {
                            let s = txt.to_string();
                            if !s.is_empty() && !ctrl && !super_key {
                                self.overlay.palette_query.push_str(&s);
                                self.overlay.palette_selected = 0;
                            }
                        }
                    }
                }
                self.request_redraw();
                return;
            }

            if self.overlay.model_picker_open {
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.dispatch(crate::app::actions::AppAction::CloseModels, event_loop);
                    }
                    Key::Named(NamedKey::Enter) => {
                        let idx = self.overlay.model_picker_selected;
                        self.dispatch(
                            crate::app::actions::AppAction::LoadModel { index: idx },
                            event_loop,
                        );
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.overlay.model_picker_selected > 0 {
                            self.overlay.model_picker_selected -= 1;
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        let count = if self.config.ai.ollama_enabled
                            && !self.settings_state.ollama_models.is_empty()
                        {
                            self.settings_state.ollama_models.len()
                        } else {
                            crate::ai::registry::MODELS.len()
                        };
                        if self.overlay.model_picker_selected + 1 < count {
                            self.overlay.model_picker_selected += 1;
                        }
                    }
                    _ => {}
                }
                self.request_redraw();
                return;
            }

            if self.overlay.shell_picker_open
                && matches!(logical_key.as_ref(), Key::Named(NamedKey::Escape))
            {
                self.dispatch(crate::app::actions::AppAction::CloseShellPicker, event_loop);
                return;
            }

            if self.git_panel.commit_input_focused && self.overlay.git_panel_open {
                let max_chars = self.commit_input_max_chars();
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.git_panel.commit_input_focused = false;
                    }
                    Key::Named(NamedKey::Enter) => {
                        if super_key || ctrl {
                            self.dispatch(crate::app::actions::AppAction::GitCommit, event_loop);
                        } else {
                            self.git_panel.insert_text("\n");
                        }
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.git_panel.backspace();
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.git_panel.move_left(shift);
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.git_panel.move_right(shift);
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.git_panel.move_up(shift, max_chars);
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.git_panel.move_down(shift, max_chars);
                    }
                    Key::Named(NamedKey::Home) => {
                        self.git_panel.move_home(shift);
                    }
                    Key::Named(NamedKey::End) => {
                        self.git_panel.move_end(shift);
                    }
                    _ => {
                        if let Some(txt) = text {
                            let s = txt.as_str();
                            if (super_key || ctrl) && (s == "a" || s == "A") {
                                self.git_panel.select_all();
                            } else if !s.is_empty() && !ctrl && !super_key {
                                self.git_panel.insert_text(s);
                            }
                        }
                    }
                }
                self.request_redraw();
                return;
            }

            if self.settings_state.ollama_host_focused && self.is_settings_active() {
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.settings_state.ollama_host_focused = false;
                        self.settings_state.ollama_host_sel_anchor = None;
                    }
                    Key::Named(NamedKey::Enter) => {
                        self.settings_state.ollama_host_focused = false;
                        self.settings_state.ollama_host_sel_anchor = None;
                        self.save_config();
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.ollama_host_delete_back();
                    }
                    Key::Named(NamedKey::Delete) => {
                        self.ollama_host_delete_forward();
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.ollama_host_move_left(shift);
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.ollama_host_move_right(shift);
                    }
                    Key::Named(NamedKey::Home) => {
                        self.ollama_host_move_home(shift);
                    }
                    Key::Named(NamedKey::End) => {
                        self.ollama_host_move_end(shift);
                    }
                    _ => {
                        if let Some(txt) = text {
                            let s = txt.as_str();
                            if (super_key || ctrl) && (s == "a" || s == "A") {
                                self.ollama_host_select_all();
                            } else if (super_key || ctrl) && (s == "v" || s == "V") {
                                if let Ok(mut clip) = arboard::Clipboard::new()
                                    && let Ok(pasted) = clip.get_text()
                                {
                                    self.ollama_host_insert(&pasted);
                                }
                            } else if (super_key || ctrl) && (s == "c" || s == "C") {
                                self.ollama_host_copy();
                            } else if (super_key || ctrl) && (s == "x" || s == "X") {
                                self.ollama_host_cut();
                            } else if !s.is_empty() && !ctrl && !super_key {
                                self.ollama_host_insert(s);
                            }
                        }
                    }
                }
                self.request_redraw();
                return;
            }

            if self.settings_state.sandbox.add_image_focused && self.is_settings_active() {
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.settings_state.sandbox.add_image_focused = false;
                    }
                    Key::Named(NamedKey::Enter) => {
                        let hit =
                            crate::ui::components::overlay::settings::SandboxSettingsHit::AddImage;
                        self.handle_sandbox_settings_hit(hit);
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.settings_state.sandbox.add_image_input.pop();
                    }
                    _ => {
                        if let Some(txt) = text {
                            let s = txt.as_str();
                            if !s.is_empty() && !s.contains('\n') && !s.contains('\r') {
                                self.settings_state.sandbox.add_image_input.push_str(s);
                            }
                        }
                    }
                }
                self.request_redraw();
                return;
            }

            if matches!(logical_key.as_ref(), Key::Named(NamedKey::Escape))
                && self.is_settings_active()
            {
                self.dispatch(crate::app::actions::AppAction::CloseSettings, event_loop);
                return;
            }

            if self.search_panel.focused
                && self.panel_layout.active_tab == crate::ui::panel_layout::SidePanelTab::Search
            {
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        if self.search_panel.query.is_empty() {
                            self.search_panel.focused = false;
                        } else {
                            self.search_panel.clear();
                        }
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.search_panel.delete_back();
                    }
                    Key::Named(NamedKey::Delete) => {
                        self.search_panel.delete_forward();
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.search_panel.move_left();
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.search_panel.move_right();
                    }
                    Key::Named(NamedKey::Home) => {
                        self.search_panel.move_home();
                    }
                    Key::Named(NamedKey::End) => {
                        self.search_panel.move_end();
                    }
                    _ => {
                        if (super_key || ctrl)
                            && matches!(logical_key.as_ref(), Key::Character(c) if c == "a")
                        {
                            self.search_panel.select_all();
                        } else if (super_key || ctrl)
                            && matches!(logical_key.as_ref(), Key::Character(c) if c == "v")
                        {
                            if let Ok(mut cb) = arboard::Clipboard::new()
                                && let Ok(pasted) = cb.get_text()
                            {
                                let cleaned: String =
                                    pasted.chars().filter(|c| !c.is_control()).collect();
                                if !cleaned.is_empty() {
                                    self.search_panel.insert_text(&cleaned);
                                }
                            }
                        } else if (super_key || ctrl)
                            && matches!(logical_key.as_ref(), Key::Character(c) if c == "c" || c == "x")
                        {
                            if let Some((start, end)) = self.search_panel.selected_range() {
                                let selected = self.search_panel.query[start..end].to_string();
                                let _ = arboard::Clipboard::new()
                                    .and_then(|mut cb| cb.set_text(selected));
                                if matches!(logical_key.as_ref(), Key::Character(c) if c == "x") {
                                    self.search_panel.delete_back();
                                }
                            }
                        } else if let Some(txt) = text {
                            let s = txt.as_str();
                            if !s.is_empty() && !ctrl && !super_key {
                                for ch in s.chars() {
                                    if !ch.is_control() {
                                        self.search_panel.insert_char(ch);
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(r) = self.renderer.as_mut() {
                    let sf = r.scale_factor as f32;
                    let panel_w = self.panel_layout.left_physical_width(sf);
                    self.search_panel
                        .ensure_cursor_visible(&mut r.font_system, sf, panel_w);
                }
                self.cursor_blink_on = true;
                self.cursor_blink_at = std::time::Instant::now();
                self.request_redraw();
                return;
            }

            if self.is_models_active() {
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        if self.models_view.search_focused {
                            self.models_view.search_focused = false;
                        } else if !self.models_view.search_query.is_empty() {
                            self.models_view.search_query.clear();
                            self.models_view.selected_index = 0;
                            self.models_view.scroll_offset = 0.0;
                        } else {
                            self.dispatch(crate::app::actions::AppAction::CloseModels, event_loop);
                        }
                        self.request_redraw();
                        return;
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.models_view.search_query.pop();
                        self.models_view.search_focused = true;
                        self.models_view.selected_index = 0;
                        self.models_view.scroll_offset = 0.0;
                        self.cursor_blink_on = true;
                        self.cursor_blink_at = std::time::Instant::now();
                        self.request_redraw();
                        return;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.models_view.selected_index > 0 {
                            self.models_view.selected_index -= 1;
                        }
                        self.request_redraw();
                        return;
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        let count = self
                            .models_view
                            .filtered_indices(&self.settings_state.models_path)
                            .len();
                        if self.models_view.selected_index + 1 < count {
                            self.models_view.selected_index += 1;
                        }
                        self.request_redraw();
                        return;
                    }
                    _ => {
                        if let Some(txt) = text {
                            let s = txt.to_string();
                            if !s.is_empty() && !ctrl && !super_key {
                                self.models_view.search_focused = true;
                                self.models_view.search_query.push_str(&s);
                                self.models_view.selected_index = 0;
                                self.models_view.scroll_offset = 0.0;
                                self.cursor_blink_on = true;
                                self.cursor_blink_at = std::time::Instant::now();
                                self.request_redraw();
                                return;
                            }
                        }
                    }
                }
            }

            if ctrl && matches!(logical_key.as_ref(), Key::Character(c) if c == "p") {
                self.dispatch(crate::app::actions::AppAction::OpenPalette, event_loop);
                return;
            }

            if ctrl
                && self.modifiers.shift_key()
                && matches!(logical_key.as_ref(), Key::Character(c) if c == "D" || c == "d")
            {
                self.widget_debug.toggle();
                self.request_redraw();
                return;
            }

            if super_key
                && let Key::Character(c) = logical_key.as_ref()
                && let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10))
                && digit >= 1
                && (digit as usize) <= self.tab_mgr.len()
            {
                self.dispatch(
                    crate::app::actions::AppAction::SwitchTab {
                        index: (digit as usize) - 1,
                    },
                    event_loop,
                );
                return;
            }

            if super_key && self.modifiers.shift_key() {
                match logical_key.as_ref() {
                    Key::Character(c) if c == "{" || c == "[" => {
                        self.dispatch(crate::app::actions::AppAction::PreviousTab, event_loop);
                        return;
                    }
                    Key::Character(c) if c == "}" || c == "]" => {
                        self.dispatch(crate::app::actions::AppAction::NextTab, event_loop);
                        return;
                    }
                    Key::Character(c) if c == "F" || c == "f" => {
                        self.dispatch(
                            crate::app::actions::AppAction::SwitchPanelTab {
                                tab: crate::ui::panel_layout::SidePanelTab::Search,
                            },
                            event_loop,
                        );
                        if !self.overlay.sidebar_open {
                            self.dispatch(
                                crate::app::actions::AppAction::ToggleSidebar,
                                event_loop,
                            );
                        }
                        self.dispatch(crate::app::actions::AppAction::FocusSearchInput, event_loop);
                        return;
                    }
                    _ => {}
                }
            }

            if super_key
                && matches!(logical_key.as_ref(), Key::Character(c) if c == "v")
                && !self.is_editor_active()
            {
                self.dispatch(crate::app::actions::AppAction::Paste, event_loop);
                return;
            }

            if super_key
                && matches!(logical_key.as_ref(), Key::Character(c) if c == "c")
                && !self.is_editor_active()
            {
                if self.ai_ctrl.state.inference_rx.is_some() {
                    self.ai_ctrl.cancel_inference();
                    if self.agent.is_some() {
                        self.cancel_agent();
                    }
                    if let Some(bl) = self.active_block_list_mut() {
                        bl.finish_last();
                    }
                    self.ai_ctrl.block_written = 0;
                    self.request_redraw();
                    return;
                }
                self.dispatch(crate::app::actions::AppAction::Copy, event_loop);
                return;
            }

            if super_key && matches!(logical_key.as_ref(), Key::Character(c) if c == "a") {
                if self.is_editor_active()
                    && let Some(ed) = self.active_editor_state_mut()
                {
                    ed.select_all();
                    self.request_redraw();
                    return;
                }
                self.dispatch(crate::app::actions::AppAction::SelectAll, event_loop);
                return;
            }

            if self.is_editor_active() {
                let shift = self.modifiers.shift_key();

                if super_key && matches!(logical_key.as_ref(), Key::Character(c) if c == "s") {
                    if let Some(ed) = self.active_editor_state_mut() {
                        let _ = ed.save();
                    }
                    self.request_redraw();
                    return;
                }

                if super_key && matches!(logical_key.as_ref(), Key::Character(c) if c == "c") {
                    if let Some(ed) = self.active_editor_state()
                        && let Some(sel_text) = ed.selected_text()
                    {
                        let len = sel_text.len();
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(&sel_text);
                        }
                        self.toast_mgr.push(
                            format!("Copied {} chars", len),
                            crate::ui::components::toast::ToastLevel::Info,
                        );
                    }
                    return;
                }

                if super_key && matches!(logical_key.as_ref(), Key::Character(c) if c == "v") {
                    let paste_text = arboard::Clipboard::new()
                        .ok()
                        .and_then(|mut cb| cb.get_text().ok());
                    if let Some(txt) = paste_text {
                        let sf = self.renderer.as_ref().unwrap().scale_factor as f32;
                        let vh = self.renderer.as_ref().unwrap().height as usize;
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.insert_str(&txt);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                    }
                    self.request_redraw();
                    return;
                }

                if super_key && matches!(logical_key.as_ref(), Key::Character(c) if c == "x") {
                    let sel_text = self.active_editor_state().and_then(|ed| ed.selected_text());
                    if let Some(text_to_cut) = sel_text {
                        let len = text_to_cut.len();
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(&text_to_cut);
                        }
                        self.toast_mgr.push(
                            format!("Cut {} chars", len),
                            crate::ui::components::toast::ToastLevel::Info,
                        );
                        let sf = self.renderer.as_ref().unwrap().scale_factor as f32;
                        let vh = self.renderer.as_ref().unwrap().height as usize;
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.push_undo_snapshot();
                            ed.delete_selection();
                            ed.ensure_cursor_visible(sf, vh);
                        }
                    }
                    self.request_redraw();
                    return;
                }

                if super_key && matches!(logical_key.as_ref(), Key::Character(c) if c == "z") {
                    if shift {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.redo();
                        }
                    } else if let Some(ed) = self.active_editor_state_mut() {
                        ed.undo();
                    }
                    self.request_redraw();
                    return;
                }

                if self.modifiers.alt_key()
                    && matches!(logical_key.as_ref(), Key::Character(c) if c == "z")
                {
                    if let Some(ed) = self.active_editor_state_mut() {
                        ed.toggle_word_wrap();
                    }
                    self.request_redraw();
                    return;
                }

                use crate::ui::editor::CursorMove;
                let sf = self.renderer.as_ref().unwrap().scale_factor as f32;
                let vh = self.renderer.as_ref().unwrap().height as usize;
                let alt = self.modifiers.alt_key();

                // Cmd+Backspace — delete to line start
                if super_key && matches!(logical_key.as_ref(), Key::Named(NamedKey::Backspace)) {
                    if let Some(ed) = self.active_editor_state_mut() {
                        ed.delete_to_line_start();
                        ed.ensure_cursor_visible(sf, vh);
                    }
                    self.request_redraw();
                    return;
                }

                // Option+Backspace — delete word backward
                if alt && matches!(logical_key.as_ref(), Key::Named(NamedKey::Backspace)) {
                    if let Some(ed) = self.active_editor_state_mut() {
                        ed.delete_word_backward();
                        ed.ensure_cursor_visible(sf, vh);
                    }
                    self.request_redraw();
                    return;
                }

                // Option+Delete — delete word forward
                if alt && matches!(logical_key.as_ref(), Key::Named(NamedKey::Delete)) {
                    if let Some(ed) = self.active_editor_state_mut() {
                        ed.delete_word_forward();
                        ed.ensure_cursor_visible(sf, vh);
                    }
                    self.request_redraw();
                    return;
                }

                let handled = match logical_key.as_ref() {
                    // Cmd+Arrow — line/document boundaries
                    Key::Named(NamedKey::ArrowLeft) if super_key => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::Home, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::ArrowRight) if super_key => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::End, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::ArrowUp) if super_key => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::DocumentStart, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::ArrowDown) if super_key => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::DocumentEnd, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }

                    // Option+Arrow — word movement
                    Key::Named(NamedKey::ArrowLeft) if alt => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::WordLeft, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::ArrowRight) if alt => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::WordRight, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }

                    // Plain / Shift arrows
                    Key::Named(NamedKey::ArrowLeft) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::Left, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::Right, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::Up, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::Down, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::Home) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::Home, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::End) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.move_cursor(CursorMove::End, shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::PageUp) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            let visible = (vh as f32 / (20.0 * sf)) as usize;
                            ed.move_cursor(CursorMove::PageUp(visible), shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::PageDown) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            let visible = (vh as f32 / (20.0 * sf)) as usize;
                            ed.move_cursor(CursorMove::PageDown(visible), shift);
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::Backspace) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.delete_backward();
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::Delete) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.delete_forward();
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::Enter) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.new_line();
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::Tab) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.insert_str("    ");
                            ed.ensure_cursor_visible(sf, vh);
                        }
                        true
                    }
                    Key::Named(NamedKey::Escape) => {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.set_cursor_pos(ed.cursor_line(), ed.cursor_col());
                        }
                        true
                    }
                    _ => {
                        if !ctrl && !super_key {
                            if let Some(txt) = text {
                                let s = txt.to_string();
                                if !s.is_empty() {
                                    if let Some(ed) = self.active_editor_state_mut() {
                                        for ch in s.chars() {
                                            ed.insert_char(ch);
                                        }
                                        ed.ensure_cursor_visible(sf, vh);
                                    }
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                };

                if handled {
                    self.cursor_blink_on = true;
                    self.cursor_blink_at = Instant::now();
                    self.request_redraw();
                    return;
                }
            }

            let use_smart_input = self.settings_state.input_type == InputType::Smart
                && !self.is_sandbox_active()
                && self
                    .active_terminal()
                    .map(|t| !t.is_app_controlled())
                    .unwrap_or(false);

            let command_running = self
                .active_block_list()
                .map(|bl| bl.last_is_running())
                .unwrap_or(false);

            if use_smart_input && command_running {
                let bytes: Option<Cow<'static, [u8]>> = if ctrl {
                    match logical_key.as_ref() {
                        Key::Character(c) => {
                            let ch = c.chars().next().unwrap_or('\0');
                            if ch.is_ascii_lowercase() || ch.is_ascii_uppercase() {
                                let code = (ch.to_ascii_lowercase() as u8) - b'a' + 1;
                                Some(Cow::Owned(vec![code]))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    match logical_key.as_ref() {
                        Key::Named(NamedKey::Enter) => Some(Cow::Borrowed(b"\r")),
                        Key::Named(NamedKey::Backspace) => Some(Cow::Borrowed(b"\x7f")),
                        Key::Named(NamedKey::Tab) => Some(Cow::Borrowed(b"\t")),
                        Key::Named(NamedKey::Escape) => Some(Cow::Borrowed(b"\x1b")),
                        Key::Named(NamedKey::ArrowUp) => Some(Cow::Borrowed(b"\x1b[A")),
                        Key::Named(NamedKey::ArrowDown) => Some(Cow::Borrowed(b"\x1b[B")),
                        Key::Named(NamedKey::ArrowRight) => Some(Cow::Borrowed(b"\x1b[C")),
                        Key::Named(NamedKey::ArrowLeft) => Some(Cow::Borrowed(b"\x1b[D")),
                        Key::Named(NamedKey::Home) => Some(Cow::Borrowed(b"\x1b[H")),
                        Key::Named(NamedKey::End) => Some(Cow::Borrowed(b"\x1b[F")),
                        Key::Named(NamedKey::PageUp) => Some(Cow::Borrowed(b"\x1b[5~")),
                        Key::Named(NamedKey::PageDown) => Some(Cow::Borrowed(b"\x1b[6~")),
                        Key::Named(NamedKey::Delete) => Some(Cow::Borrowed(b"\x1b[3~")),
                        _ => {
                            if let Some(txt) = text {
                                let s = txt.to_string();
                                if !s.is_empty() {
                                    Some(Cow::Owned(s.into_bytes()))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                    }
                };

                if let Some(data) = bytes {
                    self.send_input_to_active(&data);
                }
                self.request_redraw();
            } else if use_smart_input {
                let agent_handled = if let Some(ref orch) = self.agent {
                    use crate::agent::session::AgentStatus;
                    matches!(orch.session.status, AgentStatus::AwaitingApproval(_))
                } else {
                    false
                };

                if agent_handled {
                    match logical_key.as_ref() {
                        Key::Named(NamedKey::Enter) => {
                            let sel = self.agent_approval_selected();
                            let decision = match sel {
                                1 => crate::agent::session::ApprovalDecision::ApproveToolForSession,
                                2 => crate::agent::session::ApprovalDecision::Reject {
                                    user_message: None,
                                },
                                _ => crate::agent::session::ApprovalDecision::ApproveOnce,
                            };
                            self.dispatch(
                                crate::app::actions::AppAction::AgentApproval { decision },
                                event_loop,
                            );
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("a") => {
                            self.set_agent_approval_selection(1);
                            self.dispatch(
                                crate::app::actions::AppAction::AgentApproval {
                                    decision: crate::agent::session::ApprovalDecision::ApproveToolForSession,
                                },
                                event_loop,
                            );
                        }
                        Key::Named(NamedKey::Escape) => {
                            self.set_agent_approval_selection(2);
                            self.dispatch(
                                crate::app::actions::AppAction::AgentApproval {
                                    decision: crate::agent::session::ApprovalDecision::Reject {
                                        user_message: None,
                                    },
                                },
                                event_loop,
                            );
                        }
                        Key::Named(NamedKey::Tab) | Key::Named(NamedKey::ArrowRight) => {
                            self.cycle_agent_approval_selection(1);
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.cycle_agent_approval_selection(-1);
                        }
                        _ => {}
                    }
                    self.request_redraw();
                    return;
                }

                let slash_handled = if self.smart_input.slash_menu_open {
                    match logical_key.as_ref() {
                        Key::Named(NamedKey::Escape) => {
                            self.smart_input.slash_menu_open = false;
                            self.smart_input.slash_selected = 0;
                            true
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            if self.smart_input.slash_selected > 0 {
                                self.smart_input.slash_selected -= 1;
                            }
                            true
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            let count = self.smart_input.filtered_slash_commands().len();
                            if self.smart_input.slash_selected + 1 < count {
                                self.smart_input.slash_selected += 1;
                            }
                            true
                        }
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Tab) => {
                            let filtered = self.smart_input.filtered_slash_commands();
                            if let Some(cmd) = filtered.get(self.smart_input.slash_selected) {
                                let name = cmd.name.to_string();
                                self.execute_slash_command(&name, event_loop);
                            }
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                };

                if !slash_handled {
                    match logical_key.as_ref() {
                        Key::Named(NamedKey::Enter) => {
                            use crate::ui::components::prompt_bar::InputMode;
                            let text = self.smart_input.text.trim().to_string();

                            if text == "/close" {
                                if self.smart_input.input_mode == InputMode::Agent {
                                    self.dispatch(
                                        crate::app::actions::AppAction::ExitAgentMode,
                                        event_loop,
                                    );
                                }
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if text.starts_with("/agent")
                                && (text.len() == 6 || text.as_bytes().get(6) == Some(&b' '))
                            {
                                let task = if text.len() > 6 { text[7..].trim() } else { "" };
                                self.dispatch(
                                    crate::app::actions::AppAction::EnterAgentMode,
                                    event_loop,
                                );
                                if !task.is_empty() {
                                    self.dispatch(
                                        crate::app::actions::AppAction::StartAgent {
                                            task: task.to_string(),
                                        },
                                        event_loop,
                                    );
                                }
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if self.smart_input.input_mode == InputMode::Agent
                                && !text.is_empty()
                                && !text.starts_with('/')
                            {
                                self.dispatch(
                                    crate::app::actions::AppAction::StartAgent { task: text },
                                    event_loop,
                                );
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if let Some(rest) = text.strip_prefix("/ask ") {
                                if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                                    && let super::super::TabKind::Terminal {
                                        terminal,
                                        block_list,
                                        ..
                                    } = &mut tab.kind
                                {
                                    block_list.capture_output(terminal);
                                }
                                let query = rest.trim().to_string();
                                if !query.is_empty() {
                                    self.start_ai_query(&query);
                                }
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if text == "/ask" {
                                self.smart_input.text = "/ask ".to_string();
                                self.smart_input.cursor = 5;
                            } else if text.starts_with("/summarize") {
                                if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                                    && let super::super::TabKind::Terminal {
                                        terminal,
                                        block_list,
                                        ..
                                    } = &mut tab.kind
                                {
                                    block_list.capture_output(terminal);
                                }
                                self.start_summarize();
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if text == "/clear" {
                                if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                                    && let super::super::TabKind::Terminal {
                                        terminal,
                                        block_list,
                                        ..
                                    } = &mut tab.kind
                                {
                                    terminal.input(Cow::Borrowed(b"clear\n"));
                                    block_list.blocks.clear();
                                    block_list.sync_checkpoint(terminal);
                                }
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if text == "/help" {
                                self.show_help_block();
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if text == "/models" || text.starts_with("/models ") {
                                self.open_models_view();
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if text == "clear" || text == "cls" {
                                if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                                    && let super::super::TabKind::Terminal {
                                        terminal,
                                        block_list,
                                        ..
                                    } = &mut tab.kind
                                {
                                    terminal.input(Cow::Borrowed(b"clear\n"));
                                    block_list.blocks.clear();
                                    block_list.scroll_offset = 0.0;
                                    block_list.sync_checkpoint(terminal);
                                }
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if let Some(open_path) = try_intercept_editor_command(
                                &text,
                                self.active_terminal().and_then(|t| t.cwd()).as_deref(),
                            ) {
                                self.dispatch(
                                    crate::app::actions::AppAction::OpenFile { path: open_path },
                                    event_loop,
                                );
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                            } else if let Some(tab) =
                                self.tab_mgr.get_mut(self.tab_mgr.active_index())
                                && let super::super::TabKind::Terminal {
                                    terminal,
                                    block_list,
                                    ..
                                } = &mut tab.kind
                            {
                                if !text.is_empty() {
                                    block_list.capture_output(terminal);
                                    let prompt_info = terminal.prompt_info();
                                    block_list.push_command(prompt_info, text.clone());
                                    let cmd = format!("{}\n", text);
                                    log::info!("smart input: sending command to PTY: {:?}", text);
                                    terminal.input(Cow::Owned(cmd.into_bytes()));
                                    self.smart_input.pending_command = Some(text);
                                    self.smart_input.command_started = Some(Instant::now());
                                    self.smart_input.text.clear();
                                    self.smart_input.cursor = 0;
                                } else {
                                    terminal.input(Cow::Borrowed(b"\n"));
                                }
                            }
                        }
                        Key::Named(NamedKey::Backspace) => {
                            if self.smart_input.cursor > 0 {
                                let new_cursor = self.smart_input.text[..self.smart_input.cursor]
                                    .char_indices()
                                    .next_back()
                                    .map(|(i, _)| i)
                                    .unwrap_or(0);
                                self.smart_input
                                    .text
                                    .drain(new_cursor..self.smart_input.cursor);
                                self.smart_input.cursor = new_cursor;
                            }
                        }
                        Key::Named(NamedKey::Delete) => {
                            if self.smart_input.cursor < self.smart_input.text.len() {
                                let next = self.smart_input.text[self.smart_input.cursor..]
                                    .char_indices()
                                    .nth(1)
                                    .map(|(i, _)| self.smart_input.cursor + i)
                                    .unwrap_or(self.smart_input.text.len());
                                self.smart_input.text.drain(self.smart_input.cursor..next);
                            }
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            if self.smart_input.cursor > 0 {
                                self.smart_input.cursor = self.smart_input.text
                                    [..self.smart_input.cursor]
                                    .char_indices()
                                    .next_back()
                                    .map(|(i, _)| i)
                                    .unwrap_or(0);
                            }
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            if self.smart_input.cursor == self.smart_input.text.len() {
                                if let Some(suggestion) = self.smart_input.suggestion.take() {
                                    self.smart_input.text = suggestion;
                                    self.smart_input.cursor = self.smart_input.text.len();
                                }
                            } else {
                                self.smart_input.cursor = self.smart_input.text
                                    [self.smart_input.cursor..]
                                    .char_indices()
                                    .nth(1)
                                    .map(|(i, _)| self.smart_input.cursor + i)
                                    .unwrap_or(self.smart_input.text.len());
                            }
                        }
                        Key::Named(NamedKey::Home) => {
                            self.smart_input.cursor = 0;
                        }
                        Key::Named(NamedKey::End) => {
                            self.smart_input.cursor = self.smart_input.text.len();
                        }
                        Key::Named(NamedKey::Escape) => {
                            if self.ai_ctrl.state.inference_rx.is_some() {
                                self.ai_ctrl.cancel_inference();
                                if self.agent.is_some() {
                                    self.cancel_agent();
                                }
                                if let Some(bl) = self.active_block_list_mut() {
                                    bl.finish_last();
                                }
                                self.ai_ctrl.block_written = 0;
                            } else if self.agent.is_some() {
                                self.cancel_agent();
                            } else {
                                self.smart_input.text.clear();
                                self.smart_input.cursor = 0;
                                self.smart_input.ai_suggestion = None;
                            }
                        }
                        Key::Named(NamedKey::Tab) => {
                            if let Some(suggestion) = self.smart_input.suggestion.take() {
                                self.smart_input.text = suggestion;
                                self.smart_input.cursor = self.smart_input.text.len();
                            } else if let Some(ai_cmd) = self.smart_input.ai_suggestion.take() {
                                self.smart_input.text = ai_cmd;
                                self.smart_input.cursor = self.smart_input.text.len();
                            }
                        }
                        Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowDown) => {
                            let history: Vec<String> = self
                                .active_block_list()
                                .map(|bl| {
                                    bl.command_history().iter().map(|s| s.to_string()).collect()
                                })
                                .unwrap_or_default();
                            if history.is_empty() {
                            } else if matches!(logical_key.as_ref(), Key::Named(NamedKey::ArrowUp))
                            {
                                match self.smart_input.history_index {
                                    None => {
                                        self.smart_input.history_stash =
                                            self.smart_input.text.clone();
                                        self.smart_input.history_index = Some(0);
                                        self.smart_input.text = history[history.len() - 1].clone();
                                        self.smart_input.cursor = self.smart_input.text.len();
                                    }
                                    Some(idx) => {
                                        let new_idx = (idx + 1).min(history.len() - 1);
                                        self.smart_input.history_index = Some(new_idx);
                                        self.smart_input.text =
                                            history[history.len() - 1 - new_idx].clone();
                                        self.smart_input.cursor = self.smart_input.text.len();
                                    }
                                }
                            } else {
                                match self.smart_input.history_index {
                                    Some(0) => {
                                        self.smart_input.history_index = None;
                                        self.smart_input.text =
                                            self.smart_input.history_stash.clone();
                                        self.smart_input.cursor = self.smart_input.text.len();
                                    }
                                    Some(idx) => {
                                        let new_idx = idx - 1;
                                        self.smart_input.history_index = Some(new_idx);
                                        self.smart_input.text =
                                            history[history.len() - 1 - new_idx].clone();
                                        self.smart_input.cursor = self.smart_input.text.len();
                                    }
                                    None => {}
                                }
                            }
                        }
                        _ => {
                            if ctrl {
                                if matches!(logical_key.as_ref(), Key::Character(c) if c == "c") {
                                    if let Some(terminal) = self.active_terminal() {
                                        terminal.input(Cow::Borrowed(b"\x03"));
                                    }
                                    if let Some(started) = self.smart_input.command_started.take() {
                                        self.smart_input.last_command_duration =
                                            Some(started.elapsed());
                                    }
                                    self.smart_input.pending_command = None;
                                    self.smart_input.text.clear();
                                    self.smart_input.cursor = 0;
                                } else if matches!(logical_key.as_ref(), Key::Character(c) if c == "a")
                                {
                                    self.smart_input.cursor = 0;
                                } else if matches!(logical_key.as_ref(), Key::Character(c) if c == "e")
                                {
                                    self.smart_input.cursor = self.smart_input.text.len();
                                } else if matches!(logical_key.as_ref(), Key::Character(c) if c == "u")
                                {
                                    self.smart_input.text.clear();
                                    self.smart_input.cursor = 0;
                                } else if matches!(logical_key.as_ref(), Key::Character(c) if c == "k")
                                {
                                    self.smart_input.text.truncate(self.smart_input.cursor);
                                } else if matches!(logical_key.as_ref(), Key::Character(c) if c == "w")
                                {
                                    let before = &self.smart_input.text[..self.smart_input.cursor];
                                    let trimmed = before.trim_end();
                                    let new_end = trimmed
                                        .rfind(|c: char| c.is_whitespace())
                                        .map(|i| i + 1)
                                        .unwrap_or(0);
                                    self.smart_input
                                        .text
                                        .drain(new_end..self.smart_input.cursor);
                                    self.smart_input.cursor = new_end;
                                }
                            } else if !super_key && let Some(txt) = text {
                                let s = txt.to_string();
                                if !s.is_empty() {
                                    self.smart_input
                                        .text
                                        .insert_str(self.smart_input.cursor, &s);
                                    self.smart_input.cursor += s.len();
                                }
                            }
                        }
                    }
                }

                self.smart_input.update_slash_menu();
                let cwd = self.active_terminal().and_then(|t| t.cwd());
                self.smart_input.update_suggestion(cwd.as_deref());
                if !matches!(
                    logical_key.as_ref(),
                    Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowDown)
                ) {
                    self.smart_input.ai_suggestion = None;
                    if self.smart_input.history_index.is_some()
                        && !matches!(logical_key.as_ref(), Key::Named(NamedKey::Enter))
                    {
                        self.smart_input.history_index = None;
                    }
                }
                self.cursor_blink_on = true;
                self.cursor_blink_at = Instant::now();
                self.request_redraw();
            } else if let Some(terminal) = self.active_terminal() {
                let bytes: Option<Cow<'static, [u8]>> = if ctrl {
                    match logical_key.as_ref() {
                        Key::Character(c) => {
                            let ch = c.chars().next().unwrap_or('\0');
                            if ch.is_ascii_lowercase() || ch.is_ascii_uppercase() {
                                let code = (ch.to_ascii_lowercase() as u8) - b'a' + 1;
                                Some(Cow::Owned(vec![code]))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    match logical_key.as_ref() {
                        Key::Named(NamedKey::Enter) => Some(Cow::Borrowed(b"\r")),
                        Key::Named(NamedKey::Backspace) => Some(Cow::Borrowed(b"\x7f")),
                        Key::Named(NamedKey::Tab) => Some(Cow::Borrowed(b"\t")),
                        Key::Named(NamedKey::Escape) => Some(Cow::Borrowed(b"\x1b")),
                        Key::Named(NamedKey::ArrowUp) => Some(Cow::Borrowed(b"\x1b[A")),
                        Key::Named(NamedKey::ArrowDown) => Some(Cow::Borrowed(b"\x1b[B")),
                        Key::Named(NamedKey::ArrowRight) => Some(Cow::Borrowed(b"\x1b[C")),
                        Key::Named(NamedKey::ArrowLeft) => Some(Cow::Borrowed(b"\x1b[D")),
                        Key::Named(NamedKey::Home) => Some(Cow::Borrowed(b"\x1b[H")),
                        Key::Named(NamedKey::End) => Some(Cow::Borrowed(b"\x1b[F")),
                        Key::Named(NamedKey::PageUp) => Some(Cow::Borrowed(b"\x1b[5~")),
                        Key::Named(NamedKey::PageDown) => Some(Cow::Borrowed(b"\x1b[6~")),
                        Key::Named(NamedKey::Delete) => Some(Cow::Borrowed(b"\x1b[3~")),
                        _ => {
                            if let Some(txt) = text {
                                let s = txt.to_string();
                                if !s.is_empty() {
                                    Some(Cow::Owned(s.into_bytes()))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                    }
                };

                if let Some(data) = bytes {
                    terminal.input(data);
                }
            }
        }
    }

    /// Open a context menu for a file tree item.
    fn open_file_tree_context_menu(
        &mut self,
        path: std::path::PathBuf,
        anchor_x: usize,
        anchor_y: usize,
    ) {
        use crate::ui::components::context_menu::{ContextMenuItem, ContextMenuState};

        let is_dir = path.is_dir();
        let mut items = Vec::new();

        items.push(ContextMenuItem::action("new_file", "New File"));
        items.push(ContextMenuItem::action("new_folder", "New Folder"));
        items.push(ContextMenuItem::Separator);

        if !is_dir {
            items.push(ContextMenuItem::action("open", "Open"));
        }
        items.push(ContextMenuItem::action("rename", "Rename"));
        items.push(ContextMenuItem::Separator);
        items.push(ContextMenuItem::action(
            "reveal_in_finder",
            "Reveal in Finder",
        ));
        items.push(ContextMenuItem::Separator);
        items.push(ContextMenuItem::destructive("delete", "Delete"));

        self.context_menu = Some(ContextMenuState::new(items, anchor_x, anchor_y));
        self.context_menu_target_path = Some(path);
    }

    fn open_git_file_context_menu(&mut self, rel_path: String, anchor_x: usize, anchor_y: usize) {
        use crate::ui::components::context_menu::{ContextMenuItem, ContextMenuState};

        let items = vec![
            ContextMenuItem::destructive("git_discard", "Discard Changes"),
            ContextMenuItem::Separator,
            ContextMenuItem::action("git_open", "Open File"),
            ContextMenuItem::action("git_gitignore", "Add to .gitignore"),
            ContextMenuItem::Separator,
            ContextMenuItem::action("git_reveal", "Reveal in Finder"),
        ];

        self.context_menu = Some(ContextMenuState::new(items, anchor_x, anchor_y));
        self.context_menu_target_path = Some(std::path::PathBuf::from(rel_path));
    }

    fn open_tab_context_menu(&mut self, tab_idx: usize, anchor_x: usize, anchor_y: usize) {
        use crate::ui::components::context_menu::{ContextMenuItem, ContextMenuState};

        let tab_count = self.tab_mgr.len();
        let has_others = tab_count > 1;
        let has_right = tab_idx + 1 < tab_count;

        let mut items = Vec::new();
        items.push(ContextMenuItem::action("tab_close", "Close"));
        if has_others {
            items.push(ContextMenuItem::action("tab_close_others", "Close Others"));
        } else {
            items.push(ContextMenuItem::disabled(
                "tab_close_others",
                "Close Others",
            ));
        }
        if has_right {
            items.push(ContextMenuItem::action(
                "tab_close_right",
                "Close to the Right",
            ));
        } else {
            items.push(ContextMenuItem::disabled(
                "tab_close_right",
                "Close to the Right",
            ));
        }
        items.push(ContextMenuItem::action("tab_close_all", "Close All"));
        items.push(ContextMenuItem::Separator);
        items.push(ContextMenuItem::action("tab_copy_path", "Copy Path"));

        self.context_menu = Some(ContextMenuState::new(items, anchor_x, anchor_y));
        self.context_menu_target_tab = Some(tab_idx);
    }

    /// Handle a left-click inside the editor view — position cursor at click.
    fn handle_tab_context_action(
        &mut self,
        id: &str,
        tab_idx: usize,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        use crate::app::actions::AppAction;
        match id {
            "tab_close" => {
                self.dispatch(AppAction::CloseTab { index: tab_idx }, event_loop);
            }
            "tab_close_others" => {
                let count = self.tab_mgr.len();
                for i in (0..count).rev() {
                    if i != tab_idx {
                        self.dispatch(AppAction::ForceCloseTab { index: i }, event_loop);
                    }
                }
            }
            "tab_close_right" => {
                let count = self.tab_mgr.len();
                for i in (tab_idx + 1..count).rev() {
                    self.dispatch(AppAction::ForceCloseTab { index: i }, event_loop);
                }
            }
            "tab_close_all" => {
                let count = self.tab_mgr.len();
                for i in (0..count).rev() {
                    self.dispatch(AppAction::ForceCloseTab { index: i }, event_loop);
                }
            }
            "tab_copy_path" => {
                if let Some(tab) = self.tab_mgr.get(tab_idx) {
                    let text = match &tab.kind {
                        super::super::TabKind::Terminal { terminal, .. } => {
                            terminal.cwd().unwrap_or_default()
                        }
                        super::super::TabKind::Editor { state, .. } => {
                            state.path.to_string_lossy().to_string()
                        }
                        _ => String::new(),
                    };
                    if !text.is_empty()
                        && let Ok(mut clipboard) = arboard::Clipboard::new()
                    {
                        let _ = clipboard.set_text(&text);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_editor_click(&mut self) {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };
        let sf = renderer.scale_factor as f32;
        let bar_h = renderer.tab_bar_height as usize;
        let x_off = self.side_panel_x_offset();

        let phys_x = self.cursor_pos.0 as usize;
        let phys_y = self.cursor_pos.1 as usize;

        if phys_y < bar_h {
            return;
        }

        let git_w = if self.overlay.git_panel_open {
            self.panel_layout.right_physical_width(sf)
        } else {
            0
        };
        let right_edge = (renderer.width as usize).saturating_sub(git_w);
        if phys_x >= right_edge {
            return;
        }

        let content_h = (renderer.height as usize).saturating_sub(bar_h);
        let shift = self.modifiers.shift_key();

        if let Some(state) = self.active_editor_state()
            && let Some((line, col)) = crate::ui::components::editor_renderer::hit_test_cursor(
                state, phys_x, phys_y, x_off, bar_h, sf,
            )
        {
            let viewport_h = content_h;
            if let Some(ed) = self.active_editor_state_mut() {
                if shift {
                    ed.set_cursor_pos_selecting(line, col);
                } else {
                    ed.set_cursor_pos(line, col);
                }
                ed.ensure_cursor_visible(sf, viewport_h);
            }
            self.editor_selecting = true;
            self.request_redraw();
        }
    }

    pub(crate) fn handle_mouse_event(&mut self, event: &WindowEvent, event_loop: &ActiveEventLoop) {
        match event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.overlay.is_confirm_close_open() {
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    let (bw, bh) = self
                        .renderer
                        .as_ref()
                        .map(|r| (r.width as usize, r.height as usize))
                        .unwrap_or((0, 0));
                    let hit = crate::ui::components::overlay::confirm_close_hit_test(
                        self.cursor_pos.0 as usize,
                        self.cursor_pos.1 as usize,
                        bw,
                        bh,
                        sf,
                    );
                    if let Some(idx) = self.overlay.confirm_close_tab {
                        match hit {
                            crate::ui::components::overlay::ConfirmCloseHit::Save => {
                                self.dispatch(
                                    crate::app::actions::AppAction::SaveAndCloseTab { index: idx },
                                    event_loop,
                                );
                            }
                            crate::ui::components::overlay::ConfirmCloseHit::DontSave => {
                                self.dispatch(
                                    crate::app::actions::AppAction::ForceCloseTab { index: idx },
                                    event_loop,
                                );
                            }
                            crate::ui::components::overlay::ConfirmCloseHit::Cancel
                            | crate::ui::components::overlay::ConfirmCloseHit::Backdrop => {
                                self.dispatch(
                                    crate::app::actions::AppAction::DismissConfirmClose,
                                    event_loop,
                                );
                            }
                        }
                    }
                    return;
                }

                if self.overlay.pro_panel_open {
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    let (bw, bh) = self.buf_size();
                    if let Some(hit) = crate::ui::components::overlay::pro_panel::pro_panel_hit_test(
                        bw,
                        bh,
                        self.license_mgr.is_pro(),
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                        sf,
                    ) {
                        use crate::ui::components::overlay::pro_panel::ProPanelHit;
                        match hit {
                            ProPanelHit::BuyPro => {
                                self.dispatch(crate::app::actions::AppAction::BuyPro, event_loop)
                            }
                            ProPanelHit::ActivateKey => self.dispatch(
                                crate::app::actions::AppAction::ActivateLicense,
                                event_loop,
                            ),
                            ProPanelHit::Deactivate => self.dispatch(
                                crate::app::actions::AppAction::DeactivateLicense,
                                event_loop,
                            ),
                            ProPanelHit::FocusInput => {
                                self.overlay.pro_license_focused = true;
                                self.request_redraw();
                            }
                            ProPanelHit::Close | ProPanelHit::Backdrop => self.dispatch(
                                crate::app::actions::AppAction::CloseProPanel,
                                event_loop,
                            ),
                        }
                    }
                    return;
                }

                if self.overlay.usage_panel_open {
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    let (bw, bh) = self.buf_size();
                    if let Some(hit) =
                        crate::ui::components::overlay::usage_panel::usage_panel_hit_test(
                            bw,
                            bh,
                            &self.usage_tracker,
                            self.cursor_pos.0,
                            self.cursor_pos.1,
                            sf,
                        )
                    {
                        use crate::ui::components::overlay::usage_panel::UsagePanelHit;
                        match hit {
                            UsagePanelHit::UpgradePro => self
                                .dispatch(crate::app::actions::AppAction::OpenProPanel, event_loop),
                            UsagePanelHit::Close | UsagePanelHit::Backdrop => self.dispatch(
                                crate::app::actions::AppAction::CloseUsagePanel,
                                event_loop,
                            ),
                        }
                    }
                    return;
                }

                if self.toast_mgr.has_active() {
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    let bw = self
                        .renderer
                        .as_ref()
                        .map(|r| r.width as usize)
                        .unwrap_or(0);
                    if let Some(idx) =
                        self.toast_mgr
                            .hit_test(self.cursor_pos.0, self.cursor_pos.1, bw, sf)
                    {
                        self.toast_mgr.dismiss_at(idx);
                        self.request_redraw();
                        return;
                    }
                }

                if let Some(menu) = &self.context_menu {
                    let mx = self.cursor_pos.0 as usize;
                    let my = self.cursor_pos.1 as usize;
                    let (bw, bh, sf) = self
                        .renderer
                        .as_ref()
                        .map(|r| (r.width as usize, r.height as usize, r.scale_factor as f32))
                        .unwrap_or((0, 0, 1.0));
                    if crate::ui::components::context_menu::is_inside_menu(menu, mx, my, bw, bh, sf)
                    {
                        if let Some(action_id) =
                            crate::ui::components::context_menu::context_menu_hit_test(
                                menu, mx, my, bw, bh, sf,
                            )
                        {
                            let id = action_id.to_string();
                            let path = self.context_menu_target_path.clone();
                            let tab_idx = self.context_menu_target_tab;
                            self.context_menu = None;
                            self.context_menu_target_path = None;
                            self.context_menu_target_tab = None;
                            if id.starts_with("tab_") {
                                if let Some(idx) = tab_idx {
                                    self.handle_tab_context_action(&id, idx, event_loop);
                                }
                            } else if let Some(p) = path {
                                self.dispatch(
                                    crate::app::actions::AppAction::ContextMenuAction {
                                        id,
                                        path: p,
                                    },
                                    event_loop,
                                );
                            }
                            self.request_redraw();
                            return;
                        }
                    } else {
                        self.context_menu = None;
                        self.context_menu_target_path = None;
                        self.context_menu_target_tab = None;
                        self.request_redraw();
                        return;
                    }
                }

                if let Some((rx, ry, rw, rh)) = self.overlay.stop_button_rect {
                    let cx = self.cursor_pos.0 as usize;
                    let cy = self.cursor_pos.1 as usize;
                    if cx >= rx && cx < rx + rw && cy >= ry && cy < ry + rh {
                        self.dispatch(crate::app::actions::AppAction::CancelInference, event_loop);
                        return;
                    }
                }

                if self.overlay.cwd_dropdown_open {
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    if let Some((bx, by, _, _)) = self.overlay.cwd_badge_rect {
                        if let Some(idx) = crate::ui::components::prompt_bar::cwd_dropdown::hit_test(
                            self.cursor_pos.0,
                            self.cursor_pos.1,
                            bx,
                            by,
                            self.overlay.cwd_dropdown_entries.len(),
                            self.overlay.cwd_dropdown_scroll,
                            sf,
                        ) {
                            let dir_name = self.overlay.cwd_dropdown_entries[idx].clone();
                            self.overlay.close_cwd_dropdown();
                            let cwd = self.resolve_cwd().unwrap_or_else(|| "~".into());
                            let target = std::path::Path::new(&cwd).join(&dir_name);
                            let path_str = target.to_string_lossy().to_string();
                            let cmd_text = format!("cd {}", shell_escape(&path_str));
                            if let Some(crate::app::TabKind::Terminal {
                                terminal,
                                block_list,
                                ..
                            }) = self.tab_mgr.active_tab_mut().map(|t| &mut t.kind)
                            {
                                block_list.capture_output(terminal);
                                let prompt_info = terminal.prompt_info();
                                block_list.push_command(prompt_info, cmd_text.clone());
                                let cmd = format!("{cmd_text}\n");
                                terminal.input(std::borrow::Cow::Owned(cmd.into_bytes()));
                                self.smart_input.pending_command = Some(cmd_text);
                                self.smart_input.command_started = Some(std::time::Instant::now());
                            }
                            self.request_redraw();
                            return;
                        }

                        let in_dropdown = crate::ui::components::prompt_bar::cwd_dropdown::contains(
                            self.cursor_pos.0,
                            self.cursor_pos.1,
                            bx,
                            by,
                            self.overlay.cwd_dropdown_entries.len(),
                            sf,
                        );
                        if !in_dropdown {
                            self.overlay.close_cwd_dropdown();
                            self.request_redraw();
                        }
                    } else {
                        self.overlay.close_cwd_dropdown();
                        self.request_redraw();
                    }
                }

                if let Some((rx, ry, rw, rh)) = self.overlay.cwd_badge_rect {
                    let cx = self.cursor_pos.0 as usize;
                    let cy = self.cursor_pos.1 as usize;
                    if cx >= rx && cx < rx + rw && cy >= ry && cy < ry + rh {
                        if self.overlay.cwd_dropdown_open {
                            self.overlay.close_cwd_dropdown();
                        } else {
                            let cwd = self.resolve_cwd().unwrap_or_else(|| "~".into());
                            let entries =
                                crate::ui::components::prompt_bar::cwd_dropdown::list_subdirectories(
                                    &cwd,
                                );
                            self.overlay.open_cwd_dropdown(entries);
                        }
                        self.request_redraw();
                        return;
                    }
                }

                if self.usage_limit_banner.is_visible()
                    && let Some(renderer) = &self.renderer
                {
                    let sf = renderer.scale_factor as f32;
                    let bw = renderer.width as usize;
                    let bh = renderer.height as usize;
                    if let Some(hit) = crate::ui::components::usage_limit_banner::hit_test(
                        &self.usage_limit_banner,
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                        bw,
                        bh,
                        sf,
                    ) {
                        use crate::ui::components::usage_limit_banner::UsageLimitBannerHit;
                        match hit {
                            UsageLimitBannerHit::Dismiss | UsageLimitBannerHit::Backdrop => {
                                self.dispatch(
                                    crate::app::actions::AppAction::DismissUsageLimitBanner,
                                    event_loop,
                                );
                            }
                            UsageLimitBannerHit::Upgrade => {
                                self.dispatch(
                                    crate::app::actions::AppAction::DismissUsageLimitBanner,
                                    event_loop,
                                );
                                self.dispatch(
                                    crate::app::actions::AppAction::OpenProPanel,
                                    event_loop,
                                );
                            }
                        }
                        return;
                    }
                }

                if self.hint_banner.is_visible()
                    && let Some(renderer) = &self.renderer
                {
                    let sf = renderer.scale_factor as f32;
                    let prompt_h = crate::ui::components::prompt_bar::prompt_bar_height(sf);
                    let banner_h =
                        crate::ui::components::hint_banner::banner_height(&self.hint_banner, sf);
                    if banner_h > 0 {
                        let banner_y =
                            (renderer.height as usize).saturating_sub(prompt_h + banner_h);
                        let git_w = if self.overlay.git_panel_open {
                            self.panel_layout.right_physical_width(sf)
                        } else {
                            0
                        };
                        let banner_w = (renderer.width as usize).saturating_sub(git_w);
                        if crate::ui::components::hint_banner::hit_test_dismiss(
                            self.cursor_pos.0,
                            self.cursor_pos.1,
                            &self.hint_banner,
                            banner_y,
                            banner_w,
                            sf,
                        ) {
                            self.dispatch(
                                crate::app::actions::AppAction::DismissHintBanner,
                                event_loop,
                            );
                            return;
                        }
                    }
                }

                if self.overlay.shell_picker_open {
                    let (ax, ay) = self.shell_picker_anchor();
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    let picker_state = self.build_shell_picker_state();
                    if let Some(choice) = crate::ui::components::overlay::shell_picker_hit_test(
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                        ax,
                        ay,
                        &picker_state,
                        sf,
                    ) {
                        use crate::ui::components::overlay::ShellPickerChoice;
                        self.dispatch(crate::app::actions::AppAction::CloseShellPicker, event_loop);
                        match choice {
                            ShellPickerChoice::LocalShell(idx) => {
                                let path = self.available_shells[idx].1.clone();
                                self.dispatch(
                                    crate::app::actions::AppAction::CreateTab {
                                        shell_path: Some(path),
                                    },
                                    event_loop,
                                );
                            }
                            ShellPickerChoice::Sandbox(idx) => {
                                self.dispatch(
                                    crate::app::actions::AppAction::CreateSandboxTab {
                                        image_idx: idx,
                                    },
                                    event_loop,
                                );
                            }
                        }
                        return;
                    }
                    let on_button = self.renderer.as_ref().is_some_and(|r| {
                        matches!(
                            crate::ui::components::tab_bar::hit_test(
                                self.cursor_pos.0,
                                self.cursor_pos.1,
                                self.tab_mgr.len(),
                                r.tab_bar_height as f64,
                                r.width as f64,
                                r.scale_factor,
                                self.is_fullscreen,
                                self.overlay.update_badge_w,
                            ),
                            crate::ui::components::tab_bar::TabBarHit::ShellPicker
                        )
                    });
                    if !on_button {
                        self.dispatch(crate::app::actions::AppAction::CloseShellPicker, event_loop);
                    }
                }

                if self.scrollbar_hovered {
                    self.scrollbar_dragging = true;
                    self.scrollbar_drag_start_y = self.cursor_pos.1;
                    self.scrollbar_drag_start_scroll = self
                        .active_block_list()
                        .map(|bl| bl.scroll_offset)
                        .unwrap_or(0.0);
                    return;
                }

                if self.overlay.user_menu_open
                    && let Some(renderer) = &self.renderer
                {
                    let item = crate::ui::components::tab_bar::user_menu_hit_test(
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                        renderer.tab_bar_height as f64,
                        renderer.width as f64,
                        renderer.scale_factor,
                        self.license_mgr.is_pro(),
                    );
                    self.overlay.close_user_menu();
                    match item {
                        Some(0) => {
                            self.dispatch(crate::app::actions::AppAction::OpenSettings, event_loop);
                        }
                        Some(1) => {
                            self.dispatch(crate::app::actions::AppAction::OpenModels, event_loop);
                        }
                        Some(2) if !self.license_mgr.is_pro() => {
                            self.dispatch(crate::app::actions::AppAction::OpenProPanel, event_loop);
                        }
                        _ => {}
                    }
                    self.request_redraw();
                    return;
                }

                if self.overlay.update_dropdown_open
                    && let Some(renderer) = &self.renderer
                {
                    let item = crate::ui::components::tab_bar::update_dropdown_hit_test(
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                        renderer.tab_bar_height as f64,
                        renderer.width as f64,
                        renderer.scale_factor,
                        self.overlay.update_badge_w.unwrap_or(0.0),
                    );
                    self.overlay.close_update_dropdown();
                    if item == Some(0) && !self.overlay.update_downloading {
                        if self.overlay.update_downloaded.is_some() {
                            self.dispatch(
                                crate::app::actions::AppAction::InstallUpdate,
                                event_loop,
                            );
                        } else {
                            self.dispatch(
                                crate::app::actions::AppAction::DownloadUpdate,
                                event_loop,
                            );
                        }
                    }
                    self.request_redraw();
                    return;
                }

                if let Some(renderer) = &self.renderer {
                    let hit = crate::ui::components::tab_bar::hit_test(
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                        self.tab_mgr.len(),
                        renderer.tab_bar_height as f64,
                        renderer.width as f64,
                        renderer.scale_factor,
                        self.is_fullscreen,
                        self.overlay.update_badge_w,
                    );
                    use crate::app::actions::AppAction;
                    match hit {
                        TabBarHit::Tab(idx) if idx < self.tab_mgr.len() => {
                            self.drag.begin(idx, self.cursor_pos.0);
                            self.dispatch(AppAction::SwitchTab { index: idx }, event_loop);
                        }
                        TabBarHit::CloseTab(idx) if idx < self.tab_mgr.len() => {
                            self.dispatch(AppAction::CloseTab { index: idx }, event_loop);
                        }
                        TabBarHit::Tab(_) | TabBarHit::CloseTab(_) => {}
                        TabBarHit::NewTab => {
                            self.dispatch(AppAction::CreateTab { shell_path: None }, event_loop);
                        }
                        TabBarHit::ShellPicker => {
                            self.dispatch(AppAction::ToggleShellPicker, event_loop);
                        }
                        TabBarHit::SidebarToggle => {
                            self.dispatch(AppAction::ToggleSidebar, event_loop);
                        }
                        TabBarHit::GitPanelToggle => {
                            self.dispatch(AppAction::ToggleGitPanel, event_loop);
                        }
                        TabBarHit::Settings => {
                            self.dispatch(AppAction::ToggleUserMenu, event_loop);
                        }
                        TabBarHit::UpdateBadge => {
                            self.dispatch(AppAction::ToggleUpdateDropdown, event_loop);
                        }
                        TabBarHit::EmptyBar => {
                            let now = Instant::now();
                            let is_double = self
                                .overlay
                                .last_empty_bar_click
                                .map(|t| now.duration_since(t).as_millis() < 400)
                                .unwrap_or(false);

                            if is_double {
                                self.overlay.last_empty_bar_click = None;
                                #[cfg(target_os = "macos")]
                                if let Some(window) = &self.window {
                                    use winit::raw_window_handle::{
                                        HasWindowHandle, RawWindowHandle,
                                    };
                                    if let Ok(RawWindowHandle::AppKit(handle)) =
                                        window.window_handle().map(|h| h.as_raw())
                                    {
                                        unsafe {
                                            let ns_view: objc2::rc::Retained<
                                                objc2_app_kit::NSView,
                                            > = objc2::rc::Retained::retain(
                                                handle.ns_view.as_ptr()
                                                    as *mut objc2_app_kit::NSView,
                                            )
                                            .unwrap();
                                            if let Some(ns_window) = ns_view.window() {
                                                ns_window.zoom(None);
                                            }
                                        }
                                    }
                                }
                            } else {
                                self.overlay.last_empty_bar_click = Some(now);
                                if let Some(window) = &self.window {
                                    let _ = window.drag_window();
                                }
                            }
                        }
                        TabBarHit::None => {
                            if self.overlay.sidebar_open && self.panel_layout.left_resize.hovered {
                                self.panel_layout.begin_resize();
                                return;
                            }

                            if self.overlay.git_panel_open && self.panel_layout.right_resize.hovered
                            {
                                self.panel_layout.begin_right_resize();
                                return;
                            }

                            if self.overlay.sidebar_open && self.file_tree.scrollbar_hovered {
                                self.file_tree.scrollbar_dragging = true;
                                self.file_tree.scrollbar_drag_start_y = self.cursor_pos.1;
                                self.file_tree.scrollbar_drag_start_scroll =
                                    self.file_tree.scroll_offset;
                                return;
                            }

                            if self.overlay.sidebar_open
                                && let Some(action) = self.side_panel_action()
                            {
                                if self.file_tree.renaming_idx.is_some() {
                                    self.file_tree.cancel_rename();
                                    self.request_redraw();
                                }
                                if matches!(
                                    action,
                                    crate::app::actions::AppAction::FocusSearchInput
                                ) {
                                    self.search_panel.focused = true;
                                    if let Some(r) = self.renderer.as_mut() {
                                        let sf = r.scale_factor as f32;
                                        let pos = self.search_panel.click_to_cursor(
                                            self.cursor_pos.0,
                                            &mut r.font_system,
                                            sf,
                                        );
                                        self.search_panel.cursor = pos;
                                        self.search_panel.selection_anchor = Some(pos);
                                        self.search_panel.input_mouse_dragging = true;
                                    }
                                    self.cursor_blink_on = true;
                                    self.cursor_blink_at = std::time::Instant::now();
                                    self.request_redraw();
                                } else {
                                    self.search_panel.focused = false;
                                    match &action {
                                        crate::app::actions::AppAction::OpenFile { path }
                                        | crate::app::actions::AppAction::ToggleFileTreeNode {
                                            path,
                                        } => {
                                            self.path_drag.begin(
                                                path.clone(),
                                                action,
                                                self.cursor_pos.0,
                                                self.cursor_pos.1,
                                            );
                                        }
                                        _ => {
                                            self.dispatch(action, event_loop);
                                        }
                                    }
                                }
                            }

                            if self.overlay.git_panel_open && self.git_panel.scrollbar_hovered {
                                self.git_panel.scrollbar_dragging = true;
                                self.git_panel.scrollbar_drag_start_y = self.cursor_pos.1;
                                self.git_panel.scrollbar_drag_start_scroll =
                                    self.git_panel.scroll_offset;
                                return;
                            }

                            if self.overlay.git_panel_open {
                                if let Some(action) = self.git_panel_action() {
                                    if !matches!(
                                        action,
                                        crate::app::actions::AppAction::GitFocusCommitInput { .. }
                                    ) {
                                        self.git_panel.commit_input_focused = false;
                                    }
                                    if let crate::app::actions::AppAction::GitOpenFileDiff {
                                        index,
                                    } = &action
                                    {
                                        if let Some(abs) = self.git_entry_abs_path(*index) {
                                            self.path_drag.begin(
                                                abs,
                                                action,
                                                self.cursor_pos.0,
                                                self.cursor_pos.1,
                                            );
                                        } else {
                                            self.dispatch(action, event_loop);
                                        }
                                    } else {
                                        self.dispatch(action, event_loop);
                                    }
                                    self.request_redraw();
                                    return;
                                } else {
                                    self.git_panel.commit_input_focused = false;
                                }
                            } else if self.git_panel.commit_input_focused {
                                self.git_panel.commit_input_focused = false;
                            }

                            if self.search_panel.focused {
                                let in_input = self.renderer.as_ref().is_some_and(|r| {
                                    let sf = r.scale_factor as f32;
                                    let header_h = (40.0 * sf) as usize;
                                    let border_w = (1.0 * sf).max(1.0) as usize;
                                    let content_y = r.tab_bar_height as usize + header_h + border_w;
                                    let panel_w = self.panel_layout.left_physical_width(sf);
                                    crate::ui::search_panel::is_in_input(
                                        self.cursor_pos.0,
                                        self.cursor_pos.1,
                                        content_y,
                                        sf,
                                        panel_w,
                                    )
                                });
                                if !in_input {
                                    self.search_panel.focused = false;
                                }
                            }

                            if self.is_settings_active() {
                                self.handle_settings_click();
                            } else if self.is_models_active() {
                                self.handle_models_click();
                            } else if self.is_editor_active() {
                                if self.diff_split_hovered {
                                    self.diff_split_dragging = true;
                                    self.request_redraw();
                                } else if self.editor_scrollbar
                                    != crate::ui::components::editor_renderer::ScrollbarHit::None
                                {
                                    self.editor_scrollbar_dragging = self.editor_scrollbar;
                                    self.update_editor_scrollbar_drag(
                                        self.cursor_pos.0,
                                        self.cursor_pos.1,
                                    );
                                    self.request_redraw();
                                } else {
                                    self.handle_editor_click();
                                }
                            } else if !self.overlay.palette_open
                                && !self.overlay.model_picker_open
                                && !self.overlay.shell_picker_open
                            {
                                let grid_visible = self.is_grid_visible();
                                if grid_visible {
                                    if let Some((point, side)) = self
                                        .mouse_to_grid_point(self.cursor_pos.0, self.cursor_pos.1)
                                    {
                                        if self.modifiers.super_key()
                                            && let Some(terminal) = self.active_terminal()
                                        {
                                            let line_text = terminal.screen_row_text(point.line.0);
                                            if let Some(url) = crate::terminal::url_at_col(
                                                &line_text,
                                                point.column.0,
                                            ) {
                                                log::info!("Cmd+Click opening URL: {}", url);
                                                #[cfg(target_os = "macos")]
                                                {
                                                    let _ = std::process::Command::new("open")
                                                        .arg(&url)
                                                        .spawn();
                                                }
                                                #[cfg(not(target_os = "macos"))]
                                                {
                                                    let _ = std::process::Command::new("xdg-open")
                                                        .arg(&url)
                                                        .spawn();
                                                }
                                                return;
                                            }
                                        }

                                        let forward_mouse = !self.modifiers.alt_key()
                                            && self
                                                .active_terminal()
                                                .map(|t| t.has_mouse_mode())
                                                .unwrap_or(false);

                                        if forward_mouse {
                                            if let Some(terminal) = self.active_terminal() {
                                                let seq = sgr_mouse_press(
                                                    0,
                                                    point.column.0 as u32,
                                                    point.line.0 as u32,
                                                );
                                                terminal.input(std::borrow::Cow::Owned(
                                                    seq.into_bytes(),
                                                ));
                                            }
                                            self.mouse_forwarding = true;
                                            return;
                                        }

                                        if let Some(terminal) = self.active_terminal() {
                                            terminal.start_selection(point, side);
                                            self.selecting = true;
                                            if let Some(r) = self.renderer.as_mut() {
                                                r.invalidate_grid_cache();
                                            }
                                            self.request_redraw();
                                        }
                                    }
                                } else if let Some(link) = &self.hovered_link {
                                    let path = link.path.clone();
                                    let is_dir = std::path::Path::new(&path).is_dir();
                                    if is_dir {
                                        log::info!("cd into directory: {}", path);
                                        if let Some(tab) =
                                            self.tab_mgr.get_mut(self.tab_mgr.active_index())
                                            && let super::super::TabKind::Terminal {
                                                terminal,
                                                block_list,
                                                ..
                                            } = &mut tab.kind
                                        {
                                            block_list.capture_output(terminal);
                                            let prompt_info = terminal.prompt_info();
                                            let cmd_text = format!("cd {}", shell_escape(&path));
                                            block_list.push_command(prompt_info, cmd_text.clone());
                                            let cmd = format!("{}\n", cmd_text);
                                            terminal.input(Cow::Owned(cmd.into_bytes()));
                                            self.smart_input.pending_command = Some(cmd_text);
                                            self.smart_input.command_started = Some(Instant::now());
                                        }
                                    } else {
                                        log::info!("Opening file in editor: {}", path);
                                        self.open_file_in_editor(std::path::Path::new(&path));
                                    }
                                    self.request_redraw();
                                } else if let Some(pos) =
                                    self.mouse_to_block_pos(self.cursor_pos.0, self.cursor_pos.1)
                                {
                                    self.block_selection =
                                        Some(crate::blocks::BlockSelection::new(pos));
                                    self.selecting = true;
                                    self.request_redraw();
                                }
                            }
                        }
                    }
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self.scrollbar_dragging {
                    self.scrollbar_dragging = false;
                    self.request_redraw();
                }

                if self.editor_scrollbar_dragging
                    != crate::ui::components::editor_renderer::ScrollbarHit::None
                {
                    self.editor_scrollbar_dragging =
                        crate::ui::components::editor_renderer::ScrollbarHit::None;
                    self.request_redraw();
                }

                if self.diff_split_dragging {
                    self.diff_split_dragging = false;
                    self.request_redraw();
                }

                if self.file_tree.scrollbar_dragging {
                    self.file_tree.scrollbar_dragging = false;
                    self.request_redraw();
                }
                if self.side_panel.scrollbar_dragging {
                    self.side_panel.scrollbar_dragging = false;
                    self.request_redraw();
                }
                if self.git_panel.scrollbar_dragging {
                    self.git_panel.scrollbar_dragging = false;
                    self.request_redraw();
                }

                if self.search_panel.input_mouse_dragging {
                    self.search_panel.input_mouse_dragging = false;
                    if self.search_panel.selection_anchor == Some(self.search_panel.cursor) {
                        self.search_panel.selection_anchor = None;
                    }
                    self.request_redraw();
                }

                self.handle_settings_mouse_release();

                if self.panel_layout.left_resize.dragging {
                    self.panel_layout.end_resize();
                    self.sync_panel_insets();
                    self.request_redraw();
                }

                if self.panel_layout.right_resize.dragging {
                    self.panel_layout.end_right_resize();
                    self.sync_panel_insets();
                    self.request_redraw();
                }

                if self.drag.dragging.is_some() {
                    if let Some(renderer) = &self.renderer {
                        if let Some((from, to)) = self.drag.resolve_drop(
                            self.tab_mgr.len(),
                            renderer.scale_factor,
                            renderer.width as f64,
                            self.is_fullscreen,
                        ) {
                            self.tab_mgr.reorder(from, to);
                            if let Some(r) = self.renderer.as_mut() {
                                r.invalidate_grid_cache();
                            };
                        }
                    } else {
                        self.drag.reset();
                    }
                    self.request_redraw();
                }

                if self.path_drag.is_active() {
                    if let Some(path) = self.path_drag.take_drop_path() {
                        self.insert_dropped_path(&path);
                    }
                    self.request_redraw();
                } else if let Some(action) = self.path_drag.take_click_action() {
                    self.dispatch(action, event_loop);
                    self.request_redraw();
                }

                self.selecting = false;
                self.editor_selecting = false;
                if self.mouse_forwarding {
                    self.mouse_forwarding = false;
                    if let Some((point, _)) =
                        self.mouse_to_grid_point(self.cursor_pos.0, self.cursor_pos.1)
                        && let Some(terminal) = self.active_terminal()
                    {
                        let seq = sgr_mouse_release(0, point.column.0 as u32, point.line.0 as u32);
                        terminal.input(std::borrow::Cow::Owned(seq.into_bytes()));
                    }
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                self.context_menu = None;
                self.context_menu_target_tab = None;

                if let Some(renderer) = &self.renderer {
                    let hit = crate::ui::components::tab_bar::hit_test(
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                        self.tab_mgr.len(),
                        renderer.tab_bar_height as f64,
                        renderer.width as f64,
                        renderer.scale_factor,
                        self.is_fullscreen,
                        self.overlay.update_badge_w,
                    );
                    if let crate::ui::components::tab_bar::TabBarHit::Tab(idx) = hit {
                        self.open_tab_context_menu(
                            idx,
                            self.cursor_pos.0 as usize,
                            self.cursor_pos.1 as usize,
                        );
                        self.request_redraw();
                        return;
                    }
                }

                if self.overlay.sidebar_open
                    && self.panel_layout.active_tab == crate::ui::panel_layout::SidePanelTab::Files
                    && let Some(renderer) = &self.renderer
                {
                    let sf = renderer.scale_factor as f32;
                    let panel_w = self.panel_layout.left_physical_width(sf);
                    if (self.cursor_pos.0 as usize) < panel_w {
                        let header_h = 40.0 * sf;
                        let border_w = (1.0 * sf).max(1.0);
                        let content_y = renderer.tab_bar_height as f32 + header_h + border_w;
                        if self.cursor_pos.1 as f32 > content_y {
                            let hit_path = crate::ui::file_tree::hit_test(
                                self.cursor_pos.1,
                                content_y as usize,
                                self.file_tree.scroll_offset,
                                &self.file_tree,
                                renderer.scale_factor,
                            );
                            let target = hit_path.unwrap_or_else(|| {
                                self.file_tree
                                    .root
                                    .as_ref()
                                    .map(|r| r.path.clone())
                                    .unwrap_or_default()
                            });
                            if !target.as_os_str().is_empty() {
                                self.open_file_tree_context_menu(
                                    target,
                                    self.cursor_pos.0 as usize,
                                    self.cursor_pos.1 as usize,
                                );
                                self.request_redraw();
                                return;
                            }
                        }
                    }
                }

                if self.overlay.git_panel_open
                    && let Some(renderer) = &self.renderer
                {
                    let hit = crate::ui::components::git_panel::hit_test(
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                        &self.git_panel,
                        &self.panel_layout,
                        renderer.tab_bar_height as f64,
                        renderer.width as usize,
                        renderer.scale_factor,
                    );
                    let file_idx = match hit {
                        crate::ui::components::git_panel::GitPanelHit::SelectFile(i)
                        | crate::ui::components::git_panel::GitPanelHit::StageFile(i)
                        | crate::ui::components::git_panel::GitPanelHit::UnstageFile(i) => Some(i),
                        _ => None,
                    };
                    if let Some(idx) = file_idx
                        && let Some(entry) = self.git_panel.data.entries.get(idx)
                    {
                        self.open_git_file_context_menu(
                            entry.path.clone(),
                            self.cursor_pos.0 as usize,
                            self.cursor_pos.1 as usize,
                        );
                        self.request_redraw();
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = (position.x, position.y);

                if self.overlay.is_confirm_close_open() {
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    let (bw, bh) = self
                        .renderer
                        .as_ref()
                        .map(|r| (r.width as usize, r.height as usize))
                        .unwrap_or((0, 0));
                    let prev = self.overlay.confirm_close_hovered;
                    self.overlay.confirm_close_hovered =
                        crate::ui::components::overlay::confirm_close_hover_test(
                            position.x as usize,
                            position.y as usize,
                            bw,
                            bh,
                            sf,
                        );
                    if self.overlay.confirm_close_hovered != prev {
                        self.request_redraw();
                    }
                    if self.overlay.confirm_close_hovered.is_some() {
                        if let Some(w) = &self.window {
                            w.set_cursor(winit::window::CursorIcon::Pointer);
                        }
                    } else if let Some(w) = &self.window {
                        w.set_cursor(winit::window::CursorIcon::Default);
                    }
                    return;
                }

                if self.overlay.pro_panel_open {
                    let sf = self.scale_factor();
                    let (bw, bh) = self.buf_size();
                    let prev = self.overlay.pro_panel_hovered;
                    self.overlay.pro_panel_hovered =
                        crate::ui::components::overlay::pro_panel::pro_panel_hover_test(
                            bw,
                            bh,
                            self.license_mgr.is_pro(),
                            position.x,
                            position.y,
                            sf,
                        );
                    if self.overlay.pro_panel_hovered != prev {
                        self.request_redraw();
                    }
                    return;
                }

                if self.overlay.usage_panel_open {
                    let sf = self.scale_factor();
                    let (bw, bh) = self.buf_size();
                    let prev = self.overlay.pro_panel_hovered;
                    self.overlay.pro_panel_hovered =
                        crate::ui::components::overlay::usage_panel::usage_panel_hover_test(
                            bw,
                            bh,
                            &self.usage_tracker,
                            position.x,
                            position.y,
                            sf,
                        );
                    if self.overlay.pro_panel_hovered != prev {
                        self.request_redraw();
                    }
                    return;
                }

                let hover_update = if let Some(menu) = &self.context_menu {
                    let (bw, bh, sf) = self
                        .renderer
                        .as_ref()
                        .map(|r| (r.width as usize, r.height as usize, r.scale_factor as f32))
                        .unwrap_or((0, 0, 1.0));
                    let prev = menu.hovered;
                    let new_hover = crate::ui::components::context_menu::context_menu_hover_test(
                        menu,
                        position.x as usize,
                        position.y as usize,
                        bw,
                        bh,
                        sf,
                    );
                    Some((prev, new_hover))
                } else {
                    None
                };
                if let Some((prev, new_hover)) = hover_update
                    && let Some(menu) = &mut self.context_menu
                {
                    menu.hovered = new_hover;
                    if menu.hovered != prev {
                        self.request_redraw();
                    }
                }

                if self.search_panel.input_mouse_dragging {
                    if let Some(r) = self.renderer.as_mut() {
                        let sf = r.scale_factor as f32;
                        let pos =
                            self.search_panel
                                .click_to_cursor(position.x, &mut r.font_system, sf);
                        self.search_panel.cursor = pos;
                    }
                    self.request_redraw();
                }

                if self.mouse_forwarding
                    && let Some((point, _)) = self.mouse_to_grid_point(position.x, position.y)
                    && let Some(terminal) = self.active_terminal()
                {
                    let seq = sgr_mouse_move(point.column.0 as u32, point.line.0 as u32);
                    terminal.input(std::borrow::Cow::Owned(seq.into_bytes()));
                }

                if self.selecting {
                    if self.block_selection.is_some() {
                        if let Some(pos) = self.mouse_to_block_pos(position.x, position.y) {
                            if let Some(sel) = &mut self.block_selection {
                                sel.head = pos;
                            }
                            self.request_redraw();
                        }
                    } else if let Some((point, side)) =
                        self.mouse_to_grid_point(position.x, position.y)
                        && let Some(terminal) = self.active_terminal()
                    {
                        terminal.update_selection(point, side);
                        if let Some(r) = self.renderer.as_mut() {
                            r.invalidate_grid_cache();
                        }
                        self.request_redraw();
                    }
                }

                if self.editor_selecting
                    && let Some(renderer) = &self.renderer
                {
                    let sf = renderer.scale_factor as f32;
                    let bar_h = renderer.tab_bar_height as usize;
                    let x_off = self.side_panel_x_offset();
                    let content_h = (renderer.height as usize).saturating_sub(bar_h);

                    if let Some(state) = self.active_editor_state()
                        && let Some((line, col)) =
                            crate::ui::components::editor_renderer::hit_test_cursor(
                                state,
                                position.x as usize,
                                position.y as usize,
                                x_off,
                                bar_h,
                                sf,
                            )
                    {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.set_cursor_pos_selecting(line, col);
                            ed.ensure_cursor_visible(sf, content_h);
                        }
                        self.request_redraw();
                    }
                }

                if self.drag.dragging.is_some() {
                    self.drag.update(position.x);
                    self.request_redraw();
                }

                if self.path_drag.update(position.x, position.y) {
                    self.request_redraw();
                }

                if self.panel_layout.left_resize.dragging {
                    if let Some(renderer) = &self.renderer {
                        let sf = renderer.scale_factor as f32;
                        let new_w = position.x as f32 / sf;
                        self.panel_layout.set_left_width(new_w);
                    }
                    self.sync_panel_insets();
                    self.request_redraw();
                }

                if self.panel_layout.right_resize.dragging {
                    if let Some(renderer) = &self.renderer {
                        let sf = renderer.scale_factor as f32;
                        let new_w = (renderer.width as f32 - position.x as f32) / sf;
                        self.panel_layout.set_right_width(new_w);
                    }
                    self.sync_panel_insets();
                    self.request_redraw();
                }

                if self.diff_split_dragging {
                    if let Some(renderer) = &self.renderer {
                        let sf = renderer.scale_factor as f32;
                        let x_off = self.side_panel_x_offset();
                        let git_w = if self.overlay.git_panel_open {
                            self.panel_layout.right_physical_width(sf)
                        } else {
                            0
                        };
                        let content_w = (renderer.width as usize)
                            .saturating_sub(x_off)
                            .saturating_sub(git_w);
                        let center_div_w = (1.0_f32 * sf).max(1.0) as usize;
                        let usable = content_w.saturating_sub(center_div_w);
                        if usable > 0 {
                            let rel = (position.x as usize).saturating_sub(x_off);
                            let frac = (rel as f32 / usable as f32).clamp(0.15, 0.85);
                            if let Some(ed) = self.active_editor_state_mut() {
                                ed.diff_split_frac = frac;
                            }
                        }
                    }
                    self.request_redraw();
                }

                if self.overlay.sidebar_open {
                    if let Some(renderer) = &self.renderer {
                        let prev = self.panel_layout.left_resize.hovered;
                        self.panel_layout.left_resize.hovered = self
                            .panel_layout
                            .is_in_resize_zone(position.x, renderer.scale_factor);
                        if self.panel_layout.left_resize.hovered != prev {
                            self.request_redraw();
                        }
                    }
                } else {
                    self.panel_layout.left_resize.hovered = false;
                }

                if self.overlay.git_panel_open {
                    if let Some(renderer) = &self.renderer {
                        let prev = self.panel_layout.right_resize.hovered;
                        self.panel_layout.right_resize.hovered =
                            self.panel_layout.is_in_right_resize_zone(
                                position.x,
                                renderer.scale_factor,
                                renderer.width as usize,
                            );
                        if self.panel_layout.right_resize.hovered != prev {
                            self.request_redraw();
                        }
                    }
                } else {
                    self.panel_layout.right_resize.hovered = false;
                }

                let prev_close = self.overlay.hovered_close;
                let prev_avatar = self.overlay.avatar_hovered;
                let prev_new_tab = self.overlay.new_tab_hovered;
                let prev_shell_btn = self.overlay.shell_picker_btn_hovered;
                let prev_sidebar = self.overlay.sidebar_hovered;
                let prev_git_panel = self.overlay.git_panel_hovered;
                let prev_user_menu = self.overlay.user_menu_hovered;
                let prev_update_badge = self.overlay.update_badge_hovered;
                let prev_update_dd = self.overlay.update_dropdown_hovered;
                let panel_changed = self.update_hover();

                if let Some(renderer) = &self.renderer {
                    let bar_h = renderer.tab_bar_height as f64;
                    let buf_w = renderer.width as f64;
                    let sf = renderer.scale_factor;

                    if let Some(bw) = self.overlay.update_badge_w {
                        self.overlay.update_badge_hovered =
                            crate::ui::components::tab_bar::is_update_badge_hovered(
                                self.cursor_pos.0,
                                self.cursor_pos.1,
                                bar_h,
                                buf_w,
                                sf,
                                bw,
                            );
                        if self.overlay.update_dropdown_open {
                            self.overlay.update_dropdown_hovered =
                                crate::ui::components::tab_bar::update_dropdown_hit_test(
                                    self.cursor_pos.0,
                                    self.cursor_pos.1,
                                    bar_h,
                                    buf_w,
                                    sf,
                                    bw,
                                );
                        } else {
                            self.overlay.update_dropdown_hovered = None;
                        }
                    } else {
                        self.overlay.update_badge_hovered = false;
                        self.overlay.update_dropdown_hovered = None;
                    }
                }

                if panel_changed
                    || self.overlay.hovered_close != prev_close
                    || self.overlay.avatar_hovered != prev_avatar
                    || self.overlay.new_tab_hovered != prev_new_tab
                    || self.overlay.shell_picker_btn_hovered != prev_shell_btn
                    || self.overlay.sidebar_hovered != prev_sidebar
                    || self.overlay.git_panel_hovered != prev_git_panel
                    || self.overlay.user_menu_hovered != prev_user_menu
                    || self.overlay.update_badge_hovered != prev_update_badge
                    || self.overlay.update_dropdown_hovered != prev_update_dd
                {
                    self.request_redraw();
                }

                if self.file_tree.scrollbar_dragging
                    && let Some(renderer) = &self.renderer
                {
                    let sf = renderer.scale_factor as f32;
                    let header_h = (40.0 * sf) as usize;
                    let border_w = (1.0 * sf).max(1.0) as usize;
                    let content_y = renderer.tab_bar_height as usize + header_h + border_w;
                    let visible_h = (renderer.height as usize).saturating_sub(content_y);
                    let item_h = (crate::ui::file_tree::ITEM_HEIGHT_PX * sf) as usize;
                    let pad_y = (crate::ui::file_tree::PAD_Y_PX * sf) as usize;
                    let total_h = self.file_tree.row_count() * item_h + pad_y * 2;
                    let drag_dy = position.y - self.file_tree.scrollbar_drag_start_y;
                    self.file_tree.scroll_offset =
                        crate::ui::components::side_panel::panel_scrollbar_drag_to_scroll(
                            drag_dy,
                            self.file_tree.scrollbar_drag_start_scroll,
                            visible_h,
                            total_h,
                            sf,
                        );
                    self.request_redraw();
                }

                if self.git_panel.scrollbar_dragging
                    && let Some(renderer) = &self.renderer
                {
                    let sf = renderer.scale_factor as f32;
                    let header_h = (40.0 * sf) as usize;
                    let border_w = (1.0 * sf).max(1.0) as usize;
                    let content_y = renderer.tab_bar_height as usize + header_h + border_w;
                    let visible_h = (renderer.height as usize).saturating_sub(content_y);
                    let total_h = crate::ui::components::git_panel::content_height(
                        &self.git_panel,
                        self.panel_layout.git_tab,
                        sf,
                    ) as usize;
                    let drag_dy = position.y - self.git_panel.scrollbar_drag_start_y;
                    self.git_panel.scroll_offset =
                        crate::ui::components::side_panel::panel_scrollbar_drag_to_scroll(
                            drag_dy,
                            self.git_panel.scrollbar_drag_start_scroll,
                            visible_h,
                            total_h,
                            sf,
                        );
                    self.request_redraw();
                }

                self.update_settings_hover();
                self.handle_settings_mouse_move();

                self.update_models_hover();

                if self.hint_banner.is_visible() {
                    let prev = self.hint_banner.dismiss_hovered;
                    if let Some(renderer) = &self.renderer {
                        let sf = renderer.scale_factor as f32;
                        let prompt_h = crate::ui::components::prompt_bar::prompt_bar_height(sf);
                        let banner_h = crate::ui::components::hint_banner::banner_height(
                            &self.hint_banner,
                            sf,
                        );
                        if banner_h > 0 {
                            let banner_y =
                                (renderer.height as usize).saturating_sub(prompt_h + banner_h);
                            let git_w = if self.overlay.git_panel_open {
                                self.panel_layout.right_physical_width(sf)
                            } else {
                                0
                            };
                            let banner_w = (renderer.width as usize).saturating_sub(git_w);
                            self.hint_banner.dismiss_hovered =
                                crate::ui::components::hint_banner::hit_test_dismiss(
                                    position.x,
                                    position.y,
                                    &self.hint_banner,
                                    banner_y,
                                    banner_w,
                                    sf,
                                );
                        } else {
                            self.hint_banner.dismiss_hovered = false;
                        }
                    }
                    if self.hint_banner.dismiss_hovered != prev {
                        self.request_redraw();
                    }
                }

                if self.usage_limit_banner.is_visible() {
                    let prev = self.usage_limit_banner.hovered;
                    if let Some(renderer) = &self.renderer {
                        let sf = renderer.scale_factor as f32;
                        let bw = renderer.width as usize;
                        let bh = renderer.height as usize;
                        self.usage_limit_banner.hovered =
                            crate::ui::components::usage_limit_banner::hover_test(
                                &self.usage_limit_banner,
                                position.x,
                                position.y,
                                bw,
                                bh,
                                sf,
                            );
                    }
                    if self.usage_limit_banner.hovered != prev {
                        self.request_redraw();
                    }
                }

                if self.overlay.shell_picker_open {
                    let (ax, ay) = self.shell_picker_anchor();
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    let picker_state = self.build_shell_picker_state();
                    self.overlay.shell_picker_hovered =
                        crate::ui::components::overlay::shell_picker_hover_test(
                            self.cursor_pos.0,
                            self.cursor_pos.1,
                            ax,
                            ay,
                            &picker_state,
                            sf,
                        );
                    self.request_redraw();
                }

                {
                    let prev = self.overlay.cwd_badge_hovered;
                    self.overlay.cwd_badge_hovered =
                        if let Some((rx, ry, rw, rh)) = self.overlay.cwd_badge_rect {
                            let cx = self.cursor_pos.0 as usize;
                            let cy = self.cursor_pos.1 as usize;
                            cx >= rx && cx < rx + rw && cy >= ry && cy < ry + rh
                        } else {
                            false
                        };
                    if self.overlay.cwd_badge_hovered != prev {
                        self.request_redraw();
                    }
                }

                if self.overlay.cwd_dropdown_open {
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    let prev = self.overlay.cwd_dropdown_hovered;
                    self.overlay.cwd_dropdown_hovered =
                        if let Some((bx, by, _, _)) = self.overlay.cwd_badge_rect {
                            crate::ui::components::prompt_bar::cwd_dropdown::hover_test(
                                self.cursor_pos.0,
                                self.cursor_pos.1,
                                bx,
                                by,
                                self.overlay.cwd_dropdown_entries.len(),
                                self.overlay.cwd_dropdown_scroll,
                                sf,
                            )
                        } else {
                            None
                        };
                    if self.overlay.cwd_dropdown_hovered != prev {
                        self.request_redraw();
                    }
                }

                {
                    let prev_link = self.hovered_link.take();
                    let is_smart = self.settings_state.input_type == InputType::Smart;
                    let is_app = self
                        .active_terminal()
                        .map(|t| t.is_app_controlled())
                        .unwrap_or(true);
                    if is_smart
                        && !is_app
                        && !self.selecting
                        && let Some(pos) = self.mouse_to_block_pos(position.x, position.y)
                        && let Some(bl) = self.active_block_list()
                        && let Some(block) = bl.blocks.get(pos.block_idx)
                    {
                        let max_chars = self.block_max_chars();
                        if let Some(line_text) =
                            crate::ui::components::block_renderer::visual_line_text(
                                &block.output,
                                max_chars,
                                pos.line_idx,
                            )
                            && let Some((cs, ce, token)) =
                                crate::blocks::path_token_at(&line_text, pos.char_idx)
                        {
                            let cwd = block
                                .prompt
                                .segments
                                .first()
                                .filter(|seg| seg.kind == crate::prompt::SegmentKind::Cwd)
                                .map(|seg| seg.text.as_str())
                                .unwrap_or("~");
                            let cwd_expanded = if cwd.starts_with('~') {
                                dirs::home_dir()
                                    .map(|h| {
                                        let rest = cwd.strip_prefix('~').unwrap_or(cwd);
                                        let rest = rest.strip_prefix('/').unwrap_or(rest);
                                        if rest.is_empty() {
                                            h.to_string_lossy().to_string()
                                        } else {
                                            h.join(rest).to_string_lossy().to_string()
                                        }
                                    })
                                    .unwrap_or_else(|| cwd.to_string())
                            } else {
                                cwd.to_string()
                            };
                            if let Some(resolved) =
                                crate::blocks::resolve_path(&token, &cwd_expanded)
                            {
                                self.hovered_link = Some(crate::blocks::HoveredLink {
                                    block_idx: pos.block_idx,
                                    visual_line_idx: pos.line_idx,
                                    char_start: cs,
                                    char_end: ce,
                                    path: resolved,
                                });
                            }
                        }
                    }
                    if self.hovered_link != prev_link {
                        self.request_redraw();
                    }
                }

                if self.scrollbar_dragging {
                    self.update_scrollbar_drag(position.y);
                    self.request_redraw();
                } else {
                    let prev_hovered = self.scrollbar_hovered;
                    self.scrollbar_hovered = self.is_over_scrollbar(position.x, position.y);
                    if self.scrollbar_hovered != prev_hovered {
                        self.request_redraw();
                    }
                }

                if self.editor_scrollbar_dragging
                    != crate::ui::components::editor_renderer::ScrollbarHit::None
                {
                    self.update_editor_scrollbar_drag(position.x, position.y);
                    self.request_redraw();
                } else if self.is_editor_active() {
                    let prev = self.editor_scrollbar;
                    self.editor_scrollbar = self.editor_scrollbar_hit_test(position.x, position.y);
                    if self.editor_scrollbar != prev {
                        self.request_redraw();
                    }

                    let prev_split = self.diff_split_hovered;
                    self.diff_split_hovered = self.diff_divider_hit_test(position.x, position.y);
                    if self.diff_split_hovered != prev_split {
                        self.request_redraw();
                    }
                } else if self.editor_scrollbar
                    != crate::ui::components::editor_renderer::ScrollbarHit::None
                {
                    self.editor_scrollbar =
                        crate::ui::components::editor_renderer::ScrollbarHit::None;
                    self.request_redraw();
                }

                if let Some(window) = &self.window {
                    let tab_bar_interactive = self.overlay.hovered_close.is_some()
                        || self.overlay.avatar_hovered
                        || self.overlay.new_tab_hovered
                        || self.overlay.shell_picker_btn_hovered
                        || self.overlay.sidebar_hovered
                        || self.overlay.update_badge_hovered
                        || self.overlay.update_dropdown_hovered.is_some()
                        || (self.overlay.user_menu_open
                            && self.overlay.user_menu_hovered.is_some());
                    let side_panel_interactive = self.overlay.sidebar_open
                        && (self.side_panel.hovered_item.is_some()
                            || self.side_panel.hovered_toolbar_btn.is_some()
                            || self.file_tree.hovered_idx.is_some()
                            || self.search_panel.hovered_idx.is_some()
                            || self.side_panel.sandbox.stop_hovered);
                    let panel_resize_active =
                        self.overlay.sidebar_open && self.panel_layout.left_resize.hovered;
                    let models_interactive = self.models_view.hovered_action.is_some()
                        || self.models_view.hovered_delete.is_some();
                    let link_interactive = self.hovered_link.is_some();
                    let settings_interactive = self.is_settings_active()
                        && (self.settings_state.hovered.is_some()
                            || self.settings_state.hovered_btn.is_some()
                            || self.settings_state.font_picker_hovered.is_some()
                            || self.settings_state.sandbox.hovered_hit.is_some()
                            || self.settings_state.about_hovered.is_some());

                    let banner_interactive = self.hint_banner.dismiss_hovered
                        || self.usage_limit_banner.hovered.is_some();
                    let git_panel_interactive = self.overlay.git_panel_open
                        && crate::ui::components::git_panel::wants_pointer(&self.git_panel);
                    let right_resize_active =
                        self.overlay.git_panel_open && self.panel_layout.right_resize.hovered;

                    let shell_picker_interactive = self.overlay.shell_picker_open
                        && self.overlay.shell_picker_hovered.is_some();

                    let cwd_interactive = self.overlay.cwd_badge_hovered
                        || (self.overlay.cwd_dropdown_open
                            && self.overlay.cwd_dropdown_hovered.is_some());

                    let editor_text_area = self.is_editor_active()
                        && !panel_resize_active
                        && !right_resize_active
                        && !tab_bar_interactive
                        && !git_panel_interactive
                        && !self.usage_limit_banner.is_visible()
                        && !self.is_settings_active()
                        && !self.diff_split_hovered
                        && !self.diff_split_dragging
                        && self.renderer.as_ref().is_some_and(|r| {
                            let below_bar = self.cursor_pos.1 > r.tab_bar_height as f64;
                            let sf = r.scale_factor as f32;
                            let left_edge = if self.overlay.sidebar_open {
                                self.panel_layout.left_physical_width(sf) as f64
                            } else {
                                0.0
                            };
                            let git_w = if self.overlay.git_panel_open {
                                self.panel_layout.right_physical_width(sf) as f64
                            } else {
                                0.0
                            };
                            let right_edge = r.width as f64 - git_w;
                            below_bar
                                && self.cursor_pos.0 >= left_edge
                                && self.cursor_pos.0 < right_edge
                        });

                    if self.path_drag.is_active() {
                        window.set_cursor(winit::window::CursorIcon::Grabbing);
                    } else if panel_resize_active
                        || right_resize_active
                        || self.diff_split_hovered
                        || self.diff_split_dragging
                    {
                        window.set_cursor(winit::window::CursorIcon::ColResize);
                    } else if editor_text_area {
                        window.set_cursor(winit::window::CursorIcon::Text);
                    } else if tab_bar_interactive
                        || side_panel_interactive
                        || models_interactive
                        || link_interactive
                        || settings_interactive
                        || banner_interactive
                        || git_panel_interactive
                        || shell_picker_interactive
                        || cwd_interactive
                    {
                        window.set_cursor(winit::window::CursorIcon::Pointer);
                    } else {
                        window.set_cursor(winit::window::CursorIcon::Default);
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if self.is_models_active() {
                    let dy = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 40.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    };
                    self.models_view.scroll_offset = (self.models_view.scroll_offset - dy).max(0.0);
                    if let Some(renderer) = &self.renderer {
                        let sf = renderer.scale_factor as f32;
                        let content_h =
                            renderer.height.saturating_sub(renderer.tab_bar_height) as usize;
                        let max = crate::ui::components::overlay::models_view::max_scroll(
                            &self.models_view,
                            sf,
                            content_h,
                            &self.settings_state.models_path,
                        );
                        self.models_view.scroll_offset = self.models_view.scroll_offset.min(max);
                    }
                    self.request_redraw();
                    return;
                }

                if self.is_settings_active()
                    && self.settings_state.active
                        == crate::ui::components::overlay::SettingsCategory::Sandbox
                {
                    let dy = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 40.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    };
                    self.settings_state.sandbox.scroll_offset =
                        (self.settings_state.sandbox.scroll_offset - dy).max(0.0);
                    if let Some(renderer) = &self.renderer {
                        let sf = renderer.scale_factor as f32;
                        let bar_h = renderer.tab_bar_height as usize;
                        let bh = renderer.height as usize;
                        let x_offset = self.side_panel_x_offset();
                        let git_w = if self.overlay.git_panel_open {
                            self.panel_layout.right_physical_width(sf)
                        } else {
                            0
                        };
                        let area_y = bar_h;
                        let area_h = bh.saturating_sub(bar_h);
                        let _ = (x_offset, git_w);
                        let body_y = area_y + (16.0 * sf) as usize;
                        let viewport_h = (area_y + area_h).saturating_sub(body_y) as f32;
                        let total = crate::ui::components::overlay::settings::sandbox_settings::sandbox_settings_content_height(sf);
                        let max = (total - viewport_h).max(0.0);
                        self.settings_state.sandbox.scroll_offset =
                            self.settings_state.sandbox.scroll_offset.min(max);
                    }
                    self.request_redraw();
                    return;
                }

                if self.overlay.sidebar_open
                    && let Some(renderer) = &self.renderer
                {
                    let panel_w = self
                        .panel_layout
                        .left_physical_width(renderer.scale_factor as f32);
                    if self.cursor_pos.0 < panel_w as f64
                        && self.cursor_pos.1 > renderer.tab_bar_height as f64
                    {
                        let dy = match delta {
                            winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 40.0,
                            winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                        };
                        let sf = renderer.scale_factor as f32;
                        let header_h = (40.0 * sf) as usize;
                        let border_w = (1.0 * sf).max(1.0) as usize;
                        let content_y = renderer.tab_bar_height as usize + header_h + border_w;
                        let visible_h = (renderer.height as usize).saturating_sub(content_y);

                        match self.panel_layout.active_tab {
                            crate::ui::panel_layout::SidePanelTab::Sessions => {
                                let item_h = (56.0 * sf) as usize;
                                let total_h = self.session_mgr.count() * item_h;
                                let max_scroll = total_h.saturating_sub(visible_h);
                                self.side_panel.scroll_offset = (self.side_panel.scroll_offset
                                    - dy)
                                    .max(0.0)
                                    .min(max_scroll as f32);
                            }
                            crate::ui::panel_layout::SidePanelTab::Files => {
                                let item_h = (crate::ui::file_tree::ITEM_HEIGHT_PX * sf) as usize;
                                let pad_y = (crate::ui::file_tree::PAD_Y_PX * sf) as usize;
                                let total_h = self.file_tree.row_count() * item_h + pad_y * 2;
                                let max_scroll = total_h.saturating_sub(visible_h);
                                self.file_tree.scroll_offset = (self.file_tree.scroll_offset - dy)
                                    .max(0.0)
                                    .min(max_scroll as f32);
                            }
                            crate::ui::panel_layout::SidePanelTab::Sandbox => {}
                            crate::ui::panel_layout::SidePanelTab::Search => {
                                let row_h = (24.0 * sf) as usize;
                                let total_h = self.search_panel.flat_row_count() * row_h;
                                let max_scroll = total_h.saturating_sub(visible_h);
                                self.search_panel.scroll_offset = (self.search_panel.scroll_offset
                                    - dy)
                                    .max(0.0)
                                    .min(max_scroll as f32);
                            }
                        }
                        self.request_redraw();
                        return;
                    }
                }

                if self.overlay.git_panel_open
                    && let Some(renderer) = &self.renderer
                {
                    let sf = renderer.scale_factor as f32;
                    let panel_w = self.panel_layout.right_physical_width(sf);
                    let panel_x = renderer.width as f64 - panel_w as f64;
                    let bar_h = renderer.tab_bar_height as f64;
                    if self.cursor_pos.0 >= panel_x && self.cursor_pos.1 > bar_h {
                        let dy = match delta {
                            winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 40.0,
                            winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                        };
                        let header_h = (40.0 * sf) as usize;
                        let border_w = (1.0 * sf).max(1.0) as usize;
                        let content_y = renderer.tab_bar_height as usize + header_h + border_w;
                        let visible_h = (renderer.height as usize).saturating_sub(content_y) as f32;
                        let max = crate::ui::components::git_panel::max_scroll(
                            &self.git_panel,
                            self.panel_layout.git_tab,
                            visible_h,
                            sf,
                        );
                        self.git_panel.scroll_offset =
                            (self.git_panel.scroll_offset - dy).max(0.0).min(max);
                        crate::ui::components::git_panel::update_hover(
                            &mut self.git_panel,
                            &self.panel_layout,
                            self.cursor_pos.0,
                            self.cursor_pos.1,
                            renderer.tab_bar_height as f64,
                            renderer.width as usize,
                            renderer.scale_factor,
                        );
                        self.request_redraw();
                        return;
                    }
                }

                if self.is_editor_active() {
                    let shift = self.modifiers.shift_key();
                    let dy = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 40.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    };
                    let dx = match delta {
                        winit::event::MouseScrollDelta::LineDelta(x, _) => *x * 40.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.x as f32,
                    };

                    if shift || dx.abs() > dy.abs().max(0.1) {
                        let scroll_amount = if shift { -dy } else { -dx };
                        if let Some(renderer) = &self.renderer {
                            let sf = renderer.scale_factor as f32;
                            if let Some(ed) = self.active_editor_state() {
                                let max_x =
                                    crate::ui::components::editor_renderer::max_scroll_x(ed, sf);
                                if let Some(ed) = self.active_editor_state_mut() {
                                    let new_x = (ed.scroll_x() + scroll_amount).clamp(0.0, max_x);
                                    ed.set_scroll_x(new_x);
                                }
                            }
                        } else if let Some(ed) = self.active_editor_state_mut() {
                            let new_x = (ed.scroll_x() + scroll_amount).max(0.0);
                            ed.set_scroll_x(new_x);
                        }
                    } else {
                        if let Some(ed) = self.active_editor_state_mut() {
                            ed.scroll_offset = (ed.scroll_offset - dy).max(0.0);
                        }
                        if let Some(renderer) = &self.renderer {
                            let sf = renderer.scale_factor as f32;
                            let content_h = (renderer.height as usize)
                                .saturating_sub(renderer.tab_bar_height as usize);
                            if let Some(ed) = self.active_editor_state() {
                                let total_h =
                                    crate::ui::components::editor_renderer::content_height_px(
                                        ed, sf,
                                    );
                                let max_scroll = total_h.saturating_sub(content_h);
                                if let Some(ed) = self.active_editor_state_mut() {
                                    ed.scroll_offset = ed.scroll_offset.min(max_scroll as f32);
                                }
                            }
                        }
                    }
                    self.request_redraw();
                    return;
                }

                let input_type = self.settings_state.input_type;
                let is_smart =
                    input_type == crate::ui::components::overlay::settings::InputType::Smart;
                let is_app = self
                    .active_terminal()
                    .map(|t| t.is_app_controlled())
                    .unwrap_or(false);

                let has_blocks = self
                    .active_block_list()
                    .map(|bl| bl.len() > 0)
                    .unwrap_or(false);
                if is_smart && !is_app && has_blocks {
                    let dy = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 40.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    };
                    if let Some(bl) = self.active_block_list_mut() {
                        bl.scroll_offset = (bl.scroll_offset + dy).max(0.0);
                    }

                    if let Some(renderer) = &self.renderer {
                        let sf = renderer.scale_factor as f32;
                        let bar_h = renderer.tab_bar_height as usize;
                        let prompt_h = crate::ui::components::prompt_bar::prompt_bar_height(sf);
                        let banner_h = self.total_banners_height(sf);
                        let available_h =
                            (renderer.height as usize).saturating_sub(bar_h + prompt_h + banner_h);
                        let pad = renderer.grid_padding_for(false, false) as usize;
                        let block_pad_inner = (10.0 * sf) as usize;
                        let content_w =
                            (renderer.width as usize).saturating_sub((pad + block_pad_inner) * 2);
                        let char_w_px = renderer.block_char_width as usize;
                        let max_chars = if char_w_px > 0 {
                            content_w / char_w_px
                        } else {
                            0
                        };
                        if let Some(bl) = self.active_block_list() {
                            let total_h = crate::ui::components::block_renderer::total_height(
                                bl, sf, max_chars,
                            ) as usize;
                            let max_scroll = total_h.saturating_sub(available_h);
                            if let Some(bl) = self.active_block_list_mut() {
                                bl.scroll_offset = bl.scroll_offset.min(max_scroll as f32);
                            }
                        }
                    }
                    self.request_redraw();
                } else if let Some(terminal) = self.active_terminal() {
                    let lines = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y as i32,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0) as i32,
                    };
                    if terminal.has_mouse_mode() {
                        let (col, row) = self
                            .mouse_to_grid_point(self.cursor_pos.0, self.cursor_pos.1)
                            .map(|(p, _)| (p.column.0 as u32, p.line.0 as u32))
                            .unwrap_or((0, 0));
                        let up = lines > 0;
                        for _ in 0..lines.abs() {
                            let seq = sgr_mouse_scroll(up, col, row);
                            terminal.input(Cow::Owned(seq.into_bytes()));
                        }
                    } else if lines > 0 {
                        for _ in 0..lines.abs() {
                            terminal.input(Cow::Borrowed(b"\x1b[A"));
                        }
                    } else if lines < 0 {
                        for _ in 0..lines.abs() {
                            terminal.input(Cow::Borrowed(b"\x1b[B"));
                        }
                    }
                }
            }

            _ => {}
        }
    }

    /// Convert mouse pixel position to a terminal grid Point + Side.
    /// Returns None if the position is outside the grid area.
    fn mouse_to_grid_point(
        &self,
        mx: f64,
        my: f64,
    ) -> Option<(
        alacritty_terminal::index::Point,
        alacritty_terminal::index::Side,
    )> {
        let renderer = self.renderer.as_ref()?;
        let terminal = self.active_terminal()?;
        let is_app = terminal.is_app_controlled();
        let pad = renderer.grid_padding_for(is_app, false) as f64;
        let bar_h = renderer.tab_bar_height as f64;

        let grid_x = mx - pad;
        let grid_y = my - bar_h;
        if grid_x < 0.0 || grid_y < 0.0 {
            return None;
        }

        let cw = renderer.cell_width as f64;
        let ch = renderer.cell_height as f64;
        if cw <= 0.0 || ch <= 0.0 {
            return None;
        }

        let col = (grid_x / cw) as usize;
        let row = (grid_y / ch) as i32;

        let term = terminal.term.lock();
        let cols = term.grid().columns();
        let rows = term.grid().screen_lines();

        let col = col.min(cols.saturating_sub(1));
        let row = row.min(rows as i32 - 1).max(0);

        let side = if (grid_x / cw).fract() < 0.5 {
            alacritty_terminal::index::Side::Left
        } else {
            alacritty_terminal::index::Side::Right
        };

        let point = alacritty_terminal::index::Point::new(
            alacritty_terminal::index::Line(row),
            alacritty_terminal::index::Column(col),
        );
        Some((point, side))
    }

    /// Whether the raw terminal grid is currently visible (as opposed to block view).
    fn is_grid_visible(&self) -> bool {
        let terminal = match self.active_terminal() {
            Some(t) => t,
            None => return false,
        };
        let is_app = terminal.is_app_controlled();
        if is_app {
            return true;
        }
        if self.settings_state.input_type != InputType::Smart {
            return true;
        }
        self.active_block_list()
            .map(|bl| bl.blocks.is_empty())
            .unwrap_or(true)
    }

    /// Query: map the current cursor position to a side-panel [`AppAction`].
    ///
    /// Returns `None` if the click didn't land on an actionable item.
    /// This is pure read, no state mutation.
    fn side_panel_action(&self) -> Option<crate::app::actions::AppAction> {
        use crate::app::actions::AppAction;
        use crate::ui::components::side_panel::{self, SidePanelHit};

        let renderer = self.renderer.as_ref()?;
        let session_count = self.session_mgr.count();
        let sandbox_info = self.build_sandbox_info();
        let hit = side_panel::hit_test(
            self.cursor_pos.0,
            self.cursor_pos.1,
            session_count,
            renderer.tab_bar_height as f64,
            renderer.scale_factor,
            self.side_panel.scroll_offset,
            &self.panel_layout,
            sandbox_info.as_ref(),
        );
        match hit {
            SidePanelHit::OpenSession(visual_idx) => {
                let session_id = self.resolve_session_index(visual_idx)?;
                Some(AppAction::OpenSession { session_id })
            }
            SidePanelHit::ClearSession(visual_idx) => {
                let session_id = self.resolve_session_index(visual_idx)?;
                Some(AppAction::ClearSession { session_id })
            }
            SidePanelHit::ToolbarSessions => Some(AppAction::SwitchPanelTab {
                tab: crate::ui::panel_layout::SidePanelTab::Sessions,
            }),
            SidePanelHit::ToolbarFiles => Some(AppAction::SwitchPanelTab {
                tab: crate::ui::panel_layout::SidePanelTab::Files,
            }),
            SidePanelHit::ToolbarSandbox => Some(AppAction::SwitchPanelTab {
                tab: crate::ui::panel_layout::SidePanelTab::Sandbox,
            }),
            SidePanelHit::ToolbarSearch => Some(AppAction::SwitchPanelTab {
                tab: crate::ui::panel_layout::SidePanelTab::Search,
            }),
            SidePanelHit::StopSandbox => Some(AppAction::StopSandbox),
            SidePanelHit::None => {
                if self.panel_layout.active_tab == crate::ui::panel_layout::SidePanelTab::Files {
                    let panel_w = self
                        .panel_layout
                        .left_physical_width(renderer.scale_factor as f32);
                    if self.cursor_pos.0 < panel_w as f64 {
                        let header_h = 40.0 * renderer.scale_factor;
                        let border_w = (1.0 * renderer.scale_factor).max(1.0);
                        let content_y = renderer.tab_bar_height as f64 + header_h + border_w;
                        if self.cursor_pos.1 > content_y
                            && let Some(path) = crate::ui::file_tree::hit_test(
                                self.cursor_pos.1,
                                (renderer.tab_bar_height as f64 + header_h + border_w) as usize,
                                self.file_tree.scroll_offset,
                                &self.file_tree,
                                renderer.scale_factor,
                            )
                        {
                            if path.is_dir() {
                                return Some(AppAction::ToggleFileTreeNode { path });
                            } else {
                                return Some(AppAction::OpenFile { path });
                            }
                        }
                    }
                }
                if self.panel_layout.active_tab == crate::ui::panel_layout::SidePanelTab::Search {
                    let panel_w = self
                        .panel_layout
                        .left_physical_width(renderer.scale_factor as f32);
                    if self.cursor_pos.0 < panel_w as f64 {
                        let header_h = 40.0 * renderer.scale_factor;
                        let border_w = (1.0 * renderer.scale_factor).max(1.0);
                        let content_y =
                            (renderer.tab_bar_height as f64 + header_h + border_w) as usize;
                        let sf = renderer.scale_factor;

                        if crate::ui::search_panel::is_in_input(
                            self.cursor_pos.0,
                            self.cursor_pos.1,
                            content_y,
                            sf as f32,
                            panel_w,
                        ) {
                            return Some(AppAction::FocusSearchInput);
                        }

                        if let Some(flat_idx) = crate::ui::search_panel::hit_test(
                            self.cursor_pos.1,
                            content_y,
                            self.search_panel.scroll_offset,
                            &self.search_panel,
                            sf,
                        ) && let Some((path, line)) =
                            self.search_panel.path_at_flat_index(flat_idx)
                        {
                            return Some(AppAction::OpenFileAtLine { path, line });
                        }
                    }
                }
                None
            }
        }
    }

    /// Query: max chars per visual line in the commit textarea.
    fn commit_input_max_chars(&self) -> usize {
        let sf = self
            .renderer
            .as_ref()
            .map(|r| r.scale_factor)
            .unwrap_or(1.0) as f32;
        let panel_w = self.panel_layout.right_physical_width(sf) as f32;
        let input_pad_x = crate::ui::components::git_panel::COMMIT_INPUT_PAD_X * sf;
        let char_w = 7.0 * sf;
        let text_max_px = panel_w - input_pad_x * 2.0 - 8.0 * sf - 16.0 * sf - 16.0 * sf;
        (text_max_px / char_w).floor().max(1.0) as usize
    }

    /// Query: map the current cursor position to a git-panel [`AppAction`].
    fn git_panel_action(&self) -> Option<crate::app::actions::AppAction> {
        use crate::app::actions::AppAction;
        use crate::ui::components::git_panel::{self, GitPanelHit};

        let renderer = self.renderer.as_ref()?;
        let hit = git_panel::hit_test(
            self.cursor_pos.0,
            self.cursor_pos.1,
            &self.git_panel,
            &self.panel_layout,
            renderer.tab_bar_height as f64,
            renderer.width as usize,
            renderer.scale_factor,
        );
        match hit {
            GitPanelHit::ToolbarChanges => Some(AppAction::SwitchGitPanelTab {
                tab: crate::ui::panel_layout::GitPanelTab::Changes,
            }),
            GitPanelHit::ToolbarBranches => Some(AppAction::SwitchGitPanelTab {
                tab: crate::ui::panel_layout::GitPanelTab::Branches,
            }),
            GitPanelHit::SelectFile(idx) => Some(AppAction::GitOpenFileDiff { index: idx }),
            GitPanelHit::StageFile(idx) => Some(AppAction::GitStageFile { index: idx }),
            GitPanelHit::UnstageFile(idx) => Some(AppAction::GitUnstageFile { index: idx }),
            GitPanelHit::CommitInput { rel_x, rel_y } => {
                Some(AppAction::GitFocusCommitInput { rel_x, rel_y })
            }
            GitPanelHit::CommitButton => Some(AppAction::GitCommit),
            GitPanelHit::GenerateButton => {
                if self.git_panel.generating_commit_msg {
                    Some(AppAction::GitCancelGenerateCommitMessage)
                } else {
                    Some(AppAction::GitGenerateCommitMessage)
                }
            }
            GitPanelHit::StageAll => Some(AppAction::GitStageAll),
            GitPanelHit::UnstageAll => Some(AppAction::GitUnstageAll),
            GitPanelHit::CheckoutBranch(idx) => Some(AppAction::GitCheckoutBranch { index: idx }),
            GitPanelHit::None => None,
        }
    }

    fn git_entry_abs_path(&self, index: usize) -> Option<std::path::PathBuf> {
        let entry = self.git_panel.data.entries.get(index)?;
        let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
        let repo = crate::git::GitRepo::discover(&cwd)?;
        let workdir = repo
            .workdir_path()
            .unwrap_or_else(|| std::path::PathBuf::from(&cwd));
        Some(workdir.join(&entry.path))
    }

    /// Convert mouse pixel position to a block text position (for Smart mode selection).
    fn mouse_to_block_pos(&self, mx: f64, my: f64) -> Option<crate::blocks::BlockTextPos> {
        let renderer = self.renderer.as_ref()?;
        let sf = renderer.scale_factor as f32;
        let pad = renderer.grid_padding_for(false, false) as usize;
        let bar_h = renderer.tab_bar_height as usize;
        let prompt_h = crate::ui::components::prompt_bar::prompt_bar_height(sf);
        let banner_h = self.total_banners_height(sf);
        let y_end = (renderer.height as usize).saturating_sub(prompt_h + banner_h);
        let x_offset = if self.overlay.sidebar_open {
            self.panel_layout.left_physical_width(sf)
        } else {
            0
        };
        let bl = self.active_block_list()?;
        crate::ui::components::block_renderer::hit_test(
            bl,
            mx,
            my,
            bar_h,
            y_end,
            pad,
            x_offset,
            renderer.width as usize,
            sf,
            renderer.block_char_width,
        )
    }

    /// Compute the maximum character count per line in block view.
    fn block_max_chars(&self) -> usize {
        self.renderer
            .as_ref()
            .map(|r| {
                let sf = r.scale_factor as f32;
                let pad = r.grid_padding_for(false, false) as usize;
                let x_offset = if self.overlay.sidebar_open {
                    self.panel_layout.left_physical_width(sf)
                } else {
                    0
                };
                let block_pad_inner = (10.0 * sf) as usize;
                let left_pad = pad + x_offset + block_pad_inner;
                let right_pad = pad + block_pad_inner;
                let git_panel_w = if self.overlay.git_panel_open {
                    self.panel_layout.right_physical_width(sf)
                } else {
                    0
                };
                let content_w =
                    (r.width as usize).saturating_sub(left_pad + right_pad + git_panel_w);
                let char_w_px = r.block_char_width as usize;
                if char_w_px > 0 {
                    content_w / char_w_px
                } else {
                    0
                }
            })
            .unwrap_or(0)
    }

    /// Check whether the cursor is over the scrollbar thumb area.
    fn is_over_scrollbar(&self, mx: f64, my: f64) -> bool {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return false,
        };
        let sf = renderer.scale_factor as f32;
        let input_type = self.settings_state.input_type;
        let is_smart = input_type == crate::ui::components::overlay::settings::InputType::Smart;
        let is_app = self
            .active_terminal()
            .map(|t| t.is_app_controlled())
            .unwrap_or(false);
        if !is_smart || is_app {
            return false;
        }
        let bl = match self.active_block_list() {
            Some(bl) => bl,
            None => return false,
        };
        if bl.blocks.is_empty() {
            return false;
        }
        let bar_h = renderer.tab_bar_height as usize;
        let prompt_h = crate::ui::components::prompt_bar::prompt_bar_height(sf);
        let banner_h = self.total_banners_height(sf);
        let available_h = (renderer.height as usize).saturating_sub(bar_h + prompt_h + banner_h);
        let max_chars = self.block_max_chars();
        let total_h =
            crate::ui::components::block_renderer::total_height(bl, sf, max_chars) as usize;
        if total_h <= available_h {
            return false;
        }
        let scroll = bl.scroll_offset.max(0.0) as usize;
        let git_panel_w = if self.overlay.git_panel_open {
            self.panel_layout.right_physical_width(sf)
        } else {
            0
        };
        let geom = crate::ui::components::block_renderer::scrollbar_geometry(
            (renderer.width as usize).saturating_sub(git_panel_w),
            bar_h,
            available_h,
            total_h,
            scroll,
            sf,
        );
        let px = mx as usize;
        let py = my as usize;
        let hit_x_start = geom.track_x.saturating_sub(geom.width);
        px >= hit_x_start
            && px < geom.track_x + geom.width + geom.width
            && py >= geom.thumb_y
            && py < geom.thumb_y + geom.thumb_h
    }

    /// Update scroll offset based on mouse drag position.
    fn update_scrollbar_drag(&mut self, my: f64) {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };
        let sf = renderer.scale_factor as f32;
        let bar_h = renderer.tab_bar_height as usize;
        let prompt_h = crate::ui::components::prompt_bar::prompt_bar_height(sf);
        let banner_h = self.total_banners_height(sf);
        let available_h = (renderer.height as usize).saturating_sub(bar_h + prompt_h + banner_h);
        let max_chars = self.block_max_chars();
        let bl = match self.active_block_list() {
            Some(bl) => bl,
            None => return,
        };
        let total_h =
            crate::ui::components::block_renderer::total_height(bl, sf, max_chars) as usize;
        if total_h <= available_h {
            return;
        }
        let git_panel_w = if self.overlay.git_panel_open {
            self.panel_layout.right_physical_width(sf)
        } else {
            0
        };
        let geom = crate::ui::components::block_renderer::scrollbar_geometry(
            (renderer.width as usize).saturating_sub(git_panel_w),
            bar_h,
            available_h,
            total_h,
            bl.scroll_offset.max(0.0) as usize,
            sf,
        );
        let track_usable = geom.track_h.saturating_sub(geom.thumb_h) as f64;
        if track_usable <= 0.0 {
            return;
        }
        let dy_px = my - self.scrollbar_drag_start_y;
        let scroll_delta = -(dy_px / track_usable) * geom.max_scroll as f64;
        let new_scroll = (self.scrollbar_drag_start_scroll as f64 + scroll_delta)
            .max(0.0)
            .min(geom.max_scroll as f64);
        if let Some(bl) = self.active_block_list_mut() {
            bl.scroll_offset = new_scroll as f32;
        }
    }

    /// Hit-test the editor scrollbar at cursor position.
    fn editor_scrollbar_hit_test(
        &self,
        mx: f64,
        my: f64,
    ) -> crate::ui::components::editor_renderer::ScrollbarHit {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return crate::ui::components::editor_renderer::ScrollbarHit::None,
        };
        let sf = renderer.scale_factor as f32;
        let bar_h = renderer.tab_bar_height as usize;
        let x_off = self.side_panel_x_offset();
        let git_w = if self.overlay.git_panel_open {
            self.panel_layout.right_physical_width(sf)
        } else {
            0
        };
        let content_h = (renderer.height as usize).saturating_sub(bar_h);
        let content_w = (renderer.width as usize)
            .saturating_sub(x_off)
            .saturating_sub(git_w);

        if let Some(state) = self.active_editor_state() {
            crate::ui::components::editor_renderer::scrollbar_hit_test(
                state,
                mx as usize,
                my as usize,
                x_off,
                bar_h,
                content_w,
                content_h,
                sf,
            )
        } else {
            crate::ui::components::editor_renderer::ScrollbarHit::None
        }
    }

    fn diff_divider_hit_test(&self, mx: f64, my: f64) -> bool {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return false,
        };
        let state = match self.active_editor_state() {
            Some(s) if s.has_diff_view() => s,
            _ => return false,
        };
        let sf = renderer.scale_factor as f32;
        let bar_h = renderer.tab_bar_height as usize;
        let x_off = self.side_panel_x_offset();
        let git_w = if self.overlay.git_panel_open {
            self.panel_layout.right_physical_width(sf)
        } else {
            0
        };
        let content_w = (renderer.width as usize)
            .saturating_sub(x_off)
            .saturating_sub(git_w);

        let div_x = crate::ui::components::editor_renderer::diff_divider_x(
            x_off,
            content_w,
            state.diff_split_frac,
            sf,
        );
        let grab_zone = (4.0 * sf).max(3.0) as usize;
        let px = mx as usize;
        let py = my as usize;
        px.abs_diff(div_x) <= grab_zone && py > bar_h
    }

    /// Update editor scroll during scrollbar drag.
    fn update_editor_scrollbar_drag(&mut self, mx: f64, my: f64) {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };
        let sf = renderer.scale_factor as f32;
        let bar_h = renderer.tab_bar_height as usize;
        let x_off = self.side_panel_x_offset();
        let git_w = if self.overlay.git_panel_open {
            self.panel_layout.right_physical_width(sf)
        } else {
            0
        };
        let content_h = (renderer.height as usize).saturating_sub(bar_h);
        let content_w = (renderer.width as usize)
            .saturating_sub(x_off)
            .saturating_sub(git_w);

        match self.editor_scrollbar_dragging {
            crate::ui::components::editor_renderer::ScrollbarHit::Vertical => {
                if let Some(state) = self.active_editor_state() {
                    let new_scroll =
                        crate::ui::components::editor_renderer::vertical_drag_to_scroll(
                            state, my, bar_h, content_h, sf,
                        );
                    if let Some(ed) = self.active_editor_state_mut() {
                        ed.scroll_offset = new_scroll;
                    }
                }
            }
            crate::ui::components::editor_renderer::ScrollbarHit::Horizontal => {
                if let Some(state) = self.active_editor_state() {
                    let new_scroll =
                        crate::ui::components::editor_renderer::horizontal_drag_to_scroll(
                            state, mx, x_off, content_w, sf,
                        );
                    if let Some(ed) = self.active_editor_state_mut() {
                        ed.set_scroll_x(new_scroll);
                    }
                }
            }
            crate::ui::components::editor_renderer::ScrollbarHit::None => {}
        }
    }

    /// Copy selected text to the system clipboard.
    /// Tries block selection first, then terminal grid selection.
    /// The selection is preserved so the user can see what was copied.
    pub(crate) fn perform_copy(&mut self) {
        if self.search_panel.focused {
            if let Some((start, end)) = self.search_panel.selected_range() {
                let selected = self.search_panel.query[start..end].to_string();
                let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(selected));
            }
            return;
        }
        if self.is_editor_active() {
            if let Some(ed) = self.active_editor_state()
                && let Some(sel_text) = ed.selected_text()
            {
                let len = sel_text.len();
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(&sel_text);
                }
                self.toast_mgr.push(
                    format!("Copied {} chars", len),
                    crate::ui::components::toast::ToastLevel::Info,
                );
            }
            return;
        }

        if let Some(sel) = &self.block_selection {
            let bl = match self.active_block_list() {
                Some(bl) => bl,
                None => return,
            };
            let max_chars = self
                .renderer
                .as_ref()
                .map(|r| {
                    let sf = r.scale_factor as f32;
                    let pad = r.grid_padding_for(false, false) as usize;
                    let block_pad_inner = (10.0 * sf) as usize;
                    let content_w = (r.width as usize).saturating_sub((pad + block_pad_inner) * 2);
                    let char_w_px = r.block_char_width as usize;
                    if char_w_px > 0 {
                        content_w / char_w_px
                    } else {
                        0
                    }
                })
                .unwrap_or(0);
            let text = sel.extract_text(&bl.blocks, max_chars);
            log::info!("Copy block selection: {} chars", text.len());
            if !text.is_empty() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&text);
                }
                self.toast_mgr.push(
                    format!("Copied {} chars", text.len()),
                    crate::ui::components::toast::ToastLevel::Info,
                );
            }
            return;
        }

        let grid_text = self.active_terminal().and_then(|t| t.selection_to_string());
        if let Some(text) = grid_text {
            log::info!("Copy grid selection: {} chars", text.len());
            let len = text.len();
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(&text);
            }
            self.toast_mgr.push(
                format!("Copied {} chars", len),
                crate::ui::components::toast::ToastLevel::Info,
            );
            if let Some(terminal) = self.active_terminal() {
                terminal.clear_selection();
            }
            if let Some(r) = self.renderer.as_mut() {
                r.invalidate_grid_cache();
            }
        }
    }

    /// Paste text from the system clipboard.
    pub(crate) fn perform_paste(&mut self) {
        let paste_text = match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
            Ok(t) if !t.is_empty() => t,
            _ => return,
        };

        if self.search_panel.focused {
            let cleaned: String = paste_text.chars().filter(|c| !c.is_control()).collect();
            if !cleaned.is_empty() {
                self.search_panel.insert_text(&cleaned);
                if let Some(r) = self.renderer.as_mut() {
                    let sf = r.scale_factor as f32;
                    let panel_w = self.panel_layout.left_physical_width(sf);
                    self.search_panel
                        .ensure_cursor_visible(&mut r.font_system, sf, panel_w);
                }
            }
            self.request_redraw();
            return;
        }

        if self.overlay.pro_panel_open {
            let cleaned: String = paste_text.chars().filter(|c| !c.is_control()).collect();
            if !cleaned.is_empty() {
                let cur = self.overlay.pro_license_cursor;
                let byte_pos = self
                    .overlay
                    .pro_license_input
                    .char_indices()
                    .nth(cur)
                    .map(|(i, _)| i)
                    .unwrap_or(self.overlay.pro_license_input.len());
                self.overlay
                    .pro_license_input
                    .insert_str(byte_pos, &cleaned);
                self.overlay.pro_license_cursor += cleaned.chars().count();
            }
            self.request_redraw();
            return;
        }

        if self.file_tree.renaming_idx.is_some() {
            let cleaned: String = paste_text
                .chars()
                .filter(|c| !c.is_control() && *c != '/')
                .collect();
            if !cleaned.is_empty() {
                let cur = self.file_tree.rename_cursor;
                let byte_pos = self
                    .file_tree
                    .rename_text
                    .char_indices()
                    .nth(cur)
                    .map(|(i, _)| i)
                    .unwrap_or(self.file_tree.rename_text.len());
                self.file_tree.rename_text.insert_str(byte_pos, &cleaned);
                self.file_tree.rename_cursor += cleaned.chars().count();
            }
            self.request_redraw();
            return;
        }

        if self.overlay.palette_open {
            let cleaned: String = paste_text.chars().filter(|c| !c.is_control()).collect();
            if !cleaned.is_empty() {
                self.overlay.palette_query.push_str(&cleaned);
                self.overlay.palette_selected = 0;
            }
            self.request_redraw();
            return;
        }

        if self.is_editor_active() {
            let sf = self
                .renderer
                .as_ref()
                .map_or(1.0, |r| r.scale_factor as f32);
            let vh = self.renderer.as_ref().map_or(600, |r| r.height as usize);
            if let Some(ed) = self.active_editor_state_mut() {
                ed.insert_str(&paste_text);
                ed.ensure_cursor_visible(sf, vh);
            }
            self.request_redraw();
            return;
        }

        let use_smart = self.settings_state.input_type == InputType::Smart
            && self
                .active_terminal()
                .map(|t| !t.is_app_controlled())
                .unwrap_or(false);
        if use_smart {
            self.smart_input
                .text
                .insert_str(self.smart_input.cursor, &paste_text);
            self.smart_input.cursor += paste_text.len();
            self.smart_input.update_slash_menu();
            let cwd = self.active_terminal().and_then(|t| t.cwd());
            self.smart_input.update_suggestion(cwd.as_deref());
        } else if self.is_sandbox_active() {
            let mut data = Vec::new();
            data.extend_from_slice(b"\x1b[200~");
            data.extend_from_slice(paste_text.as_bytes());
            data.extend_from_slice(b"\x1b[201~");
            self.send_input_to_active(&data);
        } else if let Some(terminal) = self.active_terminal() {
            let mut data = Vec::new();
            data.extend_from_slice(b"\x1b[200~");
            data.extend_from_slice(paste_text.as_bytes());
            data.extend_from_slice(b"\x1b[201~");
            terminal.input(Cow::Owned(data));
        }
    }

    pub(crate) fn perform_cut(&mut self) {
        if self.search_panel.focused {
            if let Some((start, end)) = self.search_panel.selected_range() {
                let selected = self.search_panel.query[start..end].to_string();
                let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(selected));
                self.search_panel.delete_selection();
                if let Some(r) = self.renderer.as_mut() {
                    let sf = r.scale_factor as f32;
                    let panel_w = self.panel_layout.left_physical_width(sf);
                    self.search_panel
                        .ensure_cursor_visible(&mut r.font_system, sf, panel_w);
                }
            }
            self.request_redraw();
            return;
        }
        if self.is_editor_active() {
            let sel_text = self.active_editor_state().and_then(|ed| ed.selected_text());
            if let Some(text) = sel_text {
                let len = text.len();
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(&text);
                }
                self.toast_mgr.push(
                    format!("Cut {} chars", len),
                    crate::ui::components::toast::ToastLevel::Info,
                );
                let sf = self
                    .renderer
                    .as_ref()
                    .map_or(1.0, |r| r.scale_factor as f32);
                let vh = self.renderer.as_ref().map_or(600, |r| r.height as usize);
                if let Some(ed) = self.active_editor_state_mut() {
                    ed.push_undo_snapshot();
                    ed.delete_selection();
                    ed.ensure_cursor_visible(sf, vh);
                }
            }
            self.request_redraw();
            return;
        }
        self.perform_copy();
    }

    /// Select all text in the current block view or terminal grid.
    pub(crate) fn perform_select_all(&mut self) {
        if self.search_panel.focused {
            self.search_panel.select_all();
            self.request_redraw();
            return;
        }
        if self.is_editor_active() {
            if let Some(ed) = self.active_editor_state_mut() {
                ed.select_all();
            }
            self.request_redraw();
            return;
        }

        let use_smart = self.settings_state.input_type == InputType::Smart
            && self
                .active_terminal()
                .map(|t| !t.is_app_controlled())
                .unwrap_or(false);

        if use_smart {
            let bl = match self.active_block_list() {
                Some(bl) => bl,
                None => return,
            };
            if !bl.blocks.is_empty() {
                let max_chars = self
                    .renderer
                    .as_ref()
                    .map(|r| {
                        let sf = r.scale_factor as f32;
                        let pad = r.grid_padding_for(false, false) as usize;
                        let block_pad_inner = (10.0 * sf) as usize;
                        let content_w =
                            (r.width as usize).saturating_sub((pad + block_pad_inner) * 2);
                        let char_w_px = r.block_char_width as usize;
                        if char_w_px > 0 {
                            content_w / char_w_px
                        } else {
                            0
                        }
                    })
                    .unwrap_or(0);

                let last_block_idx = bl.blocks.len() - 1;
                let last_block = &bl.blocks[last_block_idx];
                let last_line_count =
                    crate::ui::components::block_renderer::wrapped_output_line_count(
                        &last_block.output,
                        max_chars,
                    );
                let last_line_idx = last_line_count.saturating_sub(1);
                let last_char_idx = last_block
                    .output
                    .last()
                    .map(|line| {
                        let text: String = line.iter().map(|s| s.text.as_str()).collect();
                        text.chars().count()
                    })
                    .unwrap_or(0);

                let anchor = crate::blocks::BlockTextPos {
                    block_idx: 0,
                    line_idx: 0,
                    char_idx: 0,
                };
                let head = crate::blocks::BlockTextPos {
                    block_idx: last_block_idx,
                    line_idx: last_line_idx,
                    char_idx: last_char_idx,
                };
                self.block_selection = Some(crate::blocks::BlockSelection { anchor, head });
            }
        }
    }

    pub(crate) fn perform_undo(&mut self) {
        if self.is_editor_active() {
            if let Some(ed) = self.active_editor_state_mut() {
                ed.undo();
            }
            self.request_redraw();
        }
    }

    pub(crate) fn perform_redo(&mut self) {
        if self.is_editor_active() {
            if let Some(ed) = self.active_editor_state_mut() {
                ed.redo();
            }
            self.request_redraw();
        }
    }

    fn is_cursor_over_smart_input(&self) -> bool {
        let renderer = match self.renderer.as_ref() {
            Some(r) => r,
            None => return false,
        };
        let sf = renderer.scale_factor as f32;
        let is_smart = self
            .tab_mgr
            .active_tab()
            .and_then(|t| t.terminal())
            .map(|t| !t.is_app_controlled())
            .unwrap_or(false);
        if !is_smart {
            return false;
        }
        let prompt_h = crate::ui::components::prompt_bar::prompt_bar_height(sf);
        let prompt_top = (renderer.height as usize).saturating_sub(prompt_h);
        self.cursor_pos.1 >= prompt_top as f64
    }

    fn insert_dropped_path(&mut self, path: &std::path::Path) {
        let escaped = shell_escape(&path.to_string_lossy());
        if self.is_cursor_over_smart_input() {
            self.smart_input
                .text
                .insert_str(self.smart_input.cursor, &escaped);
            self.smart_input.cursor += escaped.len();
            self.smart_input.update_slash_menu();
            let cwd = self.active_terminal().and_then(|t| t.cwd());
            self.smart_input.update_suggestion(cwd.as_deref());
        } else if let Some(terminal) = self.active_terminal() {
            terminal.input(std::borrow::Cow::Owned(escaped.into_bytes()));
        }
        self.request_redraw();
    }

    pub(crate) fn handle_dropped_file(&mut self, path: std::path::PathBuf) {
        self.insert_dropped_path(&path);
    }
}

/// Editor commands that should be intercepted and opened in the built-in editor.
const EDITOR_COMMANDS: &[&str] = &[
    "vim", "vi", "nvim", "nano", "pico", "emacs", "code", "subl", "edit", "open",
];

/// Query: check if a command is an editor invocation with a file path.
/// Returns the resolved path if the command should be intercepted.
fn try_intercept_editor_command(cmd: &str, cwd: Option<&str>) -> Option<std::path::PathBuf> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let binary = std::path::Path::new(parts[0])
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| parts[0].to_string());

    if !EDITOR_COMMANDS.contains(&binary.as_str()) {
        return None;
    }

    let file_arg = parts[1..].iter().rev().find(|a| !a.starts_with('-'))?;

    let path = std::path::Path::new(file_arg);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base) = cwd {
        std::path::Path::new(base).join(path)
    } else {
        std::env::current_dir().ok()?.join(path)
    };

    if resolved.is_file() {
        Some(resolved)
    } else {
        None
    }
}

/// Escape a path for shell use — wraps in single quotes if it contains special chars.
fn shell_escape(s: &str) -> String {
    if s.contains(|c: char| c.is_whitespace() || "\"'\\$`!#&|;(){}[]<>?*~".contains(c)) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

fn sgr_mouse_press(button: u32, col: u32, row: u32) -> String {
    format!("\x1b[<{};{};{}M", button, col + 1, row + 1)
}

fn sgr_mouse_release(button: u32, col: u32, row: u32) -> String {
    format!("\x1b[<{};{};{}m", button, col + 1, row + 1)
}

fn sgr_mouse_move(col: u32, row: u32) -> String {
    format!("\x1b[<32;{};{}M", col + 1, row + 1)
}

fn sgr_mouse_scroll(up: bool, col: u32, row: u32) -> String {
    let button = if up { 64 } else { 65 };
    format!("\x1b[<{};{};{}M", button, col + 1, row + 1)
}
