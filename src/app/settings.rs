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
        }
        self.save_config();
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
}
