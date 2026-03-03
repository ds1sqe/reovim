//! Completion source registry.
//!
//! String-keyed registry for open-ended source extensibility.
//! Any module can register new completion sources without modifying
//! the driver crate.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use reovim_kernel::api::Service;

use crate::CompletionSource;

/// Registry for completion sources.
///
/// Uses string keys (`source.id()`) for open-ended extensibility.
/// Stored in `ServiceRegistry` via the `Service` marker trait.
pub struct CompletionSourceRegistry {
    sources: RwLock<HashMap<&'static str, Arc<dyn CompletionSource>>>,
}

impl Service for CompletionSourceRegistry {}

impl CompletionSourceRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sources: RwLock::new(HashMap::new()),
        }
    }

    /// Register a completion source. Key is `source.id()`.
    ///
    /// If a source with the same ID already exists, it is replaced.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn register(&self, source: Arc<dyn CompletionSource>) {
        self.sources
            .write()
            .expect("CompletionSourceRegistry lock poisoned")
            .insert(source.id(), source);
    }

    /// Get a source by ID.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<Arc<dyn CompletionSource>> {
        self.sources
            .read()
            .expect("CompletionSourceRegistry lock poisoned")
            .get(id)
            .cloned()
    }

    /// List all registered source IDs.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn list(&self) -> Vec<&'static str> {
        self.sources
            .read()
            .expect("CompletionSourceRegistry lock poisoned")
            .keys()
            .copied()
            .collect()
    }

    /// Get all registered sources.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn all(&self) -> Vec<Arc<dyn CompletionSource>> {
        self.sources
            .read()
            .expect("CompletionSourceRegistry lock poisoned")
            .values()
            .cloned()
            .collect()
    }

    /// Number of registered sources.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources
            .read()
            .expect("CompletionSourceRegistry lock poisoned")
            .len()
    }

    /// Whether the registry is empty.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources
            .read()
            .expect("CompletionSourceRegistry lock poisoned")
            .is_empty()
    }
}

impl Default for CompletionSourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::{CompletionContext, CompletionItem, CompletionKind};

    use super::*;

    struct TestSource {
        source_id: &'static str,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CompletionSource for TestSource {
        fn id(&self) -> &'static str {
            self.source_id
        }

        fn priority(&self) -> u16 {
            100
        }

        fn is_available(&self, _ctx: &CompletionContext) -> bool {
            true
        }

        fn complete(&self, _ctx: &CompletionContext) -> Vec<CompletionItem> {
            vec![CompletionItem {
                label: format!("{}_item", self.source_id),
                insert_text: format!("{}_item", self.source_id),
                kind: CompletionKind::Text,
                detail: None,
                documentation: None,
                source_id: self.source_id,
                is_snippet: false,
                sort_priority: 100,
            }]
        }
    }

    #[test]
    fn new_registry_is_empty() {
        let registry = CompletionSourceRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn default_registry_is_empty() {
        let registry = CompletionSourceRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn register_and_get() {
        let registry = CompletionSourceRegistry::new();
        let source: Arc<dyn CompletionSource> = Arc::new(TestSource {
            source_id: "buffer-words",
        });
        registry.register(source);

        let got = registry.get("buffer-words");
        assert!(got.is_some());
        assert_eq!(got.unwrap().id(), "buffer-words");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let registry = CompletionSourceRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn register_multiple() {
        let registry = CompletionSourceRegistry::new();
        registry.register(Arc::new(TestSource {
            source_id: "buffer-words",
        }));
        registry.register(Arc::new(TestSource { source_id: "lsp" }));
        registry.register(Arc::new(TestSource {
            source_id: "snippets",
        }));

        assert_eq!(registry.len(), 3);
        assert!(!registry.is_empty());

        let mut ids = registry.list();
        ids.sort_unstable();
        assert_eq!(ids, vec!["buffer-words", "lsp", "snippets"]);
    }

    #[test]
    fn register_duplicate_replaces() {
        let registry = CompletionSourceRegistry::new();
        registry.register(Arc::new(TestSource { source_id: "lsp" }));
        registry.register(Arc::new(TestSource { source_id: "lsp" }));

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn list_empty_registry() {
        let registry = CompletionSourceRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn all_returns_sources() {
        let registry = CompletionSourceRegistry::new();
        registry.register(Arc::new(TestSource { source_id: "a" }));
        registry.register(Arc::new(TestSource { source_id: "b" }));

        let sources = registry.all();
        assert_eq!(sources.len(), 2);

        let mut ids: Vec<&str> = sources.iter().map(|s| s.id()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn all_empty_registry() {
        let registry = CompletionSourceRegistry::new();
        assert!(registry.all().is_empty());
    }

    fn assert_service(_: &dyn Service) {}

    #[test]
    fn registry_implements_service() {
        let registry = CompletionSourceRegistry::new();
        assert_service(&registry);
    }
}
