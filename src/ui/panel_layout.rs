//! Panel layout system — manages left and right panel geometry.
//!
//! Owns panel widths, resize states, and active sub-tabs.
//! The renderer queries `content_x_offset()` / `right_physical_width()`
//! to position all content areas relative to the panels.

/// Which sub-view is active inside the left side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidePanelTab {
    #[default]
    Sessions,
    Files,
    /// Sandbox management — only visible when a sandbox tab is active.
    Sandbox,
    Search,
}

/// Which sub-view is active inside the right (git) panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitPanelTab {
    #[default]
    Changes,
    Branches,
}

/// Drag-to-resize state for a panel edge.
#[derive(Debug, Default)]
pub struct ResizeState {
    /// True while the user is actively dragging the resize handle.
    pub dragging: bool,
    /// True when the cursor hovers over the resize hit-zone.
    pub hovered: bool,
}

const DEFAULT_WIDTH: f32 = 260.0;
const MIN_WIDTH: f32 = 180.0;
const MAX_WIDTH: f32 = 480.0;
/// Physical hit-zone half-width around the panel border (logical px).
const RESIZE_HIT_ZONE: f32 = 4.0;

/// Centralised geometry for left and right panels.
pub struct PanelLayout {
    /// Logical width of the left panel (before scale factor).
    left_width: f32,
    pub left_resize: ResizeState,
    pub active_tab: SidePanelTab,

    /// Logical width of the right (git) panel (before scale factor).
    right_width: f32,
    pub right_resize: ResizeState,
    pub git_tab: GitPanelTab,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            left_width: DEFAULT_WIDTH,
            left_resize: ResizeState::default(),
            active_tab: SidePanelTab::default(),
            right_width: DEFAULT_WIDTH,
            right_resize: ResizeState::default(),
            git_tab: GitPanelTab::default(),
        }
    }
}

impl PanelLayout {
    /// Logical width of the left panel.
    pub fn left_width(&self) -> f32 {
        self.left_width
    }

    /// Physical width of the left panel.
    pub fn left_physical_width(&self, sf: f32) -> usize {
        (self.left_width * sf) as usize
    }

    /// Check whether a physical X coordinate is within the resize
    /// hit-zone of the left panel's right edge.
    pub fn is_in_resize_zone(&self, phys_x: f64, sf: f64) -> bool {
        let edge = self.left_width as f64 * sf;
        let zone = RESIZE_HIT_ZONE as f64 * sf;
        phys_x >= edge - zone && phys_x <= edge + zone
    }

    /// Physical width of the right panel.
    pub fn right_physical_width(&self, sf: f32) -> usize {
        (self.right_width * sf) as usize
    }

    /// Check whether a physical X coordinate is within the resize
    /// hit-zone of the right panel's left edge.
    ///
    /// `buf_w` is the total buffer width in physical pixels.
    pub fn is_in_right_resize_zone(&self, phys_x: f64, sf: f64, buf_w: usize) -> bool {
        let edge = buf_w as f64 - self.right_width as f64 * sf;
        let zone = RESIZE_HIT_ZONE as f64 * sf;
        phys_x >= edge - zone && phys_x <= edge + zone
    }

    /// Set the left panel width (logical px), clamped to bounds.
    pub fn set_left_width(&mut self, w: f32) {
        self.left_width = w.clamp(MIN_WIDTH, MAX_WIDTH);
    }

    /// Begin a left-panel resize drag.
    pub fn begin_resize(&mut self) {
        self.left_resize.dragging = true;
    }

    /// End a left-panel resize drag.
    pub fn end_resize(&mut self) {
        self.left_resize.dragging = false;
    }

    /// Switch the active left-panel sub-tab.
    pub fn switch_tab(&mut self, tab: SidePanelTab) {
        self.active_tab = tab;
    }

    /// Set the right panel width (logical px), clamped to bounds.
    pub fn set_right_width(&mut self, w: f32) {
        self.right_width = w.clamp(MIN_WIDTH, MAX_WIDTH);
    }

    /// Begin a right-panel resize drag.
    pub fn begin_right_resize(&mut self) {
        self.right_resize.dragging = true;
    }

    /// End a right-panel resize drag.
    pub fn end_right_resize(&mut self) {
        self.right_resize.dragging = false;
    }

    /// Switch the active right-panel (git) sub-tab.
    pub fn switch_git_tab(&mut self, tab: GitPanelTab) {
        self.git_tab = tab;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let pl = PanelLayout::default();
        assert_eq!(pl.left_width(), DEFAULT_WIDTH);
        assert_eq!(pl.right_physical_width(1.0), DEFAULT_WIDTH as usize);
        assert_eq!(pl.active_tab, SidePanelTab::Sessions);
        assert_eq!(pl.git_tab, GitPanelTab::Changes);
        assert!(!pl.left_resize.dragging);
        assert!(!pl.left_resize.hovered);
        assert!(!pl.right_resize.dragging);
        assert!(!pl.right_resize.hovered);
    }

    #[test]
    fn set_left_width_clamps() {
        let mut pl = PanelLayout::default();
        pl.set_left_width(100.0);
        assert_eq!(pl.left_width(), MIN_WIDTH);
        pl.set_left_width(999.0);
        assert_eq!(pl.left_width(), MAX_WIDTH);
        pl.set_left_width(300.0);
        assert_eq!(pl.left_width(), 300.0);
    }

    #[test]
    fn set_right_width_clamps() {
        let mut pl = PanelLayout::default();
        pl.set_right_width(100.0);
        assert_eq!(pl.right_physical_width(1.0), MIN_WIDTH as usize);
        pl.set_right_width(999.0);
        assert_eq!(pl.right_physical_width(1.0), MAX_WIDTH as usize);
        pl.set_right_width(350.0);
        assert_eq!(pl.right_physical_width(1.0), 350_usize);
    }

    #[test]
    fn physical_width() {
        let pl = PanelLayout::default();
        assert_eq!(pl.left_physical_width(2.0), (DEFAULT_WIDTH * 2.0) as usize);
        assert_eq!(pl.right_physical_width(2.0), (DEFAULT_WIDTH * 2.0) as usize);
    }

    #[test]
    fn left_resize_zone_hit_test() {
        let pl = PanelLayout::default();
        let sf = 2.0;
        let edge = DEFAULT_WIDTH as f64 * sf;
        assert!(pl.is_in_resize_zone(edge, sf));
        assert!(pl.is_in_resize_zone(edge - 3.0 * sf, sf));
        assert!(!pl.is_in_resize_zone(0.0, sf));
        assert!(!pl.is_in_resize_zone(edge + 20.0 * sf, sf));
    }

    #[test]
    fn right_resize_zone_hit_test() {
        let pl = PanelLayout::default();
        let sf = 2.0;
        let buf_w = 2000;
        let edge = buf_w as f64 - DEFAULT_WIDTH as f64 * sf;
        assert!(pl.is_in_right_resize_zone(edge, sf, buf_w));
        assert!(pl.is_in_right_resize_zone(edge + 3.0, sf, buf_w));
        assert!(!pl.is_in_right_resize_zone(0.0, sf, buf_w));
        assert!(!pl.is_in_right_resize_zone(buf_w as f64, sf, buf_w));
    }

    #[test]
    fn switch_tab() {
        let mut pl = PanelLayout::default();
        pl.switch_tab(SidePanelTab::Files);
        assert_eq!(pl.active_tab, SidePanelTab::Files);
        pl.switch_tab(SidePanelTab::Sessions);
        assert_eq!(pl.active_tab, SidePanelTab::Sessions);
    }

    #[test]
    fn switch_git_tab() {
        let mut pl = PanelLayout::default();
        pl.switch_git_tab(GitPanelTab::Branches);
        assert_eq!(pl.git_tab, GitPanelTab::Branches);
        pl.switch_git_tab(GitPanelTab::Changes);
        assert_eq!(pl.git_tab, GitPanelTab::Changes);
    }

    #[test]
    fn begin_end_resize() {
        let mut pl = PanelLayout::default();
        pl.begin_resize();
        assert!(pl.left_resize.dragging);
        pl.end_resize();
        assert!(!pl.left_resize.dragging);
    }

    #[test]
    fn begin_end_right_resize() {
        let mut pl = PanelLayout::default();
        pl.begin_right_resize();
        assert!(pl.right_resize.dragging);
        pl.end_right_resize();
        assert!(!pl.right_resize.dragging);
    }
}
