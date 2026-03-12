use super::*;

// ========================================================================
// GutterCell tests
// ========================================================================

#[test]
fn test_gutter_cell_new() {
    let cell = GutterCell::new('x', Style::default());
    assert_eq!(cell.char, 'x');
}

#[test]
fn test_gutter_cell_space() {
    let cell = GutterCell::space();
    assert_eq!(cell.char, ' ');
}

#[test]
fn test_gutter_cell_width_ascii() {
    let cell = GutterCell::new('a', Style::default());
    assert_eq!(cell.width(), 1);
}

#[test]
fn test_gutter_cell_width_wide() {
    // CJK character is width 2
    let cell = GutterCell::new('中', Style::default());
    assert_eq!(cell.width(), 2);
}

#[test]
fn test_gutter_cell_default() {
    let cell = GutterCell::default();
    assert_eq!(cell.char, ' ');
}

// ========================================================================
// PresentedOutput tests
// ========================================================================

#[test]
fn test_presented_output_cell() {
    let output = PresentedOutput::cell('x', Style::default());
    assert_eq!(output.width(), 1);
    assert!(!output.is_hidden());
}

#[test]
fn test_presented_output_text() {
    let output = PresentedOutput::text("123", &Style::default());
    assert_eq!(output.width(), 3);
}

#[test]
fn test_presented_output_hidden() {
    let output = PresentedOutput::hidden();
    assert_eq!(output.width(), 0);
    assert!(output.is_hidden());
}

#[test]
fn test_presented_output_into_cells() {
    let output = PresentedOutput::text("ab", &Style::default());
    let cells = output.into_cells();
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[0].char, 'a');
    assert_eq!(cells[1].char, 'b');
}

#[test]
fn test_presented_output_into_cells_hidden() {
    let output = PresentedOutput::hidden();
    let cells = output.into_cells();
    assert!(cells.is_empty());
}

// ========================================================================
// KindPattern tests
// ========================================================================

#[test]
fn test_kind_pattern_exact() {
    let pattern = KindPattern::exact("line_number");
    assert!(pattern.matches(&AnnotationKind::new("line_number")));
    assert!(!pattern.matches(&AnnotationKind::new("diagnostic.error")));
}

#[test]
fn test_kind_pattern_prefix() {
    let pattern = KindPattern::prefix("diagnostic");
    assert!(pattern.matches(&AnnotationKind::new("diagnostic.error")));
    assert!(pattern.matches(&AnnotationKind::new("diagnostic.warning")));
    assert!(!pattern.matches(&AnnotationKind::new("line_number")));
}

#[test]
fn test_kind_pattern_prefix_exact_match() {
    // Prefix should also match exact name
    let pattern = KindPattern::prefix("diagnostic");
    assert!(pattern.matches(&AnnotationKind::new("diagnostic")));
}

#[test]
fn test_kind_pattern_prefix_no_partial() {
    // "diag" should NOT match "diagnostic.error"
    let pattern = KindPattern::prefix("diag");
    assert!(!pattern.matches(&AnnotationKind::new("diagnostic.error")));
}

#[test]
fn test_kind_pattern_all() {
    let pattern = KindPattern::All;
    assert!(pattern.matches(&AnnotationKind::new("line_number")));
    assert!(pattern.matches(&AnnotationKind::new("diagnostic.error")));
    assert!(pattern.matches(&AnnotationKind::new("anything")));
}

// ========================================================================
// ColumnWidth tests
// ========================================================================

#[test]
fn test_column_width_fixed() {
    let width = ColumnWidth::fixed(5);
    assert_eq!(width.min_width(), 5);
    assert_eq!(width.max_width(), 5);
    assert_eq!(width.resolve(3), 5);
    assert_eq!(width.resolve(10), 5);
}

#[test]
fn test_column_width_dynamic() {
    let width = ColumnWidth::dynamic(2, 10);
    assert_eq!(width.min_width(), 2);
    assert_eq!(width.max_width(), 10);

    // Below min
    assert_eq!(width.resolve(1), 2);
    // Within range
    assert_eq!(width.resolve(5), 5);
    // Above max
    assert_eq!(width.resolve(15), 10);
}

#[test]
fn test_column_width_default() {
    let width = ColumnWidth::default();
    assert_eq!(width.min_width(), 1);
    assert_eq!(width.max_width(), 1);
}

// ========================================================================
// PresenterContext tests
// ========================================================================

#[test]
fn test_presenter_context_new() {
    let ctx = PresenterContext::new(100, 50, true);
    assert_eq!(ctx.total_lines, 100);
    assert_eq!(ctx.cursor_line, 50);
    assert!(ctx.is_cursor_line);
}

#[test]
fn test_presenter_context_default() {
    let ctx = PresenterContext::default();
    assert_eq!(ctx.total_lines, 0);
    assert_eq!(ctx.cursor_line, 0);
    assert!(!ctx.is_cursor_line);
}

// ========================================================================
// Mock presenter for trait tests
// ========================================================================

struct MockPresenter {
    pattern: KindPattern,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl AnnotationPresenter for MockPresenter {
    fn id(&self) -> &'static str {
        "test.mock"
    }

    fn handles(&self) -> KindPattern {
        self.pattern.clone()
    }

    #[allow(clippy::option_if_let_else)]
    fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
        if let Some(n) = annotation.payload.as_number() {
            PresentedOutput::text(&n.to_string(), &Style::default())
        } else {
            PresentedOutput::cell('?', Style::default())
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn column_width(&self, ctx: &PresenterContext) -> ColumnWidth {
        // Calculate digits needed for line count
        let digits = if ctx.total_lines == 0 {
            1
        } else {
            (ctx.total_lines as f64).log10().floor() as u16 + 1
        };
        ColumnWidth::dynamic(2, digits.max(2))
    }
}

#[test]
fn test_mock_presenter_can_handle() {
    let presenter = MockPresenter {
        pattern: KindPattern::exact("line_number"),
    };
    assert!(presenter.can_handle(&AnnotationKind::new("line_number")));
    assert!(!presenter.can_handle(&AnnotationKind::new("diagnostic.error")));
}

#[test]
fn test_mock_presenter_present() {
    let presenter = MockPresenter {
        pattern: KindPattern::All,
    };
    let annotation = Annotation::line_number(0, 42);
    let ctx = PresenterContext::default();

    let output = presenter.present(&annotation, &ctx);
    assert_eq!(output.width(), 2); // "42" is 2 chars
}

#[test]
fn test_mock_presenter_column_width() {
    let presenter = MockPresenter {
        pattern: KindPattern::All,
    };

    // 100 lines = 3 digits
    let ctx = PresenterContext::new(100, 0, false);
    let width = presenter.column_width(&ctx);
    assert_eq!(width.max_width(), 3);

    // 10000 lines = 5 digits
    let ctx = PresenterContext::new(10000, 0, false);
    let width = presenter.column_width(&ctx);
    assert_eq!(width.max_width(), 5);
}

#[test]
fn test_presenter_is_object_safe() {
    let presenter = MockPresenter {
        pattern: KindPattern::All,
    };
    let _: &dyn AnnotationPresenter = &presenter;
}

#[test]
fn test_presenter_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MockPresenter>();
}

// ========================================================================
// Additional coverage tests
// ========================================================================

#[test]
fn test_gutter_cell_space_styled() {
    // Covers lines 60-62: space_styled constructor
    let style = Style::default();
    let cell = GutterCell::space_styled(style);
    assert_eq!(cell.char, ' ');
    assert_eq!(cell.width(), 1);
}

#[test]
fn test_presented_output_into_cells_single_cell() {
    // Covers line 138: Cell variant of into_cells
    let output = PresentedOutput::cell('x', Style::default());
    let cells = output.into_cells();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].char, 'x');
}
