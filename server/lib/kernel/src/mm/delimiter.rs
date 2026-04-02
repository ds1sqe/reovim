//! Delimiter matching for bracket pairs and quotes.
//!
//! This module provides functions for finding matching delimiter pairs
//! such as parentheses, brackets, braces, and quotes. It supports both
//! symmetric delimiters (quotes) and asymmetric delimiters (brackets).
//!
//! # Design Philosophy
//!
//! Following the kernel "mechanism, not policy" principle:
//! - Pure functions operating on any [`TextGeometry`] implementor
//! - No text object semantics (that's policy in modules)
//! - No escape sequence handling (that's syntax-aware policy)
//!
//! # Delimiter Types
//!
//! - **Asymmetric**: `()`, `[]`, `{}`, `<>` - Requires depth tracking
//! - **Symmetric**: `""`, `''`, ``` `` ``` - Paired by position
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//! # use reovim_types_text::{Position, SimpleText};
//!
//! let buffer = SimpleText::new("fn foo(bar, baz) {}");
//!
//! // Find matching parens around position (0, 10) - inside the parens
//! let result = find_delimiter_pair(&buffer, Position::new(0, 10), '(', ')');
//! assert!(result.is_some());
//! let (open, close) = result.unwrap();
//! assert_eq!(open, Position::new(0, 6));
//! assert_eq!(close, Position::new(0, 15));
//! ```

use reovim_types_text::{Position, TextGeometry};

/// Find matching delimiter pair containing the given position.
///
/// Searches for the innermost delimiter pair that contains the cursor
/// position. For asymmetric delimiters like `()`, uses depth counting.
/// For symmetric delimiters like `""`, pairs by position within the line.
///
/// # Arguments
///
/// * `text` - Any [`TextGeometry`] implementor to search
/// * `pos` - The position to search around
/// * `open` - The opening delimiter character
/// * `close` - The closing delimiter character
///
/// # Returns
///
/// `Some((open_pos, close_pos))` if a matching pair is found,
/// `None` otherwise.
///
/// # Examples
///
/// ```
/// use reovim_kernel::api::v1::*;
/// # use reovim_types_text::{Position, SimpleText};
///
/// let buffer = SimpleText::new("(hello)");
/// let result = find_delimiter_pair(&buffer, Position::new(0, 3), '(', ')');
/// assert!(result.is_some());
/// ```
#[must_use]
pub fn find_delimiter_pair(
    text: &dyn TextGeometry,
    pos: Position,
    open: char,
    close: char,
) -> Option<(Position, Position)> {
    if text.is_empty() {
        return None;
    }

    let is_symmetric = open == close;
    if is_symmetric {
        find_symmetric_pair(text, pos, open)
    } else {
        find_asymmetric_pair(text, pos, open, close)
    }
}

/// Find matching symmetric delimiter pair (quotes).
///
/// For symmetric delimiters, pairs are determined by position:
/// the 1st and 2nd occurrence form a pair, 3rd and 4th, etc.
/// This only searches within the current line.
fn find_symmetric_pair(
    text: &dyn TextGeometry,
    pos: Position,
    quote: char,
) -> Option<(Position, Position)> {
    let line = text.line(pos.line)?;
    let chars: Vec<char> = line.chars().collect();

    // Find all quote positions on current line
    let quote_positions: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|&(_, c)| *c == quote)
        .map(|(i, _)| i)
        .collect();

    // Need at least 2 quotes to form a pair
    if quote_positions.len() < 2 {
        return None;
    }

    // Find pair containing cursor
    // Quotes are paired: 0-1, 2-3, 4-5, etc.
    let mut i = 0;
    while i + 1 < quote_positions.len() {
        let open_idx = quote_positions[i];
        let close_idx = quote_positions[i + 1];

        if pos.column >= open_idx && pos.column <= close_idx {
            return Some((Position::new(pos.line, open_idx), Position::new(pos.line, close_idx)));
        }
        i += 2;
    }

    None
}

/// Find matching asymmetric delimiter pair (brackets).
///
/// Uses depth counting to find the innermost matching pair.
/// Searches backward for the opening delimiter and forward for
/// the closing delimiter.
///
/// Handles the edge case when the cursor is positioned ON a delimiter
/// character by adjusting the search start positions.
fn find_asymmetric_pair(
    text: &dyn TextGeometry,
    pos: Position,
    open: char,
    close: char,
) -> Option<(Position, Position)> {
    // Check if we're at a delimiter character
    let line = text.line(pos.line)?;
    let chars: Vec<char> = line.chars().collect();
    let current_char = chars.get(pos.column).copied();

    match current_char {
        Some(c) if c == open => {
            // At opening delimiter: this is the open, search forward for close
            let close_pos =
                find_forward(text, Position::new(pos.line, pos.column + 1), open, close)?;
            Some((pos, close_pos))
        }
        Some(c) if c == close => {
            // At closing delimiter: search backward for open, this is the close
            let open_pos = find_backward(
                text,
                Position::new(pos.line, pos.column.saturating_sub(1)),
                open,
                close,
            )?;
            Some((open_pos, pos))
        }
        _ => {
            // Inside delimiters: search both directions
            let open_pos = find_backward(text, pos, open, close)?;
            let close_pos = find_forward(text, pos, open, close)?;
            Some((open_pos, close_pos))
        }
    }
}

/// Search backward for opening delimiter.
#[cfg_attr(coverage_nightly, coverage(off))]
fn find_backward(
    text: &dyn TextGeometry,
    pos: Position,
    open: char,
    close: char,
) -> Option<Position> {
    let mut y = pos.line;
    let mut x = pos.column;
    let mut depth = 0i32;

    loop {
        if let Some(line) = text.line(y) {
            let chars: Vec<char> = line.chars().collect();
            let start = x.min(chars.len().saturating_sub(1));

            // Search backward through this line
            for i in (0..=start).rev() {
                if i >= chars.len() {
                    continue;
                }
                let c = chars[i];
                if c == close {
                    depth += 1;
                } else if c == open {
                    if depth == 0 {
                        return Some(Position::new(y, i));
                    }
                    depth -= 1;
                }
            }
        }

        if y == 0 {
            break;
        }
        y -= 1;
        x = text.line_len(y).unwrap_or(0);
    }
    None
}

/// Search forward for closing delimiter.
#[cfg_attr(coverage_nightly, coverage(off))]
fn find_forward(
    text: &dyn TextGeometry,
    pos: Position,
    open: char,
    close: char,
) -> Option<Position> {
    let mut y = pos.line;
    let mut x = pos.column;
    let mut depth = 0i32;
    let line_count = text.line_count();

    loop {
        if let Some(line) = text.line(y) {
            let chars: Vec<char> = line.chars().collect();
            let start = if y == pos.line { x } else { 0 };

            // Search forward through this line
            for (i, &c) in chars.iter().enumerate().skip(start) {
                if c == open {
                    depth += 1;
                } else if c == close {
                    if depth == 0 {
                        return Some(Position::new(y, i));
                    }
                    depth -= 1;
                }
            }
        }

        y += 1;
        if y >= line_count {
            break;
        }
        x = 0;
    }
    None
}

/// Find the matching delimiter for a delimiter at the given position.
///
/// If the cursor is ON a delimiter character, finds its match.
/// Returns `None` if the position is not on a delimiter or no match exists.
///
/// # Arguments
///
/// * `text` - Any [`TextGeometry`] implementor to search
/// * `pos` - The position of the delimiter
///
/// # Returns
///
/// The position of the matching delimiter, or `None`.
#[must_use]
pub fn find_matching_delimiter(text: &dyn TextGeometry, pos: Position) -> Option<Position> {
    let line = text.line(pos.line)?;
    let chars: Vec<char> = line.chars().collect();
    let c = *chars.get(pos.column)?;

    // Start search from position+1 for opening delimiters (skip the delimiter itself)
    // For closing delimiters, search backward from position-1
    match c {
        '(' => find_forward(text, Position::new(pos.line, pos.column + 1), '(', ')'),
        ')' => find_backward(text, Position::new(pos.line, pos.column.saturating_sub(1)), '(', ')'),
        '[' => find_forward(text, Position::new(pos.line, pos.column + 1), '[', ']'),
        ']' => find_backward(text, Position::new(pos.line, pos.column.saturating_sub(1)), '[', ']'),
        '{' => find_forward(text, Position::new(pos.line, pos.column + 1), '{', '}'),
        '}' => find_backward(text, Position::new(pos.line, pos.column.saturating_sub(1)), '{', '}'),
        '<' => find_forward(text, Position::new(pos.line, pos.column + 1), '<', '>'),
        '>' => find_backward(text, Position::new(pos.line, pos.column.saturating_sub(1)), '<', '>'),
        _ => None,
    }
}
