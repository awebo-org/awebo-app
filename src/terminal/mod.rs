use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::{Flags, LineLength};
use alacritty_terminal::term::{Config, TermMode};
use alacritty_terminal::tty;

use parking_lot::Mutex;

use crate::blocks::{StyledLine, StyledSpan};
use crate::prompt::PromptState;
use crate::renderer::theme;

#[derive(Clone)]
pub struct JsonEventProxy {
    pub proxy: winit::event_loop::EventLoopProxy<TerminalEvent>,
    pub title: Arc<Mutex<String>>,
}

#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Wakeup,
    Title(String),
    Exit,
    MenuAction(muda::MenuEvent),
    AiError(String),
    /// A command finished with the given exit code (from shell integration hook).
    CommandExitCode(i32),
    /// Background model file deletion completed (registry index).
    ModelDeleted(usize),
    /// An agent tool finished executing on a background thread.
    ToolComplete {
        request: crate::agent::parser::ToolCallRequest,
        result: crate::agent::tools::ToolResult,
    },
    /// A sandbox shell process exited.
    SandboxExit {
        name: String,
        code: i32,
    },
    /// A sandbox encountered an error during creation or runtime.
    SandboxError(String),
    /// A toast notification to display to the user.
    Toast(String),
    /// A toast with a specific severity level.
    ToastLevel(String, crate::ui::components::toast::ToastLevel),
}

impl JsonEventProxy {
    pub fn new(proxy: winit::event_loop::EventLoopProxy<TerminalEvent>) -> Self {
        Self {
            proxy,
            title: Arc::new(Mutex::new(String::from("~"))),
        }
    }
}

/// Prefix used by our shell integration hook to report exit codes
/// via OSC title sequences: `\e]0;__TERM_EC:{code}__\a`
const EXIT_CODE_PREFIX: &str = "__TERM_EC:";
const EXIT_CODE_SUFFIX: &str = "__";

impl EventListener for JsonEventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::Wakeup => {
                let _ = self.proxy.send_event(TerminalEvent::Wakeup);
            }
            Event::Title(title) => {
                if let Some(rest) = title.strip_prefix(EXIT_CODE_PREFIX)
                    && let Some(code_str) = rest.strip_suffix(EXIT_CODE_SUFFIX)
                    && let Ok(code) = code_str.parse::<i32>()
                {
                    let _ = self.proxy.send_event(TerminalEvent::CommandExitCode(code));
                    return;
                }
                *self.title.lock() = title.clone();
                let _ = self.proxy.send_event(TerminalEvent::Title(title));
            }
            Event::Exit | Event::ChildExit(_) => {
                let _ = self.proxy.send_event(TerminalEvent::Exit);
            }
            _ => {}
        }
    }
}

pub struct Terminal {
    pub term: Arc<FairMutex<Term<JsonEventProxy>>>,
    pub sender: EventLoopSender,
    pub title: Arc<Mutex<String>>,
    pub child_pid: u32,
    pub prompt_state: PromptState,
}

impl Terminal {
    /// Create a new terminal with the given dimensions and shell path.
    ///
    /// If `shell_path` is `None`, the user's default shell is used.
    pub fn new(
        cols: u16,
        lines: u16,
        cell_width: u16,
        cell_height: u16,
        event_proxy: JsonEventProxy,
        shell_path: Option<&str>,
    ) -> Self {
        let config = Config::default();

        let window_size = WindowSize {
            num_cols: cols,
            num_lines: lines,
            cell_width,
            cell_height,
        };

        let title = event_proxy.title.clone();

        tty::setup_env();

        let term_size = TermSize(window_size);
        let term = Term::new(config, &term_size, event_proxy.clone());
        let term = Arc::new(FairMutex::new(term));

        let shell = shell_path
            .map(|s| s.to_string())
            .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string()));

        let shell_display = std::path::Path::new(&shell)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| shell.clone());

        let prompt_state = PromptState::new(&shell_display);

        let mut env = HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());
        env.insert(
            "LANG".to_string(),
            std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".to_string()),
        );
        env.insert("TERM_PROGRAM".to_string(), "terminal".to_string());
        env.insert("TERM_PROGRAM_VERSION".to_string(), "0.1.0".to_string());

        let pty_config = tty::Options {
            shell: Some(tty::Shell::new(shell, vec!["-l".to_string()])),
            env,
            ..tty::Options::default()
        };

        let pty = tty::new(&pty_config, window_size, 0).expect("Failed to create PTY");

        let child_pid = pty.child().id();

        let event_loop = EventLoop::new(term.clone(), event_proxy, pty, false, false)
            .expect("Failed to create event loop");

        let sender = event_loop.channel();
        event_loop.spawn();

        let shell_hook = shell_integration_hook(&shell_display);
        if !shell_hook.is_empty() {
            let _ = sender.send(Msg::Input(Cow::Owned(shell_hook.into_bytes())));
        }

        Self {
            term,
            sender,
            title,
            child_pid,
            prompt_state,
        }
    }

    pub fn display_title(&self) -> String {
        let cwd = process_cwd(self.child_pid);
        if let Some(path) = cwd {
            abbreviate_path(&path)
        } else {
            let t = self.title.lock();
            if t.is_empty() {
                "~".to_string()
            } else {
                t.clone()
            }
        }
    }

    pub fn cwd(&self) -> Option<String> {
        process_cwd(self.child_pid)
    }

    /// Returns `true` when a TUI application has taken control of the
    /// terminal.  This is detected by alt screen being active **or** the
    /// cursor being hidden (many TUI apps like Claude Code hide the
    /// cursor without using the alternate screen buffer).
    pub fn is_app_controlled(&self) -> bool {
        let mode = *self.term.lock().mode();
        mode.contains(TermMode::ALT_SCREEN) || !mode.contains(TermMode::SHOW_CURSOR)
    }

    pub fn prompt_info(&self) -> crate::prompt::PromptInfo {
        self.prompt_state.collect(self.cwd().as_deref())
    }

    pub fn input(&self, data: Cow<'static, [u8]>) {
        let _ = self.sender.send(Msg::Input(data));
    }

    /// Start a new character-level selection at the given grid point.
    pub fn start_selection(
        &self,
        point: alacritty_terminal::index::Point,
        side: alacritty_terminal::index::Side,
    ) {
        let mut term = self.term.lock();
        let ty = alacritty_terminal::selection::SelectionType::Simple;
        term.selection = Some(alacritty_terminal::selection::Selection::new(
            ty, point, side,
        ));
    }

    /// Extend the current selection to the given grid point.
    pub fn update_selection(
        &self,
        point: alacritty_terminal::index::Point,
        side: alacritty_terminal::index::Side,
    ) {
        let mut term = self.term.lock();
        if let Some(sel) = &mut term.selection {
            sel.update(point, side);
        }
    }

    /// Extract the selected text as a string, if any.
    pub fn selection_to_string(&self) -> Option<String> {
        let term = self.term.lock();
        term.selection_to_string()
    }

    /// Clear the current selection.
    pub fn clear_selection(&self) {
        let mut term = self.term.lock();
        term.selection = None;
    }

    /// Extract the text content of a viewport row (0-based from top of screen).
    pub fn screen_row_text(&self, row: i32) -> String {
        let term = self.term.lock();
        let grid = term.grid();
        let line = Line(row);
        if line < grid.topmost_line() || line > grid.bottommost_line() {
            return String::new();
        }
        let row_ref = &grid[line];
        let len = row_ref.line_length();
        let mut text = String::new();
        for col_idx in 0..len.0 {
            let cell = &row_ref[Column(col_idx)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            text.push(cell.c);
        }
        text
    }

    pub fn resize(&self, window_size: WindowSize) {
        let _ = self.sender.send(Msg::Resize(window_size));
        let mut term = self.term.lock();
        let term_size = TermSize(window_size);
        term.resize(term_size);
    }

    /// Like `read_lines_from` but preserves foreground color information
    /// from the terminal grid cells, returning styled lines.
    ///
    /// The `from_abs` parameter is an absolute scroll position (cursor
    /// distance from the top of the scrollback at the time the checkpoint
    /// was taken).  When the scrollback buffer overflows and old lines are
    /// evicted, the checkpoint can become invalid — `from_abs` points
    /// past the current grid.  In that case we recover by reading from
    /// the oldest surviving line.
    pub fn read_styled_lines_from(&self, from_abs: i32) -> (Vec<StyledLine>, i32) {
        let term = self.term.lock();
        let content = term.renderable_content();
        let colors = content.colors;
        let grid = term.grid();
        let top = grid.topmost_line();
        let cursor_line = grid.cursor.point.line;
        let end = cursor_line;

        let mut start = Line(from_abs + top.0);

        if start > end || start < top {
            if from_abs != 0 {
                log::warn!(
                    "read_styled_lines_from: scrollback overflow — from_abs={} top={} computed_start={} cursor={}, resetting to top",
                    from_abs,
                    top.0,
                    start.0,
                    end.0,
                );
            }
            start = top;
        }

        let mut out: Vec<StyledLine> = Vec::new();
        let mut line = start;
        while line < end {
            if line > grid.bottommost_line() {
                break;
            }
            if line >= top {
                out.push(Self::row_to_styled(&grid[line], colors));
            }
            line += 1i32;
        }

        let new_abs = end.0 - top.0;
        (out, new_abs)
    }

    /// Convert a grid row into a styled line, coalescing runs of
    /// identical foreground color.
    fn row_to_styled(
        row: &alacritty_terminal::grid::Row<alacritty_terminal::term::cell::Cell>,
        colors: &alacritty_terminal::term::color::Colors,
    ) -> StyledLine {
        let line_len = row.line_length();
        let mut spans: StyledLine = Vec::new();
        let mut current_text = String::new();
        let mut current_fg: crate::renderer::pixel_buffer::Rgb = (171, 178, 191); // default fg

        for col_idx in 0..line_len.0 {
            let cell = &row[Column(col_idx)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }

            let fg = theme::resolve_color(&cell.fg, colors);

            if fg != current_fg && !current_text.is_empty() {
                spans.push(StyledSpan::plain(
                    std::mem::take(&mut current_text),
                    current_fg,
                ));
            }
            current_fg = fg;

            current_text.push(cell.c);
            if let Some(zw) = cell.zerowidth() {
                for &c in zw {
                    current_text.push(c);
                }
            }
        }

        let trimmed = current_text.trim_end().to_string();
        if !trimmed.is_empty() {
            spans.push(StyledSpan::plain(trimmed, current_fg));
        }

        spans
    }

    /// Returns `true` when the cursor sits on a line that looks like a
    /// shell prompt (PS1).  The heuristic checks that:
    /// 1. The cursor column is > 0 (there is text before the cursor).
    /// 2. The text before the cursor ends with a common prompt suffix
    ///    (`%`, `$`, `#`, `❯`, `>`) followed by a single space.
    ///
    /// This avoids false positives from commands that print text and
    /// leave the cursor at a non-zero column.
    pub fn cursor_on_prompt(&self) -> bool {
        let term = self.term.lock();
        let grid = term.grid();
        let cursor = &grid.cursor;
        let col = cursor.point.column.0;
        if col == 0 {
            return false;
        }
        let row = &grid[cursor.point.line];
        let mut text = String::new();
        for c in 0..col {
            let cell = &row[Column(c)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            text.push(cell.c);
        }
        let trimmed = text.trim_end();
        crate::blocks::text_looks_like_prompt(trimmed)
    }

    /// Read the current cursor line as a styled line.
    /// Used to capture prompts like "Password:" that appear on the cursor
    /// line and would otherwise be missed by `read_styled_lines_from`.
    pub fn read_cursor_line_if_not_prompt(&self) -> Option<StyledLine> {
        let term = self.term.lock();
        let grid = term.grid();
        let cursor = &grid.cursor;
        let col = cursor.point.column.0;
        if col == 0 {
            return None;
        }
        let row = &grid[cursor.point.line];

        let mut text = String::new();
        for c in 0..col {
            let cell = &row[Column(c)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            text.push(cell.c);
        }
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        if crate::blocks::text_looks_like_prompt(trimmed) {
            return None;
        }

        let content = term.renderable_content();
        let line = Self::row_to_styled(row, content.colors);
        Some(line)
    }
}

/// Build a shell command that installs an invisible hook to report exit
/// codes.  The hook runs after every command (via precmd for zsh,
/// PROMPT_COMMAND for bash) and sends the exit code as a special OSC
/// title that our `JsonEventProxy` intercepts.  The title is consumed
/// before reaching `self.title`, so it is invisible to the user.
///
/// The command is wrapped in a way that:
/// - Does not echo anything to the terminal
/// - Does not appear in shell history
/// - Works even when the shell sources .zshrc/.bashrc
fn shell_integration_hook(shell_name: &str) -> String {
    match shell_name {
        "zsh" => {
            " __term_report_ec() { printf '\\e]0;__TERM_EC:%d__\\a' $? }; precmd_functions=(__term_report_ec $precmd_functions)\r".to_string()
        }
        "bash" => {
            " __term_report_ec() { local ec=$?; printf '\\e]0;__TERM_EC:%d__\\a' $ec; return $ec; }; PROMPT_COMMAND=\"__term_report_ec;${PROMPT_COMMAND:-}\"\r".to_string()
        }
        "fish" => {
            " function __term_report_ec --on-event fish_prompt; printf '\\e]0;__TERM_EC:%d__\\a' $status; end\r".to_string()
        }
        _ => String::new(),
    }
}

pub struct TermSize(pub WindowSize);

impl Dimensions for TermSize {
    fn columns(&self) -> usize {
        self.0.num_cols as usize
    }

    fn screen_lines(&self) -> usize {
        self.0.num_lines as usize
    }

    fn total_lines(&self) -> usize {
        self.screen_lines()
    }
}

/// Abbreviate a filesystem path for display in a tab title.
///
/// Replaces the home directory prefix with `~` and returns the
/// last component for deep paths.
fn abbreviate_path(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }

    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    if !home.is_empty() && path.starts_with(&home) {
        let rest = &path[home.len()..];
        if rest.is_empty() {
            "~".to_string()
        } else {
            format!("~{rest}")
        }
    } else {
        path.to_string()
    }
}

/// Find the URL in `line` whose character span covers `col` (0-based).
///
/// Recognises `http://` and `https://` schemes.  Returns the full URL
/// string or `None` if no URL covers the given column.
pub fn url_at_col(line: &str, col: usize) -> Option<String> {
    for scheme in &["https://", "http://"] {
        let mut search_from = 0;
        while let Some(start) = line[search_from..].find(scheme) {
            let abs_start = search_from + start;
            let rest = &line[abs_start..];
            let end = rest
                .find(|c: char| {
                    c.is_whitespace()
                        || c == '\''
                        || c == '"'
                        || c == '>'
                        || c == '<'
                        || c == ')'
                        || c == ']'
                })
                .unwrap_or(rest.len());
            let char_start = line[..abs_start].chars().count();
            let char_end = char_start + rest[..end].chars().count();
            if col >= char_start && col < char_end {
                return Some(rest[..end].to_string());
            }
            search_from = abs_start + end;
        }
    }
    None
}

/// Read the current working directory of a process by PID.
///
/// Uses `proc_pidinfo` on macOS and `/proc/PID/cwd` on Linux.
#[cfg(target_os = "macos")]
fn process_cwd(pid: u32) -> Option<String> {
    use std::ffi::CStr;
    use std::mem;

    #[repr(C)]
    struct VnodePathInfo {
        _cdir: VnodeInfoPath,
        _rdir: VnodeInfoPath,
    }

    #[repr(C)]
    struct VnodeInfoPath {
        _vi: [u8; 152],
        vip_path: [u8; 1024],
    }

    const PROC_PIDVNODEPATHINFO: i32 = 9;

    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut libc::c_void,
            buffersize: i32,
        ) -> i32;
    }

    let mut info: VnodePathInfo = unsafe { mem::zeroed() };
    let size = mem::size_of::<VnodePathInfo>() as i32;

    let ret = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDVNODEPATHINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        )
    };

    if ret <= 0 {
        return None;
    }

    let cstr = unsafe { CStr::from_ptr(info._cdir.vip_path.as_ptr() as *const i8) };
    cstr.to_str().ok().map(|s| s.to_string())
}

#[cfg(target_os = "linux")]
fn process_cwd(pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/cwd"))
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
fn process_cwd(_pid: u32) -> Option<String> {
    None
}

/// Detect available shells on the system.
///
/// On Unix, reads `/etc/shells` and validates each path exists.
/// On Windows, scans PATH for common shell executables.
pub fn detect_shells() -> Vec<(String, String)> {
    let mut shells: Vec<(String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    #[cfg(unix)]
    {
        if let Ok(contents) = std::fs::read_to_string("/etc/shells") {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if std::path::Path::new(line).exists() && seen.insert(line.to_string()) {
                    let name = std::path::Path::new(line)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| line.to_string());
                    shells.push((name, line.to_string()));
                }
            }
        }
    }

    #[cfg(windows)]
    {
        let candidates = ["pwsh.exe", "powershell.exe", "bash.exe", "cmd.exe"];
        for name in &candidates {
            if let Ok(output) = std::process::Command::new("where").arg(name).output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !path.is_empty() && seen.insert(path.clone()) {
                        let display = name.trim_end_matches(".exe").to_string();
                        shells.push((display, path));
                    }
                }
            }
        }
    }

    shells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviate_path_root() {
        assert_eq!(abbreviate_path("/"), "/");
    }

    #[test]
    fn abbreviate_path_home_dir() {
        if let Some(home) = dirs::home_dir() {
            let home_str = home.to_string_lossy().to_string();
            assert_eq!(abbreviate_path(&home_str), "~");
        }
    }

    #[test]
    fn abbreviate_path_home_subdir() {
        if let Some(home) = dirs::home_dir() {
            let path = format!("{}/subdir", home.to_string_lossy());
            assert_eq!(abbreviate_path(&path), "~/subdir");
        }
    }

    #[test]
    fn abbreviate_path_non_home() {
        let result = abbreviate_path("/var/log/syslog");
        assert_eq!(result, "/var/log/syslog");
    }

    #[test]
    fn url_at_col_http() {
        let line = "  GET http://business.localhost:1355/api 200 in 4ms";
        assert_eq!(
            url_at_col(line, 6),
            Some("http://business.localhost:1355/api".to_string()),
        );
    }

    #[test]
    fn url_at_col_https() {
        let line = "Visit https://example.com/path?q=1 for details";
        assert_eq!(
            url_at_col(line, 10),
            Some("https://example.com/path?q=1".to_string()),
        );
    }

    #[test]
    fn url_at_col_outside() {
        let line = "no url here at all";
        assert_eq!(url_at_col(line, 5), None);
    }

    #[test]
    fn url_at_col_before_url() {
        let line = "prefix http://x.com rest";
        assert_eq!(url_at_col(line, 2), None);
    }
}
