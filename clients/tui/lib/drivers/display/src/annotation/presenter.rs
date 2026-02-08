//! Annotation presenter trait for gutter rendering.
//!
//! Presenters handle the visual representation of annotations.
//! Each presenter knows how to render specific annotation kinds.
//!
//! # Architecture
//!
//! ```text
//! AnnotationPresenter (mechanism trait - this module)
//!        ↓ implements
//! LineNumberPresenter (policy - in server/modules/vim)
//! DiagnosticPresenter (policy - in server/modules/lsp)
//! GitPresenter        (policy - in server/modules/git)
//! ```
//!
//! # Design
//!
//! Presenters are the visual counterpart to sources:
//! - **Source**: Generates annotation data (what exists)
//! - **Presenter**: Renders annotation data (how it looks)
//!
//! This separation allows:
//! - Same data rendered differently based on configuration
//! - Multiple presenters for the same annotation kind
//! - Theme-aware rendering

use crate::highlight::Style;

use super::types::{Annotation, AnnotationKind};

/// A single cell in the gutter.
///
/// Represents one character position with styling.
#[derive(Debug, Clone)]
pub struct GutterCell {
    /// Character to display.
    pub char: char,
    /// Style for this cell.
    pub style: Style,
}

impl GutterCell {
    /// Create a new gutter cell.
    #[must_use]
    pub const fn new(char: char, style: Style) -> Self {
        Self { char, style }
    }

    /// Create a space cell with default style.
    #[must_use]
    pub fn space() -> Self {
        Self {
            char: ' ',
            style: Style::default(),
        }
    }

    /// Create a space cell with the given style.
    #[must_use]
    pub const fn space_styled(style: Style) -> Self {
        Self { char: ' ', style }
    }

    /// Get the display width of this cell.
    ///
    /// Most characters are width 1, but some (CJK, emoji) are width 2.
    #[must_use]
    pub fn width(&self) -> usize {
        unicode_width::UnicodeWidthChar::width(self.char).unwrap_or(1)
    }
}

impl Default for GutterCell {
    fn default() -> Self {
        Self::space()
    }
}

/// Output from a presenter for a single annotation.
///
/// Presenters can return different output types:
/// - Single cell (most common)
/// - Multiple cells (for text like line numbers)
/// - Hidden (annotation exists but shouldn't be displayed)
#[derive(Debug, Clone)]
pub enum PresentedOutput {
    /// Single character cell.
    Cell(GutterCell),
    /// Multiple cells (e.g., line number "123").
    Cells(Vec<GutterCell>),
    /// Don't display this annotation.
    Hidden,
}

impl PresentedOutput {
    /// Create a single cell output.
    #[must_use]
    pub const fn cell(char: char, style: Style) -> Self {
        Self::Cell(GutterCell::new(char, style))
    }

    /// Create a multi-cell output from a string.
    #[must_use]
    pub fn text(s: &str, style: &Style) -> Self {
        Self::Cells(
            s.chars()
                .map(|c| GutterCell::new(c, style.clone()))
                .collect(),
        )
    }

    /// Create a hidden output.
    #[must_use]
    pub const fn hidden() -> Self {
        Self::Hidden
    }

    /// Get the display width of this output.
    #[must_use]
    pub fn width(&self) -> usize {
        match self {
            Self::Cell(cell) => cell.width(),
            Self::Cells(cells) => cells.iter().map(GutterCell::width).sum(),
            Self::Hidden => 0,
        }
    }

    /// Check if this output is hidden.
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }

    /// Convert to cells, returning empty vec for hidden.
    #[must_use]
    pub fn into_cells(self) -> Vec<GutterCell> {
        match self {
            Self::Cell(cell) => vec![cell],
            Self::Cells(cells) => cells,
            Self::Hidden => Vec::new(),
        }
    }
}

/// Pattern for matching annotation kinds.
///
/// Used by presenters to declare which kinds they handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KindPattern {
    /// Match exactly this kind name.
    Exact(String),
    /// Match any kind starting with this prefix (followed by `.`).
    Prefix(String),
    /// Match all annotation kinds.
    All,
}

impl KindPattern {
    /// Create an exact match pattern.
    #[must_use]
    pub fn exact(name: impl Into<String>) -> Self {
        Self::Exact(name.into())
    }

    /// Create a prefix match pattern.
    ///
    /// The prefix should not include the trailing dot.
    /// E.g., `prefix("diagnostic")` matches `"diagnostic.error"`, `"diagnostic.warning"`.
    #[must_use]
    pub fn prefix(name: impl Into<String>) -> Self {
        Self::Prefix(name.into())
    }

    /// Check if this pattern matches the given kind.
    #[must_use]
    pub fn matches(&self, kind: &AnnotationKind) -> bool {
        match self {
            Self::Exact(name) => kind.name() == name,
            Self::Prefix(prefix) => kind.is_prefix(prefix),
            Self::All => true,
        }
    }
}

/// Trait for annotation presenters (mechanism).
///
/// Presenters are responsible for:
/// - Declaring which annotation kinds they handle
/// - Rendering annotations to gutter cells
/// - Calculating column width requirements
///
/// # Thread Safety
///
/// Presenters must be `Send + Sync` for concurrent access.
///
/// # Example
///
/// ```ignore
/// struct SignPresenter;
///
/// impl AnnotationPresenter for SignPresenter {
///     fn id(&self) -> &'static str { "sign" }
///
///     fn handles(&self) -> KindPattern {
///         KindPattern::prefix("sign")
///     }
///
///     fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
///         match annotation.payload {
///             AnnotationPayload::Text(ref s) => {
///                 PresentedOutput::cell(s.chars().next().unwrap_or('?'), Style::default())
///             }
///             _ => PresentedOutput::hidden(),
///         }
///     }
///
///     fn column_width(&self, _ctx: &PresenterContext) -> ColumnWidth {
///         ColumnWidth::Fixed(2)
///     }
/// }
/// ```
pub trait AnnotationPresenter: Send + Sync {
    /// Unique identifier for this presenter.
    fn id(&self) -> &'static str;

    /// Pattern for annotation kinds this presenter handles.
    fn handles(&self) -> KindPattern;

    /// Check if this presenter can handle the given kind.
    ///
    /// Default implementation uses `handles().matches(kind)`.
    fn can_handle(&self, kind: &AnnotationKind) -> bool {
        self.handles().matches(kind)
    }

    /// Render an annotation to gutter output.
    ///
    /// # Parameters
    ///
    /// - `annotation`: The annotation to render
    /// - `ctx`: Context about the current rendering state
    ///
    /// # Returns
    ///
    /// The visual representation of this annotation.
    fn present(&self, annotation: &Annotation, ctx: &PresenterContext) -> PresentedOutput;

    /// Get the column width for this presenter.
    ///
    /// Used by the composer to allocate gutter space.
    fn column_width(&self, ctx: &PresenterContext) -> ColumnWidth;
}

/// Context available to presenters when rendering.
#[derive(Debug, Clone, Default)]
pub struct PresenterContext {
    /// Total lines in the buffer.
    pub total_lines: usize,
    /// Current cursor line.
    pub cursor_line: usize,
    /// Whether this line is the cursor line.
    pub is_cursor_line: bool,
}

impl PresenterContext {
    /// Create a new presenter context.
    #[must_use]
    pub const fn new(total_lines: usize, cursor_line: usize, is_cursor_line: bool) -> Self {
        Self {
            total_lines,
            cursor_line,
            is_cursor_line,
        }
    }
}

/// Column width specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnWidth {
    /// Fixed width in characters.
    Fixed(u16),
    /// Dynamic width with min/max bounds.
    Dynamic {
        /// Minimum width.
        min: u16,
        /// Maximum width.
        max: u16,
    },
}

impl ColumnWidth {
    /// Create a fixed width.
    #[must_use]
    pub const fn fixed(width: u16) -> Self {
        Self::Fixed(width)
    }

    /// Create a dynamic width with bounds.
    #[must_use]
    pub const fn dynamic(min: u16, max: u16) -> Self {
        Self::Dynamic { min, max }
    }

    /// Get the minimum width.
    #[must_use]
    pub const fn min_width(&self) -> u16 {
        match *self {
            Self::Fixed(w) => w,
            Self::Dynamic { min, .. } => min,
        }
    }

    /// Get the maximum width.
    #[must_use]
    pub const fn max_width(&self) -> u16 {
        match *self {
            Self::Fixed(w) => w,
            Self::Dynamic { max, .. } => max,
        }
    }

    /// Calculate actual width given content requirements.
    #[must_use]
    pub const fn resolve(&self, content_width: u16) -> u16 {
        match *self {
            Self::Fixed(w) => w,
            Self::Dynamic { min, max } => {
                if content_width < min {
                    min
                } else if content_width > max {
                    max
                } else {
                    content_width
                }
            }
        }
    }
}

impl Default for ColumnWidth {
    fn default() -> Self {
        Self::Fixed(1)
    }
}

// Ensure trait is object-safe
const _: () = {
    fn _assert_object_safe(_: &dyn AnnotationPresenter) {}
};

#[cfg(test)]
mod tests {
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
}
