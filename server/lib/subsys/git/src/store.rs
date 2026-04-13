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
            .field("registered", &self.inner.read().unwrap().is_some())
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
