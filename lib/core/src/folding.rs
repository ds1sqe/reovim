//! Fold data types
//!
//! This module contains the fold data types used by treesitter to produce fold ranges.
//! The stateful management (`FoldState`, `FoldManager`) lives in the fold plugin crate.

/// Kind of fold (what construct it represents)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    /// Function body
    Function,
    /// Class/struct/impl body
    Class,
    /// Import group
    Import,
    /// Comment block
    Comment,
    /// Generic block (if, for, while, etc.)
    Block,
}

/// A foldable range in the buffer
#[derive(Debug, Clone)]
pub struct FoldRange {
    /// Starting line (0-indexed)
    pub start_line: u32,
    /// Ending line (0-indexed, inclusive)
    pub end_line: u32,
    /// Kind of fold
    pub kind: FoldKind,
    /// Preview text (first line content)
    pub preview: String,
}

impl FoldRange {
    /// Create a new fold range
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String is not const
    pub fn new(start_line: u32, end_line: u32, kind: FoldKind, preview: String) -> Self {
        Self {
            start_line,
            end_line,
            kind,
            preview,
        }
    }

    /// Check if this range spans multiple lines
    #[must_use]
    pub const fn is_foldable(&self) -> bool {
        self.end_line > self.start_line
    }

    /// Get the number of lines in this fold
    #[must_use]
    pub const fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Check if a line is contained within this fold (excluding the start line)
    #[must_use]
    pub const fn contains_line(&self, line: u32) -> bool {
        line > self.start_line && line <= self.end_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_range() {
        let range = FoldRange::new(5, 10, FoldKind::Function, "fn foo()".to_string());

        assert!(range.is_foldable());
        assert_eq!(range.line_count(), 6);
        assert!(!range.contains_line(5)); // Start line not contained
        assert!(range.contains_line(6));
        assert!(range.contains_line(10));
        assert!(!range.contains_line(11));
    }
}
