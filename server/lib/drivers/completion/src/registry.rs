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
#[path = "registry_tests.rs"]
mod tests;
