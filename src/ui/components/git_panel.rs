//! Right side panel — Git source control management.
//!
//! VS Code-inspired layout: commit input → commit button → staged/unstaged
//! file lists with status badges. Mirrors the left panel architecture.

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};

use crate::git::{BranchEntry, FileStatus, GitRepo, StatusEntry};
use crate::renderer::icons::{Icon, IconRenderer};
use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};
use crate::renderer::text::{draw_text_at, draw_text_at_bold, measure_text_width_bold};
use crate::renderer::theme;
use crate::ui::panel_layout::{GitPanelTab, PanelLayout};
const HEADER_HEIGHT: f32 = 40.0;
const TOOLBAR_BTN_SIZE: f32 = 14.0;
const TOOLBAR_BTN_CONTAINER: f32 = 26.0;
const TOOLBAR_BTN_GAP: f32 = 2.0;
const TOOLBAR_BTN_RADIUS: f32 = 5.0;
const TOOLBAR_PAD_X: f32 = 8.0;

const COMMIT_INPUT_HEIGHT: f32 = 32.0;
pub(crate) const COMMIT_INPUT_PAD_X: f32 = 10.0;
const COMMIT_INPUT_RADIUS: f32 = 4.0;
const COMMIT_BTN_HEIGHT: f32 = 30.0;
const COMMIT_BTN_RADIUS: f32 = 5.0;
const COMMIT_SECTION_PAD: f32 = 8.0;

const FILE_ITEM_HEIGHT: f32 = 26.0;
const FILE_PAD_X: f32 = 10.0;
const SECTION_HEADER_HEIGHT: f32 = 26.0;
const SECTION_PAD_Y: f32 = 4.0;
const COUNT_BADGE_SIZE: f32 = 18.0;
const COUNT_BADGE_RADIUS: f32 = 9.0;

/// Click result from the git panel.
#[derive(Debug, Clone, PartialEq)]
pub enum GitPanelHit {
    ToolbarChanges,
    ToolbarBranches,
    /// Click inside the commit input at relative (x, y) within the text area.
    CommitInput {
        rel_x: f64,
        rel_y: f64,
    },
    CommitButton,
    GenerateButton,
    StageAll,
    UnstageAll,
    StageFile(usize),
    UnstageFile(usize),
    SelectFile(usize),
    CheckoutBranch(usize),
    None,
}

/// Cached git data refreshed on panel open / interaction.
#[derive(Default)]
pub struct GitPanelData {
    pub entries: Vec<StatusEntry>,
    pub branches: Vec<BranchEntry>,
    pub current_branch: String,
    pub has_repo: bool,
    pub additions: usize,
    pub deletions: usize,
}

/// Persistent UI state for the git panel.
pub struct GitPanelState {
    pub data: GitPanelData,
    pub commit_message: String,
    pub commit_input_focused: bool,
    pub generating_commit_msg: bool,
    /// Set when generate was requested but model was not loaded yet.
    pub pending_generate_commit_msg: bool,
    pub scroll_offset: f32,
    pub hovered_item: Option<usize>,
    pub hovered_toolbar_btn: Option<GitPanelTab>,
    pub hovered_commit_btn: bool,
    pub hovered_generate_btn: bool,
    pub hovered_stage_all: bool,
    pub hovered_unstage_all: bool,
    /// Hovered branch index in the branches tab.
    pub hovered_branch: Option<usize>,
    /// Cursor byte offset within commit_message.
    pub cursor: usize,
    /// Selection anchor byte offset (if Some, text between anchor..cursor is selected).
    pub selection_anchor: Option<usize>,
    pub scrollbar_hovered: bool,
    pub scrollbar_dragging: bool,
    pub scrollbar_drag_start_y: f64,
    pub scrollbar_drag_start_scroll: f32,
}

impl Default for GitPanelState {
    fn default() -> Self {
        Self {
            data: GitPanelData::default(),
            commit_message: String::new(),
            commit_input_focused: false,
            generating_commit_msg: false,
            pending_generate_commit_msg: false,
            scroll_offset: 0.0,
            hovered_item: None,
            hovered_toolbar_btn: None,
            hovered_commit_btn: false,
            hovered_generate_btn: false,
            hovered_stage_all: false,
            hovered_unstage_all: false,
            hovered_branch: None,
            cursor: 0,
            selection_anchor: None,
            scrollbar_hovered: false,
            scrollbar_dragging: false,
            scrollbar_drag_start_y: 0.0,
            scrollbar_drag_start_scroll: 0.0,
        }
    }
}

impl GitPanelState {
    /// Refresh git data from the given working directory.
    pub fn refresh(&mut self, cwd: &str) {
        if let Some(repo) = GitRepo::discover(cwd) {
            self.data.has_repo = true;
            self.data.current_branch = repo.current_branch();
            self.data.entries = repo.status_entries();
            self.data.branches = repo.branches();
            let (additions, deletions) = repo.diff_stat();
            self.data.additions = additions;
            self.data.deletions = deletions;
        } else {
            self.data = GitPanelData::default();
        }
    }

    /// Return sorted (start, end) byte range of the selection, if any.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|a| {
            let (s, e) = if a < self.cursor {
                (a, self.cursor)
            } else {
                (self.cursor, a)
            };
            (
                s.min(self.commit_message.len()),
                e.min(self.commit_message.len()),
            )
        })
    }

    /// Delete selected text and collapse cursor to selection start.
    pub fn delete_selection(&mut self) {
        if let Some((s, e)) = self.selection_range() {
            self.commit_message.replace_range(s..e, "");
            self.cursor = s;
            self.selection_anchor = None;
        }
    }

    /// Select all text.
    pub fn select_all(&mut self) {
        if self.commit_message.is_empty() {
            return;
        }
        self.selection_anchor = Some(0);
        self.cursor = self.commit_message.len();
    }

    /// Move cursor left by one char, with optional shift-select.
    pub fn move_left(&mut self, shift: bool) {
        if !shift {
            self.selection_anchor = None;
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        if self.cursor > 0 {
            let prev = self.commit_message[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.cursor = prev;
        }
    }

    /// Move cursor right by one char, with optional shift-select.
    pub fn move_right(&mut self, shift: bool) {
        if !shift {
            self.selection_anchor = None;
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        if self.cursor < self.commit_message.len() {
            let next = self.commit_message[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.commit_message.len());
            self.cursor = next;
        }
    }

    /// Move cursor to start of text, with optional shift-select.
    pub fn move_home(&mut self, shift: bool) {
        if !shift {
            self.selection_anchor = None;
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        self.cursor = 0;
    }

    /// Move cursor to end of text, with optional shift-select.
    pub fn move_end(&mut self, shift: bool) {
        if !shift {
            self.selection_anchor = None;
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        self.cursor = self.commit_message.len();
    }

    /// Insert text at cursor, replacing any selection.
    pub fn insert_text(&mut self, s: &str) {
        self.delete_selection();
        self.commit_message.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    /// Delete one char before cursor or delete selection.
    pub fn backspace(&mut self) {
        if self.selection_anchor.is_some() {
            self.delete_selection();
        } else if self.cursor > 0 {
            let prev = self.commit_message[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.commit_message.replace_range(prev..self.cursor, "");
            self.cursor = prev;
        }
    }

    /// Compute the cursor byte offset from a click at (rel_x, rel_y) in the text area.
    pub fn cursor_from_click(&mut self, rel_x: f64, rel_y: f64, char_w: f64, max_chars: usize) {
        let line_h = 16.0;
        let row = (rel_y / line_h).floor().max(0.0) as usize;
        let col = (rel_x / char_w).round().max(0.0) as usize;
        let vlines = wrap_lines(&self.commit_message, max_chars);
        if let Some((byte_start, text)) = vlines.get(row) {
            let clamped = col.min(text.len());
            self.cursor = byte_start + clamped;
        } else {
            self.cursor = self.commit_message.len();
        }
        self.selection_anchor = None;
    }

    /// Move cursor up one visual line, preserving approximate column.
    pub fn move_up(&mut self, shift: bool, max_chars: usize) {
        if !shift {
            self.selection_anchor = None;
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        let vlines = wrap_lines(&self.commit_message, max_chars);
        let (row, col) = visual_cursor_pos(&vlines, self.cursor);
        if row == 0 {
            self.cursor = 0;
            return;
        }
        let (prev_start, prev_text) = vlines[row - 1];
        self.cursor = prev_start + col.min(prev_text.len());
    }

    /// Move cursor down one visual line, preserving approximate column.
    pub fn move_down(&mut self, shift: bool, max_chars: usize) {
        if !shift {
            self.selection_anchor = None;
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        let vlines = wrap_lines(&self.commit_message, max_chars);
        let (row, col) = visual_cursor_pos(&vlines, self.cursor);
        if row + 1 >= vlines.len() {
            self.cursor = self.commit_message.len();
            return;
        }
        let (next_start, next_text) = vlines[row + 1];
        self.cursor = next_start + col.min(next_text.len());
    }
}

/// Compute total content height for the active tab (in physical pixels).
pub fn content_height(state: &GitPanelState, tab: GitPanelTab, sf: f32) -> f32 {
    match tab {
        GitPanelTab::Changes => changes_content_height(state, sf),
        GitPanelTab::Branches => {
            let item_h = FILE_ITEM_HEIGHT * sf;
            let local = state.data.branches.iter().filter(|b| !b.is_remote).count();
            let remote = state.data.branches.iter().filter(|b| b.is_remote).count();
            if local == 0 && remote == 0 {
                return item_h + SECTION_PAD_Y * sf;
            }
            let mut h = SECTION_PAD_Y * sf;
            if local > 0 {
                h += item_h + local as f32 * item_h;
            }
            if remote > 0 {
                h += SECTION_PAD_Y * sf + item_h + remote as f32 * item_h;
            }
            h
        }
    }
}

/// Maximum scroll offset for the active tab.
pub fn max_scroll(state: &GitPanelState, tab: GitPanelTab, visible_h: f32, sf: f32) -> f32 {
    (content_height(state, tab, sf) - visible_h).max(0.0)
}

/// Compute visual (wrapped) lines from text. Each entry is (byte_start, content).
fn wrap_lines(text: &str, max_chars: usize) -> Vec<(usize, &str)> {
    if text.is_empty() {
        return vec![(0, "")];
    }
    let max_chars = max_chars.max(1);
    let mut result = Vec::new();
    let mut byte_off: usize = 0;
    for logical_line in text.split('\n') {
        if logical_line.is_empty() {
            result.push((byte_off, ""));
        } else {
            let mut start = 0;
            while start < logical_line.len() {
                let end = (start + max_chars).min(logical_line.len());
                let end = if end < logical_line.len() {
                    logical_line[..end]
                        .char_indices()
                        .next_back()
                        .map(|(i, c)| i + c.len_utf8())
                        .unwrap_or(end)
                } else {
                    end
                };
                result.push((byte_off + start, &logical_line[start..end]));
                start = end;
            }
        }
        byte_off += logical_line.len() + 1; // +1 for '\n'
    }
    result
}

/// Count how many visual lines the commit text produces at the given width.
fn visual_line_count(text: &str, max_chars: usize) -> usize {
    wrap_lines(text, max_chars).len()
}

/// Return (visual_row, col_within_row) for a byte cursor in wrapped lines.
fn visual_cursor_pos(vlines: &[(usize, &str)], cursor: usize) -> (usize, usize) {
    for (i, (start, text)) in vlines.iter().enumerate() {
        let end = start + text.len();
        if cursor >= *start && cursor <= end {
            return (i, cursor - start);
        }
    }
    let last = vlines.len().saturating_sub(1);
    (last, 0)
}

fn changes_content_height(state: &GitPanelState, sf: f32) -> f32 {
    let pad = COMMIT_SECTION_PAD * sf;
    let line_h = 16.0 * sf;
    let char_w = 7.0 * sf;
    let input_pad_x = COMMIT_INPUT_PAD_X * sf;
    let approx_panel_w = 280.0 * sf;
    let text_max_px = approx_panel_w - input_pad_x * 2.0 - 8.0 * sf - 16.0 * sf - 16.0 * sf;
    let max_chars = (text_max_px / char_w).floor().max(1.0) as usize;
    let line_count = visual_line_count(&state.commit_message, max_chars).max(1) as f32;
    let input_h = (COMMIT_INPUT_HEIGHT * sf).max(line_count * line_h + 8.0 * sf);
    let btn_h = COMMIT_BTN_HEIGHT * sf;

    let staged = state.data.entries.iter().filter(|e| e.staged).count();
    let unstaged = state.data.entries.iter().filter(|e| !e.staged).count();
    let item_h = FILE_ITEM_HEIGHT * sf;
    let section_h = SECTION_HEADER_HEIGHT * sf;

    let mut h = pad + input_h + pad + btn_h + pad;
    if staged > 0 {
        h += section_h + staged as f32 * item_h + SECTION_PAD_Y * sf;
    }
    if unstaged > 0 {
        h += section_h + unstaged as f32 * item_h;
    }
    if staged == 0 && unstaged == 0 {
        h += 60.0 * sf;
    }
    h
}

/// Update hover state for the git panel. Returns true if anything changed.
pub fn update_hover(
    state: &mut GitPanelState,
    panel_layout: &PanelLayout,
    phys_x: f64,
    phys_y: f64,
    bar_h: f64,
    buf_w: usize,
    sf: f64,
) -> bool {
    let hit = hit_test(phys_x, phys_y, state, panel_layout, bar_h, buf_w, sf);
    let mut changed = false;

    let new_toolbar = match &hit {
        GitPanelHit::ToolbarChanges => Some(GitPanelTab::Changes),
        GitPanelHit::ToolbarBranches => Some(GitPanelTab::Branches),
        _ => None,
    };
    if state.hovered_toolbar_btn != new_toolbar {
        state.hovered_toolbar_btn = new_toolbar;
        changed = true;
    }

    let new_commit = matches!(hit, GitPanelHit::CommitButton);
    if state.hovered_commit_btn != new_commit {
        state.hovered_commit_btn = new_commit;
        changed = true;
    }

    let new_gen = matches!(hit, GitPanelHit::GenerateButton);
    if state.hovered_generate_btn != new_gen {
        state.hovered_generate_btn = new_gen;
        changed = true;
    }

    let new_stage_all = matches!(hit, GitPanelHit::StageAll);
    if state.hovered_stage_all != new_stage_all {
        state.hovered_stage_all = new_stage_all;
        changed = true;
    }

    let new_unstage_all = matches!(hit, GitPanelHit::UnstageAll);
    if state.hovered_unstage_all != new_unstage_all {
        state.hovered_unstage_all = new_unstage_all;
        changed = true;
    }

    let new_item = match &hit {
        GitPanelHit::SelectFile(idx)
        | GitPanelHit::StageFile(idx)
        | GitPanelHit::UnstageFile(idx) => Some(*idx),
        _ => None,
    };
    if state.hovered_item != new_item {
        state.hovered_item = new_item;
        changed = true;
    }

    let new_branch = match &hit {
        GitPanelHit::CheckoutBranch(idx) => Some(*idx),
        _ => None,
    };
    if state.hovered_branch != new_branch {
        state.hovered_branch = new_branch;
        changed = true;
    }

    changed
}

/// Returns true when cursor is over an interactive git panel element (for pointer cursor).
pub fn wants_pointer(state: &GitPanelState) -> bool {
    state.hovered_item.is_some()
        || state.hovered_toolbar_btn.is_some()
        || state.hovered_commit_btn
        || state.hovered_generate_btn
        || state.hovered_stage_all
        || state.hovered_unstage_all
        || state.hovered_branch.is_some()
        || state.scrollbar_hovered
        || state.scrollbar_dragging
}

const COLOR_ADDED: Rgb = (76, 175, 80);
const COLOR_MODIFIED: Rgb = (255, 183, 77);
const COLOR_DELETED: Rgb = (239, 83, 80);
const COLOR_UNTRACKED: Rgb = (120, 120, 120);
const COLOR_RENAMED: Rgb = (100, 181, 246);
const COLOR_CONFLICTED: Rgb = (255, 87, 34);

/// Truncate text to fit within `max_px` width (approximate), appending "…" if needed.
fn truncate_to_fit(text: &str, char_w: f32, max_px: f32) -> String {
    let max_chars = (max_px / char_w).floor() as usize;
    if text.len() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let mut s: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    s.push('…');
    s
}

/// Draw the git panel on the right side.
/// Returns the physical width consumed (0 if panel is closed).
pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &GitPanelState,
    panel_layout: &PanelLayout,
    bar_h: usize,
    sf: f32,
    cursor_blink_on: bool,
) -> usize {
    let panel_w = panel_layout.right_physical_width(sf);
    let panel_h = buf.height.saturating_sub(bar_h);
    if panel_w == 0 || panel_h == 0 {
        return 0;
    }

    let panel_x = buf.width.saturating_sub(panel_w);

    buf.fill_rect(panel_x, bar_h, panel_w, panel_h, (0, 0, 0));

    let border_w = (1.0 * sf).max(1.0) as usize;
    buf.fill_rect(panel_x, bar_h, border_w, panel_h, theme::BORDER);

    if !state.data.has_repo {
        draw_no_repo(
            buf,
            font_system,
            swash_cache,
            panel_x,
            bar_h,
            panel_w,
            panel_h,
            sf,
        );
        return panel_w;
    }

    let header_h = (HEADER_HEIGHT * sf) as usize;
    draw_toolbar(
        buf,
        font_system,
        swash_cache,
        icon_renderer,
        state,
        panel_layout,
        panel_x,
        bar_h,
        panel_w,
        header_h,
        sf,
    );

    let hdr_border_y = bar_h + header_h;
    buf.fill_rect(panel_x, hdr_border_y, panel_w, border_w, theme::BORDER);

    let content_y = hdr_border_y + border_w;
    let content_h = panel_h.saturating_sub(header_h + border_w);
    let scroll = state.scroll_offset as usize;

    match panel_layout.git_tab {
        GitPanelTab::Changes => {
            draw_changes_tab(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                state,
                panel_x,
                content_y,
                panel_w,
                content_h,
                sf,
                scroll,
                cursor_blink_on,
            );
        }
        GitPanelTab::Branches => {
            draw_branches_tab(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                state,
                panel_x,
                content_y,
                panel_w,
                content_h,
                sf,
                scroll,
            );
        }
    }

    let total_h = content_height(state, panel_layout.git_tab, sf) as usize;
    let sb_hover = state.scrollbar_hovered || state.scrollbar_dragging;
    draw_git_scrollbar(
        buf, panel_x, panel_w, content_y, content_h, total_h, scroll, sb_hover, sf,
    );

    buf.fill_rect(panel_x, bar_h, panel_w, header_h, (0, 0, 0));
    buf.fill_rect(panel_x, bar_h, border_w, header_h, theme::BORDER);
    draw_toolbar(
        buf,
        font_system,
        swash_cache,
        icon_renderer,
        state,
        panel_layout,
        panel_x,
        bar_h,
        panel_w,
        header_h,
        sf,
    );
    buf.fill_rect(panel_x, hdr_border_y, panel_w, border_w, theme::BORDER);

    panel_w
}

pub fn hit_test(
    phys_x: f64,
    phys_y: f64,
    state: &GitPanelState,
    panel_layout: &PanelLayout,
    bar_h: f64,
    buf_w: usize,
    sf: f64,
) -> GitPanelHit {
    let panel_w = panel_layout.right_physical_width(sf as f32) as f64;
    let panel_x = buf_w as f64 - panel_w;

    if phys_x < panel_x || phys_y < bar_h {
        return GitPanelHit::None;
    }

    let rel_x = phys_x - panel_x;
    let rel_y = phys_y - bar_h;

    let header_h = HEADER_HEIGHT as f64 * sf;
    if rel_y < header_h {
        return toolbar_hit_test(rel_x, header_h, sf);
    }

    let border_w = (1.0 * sf).max(1.0);
    let content_y_start = header_h + border_w;
    let content_rel_y = rel_y - content_y_start + state.scroll_offset as f64;

    match panel_layout.git_tab {
        GitPanelTab::Changes => changes_hit_test(content_rel_y, rel_x, panel_w, state, sf),
        GitPanelTab::Branches => branches_hit_test(content_rel_y, state, sf),
    }
}

fn draw_toolbar(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &GitPanelState,
    panel_layout: &PanelLayout,
    panel_x: usize,
    toolbar_y: usize,
    panel_w: usize,
    header_h: usize,
    sf: f32,
) {
    let btn_sz = (TOOLBAR_BTN_SIZE * sf).round() as u32;
    let container = (TOOLBAR_BTN_CONTAINER * sf) as usize;
    let gap = (TOOLBAR_BTN_GAP * sf) as usize;
    let pad_x = (TOOLBAR_PAD_X * sf) as usize;
    let radius = (TOOLBAR_BTN_RADIUS * sf) as usize;
    let icon_inset = (container as f32 - btn_sz as f32) / 2.0;

    let tabs = [
        (GitPanelTab::Changes, Icon::Files),
        (GitPanelTab::Branches, Icon::GitBranch),
    ];

    let cy = toolbar_y + (header_h.saturating_sub(container)) / 2;

    for (i, (tab, icon)) in tabs.iter().enumerate() {
        let bx = panel_x + pad_x + i * (container + gap);
        let is_active = panel_layout.git_tab == *tab;
        let is_hovered = state.hovered_toolbar_btn == Some(*tab);

        let bg = if is_active {
            Some(theme::BG_ELEVATED)
        } else if is_hovered {
            Some(theme::BG_HOVER)
        } else {
            None
        };
        if let Some(bg_color) = bg {
            super::overlay::fill_rounded_rect(buf, bx, cy, container, container, radius, bg_color);
        }

        let ix = bx + icon_inset as usize;
        let iy = cy + icon_inset as usize;
        let color = if is_active {
            theme::FG_BRIGHT
        } else if is_hovered {
            theme::FG_PRIMARY
        } else {
            theme::FG_MUTED
        };
        icon_renderer.draw(buf, *icon, ix, iy, btn_sz, color);
    }

    let additions = state.data.additions;
    let deletions = state.data.deletions;
    if additions > 0 || deletions > 0 {
        const ADD_FG: Rgb = (63, 185, 80);
        const DEL_FG: Rgb = (248, 81, 73);

        let metrics = Metrics::new(12.0 * sf, 16.0 * sf);
        let add_label = format!("+{additions}");
        let del_label = format!("-{deletions}");
        let inner_gap = (4.0 * sf) as usize;
        let add_w = measure_text_width_bold(font_system, &add_label, metrics, Family::Monospace)
            .ceil() as usize;
        let del_w = measure_text_width_bold(font_system, &del_label, metrics, Family::Monospace)
            .ceil() as usize;
        let total_w = add_w + inner_gap + del_w;

        let right_margin = pad_x;
        let tx = panel_x + panel_w - right_margin - total_w;
        let text_y = toolbar_y + ((header_h as f32 - metrics.line_height) / 2.0) as usize;
        let clip_h = toolbar_y + header_h;

        draw_text_at_bold(
            buf,
            font_system,
            swash_cache,
            tx,
            text_y,
            clip_h,
            &add_label,
            metrics,
            ADD_FG,
            Family::Monospace,
        );
        draw_text_at_bold(
            buf,
            font_system,
            swash_cache,
            tx + add_w + inner_gap,
            text_y,
            clip_h,
            &del_label,
            metrics,
            DEL_FG,
            Family::Monospace,
        );
    }
}

fn toolbar_hit_test(rel_x: f64, _header_h: f64, sf: f64) -> GitPanelHit {
    let container = TOOLBAR_BTN_CONTAINER as f64 * sf;
    let gap = TOOLBAR_BTN_GAP as f64 * sf;
    let pad_x = TOOLBAR_PAD_X as f64 * sf;

    let tabs = [GitPanelHit::ToolbarChanges, GitPanelHit::ToolbarBranches];
    for (i, hit) in tabs.into_iter().enumerate() {
        let bx = pad_x + i as f64 * (container + gap);
        if rel_x >= bx && rel_x < bx + container {
            return hit;
        }
    }
    GitPanelHit::None
}

fn draw_no_repo(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    panel_x: usize,
    bar_h: usize,
    panel_w: usize,
    panel_h: usize,
    sf: f32,
) {
    let clip_h = bar_h + panel_h;
    let msg = "No git repository";
    let metrics = Metrics::new(13.0 * sf, 18.0 * sf);
    let text_w = msg.len() as f32 * 7.5 * sf;
    let tx = panel_x + ((panel_w as f32 - text_w) / 2.0).max(0.0) as usize;
    let ty = bar_h + (panel_h as f32 / 3.0) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        tx,
        ty,
        clip_h,
        msg,
        metrics,
        theme::FG_MUTED,
        Family::SansSerif,
    );
}

fn draw_changes_tab(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &GitPanelState,
    panel_x: usize,
    content_y: usize,
    panel_w: usize,
    content_h: usize,
    sf: f32,
    scroll: usize,
    cursor_blink_on: bool,
) {
    let clip_h = content_y + content_h;
    let pad = (COMMIT_SECTION_PAD * sf) as usize;
    let min_input_h = (COMMIT_INPUT_HEIGHT * sf) as usize;
    let input_pad_x = (COMMIT_INPUT_PAD_X * sf) as usize;
    let input_r = (COMMIT_INPUT_RADIUS * sf) as usize;
    let btn_h = (COMMIT_BTN_HEIGHT * sf) as usize;
    let btn_r = (COMMIT_BTN_RADIUS * sf) as usize;
    let max_y = content_y + content_h;
    let max_input_h = (content_h * 2) / 5;

    let mut y = (content_y + pad).saturating_sub(scroll);

    let input_x = panel_x + input_pad_x;
    let input_w = panel_w.saturating_sub(input_pad_x * 2);
    let line_h = (16.0 * sf) as usize;
    let char_w = 7.0 * sf;
    let text_max_px = input_w as f32 - 8.0 * sf - 16.0 * sf - 16.0 * sf;
    let max_chars = (text_max_px / char_w).floor().max(1.0) as usize;
    let visual_lines = wrap_lines(&state.commit_message, max_chars);
    let vline_count = visual_lines.len().max(1);
    let natural_h = min_input_h.max(vline_count * line_h + (8.0 * sf) as usize);
    let input_h = natural_h.min(max_input_h).max(min_input_h);

    if y >= content_y && y + input_h <= max_y {
        let border_color = if state.commit_input_focused {
            theme::PRIMARY
        } else {
            theme::BORDER
        };
        super::overlay::fill_rounded_rect(buf, input_x, y, input_w, input_h, input_r, (18, 18, 22));
        draw_rounded_border(buf, input_x, y, input_w, input_h, border_color, sf);

        let text_x = input_x + (8.0 * sf) as usize;
        let text_metrics = Metrics::new(12.0 * sf, 16.0 * sf);

        if state.generating_commit_msg && state.commit_message.is_empty() {
            let text_y = y + (min_input_h as f32 / 2.0 - 7.0 * sf) as usize;
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                text_x,
                text_y,
                clip_h,
                "Generating…",
                text_metrics,
                theme::FG_MUTED,
                Family::SansSerif,
            );
        } else if state.commit_message.is_empty() {
            let text_y = y + (min_input_h as f32 / 2.0 - 7.0 * sf) as usize;
            let placeholder = format!(
                "Message (\u{2318}Enter on \"{}\")",
                state.data.current_branch
            );
            let truncated = truncate_to_fit(&placeholder, char_w, text_max_px);
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                text_x,
                text_y,
                clip_h,
                &truncated,
                text_metrics,
                theme::FG_MUTED,
                Family::SansSerif,
            );
            if state.commit_input_focused && cursor_blink_on {
                let cursor_w = (1.0 * sf).max(1.0) as usize;
                buf.fill_rect(text_x, text_y, cursor_w, line_h, theme::FG_BRIGHT);
            }
        } else {
            let sel = state.selection_range();
            let text_top = y + (4.0 * sf) as usize;
            let mut ly = text_top;
            for (vline_start, vline_text) in &visual_lines {
                if ly + line_h > y + input_h {
                    break;
                }
                let vline_end = vline_start + vline_text.len();
                if let Some((sel_s, sel_e)) = sel
                    && sel_s < vline_end
                    && sel_e > *vline_start
                {
                    let hl_start = sel_s.max(*vline_start) - vline_start;
                    let hl_end = sel_e.min(vline_end) - vline_start;
                    let hl_x = text_x + (hl_start as f32 * char_w) as usize;
                    let hl_w = ((hl_end - hl_start) as f32 * char_w).max(char_w) as usize;
                    buf.fill_rect(
                        hl_x,
                        ly,
                        hl_w.min(input_w - (8.0 * sf) as usize * 2),
                        line_h,
                        (60, 90, 160),
                    );
                }
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    text_x,
                    ly,
                    clip_h,
                    vline_text,
                    text_metrics,
                    (220, 220, 220),
                    Family::SansSerif,
                );
                if state.commit_input_focused
                    && cursor_blink_on
                    && state.cursor >= *vline_start
                    && state.cursor <= vline_end
                {
                    let col = state.cursor - vline_start;
                    let cx = text_x + (col as f32 * char_w) as usize;
                    let cursor_w = (1.0 * sf).max(1.0) as usize;
                    buf.fill_rect(cx, ly, cursor_w, line_h, theme::FG_BRIGHT);
                }
                ly += line_h;
            }
            if state.commit_input_focused
                && cursor_blink_on
                && state.cursor == state.commit_message.len()
                && state.commit_message.ends_with('\n')
                && ly + line_h <= y + input_h
            {
                let cursor_w = (1.0 * sf).max(1.0) as usize;
                buf.fill_rect(text_x, ly, cursor_w, line_h, theme::FG_BRIGHT);
            }
        }

        let gen_sz = (16.0 * sf).round() as u32;
        let gen_x = input_x + input_w - gen_sz as usize - (8.0 * sf) as usize;
        let gen_y = y + (min_input_h - gen_sz as usize) / 2;
        let (gen_icon, gen_color) = if state.generating_commit_msg {
            (Icon::Stop, theme::PRIMARY)
        } else if state.hovered_generate_btn {
            (Icon::Sparkle, theme::FG_BRIGHT)
        } else {
            (Icon::Sparkle, theme::FG_MUTED)
        };
        icon_renderer.draw(buf, gen_icon, gen_x, gen_y, gen_sz, gen_color);
    }
    y += input_h + pad;

    if y >= content_y && y + btn_h <= max_y {
        let btn_x = panel_x + input_pad_x;
        let btn_w = panel_w.saturating_sub(input_pad_x * 2);
        let btn_color = if state.hovered_commit_btn {
            theme::PRIMARY_HOVER
        } else {
            theme::PRIMARY
        };
        super::overlay::fill_rounded_rect(buf, btn_x, y, btn_w, btn_h, btn_r, btn_color);

        let label = "\u{2713} Commit";
        let label_metrics = Metrics::new(12.5 * sf, 16.0 * sf);
        let label_w = label.len() as f32 * 7.0 * sf;
        let label_x = btn_x + ((btn_w as f32 - label_w) / 2.0).max(0.0) as usize;
        let label_y = y + (btn_h as f32 / 2.0 - 8.0 * sf) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            label_x,
            label_y,
            clip_h,
            label,
            label_metrics,
            (255, 255, 255),
            Family::SansSerif,
        );
    }
    y += btn_h + pad;

    let staged: Vec<_> = state
        .data
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.staged)
        .collect();
    let unstaged: Vec<_> = state
        .data
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.staged)
        .collect();

    if !staged.is_empty() {
        let sh = (SECTION_HEADER_HEIGHT * sf) as usize;
        if y >= content_y && y + sh <= max_y {
            draw_section_header(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                panel_x,
                y,
                panel_w,
                sf,
                clip_h,
                "Staged Changes",
                staged.len(),
                state.hovered_unstage_all,
                true,
            );
        }
        y += sh;

        let item_h = (FILE_ITEM_HEIGHT * sf) as usize;
        for (global_idx, entry) in &staged {
            if y >= max_y {
                break;
            }
            if y + item_h > content_y {
                let hovered = state.hovered_item == Some(*global_idx);
                draw_file_entry(
                    buf,
                    font_system,
                    swash_cache,
                    icon_renderer,
                    entry,
                    panel_x,
                    y,
                    panel_w,
                    item_h,
                    sf,
                    hovered,
                    clip_h,
                    true,
                );
            }
            y += item_h;
        }
        y += (SECTION_PAD_Y * sf) as usize;
    }

    if !unstaged.is_empty() {
        let sh = (SECTION_HEADER_HEIGHT * sf) as usize;
        if y >= content_y && y + sh <= max_y {
            draw_section_header(
                buf,
                font_system,
                swash_cache,
                icon_renderer,
                panel_x,
                y,
                panel_w,
                sf,
                clip_h,
                "Changes",
                unstaged.len(),
                state.hovered_stage_all,
                false,
            );
        }
        y += sh;

        let item_h = (FILE_ITEM_HEIGHT * sf) as usize;
        for (global_idx, entry) in &unstaged {
            if y >= max_y {
                break;
            }
            if y + item_h > content_y {
                let hovered = state.hovered_item == Some(*global_idx);
                draw_file_entry(
                    buf,
                    font_system,
                    swash_cache,
                    icon_renderer,
                    entry,
                    panel_x,
                    y,
                    panel_w,
                    item_h,
                    sf,
                    hovered,
                    clip_h,
                    false,
                );
            }
            y += item_h;
        }
    }

    if staged.is_empty() && unstaged.is_empty() {
        let msg = "No changes";
        let metrics = Metrics::new(12.0 * sf, 16.0 * sf);
        let text_w = msg.len() as f32 * 7.0 * sf;
        let tx = panel_x + ((panel_w as f32 - text_w) / 2.0).max(0.0) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            tx,
            y + (40.0 * sf) as usize,
            clip_h,
            msg,
            metrics,
            theme::FG_MUTED,
            Family::SansSerif,
        );
    }
}

fn commit_zone_y_offsets(sf: f64, state: &GitPanelState) -> (f64, f64, f64, f64, f64) {
    let pad = COMMIT_SECTION_PAD as f64 * sf;
    let min_input_h = COMMIT_INPUT_HEIGHT as f64 * sf;
    let line_h = 16.0 * sf;
    let char_w = 7.0 * sf;
    let input_pad_x = COMMIT_INPUT_PAD_X as f64 * sf;
    let approx_panel_w = 280.0 * sf;
    let text_max_px = approx_panel_w - input_pad_x * 2.0 - 8.0 * sf - 16.0 * sf - 16.0 * sf;
    let max_chars = (text_max_px / char_w).floor().max(1.0) as usize;
    let line_count = visual_line_count(&state.commit_message, max_chars).max(1) as f64;
    let input_h = min_input_h.max(line_count * line_h + 8.0 * sf);
    let btn_h = COMMIT_BTN_HEIGHT as f64 * sf;
    let input_y = pad;
    let input_end = input_y + input_h;
    let btn_y = input_end + pad;
    let btn_end = btn_y + btn_h;
    let list_start = btn_end + pad;
    (input_y, input_end, btn_y, btn_end, list_start)
}

fn changes_hit_test(
    content_rel_y: f64,
    rel_x: f64,
    panel_w: f64,
    state: &GitPanelState,
    sf: f64,
) -> GitPanelHit {
    let (input_y, input_end, btn_y, btn_end, list_start) = commit_zone_y_offsets(sf, state);
    let input_pad_x = COMMIT_INPUT_PAD_X as f64 * sf;
    let input_w = panel_w - input_pad_x * 2.0;

    if content_rel_y >= input_y && content_rel_y < input_end {
        let gen_sz = 16.0 * sf;
        let gen_x = input_w - gen_sz - 8.0 * sf;
        if rel_x >= input_pad_x + gen_x {
            return GitPanelHit::GenerateButton;
        }
        let text_pad = 8.0 * sf;
        let click_x = (rel_x - input_pad_x - text_pad).max(0.0);
        let click_y = (content_rel_y - input_y - 4.0 * sf).max(0.0);
        return GitPanelHit::CommitInput {
            rel_x: click_x,
            rel_y: click_y,
        };
    }

    if content_rel_y >= btn_y && content_rel_y < btn_end {
        return GitPanelHit::CommitButton;
    }

    let item_h = FILE_ITEM_HEIGHT as f64 * sf;
    let section_h = SECTION_HEADER_HEIGHT as f64 * sf;
    let section_pad = SECTION_PAD_Y as f64 * sf;

    let staged: Vec<_> = state
        .data
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.staged)
        .collect();
    let unstaged: Vec<_> = state
        .data
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.staged)
        .collect();

    let header_action_zone = panel_w - 50.0 * sf;
    let file_action_zone = panel_w - 40.0 * sf;

    let mut y = list_start;

    if !staged.is_empty() {
        if content_rel_y >= y && content_rel_y < y + section_h {
            if rel_x >= header_action_zone {
                return GitPanelHit::UnstageAll;
            }
            return GitPanelHit::None;
        }
        y += section_h;
        for (global_idx, _) in &staged {
            if content_rel_y >= y && content_rel_y < y + item_h {
                if rel_x >= file_action_zone {
                    return GitPanelHit::UnstageFile(*global_idx);
                }
                return GitPanelHit::SelectFile(*global_idx);
            }
            y += item_h;
        }
        y += section_pad;
    }

    if !unstaged.is_empty() {
        if content_rel_y >= y && content_rel_y < y + section_h {
            if rel_x >= header_action_zone {
                return GitPanelHit::StageAll;
            }
            return GitPanelHit::None;
        }
        y += section_h;
        for (global_idx, _) in &unstaged {
            if content_rel_y >= y && content_rel_y < y + item_h {
                if rel_x >= file_action_zone {
                    return GitPanelHit::StageFile(*global_idx);
                }
                return GitPanelHit::SelectFile(*global_idx);
            }
            y += item_h;
        }
    }

    GitPanelHit::None
}

fn draw_section_header(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    panel_x: usize,
    y: usize,
    panel_w: usize,
    sf: f32,
    clip_h: usize,
    label: &str,
    count: usize,
    action_hovered: bool,
    is_staged: bool,
) {
    let header_h = (SECTION_HEADER_HEIGHT * sf) as usize;
    let pad_x = (FILE_PAD_X * sf) as usize;

    let chev_sz = (12.0 * sf).round() as u32;
    let chev_x = panel_x + pad_x;
    let chev_y = y + (header_h - chev_sz as usize) / 2;
    icon_renderer.draw(
        buf,
        Icon::ChevronDown,
        chev_x,
        chev_y,
        chev_sz,
        theme::FG_MUTED,
    );

    let label_metrics = Metrics::new(11.5 * sf, 15.0 * sf);
    let label_x = chev_x + chev_sz as usize + (4.0 * sf) as usize;
    let label_y = y + (header_h as f32 / 2.0 - 7.5 * sf) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        label_x,
        label_y,
        clip_h,
        label,
        label_metrics,
        theme::FG_PRIMARY,
        Family::SansSerif,
    );

    let badge_sz = (COUNT_BADGE_SIZE * sf) as usize;
    let badge_r = (COUNT_BADGE_RADIUS * sf) as usize;
    let badge_x = panel_x + panel_w - pad_x - badge_sz;
    let badge_y = y + (header_h - badge_sz) / 2;
    super::overlay::fill_rounded_rect(
        buf,
        badge_x,
        badge_y,
        badge_sz,
        badge_sz,
        badge_r,
        (55, 55, 62),
    );

    let count_str = count.to_string();
    let count_metrics = Metrics::new(10.0 * sf, 13.0 * sf);
    let count_w = count_str.len() as f32 * 6.0 * sf;
    let count_x = badge_x + ((badge_sz as f32 - count_w) / 2.0).max(0.0) as usize;
    let count_y = badge_y + (badge_sz as f32 / 2.0 - 6.5 * sf) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        count_x,
        count_y,
        clip_h,
        &count_str,
        count_metrics,
        (200, 200, 200),
        Family::Monospace,
    );

    let action_sz = (14.0 * sf).round() as u32;
    let action_x = badge_x - action_sz as usize - (6.0 * sf) as usize;
    let action_y = y + (header_h - action_sz as usize) / 2;
    let action_icon = if is_staged { Icon::Close } else { Icon::Plus };
    let action_color = if action_hovered {
        theme::FG_BRIGHT
    } else {
        theme::FG_MUTED
    };
    icon_renderer.draw(
        buf,
        action_icon,
        action_x,
        action_y,
        action_sz,
        action_color,
    );
}

fn draw_file_entry(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    entry: &StatusEntry,
    panel_x: usize,
    y: usize,
    panel_w: usize,
    item_h: usize,
    sf: f32,
    hovered: bool,
    clip_h: usize,
    is_staged: bool,
) {
    if hovered {
        buf.fill_rect(panel_x, y, panel_w, item_h, theme::BG_HOVER);
    }

    let pad_x = (FILE_PAD_X * sf) as usize;
    let metrics = Metrics::new(12.0 * sf, 16.0 * sf);
    let char_w = 7.2 * sf;

    let icon = crate::renderer::icons::icon_for_filename(&entry.path);
    let icon_sz = (14.0 * sf).round() as u32;
    let icon_x = panel_x + pad_x;
    let icon_y = y + (item_h - icon_sz as usize) / 2;
    icon_renderer.draw_colored(buf, icon, icon_x, icon_y, icon_sz);

    let badge_char = status_badge_char(entry.status);
    let badge_color = status_color(entry.status);
    let badge_w = (badge_char.len() as f32 * 7.0 * sf) as usize;
    let action_sz = (14.0 * sf).round() as u32;
    let right_reserved = pad_x + badge_w + action_sz as usize + (12.0 * sf) as usize;

    let text_x = icon_x + icon_sz as usize + (6.0 * sf) as usize;
    let text_y = y + (item_h as f32 / 2.0 - 8.0 * sf) as usize;
    let text_color = if hovered {
        theme::TAB_ACTIVE_TEXT
    } else {
        (200, 200, 200)
    };
    let text_max_px = (panel_x + panel_w).saturating_sub(text_x + right_reserved) as f32;

    let basename = entry.path.rsplit('/').next().unwrap_or(&entry.path);
    let parent = entry.path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");

    if parent.is_empty() {
        let truncated = truncate_to_fit(basename, char_w, text_max_px);
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            text_x,
            text_y,
            clip_h,
            &truncated,
            metrics,
            text_color,
            Family::SansSerif,
        );
    } else {
        let name_max = text_max_px * 0.6;
        let truncated_name = truncate_to_fit(basename, char_w, name_max);
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            text_x,
            text_y,
            clip_h,
            &truncated_name,
            metrics,
            text_color,
            Family::SansSerif,
        );

        let name_drawn_w = (truncated_name.len() as f32 * char_w) as usize;
        let dir_x = text_x + name_drawn_w + (4.0 * sf) as usize;
        let dir_max = text_max_px - name_drawn_w as f32 - 4.0 * sf;
        if dir_max > 10.0 * sf {
            let dir_metrics = Metrics::new(11.0 * sf, 14.0 * sf);
            let dir_char_w = 6.5 * sf;
            let truncated_dir = truncate_to_fit(parent, dir_char_w, dir_max);
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                dir_x,
                text_y,
                clip_h,
                &truncated_dir,
                dir_metrics,
                theme::FG_MUTED,
                Family::SansSerif,
            );
        }
    }

    let badge_metrics = Metrics::new(11.0 * sf, 14.0 * sf);
    let badge_x = panel_x + panel_w - pad_x - badge_w;
    let badge_y = y + (item_h as f32 / 2.0 - 7.0 * sf) as usize;
    draw_text_at(
        buf,
        font_system,
        swash_cache,
        badge_x,
        badge_y,
        clip_h,
        badge_char,
        badge_metrics,
        badge_color,
        Family::Monospace,
    );

    if hovered {
        let action_x = badge_x.saturating_sub(action_sz as usize + (6.0 * sf) as usize);
        let action_y = y + (item_h - action_sz as usize) / 2;
        let action_icon = if is_staged { Icon::Close } else { Icon::Plus };
        icon_renderer.draw(
            buf,
            action_icon,
            action_x,
            action_y,
            action_sz,
            theme::FG_BRIGHT,
        );
    }
}

fn draw_rounded_border(
    buf: &mut PixelBuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: Rgb,
    sf: f32,
) {
    let t = (1.0 * sf).max(1.0) as usize;
    buf.fill_rect(x, y, w, t, color);
    buf.fill_rect(x, y + h - t, w, t, color);
    buf.fill_rect(x, y, t, h, color);
    buf.fill_rect(x + w - t, y, t, h, color);
}

fn status_badge_char(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Modified => "M",
        FileStatus::Added => "A",
        FileStatus::Deleted => "D",
        FileStatus::Renamed => "R",
        FileStatus::Untracked => "U",
        FileStatus::Conflicted => "!",
    }
}

fn status_color(status: FileStatus) -> Rgb {
    match status {
        FileStatus::Modified => COLOR_MODIFIED,
        FileStatus::Added => COLOR_ADDED,
        FileStatus::Deleted => COLOR_DELETED,
        FileStatus::Renamed => COLOR_RENAMED,
        FileStatus::Untracked => COLOR_UNTRACKED,
        FileStatus::Conflicted => COLOR_CONFLICTED,
    }
}

fn draw_branches_tab(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &GitPanelState,
    panel_x: usize,
    content_y: usize,
    panel_w: usize,
    content_h: usize,
    sf: f32,
    scroll: usize,
) {
    let clip_h = content_y + content_h;
    let item_h = (FILE_ITEM_HEIGHT * sf) as usize;
    let pad_x = (FILE_PAD_X * sf) as usize;
    let metrics = Metrics::new(12.0 * sf, 16.0 * sf);

    if state.data.branches.is_empty() {
        let msg = "No branches";
        let ty = (content_y + (SECTION_PAD_Y * sf) as usize).saturating_sub(scroll);
        if ty >= content_y {
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                panel_x + pad_x,
                ty,
                clip_h,
                msg,
                metrics,
                theme::FG_MUTED,
                Family::SansSerif,
            );
        }
        return;
    }

    let section_pad = (SECTION_PAD_Y * sf) as usize;
    let char_w = 7.0 * sf;
    let max_text_px = (panel_w - pad_x * 2) as f32;
    let mut y = (content_y + section_pad).saturating_sub(scroll);
    let max_y = content_y + content_h;

    let local: Vec<_> = state
        .data
        .branches
        .iter()
        .filter(|b| !b.is_remote)
        .collect();
    let remote: Vec<_> = state.data.branches.iter().filter(|b| b.is_remote).collect();

    if !local.is_empty() {
        let label_metrics = Metrics::new(11.0 * sf, 15.0 * sf);
        if y >= content_y && y + item_h <= max_y {
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                panel_x + pad_x,
                y,
                clip_h,
                &format!("Local ({})", local.len()),
                label_metrics,
                theme::FG_MUTED,
                Family::SansSerif,
            );
        }
        y += item_h;
        for branch in &local {
            if y >= max_y {
                break;
            }
            if y + item_h > content_y {
                if branch.is_current {
                    buf.fill_rect(panel_x, y, panel_w, item_h, theme::BG_ELEVATED);
                }
                let icon_sz = (14.0 * sf).round() as u32;
                let icon_x = panel_x + pad_x;
                let icon_y = y + (item_h - icon_sz as usize) / 2;
                let icon_color = if branch.is_current {
                    theme::PRIMARY
                } else {
                    theme::FG_MUTED
                };
                icon_renderer.draw(buf, Icon::GitBranch, icon_x, icon_y, icon_sz, icon_color);
                let text_x = icon_x + icon_sz as usize + (6.0 * sf) as usize;
                let text_y = y + (item_h as f32 / 2.0 - 8.0 * sf) as usize;
                let text_color = if branch.is_current {
                    (255, 255, 255)
                } else {
                    (200, 200, 200)
                };
                let branch_max = max_text_px - icon_sz as f32 - 6.0 * sf;
                let display = truncate_to_fit(&branch.name, char_w, branch_max);
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    text_x,
                    text_y,
                    clip_h,
                    &display,
                    metrics,
                    text_color,
                    Family::SansSerif,
                );
            }
            y += item_h;
        }
    }

    if !remote.is_empty() {
        y += section_pad;
        let label_metrics = Metrics::new(11.0 * sf, 15.0 * sf);
        if y >= content_y && y + item_h <= max_y {
            draw_text_at(
                buf,
                font_system,
                swash_cache,
                panel_x + pad_x,
                y,
                clip_h,
                &format!("Remote ({})", remote.len()),
                label_metrics,
                theme::FG_MUTED,
                Family::SansSerif,
            );
        }
        y += item_h;
        for branch in &remote {
            if y >= max_y {
                break;
            }
            if y + item_h > content_y {
                let text_y = y + (item_h as f32 / 2.0 - 8.0 * sf) as usize;
                let display = truncate_to_fit(&branch.name, char_w, max_text_px);
                draw_text_at(
                    buf,
                    font_system,
                    swash_cache,
                    panel_x + pad_x,
                    text_y,
                    clip_h,
                    &display,
                    metrics,
                    theme::FG_MUTED,
                    Family::SansSerif,
                );
            }
            y += item_h;
        }
    }
}

fn branches_hit_test(content_rel_y: f64, state: &GitPanelState, sf: f64) -> GitPanelHit {
    let item_h = FILE_ITEM_HEIGHT as f64 * sf;
    let section_pad = SECTION_PAD_Y as f64 * sf;
    let local: Vec<_> = state
        .data
        .branches
        .iter()
        .enumerate()
        .filter(|(_, b)| !b.is_remote)
        .collect();
    let mut y = section_pad;
    if !local.is_empty() {
        y += item_h;
        for (idx, _) in &local {
            if content_rel_y >= y && content_rel_y < y + item_h {
                return GitPanelHit::CheckoutBranch(*idx);
            }
            y += item_h;
        }
    }
    GitPanelHit::None
}

const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_MARGIN: f32 = 2.0;
const SCROLLBAR_MIN_THUMB: f32 = 20.0;
const SCROLLBAR_COLOR: Rgb = (80, 84, 96);
const SCROLLBAR_HOVER_COLOR: Rgb = crate::renderer::theme::SCROLLBAR_THUMB_HOVER;

/// Scrollbar thumb rect for the git panel: (x, y, w, h). None if content fits.
pub fn git_scrollbar_thumb_rect(
    panel_x: usize,
    panel_w: usize,
    y_start: usize,
    visible_h: usize,
    total_h: usize,
    scroll: usize,
    sf: f32,
) -> Option<(usize, usize, usize, usize)> {
    if total_h <= visible_h || visible_h == 0 {
        return None;
    }
    let sb_w = (SCROLLBAR_WIDTH * sf).max(4.0) as usize;
    let sb_margin = (SCROLLBAR_MARGIN * sf) as usize;
    let border_w = (1.0 * sf).max(1.0) as usize;
    let track_x = panel_x + panel_w.saturating_sub(sb_w + sb_margin + border_w);
    let track_h = visible_h;
    let thumb_h = ((visible_h as f64 / total_h as f64) * track_h as f64)
        .max(SCROLLBAR_MIN_THUMB as f64 * sf as f64) as usize;
    let max_scroll = total_h.saturating_sub(visible_h);
    let frac = if max_scroll > 0 {
        scroll.min(max_scroll) as f64 / max_scroll as f64
    } else {
        0.0
    };
    let thumb_y = y_start + (frac * (track_h.saturating_sub(thumb_h)) as f64) as usize;
    Some((track_x, thumb_y, sb_w, thumb_h))
}

/// Hit-test: is (px, py) inside the git panel scrollbar thumb?
pub fn git_scrollbar_hit_test(
    px: usize,
    py: usize,
    panel_x: usize,
    panel_w: usize,
    y_start: usize,
    visible_h: usize,
    total_h: usize,
    scroll: usize,
    sf: f32,
) -> bool {
    if let Some((tx, ty, tw, th)) =
        git_scrollbar_thumb_rect(panel_x, panel_w, y_start, visible_h, total_h, scroll, sf)
    {
        let margin = (4.0 * sf) as usize;
        px + margin >= tx && px < tx + tw + margin && py >= ty && py < ty + th
    } else {
        false
    }
}

fn draw_git_scrollbar(
    buf: &mut PixelBuffer,
    panel_x: usize,
    panel_w: usize,
    y_start: usize,
    visible_h: usize,
    total_h: usize,
    scroll: usize,
    hovered: bool,
    sf: f32,
) {
    if let Some((tx, ty, tw, th)) =
        git_scrollbar_thumb_rect(panel_x, panel_w, y_start, visible_h, total_h, scroll, sf)
    {
        let color = if hovered {
            SCROLLBAR_HOVER_COLOR
        } else {
            SCROLLBAR_COLOR
        };
        buf.fill_rect(tx, ty, tw, th, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_panel_state_default() {
        let s = GitPanelState::default();
        assert!(!s.data.has_repo);
        assert!(s.data.entries.is_empty());
        assert!(s.commit_message.is_empty());
        assert!(!s.commit_input_focused);
    }

    #[test]
    fn staged_unstaged_split() {
        let mut s = GitPanelState::default();
        s.data.entries = vec![
            StatusEntry {
                path: "a.rs".into(),
                status: FileStatus::Modified,
                staged: true,
            },
            StatusEntry {
                path: "b.rs".into(),
                status: FileStatus::Added,
                staged: false,
            },
            StatusEntry {
                path: "c.rs".into(),
                status: FileStatus::Deleted,
                staged: true,
            },
        ];
        let staged: Vec<_> = s.data.entries.iter().filter(|e| e.staged).collect();
        let unstaged: Vec<_> = s.data.entries.iter().filter(|e| !e.staged).collect();
        assert_eq!(staged.len(), 2);
        assert_eq!(unstaged.len(), 1);
    }

    #[test]
    fn status_badges() {
        assert_eq!(status_badge_char(FileStatus::Modified), "M");
        assert_eq!(status_badge_char(FileStatus::Added), "A");
        assert_eq!(status_badge_char(FileStatus::Deleted), "D");
        assert_eq!(status_badge_char(FileStatus::Renamed), "R");
        assert_eq!(status_badge_char(FileStatus::Untracked), "U");
        assert_eq!(status_badge_char(FileStatus::Conflicted), "!");
    }

    #[test]
    fn status_colors_unique() {
        let colors = [
            status_color(FileStatus::Modified),
            status_color(FileStatus::Added),
            status_color(FileStatus::Deleted),
            status_color(FileStatus::Renamed),
            status_color(FileStatus::Untracked),
            status_color(FileStatus::Conflicted),
        ];
        for (i, a) in colors.iter().enumerate() {
            for (j, b) in colors.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn toolbar_hit_detects_tabs() {
        let sf = 2.0;
        let header_h = HEADER_HEIGHT as f64 * sf;
        let pad = TOOLBAR_PAD_X as f64 * sf;
        let x = pad + 5.0;
        assert_eq!(
            toolbar_hit_test(x, header_h, sf),
            GitPanelHit::ToolbarChanges
        );
    }

    #[test]
    fn git_panel_hit_none_outside() {
        let state = GitPanelState::default();
        let layout = PanelLayout::default();
        let hit = hit_test(0.0, 0.0, &state, &layout, 42.0, 2000, 2.0);
        assert_eq!(hit, GitPanelHit::None);
    }

    #[test]
    fn commit_zone_offsets_positive() {
        let state = GitPanelState::default();
        let (iy, ie, by, be, ls) = commit_zone_y_offsets(2.0, &state);
        assert!(iy < ie);
        assert!(ie < by);
        assert!(by < be);
        assert!(be < ls);
    }
}
