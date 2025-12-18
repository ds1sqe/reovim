mod color;
mod span;
pub mod store;
mod style;
mod theme;

pub use {
    color::{ColorMode, downgrade_color, rgb_to_ansi256},
    span::Span,
    store::{BufferHighlights, HighlightStore, LineHighlight},
    style::{Attributes, Style},
    theme::{StatusLineModeStyles, Theme, ThemeName},
};

/// Identifies the source/type of highlight for layering and management
/// Lower values have lower priority (get overridden by higher values)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HighlightGroup {
    /// Base syntax highlighting (lowest priority)
    Syntax = 0,
    /// Search matches
    Search = 10,
    /// Incremental search (current match)
    IncSearch = 15,
    /// Visual selection
    Visual = 20,
    /// Diagnostics
    DiagnosticHint = 30,
    DiagnosticInfo = 31,
    DiagnosticWarn = 32,
    DiagnosticError = 33,
    /// Cursor line highlight
    CursorLine = 40,
    /// Custom user highlights (highest priority)
    Custom = 100,
}

/// A single highlight entry combining span, style, and group
#[derive(Debug, Clone)]
pub struct Highlight {
    pub span: Span,
    pub style: Style,
    pub group: HighlightGroup,
}

impl Highlight {
    #[must_use]
    pub const fn new(span: Span, style: Style, group: HighlightGroup) -> Self {
        Self { span, style, group }
    }
}
