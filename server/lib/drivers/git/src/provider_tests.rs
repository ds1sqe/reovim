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

/// Default `stage_file()` returns false.
#[test]
fn stage_file_default_returns_false() {
    let provider = MinimalProvider;
    assert!(!provider.stage_file(Path::new("/tmp/test.rs")));
}

/// Default `reset_file()` returns false.
#[test]
fn reset_file_default_returns_false() {
    let provider = MinimalProvider;
    assert!(!provider.reset_file(Path::new("/tmp/test.rs")));
}

/// Default `unstage_file()` returns false.
#[test]
fn unstage_file_default_returns_false() {
    let provider = MinimalProvider;
    assert!(!provider.unstage_file(Path::new("/tmp/test.rs")));
}

/// Default `stage_lines()` returns false.
#[test]
fn stage_lines_default_returns_false() {
    let provider = MinimalProvider;
    assert!(!provider.stage_lines(Path::new("/tmp"), "patch"));
}

/// Default `reset_lines()` returns false.
#[test]
fn reset_lines_default_returns_false() {
    let provider = MinimalProvider;
    assert!(!provider.reset_lines(Path::new("/tmp"), "patch"));
}

/// Default `diff_content()` returns None.
#[test]
fn diff_content_default_returns_none() {
    let provider = MinimalProvider;
    assert!(provider.diff_content(Path::new("/tmp/test.rs")).is_none());
}

/// Default `invalidate()` is a no-op (does not panic).
#[test]
fn invalidate_default_is_noop() {
    let provider = MinimalProvider;
    provider.invalidate(Path::new("/tmp/test.rs"));
}
