use {
    super::*,
    crate::types::*,
    reovim_kernel::api::v1::Service,
    std::{path::Path, sync::Arc},
};

/// Minimal mock for testing the store.
struct MockGitProvider;

impl GitProvider for MockGitProvider {
    fn current_branch(&self, _cwd: &Path) -> Option<String> {
        Some("main".to_owned())
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

#[test]
fn store_default_is_empty() {
    let store = GitProviderStore::default();
    assert!(store.get().is_none());
}

#[test]
fn store_register_and_get() {
    let store = GitProviderStore::new();
    assert!(store.get().is_none());

    store.register(Arc::new(MockGitProvider));
    let provider = store.get();
    assert!(provider.is_some());

    let branch = provider.unwrap().current_branch(Path::new("."));
    assert_eq!(branch, Some("main".to_owned()));
}

#[test]
fn store_register_replaces() {
    let store = GitProviderStore::new();
    store.register(Arc::new(MockGitProvider));
    store.register(Arc::new(MockGitProvider));
    // Should not panic, just replace
    assert!(store.get().is_some());
}

#[test]
fn store_debug() {
    let store = GitProviderStore::new();
    let debug = format!("{store:?}");
    assert!(debug.contains("GitProviderStore"));
}

#[test]
fn store_is_service() {
    fn assert_service<T: Service>() {}
    assert_service::<GitProviderStore>();
}
