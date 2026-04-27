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
//! LineNumberPresenter (policy - in ext/server/modules/vim)
//! DiagnosticPresenter (policy - in ext/server/modules/lsp)
//! GitPresenter        (policy - in ext/server/modules/git)
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

use super::{Annotation, AnnotationKind};

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn _assert_object_safe(_: &dyn AnnotationPresenter) {}
};

#[cfg(test)]
#[path = "presenter_tests.rs"]
mod tests;
