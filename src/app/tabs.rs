use std::path::Path;
use std::sync::Arc;

use alacritty_terminal::Term;
use alacritty_terminal::event::WindowSize;
use alacritty_terminal::sync::FairMutex;
use winit::event_loop::ActiveEventLoop;

use crate::blocks::BlockList;
use crate::renderer::Renderer;
use crate::sandbox::bridge::SandboxBridge;
use crate::terminal::{JsonEventProxy, Terminal};
use crate::ui::components::tab_bar::TabInfo;
use crate::ui::editor::EditorState;

use crate::session::SessionId;

use super::router::{Route, Router};

/// Content-specific data carried by each tab.
pub(crate) enum TabKind {
    Terminal {
        terminal: Terminal,
        is_alt: bool,
        block_list: BlockList,
    },
    /// A sandbox terminal running inside a microsandbox microVM.
    Sandbox {
        bridge: SandboxBridge,
        block_list: BlockList,
    },
    Editor {
        state: EditorState,
    },
    Settings,
    Models,
}

/// A single tab — owns a Router (current view) and its content data.
pub(crate) struct Tab {
    pub router: Router,
    pub kind: TabKind,
    pub session_id: Option<SessionId>,
}

impl Tab {
    pub fn new_terminal(terminal: Terminal) -> Self {
        Self {
            router: Router::new(),
            kind: TabKind::Terminal {
                terminal,
                is_alt: false,
                block_list: BlockList::new(),
            },
            session_id: None,
        }
    }

    /// Create a terminal tab with a pre-populated block_list (for session restore).
    pub fn new_terminal_with_blocks(
        terminal: Terminal,
        session_id: SessionId,
        block_list: BlockList,
    ) -> Self {
        Self {
            router: Router::new(),
            kind: TabKind::Terminal {
                terminal,
                is_alt: false,
                block_list,
            },
            session_id: Some(session_id),
        }
    }

    pub fn new_sandbox(bridge: SandboxBridge) -> Self {
        Self {
            router: Router::new(),
            kind: TabKind::Sandbox {
                bridge,
                block_list: BlockList::new(),
            },
            session_id: None,
        }
    }

    pub fn new_settings() -> Self {
        let mut router = Router::new();
        router.replace(Route::Settings);
        Self {
            router,
            kind: TabKind::Settings,
            session_id: None,
        }
    }

    pub fn new_models() -> Self {
        let mut router = Router::new();
        router.replace(Route::Models);
        Self {
            router,
            kind: TabKind::Models,
            session_id: None,
        }
    }

    /// Create a new editor tab for the given file path.
    pub fn new_editor(state: EditorState) -> Self {
        let mut router = Router::new();
        router.replace(Route::Editor);
        Self {
            router,
            kind: TabKind::Editor { state },
            session_id: None,
        }
    }

    pub fn title(&self) -> String {
        match &self.kind {
            TabKind::Terminal { terminal, .. } => terminal.display_title(),
            TabKind::Sandbox { bridge, .. } => bridge.display_name.clone(),
            TabKind::Editor { state } => state.file_name(),
            TabKind::Settings => "Settings".to_string(),
            TabKind::Models => "Local Models".to_string(),
        }
    }

    pub fn terminal(&self) -> Option<&Terminal> {
        match &self.kind {
            TabKind::Terminal { terminal, .. } => Some(terminal),
            _ => None,
        }
    }

    /// Query: get the underlying alacritty Term for grid rendering.
    /// Works for both Terminal and Sandbox tabs.
    pub fn term_handle(&self) -> Option<Arc<FairMutex<Term<JsonEventProxy>>>> {
        match &self.kind {
            TabKind::Terminal { terminal, .. } => Some(terminal.term.clone()),
            TabKind::Sandbox { bridge, .. } => Some(bridge.term.clone()),
            _ => None,
        }
    }

    /// Query: whether this tab renders as a raw terminal grid.
    /// True for app-controlled terminals (vim/htop) and sandbox tabs.
    pub fn is_app_controlled(&self) -> bool {
        match &self.kind {
            TabKind::Terminal { terminal, .. } => terminal.is_app_controlled(),
            TabKind::Sandbox { .. } => true,
            _ => false,
        }
    }

    pub fn block_list(&self) -> Option<&BlockList> {
        match &self.kind {
            TabKind::Terminal { block_list, .. } | TabKind::Sandbox { block_list, .. } => {
                Some(block_list)
            }
            _ => None,
        }
    }

    pub fn block_list_mut(&mut self) -> Option<&mut BlockList> {
        match &mut self.kind {
            TabKind::Terminal { block_list, .. } | TabKind::Sandbox { block_list, .. } => {
                Some(block_list)
            }
            _ => None,
        }
    }

    pub fn is_settings(&self) -> bool {
        matches!(&self.kind, TabKind::Settings)
    }

    pub fn is_models(&self) -> bool {
        matches!(&self.kind, TabKind::Models)
    }

    pub fn editor_state(&self) -> Option<&EditorState> {
        match &self.kind {
            TabKind::Editor { state } => Some(state),
            _ => None,
        }
    }

    pub fn editor_state_mut(&mut self) -> Option<&mut EditorState> {
        match &mut self.kind {
            TabKind::Editor { state } => Some(state),
            _ => None,
        }
    }

    /// Returns the file path if this is an editor tab.
    pub fn editor_path(&self) -> Option<&Path> {
        self.editor_state().map(|s| s.path.as_path())
    }

    pub fn route(&self) -> Route {
        self.router.current()
    }

    pub fn cwd(&self) -> Option<String> {
        match &self.kind {
            TabKind::Terminal { terminal, .. } => terminal.cwd(),
            _ => None,
        }
    }
}

/// Owns the tab collection and active-tab index.
/// Enforces invariants: `active < tabs.len()` (when non-empty).
pub(crate) struct TabManager {
    tabs: Vec<Tab>,
    active: usize,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn get(&self, idx: usize) -> Option<&Tab> {
        self.tabs.get(idx)
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(idx)
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    pub fn active_terminal(&self) -> Option<&Terminal> {
        self.active_tab().and_then(|t| t.terminal())
    }

    pub fn active_block_list(&self) -> Option<&BlockList> {
        self.active_tab().and_then(|t| t.block_list())
    }

    pub fn active_block_list_mut(&mut self) -> Option<&mut BlockList> {
        self.active_tab_mut().and_then(|t| t.block_list_mut())
    }

    /// Find the tab index for a given session ID.
    pub fn index_for_session(&self, session_id: SessionId) -> Option<usize> {
        self.tabs
            .iter()
            .position(|t| t.session_id == Some(session_id))
    }

    /// Find the index of a Settings tab, if any.
    pub fn find_settings(&self) -> Option<usize> {
        self.tabs.iter().position(|t| t.is_settings())
    }

    /// Find the index of a Models tab, if any.
    pub fn find_models(&self) -> Option<usize> {
        self.tabs.iter().position(|t| t.is_models())
    }

    /// Find the index of an editor tab for the given file path, if any.
    pub fn find_editor(&self, path: &Path) -> Option<usize> {
        self.tabs.iter().position(|t| t.editor_path() == Some(path))
    }

    /// Build display info for the tab bar.
    pub fn build_tab_infos(&self) -> Vec<TabInfo> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let is_error = match &tab.kind {
                    TabKind::Terminal { block_list, .. } | TabKind::Sandbox { block_list, .. } => {
                        block_list.last_is_error()
                    }
                    TabKind::Settings | TabKind::Models | TabKind::Editor { .. } => false,
                };
                let (icon, is_muted) = match &tab.kind {
                    TabKind::Sandbox { bridge, .. } => (
                        Some(crate::renderer::icons::Icon::CodeSandbox),
                        !bridge.is_alive(),
                    ),
                    _ => (None, false),
                };
                TabInfo {
                    title: tab.title(),
                    is_active: i == self.active,
                    is_error,
                    icon,
                    is_muted,
                }
            })
            .collect()
    }

    /// Iterate over all tabs.
    pub fn iter(&self) -> impl Iterator<Item = &Tab> {
        self.tabs.iter()
    }

    /// Push a new tab and switch to it. Returns the new tab's index.
    pub fn push(&mut self, tab: Tab) -> usize {
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.active
    }

    /// Switch to the tab at `idx`. No-op if out of bounds.
    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active = idx;
        }
    }

    /// Remove the tab at `idx` and adjust `active`. Returns true if tabs
    /// are now empty (caller should exit the app).
    pub fn remove(&mut self, idx: usize) -> bool {
        if idx >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            return true;
        }
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        false
    }

    /// Switch to previous tab (wrapping).
    pub fn previous(&mut self) {
        if !self.tabs.is_empty() {
            self.active = if self.active == 0 {
                self.tabs.len() - 1
            } else {
                self.active - 1
            };
        }
    }

    /// Switch to next tab (wrapping).
    pub fn next(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    /// Move a tab from one position to another (drag-and-drop reorder).
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active = to;
    }
}

impl super::App {
    pub(crate) fn create_tab(&mut self, shell_path: Option<&str>) {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };

        let working_directory = self
            .tab_mgr
            .active_tab()
            .and_then(|t| t.cwd())
            .map(std::path::PathBuf::from);

        let is_alt = false;
        let term_h = renderer.terminal_height(is_alt);
        let cols = (renderer.terminal_width(is_alt) as f32 / renderer.cell_width) as u16;
        let lines = (term_h as f32 / renderer.cell_height) as u16;
        let cols = cols.max(2);
        let lines = lines.max(2);

        log::info!(
            "create_tab: {}x{} cols/lines (term_w={}, term_h={}, cell={}x{})",
            cols,
            lines,
            renderer.terminal_width(is_alt),
            term_h,
            renderer.cell_width,
            renderer.cell_height,
        );

        let event_proxy = JsonEventProxy::new(self.proxy.clone());
        let terminal = Terminal::new(
            cols,
            lines,
            renderer.cell_width as u16,
            renderer.cell_height as u16,
            event_proxy,
            shell_path,
            working_directory,
        );

        self.tab_mgr.push(Tab::new_terminal(terminal));
        if let Some(r) = self.renderer.as_mut() {
            r.invalidate_grid_cache();
        };
    }

    /// Command: create a sandbox terminal tab for the given image index.
    pub(crate) fn create_sandbox_tab(&mut self, image_idx: usize) {
        let builtin_count = crate::sandbox::images::IMAGES.len();

        let (display_name, config);
        if image_idx < builtin_count {
            let image = &crate::sandbox::images::IMAGES[image_idx];
            display_name = image.display_name.to_string();
            config = crate::sandbox::config::SandboxConfig::for_image(image.id);
        } else {
            let ci_idx = image_idx - builtin_count;
            match self.config.sandbox.custom_images.get(ci_idx) {
                Some(ci) => {
                    display_name = ci.display_name.clone();
                    config = crate::sandbox::config::SandboxConfig::new(
                        &ci.oci_ref,
                        &ci.oci_ref,
                        &ci.default_shell,
                        &ci.default_workdir,
                    );
                }
                None => {
                    log::error!("[sandbox] Invalid custom image index: {}", ci_idx);
                    return;
                }
            }
        }

        if !self.sandbox_mgr.is_available() {
            log::warn!("[sandbox] Not available on this platform");
            self.toast_mgr.push(
                "Sandbox unavailable — requires macOS Apple Silicon or Linux with KVM".into(),
                crate::ui::components::toast::ToastLevel::Warning,
            );
            return;
        }

        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };

        let is_alt = false;
        let term_h = renderer.terminal_height(is_alt);
        let cols = (renderer.terminal_width(is_alt) as f32 / renderer.cell_width) as u16;
        let lines = (term_h as f32 / renderer.cell_height) as u16;
        let cols = cols.max(2);
        let lines = lines.max(2);

        let mut config = config;
        config.cpus = self.config.sandbox.default_cpus;
        config.memory_mib = self.config.sandbox.default_memory_mib;
        if let Some(cwd) = self.active_terminal().and_then(|t| t.cwd()) {
            config.mount_workspace(std::path::PathBuf::from(cwd));
        } else if let Ok(cwd) = std::env::current_dir() {
            config.mount_workspace(cwd);
        }
        for vol in &self.config.sandbox.volumes {
            config.volumes.push(crate::sandbox::config::VolumeMount {
                guest_path: vol.guest_path.clone(),
                host_path: std::path::PathBuf::from(&vol.host_path),
            });
        }

        let event_proxy = JsonEventProxy::new(self.proxy.clone());

        match SandboxBridge::spawn(
            config,
            cols,
            lines,
            renderer.cell_width as u16,
            renderer.cell_height as u16,
            event_proxy,
            &self.sandbox_mgr,
        ) {
            Ok(bridge) => {
                log::info!("[sandbox] Spawned sandbox tab for '{}'", display_name);
                self.tab_mgr.push(Tab::new_sandbox(bridge));
                if let Some(r) = self.renderer.as_mut() {
                    r.invalidate_grid_cache();
                }
            }
            Err(e) => {
                log::error!("[sandbox] Failed to create sandbox: {}", e);
                self.toast_mgr.push(
                    format!("Sandbox error: {}", e),
                    crate::ui::components::toast::ToastLevel::Error,
                );
            }
        }
    }

    /// Attempt to close tab. If it's an unsaved editor, shows a confirmation dialog instead.
    pub(crate) fn close_tab(
        &mut self,
        idx: usize,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        if idx >= self.tab_mgr.len() {
            return;
        }

        if let Some(tab) = self.tab_mgr.get(idx)
            && let Some(editor) = tab.editor_state()
            && editor.is_modified()
        {
            self.overlay.request_confirm_close(idx);
            return;
        }

        self.force_close_tab(idx, event_loop);
    }

    /// Close tab unconditionally, bypassing unsaved checks.
    pub(crate) fn force_close_tab(
        &mut self,
        idx: usize,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        if idx >= self.tab_mgr.len() {
            return;
        }

        self.overlay.dismiss_confirm_close();

        if let Some(terminal) = self.tab_mgr.get(idx).and_then(|t| t.terminal()) {
            let _ = terminal
                .sender
                .send(alacritty_terminal::event_loop::Msg::Shutdown);
        }
        let empty = self.tab_mgr.remove(idx);

        if empty {
            event_loop.exit();
            return;
        }

        if let Some(r) = self.renderer.as_mut() {
            r.invalidate_grid_cache();
        };
    }

    /// Open a file in an editor tab, or switch to an existing editor for that path.
    pub(crate) fn open_file_in_editor(&mut self, path: &std::path::Path) {
        if let Some(idx) = self.tab_mgr.find_editor(path) {
            self.tab_mgr.switch_to(idx);
            if let Some(r) = self.renderer.as_mut() {
                r.invalidate_grid_cache();
            }
            return;
        }

        if self.is_editor_tab_limit_reached() {
            let limit = crate::usage::Feature::EditorTabs.free_limit();
            self.show_limit_reached(crate::usage::Feature::EditorTabs, limit, limit);
            return;
        }

        match EditorState::open(path, Some(&mut self.syntax)) {
            Ok(mut state) => {
                state.refresh_highlights(&self.syntax);
                self.tab_mgr.push(Tab::new_editor(state));
                if let Some(r) = self.renderer.as_mut() {
                    r.invalidate_grid_cache();
                }
            }
            Err(e) => {
                log::error!("Failed to open file {:?}: {}", path, e);
            }
        }
    }

    /// Open a file in the editor with side-by-side git diff view.
    pub(crate) fn open_diff_in_editor(
        &mut self,
        path: &std::path::Path,
        hunks: &[crate::git::DiffHunkData],
    ) {
        use crate::ui::editor::EditorState;

        if let Some(idx) = self.tab_mgr.find_editor(path) {
            self.tab_mgr.switch_to(idx);
            if let Some(tab) = self.tab_mgr.active_tab_mut()
                && let Some(editor) = tab.editor_state_mut()
            {
                editor.diff_view = Some(crate::ui::editor::build_diff_rows(hunks));
            }
            if let Some(r) = self.renderer.as_mut() {
                r.invalidate_grid_cache();
            }
            return;
        }

        if self.is_editor_tab_limit_reached() {
            let limit = crate::usage::Feature::EditorTabs.free_limit();
            self.show_limit_reached(crate::usage::Feature::EditorTabs, limit, limit);
            return;
        }

        match EditorState::open_diff(path, hunks, Some(&mut self.syntax)) {
            Ok(mut state) => {
                state.refresh_highlights(&self.syntax);
                self.tab_mgr.push(Tab::new_editor(state));
                if let Some(r) = self.renderer.as_mut() {
                    r.invalidate_grid_cache();
                }
            }
            Err(e) => {
                log::error!("Failed to open diff for {:?}: {}", path, e);
            }
        }
    }

    /// Handle a file tree context menu action (new file, new folder, rename, delete).
    pub(crate) fn handle_file_tree_context_action(
        &mut self,
        action_id: &str,
        path: &std::path::Path,
    ) {
        match action_id {
            "new_file" => {
                let dir = if path.is_dir() {
                    path
                } else {
                    path.parent().unwrap_or(path)
                };
                let target = dir.join("untitled");
                match std::fs::File::create(&target) {
                    Ok(_) => {
                        self.reload_file_tree_at(dir);
                        self.open_file_in_editor(&target);
                    }
                    Err(e) => log::error!("Failed to create file: {}", e),
                }
            }
            "new_folder" => {
                let dir = if path.is_dir() {
                    path
                } else {
                    path.parent().unwrap_or(path)
                };
                let target = dir.join("new_folder");
                match std::fs::create_dir(&target) {
                    Ok(_) => self.reload_file_tree_at(dir),
                    Err(e) => log::error!("Failed to create folder: {}", e),
                }
            }
            "delete" => {
                let result = if path.is_dir() {
                    std::fs::remove_dir_all(path)
                } else {
                    std::fs::remove_file(path)
                };
                match result {
                    Ok(_) => {
                        if let Some(parent) = path.parent() {
                            self.reload_file_tree_at(parent);
                        }
                    }
                    Err(e) => log::error!("Failed to delete {:?}: {}", path, e),
                }
            }
            "rename" => {
                if let Some(idx) = self.file_tree.index_for_path(path) {
                    self.file_tree.begin_rename(idx);
                    self.pending_redraw = true;
                }
            }
            "open" => {
                if !path.is_dir() {
                    self.open_file_in_editor(path);
                }
            }
            "reveal_in_finder" => {
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open")
                        .arg("-R")
                        .arg(path)
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(path.parent().unwrap_or(path))
                        .spawn();
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("explorer")
                        .arg("/select,")
                        .arg(path)
                        .spawn();
                }
            }
            _ => log::warn!("Unknown context action: {}", action_id),
        }
    }

    /// Dispatch a git panel context-menu action by id.
    pub(crate) fn handle_git_context_action(
        &mut self,
        action_id: &str,
        path: &std::path::Path,
        event_loop: &ActiveEventLoop,
    ) {
        let rel = path.to_string_lossy().to_string();
        let action = match action_id {
            "git_discard" => {
                Some(crate::app::actions::AppAction::GitDiscardFileChanges { path: rel })
            }
            "git_gitignore" => {
                Some(crate::app::actions::AppAction::GitAddToGitignore { path: rel })
            }
            "git_open" => Some(crate::app::actions::AppAction::GitOpenFile { path: rel }),
            "git_reveal" => Some(crate::app::actions::AppAction::GitRevealInFinder { path: rel }),
            _ => {
                log::warn!("Unknown git context action: {action_id}");
                None
            }
        };
        if let Some(a) = action {
            self.dispatch(a, event_loop);
        }
    }

    /// Reload the file tree at a specific directory (after add/delete).
    pub(crate) fn reload_file_tree_at(&mut self, dir: &std::path::Path) {
        if let Some(root) = &self.file_tree.root {
            let root_path = root.path.clone();
            if dir.starts_with(&root_path) {
                if let Some(root_mut) = &mut self.file_tree.root {
                    crate::ui::file_tree::load_children_at(root_mut, dir);
                }
                self.file_tree.rebuild_cache();
            }
        }
    }

    pub(crate) fn refresh_file_tree(&mut self) {
        let expanded: Vec<std::path::PathBuf> = self.file_tree.expanded.iter().cloned().collect();
        for dir in &expanded {
            if let Some(root) = &mut self.file_tree.root {
                crate::ui::file_tree::load_children_at(root, dir);
            }
        }
        self.file_tree.rebuild_cache();
        let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
        self.file_tree
            .update_git_ignored(std::path::Path::new(&cwd));
        self.file_tree.rebuild_cache();
    }

    /// Open a live terminal tab pre-populated with the history of a past session.
    ///
    /// **Deduplication**: if a tab for this session already exists, switch to it
    /// instead of spawning a new terminal. This prevents duplicate tabs and
    /// resource leaks from repeated clicks.
    pub(crate) fn open_or_switch_session(&mut self, session_id: crate::session::SessionId) {
        if let Some(idx) = self.tab_mgr.index_for_session(session_id) {
            self.tab_mgr.switch_to(idx);
            if let Some(r) = self.renderer.as_mut() {
                r.invalidate_grid_cache();
            }
            return;
        }
        self.create_session_tab(session_id);
    }

    /// Command: spawn a new terminal tab pre-populated with session history.
    fn create_session_tab(&mut self, session_id: crate::session::SessionId) {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };
        let session = match self.session_mgr.get_session(session_id) {
            Some(s) => s,
            None => return,
        };
        let block_list = session.to_block_list();

        let is_alt = false;
        let term_h = renderer.terminal_height(is_alt);
        let cols = (renderer.terminal_width(is_alt) as f32 / renderer.cell_width) as u16;
        let lines = (term_h as f32 / renderer.cell_height) as u16;
        let cols = cols.max(2);
        let lines = lines.max(2);

        let event_proxy = JsonEventProxy::new(self.proxy.clone());
        let terminal = Terminal::new(
            cols,
            lines,
            renderer.cell_width as u16,
            renderer.cell_height as u16,
            event_proxy,
            None,
            None,
        );

        self.tab_mgr.push(Tab::new_terminal_with_blocks(
            terminal, session_id, block_list,
        ));
        if let Some(r) = self.renderer.as_mut() {
            r.invalidate_grid_cache();
        };
    }

    pub(crate) fn build_tab_infos(&self) -> Vec<TabInfo> {
        self.tab_mgr.build_tab_infos()
    }

    /// Compute the PTY window size for a tab, given its current alt-screen
    /// state (which determines the grid padding).
    pub(crate) fn compute_window_size(renderer: &Renderer, is_alt: bool) -> WindowSize {
        let term_h = renderer.terminal_height(is_alt);
        let cols = (renderer.terminal_width(is_alt) as f32 / renderer.cell_width).max(2.0) as u16;
        let lines = (term_h as f32 / renderer.cell_height).max(2.0) as u16;
        WindowSize {
            num_cols: cols,
            num_lines: lines,
            cell_width: renderer.cell_width as u16,
            cell_height: renderer.cell_height as u16,
        }
    }

    /// Check each terminal tab for an app-controlled mode toggle since the
    /// last sync, and resize any whose mode has changed.
    pub(crate) fn sync_app_state(&mut self) -> bool {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return false,
        };
        let mut any = false;
        let active_idx = self.tab_mgr.active_index();
        for i in 0..self.tab_mgr.len() {
            let tab = match self.tab_mgr.get(i) {
                Some(t) => t,
                None => continue,
            };
            let (now_app, prev) = match &tab.kind {
                TabKind::Terminal {
                    terminal, is_alt, ..
                } => (terminal.is_app_controlled(), *is_alt),
                _ => continue,
            };
            if now_app != prev {
                let ws = Self::compute_window_size(renderer, now_app);
                log::info!(
                    "tab {} app_controlled {} -> {}: resize to {}x{} (cell {}x{})",
                    i,
                    prev,
                    now_app,
                    ws.num_cols,
                    ws.num_lines,
                    ws.cell_width,
                    ws.cell_height
                );
                if let Some(tab) = self.tab_mgr.get_mut(i)
                    && let TabKind::Terminal {
                        terminal,
                        block_list,
                        is_alt,
                        ..
                    } = &mut tab.kind
                {
                    terminal.resize(ws);
                    *is_alt = now_app;
                    if !now_app && i == active_idx {
                        block_list.finish_app_block(terminal);
                    }
                }
                any = true;
            }
        }
        any
    }

    /// Update the renderer's panel insets and resize all terminal tabs
    /// to match the new available width.
    pub(crate) fn sync_panel_insets(&mut self) {
        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };
        let sf = renderer.scale_factor as f32;
        let left = if self.overlay.sidebar_open {
            self.panel_layout.left_physical_width(sf) as u32
        } else {
            0
        };
        let right = if self.overlay.git_panel_open {
            self.panel_layout.right_physical_width(sf) as u32
        } else {
            0
        };
        if renderer.panel_inset_left == left && renderer.panel_inset_right == right {
            return;
        }
        renderer.panel_inset_left = left;
        renderer.panel_inset_right = right;
        renderer.invalidate_grid_cache();

        for tab in self.tab_mgr.iter() {
            match &tab.kind {
                super::TabKind::Terminal {
                    terminal, is_alt, ..
                } => {
                    let ws = Self::compute_window_size(renderer, *is_alt);
                    terminal.resize(ws);
                }
                super::TabKind::Sandbox { bridge, .. } => {
                    let ws = Self::compute_window_size(renderer, false);
                    let term_size = crate::terminal::TermSize(ws);
                    let mut t = bridge.term.lock();
                    t.resize(term_size);
                }
                _ => {}
            }
        }
    }

    /// Convenience: get the active tab's terminal (if it is one).
    pub(crate) fn active_terminal(&self) -> Option<&Terminal> {
        self.tab_mgr.active_terminal()
    }

    /// Convenience: get the active tab's block list (if terminal).
    pub(crate) fn active_block_list(&self) -> Option<&BlockList> {
        self.tab_mgr.active_block_list()
    }

    /// Convenience: get the active tab's block list mutably.
    pub(crate) fn active_block_list_mut(&mut self) -> Option<&mut BlockList> {
        self.tab_mgr.active_block_list_mut()
    }

    /// Best-effort CWD: active terminal → editor file parent → any terminal.
    pub(crate) fn resolve_cwd(&self) -> Option<String> {
        if let Some(cwd) = self.active_terminal().and_then(|t| t.cwd()) {
            return Some(cwd);
        }
        if let Some(tab) = self.tab_mgr.active_tab()
            && let Some(path) = tab.editor_path()
            && let Some(parent) = path.parent()
        {
            return Some(parent.to_string_lossy().into_owned());
        }
        for i in 0..self.tab_mgr.len() {
            if let Some(t) = self.tab_mgr.get(i)
                && let Some(cwd) = t.cwd()
            {
                return Some(cwd);
            }
        }
        None
    }

    fn is_editor_tab_limit_reached(&self) -> bool {
        if self.usage_tracker.is_pro() {
            return false;
        }
        let mut count = 0u32;
        for i in 0..self.tab_mgr.len() {
            if let Some(t) = self.tab_mgr.get(i)
                && matches!(t.kind, super::TabKind::Editor { .. })
            {
                count += 1;
            }
        }
        count >= crate::usage::Feature::EditorTabs.free_limit()
    }

    pub(crate) fn show_limit_reached(
        &mut self,
        feature: crate::usage::Feature,
        used: u32,
        limit: u32,
    ) {
        self.usage_limit_banner.show();
        self.toast_mgr.push(
            format!("{} — limit reached ({}/{})", feature.label(), used, limit),
            crate::ui::components::toast::ToastLevel::Warning,
        );
    }

    pub(crate) fn maybe_show_first_use_hint(&mut self, feature: crate::usage::Feature) {
        if self.usage_tracker.is_pro() {
            return;
        }
        let hints = &self.config.general.hints;
        let already_seen = match feature {
            crate::usage::Feature::Ask => hints.seen_ask,
            crate::usage::Feature::Agent => hints.seen_agent,
            crate::usage::Feature::Sandbox => hints.seen_sandbox,
            crate::usage::Feature::Git => hints.seen_git,
            crate::usage::Feature::EditorTabs => return,
        };
        if already_seen {
            return;
        }
        match feature {
            crate::usage::Feature::Ask => self.config.general.hints.seen_ask = true,
            crate::usage::Feature::Agent => self.config.general.hints.seen_agent = true,
            crate::usage::Feature::Sandbox => self.config.general.hints.seen_sandbox = true,
            crate::usage::Feature::Git => self.config.general.hints.seen_git = true,
            crate::usage::Feature::EditorTabs => {}
        }
        self.config.save();
        let limit = feature.free_limit();
        let label = feature.label();
        self.toast_mgr.push(
            format!("{label}: {limit} free uses/day (Pro = unlimited)"),
            crate::ui::components::toast::ToastLevel::Info,
        );
    }

    /// Command: send input bytes to the active tab (terminal or sandbox).
    pub(crate) fn buf_size(&self) -> (usize, usize) {
        self.renderer
            .as_ref()
            .map(|r| (r.width as usize, r.height as usize))
            .unwrap_or((0, 0))
    }

    pub(crate) fn scale_factor(&self) -> f32 {
        self.renderer
            .as_ref()
            .map(|r| r.scale_factor as f32)
            .unwrap_or(1.0)
    }

    pub(crate) fn total_banners_height(&self, sf: f32) -> usize {
        let hint = crate::ui::components::hint_banner::banner_height(&self.hint_banner, sf);
        let gap = if hint > 0 { (10.0 * sf) as usize } else { 0 };
        hint + gap
    }

    pub(crate) fn send_input_to_active(&self, data: &[u8]) {
        if let Some(tab) = self.tab_mgr.active_tab() {
            match &tab.kind {
                TabKind::Terminal { terminal, .. } => {
                    terminal.input(std::borrow::Cow::Owned(data.to_vec()));
                }
                TabKind::Sandbox { bridge, .. } => {
                    bridge.input(data.to_vec());
                }
                _ => {}
            }
        }
    }

    /// Query: is the active tab a sandbox tab?
    pub(crate) fn is_sandbox_active(&self) -> bool {
        self.tab_mgr
            .active_tab()
            .map(|t| matches!(&t.kind, TabKind::Sandbox { .. }))
            .unwrap_or(false)
    }

    /// Query: is the active tab an editor tab?
    pub(crate) fn is_editor_active(&self) -> bool {
        self.tab_mgr
            .active_tab()
            .map(|t| t.route() == super::router::Route::Editor)
            .unwrap_or(false)
    }

    /// Query: read-only access to the active editor state.
    pub(crate) fn active_editor_state(&self) -> Option<&EditorState> {
        self.tab_mgr.active_tab().and_then(|t| t.editor_state())
    }

    /// Command: mutable access to the active editor state.
    pub(crate) fn active_editor_state_mut(&mut self) -> Option<&mut EditorState> {
        self.tab_mgr
            .active_tab_mut()
            .and_then(|t| t.editor_state_mut())
    }

    /// Record the last finished block of the active tab into the session manager.
    /// Creates the session lazily on first command.
    ///
    /// Skips blocks with `restored == true` to avoid re-recording history
    /// blocks that were loaded from a saved session.
    pub(crate) fn record_last_block(&mut self) {
        let tab = match self.tab_mgr.active_tab() {
            Some(t) => t,
            None => return,
        };
        let block_list = match tab.block_list() {
            Some(bl) => bl,
            None => return,
        };
        let block = match block_list.blocks.last() {
            Some(b) if b.duration.is_some() && !b.restored => b,
            _ => return,
        };

        let existing_session_id = tab.session_id;
        let block_clone = block.clone();

        let session_id = match existing_session_id {
            Some(id) => id,
            None => {
                let id = self.session_mgr.create_session();
                if let Some(tab) = self.tab_mgr.active_tab_mut() {
                    tab.session_id = Some(id);
                }
                id
            }
        };

        self.session_mgr
            .record_block(session_id, &block_clone, &block_clone.prompt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_settings_tab() -> Tab {
        Tab::new_settings()
    }

    fn make_models_tab() -> Tab {
        Tab::new_models()
    }

    #[test]
    fn new_tab_manager_is_empty() {
        let mgr = TabManager::new();
        assert_eq!(mgr.len(), 0);
        assert_eq!(mgr.len(), 0);
        assert_eq!(mgr.active_index(), 0);
        assert!(mgr.active_tab().is_none());
    }

    #[test]
    fn push_sets_active() {
        let mut mgr = TabManager::new();
        let idx0 = mgr.push(make_settings_tab());
        assert_eq!(idx0, 0);
        assert_eq!(mgr.active_index(), 0);
        assert_eq!(mgr.len(), 1);

        let idx1 = mgr.push(make_models_tab());
        assert_eq!(idx1, 1);
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn switch_to_valid_and_invalid() {
        let mut mgr = TabManager::new();
        mgr.push(make_settings_tab());
        mgr.push(make_models_tab());

        mgr.switch_to(0);
        assert_eq!(mgr.active_index(), 0);

        mgr.switch_to(99);
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn remove_adjusts_active() {
        let mut mgr = TabManager::new();
        mgr.push(make_settings_tab());
        mgr.push(make_models_tab());
        mgr.push(make_settings_tab());

        let empty = mgr.remove(2);
        assert!(!empty);
        assert_eq!(mgr.active_index(), 1);
        assert_eq!(mgr.len(), 2);
    }

    #[test]
    fn remove_last_returns_true() {
        let mut mgr = TabManager::new();
        mgr.push(make_settings_tab());
        let empty = mgr.remove(0);
        assert!(empty);
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn previous_and_next_wrap() {
        let mut mgr = TabManager::new();
        mgr.push(make_settings_tab());
        mgr.push(make_models_tab());
        mgr.push(make_settings_tab());
        mgr.switch_to(0);

        mgr.previous();
        assert_eq!(mgr.active_index(), 2);

        mgr.next();
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn reorder_moves_tab() {
        let mut mgr = TabManager::new();
        mgr.push(make_settings_tab());
        mgr.push(make_models_tab());
        mgr.push(make_settings_tab());

        mgr.reorder(0, 2);
        assert_eq!(mgr.active_index(), 2);
        assert!(mgr.get(0).unwrap().is_models());
    }

    #[test]
    fn find_settings_and_models() {
        let mut mgr = TabManager::new();
        assert!(mgr.find_settings().is_none());
        assert!(mgr.find_models().is_none());

        mgr.push(make_settings_tab());
        mgr.push(make_models_tab());

        assert_eq!(mgr.find_settings(), Some(0));
        assert_eq!(mgr.find_models(), Some(1));
    }

    #[test]
    fn build_tab_infos_marks_active() {
        let mut mgr = TabManager::new();
        mgr.push(make_settings_tab());
        mgr.push(make_models_tab());
        mgr.switch_to(0);

        let infos = mgr.build_tab_infos();
        assert_eq!(infos.len(), 2);
        assert!(infos[0].is_active);
        assert!(!infos[1].is_active);
    }
}
