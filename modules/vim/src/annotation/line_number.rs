//! Line number annotation source and presenter.
//!
//! Implements line numbers using the generic annotation system,
//! supporting absolute, relative, and hybrid modes.

use std::sync::Arc;

use {
    reovim_driver_display::{
        Annotation, AnnotationContext, AnnotationKind, AnnotationPayload, AnnotationPresenter,
        AnnotationSource, AnnotationTarget, ColumnWidth, KindPattern, LineNumberMode,
        PresentedOutput, PresenterContext, Style,
    },
    reovim_kernel::api::v1::BufferId,
};

/// Static annotation kind for line numbers.
static LINE_NUMBER_KIND: std::sync::LazyLock<AnnotationKind> =
    std::sync::LazyLock::new(|| AnnotationKind::new("line_number"));

/// Line number annotation source.
///
/// Generates line number annotations for visible lines based on the
/// configured display mode (absolute, relative, hybrid).
#[derive(Debug, Clone)]
pub struct LineNumberSource {
    /// Display mode for line numbers.
    mode: LineNumberMode,
}

impl LineNumberSource {
    /// Create a new line number source with the given mode.
    #[must_use]
    pub const fn new(mode: LineNumberMode) -> Self {
        Self { mode }
    }

    /// Create with absolute line numbers.
    #[must_use]
    pub const fn absolute() -> Self {
        Self::new(LineNumberMode::Absolute)
    }

    /// Create with relative line numbers.
    #[must_use]
    pub const fn relative() -> Self {
        Self::new(LineNumberMode::Relative)
    }

    /// Create with hybrid line numbers.
    #[must_use]
    pub const fn hybrid() -> Self {
        Self::new(LineNumberMode::Hybrid)
    }

    /// Get the current mode.
    #[must_use]
    pub const fn mode(&self) -> LineNumberMode {
        self.mode
    }

    /// Set the mode.
    pub fn set_mode(&mut self, mode: LineNumberMode) {
        self.mode = mode;
    }
}

impl AnnotationSource for LineNumberSource {
    fn id(&self) -> &'static str {
        "builtin.line_number"
    }

    fn provides(&self) -> Vec<AnnotationKind> {
        vec![LINE_NUMBER_KIND.clone()]
    }

    fn annotations(
        &self,
        _buffer_id: BufferId,
        range: std::ops::Range<usize>,
        context: &AnnotationContext,
    ) -> Vec<Annotation> {
        if self.mode == LineNumberMode::None {
            return vec![];
        }

        range
            .map(|line| {
                let number = match self.mode {
                    LineNumberMode::None => unreachable!(),
                    LineNumberMode::Absolute => line + 1,
                    LineNumberMode::Relative => line.abs_diff(context.cursor_line),
                    LineNumberMode::Hybrid if line == context.cursor_line => line + 1,
                    LineNumberMode::Hybrid => line.abs_diff(context.cursor_line),
                };

                Annotation {
                    kind: LINE_NUMBER_KIND.clone(),
                    target: AnnotationTarget::Line(line),
                    priority: 0,
                    payload: AnnotationPayload::Number(number),
                }
            })
            .collect()
    }

    fn has_annotations(&self, _buffer_id: BufferId) -> bool {
        self.mode != LineNumberMode::None
    }
}

/// Line number presenter.
///
/// Renders line number annotations as right-aligned numbers with
/// appropriate styling for cursor line vs other lines.
#[derive(Debug, Clone, Default)]
pub struct LineNumberPresenter {
    /// Style for regular line numbers.
    line_number_style: Style,
    /// Style for the cursor line number.
    cursor_line_style: Style,
}

impl LineNumberPresenter {
    /// Create a new presenter with default styles.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a presenter with custom styles.
    #[must_use]
    pub const fn with_styles(line_number_style: Style, cursor_line_style: Style) -> Self {
        Self {
            line_number_style,
            cursor_line_style,
        }
    }

    /// Set styles from a theme manager.
    #[must_use]
    pub fn from_theme(manager: &reovim_driver_display::ThemeManager) -> Self {
        use reovim_driver_display::style::groups;
        Self {
            line_number_style: manager.get_style(groups::LINE_NUMBER),
            cursor_line_style: manager.get_style(groups::LINE_NUMBER_ACTIVE),
        }
    }
}

impl AnnotationPresenter for LineNumberPresenter {
    fn id(&self) -> &'static str {
        "builtin.line_number"
    }

    fn handles(&self) -> KindPattern {
        KindPattern::exact("line_number")
    }

    fn present(&self, annotation: &Annotation, ctx: &PresenterContext) -> PresentedOutput {
        let Some(num) = annotation.payload.as_number() else {
            return PresentedOutput::Hidden;
        };

        let is_cursor_line = match annotation.target {
            AnnotationTarget::Line(l) => l == ctx.cursor_line,
            _ => false,
        };

        let style = if is_cursor_line {
            &self.cursor_line_style
        } else {
            &self.line_number_style
        };

        // Format with dynamic width based on total lines
        let digits = Self::digits_needed(ctx.total_lines);
        let text = format!("{num:>digits$} ");

        PresentedOutput::text(&text, style)
    }

    fn column_width(&self, ctx: &PresenterContext) -> ColumnWidth {
        // digits + 1 space padding
        let digits = Self::digits_needed(ctx.total_lines);
        #[allow(clippy::cast_possible_truncation)]
        let width = (digits + 1) as u16;
        ColumnWidth::fixed(width)
    }
}

impl LineNumberPresenter {
    /// Calculate digits needed to display a number.
    fn digits_needed(n: usize) -> usize {
        if n == 0 {
            1
        } else {
            #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
            let d = (n as f64).log10().floor() as usize + 1;
            d
        }
    }
}

/// Create a shared line number source.
#[must_use]
pub fn create_line_number_source(mode: LineNumberMode) -> Arc<LineNumberSource> {
    Arc::new(LineNumberSource::new(mode))
}

/// Create a shared line number presenter.
#[must_use]
pub fn create_line_number_presenter() -> Arc<LineNumberPresenter> {
    Arc::new(LineNumberPresenter::new())
}

#[cfg(test)]
mod tests {
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
}
