//! Auto-pair bracket insertion logic.
//!
//! When an opening bracket is typed, this module auto-inserts the closing bracket
//! with the cursor positioned between them.
//!
//! # Supported Pairs
//!
//! | Open | Close | Type |
//! |------|-------|------|
//! | `(`  | `)`   | Asymmetric |
//! | `[`  | `]`   | Asymmetric |
//! | `{`  | `}`   | Asymmetric |
//! | `` ` `` | `` ` `` | Symmetric |
//! | `'`  | `'`   | Symmetric |
//! | `"`  | `"`   | Symmetric |
//!
//! # Symmetric Pair Handling
//!
//! Symmetric pairs (backtick, quotes) require special tracking to prevent
//! infinite recursion. When we auto-insert a symmetric closing character,
//! we track it so that when the `BufferModified` event fires for that
//! insertion, we know to skip auto-pairing.

use std::sync::atomic::{AtomicU8, Ordering};

/// Tracks the last auto-inserted symmetric character to prevent infinite recursion.
///
/// Uses a simple encoding:
/// - 0 = none
/// - 1 = backtick
/// - 2 = single quote
/// - 3 = double quote
static LAST_AUTO_INSERT: AtomicU8 = AtomicU8::new(0);

/// Map a symmetric character to its tracking code.
const fn char_to_code(c: char) -> u8 {
    match c {
        '`' => 1,
        '\'' => 2,
        '"' => 3,
        _ => 0,
    }
}

/// Check if the given character was just auto-inserted by us.
///
/// This atomically clears the tracking state, so it can only return `true` once
/// per auto-insertion.
pub fn is_last_auto_insert(c: char) -> bool {
    let code = char_to_code(c);
    code != 0 && LAST_AUTO_INSERT.swap(0, Ordering::SeqCst) == code
}

/// Mark that we're about to auto-insert a symmetric character.
///
/// Call this before inserting the closing bracket for symmetric pairs.
pub fn mark_auto_insert(c: char) {
    let code = char_to_code(c);
    if code != 0 {
        LAST_AUTO_INSERT.store(code, Ordering::SeqCst);
    }
}

/// Result of checking if auto-pair should trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoPairResult {
    /// Insert the closing bracket.
    Insert {
        /// The closing character to insert.
        close: char,
        /// Whether this is a symmetric pair (same open and close).
        symmetric: bool,
    },
    /// Skip over an existing closing bracket (don't insert, just move cursor).
    SkipOver,
    /// Skip auto-pairing (not an opening bracket or was our own insertion).
    Skip,
}

/// Check if the inserted text should trigger auto-pairing.
///
/// Returns `AutoPairResult::Insert` if a closing bracket should be inserted,
/// or `AutoPairResult::Skip` if no action is needed.
///
/// # Arguments
///
/// * `text` - The text that was just inserted into the buffer.
///
/// # Examples
///
/// ```ignore
/// use reovim_module_pair::auto_pair::{should_auto_pair, AutoPairResult};
///
/// assert_eq!(
///     should_auto_pair("("),
///     AutoPairResult::Insert { close: ')', symmetric: false }
/// );
/// assert_eq!(should_auto_pair("a"), AutoPairResult::Skip);
/// ```
#[must_use]
pub fn should_auto_pair(text: &str) -> AutoPairResult {
    // Only handle single-character insertions
    let mut chars = text.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return AutoPairResult::Skip;
    };

    // For symmetric pairs, check if this is our own auto-inserted character
    if is_symmetric(c) && is_last_auto_insert(c) {
        return AutoPairResult::Skip;
    }

    match c {
        '(' => AutoPairResult::Insert {
            close: ')',
            symmetric: false,
        },
        '[' => AutoPairResult::Insert {
            close: ']',
            symmetric: false,
        },
        '{' => AutoPairResult::Insert {
            close: '}',
            symmetric: false,
        },
        '`' => AutoPairResult::Insert {
            close: '`',
            symmetric: true,
        },
        '\'' => AutoPairResult::Insert {
            close: '\'',
            symmetric: true,
        },
        '"' => AutoPairResult::Insert {
            close: '"',
            symmetric: true,
        },
        // Note: < is intentionally excluded (ambiguous with less-than operator)
        _ => AutoPairResult::Skip,
    }
}

/// Check if a character is a symmetric pair (same open and close).
const fn is_symmetric(c: char) -> bool {
    matches!(c, '`' | '\'' | '"')
}

/// Get the closing bracket for an opening bracket.
///
/// Returns `None` if the character is not a supported opening bracket.
#[must_use]
pub const fn get_closing_bracket(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '`' => Some('`'),
        '\'' => Some('\''),
        '"' => Some('"'),
        _ => None,
    }
}

/// Check if a character is an opening bracket.
#[must_use]
pub const fn is_opening_bracket(c: char) -> bool {
    matches!(c, '(' | '[' | '{' | '`' | '\'' | '"')
}

/// Check if a character is a closing bracket.
#[must_use]
pub const fn is_closing_bracket(c: char) -> bool {
    matches!(c, ')' | ']' | '}' | '`' | '\'' | '"')
}

/// Check if we should skip over a closing bracket instead of inserting.
///
/// Returns `true` if:
/// - The typed character is a closing bracket
/// - The character at the cursor position (before insert) is the same
///
/// # Arguments
///
/// * `typed` - The character that was just typed
/// * `char_at_cursor` - The character that was at the cursor position before the insert
#[must_use]
pub fn should_skip_over(typed: char, char_at_cursor: Option<char>) -> bool {
    is_closing_bracket(typed) && char_at_cursor == Some(typed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_auto_pair_parentheses() {
        assert_eq!(
            should_auto_pair("("),
            AutoPairResult::Insert {
                close: ')',
                symmetric: false
            }
        );
    }

    #[test]
    fn test_should_auto_pair_brackets() {
        assert_eq!(
            should_auto_pair("["),
            AutoPairResult::Insert {
                close: ']',
                symmetric: false
            }
        );
    }

    #[test]
    fn test_should_auto_pair_braces() {
        assert_eq!(
            should_auto_pair("{"),
            AutoPairResult::Insert {
                close: '}',
                symmetric: false
            }
        );
    }

    #[test]
    fn test_should_auto_pair_backtick() {
        // Reset state first
        LAST_AUTO_INSERT.store(0, Ordering::SeqCst);

        assert_eq!(
            should_auto_pair("`"),
            AutoPairResult::Insert {
                close: '`',
                symmetric: true
            }
        );
    }

    #[test]
    fn test_should_auto_pair_single_quote() {
        LAST_AUTO_INSERT.store(0, Ordering::SeqCst);

        assert_eq!(
            should_auto_pair("'"),
            AutoPairResult::Insert {
                close: '\'',
                symmetric: true
            }
        );
    }

    #[test]
    fn test_should_auto_pair_double_quote() {
        LAST_AUTO_INSERT.store(0, Ordering::SeqCst);

        assert_eq!(
            should_auto_pair("\""),
            AutoPairResult::Insert {
                close: '"',
                symmetric: true
            }
        );
    }

    #[test]
    fn test_should_skip_regular_text() {
        assert_eq!(should_auto_pair("a"), AutoPairResult::Skip);
        assert_eq!(should_auto_pair("hello"), AutoPairResult::Skip);
        assert_eq!(should_auto_pair(""), AutoPairResult::Skip);
    }

    #[test]
    fn test_should_skip_angle_brackets() {
        // < and > are intentionally NOT auto-paired
        assert_eq!(should_auto_pair("<"), AutoPairResult::Skip);
        assert_eq!(should_auto_pair(">"), AutoPairResult::Skip);
    }

    #[test]
    fn test_symmetric_pair_tracking() {
        // Reset state
        LAST_AUTO_INSERT.store(0, Ordering::SeqCst);

        // First backtick should trigger auto-pair
        assert_eq!(
            should_auto_pair("`"),
            AutoPairResult::Insert {
                close: '`',
                symmetric: true
            }
        );

        // Mark that we auto-inserted a backtick
        mark_auto_insert('`');

        // Second backtick should be skipped (it's our auto-inserted one)
        assert_eq!(should_auto_pair("`"), AutoPairResult::Skip);

        // Third backtick should trigger again (tracking was cleared)
        assert_eq!(
            should_auto_pair("`"),
            AutoPairResult::Insert {
                close: '`',
                symmetric: true
            }
        );
    }

    #[test]
    fn test_closing_bracket_lookup() {
        assert_eq!(get_closing_bracket('('), Some(')'));
        assert_eq!(get_closing_bracket('['), Some(']'));
        assert_eq!(get_closing_bracket('{'), Some('}'));
        assert_eq!(get_closing_bracket('`'), Some('`'));
        assert_eq!(get_closing_bracket('\''), Some('\''));
        assert_eq!(get_closing_bracket('"'), Some('"'));
        assert_eq!(get_closing_bracket('<'), None);
        assert_eq!(get_closing_bracket('a'), None);
    }

    #[test]
    fn test_is_opening_bracket() {
        assert!(is_opening_bracket('('));
        assert!(is_opening_bracket('['));
        assert!(is_opening_bracket('{'));
        assert!(is_opening_bracket('`'));
        assert!(is_opening_bracket('\''));
        assert!(is_opening_bracket('"'));
        assert!(!is_opening_bracket('<'));
        assert!(!is_opening_bracket('a'));
    }

    #[test]
    fn test_is_closing_bracket() {
        assert!(is_closing_bracket(')'));
        assert!(is_closing_bracket(']'));
        assert!(is_closing_bracket('}'));
        assert!(is_closing_bracket('`'));
        assert!(is_closing_bracket('\''));
        assert!(is_closing_bracket('"'));
        assert!(!is_closing_bracket('>'));
        assert!(!is_closing_bracket('a'));
    }
}
