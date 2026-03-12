use super::*;

// ========================================================================
// LineNumberSource tests
// ========================================================================

#[test]
fn test_source_id() {
    let source = LineNumberSource::absolute();
    assert_eq!(source.id(), "builtin.line_number");
}

#[test]
fn test_source_provides() {
    let source = LineNumberSource::absolute();
    let kinds = source.provides();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0].name(), "line_number");
}

#[test]
fn test_source_absolute_mode() {
    let source = LineNumberSource::absolute();
    let context = AnnotationContext::new(10, 5, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 0..3, &context);

    assert_eq!(annotations.len(), 3);
    assert_eq!(annotations[0].payload.as_number(), Some(1));
    assert_eq!(annotations[1].payload.as_number(), Some(2));
    assert_eq!(annotations[2].payload.as_number(), Some(3));
}

#[test]
fn test_source_relative_mode() {
    let source = LineNumberSource::relative();
    let context = AnnotationContext::new(10, 5, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 3..8, &context);

    assert_eq!(annotations.len(), 5);
    // Line 3: |3-5| = 2
    assert_eq!(annotations[0].payload.as_number(), Some(2));
    // Line 4: |4-5| = 1
    assert_eq!(annotations[1].payload.as_number(), Some(1));
    // Line 5: |5-5| = 0
    assert_eq!(annotations[2].payload.as_number(), Some(0));
    // Line 6: |6-5| = 1
    assert_eq!(annotations[3].payload.as_number(), Some(1));
    // Line 7: |7-5| = 2
    assert_eq!(annotations[4].payload.as_number(), Some(2));
}

#[test]
fn test_source_hybrid_mode() {
    let source = LineNumberSource::hybrid();
    let context = AnnotationContext::new(10, 5, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 3..8, &context);

    assert_eq!(annotations.len(), 5);
    // Line 3: relative = 2
    assert_eq!(annotations[0].payload.as_number(), Some(2));
    // Line 4: relative = 1
    assert_eq!(annotations[1].payload.as_number(), Some(1));
    // Line 5: cursor line, absolute = 6
    assert_eq!(annotations[2].payload.as_number(), Some(6));
    // Line 6: relative = 1
    assert_eq!(annotations[3].payload.as_number(), Some(1));
    // Line 7: relative = 2
    assert_eq!(annotations[4].payload.as_number(), Some(2));
}

#[test]
fn test_source_none_mode() {
    let source = LineNumberSource::new(LineNumberMode::None);
    let context = AnnotationContext::new(10, 0, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 0..10, &context);

    assert!(annotations.is_empty());
}

#[test]
fn test_source_has_annotations() {
    let source = LineNumberSource::absolute();
    assert!(source.has_annotations(BufferId::new()));

    let source_none = LineNumberSource::new(LineNumberMode::None);
    assert!(!source_none.has_annotations(BufferId::new()));
}

#[test]
fn test_source_uses_context_mode_when_provided() {
    // Source stored with Absolute mode
    let source = LineNumberSource::absolute();

    // Context provides Relative mode - should override stored mode
    let context =
        AnnotationContext::with_line_number_mode(10, 5, "normal", LineNumberMode::Relative);
    let annotations = source.annotations(BufferId::new(), 4..7, &context);

    assert_eq!(annotations.len(), 3);
    // Line 4: |4-5| = 1 (relative, not absolute 5)
    assert_eq!(annotations[0].payload.as_number(), Some(1));
    // Line 5: |5-5| = 0 (cursor line in relative)
    assert_eq!(annotations[1].payload.as_number(), Some(0));
    // Line 6: |6-5| = 1 (relative, not absolute 7)
    assert_eq!(annotations[2].payload.as_number(), Some(1));
}

#[test]
fn test_source_uses_stored_mode_when_context_mode_none() {
    // Source stored with Absolute mode
    let source = LineNumberSource::absolute();

    // Context without line_number_mode - should use stored mode
    let context = AnnotationContext::new(10, 5, "normal");
    let annotations = source.annotations(BufferId::new(), 0..3, &context);

    assert_eq!(annotations.len(), 3);
    // Should use Absolute mode (stored)
    assert_eq!(annotations[0].payload.as_number(), Some(1));
    assert_eq!(annotations[1].payload.as_number(), Some(2));
    assert_eq!(annotations[2].payload.as_number(), Some(3));
}

#[test]
fn test_source_context_mode_none_returns_empty() {
    // Source stored with Absolute mode
    let source = LineNumberSource::absolute();

    // Context provides None mode - should return empty
    let context = AnnotationContext::with_line_number_mode(10, 5, "normal", LineNumberMode::None);
    let annotations = source.annotations(BufferId::new(), 0..10, &context);

    assert!(annotations.is_empty());
}

// ========================================================================
// LineNumberPresenter tests
// ========================================================================

#[test]
fn test_presenter_id() {
    let presenter = LineNumberPresenter::new();
    assert_eq!(presenter.id(), "builtin.line_number");
}

#[test]
fn test_presenter_handles() {
    let presenter = LineNumberPresenter::new();
    let pattern = presenter.handles();
    assert!(pattern.matches(&AnnotationKind::new("line_number")));
    assert!(!pattern.matches(&AnnotationKind::new("diagnostic")));
}

#[test]
fn test_presenter_present() {
    let presenter = LineNumberPresenter::new();
    let annotation = Annotation {
        kind: AnnotationKind::new("line_number"),
        target: AnnotationTarget::Line(0),
        priority: 0,
        payload: AnnotationPayload::Number(42),
    };
    let ctx = PresenterContext::new(100, 5, false);

    let output = presenter.present(&annotation, &ctx);
    let cells = output.into_cells();
    assert!(!cells.is_empty());

    // Should contain "42 " (number + space)
    let text: String = cells.iter().map(|c| c.char).collect();
    assert!(text.contains("42"), "Expected '42' in output: {text}");
}

#[test]
fn test_presenter_column_width() {
    let presenter = LineNumberPresenter::new();

    // 1-9 lines: 1 digit + 1 space = 2
    let ctx = PresenterContext::new(9, 0, false);
    let width = presenter.column_width(&ctx);
    assert_eq!(width.resolve(10), 2);

    // 10-99 lines: 2 digits + 1 space = 3
    let ctx = PresenterContext::new(50, 0, false);
    let width = presenter.column_width(&ctx);
    assert_eq!(width.resolve(10), 3);

    // 100-999 lines: 3 digits + 1 space = 4
    let ctx = PresenterContext::new(500, 0, false);
    let width = presenter.column_width(&ctx);
    assert_eq!(width.resolve(10), 4);
}

#[test]
fn test_presenter_hidden_for_wrong_payload() {
    let presenter = LineNumberPresenter::new();
    let annotation = Annotation {
        kind: AnnotationKind::new("line_number"),
        target: AnnotationTarget::Line(0),
        priority: 0,
        payload: AnnotationPayload::Text("not a number".to_string()),
    };
    let ctx = PresenterContext::new(100, 0, false);

    let output = presenter.present(&annotation, &ctx);
    let cells = output.into_cells();
    assert!(cells.is_empty());
}

#[test]
fn test_digits_needed() {
    assert_eq!(LineNumberPresenter::digits_needed(0), 1);
    assert_eq!(LineNumberPresenter::digits_needed(1), 1);
    assert_eq!(LineNumberPresenter::digits_needed(9), 1);
    assert_eq!(LineNumberPresenter::digits_needed(10), 2);
    assert_eq!(LineNumberPresenter::digits_needed(99), 2);
    assert_eq!(LineNumberPresenter::digits_needed(100), 3);
    assert_eq!(LineNumberPresenter::digits_needed(999), 3);
    assert_eq!(LineNumberPresenter::digits_needed(1000), 4);
}

// ========================================================================
// Integration tests
// ========================================================================

#[test]
fn test_source_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LineNumberSource>();
}

#[test]
fn test_presenter_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LineNumberPresenter>();
}

#[test]
fn test_create_helpers() {
    let source = create_line_number_source(LineNumberMode::Absolute);
    assert_eq!(source.mode(), LineNumberMode::Absolute);

    let presenter = create_line_number_presenter();
    assert_eq!(presenter.id(), "builtin.line_number");
}

// ========================================================================
// Additional source tests
// ========================================================================

#[test]
fn test_source_set_mode() {
    let mut source = LineNumberSource::absolute();
    assert_eq!(source.mode(), LineNumberMode::Absolute);

    source.set_mode(LineNumberMode::Relative);
    assert_eq!(source.mode(), LineNumberMode::Relative);

    source.set_mode(LineNumberMode::Hybrid);
    assert_eq!(source.mode(), LineNumberMode::Hybrid);

    source.set_mode(LineNumberMode::None);
    assert_eq!(source.mode(), LineNumberMode::None);
}

#[test]
#[allow(clippy::redundant_clone)]
fn test_source_clone() {
    let source = LineNumberSource::hybrid();
    let cloned = source.clone();
    assert_eq!(cloned.mode(), LineNumberMode::Hybrid);
}

#[test]
fn test_source_debug() {
    let source = LineNumberSource::absolute();
    let debug = format!("{source:?}");
    assert!(debug.contains("LineNumberSource"));
}

#[test]
fn test_source_absolute_large_range() {
    let source = LineNumberSource::absolute();
    let context = AnnotationContext::new(1000, 500, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 998..1000, &context);

    assert_eq!(annotations.len(), 2);
    assert_eq!(annotations[0].payload.as_number(), Some(999));
    assert_eq!(annotations[1].payload.as_number(), Some(1000));
}

#[test]
fn test_source_relative_cursor_at_zero() {
    let source = LineNumberSource::relative();
    let context = AnnotationContext::new(10, 0, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 0..3, &context);

    assert_eq!(annotations.len(), 3);
    assert_eq!(annotations[0].payload.as_number(), Some(0));
    assert_eq!(annotations[1].payload.as_number(), Some(1));
    assert_eq!(annotations[2].payload.as_number(), Some(2));
}

#[test]
fn test_source_hybrid_cursor_at_first_line() {
    let source = LineNumberSource::hybrid();
    let context = AnnotationContext::new(10, 0, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 0..3, &context);

    assert_eq!(annotations.len(), 3);
    // Line 0 is cursor line: absolute = 1
    assert_eq!(annotations[0].payload.as_number(), Some(1));
    // Line 1: relative = 1
    assert_eq!(annotations[1].payload.as_number(), Some(1));
    // Line 2: relative = 2
    assert_eq!(annotations[2].payload.as_number(), Some(2));
}

#[test]
fn test_source_empty_range() {
    let source = LineNumberSource::absolute();
    let context = AnnotationContext::new(10, 0, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 5..5, &context);

    assert!(annotations.is_empty());
}

#[test]
fn test_source_single_line_range() {
    let source = LineNumberSource::absolute();
    let context = AnnotationContext::new(10, 0, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 3..4, &context);

    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].payload.as_number(), Some(4));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_source_annotation_target_is_line() {
    let source = LineNumberSource::absolute();
    let context = AnnotationContext::new(10, 0, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 0..3, &context);

    for (i, annotation) in annotations.iter().enumerate() {
        assert!(matches!(annotation.target, AnnotationTarget::Line(l) if l == i));
    }
}

#[test]
fn test_source_annotation_kind() {
    let source = LineNumberSource::absolute();
    let context = AnnotationContext::new(10, 0, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 0..1, &context);

    assert_eq!(annotations[0].kind.name(), "line_number");
}

#[test]
fn test_source_annotation_priority() {
    let source = LineNumberSource::absolute();
    let context = AnnotationContext::new(10, 0, "normal".to_string());
    let annotations = source.annotations(BufferId::new(), 0..1, &context);

    assert_eq!(annotations[0].priority, 0);
}

// ========================================================================
// Additional presenter tests
// ========================================================================

#[test]
#[allow(clippy::redundant_clone)]
fn test_presenter_with_styles() {
    let style1 = Style::default();
    let style2 = Style::default();
    let presenter = LineNumberPresenter::with_styles(style1.clone(), style2.clone());
    assert_eq!(presenter.id(), "builtin.line_number");
}

#[test]
fn test_presenter_debug() {
    let presenter = LineNumberPresenter::new();
    let debug = format!("{presenter:?}");
    assert!(debug.contains("LineNumberPresenter"));
}

#[test]
#[allow(clippy::redundant_clone)]
fn test_presenter_clone() {
    let presenter = LineNumberPresenter::new();
    let cloned = presenter.clone();
    assert_eq!(cloned.id(), "builtin.line_number");
}

#[test]
fn test_presenter_cursor_line_number() {
    let presenter = LineNumberPresenter::new();
    let annotation = Annotation {
        kind: AnnotationKind::new("line_number"),
        target: AnnotationTarget::Line(5),
        priority: 0,
        payload: AnnotationPayload::Number(6),
    };
    // cursor_line = 5 matches the annotation target line
    let ctx = PresenterContext::new(100, 5, false);

    let output = presenter.present(&annotation, &ctx);
    let cells = output.into_cells();
    assert!(!cells.is_empty());
}

#[test]
fn test_presenter_non_cursor_line_number() {
    let presenter = LineNumberPresenter::new();
    let annotation = Annotation {
        kind: AnnotationKind::new("line_number"),
        target: AnnotationTarget::Line(3),
        priority: 0,
        payload: AnnotationPayload::Number(4),
    };
    // cursor_line = 5, annotation is at line 3 - not cursor line
    let ctx = PresenterContext::new(100, 5, false);

    let output = presenter.present(&annotation, &ctx);
    let cells = output.into_cells();
    assert!(!cells.is_empty());
}

#[test]
fn test_digits_needed_large_numbers() {
    assert_eq!(LineNumberPresenter::digits_needed(10_000), 5);
    assert_eq!(LineNumberPresenter::digits_needed(99_999), 5);
    assert_eq!(LineNumberPresenter::digits_needed(100_000), 6);
}

#[test]
fn test_digits_needed_powers_of_ten() {
    assert_eq!(LineNumberPresenter::digits_needed(1), 1);
    assert_eq!(LineNumberPresenter::digits_needed(10), 2);
    assert_eq!(LineNumberPresenter::digits_needed(100), 3);
    assert_eq!(LineNumberPresenter::digits_needed(1000), 4);
    assert_eq!(LineNumberPresenter::digits_needed(10000), 5);
}

// ========================================================================
// Additional create helper tests
// ========================================================================

#[test]
fn test_create_line_number_source_relative() {
    let source = create_line_number_source(LineNumberMode::Relative);
    assert_eq!(source.mode(), LineNumberMode::Relative);
}

#[test]
fn test_create_line_number_source_hybrid() {
    let source = create_line_number_source(LineNumberMode::Hybrid);
    assert_eq!(source.mode(), LineNumberMode::Hybrid);
}

#[test]
fn test_create_line_number_source_none() {
    let source = create_line_number_source(LineNumberMode::None);
    assert_eq!(source.mode(), LineNumberMode::None);
}

#[test]
fn test_presenter_column_width_one_line() {
    let presenter = LineNumberPresenter::new();
    let ctx = PresenterContext::new(1, 0, false);
    let width = presenter.column_width(&ctx);
    assert_eq!(width.resolve(10), 2); // 1 digit + 1 space
}

#[test]
fn test_presenter_column_width_zero_lines() {
    let presenter = LineNumberPresenter::new();
    let ctx = PresenterContext::new(0, 0, false);
    let width = presenter.column_width(&ctx);
    assert_eq!(width.resolve(10), 2); // 1 digit (for 0) + 1 space
}
