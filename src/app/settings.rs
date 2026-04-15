use crate::renderer;
use crate::ui::components::overlay::{AiModelsHit, InputType};

use super::TabKind;

impl super::App {
    /// Sync in-memory settings back to config and persist to disk.
    pub(crate) fn save_config(&mut self) {
        self.config.appearance.input_type = match self.settings_state.input_type {
            InputType::Smart => "smart".into(),
            InputType::ShellPS1 => "shell_ps1".into(),
        };
        self.config.appearance.font_family = self.settings_state.font_family.clone();
        self.config.appearance.font_size = self.settings_state.font_size_px;
        self.config.appearance.line_height = self.settings_state.line_height_px;
        self.config.ai.models_path = self.settings_state.models_path.clone();
        self.config.ai.web_search = self.settings_state.web_search_enabled;
        self.config.ai.ollama_enabled = self.settings_state.ollama_enabled;
        self.config.ai.ollama_host = self.settings_state.ollama_host.clone();
        self.config.ai.ollama_model = self.settings_state.ollama_model.clone();
        if let Some(name) = &self.ai_ctrl.state.loaded_model_name {
            self.config.ai.last_model = name.clone();
        }
        self.config.save();
    }

    pub(crate) fn handle_ai_models_hit(&mut self, hit: AiModelsHit) {
        match hit {
            AiModelsHit::OpenInFinder => {
                let path = &self.settings_state.models_path;
                let dir = std::path::Path::new(path);
                if !dir.exists() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::process::Command::new("open").arg(path).spawn();
            }
            AiModelsHit::ChangePath => {
                let current = self.settings_state.models_path.clone();
                let result = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(format!(
                        "set theFolder to choose folder with prompt \"Select models directory\" default location POSIX file \"{}\"
return POSIX path of theFolder",
                        current
                    ))
                    .output();
                if let Ok(out) = result
                    && out.status.success()
                {
                    let new_path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !new_path.is_empty() {
                        self.settings_state.models_path = new_path;
                    }
                }
            }
            AiModelsHit::ToggleWebSearch => {
                self.settings_state.web_search_enabled = !self.settings_state.web_search_enabled;
            }
            AiModelsHit::OllamaHostInput => {}
            AiModelsHit::DeleteModel(idx) => {
                if self.settings_state.deleting_model.is_some() {
                    return;
                }
                if let Some(model) = crate::ai::registry::MODELS.get(idx) {
                    let path = std::path::PathBuf::from(&self.settings_state.models_path)
                        .join(model.filename);
                    if path.exists() {
                        self.settings_state.deleting_model = Some(idx);
                        let proxy = self.proxy.clone();
                        tokio::task::spawn_blocking(move || {
                            let _ = std::fs::remove_file(&path);
                            let _ =
                                proxy.send_event(crate::terminal::TerminalEvent::ModelDeleted(idx));
                        });
                    }
                }
            }
            AiModelsHit::OpenModels => {
                self.close_settings_view();
                self.open_models_view();
                return;
            }
            AiModelsHit::RuntimeLocal => {
                self.settings_state.ollama_enabled = false;
                self.settings_state.ollama_host_focused = false;
                self.settings_state.ollama_host_sel_anchor = None;
            }
            AiModelsHit::RuntimeOllama => {
                self.settings_state.ollama_enabled = true;
                self.fetch_ollama_models();
            }
            AiModelsHit::OllamaTestConnection => {
                self.toast_mgr.push(
                    "Testing Ollama connection…".to_string(),
                    crate::ui::components::toast::ToastLevel::Info,
                );
                self.fetch_ollama_models();
            }
            AiModelsHit::OllamaRefresh => {
                self.fetch_ollama_models();
            }
            AiModelsHit::OllamaSelectModel(idx) => {
                if let Some(model) = self.settings_state.ollama_models.get(idx) {
                    self.settings_state.ollama_model = model.name.clone();
                }
            }
        }
        self.save_config();
    }

    pub(crate) fn fetch_ollama_models(&self) {
        let host = self.settings_state.ollama_host.clone();
        let proxy = self.proxy.clone();
        tokio::task::spawn_blocking(move || {
            let result = crate::ai::ollama::list_models(&host);
            let _ = proxy.send_event(crate::terminal::TerminalEvent::OllamaModelsLoaded(result));
        });
    }

    pub(crate) fn apply_font_settings(&mut self) {
        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };
        let sf = renderer.scale_factor;
        let font_size = self.settings_state.font_size_px * sf as f32;
        let line_height = self.settings_state.line_height_px * sf as f32;
        renderer.font_size = font_size;
        renderer.cell_height = line_height;
        renderer.font_family = self.settings_state.font_family.clone();
        renderer.glyph_atlas.clear();
        renderer.cell_width = renderer::measure_cell_width(
            &mut renderer.font_system,
            font_size,
            line_height,
            &self.settings_state.font_family,
        );
        for tab in self.tab_mgr.iter() {
            if let TabKind::Terminal {
                terminal, is_alt, ..
            } = &tab.kind
            {
                let ws = Self::compute_window_size(renderer, *is_alt);
                terminal.resize(ws);
            }
        }
    }

    fn ollama_host_selected_range(&self) -> Option<(usize, usize)> {
        let anchor = self.settings_state.ollama_host_sel_anchor?;
        let cursor = self.settings_state.ollama_host_cursor;
        if anchor == cursor {
            return None;
        }
        Some((anchor.min(cursor), anchor.max(cursor)))
    }

    fn ollama_host_delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.ollama_host_selected_range() {
            self.settings_state.ollama_host.drain(start..end);
            self.settings_state.ollama_host_cursor = start;
            self.settings_state.ollama_host_sel_anchor = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn ollama_host_insert(&mut self, s: &str) {
        self.ollama_host_delete_selection();
        let pos = self.settings_state.ollama_host_cursor;
        self.settings_state.ollama_host.insert_str(pos, s);
        self.settings_state.ollama_host_cursor = pos + s.len();
        self.settings_state.ollama_host_sel_anchor = None;
    }

    pub(crate) fn ollama_host_delete_back(&mut self) {
        if self.ollama_host_delete_selection() {
            return;
        }
        let pos = self.settings_state.ollama_host_cursor;
        if pos == 0 {
            return;
        }
        let prev = self.settings_state.ollama_host[..pos]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        self.settings_state.ollama_host.drain(prev..pos);
        self.settings_state.ollama_host_cursor = prev;
    }

    pub(crate) fn ollama_host_delete_forward(&mut self) {
        if self.ollama_host_delete_selection() {
            return;
        }
        let pos = self.settings_state.ollama_host_cursor;
        let len = self.settings_state.ollama_host.len();
        if pos >= len {
            return;
        }
        let next = self.settings_state.ollama_host[pos..]
            .char_indices()
            .nth(1)
            .map_or(len, |(i, _)| pos + i);
        self.settings_state.ollama_host.drain(pos..next);
    }

    pub(crate) fn ollama_host_move_left(&mut self, shift: bool) {
        let pos = self.settings_state.ollama_host_cursor;
        if shift && self.settings_state.ollama_host_sel_anchor.is_none() {
            self.settings_state.ollama_host_sel_anchor = Some(pos);
        }
        if !shift {
            if let Some((start, _end)) = self.ollama_host_selected_range() {
                self.settings_state.ollama_host_cursor = start;
                self.settings_state.ollama_host_sel_anchor = None;
                return;
            }
            self.settings_state.ollama_host_sel_anchor = None;
        }
        if pos > 0 {
            let prev = self.settings_state.ollama_host[..pos]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            self.settings_state.ollama_host_cursor = prev;
        }
    }

    pub(crate) fn ollama_host_move_right(&mut self, shift: bool) {
        let pos = self.settings_state.ollama_host_cursor;
        let len = self.settings_state.ollama_host.len();
        if shift && self.settings_state.ollama_host_sel_anchor.is_none() {
            self.settings_state.ollama_host_sel_anchor = Some(pos);
        }
        if !shift {
            if let Some((_, end)) = self.ollama_host_selected_range() {
                self.settings_state.ollama_host_cursor = end;
                self.settings_state.ollama_host_sel_anchor = None;
                return;
            }
            self.settings_state.ollama_host_sel_anchor = None;
        }
        if pos < len {
            let next = self.settings_state.ollama_host[pos..]
                .char_indices()
                .nth(1)
                .map_or(len, |(i, _)| pos + i);
            self.settings_state.ollama_host_cursor = next;
        }
    }

    pub(crate) fn ollama_host_move_home(&mut self, shift: bool) {
        if shift && self.settings_state.ollama_host_sel_anchor.is_none() {
            self.settings_state.ollama_host_sel_anchor =
                Some(self.settings_state.ollama_host_cursor);
        }
        if !shift {
            self.settings_state.ollama_host_sel_anchor = None;
        }
        self.settings_state.ollama_host_cursor = 0;
    }

    pub(crate) fn ollama_host_move_end(&mut self, shift: bool) {
        if shift && self.settings_state.ollama_host_sel_anchor.is_none() {
            self.settings_state.ollama_host_sel_anchor =
                Some(self.settings_state.ollama_host_cursor);
        }
        if !shift {
            self.settings_state.ollama_host_sel_anchor = None;
        }
        self.settings_state.ollama_host_cursor = self.settings_state.ollama_host.len();
    }

    pub(crate) fn ollama_host_select_all(&mut self) {
        self.settings_state.ollama_host_sel_anchor = Some(0);
        self.settings_state.ollama_host_cursor = self.settings_state.ollama_host.len();
    }

    pub(crate) fn ollama_host_copy(&mut self) {
        if let Some((start, end)) = self.ollama_host_selected_range() {
            let text = &self.settings_state.ollama_host[start..end];
            if let Ok(mut clip) = arboard::Clipboard::new() {
                let _ = clip.set_text(text);
            }
        }
    }

    pub(crate) fn ollama_host_cut(&mut self) {
        self.ollama_host_copy();
        self.ollama_host_delete_selection();
    }
}
