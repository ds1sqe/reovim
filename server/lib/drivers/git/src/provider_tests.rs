use {
    super::*,
    crate::types::{BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry},
    std::path::Path,
};

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

/// Minimal impl to test default methods.
struct MinimalProvider;

#[cfg_attr(coverage_nightly, coverage(off))]
impl GitProvider for MinimalProvider {
    fn current_branch(&self, _cwd: &Path) -> Option<String> {
        None
    }
    fn branches(&self, _cwd: &Path) -> Vec<BranchInfo> {
        vec![]
    }
    fn status(&self, _cwd: &Path) -> Vec<StatusEntry> {
        vec![]
    }
    fn log(&self, _cwd: &Path, _query: &str, _limit: usize) -> Vec<LogEntry> {
        vec![]
    }
    fn stash_list(&self, _cwd: &Path) -> Vec<StashEntry> {
        vec![]
    }
    fn diff_hunks(&self, _path: &Path) -> Vec<DiffHunk> {
        vec![]
    }
}

/// Default `blame()` returns empty vec.
#[test]
fn blame_default_returns_empty() {
    let provider = MinimalProvider;
    let result = provider.blame(Path::new("/tmp/test.rs"));
    assert!(result.is_empty());
}
