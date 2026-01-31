//! Core types for the decoration system
//!
//! This module defines the `Decoration` enum and `DecorationGroup` for
//! visual decorations like concealment, line backgrounds, and inline styles.

use crate::highlight::{Span, Style};

/// A decoration that transforms how text is rendered
#[derive(Debug, Clone)]
pub enum Decoration {
    /// Replace a span of text with a different display string
    /// Original text is concealed, replacement is shown
    Conceal {
        span: Span,
        replacement: String,
        style: Option<Style>,
    },
    /// Apply a background color to entire line(s)
    LineBackground {
        start_line: u32,
        /// Inclusive end line
        end_line: u32,
        style: Style,
    },
    /// Hide text completely (conceal with empty replacement)
    Hide { span: Span },
    /// Apply inline style to a span (italic, bold, etc.) without hiding content
    InlineStyle { span: Span, style: Style },
}

impl Decoration {
    /// Create a conceal decoration
    #[must_use]
    pub fn conceal(span: Span, replacement: impl Into<String>, style: Option<Style>) -> Self {
        Self::Conceal {
            span,
            replacement: replacement.into(),
            style,
        }
    }

    /// Create a line background decoration
    #[must_use]
    pub const fn line_background(start_line: u32, end_line: u32, style: Style) -> Self {
        Self::LineBackground {
            start_line,
            end_line,
            style,
        }
    }

    /// Create a single-line background decoration
    #[must_use]
    pub const fn single_line_background(line: u32, style: Style) -> Self {
        Self::LineBackground {
            start_line: line,
            end_line: line,
            style,
        }
    }

    /// Create a hide decoration
    #[must_use]
    pub const fn hide(span: Span) -> Self {
        Self::Hide { span }
    }

    /// Create an inline style decoration
    #[must_use]
    pub const fn inline_style(span: Span, style: Style) -> Self {
        Self::InlineStyle { span, style }
    }

    /// Get the start line of this decoration
    #[must_use]
    pub const fn start_line(&self) -> u32 {
        match self {
            Self::Conceal { span, .. } | Self::Hide { span } | Self::InlineStyle { span, .. } => {
                span.start_line
            }
            Self::LineBackground { start_line, .. } => *start_line,
        }
    }

    /// Get the end line of this decoration
    #[must_use]
    pub const fn end_line(&self) -> u32 {
        match self {
            Self::Conceal { span, .. } | Self::Hide { span } | Self::InlineStyle { span, .. } => {
                span.end_line
            }
            Self::LineBackground { end_line, .. } => *end_line,
        }
    }

    /// Check if this decoration affects the given line
    #[must_use]
    pub const fn affects_line(&self, line: u32) -> bool {
        line >= self.start_line() && line <= self.end_line()
    }
}

/// Priority group for decoration layering
///
/// Higher priority decorations override lower priority ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum DecorationGroup {
    /// Language-specific decorations (markdown, org, etc.)
    #[default]
    Language = 0,
    /// Syntax highlighting
    Syntax = 10,
    /// Search highlights
    Search = 20,
    /// Visual selection
    Visual = 40,
}
