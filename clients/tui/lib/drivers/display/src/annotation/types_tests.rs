use super::*;

// ========================================================================
// AnnotationKind tests
// ========================================================================

#[test]
fn test_annotation_kind_new() {
    let kind = AnnotationKind::new("line_number");
    assert_eq!(kind.name(), "line_number");
}

#[test]
fn test_annotation_kind_from_string() {
    let kind = AnnotationKind::new(String::from("diagnostic.error"));
    assert_eq!(kind.name(), "diagnostic.error");
}

#[test]
fn test_annotation_kind_namespace_with_dot() {
    let kind = AnnotationKind::new("diagnostic.error");
    assert_eq!(kind.namespace(), Some("diagnostic"));
}

#[test]
fn test_annotation_kind_namespace_without_dot() {
    let kind = AnnotationKind::new("line_number");
    assert_eq!(kind.namespace(), None);
}

#[test]
fn test_annotation_kind_namespace_multiple_dots() {
    let kind = AnnotationKind::new("diagnostic.error.severe");
    // Only the first dot matters for namespace extraction
    assert_eq!(kind.namespace(), Some("diagnostic"));
}

#[test]
fn test_annotation_kind_namespace_empty() {
    let kind = AnnotationKind::new("");
    assert_eq!(kind.namespace(), None);
}

#[test]
fn test_annotation_kind_is_prefix_exact_namespace() {
    let kind = AnnotationKind::new("diagnostic.error");
    assert!(kind.is_prefix("diagnostic"));
}

#[test]
fn test_annotation_kind_is_prefix_non_matching() {
    let kind = AnnotationKind::new("diagnostic.error");
    assert!(!kind.is_prefix("git"));
}

#[test]
fn test_annotation_kind_is_prefix_partial_match_rejected() {
    // "diag" should NOT match "diagnostic.error"
    let kind = AnnotationKind::new("diagnostic.error");
    assert!(!kind.is_prefix("diag"));
}

#[test]
fn test_annotation_kind_is_prefix_exact_match() {
    // Exact match should work
    let kind = AnnotationKind::new("line_number");
    assert!(kind.is_prefix("line_number"));
}

#[test]
fn test_annotation_kind_is_prefix_underscore_boundary() {
    // "diagnostic_error" should NOT match prefix "diagnostic"
    // (underscore is not a namespace separator)
    let kind = AnnotationKind::new("diagnostic_error");
    assert!(!kind.is_prefix("diagnostic"));
}

#[test]
fn test_annotation_kind_equality() {
    let kind1 = AnnotationKind::new("line_number");
    let kind2 = AnnotationKind::new("line_number");
    let kind3 = AnnotationKind::new("diagnostic.error");
    assert_eq!(kind1, kind2);
    assert_ne!(kind1, kind3);
}

#[test]
fn test_annotation_kind_display() {
    let kind = AnnotationKind::new("diagnostic.error");
    assert_eq!(format!("{kind}"), "diagnostic.error");
}

// ========================================================================
// AnnotationTarget tests
// ========================================================================

#[test]
fn test_target_line_affects_line() {
    let target = AnnotationTarget::Line(5);
    assert!(!target.affects_line(4));
    assert!(target.affects_line(5));
    assert!(!target.affects_line(6));
}

#[test]
fn test_target_range_affects_line() {
    let target = AnnotationTarget::range(5, 10);
    assert!(!target.affects_line(4));
    assert!(target.affects_line(5));
    assert!(target.affects_line(7));
    assert!(target.affects_line(10));
    assert!(!target.affects_line(11));
}

#[test]
fn test_target_range_single_line() {
    // Range where start == end
    let target = AnnotationTarget::range(5, 5);
    assert!(!target.affects_line(4));
    assert!(target.affects_line(5));
    assert!(!target.affects_line(6));
}

#[test]
fn test_target_point_affects_line() {
    let target = AnnotationTarget::point(5, 10);
    assert!(!target.affects_line(4));
    assert!(target.affects_line(5));
    assert!(!target.affects_line(6));
}

#[test]
fn test_target_buffer_affects_all_lines() {
    let target = AnnotationTarget::Buffer;
    assert!(target.affects_line(0));
    assert!(target.affects_line(100));
    assert!(target.affects_line(1_000_000));
}

#[test]
fn test_target_start_line() {
    assert_eq!(AnnotationTarget::Line(5).start_line(), 5);
    assert_eq!(AnnotationTarget::range(3, 10).start_line(), 3);
    assert_eq!(AnnotationTarget::point(7, 15).start_line(), 7);
    assert_eq!(AnnotationTarget::Buffer.start_line(), 0);
}

#[test]
fn test_target_end_line() {
    assert_eq!(AnnotationTarget::Line(5).end_line(), 5);
    assert_eq!(AnnotationTarget::range(3, 10).end_line(), 10);
    assert_eq!(AnnotationTarget::point(7, 15).end_line(), 7);
    assert_eq!(AnnotationTarget::Buffer.end_line(), usize::MAX);
}

#[test]
fn test_target_is_single_line() {
    assert!(AnnotationTarget::Line(5).is_single_line());
    assert!(AnnotationTarget::point(5, 10).is_single_line());
    assert!(!AnnotationTarget::range(5, 10).is_single_line());
    assert!(!AnnotationTarget::Buffer.is_single_line());
}

// ========================================================================
// AnnotationPayload tests
// ========================================================================

#[test]
fn test_payload_number() {
    let payload = AnnotationPayload::Number(42);
    assert_eq!(payload.as_number(), Some(42));
    assert_eq!(payload.as_text(), None);
    assert!(!payload.is_none());
}

#[test]
fn test_payload_number_zero() {
    let payload = AnnotationPayload::Number(0);
    assert_eq!(payload.as_number(), Some(0));
}

#[test]
fn test_payload_number_max() {
    let payload = AnnotationPayload::Number(usize::MAX);
    assert_eq!(payload.as_number(), Some(usize::MAX));
}

#[test]
fn test_payload_text() {
    let payload = AnnotationPayload::text("hello");
    assert_eq!(payload.as_text(), Some("hello"));
    assert_eq!(payload.as_number(), None);
}

#[test]
fn test_payload_text_empty() {
    let payload = AnnotationPayload::text("");
    assert_eq!(payload.as_text(), Some(""));
}

#[test]
fn test_payload_severity() {
    let payload = AnnotationPayload::Severity(0);
    assert_eq!(payload.as_severity(), Some(0));

    let payload = AnnotationPayload::Severity(3);
    assert_eq!(payload.as_severity(), Some(3));
}

#[test]
fn test_payload_state() {
    let payload = AnnotationPayload::State(true);
    assert_eq!(payload.as_state(), Some(true));

    let payload = AnnotationPayload::State(false);
    assert_eq!(payload.as_state(), Some(false));
}

#[test]
fn test_payload_none() {
    let payload = AnnotationPayload::None;
    assert!(payload.is_none());
    assert_eq!(payload.as_number(), None);
    assert_eq!(payload.as_text(), None);
}

#[test]
fn test_payload_default() {
    let payload = AnnotationPayload::default();
    assert!(payload.is_none());
}

#[test]
fn test_payload_equality() {
    assert_eq!(AnnotationPayload::Number(5), AnnotationPayload::Number(5));
    assert_ne!(AnnotationPayload::Number(5), AnnotationPayload::Number(6));
    assert_eq!(AnnotationPayload::text("hello"), AnnotationPayload::text("hello"));
}

// ========================================================================
// Annotation tests
// ========================================================================

#[test]
fn test_annotation_new() {
    let annotation = Annotation::new(
        AnnotationKind::new("test"),
        AnnotationTarget::Line(5),
        10,
        AnnotationPayload::None,
    );
    assert_eq!(annotation.kind.name(), "test");
    assert_eq!(annotation.priority, 10);
}

#[test]
fn test_annotation_line_number_convenience() {
    let annotation = Annotation::line_number(5, 6);
    assert_eq!(annotation.kind.name(), "line_number");
    assert!(matches!(annotation.target, AnnotationTarget::Line(5)));
    assert_eq!(annotation.priority, 0);
    assert_eq!(annotation.payload.as_number(), Some(6));
}

#[test]
fn test_annotation_affects_line() {
    let annotation = Annotation::line_number(5, 6);
    assert!(!annotation.affects_line(4));
    assert!(annotation.affects_line(5));
    assert!(!annotation.affects_line(6));
}

#[test]
fn test_annotation_start_end_line() {
    let annotation = Annotation::new(
        AnnotationKind::new("test"),
        AnnotationTarget::range(5, 10),
        0,
        AnnotationPayload::None,
    );
    assert_eq!(annotation.start_line(), 5);
    assert_eq!(annotation.end_line(), 10);
}

#[test]
fn test_annotation_clone() {
    let annotation = Annotation::line_number(5, 6);
    let cloned = annotation.clone();
    assert_eq!(annotation.kind, cloned.kind);
    assert_eq!(annotation.priority, cloned.priority);
}

// ========================================================================
// Additional coverage tests
// ========================================================================

#[test]
fn test_payload_as_severity_none_for_non_severity() {
    // Covers line 297: _ => None branch of as_severity
    assert_eq!(AnnotationPayload::Number(42).as_severity(), None);
    assert_eq!(AnnotationPayload::text("hello").as_severity(), None);
    assert_eq!(AnnotationPayload::State(true).as_severity(), None);
    assert_eq!(AnnotationPayload::None.as_severity(), None);
}

#[test]
fn test_payload_as_state_none_for_non_state() {
    // Covers line 306: _ => None branch of as_state
    assert_eq!(AnnotationPayload::Number(42).as_state(), None);
    assert_eq!(AnnotationPayload::text("hello").as_state(), None);
    assert_eq!(AnnotationPayload::Severity(0).as_state(), None);
    assert_eq!(AnnotationPayload::None.as_state(), None);
}
