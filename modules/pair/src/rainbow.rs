//! Rainbow bracket depth calculation.
//!
//! This module provides a simple stack-based algorithm to compute
//! bracket nesting depths. The algorithm is language-agnostic and
//! operates directly on text content.
//!
//! # Algorithm
//!
//! For each bracket type, we maintain a stack of opening bracket positions.
//! When we encounter an opening bracket, we push its position and assign
//! the current stack depth. When we encounter a closing bracket, we pop
//! the stack and assign the same depth as the matching opening bracket.
//!
//! Unmatched brackets (both opening and closing) are marked with
//! `depth = usize::MAX` as a sentinel value.
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_pair::rainbow::compute_bracket_depths;
//!
//! let content = "((a + b))";
//! let brackets = compute_bracket_depths(content);
//!
//! // Outer brackets have depth 0
//! assert_eq!(brackets.get(&(0, 0)).unwrap().depth, 0);
//! assert_eq!(brackets.get(&(0, 8)).unwrap().depth, 0);
//!
//! // Inner brackets have depth 1
//! assert_eq!(brackets.get(&(0, 1)).unwrap().depth, 1);
//! assert_eq!(brackets.get(&(0, 7)).unwrap().depth, 1);
//! ```

use std::collections::HashMap;

use crate::state::BracketInfo;

/// Bracket pair definition.
///
/// Note: `<` and `>` are intentionally excluded as they're commonly used in
/// operators (`->`, `=>`, `<=`, `>=`, `<<`, `>>`) rather than as brackets.
const BRACKET_PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

/// Compute bracket depths for all brackets in content.
///
/// Returns a map from (line, col) to `BracketInfo` containing depth information.
///
/// # Algorithm
///
/// For each bracket type, we maintain a stack of opening bracket positions.
/// When we encounter an opening bracket, we push its position and assign
/// the current stack depth. When we encounter a closing bracket, we pop
/// the stack and assign the same depth as the matching opening bracket.
///
/// Unmatched brackets are marked with `depth = usize::MAX`.
#[must_use]
pub fn compute_bracket_depths(content: &str) -> HashMap<(usize, usize), BracketInfo> {
    let mut result = HashMap::new();

    // Stacks for each bracket type, storing (line, col, depth)
    let mut stacks: [Vec<(usize, usize, usize)>; 3] = [Vec::new(), Vec::new(), Vec::new()];

    for (line_idx, line) in content.lines().enumerate() {
        for (col_idx, ch) in line.chars().enumerate() {
            // Check each bracket type
            for (pair_idx, (open, close)) in BRACKET_PAIRS.iter().enumerate() {
                if ch == *open {
                    // Opening bracket: push to stack with current depth
                    let depth = stacks[pair_idx].len();
                    stacks[pair_idx].push((line_idx, col_idx, depth));

                    result.insert(
                        (line_idx, col_idx),
                        BracketInfo {
                            line: line_idx,
                            col: col_idx,
                            depth,
                            char: ch,
                        },
                    );
                } else if ch == *close {
                    // Closing bracket: pop from stack and use same depth
                    if let Some((_open_line, _open_col, depth)) = stacks[pair_idx].pop() {
                        result.insert(
                            (line_idx, col_idx),
                            BracketInfo {
                                line: line_idx,
                                col: col_idx,
                                depth,
                                char: ch,
                            },
                        );
                    } else {
                        // Unmatched closing bracket - mark with special depth
                        result.insert(
                            (line_idx, col_idx),
                            BracketInfo {
                                line: line_idx,
                                col: col_idx,
                                depth: usize::MAX, // Marks unmatched
                                char: ch,
                            },
                        );
                    }
                }
            }
        }
    }

    // Mark remaining unmatched opening brackets
    for stack in &stacks {
        for (line, col, _depth) in stack {
            if let Some(info) = result.get_mut(&(*line, *col)) {
                info.depth = usize::MAX; // Marks unmatched
            }
        }
    }

    result
}

/// Simple content hash for cache invalidation.
///
/// Uses FNV-1a hash algorithm for fast, reasonable distribution.
#[must_use]
pub fn content_hash(content: &str) -> u64 {
    // Use FNV-1a hash
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in content.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_brackets() {
        let content = "(a + b)";
        let brackets = compute_bracket_depths(content);

        assert_eq!(brackets.len(), 2);
        assert_eq!(brackets.get(&(0, 0)).unwrap().depth, 0);
        assert_eq!(brackets.get(&(0, 6)).unwrap().depth, 0);
    }

    #[test]
    fn test_nested_brackets() {
        let content = "((a))";
        let brackets = compute_bracket_depths(content);

        assert_eq!(brackets.len(), 4);
        assert_eq!(brackets.get(&(0, 0)).unwrap().depth, 0); // outer (
        assert_eq!(brackets.get(&(0, 1)).unwrap().depth, 1); // inner (
        assert_eq!(brackets.get(&(0, 3)).unwrap().depth, 1); // inner )
        assert_eq!(brackets.get(&(0, 4)).unwrap().depth, 0); // outer )
    }

    #[test]
    fn test_multiline() {
        let content = "{\n  (a)\n}";
        let brackets = compute_bracket_depths(content);

        assert_eq!(brackets.len(), 4);
        assert_eq!(brackets.get(&(0, 0)).unwrap().depth, 0); // {
        assert_eq!(brackets.get(&(1, 2)).unwrap().depth, 0); // (
        assert_eq!(brackets.get(&(1, 4)).unwrap().depth, 0); // )
        assert_eq!(brackets.get(&(2, 0)).unwrap().depth, 0); // }
    }

    #[test]
    fn test_unmatched_closing() {
        let content = "a)";
        let brackets = compute_bracket_depths(content);

        assert_eq!(brackets.len(), 1);
        assert_eq!(brackets.get(&(0, 1)).unwrap().depth, usize::MAX);
    }

    #[test]
    fn test_unmatched_opening() {
        let content = "(a";
        let brackets = compute_bracket_depths(content);

        assert_eq!(brackets.len(), 1);
        assert_eq!(brackets.get(&(0, 0)).unwrap().depth, usize::MAX);
    }

    #[test]
    fn test_angle_brackets_excluded() {
        // `<` and `>` are intentionally NOT treated as brackets
        // because they're commonly used in operators
        let content = "Vec<T>";
        let brackets = compute_bracket_depths(content);

        // No brackets should be detected (< and > are excluded)
        assert_eq!(brackets.len(), 0);
    }

    #[test]
    fn test_arrow_with_parens() {
        // `->` doesn't interfere with actual brackets
        let content = "fn foo() -> u32";
        let brackets = compute_bracket_depths(content);

        // Only ( and ) should be detected
        assert_eq!(brackets.len(), 2);
        assert!(brackets.contains_key(&(0, 6))); // (
        assert!(brackets.contains_key(&(0, 7))); // )
    }

    #[test]
    fn test_match_with_braces() {
        // `=>` doesn't interfere with actual brackets
        let content = "match x { a => b }";
        let brackets = compute_bracket_depths(content);

        // Only { and } should be detected
        assert_eq!(brackets.len(), 2);
        assert!(brackets.contains_key(&(0, 8))); // {
        assert!(brackets.contains_key(&(0, 17))); // }
    }
}
