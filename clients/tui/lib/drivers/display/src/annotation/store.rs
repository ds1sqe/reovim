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
mod tests {
    use {
        super::*,
        crate::annotation::{AnnotationKind, AnnotationPayload, AnnotationTarget},
    };

    fn make_annotation(line: usize, priority: u8) -> Annotation {
        Annotation::new(
            AnnotationKind::new("test"),
            AnnotationTarget::Line(line),
            priority,
            AnnotationPayload::None,
        )
    }

    fn make_line_number(line: usize, number: usize) -> Annotation {
        Annotation::line_number(line, number)
    }

    // ========================================================================
    // SourceId tests
    // ========================================================================

    #[test]
    fn test_source_id_new() {
        let id = SourceId::new("test.source");
        assert_eq!(id.as_str(), "test.source");
    }

    #[test]
    fn test_source_id_display() {
        let id = SourceId::new("test.source");
        assert_eq!(format!("{id}"), "test.source");
    }

    #[test]
    fn test_source_id_from_str() {
        let id: SourceId = "test.source".into();
        assert_eq!(id.as_str(), "test.source");
    }

    #[test]
    fn test_source_id_from_string() {
        let id: SourceId = String::from("test.source").into();
        assert_eq!(id.as_str(), "test.source");
    }

    #[test]
    fn test_source_id_equality() {
        let id1 = SourceId::new("test");
        let id2 = SourceId::new("test");
        let id3 = SourceId::new("other");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    // ========================================================================
    // AnnotationLayer tests
    // ========================================================================

    #[test]
    fn test_layer_new() {
        let layer = AnnotationLayer::new();
        assert!(layer.is_empty());
        assert_eq!(layer.line_count(), 0);
        assert_eq!(layer.annotation_count(), 0);
    }

    #[test]
    fn test_layer_add() {
        let mut layer = AnnotationLayer::new();
        layer.add(make_annotation(5, 10));
        assert!(!layer.is_empty());
        assert_eq!(layer.line_count(), 1);
        assert_eq!(layer.annotation_count(), 1);
    }

    #[test]
    fn test_layer_add_all() {
        let mut layer = AnnotationLayer::new();
        layer.add_all(vec![make_annotation(5, 10), make_annotation(10, 20)]);
        assert_eq!(layer.line_count(), 2);
        assert_eq!(layer.annotation_count(), 2);
    }

    #[test]
    fn test_layer_add_multiple_per_line() {
        let mut layer = AnnotationLayer::new();
        layer.add(make_annotation(5, 10));
        layer.add(make_annotation(5, 20));
        assert_eq!(layer.line_count(), 1);
        assert_eq!(layer.annotation_count(), 2);
    }

    #[test]
    fn test_layer_clear() {
        let mut layer = AnnotationLayer::new();
        layer.add(make_annotation(5, 10));
        layer.clear();
        assert!(layer.is_empty());
    }

    #[test]
    fn test_layer_replace() {
        let mut layer = AnnotationLayer::new();
        layer.add(make_annotation(5, 10));
        layer.replace(vec![make_annotation(10, 20), make_annotation(15, 30)]);
        assert_eq!(layer.line_count(), 2);
        assert_eq!(layer.annotation_count(), 2);
        assert!(layer.for_line(5).next().is_none());
    }

    #[test]
    fn test_layer_for_line() {
        let mut layer = AnnotationLayer::new();
        layer.add(make_annotation(5, 10));
        layer.add(make_annotation(5, 20));
        layer.add(make_annotation(10, 30));

        assert_eq!(layer.for_line(5).count(), 2);
        assert_eq!(layer.for_line(10).count(), 1);
        assert!(layer.for_line(7).next().is_none());
    }

    #[test]
    fn test_layer_for_range() {
        let mut layer = AnnotationLayer::new();
        layer.add(make_annotation(5, 10));
        layer.add(make_annotation(10, 20));
        layer.add(make_annotation(15, 30));

        // Range 0..10 should include line 5 only (exclusive end)
        #[allow(clippy::needless_collect)]
        let result: Vec<_> = layer.for_range(0..10).collect();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_line(), 5);

        // Range 5..16 should include all three
        assert_eq!(layer.for_range(5..16).count(), 3);

        // Range 20..30 should be empty
        assert!(layer.for_range(20..30).next().is_none());
    }

    #[test]
    fn test_layer_version_increments() {
        let mut layer = AnnotationLayer::new();
        let v0 = layer.version();

        layer.add(make_annotation(5, 10));
        let v1 = layer.version();
        assert!(v1 > v0);

        layer.clear();
        let v2 = layer.version();
        assert!(v2 > v1);

        layer.replace(vec![make_annotation(5, 10)]);
        let v3 = layer.version();
        assert!(v3 > v2);
    }

    // ========================================================================
    // AnnotationStore tests
    // ========================================================================

    #[test]
    fn test_store_new() {
        let store = AnnotationStore::new();
        assert!(store.is_empty());
        assert_eq!(store.source_count(), 0);
    }

    #[test]
    fn test_store_replace_source() {
        let mut store = AnnotationStore::new();
        let source_id = SourceId::new("test");

        store.replace_source(
            source_id.clone(),
            vec![make_annotation(5, 10), make_annotation(10, 20)],
        );

        assert!(!store.is_empty());
        assert_eq!(store.source_count(), 1);
        assert!(store.layer(&source_id).is_some());
    }

    #[test]
    fn test_store_replace_source_atomic() {
        let mut store = AnnotationStore::new();
        let source_id = SourceId::new("test");

        // First add
        store.replace_source(source_id.clone(), vec![make_annotation(5, 10)]);
        assert_eq!(store.query_line(5).len(), 1);

        // Replace with different data
        store.replace_source(source_id, vec![make_annotation(10, 20)]);
        assert!(store.query_line(5).is_empty());
        assert_eq!(store.query_line(10).len(), 1);
    }

    #[test]
    fn test_store_multiple_sources() {
        let mut store = AnnotationStore::new();

        store.replace_source(SourceId::new("source1"), vec![make_annotation(5, 10)]);
        store.replace_source(SourceId::new("source2"), vec![make_annotation(5, 20)]);

        assert_eq!(store.source_count(), 2);

        // Both annotations on line 5
        let line5 = store.query_line(5);
        assert_eq!(line5.len(), 2);
    }

    #[test]
    fn test_store_source_isolation() {
        let mut store = AnnotationStore::new();
        let source1 = SourceId::new("source1");
        let source2 = SourceId::new("source2");

        store.replace_source(source1.clone(), vec![make_annotation(5, 10)]);
        store.replace_source(source2, vec![make_annotation(10, 20)]);

        // Clearing source1 shouldn't affect source2
        store.clear_source(&source1);
        assert!(store.query_line(5).is_empty());
        assert_eq!(store.query_line(10).len(), 1);
    }

    #[test]
    fn test_store_query_priority_sorting() {
        let mut store = AnnotationStore::new();

        // Add annotations with different priorities
        store.replace_source(SourceId::new("low"), vec![make_annotation(5, 10)]);
        store.replace_source(SourceId::new("high"), vec![make_annotation(5, 50)]);
        store.replace_source(SourceId::new("medium"), vec![make_annotation(5, 30)]);

        let result = store.query_line(5);
        assert_eq!(result.len(), 3);

        // Should be sorted highest first
        assert_eq!(result[0].priority, 50);
        assert_eq!(result[1].priority, 30);
        assert_eq!(result[2].priority, 10);
    }

    #[test]
    fn test_store_query_range() {
        let mut store = AnnotationStore::new();

        store.replace_source(
            SourceId::new("test"),
            vec![
                make_annotation(5, 10),
                make_annotation(10, 20),
                make_annotation(15, 30),
            ],
        );

        // Range 0..10 should include line 5 only (exclusive end)
        let result = store.query(0..10);
        assert_eq!(result.len(), 1);

        // Range 0..16 should include lines 5, 10, and 15
        let result = store.query(0..16);
        assert_eq!(result.len(), 3);

        // Range 0..15 should include lines 5 and 10 only (exclusive end)
        let result = store.query(0..15);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_store_query_filtered() {
        let mut store = AnnotationStore::new();

        store.replace_source(
            SourceId::new("line_number"),
            vec![make_line_number(0, 1), make_line_number(1, 2)],
        );
        store.replace_source(
            SourceId::new("diagnostic"),
            vec![Annotation::new(
                AnnotationKind::new("diagnostic.error"),
                AnnotationTarget::Line(0),
                50,
                AnnotationPayload::Severity(0),
            )],
        );

        // Filter to only line numbers
        let result = store.query_filtered(0..10, |kind| kind.name() == "line_number");
        assert_eq!(result.len(), 2);

        // Filter to only diagnostics
        let result = store.query_filtered(0..10, |kind| kind.is_prefix("diagnostic"));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_store_clear_all() {
        let mut store = AnnotationStore::new();

        store.replace_source(SourceId::new("source1"), vec![make_annotation(5, 10)]);
        store.replace_source(SourceId::new("source2"), vec![make_annotation(10, 20)]);

        store.clear_all();
        assert!(store.is_empty());
        assert_eq!(store.source_count(), 0);
    }

    #[test]
    fn test_store_version() {
        let mut store = AnnotationStore::new();
        let v0 = store.version();

        store.replace_source(SourceId::new("test"), vec![make_annotation(5, 10)]);
        let v1 = store.version();
        assert!(v1 > v0);
    }

    // ========================================================================
    // BufferAnnotationStore tests
    // ========================================================================

    #[test]
    fn test_buffer_store_new() {
        let store = BufferAnnotationStore::new();
        assert_eq!(store.buffer_count(), 0);
    }

    #[test]
    fn test_buffer_store_get_or_create() {
        let mut store = BufferAnnotationStore::new();
        let buffer_id = BufferId::new();

        let annotation_store = store.get_or_create(buffer_id);
        annotation_store.replace_source(SourceId::new("test"), vec![make_annotation(5, 10)]);

        assert_eq!(store.buffer_count(), 1);
        assert!(store.contains(buffer_id));
    }

    #[test]
    fn test_buffer_store_get() {
        let mut store = BufferAnnotationStore::new();
        let buffer_id = BufferId::new();

        assert!(store.get(buffer_id).is_none());

        store.get_or_create(buffer_id);
        assert!(store.get(buffer_id).is_some());
    }

    #[test]
    fn test_buffer_store_remove() {
        let mut store = BufferAnnotationStore::new();
        let buffer_id = BufferId::new();

        store.get_or_create(buffer_id);
        assert!(store.contains(buffer_id));

        store.remove(buffer_id);
        assert!(!store.contains(buffer_id));
    }

    #[test]
    fn test_buffer_store_multiple_buffers() {
        let mut store = BufferAnnotationStore::new();
        let buffer1 = BufferId::new();
        let buffer2 = BufferId::new();

        store.get_or_create(buffer1);
        store.get_or_create(buffer2);

        assert_eq!(store.buffer_count(), 2);
    }

    #[test]
    fn test_buffer_store_clear() {
        let mut store = BufferAnnotationStore::new();

        store.get_or_create(BufferId::new());
        store.get_or_create(BufferId::new());

        store.clear();
        assert_eq!(store.buffer_count(), 0);
    }

    #[test]
    fn test_buffer_store_isolation() {
        let mut store = BufferAnnotationStore::new();
        let buffer1 = BufferId::new();
        let buffer2 = BufferId::new();

        // Add annotations to buffer1
        store
            .get_or_create(buffer1)
            .replace_source(SourceId::new("test"), vec![make_annotation(5, 10)]);

        // buffer2 should be empty
        let store2 = store.get_or_create(buffer2);
        assert!(store2.is_empty());
    }

    // ========================================================================
    // Additional coverage tests
    // ========================================================================

    #[test]
    fn test_store_layer_mut() {
        let mut store = AnnotationStore::new();
        let source_id = SourceId::new("test");

        // layer_mut should create a new layer if it doesn't exist
        let layer = store.layer_mut(&source_id);
        layer.add(make_annotation(5, 10));

        assert_eq!(store.source_count(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn test_buffer_store_get_mut() {
        let mut store = BufferAnnotationStore::new();
        let buffer_id = BufferId::new();

        // get_mut returns None for non-existent buffer
        assert!(store.get_mut(buffer_id).is_none());

        // Create the store first
        store.get_or_create(buffer_id);

        // Now get_mut should return Some
        let annotation_store = store.get_mut(buffer_id);
        assert!(annotation_store.is_some());

        // Verify we can mutate through it
        let annotation_store = annotation_store.unwrap();
        annotation_store.replace_source(SourceId::new("test"), vec![make_annotation(0, 10)]);
        assert!(!annotation_store.is_empty());
    }

    #[test]
    fn test_clear_source_nonexistent() {
        let mut store = AnnotationStore::new();
        // clear_source with a source_id that was never added (line 232 else branch)
        store.clear_source(&SourceId::new("nonexistent"));
        // Should be a no-op without panic
        assert!(store.is_empty());
    }

    #[test]
    fn test_buffer_store_buffer_ids() {
        let mut store = BufferAnnotationStore::new();
        let buffer1 = BufferId::new();
        let buffer2 = BufferId::new();

        store.get_or_create(buffer1);
        store.get_or_create(buffer2);

        let ids: Vec<_> = store.buffer_ids().collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&&buffer1));
        assert!(ids.contains(&&buffer2));
    }
}
