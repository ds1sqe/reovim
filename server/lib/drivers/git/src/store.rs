//! Git provider store for service registry.
//!
//! Holds the single registered [`GitProvider`] implementation,
//! discoverable via `ServiceRegistry::get::<GitProviderStore>()`.

use {
    reovim_kernel::api::v1::Service,
    std::{
        fmt,
        sync::{Arc, RwLock},
    },
};

use crate::provider::GitProvider;

/// Stores the registered [`GitProvider`] implementation.
///
/// Registered by the git module during `init()`. Consumers retrieve
/// the store from `ServiceRegistry` and call [`get()`](Self::get)
/// to access the provider.
///
/// # Example
///
/// ```ignore
/// // In module init():
/// let store = ctx.services.get_or_create::<GitProviderStore>();
/// store.register(Arc::new(MyGitProvider));
///
/// // In consumers:
/// let store = ctx.services.get::<GitProviderStore>()?;
/// let git = store.get()?;
/// let branch = git.current_branch(&cwd);
/// ```
pub struct GitProviderStore {
    inner: RwLock<Option<Arc<dyn GitProvider>>>,
}

impl fmt::Debug for GitProviderStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GitProviderStore")
            .field(
                "registered",
                &self.inner.read().unwrap().is_some(),
            )
            .finish()
    }
}

impl GitProviderStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(None),
        }
    }

    /// Register a provider implementation.
    ///
    /// Replaces any previously registered provider.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn register(&self, provider: Arc<dyn GitProvider>) {
        *self.inner.write().unwrap() = Some(provider);
    }

    /// Get the registered provider, if any.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get(&self) -> Option<Arc<dyn GitProvider>> {
        self.inner.read().unwrap().clone()
    }
}

impl Default for GitProviderStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for GitProviderStore {}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::*, std::path::Path};

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
}
