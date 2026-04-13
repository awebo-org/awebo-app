//! Git operations built on git2.

use std::path::Path;

/// Status of a single file in the working tree / index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

/// A file entry with its staging state.
#[derive(Debug, Clone)]
pub struct StatusEntry {
    pub path: String,
    pub status: FileStatus,
    pub staged: bool,
}

/// A branch entry.
#[derive(Debug, Clone)]
pub struct BranchEntry {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
}

/// Wrapper around `git2::Repository`.
pub struct GitRepo {
    repo: git2::Repository,
}

impl GitRepo {
    /// Open a repository by discovering upwards from `cwd`.
    /// Returns `None` if `cwd` is not inside a git repo.
    pub fn discover(cwd: &str) -> Option<Self> {
        git2::Repository::discover(cwd)
            .ok()
            .map(|repo| Self { repo })
    }

    /// Current branch name (or "HEAD" if detached).
    pub fn current_branch(&self) -> String {
        self.repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(String::from))
            .unwrap_or_else(|| "HEAD".into())
    }

    /// All files with changes (index or working tree).
    pub fn status_entries(&self) -> Vec<StatusEntry> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .exclude_submodules(true)
            .include_unmodified(false);

        let statuses = match self.repo.statuses(Some(&mut opts)) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let mut entries = Vec::with_capacity(statuses.len());
        for s in statuses.iter() {
            let path = s.path().unwrap_or("").to_string();
            let bits = s.status();

            if bits.intersects(git2::Status::INDEX_NEW) {
                entries.push(StatusEntry {
                    path: path.clone(),
                    status: FileStatus::Added,
                    staged: true,
                });
            }
            if bits.intersects(git2::Status::INDEX_MODIFIED) {
                entries.push(StatusEntry {
                    path: path.clone(),
                    status: FileStatus::Modified,
                    staged: true,
                });
            }
            if bits.intersects(git2::Status::INDEX_DELETED) {
                entries.push(StatusEntry {
                    path: path.clone(),
                    status: FileStatus::Deleted,
                    staged: true,
                });
            }
            if bits.intersects(git2::Status::INDEX_RENAMED) {
                entries.push(StatusEntry {
                    path: path.clone(),
                    status: FileStatus::Renamed,
                    staged: true,
                });
            }

            if bits.intersects(git2::Status::WT_NEW) {
                entries.push(StatusEntry {
                    path: path.clone(),
                    status: FileStatus::Untracked,
                    staged: false,
                });
            }
            if bits.intersects(git2::Status::WT_MODIFIED) {
                entries.push(StatusEntry {
                    path: path.clone(),
                    status: FileStatus::Modified,
                    staged: false,
                });
            }
            if bits.intersects(git2::Status::WT_DELETED) {
                entries.push(StatusEntry {
                    path: path.clone(),
                    status: FileStatus::Deleted,
                    staged: false,
                });
            }
            if bits.intersects(git2::Status::WT_RENAMED) {
                entries.push(StatusEntry {
                    path: path.clone(),
                    status: FileStatus::Renamed,
                    staged: false,
                });
            }
            if bits.intersects(git2::Status::CONFLICTED) {
                entries.push(StatusEntry {
                    path,
                    status: FileStatus::Conflicted,
                    staged: false,
                });
            }
        }
        entries
    }

    /// List all local and remote branches.
    pub fn branches(&self) -> Vec<BranchEntry> {
        let current = self.current_branch();
        let mut result = Vec::new();

        if let Ok(branches) = self.repo.branches(None) {
            for branch in branches.flatten() {
                let (b, btype) = branch;
                let name = b.name().ok().flatten().unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }
                let is_remote = btype == git2::BranchType::Remote;
                let is_current = !is_remote && name == current;
                result.push(BranchEntry {
                    name,
                    is_current,
                    is_remote,
                });
            }
        }
        result
    }

    /// Stage a file (add to index).
    pub fn stage_file(&self, path: &str) -> Result<(), String> {
        let mut index = self.repo.index().map_err(|e| e.message().to_string())?;
        let abs = self.repo.workdir().ok_or("bare repo")?.join(path);

        if abs.exists() {
            index
                .add_path(Path::new(path))
                .map_err(|e| e.message().to_string())?;
        } else {
            index
                .remove_path(Path::new(path))
                .map_err(|e| e.message().to_string())?;
        }
        index.write().map_err(|e| e.message().to_string())
    }

    /// Unstage a file (reset to HEAD).
    pub fn unstage_file(&self, path: &str) -> Result<(), String> {
        let head = self.repo.head().map_err(|e| e.message().to_string())?;
        let head_commit = head.peel_to_commit().map_err(|e| e.message().to_string())?;
        self.repo
            .reset_default(Some(head_commit.as_object()), [path])
            .map_err(|e| e.message().to_string())
    }

    /// Checkout a local branch by name.
    pub fn checkout_branch(&self, name: &str) -> Result<(), String> {
        let refname = format!("refs/heads/{name}");
        let obj = self
            .repo
            .revparse_single(&refname)
            .map_err(|e| e.message().to_string())?;
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.safe();
        self.repo
            .checkout_tree(&obj, Some(&mut opts))
            .map_err(|e| e.message().to_string())?;
        self.repo
            .set_head(&refname)
            .map_err(|e| e.message().to_string())
    }

    /// Discard working-tree changes for a single file (checkout from HEAD).
    pub fn discard_file_changes(&self, path: &str) -> Result<(), String> {
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.force().path(path);
        self.repo
            .checkout_head(Some(&mut opts))
            .map_err(|e| e.message().to_string())
    }

    /// Append an entry to .gitignore at the repo root.
    pub fn add_to_gitignore(&self, pattern: &str) -> Result<(), String> {
        use std::io::Write;
        let workdir = self.repo.workdir().ok_or("bare repository")?;
        let gitignore = workdir.join(".gitignore");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&gitignore)
            .map_err(|e| e.to_string())?;
        writeln!(file, "{pattern}").map_err(|e| e.to_string())
    }

    /// Commit staged changes with the given message.
    pub fn commit(&self, message: &str) -> Result<(), String> {
        let mut index = self.repo.index().map_err(|e| e.message().to_string())?;
        let tree_oid = index.write_tree().map_err(|e| e.message().to_string())?;
        let tree = self
            .repo
            .find_tree(tree_oid)
            .map_err(|e| e.message().to_string())?;
        let sig = self.repo.signature().map_err(|e| e.message().to_string())?;
        let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        self.repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .map_err(|e| e.message().to_string())?;
        Ok(())
    }

    /// Stage all unstaged files.
    pub fn stage_all(&self) -> Result<(), String> {
        let mut index = self.repo.index().map_err(|e| e.message().to_string())?;
        index
            .add_all(["."], git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| e.message().to_string())?;
        index.write().map_err(|e| e.message().to_string())
    }

    /// Unstage all staged files (reset index to HEAD).
    pub fn unstage_all(&self) -> Result<(), String> {
        let head = self.repo.head().map_err(|e| e.message().to_string())?;
        let head_commit = head.peel_to_commit().map_err(|e| e.message().to_string())?;
        self.repo
            .reset(head_commit.as_object(), git2::ResetType::Mixed, None)
            .map_err(|e| e.message().to_string())
    }

    /// Get a unified diff string for a specific file.
    pub fn diff_for_file(&self, path: &str, staged: bool) -> Result<String, String> {
        let mut opts = git2::DiffOptions::new();
        opts.pathspec(path);

        let diff = if staged {
            let head_tree = self.repo.head().ok().and_then(|h| h.peel_to_tree().ok());
            self.repo
                .diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))
        } else {
            self.repo.diff_index_to_workdir(None, Some(&mut opts))
        }
        .map_err(|e| e.message().to_string())?;

        let mut output = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let origin = line.origin();
            if origin == '+' || origin == '-' || origin == ' ' {
                output.push(origin);
            }
            if let Ok(content) = std::str::from_utf8(line.content()) {
                output.push_str(content);
            }
            true
        })
        .map_err(|e| e.message().to_string())?;

        Ok(output)
    }

    /// Full diff of all staged changes (for AI commit message generation).
    /// High-level summary of staged changes: file names with insertions/deletions stats.
    pub fn staged_diff_summary(&self) -> String {
        let head_tree = self.repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        let diff = match self.repo.diff_tree_to_index(head_tree.as_ref(), None, None) {
            Ok(d) => d,
            Err(_) => return String::new(),
        };
        let stats = diff.stats().ok();
        let mut output = String::new();
        let n = diff.deltas().len();
        for i in 0..n {
            if let Some(delta) = diff.get_delta(i) {
                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let kind = match delta.status() {
                    git2::Delta::Added => "added",
                    git2::Delta::Deleted => "deleted",
                    git2::Delta::Modified => "modified",
                    git2::Delta::Renamed => "renamed",
                    _ => "changed",
                };
                output.push_str(&format!("{path} ({kind})\n"));
            }
            if output.len() > 4000 {
                break;
            }
        }
        if let Some(s) = stats {
            output.push_str(&format!(
                "\n{} file(s), +{} -{}\n",
                s.files_changed(),
                s.insertions(),
                s.deletions()
            ));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_status_variants() {
        let variants = [
            FileStatus::Modified,
            FileStatus::Added,
            FileStatus::Deleted,
            FileStatus::Renamed,
            FileStatus::Untracked,
            FileStatus::Conflicted,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                assert_eq!(i == j, a == b);
            }
        }
    }

    #[test]
    fn status_entry_clone() {
        let e = StatusEntry {
            path: "foo.rs".into(),
            status: FileStatus::Modified,
            staged: true,
        };
        let e2 = e.clone();
        assert_eq!(e.path, e2.path);
        assert_eq!(e.status, e2.status);
        assert_eq!(e.staged, e2.staged);
    }

    #[test]
    fn branch_entry_fields() {
        let b = BranchEntry {
            name: "main".into(),
            is_current: true,
            is_remote: false,
        };
        assert!(b.is_current);
        assert!(!b.is_remote);
    }

    #[test]
    fn discover_nonexistent_returns_none() {
        assert!(GitRepo::discover("/tmp/definitely_not_a_git_repo_12345").is_none());
    }

    #[test]
    fn discover_current_dir() {
        let repo = GitRepo::discover(".");
        if let Some(r) = repo {
            let branch = r.current_branch();
            assert!(!branch.is_empty());
        }
    }

    #[test]
    fn status_entries_returns_vec() {
        if let Some(r) = GitRepo::discover(".") {
            let _ = r.status_entries(); // just ensure it doesn't panic
        }
    }
}
