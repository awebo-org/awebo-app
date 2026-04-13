use std::time::Instant;

/// Groups all transient overlay/popup state (palette, pickers, hover, tooltips).
pub(crate) struct OverlayState {
    pub palette_open: bool,
    pub palette_query: String,
    pub palette_selected: usize,
    pub model_picker_open: bool,
    pub model_picker_selected: usize,
    pub shell_picker_open: bool,
    pub shell_picker_hovered: Option<usize>,
    pub debug_panel: bool,
    pub avatar_hovered: bool,
    pub user_menu_open: bool,
    pub user_menu_hovered: Option<usize>,
    pub new_tab_hovered: bool,
    pub shell_picker_btn_hovered: bool,
    pub sidebar_open: bool,
    pub sidebar_hovered: bool,
    pub last_empty_bar_click: Option<Instant>,
    pub tooltip: Option<(String, f64, f64)>,
    pub hovered_close: Option<usize>,
    pub ctx_bar_rect: Option<(usize, usize, usize, usize)>,
    pub stop_button_rect: Option<(usize, usize, usize, usize)>,

    /// Whether the right (git) panel is visible.
    pub git_panel_open: bool,
    /// Hover state for the git panel toggle button.
    pub git_panel_hovered: bool,

    /// Unsaved file close confirmation — index of tab pending close.
    pub confirm_close_tab: Option<usize>,
    /// Button hover state for the confirmation dialog (0=Save, 1=Don't Save, 2=Cancel).
    pub confirm_close_hovered: Option<usize>,

    pub usage_panel_open: bool,
    pub pro_panel_open: bool,
    pub pro_license_input: String,
    pub pro_license_cursor: usize,
    pub pro_license_focused: bool,
    pub pro_panel_hovered: Option<usize>,
}

impl OverlayState {
    /// Open the command palette, resetting query and selection.
    pub fn open_palette(&mut self) {
        self.palette_open = true;
        self.palette_query.clear();
        self.palette_selected = 0;
    }

    /// Close the command palette and reset its state.
    pub fn close_palette(&mut self) {
        self.palette_open = false;
        self.palette_query.clear();
        self.palette_selected = 0;
    }

    /// Toggle the shell picker dropdown.
    pub fn toggle_shell_picker(&mut self) {
        self.shell_picker_open = !self.shell_picker_open;
        self.shell_picker_hovered = None;
    }

    /// Close the shell picker dropdown.
    pub fn close_shell_picker(&mut self) {
        self.shell_picker_open = false;
        self.shell_picker_hovered = None;
    }

    /// Toggle the user menu, closing shell picker when opening.
    pub fn toggle_user_menu(&mut self) {
        self.user_menu_open = !self.user_menu_open;
        self.user_menu_hovered = None;
        self.shell_picker_open = false;
    }

    /// Close the user menu.
    pub fn close_user_menu(&mut self) {
        self.user_menu_open = false;
        self.user_menu_hovered = None;
    }

    /// Close the model picker.
    pub fn close_model_picker(&mut self) {
        self.model_picker_open = false;
        self.model_picker_selected = 0;
    }

    /// Toggle the git panel, closing shell picker when opening.
    pub fn toggle_git_panel(&mut self) {
        self.git_panel_open = !self.git_panel_open;
        if self.git_panel_open {
            self.close_shell_picker();
            self.close_user_menu();
        }
    }

    /// Close all popups/overlays at once.
    pub fn close_all_popups(&mut self) {
        self.close_palette();
        self.close_model_picker();
        self.close_shell_picker();
        self.close_user_menu();
        self.dismiss_confirm_close();
        self.usage_panel_open = false;
        self.pro_panel_open = false;
        self.pro_license_input.clear();
        self.pro_license_cursor = 0;
        self.pro_license_focused = false;
    }

    /// Show the unsaved-file close confirmation dialog for the given tab index.
    pub fn request_confirm_close(&mut self, tab_idx: usize) {
        self.confirm_close_tab = Some(tab_idx);
        self.confirm_close_hovered = None;
    }

    /// Dismiss the unsaved-file close confirmation dialog.
    pub fn dismiss_confirm_close(&mut self) {
        self.confirm_close_tab = None;
        self.confirm_close_hovered = None;
    }

    /// Whether the confirm-close dialog is currently shown.
    pub fn is_confirm_close_open(&self) -> bool {
        self.confirm_close_tab.is_some()
    }

}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            model_picker_open: false,
            model_picker_selected: 0,
            shell_picker_open: false,
            shell_picker_hovered: None,
            debug_panel: false,
            avatar_hovered: false,
            user_menu_open: false,
            user_menu_hovered: None,
            new_tab_hovered: false,
            shell_picker_btn_hovered: false,
            sidebar_open: false,
            sidebar_hovered: false,
            last_empty_bar_click: None,
            tooltip: None,
            hovered_close: None,
            ctx_bar_rect: None,
            stop_button_rect: None,
            git_panel_open: false,
            git_panel_hovered: false,
            confirm_close_tab: None,
            confirm_close_hovered: None,
            usage_panel_open: false,
            pro_panel_open: false,
            pro_license_input: String::new(),
            pro_license_cursor: 0,
            pro_license_focused: false,
            pro_panel_hovered: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_state_default_values() {
        let s = OverlayState::default();
        assert!(!s.palette_open);
        assert!(s.palette_query.is_empty());
        assert_eq!(s.palette_selected, 0);
        assert!(!s.model_picker_open);
        assert_eq!(s.model_picker_selected, 0);
        assert!(!s.shell_picker_open);
        assert!(s.shell_picker_hovered.is_none());
        assert!(!s.debug_panel);
        assert!(!s.avatar_hovered);
        assert!(!s.user_menu_open);
        assert!(s.user_menu_hovered.is_none());
        assert!(!s.new_tab_hovered);
        assert!(!s.shell_picker_btn_hovered);
        assert!(!s.sidebar_open);
        assert!(!s.sidebar_hovered);
        assert!(s.last_empty_bar_click.is_none());
        assert!(s.tooltip.is_none());
        assert!(s.hovered_close.is_none());
        assert!(s.ctx_bar_rect.is_none());
        assert!(s.stop_button_rect.is_none());
        assert!(!s.git_panel_open);
        assert!(!s.git_panel_hovered);
        assert!(s.confirm_close_tab.is_none());
        assert!(s.confirm_close_hovered.is_none());
    }

    #[test]
    fn open_close_palette() {
        let mut s = OverlayState::default();
        s.open_palette();
        assert!(s.palette_open);
        assert!(s.palette_query.is_empty());

        s.palette_query.push_str("test");
        s.palette_selected = 3;
        s.close_palette();
        assert!(!s.palette_open);
        assert!(s.palette_query.is_empty());
        assert_eq!(s.palette_selected, 0);
    }

    #[test]
    fn toggle_shell_picker() {
        let mut s = OverlayState::default();
        s.toggle_shell_picker();
        assert!(s.shell_picker_open);
        s.shell_picker_hovered = Some(2);
        s.toggle_shell_picker();
        assert!(!s.shell_picker_open);
        assert!(s.shell_picker_hovered.is_none());
    }

    #[test]
    fn toggle_user_menu_closes_shell_picker() {
        let mut s = OverlayState::default();
        s.shell_picker_open = true;
        s.toggle_user_menu();
        assert!(s.user_menu_open);
        assert!(!s.shell_picker_open);
    }

    #[test]
    fn close_all_popups() {
        let mut s = OverlayState::default();
        s.open_palette();
        s.shell_picker_open = true;
        s.user_menu_open = true;
        s.model_picker_open = true;
        assert!(s.palette_open || s.shell_picker_open || s.user_menu_open || s.model_picker_open);

        s.close_all_popups();
        assert!(!s.palette_open);
        assert!(!s.shell_picker_open);
        assert!(!s.user_menu_open);
        assert!(!s.model_picker_open);
    }

    #[test]
    fn confirm_close_request_and_dismiss() {
        let mut s = OverlayState::default();
        assert!(!s.is_confirm_close_open());

        s.request_confirm_close(2);
        assert!(s.is_confirm_close_open());
        assert_eq!(s.confirm_close_tab, Some(2));
        assert!(s.confirm_close_hovered.is_none());

        s.confirm_close_hovered = Some(1);
        s.dismiss_confirm_close();
        assert!(!s.is_confirm_close_open());
        assert!(s.confirm_close_hovered.is_none());
    }

    #[test]
    fn close_all_popups_dismisses_confirm_close() {
        let mut s = OverlayState::default();
        s.request_confirm_close(0);
        assert!(s.is_confirm_close_open());
        s.close_all_popups();
        assert!(!s.is_confirm_close_open());
    }

}
