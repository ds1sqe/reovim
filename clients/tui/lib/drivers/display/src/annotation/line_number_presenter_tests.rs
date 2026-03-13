use {
    super::*,
    crate::annotation::{AnnotationKind, AnnotationPayload, AnnotationTarget},
};

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

#[test]
fn test_presenter_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LineNumberPresenter>();
}

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
