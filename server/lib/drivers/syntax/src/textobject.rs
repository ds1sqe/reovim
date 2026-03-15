//! Semantic text object types for treesitter-based code navigation.
//!
//! Defines [`TextObjectKind`], [`TextObjectScope`], and [`TextObjectRange`]
//! used by the [`SyntaxDriver::textobject_range()`](super::SyntaxDriver::textobject_range)
//! method to resolve language-aware text objects (e.g., inner function, around class).

/// Kind of semantic text object.
///
/// Maps to treesitter query capture prefixes (e.g., `Function` -> `"function"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextObjectKind {
    /// Function or method body.
    Function,
    /// Class, struct, enum, trait, or impl block.
    Class,
    /// Function argument or parameter.
    Argument,
    /// Conditional (if/else, match/switch).
    Conditional,
    /// Loop (for, while, loop).
    Loop,
    /// Comment (line or block).
    Comment,
    /// Block (braces, indented block).
    Block,
}

impl TextObjectKind {
    /// Get the treesitter capture name prefix for this kind.
    ///
    /// Used to construct capture names like `"function.inner"` or `"class.outer"`.
    #[must_use]
    pub const fn capture_name(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Argument => "argument",
            Self::Conditional => "conditional",
            Self::Loop => "loop",
            Self::Comment => "comment",
            Self::Block => "block",
        }
    }
}

impl std::fmt::Display for TextObjectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.capture_name())
    }
}

/// Inner vs around scope for text objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextObjectScope {
    /// Inner content only (e.g., function body without signature).
    Inner,
    /// Full construct including delimiters (e.g., entire function).
    Outer,
}

impl TextObjectScope {
    /// Get the treesitter capture name suffix.
    #[must_use]
    pub const fn suffix(&self) -> &'static str {
        match self {
            Self::Inner => "inner",
            Self::Outer => "outer",
        }
    }
}

impl std::fmt::Display for TextObjectScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.suffix())
    }
}

/// Result of a text object query — byte and position range in the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextObjectRange {
    /// Byte offset where the text object starts.
    pub start_byte: usize,
    /// Byte offset where the text object ends.
    pub end_byte: usize,
    /// Start row (0-indexed).
    pub start_row: u32,
    /// Start column (0-indexed).
    pub start_col: u32,
    /// End row (0-indexed).
    pub end_row: u32,
    /// End column (0-indexed).
    pub end_col: u32,
}

impl TextObjectRange {
    /// Create a new text object range.
    #[must_use]
    pub const fn new(
        start_byte: usize,
        end_byte: usize,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Self {
        Self {
            start_byte,
            end_byte,
            start_row,
            start_col,
            end_row,
            end_col,
        }
    }

    /// Get the byte length of this range.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.end_byte - self.start_byte
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kind_capture_names() {
        assert_eq!(TextObjectKind::Function.capture_name(), "function");
        assert_eq!(TextObjectKind::Class.capture_name(), "class");
        assert_eq!(TextObjectKind::Argument.capture_name(), "argument");
        assert_eq!(TextObjectKind::Conditional.capture_name(), "conditional");
        assert_eq!(TextObjectKind::Loop.capture_name(), "loop");
        assert_eq!(TextObjectKind::Comment.capture_name(), "comment");
        assert_eq!(TextObjectKind::Block.capture_name(), "block");
    }

    #[test]
    fn test_kind_display() {
        assert_eq!(format!("{}", TextObjectKind::Function), "function");
        assert_eq!(format!("{}", TextObjectKind::Class), "class");
    }

    #[test]
    fn test_scope_suffix() {
        assert_eq!(TextObjectScope::Inner.suffix(), "inner");
        assert_eq!(TextObjectScope::Outer.suffix(), "outer");
    }

    #[test]
    fn test_scope_display() {
        assert_eq!(format!("{}", TextObjectScope::Inner), "inner");
        assert_eq!(format!("{}", TextObjectScope::Outer), "outer");
    }

    #[test]
    fn test_range_construction() {
        let range = TextObjectRange::new(10, 50, 1, 0, 3, 5);
        assert_eq!(range.start_byte, 10);
        assert_eq!(range.end_byte, 50);
        assert_eq!(range.start_row, 1);
        assert_eq!(range.start_col, 0);
        assert_eq!(range.end_row, 3);
        assert_eq!(range.end_col, 5);
    }

    #[test]
    fn test_range_byte_len() {
        let range = TextObjectRange::new(10, 50, 0, 0, 0, 0);
        assert_eq!(range.byte_len(), 40);
    }

    #[test]
    fn test_kind_eq_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TextObjectKind::Function);
        set.insert(TextObjectKind::Function);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_scope_eq() {
        assert_eq!(TextObjectScope::Inner, TextObjectScope::Inner);
        assert_ne!(TextObjectScope::Inner, TextObjectScope::Outer);
    }

    #[test]
    fn test_range_eq() {
        let a = TextObjectRange::new(0, 10, 0, 0, 0, 10);
        let b = TextObjectRange::new(0, 10, 0, 0, 0, 10);
        assert_eq!(a, b);
    }
}
