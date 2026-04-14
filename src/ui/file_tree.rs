//! File tree — expandable directory tree for the side panel Files tab.
//!
//! Provides a recursive tree model, expand/collapse state tracking,
//! and pixel-based rendering with chevrons + file/folder icons.
//!
//! **Performance strategy:** the flat row list is cached and only rebuilt
//! when the tree structure changes (load / expand / collapse).  Drawing
//! and hit-testing read from the cache — no per-frame allocation.  Text
//! rendering uses a reusable `cosmic_text::Buffer` with `Shaping::Basic`
//! for maximum throughput.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use cosmic_text::{Buffer, Family, FontSystem, Metrics, SwashCache};

use crate::renderer::icons::{Icon, IconRenderer, icon_for_filename};
use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::draw_text_at_buffered;
use crate::renderer::theme;

const ITEM_HEIGHT: f32 = 24.0;
const INDENT_WIDTH: f32 = 16.0;
const ICON_SIZE: f32 = 14.0;
const ICON_GAP: f32 = 6.0;
const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 6.0;
const FONT_SIZE: f32 = 12.0;
const LINE_HEIGHT: f32 = 17.0;

/// Exposed for scrollbar total-height calculation.
pub const ITEM_HEIGHT_PX: f32 = ITEM_HEIGHT;
pub const PAD_Y_PX: f32 = PAD_Y;

/// A single entry in the file tree.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Build a tree from a directory root. Reads one level eagerly;
    /// deeper levels are loaded on expand (lazy).
    pub fn from_dir(path: &Path) -> Option<Self> {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .to_string();

        if !path.is_dir() {
            return Some(TreeNode {
                name,
                path: path.to_path_buf(),
                is_dir: false,
                children: Vec::new(),
            });
        }

        let mut children = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let child_path = entry.path();
                let child_name = entry.file_name().to_string_lossy().to_string();
                if child_name == ".git" {
                    continue;
                }
                let is_dir = child_path.is_dir();
                children.push(TreeNode {
                    name: child_name,
                    path: child_path,
                    is_dir,
                    children: Vec::new(),
                });
            }
        }
        sort_children(&mut children);

        Some(TreeNode {
            name,
            path: path.to_path_buf(),
            is_dir: true,
            children,
        })
    }

    /// Re-read this directory's children from the filesystem,
    /// preserving already-loaded subtrees for expanded directories.
    pub fn reload_children(&mut self) {
        if !self.is_dir {
            return;
        }
        let mut old: std::collections::HashMap<PathBuf, TreeNode> = self
            .children
            .drain(..)
            .map(|c| (c.path.clone(), c))
            .collect();

        if let Ok(entries) = std::fs::read_dir(&self.path) {
            for entry in entries.flatten() {
                let child_path = entry.path();
                let child_name = entry.file_name().to_string_lossy().to_string();
                if child_name == ".git" {
                    continue;
                }
                if let Some(existing) = old.remove(&child_path) {
                    self.children.push(existing);
                } else {
                    let is_dir = child_path.is_dir();
                    self.children.push(TreeNode {
                        name: child_name,
                        path: child_path,
                        is_dir,
                        children: Vec::new(),
                    });
                }
            }
            sort_children(&mut self.children);
        }
    }
}

fn sort_children(children: &mut [TreeNode]) {
    children.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// Owned snapshot of one visible row — cheap to keep around.
#[derive(Debug, Clone)]
pub struct FlatRow {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
}

/// Tracks expanded directories, cached flat list, and hover state.
pub struct FileTreeState {
    pub root: Option<TreeNode>,
    /// Set of expanded directory paths.
    pub expanded: HashSet<PathBuf>,
    /// Visual index of the hovered item (0-based flat list).
    pub hovered_idx: Option<usize>,
    pub scroll_offset: f32,
    pub scrollbar_hovered: bool,
    pub scrollbar_dragging: bool,
    pub scrollbar_drag_start_y: f64,
    pub scrollbar_drag_start_scroll: f32,
    /// Cached flattened rows — rebuilt only on structural change.
    flat_cache: Vec<FlatRow>,
}

impl Default for FileTreeState {
    fn default() -> Self {
        Self {
            root: None,
            expanded: HashSet::new(),
            hovered_idx: None,
            scroll_offset: 0.0,
            scrollbar_hovered: false,
            scrollbar_dragging: false,
            scrollbar_drag_start_y: 0.0,
            scrollbar_drag_start_scroll: 0.0,
            flat_cache: Vec::new(),
        }
    }
}

impl FileTreeState {
    /// Initialise from a working directory.
    pub fn load(&mut self, cwd: &Path) {
        if self.root.as_ref().map(|r| &r.path) == Some(&cwd.to_path_buf()) {
            return;
        }
        self.root = TreeNode::from_dir(cwd);
        self.expanded.clear();
        if let Some(root) = &self.root {
            self.expanded.insert(root.path.clone());
        }
        self.rebuild_flat_cache();
    }

    /// Toggle expand/collapse for a directory path.
    pub fn toggle_expand(&mut self, path: &Path) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.to_path_buf());
            if let Some(root) = &mut self.root {
                load_children_at(root, path);
            }
        }
        self.rebuild_flat_cache();
    }

    /// Number of visible (flattened) rows — O(1).
    pub fn row_count(&self) -> usize {
        self.flat_cache.len()
    }

    /// Rebuild the cached flat list from the current tree + expanded set.
    fn rebuild_flat_cache(&mut self) {
        self.flat_cache.clear();
        if let Some(root) = &self.root {
            flatten_into(root, &self.expanded, 0, &mut self.flat_cache);
        }
    }

    /// Public wrapper to rebuild the flat cache after external modifications.
    pub fn rebuild_cache(&mut self) {
        self.rebuild_flat_cache();
    }
}

/// Recursively find the node at `target_path` and reload its children from disk.
pub fn load_children_at(node: &mut TreeNode, target_path: &Path) {
    if node.path == target_path {
        node.reload_children();
        return;
    }
    for child in &mut node.children {
        if target_path.starts_with(&child.path) {
            load_children_at(child, target_path);
            return;
        }
    }
}

fn flatten_into(
    node: &TreeNode,
    expanded: &HashSet<PathBuf>,
    depth: usize,
    out: &mut Vec<FlatRow>,
) {
    if depth == 0 {
        if expanded.contains(&node.path) {
            for child in &node.children {
                flatten_into(child, expanded, 1, out);
            }
        }
        return;
    }
    out.push(FlatRow {
        name: node.name.clone(),
        path: node.path.clone(),
        is_dir: node.is_dir,
        depth: depth - 1,
    });
    if node.is_dir && expanded.contains(&node.path) {
        for child in &node.children {
            flatten_into(child, expanded, depth + 1, out);
        }
    }
}

/// Draw the file tree inside the side panel.
pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &FileTreeState,
    active_path: Option<&Path>,
    panel_w: usize,
    y_start: usize,
    sf: f32,
) {
    let rows = &state.flat_cache;
    if rows.is_empty() {
        return;
    }

    let item_h = (ITEM_HEIGHT * sf) as usize;
    let indent = (INDENT_WIDTH * sf) as usize;
    let icon_sz = (ICON_SIZE * sf).round() as u32;
    let icon_gap = (ICON_GAP * sf) as usize;
    let pad_x = (PAD_X * sf) as usize;
    let pad_y = (PAD_Y * sf) as usize;
    let metrics = Metrics::new(FONT_SIZE * sf, LINE_HEIGHT * sf);

    let mut text_buf = Buffer::new(font_system, metrics);

    let scroll = state.scroll_offset as usize;
    let first_visible = scroll / item_h.max(1);
    let visible_count = (buf.height.saturating_sub(y_start) / item_h.max(1)) + 2;
    let last_visible = (first_visible + visible_count).min(rows.len());

    for (i, row) in rows
        .iter()
        .enumerate()
        .skip(first_visible)
        .take(last_visible - first_visible)
    {
        let y = y_start + pad_y + i * item_h - scroll;
        if y + item_h < y_start || y >= buf.height {
            continue;
        }

        let is_hovered = state.hovered_idx == Some(i);
        let is_active = !row.is_dir && active_path.is_some_and(|p| p == row.path);

        if is_active {
            buf.fill_rect(0, y, panel_w, item_h, theme::BG_SELECTION);
            let accent_w = (2.0 * sf).max(1.0) as usize;
            buf.fill_rect(0, y, accent_w, item_h, theme::PRIMARY);
        } else if is_hovered {
            buf.fill_rect(0, y, panel_w, item_h, theme::BG_HOVER);
        }

        let x_base = pad_x + row.depth * indent;

        let mut x = x_base;
        if row.is_dir {
            let chevron_y = y + ((item_h as f32 - icon_sz as f32) / 2.0).max(0.0) as usize;
            let chevron_icon = if state.expanded.contains(&row.path) {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            };
            let chevron_color = if is_hovered || is_active {
                theme::FG_PRIMARY
            } else {
                theme::FG_MUTED
            };
            icon_renderer.draw(buf, chevron_icon, x, chevron_y, icon_sz, chevron_color);
        }
        x += icon_sz as usize + (2.0 * sf) as usize;

        let icon_y = y + ((item_h as f32 - icon_sz as f32) / 2.0).max(0.0) as usize;
        if row.is_dir {
            icon_renderer.draw(buf, Icon::Folder, x, icon_y, icon_sz, theme::FG_SECONDARY);
        } else {
            let ft_icon = icon_for_filename(&row.name);
            icon_renderer.draw_colored(buf, ft_icon, x, icon_y, icon_sz);
        }
        x += icon_sz as usize + icon_gap;

        let name_y = y + ((item_h as f32 - LINE_HEIGHT * sf) / 2.0).max(0.0) as usize;
        let max_name_px = panel_w.saturating_sub(x + pad_x);
        let max_chars = (max_name_px as f32 / (7.0 * sf)).max(1.0) as usize;
        let needs_truncation = row.name.len() > max_chars && max_chars > 3;
        let truncated;
        let display_name: &str = if needs_truncation {
            truncated = format!("{}…", &row.name[..max_chars.saturating_sub(1)]);
            &truncated
        } else {
            &row.name
        };

        let name_color = if is_active || is_hovered {
            theme::FG_BRIGHT
        } else {
            theme::FG_PRIMARY
        };
        draw_text_at_buffered(
            buf,
            font_system,
            swash_cache,
            &mut text_buf,
            x,
            name_y,
            buf.height,
            display_name,
            metrics,
            name_color,
            Family::SansSerif,
        );
    }
}

/// Hit-test the file tree. Returns the path of the clicked item.
pub fn hit_test(
    phys_y: f64,
    y_start: usize,
    scroll_offset: f32,
    state: &FileTreeState,
    sf: f64,
) -> Option<PathBuf> {
    let rows = &state.flat_cache;
    let item_h = ITEM_HEIGHT as f64 * sf;
    let pad_y = PAD_Y as f64 * sf;
    let rel_y = phys_y - y_start as f64 - pad_y + scroll_offset as f64;
    if rel_y < 0.0 {
        return None;
    }
    let idx = (rel_y / item_h) as usize;
    rows.get(idx).map(|r| r.path.clone())
}

/// Update hover state for mouse position — O(1), no tree walk.
/// Returns `true` if the hover state actually changed (for redraw gating).
pub fn update_hover(
    phys_x: f64,
    phys_y: f64,
    panel_w: usize,
    y_start: usize,
    scroll_offset: f32,
    row_count: usize,
    state: &mut FileTreeState,
    sf: f64,
) -> bool {
    let prev = state.hovered_idx;

    if phys_x < 0.0 || phys_x >= panel_w as f64 {
        state.hovered_idx = None;
        return state.hovered_idx != prev;
    }

    let item_h = ITEM_HEIGHT as f64 * sf;
    let pad_y = PAD_Y as f64 * sf;
    let rel_y = phys_y - y_start as f64 - pad_y + scroll_offset as f64;
    if rel_y < 0.0 {
        state.hovered_idx = None;
        return state.hovered_idx != prev;
    }
    let idx = (rel_y / item_h) as usize;
    state.hovered_idx = if idx < row_count { Some(idx) } else { None };
    state.hovered_idx != prev
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tree_node_from_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hello.txt"), "hi").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/nested.rs"), "fn main(){}").unwrap();

        let node = TreeNode::from_dir(dir.path()).unwrap();
        assert!(node.is_dir);
        assert_eq!(node.children.len(), 2);
        assert!(node.children[0].is_dir);
        assert!(!node.children[1].is_dir);
    }

    #[test]
    fn expand_collapse() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "").unwrap();

        let mut state = FileTreeState::default();
        state.load(dir.path());
        assert!(state.expanded.contains(dir.path()));

        let src_path = dir.path().join("src");
        assert!(!state.expanded.contains(&src_path));

        state.toggle_expand(&src_path);
        assert!(state.expanded.contains(&src_path));

        state.toggle_expand(&src_path);
        assert!(!state.expanded.contains(&src_path));
    }

    #[test]
    fn dotfiles_visible_except_git() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "").unwrap();
        fs::write(dir.path().join(".dockerignore"), "").unwrap();
        fs::write(dir.path().join("visible.txt"), "").unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir(dir.path().join(".config")).unwrap();

        let node = TreeNode::from_dir(dir.path()).unwrap();
        let names: Vec<&str> = node.children.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&".gitignore"));
        assert!(names.contains(&".dockerignore"));
        assert!(names.contains(&".config"));
        assert!(names.contains(&"visible.txt"));
        assert!(!names.contains(&".git"));
    }

    #[test]
    fn flatten_respects_expansion() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        fs::write(dir.path().join("a/x.txt"), "").unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();

        let mut state = FileTreeState::default();
        state.load(dir.path());

        assert_eq!(state.row_count(), 2);

        let a_path = dir.path().join("a");
        state.toggle_expand(&a_path);
        assert_eq!(state.row_count(), 3);
    }

    #[test]
    fn flat_cache_updated_on_toggle() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "").unwrap();
        fs::write(dir.path().join("README.md"), "").unwrap();

        let mut state = FileTreeState::default();
        state.load(dir.path());

        assert_eq!(state.row_count(), 2);
        assert_eq!(state.flat_cache[0].name, "src");
        assert!(state.flat_cache[0].is_dir);

        state.toggle_expand(&dir.path().join("src"));
        assert_eq!(state.row_count(), 3);
        assert_eq!(state.flat_cache[1].name, "lib.rs");

        state.toggle_expand(&dir.path().join("src"));
        assert_eq!(state.row_count(), 2);
    }
}
