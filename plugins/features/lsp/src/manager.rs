//! LSP manager for buffer synchronization.
//!
//! Manages the LSP client connection and document synchronization state.

use std::sync::{Arc, RwLock};

use reovim_lsp::{DiagnosticCache, LspSaturatorHandle};

use crate::{document::DocumentManager, hover::HoverCache};

/// Thread-safe wrapper for LSP state.
///
/// Registered in `PluginStateRegistry` for cross-plugin access.
pub struct SharedLspManager {
    inner: RwLock<LspManager>,
}

impl SharedLspManager {
    /// Create a new shared LSP manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(LspManager::new()),
        }
    }

    /// Access the manager immutably.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&LspManager) -> R,
    {
        let guard = self.inner.read().unwrap();
        f(&guard)
    }

    /// Access the manager mutably.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut LspManager) -> R,
    {
        let mut guard = self.inner.write().unwrap();
        f(&mut guard)
    }
}

impl Default for SharedLspManager {
    fn default() -> Self {
        Self::new()
    }
}

/// LSP manager state.
pub struct LspManager {
    /// Document state tracking.
    pub documents: DocumentManager,
    /// Handle to send requests to the saturator.
    pub handle: Option<LspSaturatorHandle>,
    /// Diagnostic cache for lock-free reads.
    pub cache: Option<Arc<DiagnosticCache>>,
    /// Hover cache for lock-free reads.
    pub hover_cache: HoverCache,
    /// Whether the LSP server is running.
    pub running: bool,
}

impl LspManager {
    /// Create a new LSP manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            documents: DocumentManager::new(),
            handle: None,
            cache: None,
            hover_cache: HoverCache::new(),
            running: false,
        }
    }

    /// Set the LSP handle and cache after startup.
    pub fn set_connection(&mut self, handle: LspSaturatorHandle, cache: Arc<DiagnosticCache>) {
        tracing::debug!("LSP: set_connection called, handle being set");
        self.handle = Some(handle);
        self.cache = Some(cache);
        self.running = true;
    }

    /// Check if the LSP server is running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Get the saturator handle.
    #[must_use]
    pub const fn handle(&self) -> Option<&LspSaturatorHandle> {
        self.handle.as_ref()
    }

    /// Get the diagnostic cache.
    #[must_use]
    pub const fn cache(&self) -> Option<&Arc<DiagnosticCache>> {
        self.cache.as_ref()
    }

    /// Shutdown the LSP server.
    pub fn shutdown(&mut self) {
        if let Some(handle) = &self.handle {
            handle.shutdown();
        }
        self.running = false;
    }
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}
