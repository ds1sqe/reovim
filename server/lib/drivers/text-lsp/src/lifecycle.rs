//! LSP server lifecycle traits.
//!
//! Defines the `LspLifecycle` trait for starting LSP servers without
//! depending on the concrete implementation in `reovim-module-lsp`.
//!
//! # Architecture
//!
//! This is mechanism: it defines WHAT operations are available.
//! The `reovim-module-lsp` provides the policy: HOW servers are started.

use std::sync::Arc;

use reovim_kernel::api::v1::{Service, ServiceRegistry};

use crate::LspServerConfig;

/// Trait for starting LSP servers.
///
/// Implemented by `reovim-module-lsp`. Consumers only need this trait
/// to trigger LSP auto-start without depending on the module.
pub trait LspLifecycle: Send + Sync {
    /// Auto-start an LSP server for a language.
    ///
    /// Implementations spawn the server asynchronously and register it
    /// in `LspProviderRegistry`. Progress notifications are pushed to
    /// `PendingNotificationQueue` if available.
    fn auto_start(
        &self,
        services: &Arc<ServiceRegistry>,
        config: LspServerConfig,
        language_id: String,
        file_path: String,
        buffer_content: String,
        buffer_id: u64,
    );
}

/// Registry for LSP lifecycle implementations.
///
/// Allows modules to register an `LspLifecycle` implementation that
/// other modules can discover via `ServiceRegistry`.
pub struct LspLifecycleRegistry {
    lifecycle: parking_lot::RwLock<Option<Arc<dyn LspLifecycle>>>,
}

impl std::fmt::Debug for LspLifecycleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspLifecycleRegistry")
            .field("has_lifecycle", &self.lifecycle.read().is_some())
            .finish()
    }
}

impl LspLifecycleRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lifecycle: parking_lot::RwLock::new(None),
        }
    }

    /// Register an LSP lifecycle implementation.
    pub fn register(&self, lifecycle: Arc<dyn LspLifecycle>) {
        *self.lifecycle.write() = Some(lifecycle);
    }

    /// Get the registered LSP lifecycle implementation.
    #[must_use]
    pub fn get(&self) -> Option<Arc<dyn LspLifecycle>> {
        self.lifecycle.read().clone()
    }
}

impl Default for LspLifecycleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for LspLifecycleRegistry {}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
