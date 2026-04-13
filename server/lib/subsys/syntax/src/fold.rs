//! Fold range types for code folding.
//!
//! This module defines types for representing foldable regions in source code,
//! such as function bodies, class definitions, and import blocks.

/// Kind of fold (what construct it represents).
///
/// Classifies foldable regions by the type of syntax construct they contain.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::FoldKind;
///
/// let kind = FoldKind::Function;
/// assert_eq!(kind, FoldKind::Function);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FoldKind {
    /// Function or method body.
    Function,
    /// Class, struct, or impl body.
    Class,
    /// Import/use group.
    Import,
    /// Comment block (multi-line comments or doc comments).
    Comment,
    /// Generic block (if, for, while, match arms, etc.).
    Block,
}

impl FoldKind {
    /// Check if this fold represents a definition (function or class).
    #[must_use]
    pub const fn is_definition(self) -> bool {
        matches!(self, Self::Function | Self::Class)
    }
}

/// A foldable range in the buffer.
///
/// Fold ranges are identified by the syntax driver and can be
/// collapsed by the editor UI. Lines are 0-indexed.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::{FoldRange, FoldKind};
///
/// let fold = FoldRange::new(5, 10, FoldKind::Function, "fn foo() {");
/// assert!(fold.is_foldable());
/// assert_eq!(fold.line_count(), 6);
/// ```
#[derive(Debug, Clone)]
pub struct FoldRange {
    /// Starting line (0-indexed).
    pub start_line: u32,
    /// Ending line (0-indexed, inclusive).
    pub end_line: u32,
    /// Kind of fold.
    pub kind: FoldKind,
    /// Preview text (first line content, for display when folded).
    pub preview: String,
}

impl FoldRange {
    /// Create a new fold range.
    #[must_use]
    pub fn new(start_line: u32, end_line: u32, kind: FoldKind, preview: impl Into<String>) -> Self {
        Self {
            start_line,
            end_line,
            kind,
            preview: preview.into(),
        }
    }

    /// Create a fold range without a preview.
    #[must_use]
    pub const fn without_preview(start_line: u32, end_line: u32, kind: FoldKind) -> Self {
        Self {
            start_line,
            end_line,
            kind,
            preview: String::new(),
        }
    }

    /// Check if this range spans multiple lines (is foldable).
    #[must_use]
    pub const fn is_foldable(&self) -> bool {
        self.end_line > self.start_line
    }

    /// Get the number of lines in this fold.
    #[must_use]
    pub const fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Get the number of hidden lines when folded.
    ///
    /// This is `line_count() - 1` since the first line is shown.
    #[must_use]
    pub const fn hidden_lines(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line)
    }

    /// Check if this fold contains a line.
    #[must_use]
    pub const fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    /// Check if this fold overlaps with another.
    #[must_use]
    pub const fn overlaps(&self, other: &Self) -> bool {
        self.start_line <= other.end_line && self.end_line >= other.start_line
    }

    /// Check if this fold contains another fold entirely.
    #[must_use]
    pub const fn contains(&self, other: &Self) -> bool {
        self.start_line <= other.start_line && self.end_line >= other.end_line
    }
}

#[cfg(test)]
#[path = "fold_tests.rs"]
mod tests;
