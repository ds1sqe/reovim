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
mod tests {
    use super::*;

    struct MockLifecycle {
        called: std::sync::atomic::AtomicBool,
    }

    impl MockLifecycle {
        fn new() -> Self {
            Self {
                called: std::sync::atomic::AtomicBool::new(false),
            }
        }

        fn was_called(&self) -> bool {
            self.called.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    impl LspLifecycle for MockLifecycle {
        fn auto_start(
            &self,
            _services: &Arc<ServiceRegistry>,
            _config: LspServerConfig,
            _language_id: String,
            _file_path: String,
            _buffer_content: String,
        ) {
            self.called
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    impl std::fmt::Debug for MockLifecycle {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockLifecycle").finish()
        }
    }

    #[test]
    fn lifecycle_registry_default_is_empty() {
        let reg = LspLifecycleRegistry::default();
        assert!(reg.get().is_none());
    }

    #[test]
    fn lifecycle_registry_new_is_empty() {
        let reg = LspLifecycleRegistry::new();
        assert!(reg.get().is_none());
    }

    #[test]
    fn lifecycle_registry_register_and_get() {
        let reg = LspLifecycleRegistry::new();
        let mock = Arc::new(MockLifecycle::new());
        reg.register(mock);
        assert!(reg.get().is_some());
    }

    #[test]
    fn lifecycle_registry_overwrite() {
        let reg = LspLifecycleRegistry::new();
        let mock1 = Arc::new(MockLifecycle::new());
        let mock2 = Arc::new(MockLifecycle::new());
        reg.register(mock1);
        reg.register(mock2);
        assert!(reg.get().is_some());
    }

    #[test]
    fn lifecycle_auto_start_mock() {
        let mock = Arc::new(MockLifecycle::new());
        let services = Arc::new(ServiceRegistry::new());
        let config = LspServerConfig::rust_analyzer(std::path::Path::new("/tmp"));

        mock.auto_start(
            &services,
            config,
            "rust".to_owned(),
            "/tmp/main.rs".to_owned(),
            "fn main() {}".to_owned(),
        );

        assert!(mock.was_called());
    }

    #[test]
    fn lifecycle_registry_get_returns_cloned_arc() {
        let reg = LspLifecycleRegistry::new();
        let mock: Arc<dyn LspLifecycle> = Arc::new(MockLifecycle::new());
        reg.register(Arc::clone(&mock));
        let got = reg.get().unwrap();
        // Should be a different Arc pointing to same allocation
        assert!(Arc::ptr_eq(&mock, &got));
    }

    #[test]
    fn lifecycle_registry_service_impl() {
        let registry = ServiceRegistry::new();
        let lifecycle_reg = Arc::new(LspLifecycleRegistry::new());
        registry.register(lifecycle_reg);

        let retrieved = registry.get::<LspLifecycleRegistry>();
        assert!(retrieved.is_some());
    }

    #[test]
    fn lifecycle_registry_debug() {
        let reg = LspLifecycleRegistry::new();
        let debug = format!("{reg:?}");
        assert!(debug.contains("LspLifecycleRegistry"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn lifecycle_trait_is_object_safe() {
        fn _accepts_ref(_: &dyn LspLifecycle) {}
        fn _accepts_box(_: Box<dyn LspLifecycle>) {}
    }

    #[test]
    fn lifecycle_registry_concurrent_access() {
        use std::sync::Arc;

        let reg = Arc::new(LspLifecycleRegistry::new());

        // Write from one thread
        let reg_clone = Arc::clone(&reg);
        let writer = std::thread::spawn(move || {
            reg_clone.register(Arc::new(MockLifecycle::new()));
        });

        writer.join().unwrap();

        // Read from multiple threads
        let mut readers = Vec::new();
        for _ in 0..4 {
            let r = Arc::clone(&reg);
            readers.push(std::thread::spawn(move || r.get().is_some()));
        }

        for handle in readers {
            assert!(handle.join().unwrap());
        }
    }

    #[test]
    fn lifecycle_new_and_default_equivalent() {
        let new = LspLifecycleRegistry::new();
        let default = LspLifecycleRegistry::default();
        assert!(new.get().is_none());
        assert!(default.get().is_none());
    }
}
