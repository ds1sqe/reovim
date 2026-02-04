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
/// use reovim_driver_syntax::FoldKind;
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
/// use reovim_driver_syntax::{FoldRange, FoldKind};
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
mod tests {
    use super::*;

    #[test]
    fn test_fold_kind_is_definition() {
        assert!(FoldKind::Function.is_definition());
        assert!(FoldKind::Class.is_definition());
        assert!(!FoldKind::Import.is_definition());
        assert!(!FoldKind::Comment.is_definition());
        assert!(!FoldKind::Block.is_definition());
    }

    #[test]
    fn test_fold_range_new() {
        let fold = FoldRange::new(5, 10, FoldKind::Function, "fn foo() {");

        assert_eq!(fold.start_line, 5);
        assert_eq!(fold.end_line, 10);
        assert_eq!(fold.kind, FoldKind::Function);
        assert_eq!(fold.preview, "fn foo() {");
    }

    #[test]
    fn test_fold_range_without_preview() {
        let fold = FoldRange::without_preview(5, 10, FoldKind::Block);

        assert_eq!(fold.start_line, 5);
        assert_eq!(fold.end_line, 10);
        assert_eq!(fold.kind, FoldKind::Block);
        assert!(fold.preview.is_empty());
    }

    #[test]
    fn test_fold_range_is_foldable() {
        let foldable = FoldRange::new(5, 10, FoldKind::Function, "");
        assert!(foldable.is_foldable());

        let single = FoldRange::new(5, 5, FoldKind::Block, "");
        assert!(!single.is_foldable());
    }

    #[test]
    fn test_fold_range_line_count() {
        let fold = FoldRange::new(5, 10, FoldKind::Function, "");
        assert_eq!(fold.line_count(), 6); // 5, 6, 7, 8, 9, 10

        let single = FoldRange::new(5, 5, FoldKind::Block, "");
        assert_eq!(single.line_count(), 1);
    }

    #[test]
    fn test_fold_range_hidden_lines() {
        let fold = FoldRange::new(5, 10, FoldKind::Function, "");
        assert_eq!(fold.hidden_lines(), 5); // Lines 6-10 are hidden

        let single = FoldRange::new(5, 5, FoldKind::Block, "");
        assert_eq!(single.hidden_lines(), 0);
    }

    #[test]
    fn test_fold_range_contains_line() {
        let fold = FoldRange::new(5, 10, FoldKind::Function, "");

        assert!(fold.contains_line(5));
        assert!(fold.contains_line(7));
        assert!(fold.contains_line(10));
        assert!(!fold.contains_line(4));
        assert!(!fold.contains_line(11));
    }

    #[test]
    fn test_fold_range_overlaps() {
        let fold1 = FoldRange::new(5, 10, FoldKind::Function, "");
        let fold2 = FoldRange::new(8, 15, FoldKind::Block, "");
        let fold3 = FoldRange::new(11, 15, FoldKind::Block, "");

        assert!(fold1.overlaps(&fold2)); // 8-10 overlap
        assert!(fold2.overlaps(&fold1));
        assert!(!fold1.overlaps(&fold3)); // No overlap
        assert!(!fold3.overlaps(&fold1));
    }

    #[test]
    fn test_fold_range_contains() {
        let outer = FoldRange::new(5, 15, FoldKind::Class, "");
        let inner = FoldRange::new(7, 12, FoldKind::Function, "");
        let overlapping = FoldRange::new(10, 20, FoldKind::Block, "");

        assert!(outer.contains(&inner));
        assert!(!inner.contains(&outer));
        assert!(!outer.contains(&overlapping));
    }
}
