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

/// Default priority assigned to factories added via
/// [`ContentCodecFactoryStore::add_factory`].
///
/// Equivalent to the median tier described in the Phase 6 plan. Modules
/// that want to override a default implementation should use
/// [`ContentCodecFactoryStore::add_factory_with_priority`] with a value
/// greater than [`DEFAULT_FACTORY_PRIORITY`]; modules that want to be
/// used only as a last-resort fallback should use a lower value.
pub const DEFAULT_FACTORY_PRIORITY: u32 = 100;

/// Internal record wrapping a factory with its registration metadata.
#[derive(Clone)]
struct FactoryEntry {
    /// Priority assigned at registration time. Higher wins.
    priority: u32,
    /// Insertion order — used as a stable tiebreaker so equal-priority
    /// factories resolve first-registered-first.
    sequence: u64,
    /// The factory itself.
    factory: Arc<dyn ContentCodecFactory>,
}

/// Store for content codec factories registered by modules during init.
///
/// Modules register their factories here, and the pipeline queries it
/// to find a codec for a given content type.
///
/// # Priority dispatch (`#740` Plan 06 Phase 6)
///
/// Each registered factory carries a `u32` priority. [`find`](Self::find)
/// consults factories in descending priority order and returns the
/// first codec produced. Ties are broken by insertion order (first
/// registered wins), so behaviour is deterministic even when two
/// modules register codecs for the same content type with identical
/// priority.
///
/// Priority is registration metadata — it lives on the factory store,
/// not on the [`ContentCodec`] trait. A parallel priority on
/// `ContentCodec` would create two uncoordinated priority systems;
/// the factory-store approach keeps dispatch control in one place.
#[derive(Default)]
pub struct ContentCodecFactoryStore {
    /// Registered factories, kept sorted by (priority desc, sequence asc).
    factories: RwLock<Vec<FactoryEntry>>,
    /// Monotonic counter used to assign a stable tiebreaker to every
    /// registration. Wrapped in `RwLock` so `add_factory_with_priority`
    /// can run under `&self`.
    next_sequence: RwLock<u64>,
}

impl ContentCodecFactoryStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a factory to the store at the default priority.
    ///
    /// Convenience wrapper around
    /// [`add_factory_with_priority`](Self::add_factory_with_priority) with
    /// `priority = DEFAULT_FACTORY_PRIORITY`. Called by codec modules
    /// that have no opinion about ordering — built-in codecs that want
    /// to claim a higher tier should use `add_factory_with_priority`.
    pub fn add_factory(&self, factory: Arc<dyn ContentCodecFactory>) {
        self.add_factory_with_priority(factory, DEFAULT_FACTORY_PRIORITY);
    }

    /// Add a factory to the store at an explicit priority.
    ///
    /// Higher priority factories are consulted first by
    /// [`find`](Self::find). Ties are broken by registration order
    /// (first-registered wins).
    ///
    /// Called by codec modules during `init()` when they want to
    /// override a default implementation or slot into a specific tier.
    pub fn add_factory_with_priority(&self, factory: Arc<dyn ContentCodecFactory>, priority: u32) {
        let sequence = {
            let mut seq = self.next_sequence.write();
            let n = *seq;
            *seq = n.wrapping_add(1);
            n
        };
        let entry = FactoryEntry {
            priority,
            sequence,
            factory,
        };
        let mut guard = self.factories.write();
        // Insert into position that keeps the vec sorted by
        // (priority desc, sequence asc). Partition_point finds the
        // first entry that compares "after" the new one, which is
        // exactly where we want to insert.
        let pos = guard.partition_point(|existing| match existing.priority.cmp(&entry.priority) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Equal => existing.sequence < entry.sequence,
            std::cmp::Ordering::Less => false,
        });
        guard.insert(pos, entry);
    }

    /// Find and create a codec for the given content type.
    ///
    /// Iterates registered factories in descending priority order
    /// (with first-registered tiebreaking for equal priority) and
    /// returns the first codec produced. The returned codec is
    /// wrapped in `Arc` so mounts can share a single instance cheaply.
    #[must_use]
    pub fn find(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        self.factories
            .read()
            .iter()
            .find_map(|entry| entry.factory.create(content_type))
    }

    /// Take all registered factories.
    ///
    /// Called by bootstrap after all modules have initialized.
    /// This drains the store; subsequent calls return empty vec.
    /// Priority / sequence metadata is discarded — the returned vec
    /// preserves priority order.
    pub fn take_factories(&self) -> Vec<Arc<dyn ContentCodecFactory>> {
        let drained = std::mem::take(&mut *self.factories.write());
        drained.into_iter().map(|entry| entry.factory).collect()
    }

    /// Enumerate every registered factory as `(name, supported_content_types)`.
    ///
    /// Used by the Phase 5 sub-commit 5c `ListAvailableCodecs` RPC so a
    /// client can populate a codec picker UI without draining the store.
    /// Content type strings are owned so the result outlives the
    /// `RwLock` read guard. Entries are returned in priority order.
    #[must_use]
    pub fn available(&self) -> Vec<(&'static str, Vec<String>)> {
        self.factories
            .read()
            .iter()
            .map(|entry| {
                (
                    entry.factory.name(),
                    entry
                        .factory
                        .supported_content_types()
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
