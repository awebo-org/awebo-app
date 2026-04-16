use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cosmic_text::{Family, FontSystem, Metrics, SwashCache};
use regex::{Regex, RegexBuilder};

use crate::renderer::icons::{Icon, IconRenderer, icon_for_filename};
use crate::renderer::pixel_buffer::PixelBuffer;
use crate::renderer::text::{
    cursor_byte_from_x, draw_text_at, draw_text_clipped, measure_text_width,
};
use crate::renderer::theme;

const INPUT_HEIGHT: f32 = 30.0;
const INPUT_PAD_X: f32 = 8.0;
const INPUT_PAD_Y: f32 = 6.0;
const INPUT_FONT_SIZE: f32 = 12.0;
const INPUT_LINE_H: f32 = 17.0;

const TOGGLE_W: f32 = 22.0;
const TOGGLE_H: f32 = 20.0;
const TOGGLE_GAP: f32 = 2.0;
const TOGGLE_RIGHT_PAD: f32 = 4.0;
const TOGGLE_FONT: f32 = 11.0;
const TOGGLE_COUNT: usize = 3;

const REGEX_ERROR_BORDER: crate::renderer::pixel_buffer::Rgb = (180, 70, 70);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchToggle {
    Case,
    Word,
    Regex,
}

const RESULT_FILE_H: f32 = 24.0;
const RESULT_LINE_H: f32 = 22.0;
const RESULT_PAD_X: f32 = 10.0;
const RESULT_FONT_SIZE: f32 = 12.0;
const RESULT_LINE_FONT: f32 = 11.0;
const RESULT_GAP: f32 = 4.0;
const RESULTS_PAD_Y: f32 = 6.0;

const MAX_RESULTS: usize = 500;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".build",
    "__pycache__",
    ".next",
    "dist",
    "build",
    ".cache",
    "vendor",
];

#[derive(Debug, Clone)]
pub struct SearchFileGroup {
    pub path: PathBuf,
    pub rel_path: String,
    pub file_name: String,
    pub dir_part: String,
    pub matches: Vec<SearchLineMatch>,
}

#[derive(Debug, Clone)]
pub struct SearchLineMatch {
    pub line_num: u32,
    pub num_str: String,
    pub snippet_trimmed: String,
}

pub struct SearchPanelState {
    pub query: String,
    pub cursor: usize,
    pub selection_anchor: Option<usize>,
    pub text_scroll_x: f32,
    pub input_mouse_dragging: bool,
    pub results: Vec<SearchFileGroup>,
    pub scroll_offset: f32,
    pub hovered_idx: Option<usize>,
    pub focused: bool,
    pub searching: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub regex_error: bool,
    pub hovered_toggle: Option<SearchToggle>,
    generation: u64,
    receiver: Option<mpsc::Receiver<SearchResult>>,
    debounce_at: Option<std::time::Instant>,
    root: PathBuf,
    pub scrollbar_hovered: bool,
    pub scrollbar_dragging: bool,
}

struct SearchResult {
    generation: u64,
    groups: Vec<SearchFileGroup>,
}

impl Default for SearchPanelState {
    fn default() -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            selection_anchor: None,
            text_scroll_x: 0.0,
            input_mouse_dragging: false,
            results: Vec::new(),
            scroll_offset: 0.0,
            hovered_idx: None,
            focused: false,
            searching: false,
            case_sensitive: false,
            whole_word: false,
            regex: false,
            regex_error: false,
            hovered_toggle: None,
            generation: 0,
            receiver: None,
            debounce_at: None,
            root: PathBuf::new(),
            scrollbar_hovered: false,
            scrollbar_dragging: false,
        }
    }
}

impl SearchPanelState {
    pub fn set_root(&mut self, root: &Path) {
        if self.root != root {
            self.root = root.to_path_buf();
            if !self.query.is_empty() {
                self.trigger_search();
            }
        }
    }

    pub fn selected_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.query.len();
    }

    pub fn text_area_x(sf: f32) -> usize {
        let input_pad_x = (INPUT_PAD_X * sf) as usize;
        let icon_sz = (14.0 * sf).round() as usize;
        let icon_x = input_pad_x + (6.0 * sf) as usize;
        icon_x + icon_sz + (6.0 * sf) as usize
    }

    pub fn text_area_clip_right(sf: f32, panel_w: usize) -> usize {
        let input_pad_x = (INPUT_PAD_X * sf) as usize;
        let input_w = panel_w.saturating_sub(input_pad_x * 2);
        let toggles_w = Self::toggles_reserved_width(sf);
        let input_right = input_pad_x + input_w;
        input_right.saturating_sub(toggles_w + (4.0 * sf) as usize)
    }

    fn toggles_reserved_width(sf: f32) -> usize {
        let tw = (TOGGLE_W * sf) as usize;
        let gap = (TOGGLE_GAP * sf) as usize;
        let pad = (TOGGLE_RIGHT_PAD * sf) as usize;
        TOGGLE_COUNT * tw + (TOGGLE_COUNT - 1) * gap + pad
    }

    /// Compute the (x, y, w, h) rect of a toggle inside the input row.
    pub fn toggle_rect(
        tog: SearchToggle,
        sf: f32,
        panel_w: usize,
        content_y: usize,
    ) -> (usize, usize, usize, usize) {
        let input_pad_x = (INPUT_PAD_X * sf) as usize;
        let input_pad_y = (INPUT_PAD_Y * sf) as usize;
        let input_h = (INPUT_HEIGHT * sf) as usize;
        let input_w = panel_w.saturating_sub(input_pad_x * 2);
        let input_right = input_pad_x + input_w;

        let tw = (TOGGLE_W * sf) as usize;
        let th = (TOGGLE_H * sf) as usize;
        let gap = (TOGGLE_GAP * sf) as usize;
        let right_pad = (TOGGLE_RIGHT_PAD * sf) as usize;

        let regex_x = input_right.saturating_sub(right_pad + tw);
        let word_x = regex_x.saturating_sub(gap + tw);
        let case_x = word_x.saturating_sub(gap + tw);

        let input_y = content_y + input_pad_y;
        let y = input_y + (input_h.saturating_sub(th)) / 2;

        let x = match tog {
            SearchToggle::Case => case_x,
            SearchToggle::Word => word_x,
            SearchToggle::Regex => regex_x,
        };
        (x, y, tw, th)
    }

    pub fn toggle_hit(
        phys_x: f64,
        phys_y: f64,
        sf: f32,
        panel_w: usize,
        content_y: usize,
    ) -> Option<SearchToggle> {
        for t in [SearchToggle::Case, SearchToggle::Word, SearchToggle::Regex] {
            let (x, y, w, h) = Self::toggle_rect(t, sf, panel_w, content_y);
            if phys_x >= x as f64
                && phys_x < (x + w) as f64
                && phys_y >= y as f64
                && phys_y < (y + h) as f64
            {
                return Some(t);
            }
        }
        None
    }

    pub fn toggle_flag(&mut self, tog: SearchToggle) {
        match tog {
            SearchToggle::Case => self.case_sensitive = !self.case_sensitive,
            SearchToggle::Word => self.whole_word = !self.whole_word,
            SearchToggle::Regex => {
                self.regex = !self.regex;
                if !self.regex {
                    self.regex_error = false;
                }
            }
        }
        self.schedule_search();
    }

    pub fn ensure_cursor_visible(&mut self, font_system: &mut FontSystem, sf: f32, panel_w: usize) {
        let text_x = Self::text_area_x(sf);
        let clip_right = Self::text_area_clip_right(sf, panel_w);
        let visible_w = clip_right.saturating_sub(text_x) as f32;
        if visible_w <= 0.0 {
            return;
        }

        let metrics = Metrics::new(INPUT_FONT_SIZE * sf, INPUT_LINE_H * sf);
        let cursor_px = measure_text_width(
            font_system,
            &self.query[..self.cursor],
            metrics,
            Family::SansSerif,
        );

        if cursor_px - self.text_scroll_x > visible_w {
            self.text_scroll_x = cursor_px - visible_w + 2.0;
        }
        if cursor_px < self.text_scroll_x {
            self.text_scroll_x = (cursor_px - 2.0).max(0.0);
        }

        let full_w = measure_text_width(font_system, &self.query, metrics, Family::SansSerif);
        if full_w <= visible_w {
            self.text_scroll_x = 0.0;
        }
    }

    pub fn click_to_cursor(&self, phys_x: f64, font_system: &mut FontSystem, sf: f32) -> usize {
        let text_x = Self::text_area_x(sf) as f64;
        let local_x = (phys_x - text_x + self.text_scroll_x as f64).max(0.0) as f32;
        let metrics = Metrics::new(INPUT_FONT_SIZE * sf, INPUT_LINE_H * sf);
        cursor_byte_from_x(
            font_system,
            &self.query,
            metrics,
            Family::SansSerif,
            local_x,
        )
    }

    pub fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selected_range() {
            self.query.drain(start..end);
            self.cursor = start;
            self.selection_anchor = None;
            self.schedule_search();
            true
        } else {
            false
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        self.delete_selection();
        self.query.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.selection_anchor = None;
        self.schedule_search();
    }

    pub fn insert_text(&mut self, s: &str) {
        self.delete_selection();
        self.query.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.selection_anchor = None;
        self.schedule_search();
    }

    pub fn delete_back(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor > 0 {
            let prev = self.query[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.drain(prev..self.cursor);
            self.cursor = prev;
            self.schedule_search();
        }
    }

    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor < self.query.len() {
            let next = self.query[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.query.len());
            self.query.drain(self.cursor..next);
            self.schedule_search();
        }
    }

    pub fn move_left(&mut self) {
        self.selection_anchor = None;
        if self.cursor > 0 {
            self.cursor = self.query[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_right(&mut self) {
        self.selection_anchor = None;
        if self.cursor < self.query.len() {
            self.cursor = self.query[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.query.len());
        }
    }

    pub fn move_home(&mut self) {
        self.selection_anchor = None;
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.selection_anchor = None;
        self.cursor = self.query.len();
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.cursor = 0;
        self.selection_anchor = None;
        self.text_scroll_x = 0.0;
        self.results.clear();
        self.scroll_offset = 0.0;
        self.searching = false;
        self.debounce_at = None;
    }

    fn schedule_search(&mut self) {
        self.debounce_at = Some(std::time::Instant::now() + std::time::Duration::from_millis(150));
    }

    pub fn has_pending_debounce(&self) -> bool {
        self.debounce_at.is_some()
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;

        if let Some(at) = self.debounce_at
            && std::time::Instant::now() >= at
        {
            self.debounce_at = None;
            self.trigger_search();
            changed = true;
        }

        if let Some(rx) = &self.receiver {
            while let Ok(result) = rx.try_recv() {
                if result.generation == self.generation {
                    self.results = result.groups;
                    self.searching = false;
                    self.scroll_offset = 0.0;
                    changed = true;
                }
            }
        }

        changed
    }

    fn trigger_search(&mut self) {
        self.generation += 1;
        let search_gen = self.generation;
        let query = self.query.clone();
        let root = self.root.clone();

        if query.is_empty() {
            self.results.clear();
            self.searching = false;
            self.regex_error = false;
            return;
        }

        let re = match build_search_regex(&query, self.case_sensitive, self.whole_word, self.regex)
        {
            Ok(r) => {
                self.regex_error = false;
                r
            }
            Err(_) => {
                self.regex_error = true;
                self.results.clear();
                self.searching = false;
                return;
            }
        };

        self.searching = true;
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);

        std::thread::spawn(move || {
            let groups = run_search(&root, &re, search_gen);
            let _ = tx.send(SearchResult {
                generation: search_gen,
                groups,
            });
        });
    }

    pub fn flat_row_count(&self) -> usize {
        let mut count = 0;
        for g in &self.results {
            count += 1;
            count += g.matches.len();
        }
        count
    }

    pub fn total_height(&self, sf: f32) -> usize {
        let input_area = INPUT_PAD_Y * sf + INPUT_HEIGHT * sf + INPUT_PAD_Y * sf;
        let mut h = input_area;
        for g in &self.results {
            h += RESULT_FILE_H * sf;
            h += g.matches.len() as f32 * RESULT_LINE_H * sf;
            h += RESULT_GAP * sf;
        }
        h += RESULTS_PAD_Y * sf * 2.0;
        h.ceil() as usize
    }

    pub fn path_at_flat_index(&self, idx: usize) -> Option<(PathBuf, Option<u32>)> {
        let mut i = 0;
        for g in &self.results {
            if i == idx {
                return Some((g.path.clone(), None));
            }
            i += 1;
            for m in &g.matches {
                if i == idx {
                    return Some((g.path.clone(), Some(m.line_num)));
                }
                i += 1;
            }
        }
        None
    }
}

/// Build the regex used for searching. Returns Err when the user-supplied
/// regex pattern fails to compile (only possible when `use_regex` is true).
fn build_search_regex(
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
) -> Result<Regex, regex::Error> {
    let base = if use_regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    let pattern = if whole_word {
        format!(r"\b(?:{base})\b")
    } else {
        base
    };
    let mut builder = RegexBuilder::new(&pattern);
    builder.case_insensitive(!case_sensitive);
    builder.build()
}

fn run_search(root: &Path, re: &Regex, _gen: u64) -> Vec<SearchFileGroup> {
    let mut groups: Vec<SearchFileGroup> = Vec::new();
    let mut total_matches = 0usize;

    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if total_matches >= MAX_RESULTS {
            break;
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut children: Vec<std::fs::DirEntry> = entries.filter_map(|e| e.ok()).collect();
        children.sort_by_key(|e| e.file_name());

        for entry in children {
            if total_matches >= MAX_RESULTS {
                break;
            }

            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if ft.is_dir() {
                if SKIP_DIRS.contains(&name_str.as_ref()) {
                    continue;
                }
                stack.push(entry.path());
                continue;
            }

            if !ft.is_file() {
                continue;
            }

            let name_match = re.is_match(&name_str);

            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path().as_path())
                .to_string_lossy()
                .to_string();

            let content_matches = search_file_contents(&entry.path(), re);

            if !name_match && content_matches.is_empty() {
                continue;
            }

            total_matches += 1 + content_matches.len();

            let file_name = entry
                .path()
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let dir_part = rel
                .strip_suffix(&file_name)
                .unwrap_or("")
                .trim_end_matches('/')
                .to_string();

            groups.push(SearchFileGroup {
                path: entry.path(),
                rel_path: rel,
                file_name,
                dir_part,
                matches: content_matches,
            });
        }
    }

    groups
}

fn search_file_contents(path: &Path, re: &Regex) -> Vec<SearchLineMatch> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };

    if meta.len() > 1_024_000 {
        return Vec::new();
    }

    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    if data.iter().take(8192).any(|&b| b == 0) {
        return Vec::new();
    }

    let text = match std::str::from_utf8(&data) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut matches = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        if re.is_match(line) {
            let snippet = if line.len() > 120 {
                format!("{}…", &line[..120])
            } else {
                line.to_string()
            };
            let line_num = (line_idx + 1) as u32;
            matches.push(SearchLineMatch {
                line_num,
                num_str: line_num.to_string(),
                snippet_trimmed: snippet.trim().to_string(),
            });
            if matches.len() >= 10 {
                break;
            }
        }
    }

    matches
}

fn truncate_chars(text: &str, max_w: f32, font_size_px: f32) -> String {
    let avg_char_w = font_size_px * 0.58;
    let max_chars = (max_w / avg_char_w).floor() as usize;
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let take = max_chars.saturating_sub(1);
    let truncated: String = text.chars().take(take).collect();
    format!("{truncated}…")
}

pub fn draw(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    icon_renderer: &mut IconRenderer,
    state: &SearchPanelState,
    panel_w: usize,
    content_y: usize,
    sf: f32,
    cursor_visible: bool,
) {
    let input_h = (INPUT_HEIGHT * sf) as usize;
    let input_pad_x = (INPUT_PAD_X * sf) as usize;
    let input_pad_y = (INPUT_PAD_Y * sf) as usize;
    let input_y = content_y + input_pad_y;

    let input_w = panel_w.saturating_sub(input_pad_x * 2);
    let input_x = input_pad_x;

    let input_bg: crate::renderer::pixel_buffer::Rgb = (22, 22, 26);
    let input_border: crate::renderer::pixel_buffer::Rgb = if state.regex && state.regex_error {
        REGEX_ERROR_BORDER
    } else if state.focused {
        theme::PRIMARY
    } else {
        (50, 50, 56)
    };

    buf.fill_rect(input_x, input_y, input_w, input_h, input_bg);

    buf.fill_rect(input_x, input_y, input_w, 1, input_border);
    buf.fill_rect(
        input_x,
        input_y + input_h.saturating_sub(1),
        input_w,
        1,
        input_border,
    );
    buf.fill_rect(input_x, input_y, 1, input_h, input_border);
    buf.fill_rect(
        input_x + input_w.saturating_sub(1),
        input_y,
        1,
        input_h,
        input_border,
    );

    let icon_sz = (14.0 * sf).round() as u32;
    let icon_x = input_x + (6.0 * sf) as usize;
    let icon_y = input_y + ((input_h as f32 - icon_sz as f32) / 2.0) as usize;
    icon_renderer.draw(buf, Icon::Search, icon_x, icon_y, icon_sz, theme::FG_DIM);

    let text_x = icon_x + icon_sz as usize + (6.0 * sf) as usize;
    let text_y = input_y + ((input_h as f32 - INPUT_LINE_H * sf) / 2.0) as usize;
    let text_metrics = Metrics::new(INPUT_FONT_SIZE * sf, INPUT_LINE_H * sf);
    let clip_right = SearchPanelState::text_area_clip_right(sf, panel_w);
    let scroll_px = state.text_scroll_x as usize;
    let draw_text_x = text_x.saturating_sub(scroll_px);

    if state.query.is_empty() {
        draw_text_clipped(
            buf,
            font_system,
            swash_cache,
            text_x,
            text_y,
            buf.height,
            clip_right,
            0,
            "Search files and content…",
            text_metrics,
            theme::FG_DIM,
            Family::SansSerif,
        );
    } else {
        if let Some((sel_start, sel_end)) = state.selected_range() {
            let sel_x_start = draw_text_x as f32
                + measure_text_width(
                    font_system,
                    &state.query[..sel_start],
                    text_metrics,
                    Family::SansSerif,
                );
            let sel_x_end = draw_text_x as f32
                + measure_text_width(
                    font_system,
                    &state.query[..sel_end],
                    text_metrics,
                    Family::SansSerif,
                );
            let sel_x0 = (sel_x_start as usize).max(text_x).min(clip_right);
            let sel_x1 = (sel_x_end as usize).min(clip_right);
            if sel_x1 > sel_x0 {
                let sel_h = (INPUT_LINE_H * sf) as usize;
                buf.fill_rect(sel_x0, text_y, sel_x1 - sel_x0, sel_h, theme::PRIMARY_DIM);
            }
        }

        draw_text_clipped(
            buf,
            font_system,
            swash_cache,
            draw_text_x,
            text_y,
            buf.height,
            clip_right,
            text_x,
            &state.query,
            text_metrics,
            theme::FG_PRIMARY,
            Family::SansSerif,
        );
    }

    if state.focused && cursor_visible && state.selected_range().is_none() {
        let before = &state.query[..state.cursor];
        let cursor_px = draw_text_x as f32
            + measure_text_width(font_system, before, text_metrics, Family::SansSerif);
        let cursor_x = cursor_px as usize;
        let cursor_h = (INPUT_LINE_H * sf) as usize;
        if cursor_x >= text_x && cursor_x < clip_right {
            buf.fill_rect(cursor_x, text_y, 1, cursor_h, theme::FG_PRIMARY);
        }
    }

    draw_search_toggles(buf, font_system, swash_cache, state, panel_w, content_y, sf);

    let results_y = input_y + input_h + input_pad_y;
    let results_h = buf.height.saturating_sub(results_y);

    if state.searching && state.results.is_empty() {
        let msg = "Searching…";
        let msg_metrics = Metrics::new(RESULT_FONT_SIZE * sf, (RESULT_FONT_SIZE + 5.0) * sf);
        let msg_y = results_y + (12.0 * sf) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            (RESULT_PAD_X * sf) as usize,
            msg_y,
            panel_w,
            msg,
            msg_metrics,
            theme::FG_DIM,
            Family::SansSerif,
        );
        return;
    }

    if state.query.is_empty() {
        return;
    }

    if !state.searching && state.results.is_empty() {
        let msg = "No results found";
        let msg_metrics = Metrics::new(RESULT_FONT_SIZE * sf, (RESULT_FONT_SIZE + 5.0) * sf);
        let msg_y = results_y + (12.0 * sf) as usize;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            (RESULT_PAD_X * sf) as usize,
            msg_y,
            panel_w,
            msg,
            msg_metrics,
            theme::FG_DIM,
            Family::SansSerif,
        );
        return;
    }

    let scroll = state.scroll_offset.max(0.0) as usize;
    let file_h = (RESULT_FILE_H * sf) as usize;
    let line_h = (RESULT_LINE_H * sf) as usize;
    let gap = (RESULT_GAP * sf) as usize;
    let pad_x = (RESULT_PAD_X * sf) as usize;
    let pad_y_top = (RESULTS_PAD_Y * sf) as usize;
    let clip_bottom = results_y + results_h;

    let file_font_px = RESULT_FONT_SIZE * sf;
    let line_font_px = RESULT_LINE_FONT * sf;
    let file_metrics = Metrics::new(file_font_px, (RESULT_FONT_SIZE + 5.0) * sf);
    let line_metrics = Metrics::new(line_font_px, (RESULT_LINE_FONT + 5.0) * sf);
    let num_metrics = line_metrics;

    let mut flat_idx = 0usize;
    let mut y_acc = pad_y_top;

    for group in &state.results {
        if y_acc + file_h <= scroll {
            y_acc += file_h;
            flat_idx += 1;
            for _ in &group.matches {
                y_acc += line_h;
                flat_idx += 1;
            }
            y_acc += gap;
            continue;
        }

        let row_top = results_y + y_acc - scroll.min(y_acc);

        if row_top >= clip_bottom {
            break;
        }

        let hovered = state.hovered_idx == Some(flat_idx);
        let draw_h = file_h.min(clip_bottom.saturating_sub(row_top));
        if hovered {
            buf.fill_rect(0, row_top, panel_w, draw_h, theme::BG_ELEVATED);
        }

        let icon = icon_for_filename(&group.rel_path);
        let icon_sz = (14.0 * sf).round() as u32;
        let icon_y_off = row_top + ((file_h as f32 - icon_sz as f32) / 2.0) as usize;
        if icon_y_off + icon_sz as usize <= clip_bottom {
            icon_renderer.draw(buf, icon, pad_x, icon_y_off, icon_sz, theme::FG_PRIMARY);
        }

        let name_x = pad_x + icon_sz as usize + (6.0 * sf) as usize;
        let name_y = row_top + ((file_h as f32 - (RESULT_FONT_SIZE + 5.0) * sf) / 2.0) as usize;
        let avail_w = panel_w.saturating_sub(name_x + pad_x) as f32;

        if group.dir_part.is_empty() {
            let display = truncate_chars(&group.file_name, avail_w, file_font_px);
            draw_text_clipped(
                buf,
                font_system,
                swash_cache,
                name_x,
                name_y,
                clip_bottom,
                panel_w,
                0,
                &display,
                file_metrics,
                theme::FG_BRIGHT,
                Family::SansSerif,
            );
        } else {
            let name_est_w = group.file_name.len() as f32 * file_font_px * 0.58;
            let gap_px = 6.0 * sf;
            let dir_avail = avail_w - name_est_w - gap_px;
            if dir_avail > file_font_px * 2.0 {
                draw_text_clipped(
                    buf,
                    font_system,
                    swash_cache,
                    name_x,
                    name_y,
                    clip_bottom,
                    panel_w,
                    0,
                    &group.file_name,
                    file_metrics,
                    theme::FG_BRIGHT,
                    Family::SansSerif,
                );
                let dir_x = name_x + name_est_w as usize + gap_px as usize;
                let dir_display = truncate_chars(&group.dir_part, dir_avail, file_font_px);
                draw_text_clipped(
                    buf,
                    font_system,
                    swash_cache,
                    dir_x,
                    name_y,
                    clip_bottom,
                    panel_w,
                    0,
                    &dir_display,
                    file_metrics,
                    theme::FG_DIM,
                    Family::SansSerif,
                );
            } else {
                let display = truncate_chars(&group.file_name, avail_w, file_font_px);
                draw_text_clipped(
                    buf,
                    font_system,
                    swash_cache,
                    name_x,
                    name_y,
                    clip_bottom,
                    panel_w,
                    0,
                    &display,
                    file_metrics,
                    theme::FG_BRIGHT,
                    Family::SansSerif,
                );
            }
        }

        y_acc += file_h;
        flat_idx += 1;

        for m in &group.matches {
            if y_acc + line_h <= scroll {
                y_acc += line_h;
                flat_idx += 1;
                continue;
            }

            let row_top = results_y + y_acc - scroll.min(y_acc);

            if row_top >= clip_bottom {
                break;
            }

            let hovered = state.hovered_idx == Some(flat_idx);
            let draw_h = line_h.min(clip_bottom.saturating_sub(row_top));
            if hovered {
                buf.fill_rect(0, row_top, panel_w, draw_h, theme::BG_ELEVATED);
            }

            let indent = pad_x + (16.0 * sf) as usize;
            let num_y = row_top + ((line_h as f32 - (RESULT_LINE_FONT + 5.0) * sf) / 2.0) as usize;
            draw_text_clipped(
                buf,
                font_system,
                swash_cache,
                indent,
                num_y,
                clip_bottom,
                panel_w,
                0,
                &m.num_str,
                num_metrics,
                theme::FG_DIM,
                Family::SansSerif,
            );

            let num_est_w = (m.num_str.len() as f32 * line_font_px * 0.62) as usize;
            let snippet_x = indent + num_est_w + (8.0 * sf) as usize;
            let snippet_avail = panel_w.saturating_sub(snippet_x + pad_x) as f32;
            let snippet_display = truncate_chars(&m.snippet_trimmed, snippet_avail, line_font_px);
            draw_text_clipped(
                buf,
                font_system,
                swash_cache,
                snippet_x,
                num_y,
                clip_bottom,
                panel_w,
                0,
                &snippet_display,
                line_metrics,
                theme::FG_PRIMARY,
                Family::SansSerif,
            );

            y_acc += line_h;
            flat_idx += 1;
        }

        y_acc += gap;
    }
}

fn draw_search_toggles(
    buf: &mut PixelBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    state: &SearchPanelState,
    panel_w: usize,
    content_y: usize,
    sf: f32,
) {
    for tog in [SearchToggle::Case, SearchToggle::Word, SearchToggle::Regex] {
        let (x, y, w, h) = SearchPanelState::toggle_rect(tog, sf, panel_w, content_y);
        let active = match tog {
            SearchToggle::Case => state.case_sensitive,
            SearchToggle::Word => state.whole_word,
            SearchToggle::Regex => state.regex,
        };
        let hovered = state.hovered_toggle == Some(tog);
        let label = match tog {
            SearchToggle::Case => "Aa",
            SearchToggle::Word => "ab",
            SearchToggle::Regex => ".*",
        };

        if active {
            buf.fill_rect(x, y, w, h, theme::PRIMARY_DIM);
        } else if hovered {
            buf.fill_rect(x, y, w, h, theme::BG_ELEVATED);
        }

        let text_color = if active {
            theme::PRIMARY
        } else {
            theme::FG_MUTED
        };

        let font_px = TOGGLE_FONT * sf;
        let metrics = Metrics::new(font_px, (TOGGLE_FONT + 4.0) * sf);
        let label_w = measure_text_width(font_system, label, metrics, Family::SansSerif);
        let tx = x + (w as f32 - label_w).max(0.0) as usize / 2;
        let ty = y + (h as f32 - (TOGGLE_FONT + 4.0) * sf) as usize / 2;
        draw_text_at(
            buf,
            font_system,
            swash_cache,
            tx,
            ty,
            y + h,
            label,
            metrics,
            text_color,
            Family::SansSerif,
        );

        if active {
            let underline_h = (1.0 * sf).max(1.0) as usize;
            let u_pad = (3.0 * sf) as usize;
            buf.fill_rect(
                x + u_pad,
                y + h.saturating_sub(underline_h + 1),
                w.saturating_sub(u_pad * 2),
                underline_h,
                theme::PRIMARY,
            );
        }
    }
}

pub fn hit_test(
    phys_y: f64,
    content_y: usize,
    scroll_offset: f32,
    state: &SearchPanelState,
    sf: f64,
) -> Option<usize> {
    let input_h = INPUT_HEIGHT as f64 * sf;
    let input_pad_y = INPUT_PAD_Y as f64 * sf;
    let results_y = content_y as f64 + input_pad_y + input_h + input_pad_y;

    if phys_y < results_y {
        return None;
    }

    let file_h = RESULT_FILE_H as f64 * sf;
    let line_h = RESULT_LINE_H as f64 * sf;
    let gap = RESULT_GAP as f64 * sf;
    let pad_y_top = RESULTS_PAD_Y as f64 * sf;

    let rel_y = phys_y - results_y + scroll_offset as f64 - pad_y_top;
    if rel_y < 0.0 {
        return None;
    }

    let mut y_acc = 0.0;
    let mut flat_idx = 0usize;

    for group in &state.results {
        if rel_y >= y_acc && rel_y < y_acc + file_h {
            return Some(flat_idx);
        }
        y_acc += file_h;
        flat_idx += 1;

        for _ in &group.matches {
            if rel_y >= y_acc && rel_y < y_acc + line_h {
                return Some(flat_idx);
            }
            y_acc += line_h;
            flat_idx += 1;
        }

        y_acc += gap;
    }

    None
}

pub fn input_rect(content_y: usize, sf: f32, panel_w: usize) -> (usize, usize, usize, usize) {
    let pad_x = (INPUT_PAD_X * sf) as usize;
    let pad_y = (INPUT_PAD_Y * sf) as usize;
    let h = (INPUT_HEIGHT * sf) as usize;
    let w = panel_w.saturating_sub(pad_x * 2);
    (pad_x, content_y + pad_y, w, h)
}

pub fn is_in_input(phys_x: f64, phys_y: f64, content_y: usize, sf: f32, panel_w: usize) -> bool {
    let (ix, iy, iw, ih) = input_rect(content_y, sf, panel_w);
    phys_x >= ix as f64
        && phys_x <= (ix + iw) as f64
        && phys_y >= iy as f64
        && phys_y <= (iy + ih) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn re(q: &str) -> Regex {
        build_search_regex(q, false, false, false).unwrap()
    }

    #[test]
    fn search_file_contents_finds_matches() {
        let dir = std::env::temp_dir().join("awebo_search_test");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("test_search.txt");
        std::fs::write(&file, "Hello World\nfoo bar\nHello Again\n").unwrap();

        let matches = search_file_contents(&file, &re("hello"));
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line_num, 1);
        assert_eq!(matches[1].line_num, 3);

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn skip_binary_files() {
        let dir = std::env::temp_dir().join("awebo_search_bin");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("binary.dat");
        let mut data = b"hello world\n".to_vec();
        data.push(0);
        data.extend_from_slice(b"more data");
        std::fs::write(&file, &data).unwrap();

        let matches = search_file_contents(&file, &re("hello"));
        assert!(matches.is_empty());

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn flat_index_maps_correctly() {
        let state = SearchPanelState {
            results: vec![
                SearchFileGroup {
                    path: PathBuf::from("/a/foo.rs"),
                    rel_path: "foo.rs".to_string(),
                    file_name: "foo.rs".to_string(),
                    dir_part: String::new(),
                    matches: vec![
                        SearchLineMatch {
                            line_num: 10,
                            num_str: "10".to_string(),
                            snippet_trimmed: "fn foo".to_string(),
                        },
                        SearchLineMatch {
                            line_num: 20,
                            num_str: "20".to_string(),
                            snippet_trimmed: "fn bar".to_string(),
                        },
                    ],
                },
                SearchFileGroup {
                    path: PathBuf::from("/a/bar.rs"),
                    rel_path: "bar.rs".to_string(),
                    file_name: "bar.rs".to_string(),
                    dir_part: String::new(),
                    matches: vec![],
                },
            ],
            ..Default::default()
        };

        assert_eq!(state.flat_row_count(), 4);
        let (p0, l0) = state.path_at_flat_index(0).unwrap();
        assert_eq!(p0, PathBuf::from("/a/foo.rs"));
        assert!(l0.is_none());

        let (p1, l1) = state.path_at_flat_index(1).unwrap();
        assert_eq!(p1, PathBuf::from("/a/foo.rs"));
        assert_eq!(l1, Some(10));

        let (p3, l3) = state.path_at_flat_index(3).unwrap();
        assert_eq!(p3, PathBuf::from("/a/bar.rs"));
        assert!(l3.is_none());
    }

    #[test]
    fn run_search_skips_git_dir() {
        let dir = std::env::temp_dir().join("awebo_search_git");
        let _ = std::fs::create_dir_all(dir.join(".git"));
        std::fs::write(dir.join(".git/config"), "hello search").unwrap();
        std::fs::write(dir.join("visible.txt"), "hello search").unwrap();

        let results = run_search(&dir, &re("hello"), 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].rel_path.contains("visible"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn case_sensitive_excludes_different_case() {
        let dir = std::env::temp_dir().join("awebo_search_case");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("case.txt");
        std::fs::write(&file, "Hello\nhello\nHELLO\n").unwrap();

        let rci = build_search_regex("hello", false, false, false).unwrap();
        let mi = search_file_contents(&file, &rci);
        assert_eq!(mi.len(), 3);

        let rcs = build_search_regex("hello", true, false, false).unwrap();
        let ms = search_file_contents(&file, &rcs);
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].line_num, 2);

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn whole_word_excludes_substrings() {
        let dir = std::env::temp_dir().join("awebo_search_word");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("word.txt");
        std::fs::write(&file, "foo\nfoobar\nbar foo baz\n").unwrap();

        let rany = build_search_regex("foo", false, false, false).unwrap();
        let m_any = search_file_contents(&file, &rany);
        assert_eq!(m_any.len(), 3);

        let rword = build_search_regex("foo", false, true, false).unwrap();
        let m_word = search_file_contents(&file, &rword);
        assert_eq!(m_word.len(), 2);
        assert_eq!(m_word[0].line_num, 1);
        assert_eq!(m_word[1].line_num, 3);

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn regex_on_matches() {
        let dir = std::env::temp_dir().join("awebo_search_regex");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("re.txt");
        std::fs::write(&file, "fn foo()\nfn bar()\nstruct S;\n").unwrap();

        let r = build_search_regex(r"^fn \w+", false, false, true).unwrap();
        let matches = search_file_contents(&file, &r);
        assert_eq!(matches.len(), 2);

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn regex_compile_error_sets_flag() {
        let mut state = SearchPanelState {
            regex: true,
            ..Default::default()
        };
        state.query = "[".to_string();
        state.trigger_search();
        assert!(state.regex_error);
        assert!(state.results.is_empty());
        assert!(!state.searching);
    }

    #[test]
    fn case_and_word_combined() {
        let dir = std::env::temp_dir().join("awebo_search_case_word");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("cw.txt");
        std::fs::write(&file, "FOO\nfoo\nfoobar\nFoo\n").unwrap();

        let r = build_search_regex("foo", true, true, false).unwrap();
        let m = search_file_contents(&file, &r);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].line_num, 2);

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn regex_with_flags() {
        let r = build_search_regex(r"\d+", false, true, true).unwrap();
        assert!(r.is_match("foo 123 bar"));
        assert!(!r.is_match("abc123def"));
    }

    #[test]
    fn toggle_flag_schedules_search() {
        let mut state = SearchPanelState {
            query: "x".to_string(),
            ..Default::default()
        };
        assert!(!state.case_sensitive);
        state.toggle_flag(SearchToggle::Case);
        assert!(state.case_sensitive);
        assert!(state.has_pending_debounce());
    }

    #[test]
    fn clear_preserves_toggles() {
        let mut state = SearchPanelState {
            case_sensitive: true,
            whole_word: true,
            regex: true,
            query: "foo".into(),
            ..Default::default()
        };
        state.clear();
        assert!(state.query.is_empty());
        assert!(state.case_sensitive);
        assert!(state.whole_word);
        assert!(state.regex);
    }
}
