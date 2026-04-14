//! Editor state and cursor management.

use std::path::{Path, PathBuf};

use crate::ui::syntax::{SyntaxRegistry, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Text,
    Image,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMove {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    WordLeft,
    WordRight,
    DocumentStart,
    DocumentEnd,
    PageUp(usize),
    PageDown(usize),
}

/// Kind of row in a side-by-side diff view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffRowKind {
    Context,
    Added,
    Removed,
    Modified,
    Separator,
}

/// A single row in the side-by-side diff view.
#[derive(Debug, Clone)]
pub struct DiffRow {
    pub left_num: Option<usize>,
    pub left_text: Option<String>,
    pub right_num: Option<usize>,
    pub right_text: Option<String>,
    pub kind: DiffRowKind,
}

pub struct EditorState {
    pub path: PathBuf,
    pub mode: EditorMode,

    // Text mode
    pub lines: Vec<String>,

    // Hex mode
    pub raw_bytes: Vec<u8>,

    // Image mode
    pub image_rgba: Vec<u8>,
    pub image_width: u32,
    pub image_height: u32,

    // Scroll
    pub scroll_offset: f32,
    scroll_x: f32,

    // Word wrap (visual-only, toggled with Alt+Z)
    word_wrap: bool,

    // Cursor (line index, byte-offset column)
    cursor_line: usize,
    cursor_col: usize,

    // Selection anchor — None means no selection
    sel_anchor: Option<(usize, usize)>,

    // Dirty flag — set when content differs from disk
    modified: bool,

    // Syntax highlighting cache
    syntax_config_idx: Option<usize>,
    highlight_cache: Vec<Token>,
    highlight_dirty: bool,

    /// Side-by-side diff rows — when present, the renderer draws split diff mode.
    pub diff_view: Option<Vec<DiffRow>>,

    /// Fraction (0.0–1.0) controlling the left/right split in diff mode.
    pub diff_split_frac: f32,
}

impl EditorState {
    pub fn file_name(&self) -> String {
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        if self.modified {
            format!("● {}", name)
        } else {
            name
        }
    }

    pub fn content_height(&self, sf: f32) -> f32 {
        if let Some(ref rows) = self.diff_view {
            let line_h = TEXT_LINE_HEIGHT * sf;
            return rows.len() as f32 * line_h + TEXT_PAD_Y * sf * 2.0;
        }
        match self.mode {
            EditorMode::Text => {
                let line_h = TEXT_LINE_HEIGHT * sf;
                self.lines.len() as f32 * line_h + TEXT_PAD_Y * sf * 2.0
            }
            EditorMode::Hex => {
                let row_count = self.raw_bytes.len().div_ceil(16);
                let line_h = HEX_LINE_HEIGHT * sf;
                row_count as f32 * line_h + HEX_PAD_Y * sf * 2.0
            }
            EditorMode::Image => self.image_height as f32,
        }
    }

    pub fn cursor_line(&self) -> usize {
        self.cursor_line
    }
    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }
    pub fn is_modified(&self) -> bool {
        self.modified
    }
    pub fn scroll_x(&self) -> f32 {
        self.scroll_x
    }

    pub fn has_selection(&self) -> bool {
        self.sel_anchor
            .is_some_and(|(al, ac)| al != self.cursor_line || ac != self.cursor_col)
    }

    /// Ordered selection range: (start_line, start_col, end_line, end_col).
    pub fn selection_range(&self) -> Option<(usize, usize, usize, usize)> {
        let (al, ac) = self.sel_anchor?;
        let (cl, cc) = (self.cursor_line, self.cursor_col);
        if !self.has_selection() {
            return None;
        }
        if (al, ac) <= (cl, cc) {
            Some((al, ac, cl, cc))
        } else {
            Some((cl, cc, al, ac))
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let (sl, sc, el, ec) = self.selection_range()?;
        if sl == el {
            let line = &self.lines[sl];
            Some(safe_slice(line, sc, ec).to_string())
        } else {
            let mut result = String::new();
            result.push_str(safe_slice_from(&self.lines[sl], sc));
            for l in (sl + 1)..el {
                result.push('\n');
                result.push_str(&self.lines[l]);
            }
            result.push('\n');
            result.push_str(safe_slice_to(&self.lines[el], ec));
            Some(result)
        }
    }

    /// Query: get highlight tokens for a byte range within the full source.
    /// Returns tokens with offsets relative to `line_start`.
    pub fn tokens_for_line(&self, line_start: usize, line_end: usize) -> &[Token] {
        if self.highlight_cache.is_empty() {
            return &[];
        }
        let start_idx = self
            .highlight_cache
            .partition_point(|t| t.end <= line_start);
        let end_idx = self.highlight_cache.partition_point(|t| t.start < line_end);
        &self.highlight_cache[start_idx..end_idx]
    }

    /// Query: whether this editor has syntax highlighting available.
    pub fn has_syntax(&self) -> bool {
        self.syntax_config_idx.is_some()
    }

    /// Query: whether this editor is showing a side-by-side diff.
    pub fn has_diff_view(&self) -> bool {
        self.diff_view.is_some()
    }
}

impl EditorState {
    pub fn open(path: &Path, syntax: Option<&mut SyntaxRegistry>) -> Result<Self, std::io::Error> {
        let mode = detect_mode(path);
        let syntax_config_idx = syntax.and_then(|s| s.config_for_path(path));

        let mut state = Self {
            path: path.to_path_buf(),
            mode,
            lines: Vec::new(),
            raw_bytes: Vec::new(),
            image_rgba: Vec::new(),
            image_width: 0,
            image_height: 0,
            scroll_offset: 0.0,
            scroll_x: 0.0,
            word_wrap: false,
            cursor_line: 0,
            cursor_col: 0,
            sel_anchor: None,
            modified: false,
            syntax_config_idx,
            highlight_cache: Vec::new(),
            highlight_dirty: true,
            diff_view: None,
            diff_split_frac: 0.5,
        };

        match mode {
            EditorMode::Text => {
                let content = std::fs::read_to_string(path)?;
                state.lines = content.lines().map(String::from).collect();
                if state.lines.is_empty() {
                    state.lines.push(String::new());
                }
            }
            EditorMode::Image => {
                let bytes = std::fs::read(path)?;
                state.load_image(&bytes);
            }
            EditorMode::Hex => {
                state.raw_bytes = std::fs::read(path)?;
            }
        }

        Ok(state)
    }

    /// Open a file in diff view: load the file and build side-by-side diff rows
    /// from structured hunk data.
    pub fn open_diff(
        path: &Path,
        hunks: &[crate::git::DiffHunkData],
        syntax: Option<&mut SyntaxRegistry>,
    ) -> Result<Self, std::io::Error> {
        let mut state = Self::open(path, syntax)?;
        state.diff_view = Some(build_diff_rows(hunks));
        Ok(state)
    }

    /// Command: recompute syntax highlight tokens from the current source text.
    pub fn refresh_highlights(&mut self, syntax: &SyntaxRegistry) {
        let config_idx = match self.syntax_config_idx {
            Some(idx) => idx,
            None => return,
        };
        if !self.highlight_dirty {
            return;
        }
        let source = self.lines.join("\n");
        self.highlight_cache = syntax.highlight(config_idx, source.as_bytes());
        self.highlight_dirty = false;
    }

    /// Mark content as modified and invalidate highlights.
    fn mark_modified(&mut self) {
        self.modified = true;
        self.highlight_dirty = true;
    }

    /// Place cursor at (line, col). Clears selection.
    pub fn set_cursor_pos(&mut self, line: usize, col: usize) {
        self.cursor_line = line.min(self.lines.len().saturating_sub(1));
        self.cursor_col = col.min(self.lines[self.cursor_line].len());
        self.sel_anchor = None;
    }

    /// Move cursor in a direction. If `selecting`, extend selection.
    pub fn move_cursor(&mut self, dir: CursorMove, selecting: bool) {
        if selecting && self.sel_anchor.is_none() {
            self.sel_anchor = Some((self.cursor_line, self.cursor_col));
        }

        match dir {
            CursorMove::Left => {
                if self.cursor_col > 0 {
                    self.cursor_col =
                        prev_char_boundary(&self.lines[self.cursor_line], self.cursor_col);
                } else if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.cursor_col = self.lines[self.cursor_line].len();
                }
            }
            CursorMove::Right => {
                let line_len = self.lines[self.cursor_line].len();
                if self.cursor_col < line_len {
                    self.cursor_col =
                        next_char_boundary(&self.lines[self.cursor_line], self.cursor_col);
                } else if self.cursor_line + 1 < self.lines.len() {
                    self.cursor_line += 1;
                    self.cursor_col = 0;
                }
            }
            CursorMove::Up => {
                if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
                }
            }
            CursorMove::Down => {
                if self.cursor_line + 1 < self.lines.len() {
                    self.cursor_line += 1;
                    self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
                }
            }
            CursorMove::Home => {
                self.cursor_col = 0;
            }
            CursorMove::End => {
                self.cursor_col = self.lines[self.cursor_line].len();
            }
            CursorMove::WordLeft => {
                let (line, col) =
                    word_boundary_left(&self.lines, self.cursor_line, self.cursor_col);
                self.cursor_line = line;
                self.cursor_col = col;
            }
            CursorMove::WordRight => {
                let (line, col) =
                    word_boundary_right(&self.lines, self.cursor_line, self.cursor_col);
                self.cursor_line = line;
                self.cursor_col = col;
            }
            CursorMove::DocumentStart => {
                self.cursor_line = 0;
                self.cursor_col = 0;
            }
            CursorMove::DocumentEnd => {
                self.cursor_line = self.lines.len() - 1;
                self.cursor_col = self.lines[self.cursor_line].len();
            }
            CursorMove::PageUp(visible_lines) => {
                self.cursor_line = self.cursor_line.saturating_sub(visible_lines);
                self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
            }
            CursorMove::PageDown(visible_lines) => {
                self.cursor_line = (self.cursor_line + visible_lines).min(self.lines.len() - 1);
                self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
            }
        }

        if !selecting {
            self.sel_anchor = None;
        }
    }

    pub fn select_all(&mut self) {
        self.sel_anchor = Some((0, 0));
        let last = self.lines.len() - 1;
        self.cursor_line = last;
        self.cursor_col = self.lines[last].len();
    }

    /// Insert a single character at cursor. Deletes selection first if any.
    pub fn insert_char(&mut self, ch: char) {
        if self.mode != EditorMode::Text {
            return;
        }
        self.delete_selection();
        let line = &mut self.lines[self.cursor_line];
        line.insert(self.cursor_col, ch);
        self.cursor_col += ch.len_utf8();
        self.mark_modified();
    }

    /// Insert a string (e.g. from paste) at cursor. Handles newlines.
    pub fn insert_str(&mut self, s: &str) {
        if self.mode != EditorMode::Text {
            return;
        }
        self.delete_selection();
        for ch in s.chars() {
            if ch == '\n' || ch == '\r' {
                self.new_line();
            } else {
                let line = &mut self.lines[self.cursor_line];
                line.insert(self.cursor_col, ch);
                self.cursor_col += ch.len_utf8();
            }
        }
        self.mark_modified();
    }

    /// Insert newline at cursor, splitting the current line.
    pub fn new_line(&mut self) {
        if self.mode != EditorMode::Text {
            return;
        }
        self.delete_selection();
        let tail = self.lines[self.cursor_line][self.cursor_col..].to_string();
        self.lines[self.cursor_line].truncate(self.cursor_col);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.lines.insert(self.cursor_line, tail);
        self.mark_modified();
    }

    /// Backspace — delete character before cursor, or delete selection.
    pub fn delete_backward(&mut self) {
        if self.mode != EditorMode::Text {
            return;
        }
        if self.delete_selection() {
            return;
        }

        if self.cursor_col > 0 {
            let prev = prev_char_boundary(&self.lines[self.cursor_line], self.cursor_col);
            self.lines[self.cursor_line].drain(prev..self.cursor_col);
            self.cursor_col = prev;
            self.mark_modified();
        } else if self.cursor_line > 0 {
            let removed = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&removed);
            self.mark_modified();
        }
    }

    /// Delete — delete character at cursor, or delete selection.
    pub fn delete_forward(&mut self) {
        if self.mode != EditorMode::Text {
            return;
        }
        if self.delete_selection() {
            return;
        }

        let line_len = self.lines[self.cursor_line].len();
        if self.cursor_col < line_len {
            let next = next_char_boundary(&self.lines[self.cursor_line], self.cursor_col);
            self.lines[self.cursor_line].drain(self.cursor_col..next);
            self.mark_modified();
        } else if self.cursor_line + 1 < self.lines.len() {
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
            self.mark_modified();
        }
    }

    /// Option+Backspace — delete one word backward.
    pub fn delete_word_backward(&mut self) {
        if self.mode != EditorMode::Text {
            return;
        }
        if self.delete_selection() {
            return;
        }
        let (new_line, new_col) =
            word_boundary_left(&self.lines, self.cursor_line, self.cursor_col);
        if new_line == self.cursor_line {
            self.lines[self.cursor_line].drain(new_col..self.cursor_col);
            self.cursor_col = new_col;
        } else {
            let tail = self.lines[self.cursor_line][self.cursor_col..].to_string();
            self.lines[new_line].truncate(new_col);
            self.lines[new_line].push_str(&tail);
            self.lines.drain((new_line + 1)..=self.cursor_line);
            self.cursor_line = new_line;
            self.cursor_col = new_col;
        }
        self.mark_modified();
    }

    /// Option+Delete — delete one word forward.
    pub fn delete_word_forward(&mut self) {
        if self.mode != EditorMode::Text {
            return;
        }
        if self.delete_selection() {
            return;
        }
        let (new_line, new_col) =
            word_boundary_right(&self.lines, self.cursor_line, self.cursor_col);
        if new_line == self.cursor_line {
            self.lines[self.cursor_line].drain(self.cursor_col..new_col);
        } else {
            let tail = self.lines[new_line][new_col..].to_string();
            self.lines[self.cursor_line].truncate(self.cursor_col);
            self.lines[self.cursor_line].push_str(&tail);
            self.lines.drain((self.cursor_line + 1)..=new_line);
        }
        self.mark_modified();
    }

    /// Cmd+Backspace — delete from cursor to line start.
    pub fn delete_to_line_start(&mut self) {
        if self.mode != EditorMode::Text {
            return;
        }
        if self.delete_selection() {
            return;
        }
        if self.cursor_col > 0 {
            self.lines[self.cursor_line].drain(..self.cursor_col);
            self.cursor_col = 0;
            self.mark_modified();
        }
    }

    /// Delete the selected region. Returns true if selection was deleted.
    pub fn delete_selection(&mut self) -> bool {
        let (sl, sc, el, ec) = match self.selection_range() {
            Some(r) => r,
            None => return false,
        };

        if sl == el {
            self.lines[sl].drain(sc..ec);
        } else {
            let tail = self.lines[el][ec..].to_string();
            self.lines[sl].truncate(sc);
            self.lines[sl].push_str(&tail);
            self.lines.drain((sl + 1)..=el);
        }

        self.cursor_line = sl;
        self.cursor_col = sc;
        self.sel_anchor = None;
        self.mark_modified();
        true
    }

    /// Save to disk.
    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if self.mode != EditorMode::Text {
            return Ok(());
        }
        let content: String = self.lines.join("\n");
        std::fs::write(&self.path, &content)?;
        self.modified = false;
        Ok(())
    }

    /// Ensure the cursor line is visible, adjusting scroll_offset.
    pub fn ensure_cursor_visible(&mut self, sf: f32, viewport_h: usize) {
        let line_h = (TEXT_LINE_HEIGHT * sf) as usize;
        let pad_y = (TEXT_PAD_Y * sf) as usize;
        let cursor_y = pad_y + self.cursor_line * line_h;
        let scroll = self.scroll_offset.max(0.0) as usize;

        if cursor_y < scroll {
            self.scroll_offset = cursor_y as f32;
        } else if cursor_y + line_h > scroll + viewport_h {
            self.scroll_offset = (cursor_y + line_h).saturating_sub(viewport_h) as f32;
        }
    }

    /// Command: set horizontal scroll offset.
    pub fn set_scroll_x(&mut self, x: f32) {
        self.scroll_x = x.max(0.0);
    }

    /// Command: toggle word wrap on/off.
    pub fn toggle_word_wrap(&mut self) {
        self.word_wrap = !self.word_wrap;
        if self.word_wrap {
            self.scroll_x = 0.0;
        }
    }

    fn load_image(&mut self, bytes: &[u8]) {
        match image::load_from_memory(bytes) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                self.image_width = rgba.width();
                self.image_height = rgba.height();
                self.image_rgba = rgba.into_raw();
            }
            Err(_) => {
                self.mode = EditorMode::Hex;
                self.raw_bytes = bytes.to_vec();
            }
        }
    }

    pub fn set_cursor_pos_selecting(&mut self, line: usize, col: usize) {
        if self.sel_anchor.is_none() {
            self.sel_anchor = Some((self.cursor_line, self.cursor_col));
        }
        self.cursor_line = line.min(self.lines.len().saturating_sub(1));
        self.cursor_col = col.min(self.lines[self.cursor_line].len());
    }
}

fn word_boundary_left(lines: &[String], line: usize, col: usize) -> (usize, usize) {
    if col == 0 {
        if line == 0 {
            return (0, 0);
        }
        return (line - 1, lines[line - 1].len());
    }

    let s = &lines[line];
    let mut i = prev_char_boundary(s, col);

    while i > 0 && s.as_bytes()[i - 1].is_ascii_whitespace() {
        i = prev_char_boundary(s, i);
    }

    while i > 0 && !s.as_bytes()[i - 1].is_ascii_whitespace() && !is_word_sep(s.as_bytes()[i - 1]) {
        i = prev_char_boundary(s, i);
    }
    (line, i)
}

fn word_boundary_right(lines: &[String], line: usize, col: usize) -> (usize, usize) {
    let s = &lines[line];
    if col >= s.len() {
        if line + 1 >= lines.len() {
            return (line, s.len());
        }
        return (line + 1, 0);
    }

    let mut i = col;

    while i < s.len() && !s.as_bytes()[i].is_ascii_whitespace() && !is_word_sep(s.as_bytes()[i]) {
        i = next_char_boundary(s, i);
    }

    while i < s.len() && s.as_bytes()[i].is_ascii_whitespace() {
        i = next_char_boundary(s, i);
    }
    (line, i)
}

fn is_word_sep(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'"'
            | b'\''
            | b','
            | b';'
            | b':'
            | b'.'
            | b'/'
            | b'\\'
            | b'='
            | b'+'
            | b'-'
            | b'*'
            | b'<'
            | b'>'
            | b'&'
            | b'|'
            | b'!'
            | b'@'
            | b'#'
            | b'%'
            | b'^'
            | b'~'
            | b'`'
    )
}

fn prev_char_boundary(s: &str, pos: usize) -> usize {
    let mut i = pos.saturating_sub(1);
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, pos: usize) -> usize {
    let mut i = pos + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i.min(s.len())
}

fn safe_slice(s: &str, start: usize, end: usize) -> &str {
    let s_start = start.min(s.len());
    let s_end = end.min(s.len());
    &s[s_start..s_end]
}

fn safe_slice_from(s: &str, start: usize) -> &str {
    &s[start.min(s.len())..]
}

fn safe_slice_to(s: &str, end: usize) -> &str {
    &s[..end.min(s.len())]
}

pub const TEXT_LINE_HEIGHT: f32 = 20.0;
pub const TEXT_FONT_SIZE: f32 = 13.0;
pub const TEXT_PAD_Y: f32 = 8.0;
pub const TEXT_PAD_X: f32 = 12.0;
pub const GUTTER_PAD_RIGHT: f32 = 16.0;

pub const HEX_LINE_HEIGHT: f32 = 18.0;
pub const HEX_FONT_SIZE: f32 = 12.0;
pub const HEX_PAD_Y: f32 = 8.0;
pub const HEX_PAD_X: f32 = 12.0;

const TEXT_EXTENSIONS: &[&str] = &[
    "rs",
    "toml",
    "json",
    "md",
    "txt",
    "yml",
    "yaml",
    "js",
    "ts",
    "jsx",
    "tsx",
    "py",
    "sh",
    "bash",
    "zsh",
    "fish",
    "css",
    "scss",
    "less",
    "html",
    "htm",
    "xml",
    "svg",
    "log",
    "env",
    "cfg",
    "ini",
    "csv",
    "tsv",
    "sql",
    "graphql",
    "rb",
    "go",
    "java",
    "kt",
    "swift",
    "c",
    "h",
    "cpp",
    "hpp",
    "cc",
    "hh",
    "cs",
    "fs",
    "ml",
    "mli",
    "hs",
    "erl",
    "ex",
    "exs",
    "lua",
    "vim",
    "dockerfile",
    "makefile",
    "cmake",
    "gitignore",
    "gitattributes",
    "editorconfig",
    "prettierrc",
    "eslintrc",
    "babelrc",
    "lock",
    "snap",
    "patch",
    "diff",
    "conf",
    "rc",
    "properties",
    "r",
    "rmd",
    "jl",
    "pl",
    "pm",
    "php",
    "dart",
    "scala",
    "clj",
    "cljs",
    "tf",
    "hcl",
    "nix",
    "dhall",
    "zig",
    "v",
    "d",
    "nim",
    "cr",
    "pony",
];

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tiff", "tif",
];

fn detect_mode(path: &Path) -> EditorMode {
    if let Some(ext) = path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        if TEXT_EXTENSIONS.iter().any(|e| *e == ext_lower) {
            return EditorMode::Text;
        }
        if IMAGE_EXTENSIONS.iter().any(|e| *e == ext_lower) {
            return EditorMode::Image;
        }
    }

    if let Some(name) = path.file_name() {
        let name_lower = name.to_string_lossy().to_lowercase();
        let extensionless_text = [
            "makefile",
            "dockerfile",
            "vagrantfile",
            "gemfile",
            "rakefile",
            "procfile",
            "brewfile",
            "justfile",
            "cmakelists.txt",
            ".gitignore",
            ".gitattributes",
            ".editorconfig",
            ".env",
            ".dockerignore",
            ".prettierrc",
            ".eslintrc",
            ".babelrc",
        ];
        if extensionless_text.iter().any(|n| *n == name_lower) {
            return EditorMode::Text;
        }
    }

    if let Ok(bytes) = std::fs::read(path) {
        let sniff_len = bytes.len().min(8192);
        let sniff = &bytes[..sniff_len];
        if std::str::from_utf8(sniff).is_ok() && !sniff.contains(&0u8) {
            return EditorMode::Text;
        }
    }

    EditorMode::Hex
}

/// Build side-by-side diff rows from structured hunk data.
/// Pairs consecutive removed/added blocks into Modified rows for true
/// side-by-side presentation.
pub fn build_diff_rows(hunks: &[crate::git::DiffHunkData]) -> Vec<DiffRow> {
    let mut rows = Vec::new();

    for (i, hunk) in hunks.iter().enumerate() {
        if i > 0 {
            rows.push(DiffRow {
                left_num: None,
                left_text: None,
                right_num: None,
                right_text: None,
                kind: DiffRowKind::Separator,
            });
        }

        let mut old_num = hunk.old_start;
        let mut new_num = hunk.new_start;

        let mut li = 0;
        while li < hunk.lines.len() {
            match hunk.lines[li].origin {
                ' ' => {
                    rows.push(DiffRow {
                        left_num: Some(old_num),
                        left_text: Some(hunk.lines[li].content.clone()),
                        right_num: Some(new_num),
                        right_text: Some(hunk.lines[li].content.clone()),
                        kind: DiffRowKind::Context,
                    });
                    old_num += 1;
                    new_num += 1;
                    li += 1;
                }
                '-' => {
                    let mut removed = Vec::new();
                    while li < hunk.lines.len() && hunk.lines[li].origin == '-' {
                        removed.push(&hunk.lines[li]);
                        li += 1;
                    }
                    let mut added = Vec::new();
                    while li < hunk.lines.len() && hunk.lines[li].origin == '+' {
                        added.push(&hunk.lines[li]);
                        li += 1;
                    }

                    let max = removed.len().max(added.len());
                    for j in 0..max {
                        let has_left = j < removed.len();
                        let has_right = j < added.len();

                        let kind = match (has_left, has_right) {
                            (true, true) => DiffRowKind::Modified,
                            (true, false) => DiffRowKind::Removed,
                            (false, true) => DiffRowKind::Added,
                            (false, false) => unreachable!(),
                        };

                        let left_n = if has_left {
                            let n = old_num;
                            old_num += 1;
                            Some(n)
                        } else {
                            None
                        };
                        let right_n = if has_right {
                            let n = new_num;
                            new_num += 1;
                            Some(n)
                        } else {
                            None
                        };

                        rows.push(DiffRow {
                            left_num: left_n,
                            left_text: removed.get(j).map(|l| l.content.clone()),
                            right_num: right_n,
                            right_text: added.get(j).map(|l| l.content.clone()),
                            kind,
                        });
                    }
                }
                '+' => {
                    rows.push(DiffRow {
                        left_num: None,
                        left_text: None,
                        right_num: Some(new_num),
                        right_text: Some(hunk.lines[li].content.clone()),
                        kind: DiffRowKind::Added,
                    });
                    new_num += 1;
                    li += 1;
                }
                _ => {
                    li += 1;
                }
            }
        }
    }

    rows
}

#[cfg(test)]
impl EditorState {
    pub fn test_text(path: &str, lines: Vec<String>) -> Self {
        Self {
            path: PathBuf::from(path),
            mode: EditorMode::Text,
            lines,
            raw_bytes: Vec::new(),
            image_rgba: Vec::new(),
            image_width: 0,
            image_height: 0,
            scroll_offset: 0.0,
            scroll_x: 0.0,
            word_wrap: false,
            cursor_line: 0,
            cursor_col: 0,
            sel_anchor: None,
            modified: false,
            syntax_config_idx: None,
            highlight_cache: Vec::new(),
            highlight_dirty: false,
            diff_view: None,
            diff_split_frac: 0.5,
        }
    }

    pub fn test_hex(path: &str, bytes: Vec<u8>) -> Self {
        Self {
            path: PathBuf::from(path),
            mode: EditorMode::Hex,
            lines: Vec::new(),
            raw_bytes: bytes,
            image_rgba: Vec::new(),
            image_width: 0,
            image_height: 0,
            scroll_offset: 0.0,
            scroll_x: 0.0,
            word_wrap: false,
            cursor_line: 0,
            cursor_col: 0,
            sel_anchor: None,
            modified: false,
            syntax_config_idx: None,
            highlight_cache: Vec::new(),
            highlight_dirty: false,
            diff_view: None,
            diff_split_frac: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_state(lines: &[&str]) -> EditorState {
        EditorState {
            path: PathBuf::from("test.rs"),
            mode: EditorMode::Text,
            lines: lines.iter().map(|s| s.to_string()).collect(),
            raw_bytes: Vec::new(),
            image_rgba: Vec::new(),
            image_width: 0,
            image_height: 0,
            scroll_offset: 0.0,
            scroll_x: 0.0,
            word_wrap: false,
            cursor_line: 0,
            cursor_col: 0,
            sel_anchor: None,
            modified: false,
            syntax_config_idx: None,
            highlight_cache: Vec::new(),
            highlight_dirty: false,
            diff_view: None,
            diff_split_frac: 0.5,
        }
    }

    #[test]
    fn detect_rust_file() {
        assert_eq!(detect_mode(Path::new("main.rs")), EditorMode::Text);
    }

    #[test]
    fn detect_png_file() {
        assert_eq!(detect_mode(Path::new("icon.png")), EditorMode::Image);
    }

    #[test]
    fn detect_unknown_binary() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("blob");
        std::fs::write(&p, &[0xFF, 0xFE, 0x00, 0x01, 0x80, 0x90]).unwrap();
        assert_eq!(detect_mode(&p), EditorMode::Hex);
    }

    #[test]
    fn detect_extensionless_text() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("readme");
        std::fs::write(&p, "hello world\n").unwrap();
        assert_eq!(detect_mode(&p), EditorMode::Text);
    }

    #[test]
    fn open_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("test.rs");
        std::fs::write(&p, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        let state = EditorState::open(&p, None).unwrap();
        assert_eq!(state.mode, EditorMode::Text);
        assert_eq!(state.lines.len(), 3);
    }

    #[test]
    fn open_hex_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("data.bin");
        std::fs::write(&p, &[0u8; 64]).unwrap();
        let state = EditorState::open(&p, None).unwrap();
        assert_eq!(state.mode, EditorMode::Hex);
        assert_eq!(state.raw_bytes.len(), 64);
    }

    #[test]
    fn insert_char_and_modified() {
        let mut s = text_state(&["hello"]);
        s.set_cursor_pos(0, 5);
        s.insert_char('!');
        assert_eq!(s.lines[0], "hello!");
        assert!(s.is_modified());
        assert_eq!(s.cursor_col(), 6);
    }

    #[test]
    fn new_line_splits() {
        let mut s = text_state(&["helloworld"]);
        s.set_cursor_pos(0, 5);
        s.new_line();
        assert_eq!(s.lines, vec!["hello", "world"]);
        assert_eq!(s.cursor_line(), 1);
        assert_eq!(s.cursor_col(), 0);
    }

    #[test]
    fn backspace_joins_lines() {
        let mut s = text_state(&["hello", "world"]);
        s.set_cursor_pos(1, 0);
        s.delete_backward();
        assert_eq!(s.lines, vec!["helloworld"]);
        assert_eq!(s.cursor_col(), 5);
    }

    #[test]
    fn delete_forward_joins_lines() {
        let mut s = text_state(&["hello", "world"]);
        s.set_cursor_pos(0, 5);
        s.delete_forward();
        assert_eq!(s.lines, vec!["helloworld"]);
    }

    #[test]
    fn select_and_delete() {
        let mut s = text_state(&["hello world"]);
        s.set_cursor_pos(0, 0);
        s.set_cursor_pos_selecting(0, 5);
        assert!(s.has_selection());
        assert_eq!(s.selected_text(), Some("hello".to_string()));
        s.delete_selection();
        assert_eq!(s.lines[0], " world");
    }

    #[test]
    fn select_all() {
        let mut s = text_state(&["abc", "def"]);
        s.select_all();
        assert_eq!(s.selected_text(), Some("abc\ndef".to_string()));
    }

    #[test]
    fn cursor_move_wraps() {
        let mut s = text_state(&["ab", "cd"]);
        s.set_cursor_pos(0, 2);
        s.move_cursor(CursorMove::Right, false);
        assert_eq!(s.cursor_line(), 1);
        assert_eq!(s.cursor_col(), 0);
        s.move_cursor(CursorMove::Left, false);
        assert_eq!(s.cursor_line(), 0);
        assert_eq!(s.cursor_col(), 2);
    }

    #[test]
    fn file_name_shows_modified() {
        let mut s = text_state(&["test"]);
        assert_eq!(s.file_name(), "test.rs");
        s.insert_char('x');
        assert!(s.file_name().starts_with("● "));
    }

    #[test]
    fn save_clears_modified() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("save_test.rs");
        std::fs::write(&p, "original").unwrap();
        let mut s = EditorState::open(&p, None).unwrap();
        s.insert_char('!');
        assert!(s.is_modified());
        s.save().unwrap();
        assert!(!s.is_modified());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "!original");
    }

    #[test]
    fn insert_str_with_newlines() {
        let mut s = text_state(&[""]);
        s.insert_str("line1\nline2\nline3");
        assert_eq!(s.lines, vec!["line1", "line2", "line3"]);
        assert_eq!(s.cursor_line(), 2);
        assert_eq!(s.cursor_col(), 5);
    }

    #[test]
    fn word_boundary_left_basic() {
        let lines = vec!["hello world".to_string()];
        assert_eq!(word_boundary_left(&lines, 0, 11), (0, 6));
        assert_eq!(word_boundary_left(&lines, 0, 6), (0, 0));
        assert_eq!(word_boundary_left(&lines, 0, 0), (0, 0));
    }

    #[test]
    fn word_boundary_right_basic() {
        let lines = vec!["hello world".to_string()];
        assert_eq!(word_boundary_right(&lines, 0, 0), (0, 6));
        assert_eq!(word_boundary_right(&lines, 0, 6), (0, 11));
    }

    #[test]
    fn word_boundary_left_wraps_to_prev_line() {
        let lines = vec!["first".to_string(), "second".to_string()];
        assert_eq!(word_boundary_left(&lines, 1, 0), (0, 5));
    }

    #[test]
    fn word_boundary_right_wraps_to_next_line() {
        let lines = vec!["first".to_string(), "second".to_string()];
        assert_eq!(word_boundary_right(&lines, 0, 5), (1, 0));
    }

    #[test]
    fn move_cursor_word_left() {
        let mut s = text_state(&["hello world"]);
        s.cursor_col = 11;
        s.move_cursor(CursorMove::WordLeft, false);
        assert_eq!(s.cursor_col(), 6);
        s.move_cursor(CursorMove::WordLeft, false);
        assert_eq!(s.cursor_col(), 0);
    }

    #[test]
    fn move_cursor_word_right() {
        let mut s = text_state(&["hello world"]);
        s.move_cursor(CursorMove::WordRight, false);
        assert_eq!(s.cursor_col(), 6);
        s.move_cursor(CursorMove::WordRight, false);
        assert_eq!(s.cursor_col(), 11);
    }

    #[test]
    fn move_cursor_document_start() {
        let mut s = text_state(&["line1", "line2", "line3"]);
        s.cursor_line = 2;
        s.cursor_col = 3;
        s.move_cursor(CursorMove::DocumentStart, false);
        assert_eq!(s.cursor_line(), 0);
        assert_eq!(s.cursor_col(), 0);
    }

    #[test]
    fn move_cursor_document_end() {
        let mut s = text_state(&["line1", "line2", "line3"]);
        s.move_cursor(CursorMove::DocumentEnd, false);
        assert_eq!(s.cursor_line(), 2);
        assert_eq!(s.cursor_col(), 5);
    }

    #[test]
    fn word_move_with_selection() {
        let mut s = text_state(&["hello world"]);
        s.move_cursor(CursorMove::WordRight, true);
        assert_eq!(s.sel_anchor, Some((0, 0)));
        assert_eq!(s.cursor_col(), 6);
    }

    #[test]
    fn delete_word_backward_basic() {
        let mut s = text_state(&["hello world"]);
        s.cursor_col = 11;
        s.delete_word_backward();
        assert_eq!(s.lines[0], "hello ");
        assert_eq!(s.cursor_col(), 6);
    }

    #[test]
    fn delete_word_forward_basic() {
        let mut s = text_state(&["hello world"]);
        s.delete_word_forward();
        assert_eq!(s.lines[0], "world");
        assert_eq!(s.cursor_col(), 0);
    }

    #[test]
    fn delete_to_line_start() {
        let mut s = text_state(&["hello world"]);
        s.cursor_col = 5;
        s.delete_to_line_start();
        assert_eq!(s.lines[0], " world");
        assert_eq!(s.cursor_col(), 0);
    }

    #[test]
    fn build_diff_rows_context_only() {
        use crate::git::{DiffHunkData, DiffLineDatum};
        let hunks = vec![DiffHunkData {
            old_start: 1,
            new_start: 1,
            lines: vec![
                DiffLineDatum {
                    origin: ' ',
                    content: "same".into(),
                },
                DiffLineDatum {
                    origin: ' ',
                    content: "also same".into(),
                },
            ],
        }];
        let rows = build_diff_rows(&hunks);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, DiffRowKind::Context);
        assert_eq!(rows[0].left_num, Some(1));
        assert_eq!(rows[0].right_num, Some(1));
    }

    #[test]
    fn build_diff_rows_added_removed() {
        use crate::git::{DiffHunkData, DiffLineDatum};
        let hunks = vec![DiffHunkData {
            old_start: 1,
            new_start: 1,
            lines: vec![
                DiffLineDatum {
                    origin: ' ',
                    content: "ctx".into(),
                },
                DiffLineDatum {
                    origin: '-',
                    content: "old".into(),
                },
                DiffLineDatum {
                    origin: '+',
                    content: "new".into(),
                },
                DiffLineDatum {
                    origin: ' ',
                    content: "ctx2".into(),
                },
            ],
        }];
        let rows = build_diff_rows(&hunks);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].kind, DiffRowKind::Context);
        assert_eq!(rows[1].kind, DiffRowKind::Modified);
        assert_eq!(rows[1].left_text.as_deref(), Some("old"));
        assert_eq!(rows[1].right_text.as_deref(), Some("new"));
        assert_eq!(rows[2].kind, DiffRowKind::Context);
    }

    #[test]
    fn build_diff_rows_unbalanced() {
        use crate::git::{DiffHunkData, DiffLineDatum};
        let hunks = vec![DiffHunkData {
            old_start: 5,
            new_start: 5,
            lines: vec![
                DiffLineDatum {
                    origin: '-',
                    content: "a".into(),
                },
                DiffLineDatum {
                    origin: '-',
                    content: "b".into(),
                },
                DiffLineDatum {
                    origin: '+',
                    content: "x".into(),
                },
            ],
        }];
        let rows = build_diff_rows(&hunks);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, DiffRowKind::Modified);
        assert_eq!(rows[0].left_num, Some(5));
        assert_eq!(rows[0].right_num, Some(5));
        assert_eq!(rows[1].kind, DiffRowKind::Removed);
        assert_eq!(rows[1].left_num, Some(6));
        assert!(rows[1].right_num.is_none());
    }

    #[test]
    fn build_diff_rows_separator_between_hunks() {
        use crate::git::{DiffHunkData, DiffLineDatum};
        let hunks = vec![
            DiffHunkData {
                old_start: 1,
                new_start: 1,
                lines: vec![DiffLineDatum {
                    origin: ' ',
                    content: "a".into(),
                }],
            },
            DiffHunkData {
                old_start: 50,
                new_start: 50,
                lines: vec![DiffLineDatum {
                    origin: ' ',
                    content: "b".into(),
                }],
            },
        ];
        let rows = build_diff_rows(&hunks);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].kind, DiffRowKind::Separator);
    }
}
