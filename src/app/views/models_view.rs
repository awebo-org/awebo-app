//! Models view — renders as a dedicated tab via the Tab+Router system.
//! Provides a full model repository browser with search, download, load/unload.

use std::collections::HashMap;
use std::sync::mpsc;

use crate::ai;
use crate::ai::model_manager::DownloadProgress;
use crate::app::router;

/// State for the Models tab view.
pub struct ModelsViewState {
    pub search_query: String,
    pub search_focused: bool,
    pub selected_index: usize,
    pub scroll_offset: f32,
    /// Active downloads keyed by model name.
    pub active_downloads: HashMap<String, DownloadProgress>,
    /// Receivers for download progress updates.
    pub download_receivers: Vec<(String, mpsc::Receiver<DownloadProgress>)>,
    pub hovered_action: Option<usize>,
    pub hovered_delete: Option<usize>,
}

impl ModelsViewState {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            search_focused: false,
            selected_index: 0,
            scroll_offset: 0.0,
            active_downloads: HashMap::new(),
            download_receivers: Vec::new(),
            hovered_action: None,
            hovered_delete: None,
        }
    }

    /// Returns filtered model indices based on current search query.
    /// Downloaded models are sorted to the top.
    pub fn filtered_indices(&self, models_path: &str) -> Vec<usize> {
        let q = self.search_query.to_lowercase();
        let models_dir = std::path::Path::new(models_path);
        let mut indices: Vec<usize> = ai::registry::MODELS
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                if q.is_empty() {
                    return true;
                }
                m.name.to_lowercase().contains(&q)
                    || m.family.to_lowercase().contains(&q)
                    || m.params.to_lowercase().contains(&q)
                    || m.quant_label.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        indices.sort_by_key(|&i| {
            let downloaded = models_dir.join(ai::registry::MODELS[i].filename).exists();
            if downloaded { 0 } else { 1 }
        });
        indices
    }

    /// Poll all active download receivers and update progress state.
    /// Returns true if any progress was updated (needs redraw).
    pub fn poll_downloads(&mut self) -> bool {
        let mut updated = false;
        let mut finished = Vec::new();

        for (name, rx) in &self.download_receivers {
            loop {
                match rx.try_recv() {
                    Ok(progress) => {
                        let done = progress.finished;
                        self.active_downloads.insert(name.clone(), progress);
                        updated = true;
                        if done {
                            finished.push(name.clone());
                            break;
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        finished.push(name.clone());
                        break;
                    }
                }
            }
        }

        for name in &finished {
            self.download_receivers.retain(|(n, _)| n != name);
        }

        updated
    }

    /// Check if a model is currently being downloaded.
    pub fn is_downloading(&self, model_name: &str) -> bool {
        self.active_downloads
            .get(model_name)
            .map(|p| !p.finished)
            .unwrap_or(false)
    }
}

impl Default for ModelsViewState {
    fn default() -> Self {
        Self::new()
    }
}

/// Human-readable byte size (e.g. "1.23 GB", "456 MB").
fn format_download_size(bytes: u64) -> String {
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else {
        format!("{:.1} MB", b / MB)
    }
}

impl super::super::App {
    /// Open the models view as a new tab (or focus existing one).
    pub(crate) fn open_models_view(&mut self) {
        if let Some(idx) = self.tab_mgr.find_models() {
            self.tab_mgr.switch_to(idx);
            self.request_redraw();
            return;
        }

        self.tab_mgr.push(super::super::Tab::new_models());

        if let Some(r) = self.renderer.as_mut() {
            r.invalidate_grid_cache();
        }
        self.request_redraw();
    }

    /// Close the models tab (if one exists).
    pub(crate) fn close_models_view(&mut self) {
        if let Some(idx) = self.tab_mgr.find_models() {
            self.tab_mgr.remove(idx);
            if let Some(r) = self.renderer.as_mut() {
                r.invalidate_grid_cache();
            }
            self.request_redraw();
        }
    }

    pub(crate) fn is_models_active(&self) -> bool {
        self.tab_mgr.get(self.tab_mgr.active_index())
            .map(|t| t.route() == router::Route::Models)
            .unwrap_or(false)
    }

    /// Start downloading a model by registry index.
    pub(crate) fn start_model_download(&mut self, registry_idx: usize) {
        let model = match ai::registry::MODELS.get(registry_idx) {
            Some(m) => m,
            None => return,
        };

        if self.models_view.is_downloading(model.name) {
            return;
        }

        let dest_dir = std::path::PathBuf::from(&self.settings_state.models_path);
        let (tx, rx) = mpsc::channel();

        ai::model_manager::download_model(
            model.hf_repo,
            model.hf_filename,
            model.name,
            &dest_dir,
            tx,
            self.proxy.clone(),
        );

        self.models_view
            .download_receivers
            .push((model.name.to_string(), rx));
    }

    /// Delete a downloaded model file by registry index.
    pub(crate) fn delete_model_file(&mut self, registry_idx: usize) {
        if let Some(model) = ai::registry::MODELS.get(registry_idx) {
            let path =
                std::path::PathBuf::from(&self.settings_state.models_path).join(model.filename);
            let _ = std::fs::remove_file(&path);

            if self.ai_ctrl.state.loaded_model_name.as_deref() == Some(model.name) {
                self.ai_ctrl.state.loaded_model = None;
                self.ai_ctrl.state.loaded_model_name = None;
                self.ai_ctrl.state.context_size = 0;
            }
        }
        self.request_redraw();
    }

    /// Load a model from the models view by registry index.
    pub(crate) fn load_model_from_view(&mut self, registry_idx: usize) {
        self.load_model_by_index(registry_idx);
    }

    /// Unload the currently loaded model.
    pub(crate) fn unload_current_model(&mut self) {
        self.ai_ctrl.state.loaded_model = None;
        self.ai_ctrl.state.loaded_model_name = None;
        self.ai_ctrl.state.context_size = 0;
        self.config.ai.last_model.clear();
        self.save_config();
        self.request_redraw();
    }

    /// Toggle auto-load setting.
    pub(crate) fn toggle_auto_load(&mut self) {
        self.config.ai.auto_load = !self.config.ai.auto_load;
        self.save_config();
        self.request_redraw();
    }

    /// Poll model download progress — called from the main event loop.
    pub(crate) fn poll_model_downloads(&mut self) {
        if !self.models_view.poll_downloads() {
            return;
        }
        self.request_redraw();

        if let Some(idx) = self.auto_download_model_idx {
            let model_name = ai::registry::MODELS[idx].name;
            if let Some(progress) = self.models_view.active_downloads.get(model_name) {
                let text = if progress.finished {
                    if let Some(ref err) = progress.error {
                        format!("Download failed: {err}")
                    } else {
                        format!("{model_name} downloaded — loading…")
                    }
                } else {
                    let pct = progress.percent();
                    let dl = progress.bytes_downloaded;
                    let total = progress.bytes_total;
                    format!(
                        "Downloading {model_name} … {pct}% ({} / {})",
                        format_download_size(dl),
                        format_download_size(total),
                    )
                };

                if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index()) {
                    if let super::super::TabKind::Terminal { block_list, .. } = &mut tab.kind {
                        if let Some(block) = block_list.blocks.last_mut() {
                            block.output = vec![crate::blocks::plain_line(
                                text,
                                crate::blocks::DEFAULT_FG,
                            )];
                            block_list.generation += 1;
                        }
                    }
                }

                if progress.finished && progress.error.is_none() {
                    self.auto_download_model_idx = None;
                    self.load_model_by_index(idx);
                } else if progress.finished {
                    self.auto_download_model_idx = None;
                }
            }
        }
    }

    /// Handle a left-click inside the models view.
    pub(crate) fn handle_models_click(&mut self) {
        if !self.is_models_active() {
            return;
        }
        let (bar_h, buf_w, sf) = match &self.renderer {
            Some(r) => (r.tab_bar_height as usize, r.width as usize, r.scale_factor as f32),
            None => return,
        };
        let x_off = self.side_panel_x_offset();

        use crate::ui::components::overlay::models_view::{models_view_hit_test, ModelsViewHit};
        let hit = models_view_hit_test(
            self.cursor_pos.0,
            self.cursor_pos.1,
            &self.models_view,
            self.ai_ctrl.state.loaded_model_name.as_deref(),
            &self.settings_state.models_path,
            bar_h,
            buf_w,
            x_off,
            sf,
        );

        match hit {
            Some(ModelsViewHit::SearchBar) => {
                self.models_view.search_focused = true;
            }
            _ => {
                self.models_view.search_focused = false;
                match hit {
                    Some(ModelsViewHit::AutoLoadToggle) => {
                        self.toggle_auto_load();
                    }
                    Some(ModelsViewHit::Download(idx)) => {
                        self.start_model_download(idx);
                    }
                    Some(ModelsViewHit::Load(idx)) => {
                        self.load_model_from_view(idx);
                    }
                    Some(ModelsViewHit::Unload(_)) => {
                        self.unload_current_model();
                    }
                    Some(ModelsViewHit::Delete(idx)) => {
                        self.delete_model_file(idx);
                    }
                    _ => {}
                }
            }
        }

        self.request_redraw();
    }

    /// Handle hover updates for the models view on CursorMoved.
    /// Returns true if the cursor is over a clickable element.
    pub(crate) fn update_models_hover(&mut self) -> bool {
        if !self.is_models_active() {
            return false;
        }
        let prev_action = self.models_view.hovered_action;
        let prev_delete = self.models_view.hovered_delete;

        self.models_view.hovered_action = None;
        self.models_view.hovered_delete = None;

        let (bar_h, buf_w, sf) = match &self.renderer {
            Some(r) => (r.tab_bar_height as usize, r.width as usize, r.scale_factor as f32),
            None => return false,
        };
        let x_off = self.side_panel_x_offset();

        use crate::ui::components::overlay::models_view::{models_view_hit_test, ModelsViewHit};
        let hit = models_view_hit_test(
            self.cursor_pos.0,
            self.cursor_pos.1,
            &self.models_view,
            self.ai_ctrl.state.loaded_model_name.as_deref(),
            &self.settings_state.models_path,
            bar_h,
            buf_w,
            x_off,
            sf,
        );

        let is_clickable = hit.is_some();

        match hit {
            Some(ModelsViewHit::Download(idx) | ModelsViewHit::Load(idx) | ModelsViewHit::Unload(idx)) => {
                let filtered = self.models_view.filtered_indices(&self.settings_state.models_path);
                if let Some(vi) = filtered.iter().position(|&i| i == idx) {
                    self.models_view.hovered_action = Some(vi);
                }
            }
            Some(ModelsViewHit::Delete(idx)) => {
                let filtered = self.models_view.filtered_indices(&self.settings_state.models_path);
                if let Some(vi) = filtered.iter().position(|&i| i == idx) {
                    self.models_view.hovered_delete = Some(vi);
                }
            }
            _ => {}
        }

        if self.models_view.hovered_action != prev_action || self.models_view.hovered_delete != prev_delete {
            self.request_redraw();
        }

        is_clickable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_view_state_default() {
        let s = ModelsViewState::new();
        assert!(s.search_query.is_empty());
        assert_eq!(s.selected_index, 0);
        assert!(s.active_downloads.is_empty());
        assert!(s.download_receivers.is_empty());
    }

    #[test]
    fn filtered_indices_returns_all_when_empty_query() {
        let s = ModelsViewState::new();
        let indices = s.filtered_indices("/nonexistent");
        assert_eq!(indices.len(), ai::registry::MODELS.len());
    }

    #[test]
    fn filtered_indices_filters_by_name() {
        let mut s = ModelsViewState::new();
        s.search_query = "llama".to_string();
        let indices = s.filtered_indices("/nonexistent");
        assert!(indices.len() < ai::registry::MODELS.len());
        for &idx in &indices {
            let m = &ai::registry::MODELS[idx];
            let found = m.name.to_lowercase().contains("llama")
                || m.family.to_lowercase().contains("llama");
            assert!(found);
        }
    }

    #[test]
    fn filtered_indices_filters_by_family() {
        let mut s = ModelsViewState::new();
        s.search_query = "OpenAI".to_string();
        let indices = s.filtered_indices("/nonexistent");
        assert!(!indices.is_empty());
        for &idx in &indices {
            let m = &ai::registry::MODELS[idx];
            assert!(
                m.family.to_lowercase().contains("openai")
                    || m.name.to_lowercase().contains("openai")
            );
        }
    }

    #[test]
    fn is_downloading_false_when_empty() {
        let s = ModelsViewState::new();
        assert!(!s.is_downloading("test"));
    }
}
