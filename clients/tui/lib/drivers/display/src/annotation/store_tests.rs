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
