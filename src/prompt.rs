use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;

pub type Rgb = (u8, u8, u8);

/// What kind of prompt segment this is (used by the renderer to pick an SVG icon).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Cwd,
    GitBranch,
    Shell,
}

#[derive(Debug, Clone)]
pub struct PromptSegment {
    pub kind: SegmentKind,
    pub text: String,
    pub fg: Rgb,
}

#[derive(Debug, Clone, Default)]
pub struct PromptInfo {
    pub segments: Vec<PromptSegment>,
    /// Lines added in the working tree (unstaged changes).
    pub diff_additions: usize,
    /// Lines removed in the working tree (unstaged changes).
    pub diff_deletions: usize,
}

impl PromptInfo {
    /// Extract the CWD segment text, if present.
    pub fn cwd(&self) -> Option<String> {
        self.segments
            .iter()
            .find(|s| matches!(s.kind, SegmentKind::Cwd))
            .map(|s| s.text.clone())
    }
}

const SEG_CWD_FG: Rgb = (160, 162, 170);
const SEG_GIT_CLEAN_FG: Rgb = (140, 155, 140);
const SEG_GIT_DIRTY_FG: Rgb = (180, 160, 120);
const SEG_SHELL_FG: Rgb = (100, 102, 110);

struct GitResult {
    branch: Option<String>,
    dirty: bool,
    additions: usize,
    deletions: usize,
    cwd: String,
    at: Instant,
}

pub struct PromptState {
    shell_name: String,
    git: Arc<Mutex<GitResult>>,
    pending: Arc<Mutex<bool>>,
}

impl PromptState {
    pub fn new(shell_name: &str) -> Self {
        Self {
            shell_name: shell_name.to_string(),
            git: Arc::new(Mutex::new(GitResult {
                branch: None,
                dirty: false,
                additions: 0,
                deletions: 0,
                cwd: String::new(),
                at: Instant::now(),
            })),
            pending: Arc::new(Mutex::new(false)),
        }
    }

    pub fn collect(&self, cwd: Option<&str>) -> PromptInfo {
        let mut segments = Vec::with_capacity(4);
        let mut diff_additions = 0;
        let mut diff_deletions = 0;

        if let Some(cwd) = cwd {
            segments.push(PromptSegment {
                kind: SegmentKind::Cwd,
                text: abbreviate_home(cwd),
                fg: SEG_CWD_FG,
            });

            self.refresh_git_if_stale(cwd);
            let cache = self.git.lock();
            if let Some(ref branch) = cache.branch {
                let suffix = if cache.dirty { "*" } else { "" };
                segments.push(PromptSegment {
                    kind: SegmentKind::GitBranch,
                    text: format!("{branch}{suffix}"),
                    fg: if cache.dirty {
                        SEG_GIT_DIRTY_FG
                    } else {
                        SEG_GIT_CLEAN_FG
                    },
                });
                diff_additions = cache.additions;
                diff_deletions = cache.deletions;
            }
        }

        segments.push(PromptSegment {
            kind: SegmentKind::Shell,
            text: self.shell_name.clone(),
            fg: SEG_SHELL_FG,
        });

        PromptInfo { segments, diff_additions, diff_deletions }
    }

    fn refresh_git_if_stale(&self, cwd: &str) {
        let cache = self.git.lock();
        let stale = cache.cwd != cwd || cache.at.elapsed().as_millis() > 2000;
        if !stale {
            return;
        }
        drop(cache);

        let mut pending = self.pending.lock();
        if *pending {
            return;
        }
        *pending = true;
        drop(pending);

        let git = Arc::clone(&self.git);
        let pending = Arc::clone(&self.pending);
        let cwd_owned = cwd.to_string();

        tokio::task::spawn_blocking(move || {
            let (branch, dirty, additions, deletions) = query_git(&cwd_owned);
            {
                let mut cache = git.lock();
                cache.branch = branch;
                cache.dirty = dirty;
                cache.additions = additions;
                cache.deletions = deletions;
                cache.cwd = cwd_owned;
                cache.at = Instant::now();
            }
            *pending.lock() = false;
        });
    }
}

fn query_git(cwd: &str) -> (Option<String>, bool, usize, usize) {
    let repo = match git2::Repository::discover(cwd) {
        Ok(r) => r,
        Err(_) => return (None, false, 0, 0),
    };

    let branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(String::from));

    let dirty = repo
        .statuses(Some(
            git2::StatusOptions::new()
                .include_untracked(true)
                .exclude_submodules(true),
        ))
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    let (additions, deletions) = diff_stats(&repo);

    (branch, dirty, additions, deletions)
}

/// Compute total lines added/removed in the working directory against HEAD.
fn diff_stats(repo: &git2::Repository) -> (usize, usize) {
    let head_tree = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_tree().ok());

    let diff = repo.diff_tree_to_workdir_with_index(
        head_tree.as_ref(),
        Some(
            git2::DiffOptions::new()
                .include_untracked(false)
                .ignore_whitespace(true),
        ),
    );

    match diff.and_then(|d| d.stats()) {
        Ok(stats) => (stats.insertions(), stats.deletions()),
        Err(_) => (0, 0),
    }
}

fn abbreviate_home(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home = home.to_string_lossy();
        if path == home.as_ref() {
            return "~".into();
        }
        if let Some(rest) = path.strip_prefix(home.as_ref()) {
            return format!("~{rest}");
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_segment_stores_text_and_color() {
        let seg = PromptSegment {
            kind: SegmentKind::Cwd,
            text: "hello".to_string(),
            fg: (255, 0, 0),
        };
        assert_eq!(seg.text, "hello");
        assert_eq!(seg.fg, (255, 0, 0));
    }

    #[test]
    fn prompt_info_default_is_empty() {
        let info = PromptInfo::default();
        assert!(info.segments.is_empty());
    }

    #[test]
    fn abbreviate_home_replaces_home() {
        if let Some(home) = dirs::home_dir() {
            let home_str = home.to_string_lossy().to_string();
            assert_eq!(abbreviate_home(&home_str), "~");
        }
    }

    #[test]
    fn abbreviate_home_replaces_home_subdir() {
        if let Some(home) = dirs::home_dir() {
            let path = format!("{}/projects/test", home.to_string_lossy());
            let result = abbreviate_home(&path);
            assert!(result.starts_with("~"));
            assert!(result.contains("projects/test"));
        }
    }

    #[test]
    fn abbreviate_home_leaves_nonhome_path() {
        let result = abbreviate_home("/tmp/something");
        assert_eq!(result, "/tmp/something");
    }

    #[test]
    fn prompt_state_collect_without_cwd() {
        let state = PromptState::new("zsh");
        let info = state.collect(None);
        assert_eq!(info.segments.len(), 1);
        assert!(info.segments[0].text.contains("zsh"));
    }

    #[test]
    fn prompt_state_collect_with_cwd() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let state = PromptState::new("bash");
        let info = state.collect(Some("/tmp"));
        assert!(info.segments.len() >= 2);
        assert!(info.segments[0].text.contains("/tmp"));
    }
}
