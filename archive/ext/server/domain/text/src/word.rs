//! Word boundary detection for text navigation.
//!
//! This module provides pure functions for classifying characters and
//! finding word boundaries. It supports both "small words" (w/b motions)
//! and "big words" (W/B motions) as defined by vim.
//!
//! # Design Philosophy
//!
//! Pure functions operating on `&[char]` slices — no buffer or position
//! knowledge, no movement commands.
//!
//! # Word Types
//!
//! - **Small words** (`w`, `b`, `e`): Sequences of word characters (alphanumeric + underscore)
//!   or sequences of punctuation, separated by whitespace or character type changes.
//! - **Big words** (`W`, `B`, `E`): Any non-whitespace sequences, separated only by whitespace.
//!
//! # Example
//!
//! ```
//! use reovim_domain_text::*;
//!
//! let text: Vec<char> = "hello_world foo.bar".chars().collect();
//!
//! // Find word boundaries for small word at position 0
//! let (start, end) = word_bounds(&text, 0, WordType::Small);
//! assert_eq!(start, 0);
//! assert_eq!(end, 10); // "hello_world"
//!
//! // Big word treats foo.bar as one word
//! let (start, end) = word_bounds(&text, 12, WordType::Big);
//! assert_eq!(start, 12);
//! assert_eq!(end, 18); // "foo.bar"
//! ```

/// Character classification for word boundary detection.
///
/// Characters are classified into three categories:
/// - `Word`: Alphanumeric and underscore (the "keyword" characters)
/// - `Punctuation`: Non-whitespace, non-word characters
/// - `Whitespace`: Spaces, tabs, newlines, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharKind {
    /// Word character: alphanumeric or underscore.
    Word,
    /// Punctuation: non-whitespace, non-word.
    Punctuation,
    /// Whitespace: space, tab, newline, etc.
    Whitespace,
}

/// Word type for boundary detection.
///
/// Determines how word boundaries are calculated:
/// - `Small`: Traditional vim "word" (w/b/e motions)
/// - `Big`: Traditional vim "WORD" (W/B/E motions)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WordType {
    /// Small word: word characters only, punctuation is separate.
    #[default]
    Small,
    /// Big word: any non-whitespace sequence.
    Big,
}

/// Classify a character.
///
/// # Examples
///
/// ```
/// use reovim_domain_text::*;
///
/// assert_eq!(char_kind('a'), CharKind::Word);
/// assert_eq!(char_kind('_'), CharKind::Word);
/// assert_eq!(char_kind('5'), CharKind::Word);
/// assert_eq!(char_kind('.'), CharKind::Punctuation);
/// assert_eq!(char_kind(' '), CharKind::Whitespace);
/// ```
#[must_use]
pub fn char_kind(c: char) -> CharKind {
    if c.is_whitespace() {
        CharKind::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        CharKind::Word
    } else {
        CharKind::Punctuation
    }
}

/// Find the start of the word containing the given position.
///
/// Searches backward from `pos` to find where the current word begins.
/// The definition of "word" depends on `word_type`.
///
/// # Arguments
///
/// * `chars` - The character slice to search
/// * `pos` - The starting position (0-indexed)
/// * `word_type` - Whether to use small or big word semantics
///
/// # Returns
///
/// The index of the first character of the word, or 0 if at start.
///
/// # Examples
///
/// ```
/// use reovim_domain_text::*;
///
/// let text: Vec<char> = "hello world".chars().collect();
/// assert_eq!(word_start(&text, 3, WordType::Small), 0); // 'l' is part of "hello"
/// assert_eq!(word_start(&text, 8, WordType::Small), 6); // 'r' is part of "world"
/// ```
#[must_use]
pub fn word_start(chars: &[char], pos: usize, word_type: WordType) -> usize {
    if chars.is_empty() || pos == 0 {
        return 0;
    }

    let pos = pos.min(chars.len() - 1);
    let current_kind = char_kind(chars[pos]);

    // For whitespace, we're not in a word
    if current_kind == CharKind::Whitespace {
        return pos;
    }

    let matches = |c: char| match word_type {
        WordType::Small => char_kind(c) == current_kind,
        WordType::Big => !c.is_whitespace(),
    };

    let mut idx = pos;
    while idx > 0 && matches(chars[idx - 1]) {
        idx -= 1;
    }
    idx
}

/// Find the end of the word containing the given position.
///
/// Searches forward from `pos` to find where the current word ends.
/// The definition of "word" depends on `word_type`.
///
/// # Arguments
///
/// * `chars` - The character slice to search
/// * `pos` - The starting position (0-indexed)
/// * `word_type` - Whether to use small or big word semantics
///
/// # Returns
///
/// The index of the last character of the word.
///
/// # Examples
///
/// ```
/// use reovim_domain_text::*;
///
/// let text: Vec<char> = "hello world".chars().collect();
/// assert_eq!(word_end(&text, 0, WordType::Small), 4); // "hello" ends at 4
/// assert_eq!(word_end(&text, 6, WordType::Small), 10); // "world" ends at 10
/// ```
#[must_use]
pub fn word_end(chars: &[char], pos: usize, word_type: WordType) -> usize {
    if chars.is_empty() {
        return 0;
    }

    let pos = pos.min(chars.len() - 1);
    let current_kind = char_kind(chars[pos]);

    // For whitespace, we're not in a word
    if current_kind == CharKind::Whitespace {
        return pos;
    }

    let matches = |c: char| match word_type {
        WordType::Small => char_kind(c) == current_kind,
        WordType::Big => !c.is_whitespace(),
    };

    let mut idx = pos;
    while idx < chars.len() - 1 && matches(chars[idx + 1]) {
        idx += 1;
    }
    idx
}

/// Find both word boundaries around a position.
///
/// Convenience function that returns both the start and end of the word
/// containing the given position.
///
/// # Arguments
///
/// * `chars` - The character slice to search
/// * `pos` - The position within the word
/// * `word_type` - Whether to use small or big word semantics
///
/// # Returns
///
/// A tuple of (`start_index`, `end_index`) for the word.
///
/// # Examples
///
/// ```
/// use reovim_domain_text::*;
///
/// let text: Vec<char> = "hello world".chars().collect();
/// let (start, end) = word_bounds(&text, 2, WordType::Small);
/// assert_eq!(start, 0);
/// assert_eq!(end, 4);
/// ```
#[must_use]
pub fn word_bounds(chars: &[char], pos: usize, word_type: WordType) -> (usize, usize) {
    (word_start(chars, pos, word_type), word_end(chars, pos, word_type))
}

/// Find the start of the next word.
///
/// Searches forward from `pos` to find the beginning of the next word.
/// Skips over the current word and any whitespace.
///
/// # Arguments
///
/// * `chars` - The character slice to search
/// * `pos` - The starting position
/// * `word_type` - Whether to use small or big word semantics
///
/// # Returns
///
/// The index of the first character of the next word, or `chars.len()`
/// if no next word exists.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn next_word_start(chars: &[char], pos: usize, word_type: WordType) -> usize {
    if chars.is_empty() {
        return 0;
    }

    let mut idx = pos.min(chars.len() - 1);
    let current_kind = char_kind(chars[idx]);

    // Skip current word (or whitespace)
    match word_type {
        WordType::Small => {
            // Skip same-kind characters
            while idx < chars.len() && char_kind(chars[idx]) == current_kind {
                idx += 1;
            }
        }
        WordType::Big => {
            // Skip non-whitespace
            if current_kind != CharKind::Whitespace {
                while idx < chars.len() && !chars[idx].is_whitespace() {
                    idx += 1;
                }
            }
        }
    }

    // Skip whitespace
    while idx < chars.len() && chars[idx].is_whitespace() {
        idx += 1;
    }

    idx
}

/// Find the end of the next word.
///
/// Searches forward from `pos` to find the end of the next word.
/// If already at a word end, moves to the end of the following word.
///
/// # Arguments
///
/// * `chars` - The character slice to search
/// * `pos` - The starting position
/// * `word_type` - Whether to use small or big word semantics
///
/// # Returns
///
/// The index of the last character of the next word.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn next_word_end(chars: &[char], pos: usize, word_type: WordType) -> usize {
    if chars.is_empty() {
        return 0;
    }

    let mut idx = pos.min(chars.len() - 1);

    // If not at end of current word, go to end of current word
    if idx < chars.len() - 1 {
        let current_kind = char_kind(chars[idx]);
        let next_kind = char_kind(chars[idx + 1]);

        let same_word = match word_type {
            WordType::Small => current_kind == next_kind && current_kind != CharKind::Whitespace,
            WordType::Big => {
                current_kind != CharKind::Whitespace && next_kind != CharKind::Whitespace
            }
        };

        if same_word {
            // Move to end of current word
            return word_end(chars, idx, word_type);
        }
    }

    // Move to start of next word, then find its end
    idx += 1;
    while idx < chars.len() && chars[idx].is_whitespace() {
        idx += 1;
    }

    if idx >= chars.len() {
        return chars.len().saturating_sub(1);
    }

    word_end(chars, idx, word_type)
}

#[cfg(test)]
#[path = "word_tests.rs"]
mod tests;
