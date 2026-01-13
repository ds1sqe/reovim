//! Decoration types for text styling and concealment.
//!
//! Decorations are visual modifications applied to text regions, such as
//! hiding text, replacing text with styled alternatives, or adding backgrounds.

use reovim_core::highlight::Style;

/// Span representing a region in the buffer.
///
/// Spans are inclusive of both start and end positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Starting line (0-indexed)
    pub start_line: u32,
    /// Starting column (0-indexed, byte offset)
    pub start_col: u32,
    /// Ending line (0-indexed)
    pub end_line: u32,
    /// Ending column (0-indexed, byte offset, exclusive)
    pub end_col: u32,
}

impl Span {
    /// Create a new span.
    #[must_use]
    pub const fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Create a span covering a single line.
    #[must_use]
    pub const fn line(line: u32, start_col: u32, end_col: u32) -> Self {
        Self {
            start_line: line,
            start_col,
            end_line: line,
            end_col,
        }
    }

    /// Check if the span affects the given line.
    #[must_use]
    pub const fn affects_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    /// Check if the span is on a single line.
    #[must_use]
    pub const fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }
}

/// Priority groups for decoration layering.
///
/// Higher values have higher priority and are rendered on top of lower values.
/// When multiple decorations affect the same position, the higher priority wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum DecorationGroup {
    /// Language-specific concealment (e.g., markdown links, org-mode)
    #[default]
    Language = 0,

    /// Syntax highlighting decorations
    Syntax = 10,

    /// Search match highlighting
    Search = 20,

    /// Diagnostic underlines and inline text
    Diagnostic = 30,

    /// Visual selection (highest priority)
    Visual = 40,
}

impl DecorationGroup {
    /// Get the priority value for sorting.
    #[must_use]
    pub const fn priority(self) -> u8 {
        self as u8
    }
}

/// Decoration applied to text regions.
///
/// Decorations modify how text is rendered without changing the underlying content.
#[derive(Debug, Clone)]
pub enum Decoration {
    /// Replace text with a styled replacement string.
    ///
    /// Used for concealment (e.g., markdown links showing `[link]` instead of `[text](url)`).
    Conceal {
        /// The span of text to conceal
        span: Span,
        /// The replacement text to display
        replacement: String,
        /// Optional style for the replacement
        style: Option<Style>,
    },

    /// Apply background color to entire lines.
    ///
    /// Used for cursor line highlighting, visual selection, etc.
    LineBackground {
        /// Starting line (inclusive)
        start_line: u32,
        /// Ending line (inclusive)
        end_line: u32,
        /// Background style to apply
        style: Style,
    },

    /// Hide text without replacement.
    ///
    /// The text is completely hidden from view but still exists in the buffer.
    Hide {
        /// The span of text to hide
        span: Span,
    },

    /// Apply inline style without hiding or replacing text.
    ///
    /// Used for search highlighting, diagnostic underlines, etc.
    InlineStyle {
        /// The span of text to style
        span: Span,
        /// Style to apply
        style: Style,
    },
}

impl Decoration {
    /// Create a conceal decoration.
    #[must_use]
    pub fn conceal(span: Span, replacement: impl Into<String>, style: Option<Style>) -> Self {
        Self::Conceal {
            span,
            replacement: replacement.into(),
            style,
        }
    }

    /// Create a line background decoration.
    #[must_use]
    pub const fn line_background(start_line: u32, end_line: u32, style: Style) -> Self {
        Self::LineBackground {
            start_line,
            end_line,
            style,
        }
    }

    /// Create a hide decoration.
    #[must_use]
    pub const fn hide(span: Span) -> Self {
        Self::Hide { span }
    }

    /// Create an inline style decoration.
    #[must_use]
    pub const fn inline_style(span: Span, style: Style) -> Self {
        Self::InlineStyle { span, style }
    }

    /// Check if this decoration affects the given line.
    #[must_use]
    pub const fn affects_line(&self, line: u32) -> bool {
        match self {
            Self::Conceal { span, .. } | Self::Hide { span } | Self::InlineStyle { span, .. } => {
                span.affects_line(line)
            }
            Self::LineBackground {
                start_line,
                end_line,
                ..
            } => line >= *start_line && line <= *end_line,
        }
    }

    /// Get the starting line of this decoration.
    #[must_use]
    pub const fn start_line(&self) -> u32 {
        match self {
            Self::Conceal { span, .. } | Self::Hide { span } | Self::InlineStyle { span, .. } => {
                span.start_line
            }
            Self::LineBackground { start_line, .. } => *start_line,
        }
    }

    /// Get the ending line of this decoration.
    #[must_use]
    pub const fn end_line(&self) -> u32 {
        match self {
            Self::Conceal { span, .. } | Self::Hide { span } | Self::InlineStyle { span, .. } => {
                span.end_line
            }
            Self::LineBackground { end_line, .. } => *end_line,
        }
    }

    /// Check if this is a conceal decoration.
    #[must_use]
    pub const fn is_conceal(&self) -> bool {
        matches!(self, Self::Conceal { .. })
    }

    /// Check if this is a line background decoration.
    #[must_use]
    pub const fn is_line_background(&self) -> bool {
        matches!(self, Self::LineBackground { .. })
    }

    /// Check if this is a hide decoration.
    #[must_use]
    pub const fn is_hide(&self) -> bool {
        matches!(self, Self::Hide { .. })
    }

    /// Check if this is an inline style decoration.
    #[must_use]
    pub const fn is_inline_style(&self) -> bool {
        matches!(self, Self::InlineStyle { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_affects_line() {
        let span = Span::new(5, 0, 10, 20);
        assert!(!span.affects_line(4));
        assert!(span.affects_line(5));
        assert!(span.affects_line(7));
        assert!(span.affects_line(10));
        assert!(!span.affects_line(11));
    }

    #[test]
    fn test_span_single_line() {
        let single = Span::line(5, 0, 10);
        assert!(single.is_single_line());

        let multi = Span::new(5, 0, 6, 10);
        assert!(!multi.is_single_line());
    }

    #[test]
    fn test_decoration_group_ordering() {
        assert!(DecorationGroup::Language < DecorationGroup::Syntax);
        assert!(DecorationGroup::Syntax < DecorationGroup::Search);
        assert!(DecorationGroup::Search < DecorationGroup::Diagnostic);
        assert!(DecorationGroup::Diagnostic < DecorationGroup::Visual);
    }

    #[test]
    fn test_decoration_affects_line() {
        let conceal = Decoration::conceal(Span::line(5, 0, 10), "test", None);
        assert!(!conceal.affects_line(4));
        assert!(conceal.affects_line(5));
        assert!(!conceal.affects_line(6));

        let line_bg = Decoration::line_background(5, 10, Style::default());
        assert!(!line_bg.affects_line(4));
        assert!(line_bg.affects_line(5));
        assert!(line_bg.affects_line(7));
        assert!(line_bg.affects_line(10));
        assert!(!line_bg.affects_line(11));
    }

    #[test]
    fn test_decoration_type_checks() {
        let conceal = Decoration::conceal(Span::line(0, 0, 5), "x", None);
        assert!(conceal.is_conceal());
        assert!(!conceal.is_line_background());

        let line_bg = Decoration::line_background(0, 0, Style::default());
        assert!(line_bg.is_line_background());
        assert!(!line_bg.is_conceal());

        let hide = Decoration::hide(Span::line(0, 0, 5));
        assert!(hide.is_hide());

        let inline = Decoration::inline_style(Span::line(0, 0, 5), Style::default());
        assert!(inline.is_inline_style());
    }
}
