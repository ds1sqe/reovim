//! Direction and boundary types for motion calculations.
//!
//! This module provides the foundational enums used by both
//! motion calculations and text object handling.

/// Direction of cursor movement.
///
/// Used by character, line, word, paragraph, and find-char motions.
///
/// # Example
///
/// ```
/// use reovim_domain_text::Direction;
///
/// let forward = Direction::Forward;
/// let backward = Direction::Backward;
/// assert!(forward.is_forward());
/// assert!(backward.is_backward());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Move forward (right for chars, down for lines)
    Forward,
    /// Move backward (left for chars, up for lines)
    Backward,
}

impl Direction {
    /// Check if direction is forward.
    #[must_use]
    pub const fn is_forward(&self) -> bool {
        matches!(self, Self::Forward)
    }

    /// Check if direction is backward.
    #[must_use]
    pub const fn is_backward(&self) -> bool {
        matches!(self, Self::Backward)
    }

    /// Get the opposite direction.
    #[must_use]
    pub const fn opposite(&self) -> Self {
        match self {
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
        }
    }
}

/// Word boundary type for word motions.
///
/// Vim distinguishes between "words" and "WORDS":
/// - **Word**: Sequences of alphanumeric characters or underscores,
///   separated by other non-whitespace characters or whitespace.
/// - **`BigWord` (WORD)**: Sequences of non-whitespace characters,
///   separated only by whitespace.
///
/// # Example
///
/// ```
/// use reovim_domain_text::WordBoundary;
///
/// let word = WordBoundary::Word;
/// let big_word = WordBoundary::BigWord;
///
/// assert!(word.is_word_char('a'));
/// assert!(word.is_word_char('_'));
/// assert!(!word.is_word_char('.'));
/// assert!(big_word.is_word_char('.'));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WordBoundary {
    /// Word: alphanumeric + underscore sequences (w/b/e)
    Word,
    /// WORD: non-whitespace sequences (W/B/E)
    BigWord,
}

impl WordBoundary {
    /// Check if a character is a word character for this boundary type.
    #[must_use]
    pub fn is_word_char(&self, c: char) -> bool {
        match self {
            Self::Word => c.is_alphanumeric() || c == '_',
            Self::BigWord => !c.is_whitespace(),
        }
    }
}

/// Line position types for line-based motions.
///
/// These correspond to vim's 0, ^, $, and g_ commands.
///
/// # Example
///
/// ```
/// use reovim_domain_text::LinePosition;
///
/// let start = LinePosition::Start;           // 0
/// let first = LinePosition::FirstNonBlank;   // ^
/// let end = LinePosition::End;               // $
/// let last = LinePosition::LastNonBlank;     // g_
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinePosition {
    /// Start of line, column 0 (vim: 0)
    Start,
    /// First non-blank character (vim: ^)
    FirstNonBlank,
    /// End of line, last character (vim: $)
    End,
    /// Last non-blank character (vim: g_)
    LastNonBlank,
}

#[cfg(test)]
#[path = "direction_tests.rs"]
mod tests;
