//! Matched-pair finding for cursor position.
//!
//! Finds the innermost bracket pair enclosing the cursor position.

use std::collections::HashMap;

use reovim_driver_syntax::BracketPair;

use crate::rainbow::BracketInfo;

/// A matched bracket pair (opening and closing positions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchedPair {
    /// Opening bracket info.
    pub open: BracketInfo,
    /// Closing bracket info.
    pub close: BracketInfo,
}

/// Find the innermost matched pair enclosing the cursor position.
///
/// Iterates all bracket pairs in the result, finds pairs that contain the
/// cursor position, and returns the smallest-range pair (innermost).
#[must_use]
#[allow(clippy::implicit_hasher)]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn find_innermost_pair(
    brackets: &HashMap<(usize, usize), BracketInfo>,
    highlight_pairs: &[BracketPair],
    cursor_line: usize,
    cursor_col: usize,
) -> Option<MatchedPair> {
    let mut best: Option<MatchedPair> = None;
    let mut best_span = usize::MAX;

    // Group openers by pair type
    for pair_def in highlight_pairs {
        if pair_def.is_symmetric() {
            continue;
        }

        // Collect matched opener/closer entries for this pair type
        let openers: Vec<&BracketInfo> = brackets
            .values()
            .filter(|b| b.ch == pair_def.open && b.depth != usize::MAX)
            .collect();

        for opener in &openers {
            // Find the matching closer at the same depth
            #[allow(clippy::suspicious_operation_groupings)]
            let closer = brackets.values().find(|b| {
                b.ch == pair_def.close
                    && b.depth == opener.depth
                    && (b.line > opener.line || (b.line == opener.line && b.col > opener.col))
            });

            let Some(closer) = closer else {
                continue;
            };

            // Check if cursor is inside this pair (inclusive of bracket positions)
            let cursor_after_open = cursor_line > opener.line
                || (cursor_line == opener.line && cursor_col >= opener.col);
            let cursor_before_close = cursor_line < closer.line
                || (cursor_line == closer.line && cursor_col <= closer.col);

            if cursor_after_open && cursor_before_close {
                // Compute span (simple line-based distance)
                let span =
                    (closer.line - opener.line) * 1000 + closer.col.saturating_sub(opener.col);

                if span < best_span {
                    best_span = span;
                    best = Some(MatchedPair {
                        open: **opener,
                        close: *closer,
                    });
                }
            }
        }
    }

    best
}

#[cfg(test)]
#[path = "matched_tests.rs"]
mod tests;
