//! Annotation storage with efficient line-based querying.
//!
//! Provides per-buffer annotation storage with source isolation and
//! priority-based conflict resolution.
//!
//! # Architecture
//!
//! ```text
//! BufferAnnotationStore (global)
//! └── HashMap<BufferId, AnnotationStore>
//!     └── AnnotationStore (per-buffer)
//!         └── HashMap<SourceId, AnnotationLayer>
//!             └── AnnotationLayer (per-source)
//!                 └── BTreeMap<line, SmallVec<Annotation>>
//! ```
//!
//! # Design Decisions
//!
//! - **Source isolation**: Each source owns its annotations separately,
//!   enabling atomic replacement without affecting other sources.
//! - **`BTreeMap` for lines**: O(log N) range queries for viewport rendering.
//! - **`SmallVec`**: Most lines have 0-2 annotations; avoid heap allocation.
//! - **Version tracking**: Enables cache invalidation for presenters.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    ops::Range,
    sync::Arc,
};

use {reovim_kernel::api::v1::BufferId, smallvec::SmallVec};

use super::types::Annotation;

/// Source identifier (matches `AnnotationSource::id()`).
///
/// Used to isolate annotations from different sources within a buffer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceId(Arc<str>);

impl SourceId {
    /// Create a new source ID.
    #[must_use]
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }

    /// Get the string value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&'static str> for SourceId {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

impl From<String> for SourceId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Single source's annotations (isolated ownership).
///
/// Each source (line numbers, diagnostics, git, etc.) has its own layer.
/// This enables:
/// - Atomic replacement of a source's annotations
/// - Independent version tracking per source
/// - No interference between sources
#[derive(Debug, Default)]
pub struct AnnotationLayer {
    /// Primary index: line → annotations
    /// `BTreeMap` for O(log N) range queries
    by_line: BTreeMap<usize, SmallVec<[Annotation; 2]>>,
    /// Version for cache invalidation (incremented on changes)
    version: u64,
}

impl AnnotationLayer {
    /// Create a new empty layer.
    ///
    /// Note: Cannot be const because `BTreeMap::new()` is not const.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new() -> Self {
        Self {
            by_line: BTreeMap::new(),
            version: 0,
        }
    }

    /// Add an annotation to this layer.
    pub fn add(&mut self, annotation: Annotation) {
        let line = annotation.start_line();
        self.by_line.entry(line).or_default().push(annotation);
        self.version = self.version.wrapping_add(1);
    }

    /// Add multiple annotations to this layer.
    pub fn add_all(&mut self, annotations: impl IntoIterator<Item = Annotation>) {
        for annotation in annotations {
            let line = annotation.start_line();
            self.by_line.entry(line).or_default().push(annotation);
        }
        self.version = self.version.wrapping_add(1);
    }

    /// Clear all annotations from this layer.
    pub fn clear(&mut self) {
        self.by_line.clear();
        self.version = self.version.wrapping_add(1);
    }

    /// Replace all annotations (atomic update).
    ///
    /// This is more efficient than clear + `add_all` for bulk updates.
    pub fn replace(&mut self, annotations: Vec<Annotation>) {
        self.by_line.clear();
        for annotation in annotations {
            let line = annotation.start_line();
            self.by_line.entry(line).or_default().push(annotation);
        }
        self.version = self.version.wrapping_add(1);
    }

    /// Get annotations for a single line.
    pub fn for_line(&self, line: usize) -> impl Iterator<Item = &Annotation> {
        self.by_line.get(&line).into_iter().flatten()
    }

    /// Get annotations for a line range (exclusive end).
    ///
    /// Returns annotations for lines in `[start, end)`.
    pub fn for_range(&self, range: Range<usize>) -> impl Iterator<Item = &Annotation> {
        self.by_line
            .range(range)
            .flat_map(|(_, annotations)| annotations.iter())
    }

    /// Check if this layer has any annotations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_line.is_empty()
    }

    /// Get the number of lines with annotations.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.by_line.len()
    }

    /// Get the total number of annotations.
    #[must_use]
    pub fn annotation_count(&self) -> usize {
        self.by_line.values().map(SmallVec::len).sum()
    }

    /// Get the version number (for cache invalidation).
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }
}

/// Per-buffer annotation storage.
///
/// Manages annotations from multiple sources with efficient querying.
///
/// # Source Isolation
///
/// Each source's annotations are stored in a separate [`AnnotationLayer`].
/// This enables atomic replacement of a source's data without affecting others.
///
/// # Query Performance
///
/// - Single line query: O(sources × log N)
/// - Range query: O(sources × (log N + M)) where M is matching annotations
#[derive(Debug, Default)]
pub struct AnnotationStore {
    /// Source ID → Layer
    layers: HashMap<SourceId, AnnotationLayer>,
    /// Aggregate version (sum of all layer versions, for cache invalidation)
    aggregate_version: u64,
}

impl AnnotationStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: HashMap::new(),
            aggregate_version: 0,
        }
    }

    /// Get or create a layer for a source.
    pub fn layer_mut(&mut self, source_id: &SourceId) -> &mut AnnotationLayer {
        self.layers.entry(source_id.clone()).or_default()
    }

    /// Get a layer for a source (if it exists).
    #[must_use]
    pub fn layer(&self, source_id: &SourceId) -> Option<&AnnotationLayer> {
        self.layers.get(source_id)
    }

    /// Replace all annotations from a source (atomic update).
    ///
    /// This is the primary method for updating annotations. It:
    /// 1. Clears the source's existing annotations
    /// 2. Adds all new annotations
    /// 3. Updates version for cache invalidation
    pub fn replace_source(&mut self, source_id: SourceId, annotations: Vec<Annotation>) {
        let layer = self.layers.entry(source_id).or_default();
        layer.replace(annotations);
        self.update_aggregate_version();
    }

    /// Clear annotations from a source.
    pub fn clear_source(&mut self, source_id: &SourceId) {
        if let Some(layer) = self.layers.get_mut(source_id) {
            layer.clear();
            self.update_aggregate_version();
        }
    }

    /// Clear all annotations from all sources.
    pub fn clear_all(&mut self) {
        self.layers.clear();
        self.aggregate_version = self.aggregate_version.wrapping_add(1);
    }

    /// Query annotations for a line range, sorted by priority (highest first).
    ///
    /// Collects annotations from all sources and sorts by priority.
    #[must_use]
    pub fn query(&self, range: Range<usize>) -> Vec<&Annotation> {
        let mut result: Vec<&Annotation> = self
            .layers
            .values()
            .flat_map(|layer| layer.for_range(range.clone()))
            .collect();

        // Sort by priority (highest first for conflict resolution)
        result.sort_by_key(|a| Reverse(a.priority));
        result
    }

    /// Query annotations for a line range with a filter.
    ///
    /// Only returns annotations where `filter(&annotation.kind)` returns true.
    #[must_use]
    pub fn query_filtered<F>(&self, range: Range<usize>, filter: F) -> Vec<&Annotation>
    where
        F: Fn(&super::types::AnnotationKind) -> bool,
    {
        let mut result: Vec<&Annotation> = self
            .layers
            .values()
            .flat_map(|layer| layer.for_range(range.clone()))
            .filter(|a| filter(&a.kind))
            .collect();

        result.sort_by_key(|a| Reverse(a.priority));
        result
    }

    /// Query annotations for a single line, sorted by priority.
    #[must_use]
    pub fn query_line(&self, line: usize) -> Vec<&Annotation> {
        self.query(line..line + 1)
    }

    /// Check if any source has annotations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.values().all(AnnotationLayer::is_empty)
    }

    /// Get the number of sources with annotations.
    #[must_use]
    pub fn source_count(&self) -> usize {
        self.layers.len()
    }

    /// Get the aggregate version (for cache invalidation).
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.aggregate_version
    }

    /// Get all source IDs.
    pub fn source_ids(&self) -> impl Iterator<Item = &SourceId> {
        self.layers.keys()
    }

    fn update_aggregate_version(&mut self) {
        self.aggregate_version = self.layers.values().map(AnnotationLayer::version).sum();
    }
}

/// Global store mapping buffer IDs to their annotation stores.
///
/// This is the top-level container for all annotation data.
#[derive(Debug, Default)]
pub struct BufferAnnotationStore {
    /// Buffer ID → Annotation store
    buffers: HashMap<BufferId, AnnotationStore>,
}

impl BufferAnnotationStore {
    /// Create a new empty buffer store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    /// Get or create an annotation store for a buffer.
    pub fn get_or_create(&mut self, buffer_id: BufferId) -> &mut AnnotationStore {
        self.buffers.entry(buffer_id).or_default()
    }

    /// Get an annotation store for a buffer (if it exists).
    #[must_use]
    pub fn get(&self, buffer_id: BufferId) -> Option<&AnnotationStore> {
        self.buffers.get(&buffer_id)
    }

    /// Get a mutable annotation store for a buffer (if it exists).
    pub fn get_mut(&mut self, buffer_id: BufferId) -> Option<&mut AnnotationStore> {
        self.buffers.get_mut(&buffer_id)
    }

    /// Remove an annotation store for a buffer.
    ///
    /// Call this when a buffer is closed to free memory.
    pub fn remove(&mut self, buffer_id: BufferId) -> Option<AnnotationStore> {
        self.buffers.remove(&buffer_id)
    }

    /// Check if a buffer has an annotation store.
    #[must_use]
    pub fn contains(&self, buffer_id: BufferId) -> bool {
        self.buffers.contains_key(&buffer_id)
    }

    /// Get the number of buffers with annotation stores.
    #[must_use]
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Get all buffer IDs.
    pub fn buffer_ids(&self) -> impl Iterator<Item = &BufferId> {
        self.buffers.keys()
    }

    /// Clear all annotation stores.
    pub fn clear(&mut self) {
        self.buffers.clear();
    }
}

// Ensure types are Send + Sync for async compatibility
const _: () = {
    #[cfg_attr(coverage_nightly, coverage(off))]
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SourceId>();
    assert_send_sync::<AnnotationLayer>();
    assert_send_sync::<AnnotationStore>();
    assert_send_sync::<BufferAnnotationStore>();
};

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
