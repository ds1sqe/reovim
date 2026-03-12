//! Text object calculations for operator-pending mode.
//!
//! Text objects define regions of text for operators like delete (d), yank (y),
//! and change (c). They come in two scopes:
//! - **Inner**: The content without delimiters/whitespace (e.g., `iw`, `i(`)
//! - **Around**: The content including delimiters/whitespace (e.g., `aw`, `a(`)

use crate::mm::{Buffer, Position};

use super::direction::WordBoundary;

/// Text object types for operator-pending mode.
///
/// Text objects define regions of text that operators act upon.
/// Each text object has inner (i) and around (a) variants.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// // Delete inner word: diw
/// let inner_word = TextObject::InnerWord(WordBoundary::Word);
///
/// // Yank around parentheses: ya(
/// let around_paren = TextObject::ABracket('(');
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObject {
    /// Inner word (iw, iW)
    InnerWord(WordBoundary),
    /// A word including surrounding whitespace (aw, aW)
    AWord(WordBoundary),
    /// Inner paragraph (ip)
    InnerParagraph,
    /// A paragraph including surrounding blank lines (ap)
    AParagraph,
    /// Inner quotes (i", i', i\`)
    InnerQuote(char),
    /// Around quotes including the quote characters (a", a', a\`)
    AQuote(char),
    /// Inner bracket (i(, i[, i{, i<)
    InnerBracket(char),
    /// Around bracket including the brackets (a(, a[, a{, a<)
    ABracket(char),
}

impl TextObject {
    /// Check if this text object is an "inner" variant.
    #[must_use]
    pub const fn is_inner(&self) -> bool {
        matches!(
            self,
            Self::InnerWord(_) | Self::InnerParagraph | Self::InnerQuote(_) | Self::InnerBracket(_)
        )
    }

    /// Check if this text object is an "around" variant.
    #[must_use]
    pub const fn is_around(&self) -> bool {
        !self.is_inner()
    }

    /// Parse a text object from scope character and object character.
    ///
    /// # Arguments
    ///
    /// * `scope` - 'i' for inner, 'a' for around
    /// * `object` - The object character (w, W, (, [, {, ", ', etc.)
    ///
    /// # Returns
    ///
    /// `Some(TextObject)` if valid, `None` otherwise.
    #[must_use]
    pub const fn from_chars(scope: char, object: char) -> Option<Self> {
        let is_inner = match scope {
            'i' => true,
            'a' => false,
            _ => return None,
        };

        match object {
            'w' => Some(if is_inner {
                Self::InnerWord(WordBoundary::Word)
            } else {
                Self::AWord(WordBoundary::Word)
            }),
            'W' => Some(if is_inner {
                Self::InnerWord(WordBoundary::BigWord)
            } else {
                Self::AWord(WordBoundary::BigWord)
            }),
            'p' => Some(if is_inner {
                Self::InnerParagraph
            } else {
                Self::AParagraph
            }),
            '(' | ')' | 'b' => Some(if is_inner {
                Self::InnerBracket('(')
            } else {
                Self::ABracket('(')
            }),
            '[' | ']' => Some(if is_inner {
                Self::InnerBracket('[')
            } else {
                Self::ABracket('[')
            }),
            '{' | '}' | 'B' => Some(if is_inner {
                Self::InnerBracket('{')
            } else {
                Self::ABracket('{')
            }),
            '<' | '>' => Some(if is_inner {
                Self::InnerBracket('<')
            } else {
                Self::ABracket('<')
            }),
            '"' => Some(if is_inner {
                Self::InnerQuote('"')
            } else {
                Self::AQuote('"')
            }),
            '\'' => Some(if is_inner {
                Self::InnerQuote('\'')
            } else {
                Self::AQuote('\'')
            }),
            '`' => Some(if is_inner {
                Self::InnerQuote('`')
            } else {
                Self::AQuote('`')
            }),
            _ => None,
        }
    }
}

/// Text object calculation engine.
///
/// Provides pure calculations for text object ranges without modifying any state.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let buffer = Buffer::from_string("hello world");
/// let pos = Position::new(0, 0);
///
/// let range = TextObjectEngine::range(
///     &buffer,
///     pos,
///     TextObject::InnerWord(WordBoundary::Word),
///     1,
/// );
///
/// assert_eq!(range, Some((Position::new(0, 0), Position::new(0, 4))));
/// ```
pub struct TextObjectEngine;

impl TextObjectEngine {
    /// Calculate the range for a text object.
    ///
    /// Returns `(start, end)` positions for the text object, where both
    /// positions are inclusive.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to calculate in
    /// * `position` - Current cursor position
    /// * `text_object` - The text object to calculate
    /// * `count` - Number of text objects (for nested brackets, etc.)
    ///
    /// # Returns
    ///
    /// `Some((start, end))` if a valid range was found, `None` otherwise.
    #[must_use]
    pub fn range(
        buffer: &Buffer,
        position: Position,
        text_object: TextObject,
        count: usize,
    ) -> Option<(Position, Position)> {
        let count = count.max(1);

        match text_object {
            TextObject::InnerWord(boundary) => Self::inner_word(buffer, position, boundary),
            TextObject::AWord(boundary) => Self::a_word(buffer, position, boundary),
            TextObject::InnerParagraph => Self::inner_paragraph(buffer, position),
            TextObject::AParagraph => Self::a_paragraph(buffer, position),
            TextObject::InnerQuote(quote) => Self::inner_quote(buffer, position, quote),
            TextObject::AQuote(quote) => Self::a_quote(buffer, position, quote),
            TextObject::InnerBracket(bracket) => {
                Self::inner_bracket(buffer, position, bracket, count)
            }
            TextObject::ABracket(bracket) => Self::a_bracket(buffer, position, bracket, count),
        }
    }

    // === Word Text Objects ===

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn inner_word(
        buffer: &Buffer,
        pos: Position,
        boundary: WordBoundary,
    ) -> Option<(Position, Position)> {
        let line = buffer.line(pos.line)?;
        let chars: Vec<char> = line.chars().collect();

        if chars.is_empty() {
            return Some((pos, pos));
        }

        let col = pos.column.min(chars.len().saturating_sub(1));
        let current_char = chars.get(col)?;

        // Determine what kind of "word" we're in
        let is_word_char = boundary.is_word_char(*current_char);
        let is_whitespace = current_char.is_whitespace();

        // Find boundaries
        let mut start = col;
        let mut end = col;

        if is_whitespace {
            // Select whitespace run
            while start > 0 && chars.get(start - 1).is_some_and(|c| c.is_whitespace()) {
                start -= 1;
            }
            while end + 1 < chars.len() && chars.get(end + 1).is_some_and(|c| c.is_whitespace()) {
                end += 1;
            }
        } else if boundary == WordBoundary::Word {
            // For small word, separate word chars from punctuation
            if is_word_char {
                while start > 0
                    && chars
                        .get(start - 1)
                        .is_some_and(|c| boundary.is_word_char(*c))
                {
                    start -= 1;
                }
                while end + 1 < chars.len()
                    && chars
                        .get(end + 1)
                        .is_some_and(|c| boundary.is_word_char(*c))
                {
                    end += 1;
                }
            } else {
                // Punctuation
                while start > 0
                    && chars
                        .get(start - 1)
                        .is_some_and(|c| !c.is_whitespace() && !boundary.is_word_char(*c))
                {
                    start -= 1;
                }
                while end + 1 < chars.len()
                    && chars
                        .get(end + 1)
                        .is_some_and(|c| !c.is_whitespace() && !boundary.is_word_char(*c))
                {
                    end += 1;
                }
            }
        } else {
            // BigWord: any non-whitespace
            while start > 0 && chars.get(start - 1).is_some_and(|c| !c.is_whitespace()) {
                start -= 1;
            }
            while end + 1 < chars.len() && chars.get(end + 1).is_some_and(|c| !c.is_whitespace()) {
                end += 1;
            }
        }

        Some((Position::new(pos.line, start), Position::new(pos.line, end)))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn a_word(
        buffer: &Buffer,
        pos: Position,
        boundary: WordBoundary,
    ) -> Option<(Position, Position)> {
        let (inner_start, inner_end) = Self::inner_word(buffer, pos, boundary)?;
        let line = buffer.line(pos.line)?;
        let chars: Vec<char> = line.chars().collect();

        let mut start = inner_start.column;
        let mut end = inner_end.column;

        // Try to include trailing whitespace first
        let mut has_trailing = false;
        while end + 1 < chars.len() && chars.get(end + 1).is_some_and(|c| c.is_whitespace()) {
            end += 1;
            has_trailing = true;
        }

        // If no trailing whitespace, include leading whitespace
        if !has_trailing {
            while start > 0 && chars.get(start - 1).is_some_and(|c| c.is_whitespace()) {
                start -= 1;
            }
        }

        Some((Position::new(pos.line, start), Position::new(pos.line, end)))
    }

    // === Paragraph Text Objects ===

    fn inner_paragraph(buffer: &Buffer, pos: Position) -> Option<(Position, Position)> {
        let line_count = buffer.line_count();
        if line_count == 0 {
            return None;
        }

        let current_line = buffer.line(pos.line)?;
        let is_empty_line = current_line.trim().is_empty();

        let mut start = pos.line;
        let mut end = pos.line;

        // Expand selection based on whether we're on empty or non-empty lines
        let predicate = |l: &str| {
            if is_empty_line {
                l.trim().is_empty()
            } else {
                !l.trim().is_empty()
            }
        };

        while start > 0 && buffer.line(start - 1).is_some_and(&predicate) {
            start -= 1;
        }
        while end + 1 < line_count && buffer.line(end + 1).is_some_and(&predicate) {
            end += 1;
        }

        let end_col = buffer.line_len(end).unwrap_or(0).saturating_sub(1);
        Some((Position::new(start, 0), Position::new(end, end_col)))
    }

    fn a_paragraph(buffer: &Buffer, pos: Position) -> Option<(Position, Position)> {
        let (inner_start, inner_end) = Self::inner_paragraph(buffer, pos)?;
        let line_count = buffer.line_count();

        let mut start = inner_start.line;
        let mut end = inner_end.line;

        // Include trailing blank lines
        while end + 1 < line_count && buffer.line(end + 1).is_some_and(|l| l.trim().is_empty()) {
            end += 1;
        }

        // If no trailing blank lines, include leading blank lines
        if end == inner_end.line {
            while start > 0 && buffer.line(start - 1).is_some_and(|l| l.trim().is_empty()) {
                start -= 1;
            }
        }

        let end_col = buffer.line_len(end).unwrap_or(0).saturating_sub(1);
        Some((Position::new(start, 0), Position::new(end, end_col)))
    }

    // === Quote Text Objects ===

    fn inner_quote(buffer: &Buffer, pos: Position, quote: char) -> Option<(Position, Position)> {
        let line = buffer.line(pos.line)?;
        let chars: Vec<char> = line.chars().collect();

        // Find quote boundaries on current line
        let (open, close) = Self::find_quote_pair(&chars, pos.column, quote)?;

        // Inner: exclude the quotes
        let start = open + 1;
        let end = close.saturating_sub(1);

        if start > end {
            // Empty quotes
            Some((Position::new(pos.line, start), Position::new(pos.line, start)))
        } else {
            Some((Position::new(pos.line, start), Position::new(pos.line, end)))
        }
    }

    fn a_quote(buffer: &Buffer, pos: Position, quote: char) -> Option<(Position, Position)> {
        let line = buffer.line(pos.line)?;
        let chars: Vec<char> = line.chars().collect();

        let (open, close) = Self::find_quote_pair(&chars, pos.column, quote)?;

        Some((Position::new(pos.line, open), Position::new(pos.line, close)))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn find_quote_pair(chars: &[char], col: usize, quote: char) -> Option<(usize, usize)> {
        // Find all quote positions on the line
        let quotes: Vec<usize> = chars
            .iter()
            .enumerate()
            .filter(|&(_, c)| *c == quote)
            .map(|(i, _)| i)
            .collect();

        if quotes.len() < 2 {
            return None;
        }

        // Find the pair that contains or is nearest to cursor
        for pair in quotes.chunks(2) {
            if pair.len() == 2 {
                let (open, close) = (pair[0], pair[1]);
                if col >= open && col <= close {
                    return Some((open, close));
                }
            }
        }

        // If cursor is before first quote, use first pair
        if col < quotes[0] && quotes.len() >= 2 {
            return Some((quotes[0], quotes[1]));
        }

        // If cursor is after last quote, use last pair
        if quotes.len() >= 2 && col > quotes[quotes.len() - 1] {
            let len = quotes.len();
            return Some((quotes[len - 2], quotes[len - 1]));
        }

        None
    }

    // === Bracket Text Objects ===

    fn inner_bracket(
        buffer: &Buffer,
        pos: Position,
        bracket: char,
        count: usize,
    ) -> Option<(Position, Position)> {
        let (open, close) = Self::get_bracket_pair(bracket)?;
        let (open_pos, close_pos) = Self::find_bracket_pair(buffer, pos, open, close, count)?;

        // Inner: exclude the brackets
        // Move open_pos forward past the bracket
        let start = Self::next_position(buffer, open_pos)?;
        // Move close_pos backward before the bracket
        let end = Self::prev_position(buffer, close_pos)?;

        if start > end {
            // Empty brackets - return position after opening bracket
            Some((start, start))
        } else {
            Some((start, end))
        }
    }

    fn a_bracket(
        buffer: &Buffer,
        pos: Position,
        bracket: char,
        count: usize,
    ) -> Option<(Position, Position)> {
        let (open, close) = Self::get_bracket_pair(bracket)?;
        Self::find_bracket_pair(buffer, pos, open, close, count)
    }

    const fn get_bracket_pair(bracket: char) -> Option<(char, char)> {
        match bracket {
            '(' | ')' => Some(('(', ')')),
            '[' | ']' => Some(('[', ']')),
            '{' | '}' => Some(('{', '}')),
            '<' | '>' => Some(('<', '>')),
            _ => None,
        }
    }

    fn find_bracket_pair(
        buffer: &Buffer,
        pos: Position,
        open: char,
        close: char,
        count: usize,
    ) -> Option<(Position, Position)> {
        // Find the opening bracket (searching backward and at cursor)
        let open_pos = Self::find_opening_bracket(buffer, pos, open, close, count)?;

        // Find the closing bracket (searching forward from opening)
        let close_pos = Self::find_closing_bracket(buffer, open_pos, open, close)?;

        Some((open_pos, close_pos))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn find_opening_bracket(
        buffer: &Buffer,
        pos: Position,
        open: char,
        close: char,
        count: usize,
    ) -> Option<Position> {
        let mut depth: isize = 0;
        let mut found_count = 0;
        let mut line_idx = pos.line;
        let mut last_open = None;

        // First check at and before current position on current line
        if let Some(line) = buffer.line(line_idx) {
            let chars: Vec<char> = line.chars().collect();
            let start_col = pos.column.min(chars.len().saturating_sub(1));

            for col in (0..=start_col).rev() {
                if let Some(&c) = chars.get(col) {
                    if c == close {
                        depth += 1;
                    } else if c == open {
                        if depth > 0 {
                            depth -= 1;
                        } else {
                            found_count += 1;
                            last_open = Some(Position::new(line_idx, col));
                            if found_count >= count {
                                return last_open;
                            }
                        }
                    }
                }
            }
        }

        // Search previous lines
        while line_idx > 0 {
            line_idx -= 1;
            if let Some(line) = buffer.line(line_idx) {
                let chars: Vec<char> = line.chars().collect();
                for col in (0..chars.len()).rev() {
                    if let Some(&c) = chars.get(col) {
                        if c == close {
                            depth += 1;
                        } else if c == open {
                            if depth > 0 {
                                depth -= 1;
                            } else {
                                found_count += 1;
                                last_open = Some(Position::new(line_idx, col));
                                if found_count >= count {
                                    return last_open;
                                }
                            }
                        }
                    }
                }
            }
        }

        last_open
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn find_closing_bracket(
        buffer: &Buffer,
        open_pos: Position,
        open: char,
        close: char,
    ) -> Option<Position> {
        let mut depth = 1;
        let mut line_idx = open_pos.line;
        let mut col = open_pos.column + 1;

        while line_idx < buffer.line_count() {
            if let Some(line) = buffer.line(line_idx) {
                let chars: Vec<char> = line.chars().collect();

                while col < chars.len() {
                    let c = chars[col];
                    if c == open {
                        depth += 1;
                    } else if c == close {
                        depth -= 1;
                        if depth == 0 {
                            return Some(Position::new(line_idx, col));
                        }
                    }
                    col += 1;
                }
            }

            line_idx += 1;
            col = 0;
        }

        None
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn next_position(buffer: &Buffer, pos: Position) -> Option<Position> {
        let line_len = buffer.line_len(pos.line)?;
        if pos.column + 1 < line_len {
            Some(Position::new(pos.line, pos.column + 1))
        } else if pos.line + 1 < buffer.line_count() {
            Some(Position::new(pos.line + 1, 0))
        } else {
            Some(Position::new(pos.line, pos.column))
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn prev_position(buffer: &Buffer, pos: Position) -> Option<Position> {
        if pos.column > 0 {
            Some(Position::new(pos.line, pos.column - 1))
        } else if pos.line > 0 {
            let prev_len = buffer.line_len(pos.line - 1)?;
            Some(Position::new(pos.line - 1, prev_len.saturating_sub(1)))
        } else {
            Some(Position::new(0, 0))
        }
    }
}

