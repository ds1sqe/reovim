//! Git provider trait — mechanism layer.
//!
//! Defines WHAT git data is available. Modules provide HOW
//! to fetch it (subprocess, libgit2, etc.).

use std::path::Path;

use crate::types::{BlameEntry, BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry};

/// Trait contract for git data access.
///
/// Implementations provide the actual git interaction (subprocess, library, etc.).
/// All methods return empty results when the tool is unavailable or the path
/// is not inside a git repository — graceful degradation by design.
pub trait GitProvider: Send + Sync {
    /// Get the current branch name.
    ///
    /// Returns `None` if not in a git repository or HEAD is detached.
    fn current_branch(&self, cwd: &Path) -> Option<String>;

    /// List all local branches.
    fn branches(&self, cwd: &Path) -> Vec<BranchInfo>;

    /// Get the working tree status (porcelain).
    fn status(&self, cwd: &Path) -> Vec<StatusEntry>;

    /// Search the commit log.
    ///
    /// When `query` is empty, returns the most recent `limit` commits.
    fn log(&self, cwd: &Path, query: &str, limit: usize) -> Vec<LogEntry>;

    /// List stash entries.
    fn stash_list(&self, cwd: &Path) -> Vec<StashEntry>;

    /// Get diff hunks for a specific file against HEAD.
    fn diff_hunks(&self, path: &Path) -> Vec<DiffHunk>;

    /// Get blame information for a file.
    ///
    /// Returns blame entries for each line in the file.
    /// Returns empty vec if not in a git repo or file is untracked.
    fn blame(&self, _path: &Path) -> Vec<BlameEntry> {
        vec![]
    }
}
