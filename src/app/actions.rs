//! Application action dispatcher.
//!
//! Every user-initiated state mutation is encoded as an [`AppAction`].
//! Event handlers produce actions, and [`App::dispatch`] executes them.

use super::tabs::TabKind;
use crate::agent::session::ApprovalDecision;
use crate::session::SessionId;

/// A discrete, side-effect-producing operation on the application state.
///
/// Actions are the only way UI event handlers should request state changes.
#[derive(Debug, Clone, PartialEq)]
pub enum AppAction {
    /// Open a session tab, or switch to it if one already exists.
    OpenSession {
        session_id: SessionId,
    },
    /// Delete a session from history.
    ClearSession {
        session_id: SessionId,
    },

    /// Create a new terminal tab, optionally with a specific shell.
    CreateTab {
        shell_path: Option<String>,
    },
    /// Create a new sandbox terminal tab with the given image index.
    CreateSandboxTab {
        image_idx: usize,
    },
    /// Close the tab at the given index.
    CloseTab {
        index: usize,
    },
    /// Force-close a tab without saving (from confirm dialog "Don't Save").
    ForceCloseTab {
        index: usize,
    },
    /// Save and then close a tab (from confirm dialog "Save").
    SaveAndCloseTab {
        index: usize,
    },
    /// Dismiss the unsaved-close confirmation dialog.
    DismissConfirmClose,
    /// Switch to the tab at the given index.
    SwitchTab {
        index: usize,
    },
    /// Switch to the previous tab.
    PreviousTab,
    /// Switch to the next tab.
    NextTab,

    /// Toggle the sessions side panel.
    ToggleSidebar,
    /// Toggle the git (right) panel.
    ToggleGitPanel,
    /// Switch the git panel sub-tab.
    SwitchGitPanelTab {
        tab: crate::ui::panel_layout::GitPanelTab,
    },
    /// Git: stage a file by index.
    GitStageFile {
        index: usize,
    },
    /// Git: unstage a file by index.
    GitUnstageFile {
        index: usize,
    },
    /// Git: stage all files.
    GitStageAll,
    /// Git: unstage all files.
    GitUnstageAll,
    /// Git: checkout a branch by index.
    GitCheckoutBranch {
        index: usize,
    },
    /// Git: focus the commit message input and position cursor at click.
    GitFocusCommitInput {
        rel_x: f64,
        rel_y: f64,
    },
    /// Git: generate AI commit message from staged diff.
    GitGenerateCommitMessage,
    /// Git: cancel ongoing AI commit message generation.
    GitCancelGenerateCommitMessage,
    /// Git: commit staged changes.
    GitCommit,
    /// Git: open file diff in editor.
    GitOpenFileDiff {
        index: usize,
    },
    /// Git: discard working-tree changes for a file by path.
    GitDiscardFileChanges {
        path: String,
    },
    /// Git: add a file pattern to .gitignore.
    GitAddToGitignore {
        path: String,
    },
    /// Git: open a file in the editor.
    GitOpenFile {
        path: String,
    },
    /// Git: reveal a file in the system file manager.
    GitRevealInFinder {
        path: String,
    },
    /// Switch the side panel sub-tab.
    SwitchPanelTab {
        tab: crate::ui::panel_layout::SidePanelTab,
    },
    /// Toggle expand/collapse of a file tree directory node.
    ToggleFileTreeNode {
        path: std::path::PathBuf,
    },
    /// Open a file from the file tree in the default editor.
    OpenFile {
        path: std::path::PathBuf,
    },
    OpenFileAtLine {
        path: std::path::PathBuf,
        line: Option<u32>,
    },
    FocusSearchInput,
    /// Toggle the debug overlay.
    ToggleDebugPanel,
    /// Open the command palette.
    OpenPalette,
    /// Toggle the shell picker dropdown.
    ToggleShellPicker,
    /// Close the shell picker without selection.
    CloseShellPicker,
    /// Toggle the user/avatar menu.
    ToggleUserMenu,
    /// Close all transient overlays (palette, pickers, menus).
    CloseAllOverlays,
    /// Handle a context menu action by id.
    ContextMenuAction {
        id: String,
        path: std::path::PathBuf,
    },

    /// Open the settings view as a tab (or focus existing).
    OpenSettings,
    /// Close the settings tab.
    CloseSettings,
    /// Open the models view as a tab.
    OpenModels,
    /// Close the models tab.
    CloseModels,

    CloseUsagePanel,
    OpenProPanel,
    CloseProPanel,
    ActivateLicense,
    DeactivateLicense,
    BuyPro,
    DismissUsageLimitBanner,

    /// Load a model by its registry index.
    LoadModel {
        index: usize,
    },
    /// Cancel ongoing AI inference.
    CancelInference,

    /// Start an agent session with a natural-language task.
    StartAgent {
        task: String,
    },
    /// User approved/rejected a pending agent tool call.
    AgentApproval {
        decision: ApprovalDecision,
    },
    /// Enter persistent agent input mode.
    EnterAgentMode,
    /// Exit persistent agent input mode.
    ExitAgentMode,

    /// Dismiss the currently visible hint banner.
    DismissHintBanner,

    /// Stop and close the active sandbox tab.
    StopSandbox,

    Copy,
    Paste,
    Cut,
    SelectAll,
    Undo,
    Redo,

    /// Download the available update.
    DownloadUpdate,
    /// Install the downloaded update and relaunch.
    InstallUpdate,
    /// Toggle the update dropdown beneath the tab-bar badge.
    ToggleUpdateDropdown,
}

impl super::App {
    /// Central action dispatcher.
    ///
    /// All state mutations flow through here.
    pub(crate) fn dispatch(
        &mut self,
        action: AppAction,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        log::info!("dispatch: {:?}", action);
        match action {
            AppAction::OpenSession { session_id } => {
                self.open_or_switch_session(session_id);
            }
            AppAction::ClearSession { session_id } => {
                self.session_mgr.clear_session(session_id);
            }

            AppAction::CreateTab { shell_path } => {
                self.create_tab(shell_path.as_deref());
            }
            AppAction::CreateSandboxTab { image_idx } => {
                let result = self.usage_tracker.can_use(crate::usage::Feature::Sandbox);
                if let crate::usage::UsageResult::Denied { used, limit } = result {
                    self.show_limit_reached(crate::usage::Feature::Sandbox, used, limit);
                    return;
                }
                self.maybe_show_first_use_hint(crate::usage::Feature::Sandbox);
                self.usage_tracker
                    .record_use(crate::usage::Feature::Sandbox);
                self.create_sandbox_tab(image_idx);
            }
            AppAction::CloseTab { index } => {
                self.close_tab(index, event_loop);
            }
            AppAction::ForceCloseTab { index } => {
                self.force_close_tab(index, event_loop);
            }
            AppAction::SaveAndCloseTab { index } => {
                if let Some(tab) = self.tab_mgr.get_mut(index)
                    && let Some(editor) = tab.editor_state_mut()
                    && let Err(e) = editor.save()
                {
                    log::error!("Failed to save file: {}", e);
                    self.overlay.dismiss_confirm_close();
                    return;
                }
                self.force_close_tab(index, event_loop);
            }
            AppAction::DismissConfirmClose => {
                self.overlay.dismiss_confirm_close();
            }
            AppAction::SwitchTab { index } => {
                self.tab_mgr.switch_to(index);
                if let Some(r) = self.renderer.as_mut() {
                    r.invalidate_grid_cache();
                }
            }
            AppAction::PreviousTab => {
                self.tab_mgr.previous();
                if let Some(r) = self.renderer.as_mut() {
                    r.invalidate_grid_cache();
                }
            }
            AppAction::NextTab => {
                self.tab_mgr.next();
                if let Some(r) = self.renderer.as_mut() {
                    r.invalidate_grid_cache();
                }
            }

            AppAction::ToggleSidebar => {
                self.overlay.sidebar_open = !self.overlay.sidebar_open;
                self.sync_panel_insets();
            }
            AppAction::ToggleGitPanel => {
                self.overlay.toggle_git_panel();
                if self.overlay.git_panel_open {
                    let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                    self.git_panel.refresh(&cwd);
                }
                self.sync_panel_insets();
            }
            AppAction::SwitchGitPanelTab { tab } => {
                self.panel_layout.switch_git_tab(tab);
                self.git_panel.scroll_offset = 0.0;
                let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                self.git_panel.refresh(&cwd);
            }
            AppAction::GitStageFile { index } => {
                if let Some(entry) = self.git_panel.data.entries.get(index) {
                    let path = entry.path.clone();
                    let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                    if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                        if let Err(e) = repo.stage_file(&path) {
                            log::warn!("git stage failed: {e}");
                        }
                        self.git_panel.refresh(&cwd);
                    }
                }
            }
            AppAction::GitUnstageFile { index } => {
                if let Some(entry) = self.git_panel.data.entries.get(index) {
                    let path = entry.path.clone();
                    let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                    if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                        if let Err(e) = repo.unstage_file(&path) {
                            log::warn!("git unstage failed: {e}");
                        }
                        self.git_panel.refresh(&cwd);
                    }
                }
            }
            AppAction::GitCheckoutBranch { index } => {
                if let Some(branch) = self.git_panel.data.branches.get(index) {
                    let name = branch.name.clone();
                    let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                    if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                        if let Err(e) = repo.checkout_branch(&name) {
                            log::warn!("git checkout failed: {e}");
                        }
                        self.git_panel.refresh(&cwd);
                    }
                }
            }
            AppAction::GitStageAll => {
                let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                    if let Err(e) = repo.stage_all() {
                        log::warn!("git stage all failed: {e}");
                    }
                    self.git_panel.refresh(&cwd);
                }
            }
            AppAction::GitUnstageAll => {
                let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                    if let Err(e) = repo.unstage_all() {
                        log::warn!("git unstage all failed: {e}");
                    }
                    self.git_panel.refresh(&cwd);
                }
            }
            AppAction::GitFocusCommitInput { rel_x, rel_y } => {
                self.git_panel.commit_input_focused = true;
                let sf = self
                    .renderer
                    .as_ref()
                    .map(|r| r.scale_factor)
                    .unwrap_or(1.0);
                let char_w = 7.0 * sf;
                let panel_w = self.panel_layout.right_physical_width(sf as f32) as f64;
                let input_pad_x = crate::ui::components::git_panel::COMMIT_INPUT_PAD_X as f64 * sf;
                let text_max_px = panel_w - input_pad_x * 2.0 - 8.0 * sf - 16.0 * sf - 16.0 * sf;
                let max_chars = (text_max_px / char_w).floor().max(1.0) as usize;
                self.git_panel
                    .cursor_from_click(rel_x, rel_y, char_w, max_chars);
            }
            AppAction::GitGenerateCommitMessage => {
                let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                    let diff = repo.staged_diff_summary();
                    if diff.is_empty() {
                        log::info!("GitGenerateCommitMessage: no staged changes");
                        self.git_panel.commit_message = "No staged changes".into();
                        return;
                    }
                    log::info!("GitGenerateCommitMessage: diff len={}", diff.len());
                    let handle = match self.ai_ctrl.state.loaded_model.take() {
                        Some(h) => {
                            log::info!(
                                "GitGenerateCommitMessage: model available, starting inference"
                            );
                            h
                        }
                        None => {
                            log::info!(
                                "GitGenerateCommitMessage: no model loaded, setting pending"
                            );
                            self.git_panel.pending_generate_commit_msg = true;
                            self.git_panel.generating_commit_msg = true;
                            self.git_panel.commit_message.clear();
                            if !self.auto_load_best_efficient() {
                                self.auto_download_default_model("generate commit message");
                            }
                            return;
                        }
                    };
                    let system = "You write git commit messages. Output ONLY the message, nothing else. Format: <type>: <short summary>. Types: feat, fix, refactor, chore, docs, style, test. Example: \"refactor: simplify user auth flow\". Max 72 chars. Plain English. No code, no file names, no markdown, no quotes, no explanation.";
                    let user_msg =
                        format!("Summarize these staged changes in one commit message:\n\n{diff}");
                    let fallback = self.ai_ctrl.fallback_template();
                    let prompt = self.ai_ctrl.state.build_custom_prompt(
                        system,
                        &[("user", &user_msg)],
                        &handle.model,
                        fallback,
                    );
                    self.ai_ctrl.state.begin_assistant_message();
                    let proxy = self.proxy.clone();
                    let cancel = self.ai_ctrl.arm_cancel();
                    let (tx, rx) = std::sync::mpsc::channel();
                    let inf_handle =
                        crate::ai::inference::run_inference(handle, prompt, tx, proxy, cancel);
                    self.ai_ctrl.state.inference_rx = Some(rx);
                    self.ai_ctrl.state.inference_handle = Some(inf_handle);
                    self.git_panel.generating_commit_msg = true;
                }
            }
            AppAction::GitCancelGenerateCommitMessage => {
                self.ai_ctrl.cancel_inference();
                self.git_panel.generating_commit_msg = false;
                self.git_panel.pending_generate_commit_msg = false;
            }
            AppAction::GitCommit => {
                let msg = self.git_panel.commit_message.trim().to_string();
                if msg.is_empty() {
                    return;
                }
                let result = self.usage_tracker.can_use(crate::usage::Feature::Git);
                if let crate::usage::UsageResult::Denied { used, limit } = result {
                    self.show_limit_reached(crate::usage::Feature::Git, used, limit);
                    return;
                }
                self.maybe_show_first_use_hint(crate::usage::Feature::Git);
                self.usage_tracker.record_use(crate::usage::Feature::Git);
                let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                    match repo.commit(&msg) {
                        Ok(()) => {
                            self.git_panel.commit_message.clear();
                            self.git_panel.refresh(&cwd);
                        }
                        Err(e) => {
                            log::warn!("git commit failed: {e}");
                        }
                    }
                }
            }
            AppAction::GitOpenFileDiff { index } => {
                if let Some(entry) = self.git_panel.data.entries.get(index) {
                    let path = entry.path.clone();
                    let staged = entry.staged;
                    let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                    if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                        let workdir = repo
                            .workdir_path()
                            .unwrap_or_else(|| std::path::PathBuf::from(&cwd));
                        match repo.diff_for_file_hunks(&path, staged) {
                            Ok(hunks) => {
                                let abs = workdir.join(&path);
                                self.open_diff_in_editor(&abs, &hunks);
                            }
                            Err(e) => {
                                log::warn!("git diff failed: {e}");
                            }
                        }
                    }
                }
            }
            AppAction::GitDiscardFileChanges { path } => {
                let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                    if let Err(e) = repo.discard_file_changes(&path) {
                        log::warn!("git discard changes failed: {e}");
                    }
                    self.git_panel.refresh(&cwd);
                }
            }
            AppAction::GitAddToGitignore { path } => {
                let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                    if let Err(e) = repo.add_to_gitignore(&path) {
                        log::warn!("git add to gitignore failed: {e}");
                    }
                    self.git_panel.refresh(&cwd);
                }
            }
            AppAction::GitOpenFile { path } => {
                let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                let abs = if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                    repo.workdir_path()
                        .unwrap_or_else(|| std::path::PathBuf::from(&cwd))
                        .join(&path)
                } else {
                    std::path::PathBuf::from(&cwd).join(&path)
                };
                if abs.exists() && !abs.is_dir() {
                    self.open_file_in_editor(&abs);
                }
            }
            AppAction::GitRevealInFinder { path } => {
                let cwd = self.resolve_cwd().unwrap_or_else(|| ".".into());
                let abs = if let Some(repo) = crate::git::GitRepo::discover(&cwd) {
                    repo.workdir_path()
                        .unwrap_or_else(|| std::path::PathBuf::from(&cwd))
                        .join(&path)
                } else {
                    std::path::PathBuf::from(&cwd).join(&path)
                };
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open")
                        .arg("-R")
                        .arg(&abs)
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(abs.parent().unwrap_or(&abs))
                        .spawn();
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("explorer")
                        .arg("/select,")
                        .arg(&abs)
                        .spawn();
                }
            }
            AppAction::SwitchPanelTab { tab } => {
                self.panel_layout.switch_tab(tab);
                if tab == crate::ui::panel_layout::SidePanelTab::Files {
                    let cwd = self
                        .resolve_cwd()
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|| {
                            std::env::current_dir()
                                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        });
                    self.file_tree.load(&cwd);
                }
                if tab == crate::ui::panel_layout::SidePanelTab::Search {
                    self.search_panel.focused = true;
                    let cwd = self
                        .resolve_cwd()
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|| {
                            std::env::current_dir()
                                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        });
                    self.search_panel.set_root(&cwd);
                } else {
                    self.search_panel.focused = false;
                }
            }
            AppAction::ToggleFileTreeNode { path } => {
                self.file_tree.toggle_expand(&path);
            }
            AppAction::OpenFile { path } => {
                self.open_file_in_editor(&path);
            }
            AppAction::OpenFileAtLine { path, line } => {
                let search_term = if !self.search_panel.query.is_empty() {
                    Some(self.search_panel.query.clone())
                } else {
                    None
                };
                self.open_file_in_editor(&path);
                if let Some(ed) = self.active_editor_state_mut() {
                    ed.search_highlight = search_term;
                }
                if let Some(line_num) = line {
                    let sf = self
                        .renderer
                        .as_ref()
                        .map(|r| r.scale_factor as f32)
                        .unwrap_or(1.0);
                    let content_h = self
                        .renderer
                        .as_ref()
                        .map(|r| (r.height as usize).saturating_sub(r.tab_bar_height as usize))
                        .unwrap_or(600);
                    if let Some(ed) = self.active_editor_state_mut() {
                        let target = (line_num as usize).saturating_sub(1);
                        ed.set_cursor_pos(target, 0);
                        ed.ensure_cursor_visible(sf, content_h);
                    }
                }
            }
            AppAction::FocusSearchInput => {
                self.search_panel.focused = true;
            }
            AppAction::ToggleDebugPanel => {
                self.overlay.debug_panel = !self.overlay.debug_panel;
            }
            AppAction::OpenPalette => {
                self.overlay.open_palette();
            }
            AppAction::ToggleShellPicker => {
                self.overlay.toggle_shell_picker();
            }
            AppAction::CloseShellPicker => {
                self.overlay.close_shell_picker();
            }
            AppAction::ToggleUserMenu => {
                self.overlay.toggle_user_menu();
            }
            AppAction::CloseAllOverlays => {
                self.overlay.close_all_popups();
                self.context_menu = None;
            }
            AppAction::ContextMenuAction { id, path } => {
                self.context_menu = None;
                self.context_menu_target_path = None;
                self.context_menu_target_tab = None;
                if id.starts_with("git_") {
                    self.handle_git_context_action(&id, &path, event_loop);
                } else {
                    self.handle_file_tree_context_action(&id, &path);
                }
            }

            AppAction::OpenSettings => {
                self.open_settings_view();
            }
            AppAction::CloseSettings => {
                self.close_settings_view();
            }
            AppAction::OpenModels => {
                self.open_models_view();
            }
            AppAction::CloseModels => {
                self.close_models_view();
            }

            AppAction::CloseUsagePanel => {
                self.overlay.usage_panel_open = false;
            }
            AppAction::OpenProPanel => {
                self.overlay.pro_panel_open = true;
                self.overlay.usage_panel_open = false;
                self.overlay.pro_panel_hovered = None;
            }
            AppAction::CloseProPanel => {
                self.overlay.pro_panel_open = false;
                self.overlay.pro_license_input.clear();
                self.overlay.pro_license_cursor = 0;
                self.overlay.pro_license_focused = false;
            }
            AppAction::ActivateLicense => {
                let key = self.overlay.pro_license_input.trim().to_string();
                if key.is_empty() {
                    self.toast_mgr.push(
                        "Please enter a license key".to_string(),
                        crate::ui::components::toast::ToastLevel::Warning,
                    );
                } else {
                    match self.license_mgr.activate(&key) {
                        Ok(_data) => {
                            self.usage_tracker.set_pro(true);
                            self.overlay.pro_panel_open = false;
                            self.overlay.pro_license_input.clear();
                            self.overlay.pro_license_cursor = 0;
                            self.toast_mgr.push(
                                "Pro license activated!".to_string(),
                                crate::ui::components::toast::ToastLevel::Success,
                            );
                        }
                        Err(e) => {
                            self.toast_mgr.push(
                                format!("Activation failed: {e}"),
                                crate::ui::components::toast::ToastLevel::Error,
                            );
                        }
                    }
                }
            }
            AppAction::DeactivateLicense => match self.license_mgr.deactivate() {
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
            AppAction::BuyPro => {
                let _ = std::process::Command::new("open")
                    .arg("https://awebo-org.lemonsqueezy.com/checkout/buy/de81be1d-d76a-4d69-a95d-9c1e94fa2c9a?media=0")
                    .spawn();
            }
            AppAction::DismissUsageLimitBanner => {
                self.usage_limit_banner.dismiss();
            }

            AppAction::LoadModel { index } => {
                self.load_model_by_index(index);
            }
            AppAction::CancelInference => {
                self.ai_ctrl.cancel_inference();
            }

            AppAction::StartAgent { task } => {
                let result = self.usage_tracker.can_use(crate::usage::Feature::Agent);
                if let crate::usage::UsageResult::Denied { used, limit } = result {
                    self.show_limit_reached(crate::usage::Feature::Agent, used, limit);
                    return;
                }
                self.maybe_show_first_use_hint(crate::usage::Feature::Agent);
                self.usage_tracker.record_use(crate::usage::Feature::Agent);
                self.start_agent(task);
            }
            AppAction::AgentApproval { decision } => {
                self.handle_agent_approval(decision);
            }
            AppAction::EnterAgentMode => {
                self.smart_input.enter_agent_mode();
                self.hint_banner.show_agent_mode();
            }
            AppAction::ExitAgentMode => {
                self.smart_input.exit_agent_mode();
                self.hint_banner.exit_agent_mode();
            }

            AppAction::DismissHintBanner => {
                self.hint_banner.dismiss();
                self.config.general.hint_banner_dismissed = true;
                self.config.save();
            }

            AppAction::StopSandbox => {
                let idx = self.tab_mgr.active_index();
                if self
                    .tab_mgr
                    .get(idx)
                    .map(|t| matches!(&t.kind, TabKind::Sandbox { .. }))
                    .unwrap_or(false)
                {
                    self.close_tab(idx, event_loop);
                }
            }

            AppAction::Copy => {
                self.perform_copy();
            }
            AppAction::Paste => {
                self.perform_paste();
            }
            AppAction::Cut => {
                self.perform_cut();
            }
            AppAction::SelectAll => {
                self.perform_select_all();
            }
            AppAction::Undo => {
                self.perform_undo();
            }
            AppAction::Redo => {
                self.perform_redo();
            }

            AppAction::DownloadUpdate => {
                if let Some(info) = self.overlay.update_available.clone() {
                    self.overlay.update_downloading = true;
                    self.toast_mgr.push(
                        format!("Downloading v{}…", info.version),
                        crate::ui::components::toast::ToastLevel::Info,
                    );
                    crate::updater::spawn_update_download(info, self.proxy.clone());
                }
            }
            AppAction::InstallUpdate => {
                if let Some(path) = self.overlay.update_downloaded.take() {
                    match crate::updater::stage_update(&path) {
                        Ok(()) => {
                            crate::updater::spawn_relaunch();
                            event_loop.exit();
                        }
                        Err(e) => {
                            self.toast_mgr.push(
                                format!("Update failed: {e}"),
                                crate::ui::components::toast::ToastLevel::Error,
                            );
                        }
                    }
                }
            }
            AppAction::ToggleUpdateDropdown => {
                self.overlay.toggle_update_dropdown();
            }
        }
        self.request_redraw();
    }
}

// Pure read-only helpers that the UI layer can use to map events into
// actions without touching mutable state.

impl super::App {
    /// Resolve a side-panel visual index to the real [`SessionId`].
    ///
    /// Returns `None` if the index is out of range.
    /// This is a *query* — no state is modified.
    pub(crate) fn resolve_session_index(&self, visual_index: usize) -> Option<SessionId> {
        self.session_mgr.sessions().nth(visual_index).map(|s| s.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_is_debug_printable() {
        let a = AppAction::OpenSession { session_id: 42 };
        let s = format!("{:?}", a);
        assert!(s.contains("OpenSession"));
        assert!(s.contains("42"));
    }

    #[test]
    fn action_equality() {
        let a = AppAction::ToggleSidebar;
        let b = AppAction::ToggleSidebar;
        assert_eq!(a, b);

        let c = AppAction::CreateTab { shell_path: None };
        let d = AppAction::CreateTab {
            shell_path: Some("/bin/zsh".into()),
        };
        assert_ne!(c, d);
    }

    #[test]
    fn all_variants_are_distinct() {
        let actions: Vec<AppAction> = vec![
            AppAction::OpenSession { session_id: 1 },
            AppAction::ClearSession { session_id: 1 },
            AppAction::CreateTab { shell_path: None },
            AppAction::CreateSandboxTab { image_idx: 0 },
            AppAction::CloseTab { index: 0 },
            AppAction::SwitchTab { index: 0 },
            AppAction::PreviousTab,
            AppAction::NextTab,
            AppAction::ToggleSidebar,
            AppAction::SwitchPanelTab {
                tab: crate::ui::panel_layout::SidePanelTab::Sessions,
            },
            AppAction::ToggleFileTreeNode {
                path: std::path::PathBuf::from("/tmp"),
            },
            AppAction::OpenFile {
                path: std::path::PathBuf::from("/tmp/test.txt"),
            },
            AppAction::ToggleDebugPanel,
            AppAction::OpenPalette,
            AppAction::ToggleShellPicker,
            AppAction::CloseShellPicker,
            AppAction::ToggleUserMenu,
            AppAction::CloseAllOverlays,
            AppAction::OpenSettings,
            AppAction::CloseSettings,
            AppAction::OpenModels,
            AppAction::CloseModels,
            AppAction::CloseUsagePanel,
            AppAction::OpenProPanel,
            AppAction::CloseProPanel,
            AppAction::ActivateLicense,
            AppAction::DeactivateLicense,
            AppAction::BuyPro,
            AppAction::DismissUsageLimitBanner,
            AppAction::LoadModel { index: 0 },
            AppAction::CancelInference,
            AppAction::StartAgent {
                task: "test".into(),
            },
            AppAction::AgentApproval {
                decision: ApprovalDecision::ApproveOnce,
            },
            AppAction::EnterAgentMode,
            AppAction::ExitAgentMode,
            AppAction::DismissHintBanner,
            AppAction::StopSandbox,
            AppAction::Copy,
            AppAction::Paste,
            AppAction::Cut,
            AppAction::SelectAll,
            AppAction::ToggleGitPanel,
            AppAction::SwitchGitPanelTab {
                tab: crate::ui::panel_layout::GitPanelTab::Changes,
            },
            AppAction::GitStageFile { index: 0 },
            AppAction::GitUnstageFile { index: 0 },
            AppAction::GitStageAll,
            AppAction::GitUnstageAll,
            AppAction::GitCheckoutBranch { index: 0 },
            AppAction::GitFocusCommitInput {
                rel_x: 0.0,
                rel_y: 0.0,
            },
            AppAction::GitGenerateCommitMessage,
            AppAction::GitCancelGenerateCommitMessage,
            AppAction::GitCommit,
            AppAction::GitOpenFileDiff { index: 0 },
            AppAction::GitDiscardFileChanges { path: "f".into() },
            AppAction::GitAddToGitignore { path: "f".into() },
            AppAction::GitOpenFile { path: "f".into() },
            AppAction::GitRevealInFinder { path: "f".into() },
        ];
        let strs: std::collections::HashSet<String> =
            actions.iter().map(|a| format!("{:?}", a)).collect();
        assert_eq!(strs.len(), actions.len());
    }
}
