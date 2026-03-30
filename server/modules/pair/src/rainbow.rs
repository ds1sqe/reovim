//! Rainbow bracket depth computation.
//!
//! Stack-based algorithm that assigns nesting depth to each bracket.
//! Independent stacks per pair type ensure `(` and `[` don't interfere.

use std::collections::HashMap;

use reovim_driver_syntax::BracketPair;

/// Information about a single bracket at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BracketInfo {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub col: usize,
    /// Nesting depth (0 = outermost). `usize::MAX` = unmatched.
    pub depth: usize,
    /// The bracket character.
    pub ch: char,
}

/// Compute bracket depths for the given content using language-specific pairs.
///
/// Returns a map from `(line, col)` to `BracketInfo` for every bracket found.
/// Uses independent stacks per pair type to correctly handle interleaved brackets.
///
/// # Panics
///
/// Panics if a bracket character matches a pair but its opener stack was not
/// initialized. This is a programming error — all pair openers are pre-inserted.
#[cfg_attr(coverage_nightly, coverage(off))]
#[must_use]
pub fn compute_bracket_depths(
    content: &str,
    rainbow_pairs: &[BracketPair],
) -> HashMap<(usize, usize), BracketInfo> {
    let mut result = HashMap::new();

    if rainbow_pairs.is_empty() {
        return result;
    }

    // Independent stack per pair type: maps open char -> Vec<(line, col)>
    let mut stacks: HashMap<char, Vec<(usize, usize)>> = HashMap::new();
    // Track depths for unmatched close resolution
    let mut depth_map: HashMap<(usize, usize), usize> = HashMap::new();

    for pair in rainbow_pairs {
        stacks.insert(pair.open, Vec::new());
    }

    let mut line = 0usize;
    let mut col = 0usize;

    for ch in content.chars() {
        if ch == '\n' {
            line += 1;
            col = 0;
            continue;
        }

        // Check if this char is an opener for any pair
        if let Some(pair) = rainbow_pairs
            .iter()
            .find(|p| p.open == ch && !p.is_symmetric())
        {
            let stack = stacks.get_mut(&pair.open).expect("stack initialized");
            let depth = stack.len();
            stack.push((line, col));
            depth_map.insert((line, col), depth);

            result.insert(
                (line, col),
                BracketInfo {
                    line,
                    col,
                    depth,
                    ch,
                },
            );
        }
        // Check if this char is a closer for any pair
        else if let Some(pair) = rainbow_pairs
            .iter()
            .find(|p| p.close == ch && !p.is_symmetric())
        {
            let stack = stacks.get_mut(&pair.open).expect("stack initialized");
            if let Some((open_line, open_col)) = stack.pop() {
                // Matched close: same depth as its opener
                let depth = depth_map.get(&(open_line, open_col)).copied().unwrap_or(0);
                result.insert(
                    (line, col),
                    BracketInfo {
                        line,
                        col,
                        depth,
                        ch,
                    },
                );
            } else {
                // Unmatched close
                result.insert(
                    (line, col),
                    BracketInfo {
                        line,
                        col,
                        depth: usize::MAX,
                        ch,
                    },
                );
            }
        }

        col += 1;
    }

    // Mark remaining unmatched openers
    for stack in stacks.values() {
        for &(open_line, open_col) in stack {
            if let Some(info) = result.get_mut(&(open_line, open_col)) {
                info.depth = usize::MAX;
            }
        }
    }

    result
}

#[cfg(test)]
#[path = "rainbow_tests.rs"]
mod tests;
