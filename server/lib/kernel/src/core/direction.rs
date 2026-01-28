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
/// use reovim_kernel::api::v1::*;
///
/// let forward = Direction::Forward;  // h, j, w, }, f
/// let backward = Direction::Backward; // l, k, b, {, F
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
/// use reovim_kernel::api::v1::*;
///
/// // Given text: "hello-world foo"
/// // Word boundaries: hello | - | world | foo
/// // BigWord boundaries: hello-world | foo
///
/// let word = WordBoundary::Word;       // w, b, e, ge
/// let big_word = WordBoundary::BigWord; // W, B, E, gE
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
/// use reovim_kernel::api::v1::*;
///
/// // Given line: "  hello world  "
/// //             ^  ^          ^ ^
/// //             |  |          | |
/// //             |  FirstNonBlank|
/// //             Start           End
/// //                         LastNonBlank
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
mod tests {
    use super::*;

    #[test]
    fn test_direction_opposite() {
        assert_eq!(Direction::Forward.opposite(), Direction::Backward);
        assert_eq!(Direction::Backward.opposite(), Direction::Forward);
    }

    #[test]
    fn test_direction_is_forward_backward() {
        assert!(Direction::Forward.is_forward());
        assert!(!Direction::Forward.is_backward());
        assert!(!Direction::Backward.is_forward());
        assert!(Direction::Backward.is_backward());
    }

    #[test]
    fn test_word_boundary_word() {
        let boundary = WordBoundary::Word;
        assert!(boundary.is_word_char('a'));
        assert!(boundary.is_word_char('Z'));
        assert!(boundary.is_word_char('0'));
        assert!(boundary.is_word_char('_'));
        assert!(!boundary.is_word_char('-'));
        assert!(!boundary.is_word_char(' '));
        assert!(!boundary.is_word_char('.'));
    }

    #[test]
    fn test_word_boundary_big_word() {
        let boundary = WordBoundary::BigWord;
        assert!(boundary.is_word_char('a'));
        assert!(boundary.is_word_char('-'));
        assert!(boundary.is_word_char('.'));
        assert!(boundary.is_word_char('_'));
        assert!(!boundary.is_word_char(' '));
        assert!(!boundary.is_word_char('\t'));
        assert!(!boundary.is_word_char('\n'));
    }
}
