//! Core types for the decoration system
//!
//! This module defines the `LanguageRenderer` trait and associated types that
//! language-specific modules implement to provide visual decorations.

use std::any::Any;

use crate::{
    highlight::{Span, Style},
    modd::EditMode,
    treesitter::LanguageId,
};

/// Context passed to language renderers for decoration generation
#[derive(Debug)]
pub struct DecorationContext<'a> {
    /// Buffer content
    pub content: &'a str,
    /// Parsed treesitter tree
    pub tree: &'a tree_sitter::Tree,
    /// Current edit mode (Insert, Normal, Visual)
    pub edit_mode: &'a EditMode,
    /// Current cursor line (0-indexed)
    pub cursor_line: u32,
    /// Buffer ID
    pub buffer_id: usize,
}

impl<'a> DecorationContext<'a> {
    /// Create a new decoration context
    #[must_use]
    pub const fn new(
        content: &'a str,
        tree: &'a tree_sitter::Tree,
        edit_mode: &'a EditMode,
        cursor_line: u32,
        buffer_id: usize,
    ) -> Self {
        Self {
            content,
            tree,
            edit_mode,
            cursor_line,
            buffer_id,
        }
    }

    /// Check if we're in insert mode
    #[must_use]
    pub const fn is_insert_mode(&self) -> bool {
        matches!(self.edit_mode, EditMode::Insert(_))
    }
}

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

/// Trait for language-specific decoration rendering
///
/// Implementations provide decorations for specific file types.
/// Core owns the trait, language modules implement it.
pub trait LanguageRenderer: Send + Sync {
    /// Language ID this renderer handles
    fn language_id(&self) -> LanguageId;

    /// File extensions this renderer applies to
    fn file_extensions(&self) -> &[&str];

    /// Generate decorations for the buffer
    ///
    /// Called when decorations need to be regenerated (buffer change, mode change, etc.)
    fn render(&self, ctx: &DecorationContext, query: &tree_sitter::Query) -> Vec<Decoration>;

    /// Whether to show raw content (no decorations) for a specific line
    ///
    /// This allows mode-aware rendering (e.g., show raw in insert mode)
    fn should_show_raw(&self, line: u32, ctx: &DecorationContext) -> bool;

    /// Get the configuration as Any for downcasting
    fn config(&self) -> &dyn Any;

    /// Get mutable configuration as Any for downcasting
    fn config_mut(&mut self) -> &mut dyn Any;

    /// Whether this renderer is enabled
    fn is_enabled(&self) -> bool;

    /// Enable or disable this renderer
    fn set_enabled(&mut self, enabled: bool);
}
