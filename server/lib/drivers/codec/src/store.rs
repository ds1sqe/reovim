//! Content codec stores for module self-registration.
//!
//! Provides [`ContentCodecFactoryStore`] and [`ContentClassifierStore`],
//! registries where codec modules register their factories and classifiers
//! during `init()`. This follows the same pattern as [`SyntaxFactoryStore`].
//!
//! # Self-Registration Pattern
//!
//! ```ignore
//! // In codec-utf8 module's init():
//! impl Module for CodecUtf8Module {
//!     fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
//!         let factory_store = ctx.services.get_or_create::<ContentCodecFactoryStore>();
//!         factory_store.add_factory(Arc::new(Utf8CodecFactory::new()));
//!
//!         let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
//!         classifier_store.add(Arc::new(Utf8Classifier::new()));
//!
//!         ProbeResult::Success
//!     }
//! }
//! ```

use std::sync::Arc;

use {parking_lot::RwLock, reovim_kernel::api::v1::Service};

use crate::{ContentClassifier, ContentCodec, ContentCodecFactory, ContentType};

/// Store for content codec factories registered by modules during init.
///
/// Modules register their factories here, and the pipeline queries it
/// to find a codec for a given content type.
#[derive(Default)]
pub struct ContentCodecFactoryStore {
    /// Registered factories.
    factories: RwLock<Vec<Arc<dyn ContentCodecFactory>>>,
}

impl ContentCodecFactoryStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a factory to the store.
    ///
    /// Called by codec modules during `init()`.
    pub fn add_factory(&self, factory: Arc<dyn ContentCodecFactory>) {
        self.factories.write().push(factory);
    }

    /// Find and create a codec for the given content type.
    ///
    /// Iterates registered factories, calling `create()` on each.
    /// Returns the first `Some` result. Post-Phase 5 sub-commit 5b the
    /// codec is wrapped in `Arc` so mounts can share a single instance
    /// cheaply.
    #[must_use]
    pub fn find(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        self.factories
            .read()
            .iter()
            .find_map(|f| f.create(content_type))
    }

    /// Take all registered factories.
    ///
    /// Called by bootstrap after all modules have initialized.
    /// This drains the store; subsequent calls return empty vec.
    pub fn take_factories(&self) -> Vec<Arc<dyn ContentCodecFactory>> {
        std::mem::take(&mut *self.factories.write())
    }

    /// Enumerate every registered factory as `(name, supported_content_types)`.
    ///
    /// Used by the Phase 5 sub-commit 5c `ListAvailableCodecs` RPC so a
    /// client can populate a codec picker UI without draining the store.
    /// Content type strings are owned so the result outlives the
    /// `RwLock` read guard.
    #[must_use]
    pub fn available(&self) -> Vec<(&'static str, Vec<String>)> {
        self.factories
            .read()
            .iter()
            .map(|f| {
                (
                    f.name(),
                    f.supported_content_types()
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                )
            })
            .collect()
    }

    /// Get the number of registered factories.
    #[must_use]
    pub fn len(&self) -> usize {
        self.factories.read().len()
    }

    /// Check if no factories are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.factories.read().is_empty()
    }
}

impl Service for ContentCodecFactoryStore {}

impl std::fmt::Debug for ContentCodecFactoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentCodecFactoryStore")
            .field("count", &self.len())
            .finish()
    }
}

/// Store for content classifiers registered by modules during init.
///
/// Classifiers are priority-sorted (highest first). The first classifier
/// to return `Some` wins.
#[derive(Default)]
pub struct ContentClassifierStore {
    /// Registered classifiers.
    classifiers: RwLock<Vec<Arc<dyn ContentClassifier>>>,
}

impl ContentClassifierStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a classifier to the store.
    ///
    /// Called by codec modules during `init()`.
    pub fn add(&self, classifier: Arc<dyn ContentClassifier>) {
        self.classifiers.write().push(classifier);
    }

    /// Classify raw bytes to determine content type.
    ///
    /// Sorts classifiers by priority (highest first), then iterates
    /// calling `classify()` on each. Returns the first `Some` result.
    ///
    /// Returns `None` if no classifier recognizes the content.
    /// The caller should treat `None` as UTF-8 fallback.
    #[must_use]
    pub fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType> {
        let mut sorted: Vec<_> = self.classifiers.read().iter().cloned().collect();
        sorted.sort_by_key(|c| std::cmp::Reverse(c.priority()));

        for classifier in &sorted {
            if let Some(content_type) = classifier.classify(raw, path) {
                return Some(content_type);
            }
        }
        None
    }

    /// Take all registered classifiers.
    pub fn take_classifiers(&self) -> Vec<Arc<dyn ContentClassifier>> {
        std::mem::take(&mut *self.classifiers.write())
    }

    /// Get the number of registered classifiers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.classifiers.read().len()
    }

    /// Check if no classifiers are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.classifiers.read().is_empty()
    }
}

impl Service for ContentClassifierStore {}

impl std::fmt::Debug for ContentClassifierStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentClassifierStore")
            .field("count", &self.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
