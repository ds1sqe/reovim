//! Snippet expander trait for cross-module decoupling.
//!
//! Defines the [`SnippetExpander`] trait so the completion module can
//! trigger snippet expansion without depending on `reovim-module-snippet`.
//!
//! # Architecture (#542)
//!
//! This is mechanism: it defines WHAT operation is available (expanding a
//! snippet body into the buffer with tab-stop navigation). The
//! `reovim-module-snippet` provides the policy: HOW snippets are parsed,
//! expanded, and navigated.

use std::sync::Arc;

use reovim_kernel::api::v1::{BufferId, Position, Service};

use crate::SessionRuntime;

/// Trait for expanding snippet bodies during completion confirm.
///
/// Implemented by `reovim-module-snippet`. The completion module invokes
/// this to insert snippet text and enter tab-stop navigation mode without
/// directly importing snippet module types.
pub trait SnippetExpander: Send + Sync {
    /// Expand a snippet body at the given position.
    ///
    /// Implementations should:
    /// 1. Parse the snippet body
    /// 2. Expand variables and compute tab stops
    /// 3. Insert the expanded text into the buffer
    /// 4. Store active snippet state in the session extension
    /// 5. Enter snippet navigation mode if tab stops exist
    /// 6. Position cursor at the first tab stop
    fn expand(
        &self,
        runtime: &mut SessionRuntime<'_>,
        buffer_id: BufferId,
        insert_pos: Position,
        snippet_body: &str,
    );
}

/// Registry for snippet expander implementations.
///
/// Allows modules to register a [`SnippetExpander`] implementation that
/// other modules can discover via `ServiceRegistry`.
pub struct SnippetExpanderRegistry {
    expander: parking_lot::RwLock<Option<Arc<dyn SnippetExpander>>>,
}

impl std::fmt::Debug for SnippetExpanderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnippetExpanderRegistry")
            .field("has_expander", &self.expander.read().is_some())
            .finish()
    }
}

impl SnippetExpanderRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            expander: parking_lot::RwLock::new(None),
        }
    }

    /// Register a snippet expander implementation.
    pub fn register(&self, expander: Arc<dyn SnippetExpander>) {
        *self.expander.write() = Some(expander);
    }

    /// Get the registered snippet expander implementation.
    #[must_use]
    pub fn get(&self) -> Option<Arc<dyn SnippetExpander>> {
        self.expander.read().clone()
    }
}

impl Default for SnippetExpanderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for SnippetExpanderRegistry {}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ServiceRegistry};

    struct MockExpander {
        called: std::sync::atomic::AtomicBool,
    }

    impl MockExpander {
        fn new() -> Self {
            Self {
                called: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    impl SnippetExpander for MockExpander {
        fn expand(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _buffer_id: BufferId,
            _insert_pos: Position,
            _snippet_body: &str,
        ) {
            self.called
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    impl std::fmt::Debug for MockExpander {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockExpander").finish()
        }
    }

    #[test]
    fn registry_default_is_empty() {
        let reg = SnippetExpanderRegistry::default();
        assert!(reg.get().is_none());
    }

    #[test]
    fn registry_new_is_empty() {
        let reg = SnippetExpanderRegistry::new();
        assert!(reg.get().is_none());
    }

    #[test]
    fn registry_register_and_get() {
        let reg = SnippetExpanderRegistry::new();
        let mock = Arc::new(MockExpander::new());
        reg.register(mock);
        assert!(reg.get().is_some());
    }

    #[test]
    fn registry_overwrite() {
        let reg = SnippetExpanderRegistry::new();
        reg.register(Arc::new(MockExpander::new()));
        reg.register(Arc::new(MockExpander::new()));
        assert!(reg.get().is_some());
    }

    #[test]
    fn registry_get_returns_cloned_arc() {
        let reg = SnippetExpanderRegistry::new();
        let mock: Arc<dyn SnippetExpander> = Arc::new(MockExpander::new());
        reg.register(Arc::clone(&mock));
        let got = reg.get().unwrap();
        assert!(Arc::ptr_eq(&mock, &got));
    }

    #[test]
    fn registry_service_impl() {
        let services = ServiceRegistry::new();
        let reg = Arc::new(SnippetExpanderRegistry::new());
        services.register(reg);
        assert!(services.get::<SnippetExpanderRegistry>().is_some());
    }

    #[test]
    fn registry_debug() {
        let reg = SnippetExpanderRegistry::new();
        let debug = format!("{reg:?}");
        assert!(debug.contains("SnippetExpanderRegistry"));
        assert!(debug.contains("false"));

        reg.register(Arc::new(MockExpander::new()));
        let debug = format!("{reg:?}");
        assert!(debug.contains("true"));
    }

    #[test]
    fn registry_concurrent_access() {
        let reg = Arc::new(SnippetExpanderRegistry::new());

        let reg_clone = Arc::clone(&reg);
        let writer = std::thread::spawn(move || {
            reg_clone.register(Arc::new(MockExpander::new()));
        });
        writer.join().unwrap();

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn trait_is_object_safe() {
        fn _accepts_ref(_: &dyn SnippetExpander) {}
        fn _accepts_box(_: Box<dyn SnippetExpander>) {}
    }

    #[test]
    fn new_and_default_equivalent() {
        let new = SnippetExpanderRegistry::new();
        let default = SnippetExpanderRegistry::default();
        assert!(new.get().is_none());
        assert!(default.get().is_none());
    }
}
