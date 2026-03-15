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
#[path = "textobject_tests.rs"]
mod tests;
