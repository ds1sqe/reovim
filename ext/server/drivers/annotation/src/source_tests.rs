use {
    super::*,
    crate::{AnnotationPayload, AnnotationTarget},
};

// Mock source for testing
struct MockSource {
    annotations: Vec<Annotation>,
}

impl MockSource {
    fn new(annotations: Vec<Annotation>) -> Self {
        Self { annotations }
    }

    fn empty() -> Self {
        Self {
            annotations: Vec::new(),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl AnnotationSource for MockSource {
    fn id(&self) -> &'static str {
        "test.mock"
    }

    fn provides(&self) -> Vec<AnnotationKind> {
        vec![AnnotationKind::new("test")]
    }

    fn annotations(
        &self,
        _buffer_id: BufferId,
        range: Range<usize>,
        _context: &AnnotationContext,
    ) -> Vec<Annotation> {
        self.annotations
            .iter()
            .filter(|a| {
                let line = a.start_line();
                range.contains(&line)
            })
            .cloned()
            .collect()
    }
}

#[test]
fn test_annotation_context_new() {
    let ctx = AnnotationContext::new(100, 50, "NORMAL");
    assert_eq!(ctx.total_lines, 100);
    assert_eq!(ctx.cursor_line, 50);
    assert_eq!(ctx.mode, "NORMAL");
}

#[test]
fn test_annotation_context_minimal() {
    let ctx = AnnotationContext::minimal(100);
    assert_eq!(ctx.total_lines, 100);
    assert_eq!(ctx.cursor_line, 0);
    assert!(ctx.mode.is_empty());
}

#[test]
fn test_annotation_context_default() {
    let ctx = AnnotationContext::default();
    assert_eq!(ctx.total_lines, 0);
    assert_eq!(ctx.cursor_line, 0);
    assert!(ctx.mode.is_empty());
}

#[test]
fn test_mock_source_id() {
    let source = MockSource::empty();
    assert_eq!(source.id(), "test.mock");
}

#[test]
fn test_mock_source_provides() {
    let source = MockSource::empty();
    let kinds = source.provides();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0].name(), "test");
}

#[test]
fn test_mock_source_annotations_empty() {
    let source = MockSource::empty();
    let ctx = AnnotationContext::minimal(10);
    let annotations = source.annotations(BufferId::new(), 0..10, &ctx);
    assert!(annotations.is_empty());
}

#[test]
fn test_mock_source_annotations_filtered() {
    let annotations = vec![
        Annotation::new(
            AnnotationKind::new("test"),
            AnnotationTarget::Line(5),
            0,
            AnnotationPayload::None,
        ),
        Annotation::new(
            AnnotationKind::new("test"),
            AnnotationTarget::Line(15),
            0,
            AnnotationPayload::None,
        ),
    ];
    let source = MockSource::new(annotations);
    let ctx = AnnotationContext::minimal(20);

    // Query range 0..10 should only return line 5
    let result = source.annotations(BufferId::new(), 0..10, &ctx);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].start_line(), 5);

    // Query range 10..20 should only return line 15
    let result = source.annotations(BufferId::new(), 10..20, &ctx);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].start_line(), 15);

    // Query range 0..20 should return both
    let result = source.annotations(BufferId::new(), 0..20, &ctx);
    assert_eq!(result.len(), 2);
}

#[test]
fn test_source_default_has_annotations() {
    let source = MockSource::empty();
    assert!(source.has_annotations(BufferId::new()));
}

#[test]
fn test_source_is_object_safe() {
    let source = MockSource::empty();
    let _: &dyn AnnotationSource = &source;
}

#[test]
fn test_source_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MockSource>();
}

// ========================================================================
// Additional coverage tests
// ========================================================================

#[test]
fn test_source_default_on_buffer_change() {
    let source = MockSource::empty();
    // Should not panic - default impl does nothing
    source.on_buffer_change(BufferId::new());
}

#[test]
fn test_annotation_context_with_line_number_mode() {
    use crate::LineNumberMode;

    let ctx = AnnotationContext::with_line_number_mode(100, 50, "NORMAL", LineNumberMode::Absolute);
    assert_eq!(ctx.total_lines, 100);
    assert_eq!(ctx.cursor_line, 50);
    assert_eq!(ctx.mode, "NORMAL");
    assert_eq!(ctx.line_number_mode(), Some(LineNumberMode::Absolute));
}
