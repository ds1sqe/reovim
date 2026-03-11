//! Git provider trait — mechanism layer.
//!
//! Defines WHAT git data is available. Modules provide HOW
//! to fetch it (subprocess, libgit2, etc.).

use std::path::Path;

use crate::types::{BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry};

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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `GitProvider` is object-safe (can be used as `dyn GitProvider`).
    #[test]
    fn git_provider_is_object_safe() {
        fn assert_object_safe(_: &dyn GitProvider) {}
        // Compile-time check only — never called.
        let _ = assert_object_safe;
    }

    /// Verify that `dyn GitProvider` satisfies Send + Sync.
    #[test]
    fn git_provider_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<Box<dyn GitProvider>>();
        assert_sync::<Box<dyn GitProvider>>();
    }
}
