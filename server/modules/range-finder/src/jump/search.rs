//! Jump search engine - pure functions for label generation and match finding.
//!
//! This module contains no state; all functions are pure and fully testable
//! in isolation. Ported from `archive/pre_kernel/plugins/features/range-finder/`.

/// Search direction relative to cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Only matches after cursor position.
    Forward,
    /// Only matches before cursor position.
    Backward,
    /// All matches except at cursor position.
    Both,
}

/// A single match with its buffer position, assigned label, and distance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpMatch {
    /// Buffer line (0-indexed).
    pub line: u32,
    /// Column position (0-indexed, byte offset).
    pub col: u32,
    /// Assigned label string (e.g. `"s"` or `"sf"`).
    pub label: String,
    /// Manhattan distance from cursor.
    pub distance: u32,
}

impl JumpMatch {
    /// Create a new jump match.
    #[must_use]
    pub const fn new(line: u32, col: u32, label: String, distance: u32) -> Self {
        Self {
            line,
            col,
            label,
            distance,
        }
    }
}

/// Home-row priority label characters (ergonomic order).
const LABELS: &str = "sfnjklhodweimbuyvrgtaqpcxz";

/// Maximum number of labels (26 x 26 = 676 two-char combinations).
pub const MAX_LABELS: usize = 26 * 26;

/// Generate labels for the given match count.
///
/// - 0: empty
/// - 1..=26: single-char labels (`s`, `f`, `n`, ...)
/// - 27..=676: two-char labels only (`ss`, `sf`, ...) — never mixed with
///   single-char to avoid ambiguity
/// - >676: capped at 676
#[must_use]
pub fn generate_labels(count: usize) -> Vec<String> {
    let safe_count = count.min(MAX_LABELS);
    let label_chars: Vec<char> = LABELS.chars().collect();
    let mut labels = Vec::with_capacity(safe_count);

    if safe_count <= 26 {
        labels.extend(label_chars.iter().take(safe_count).map(|&c| c.to_string()));
    } else {
        for i in 0..safe_count {
            let first_idx = i / 26;
            let second_idx = i % 26;
            let label = format!("{}{}", label_chars[first_idx], label_chars[second_idx]);
            labels.push(label);
        }
    }

    labels
}

/// Find all matches of `pattern` in buffer lines, sorted by Manhattan distance.
///
/// Returns matches with labels assigned in home-row priority order (closest
/// match gets the most ergonomic label).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn find_matches(
    pattern: &str,
    lines: &[String],
    cursor_line: u32,
    cursor_col: u32,
    direction: Direction,
) -> Vec<JumpMatch> {
    if pattern.is_empty() || lines.is_empty() {
        return Vec::new();
    }

    let pattern_lower = pattern.to_lowercase();
    let mut positions: Vec<(u32, u32, u32)> = Vec::new();

    for (line_idx, line) in lines.iter().enumerate() {
        let line_lower = line.to_lowercase();
        let mut col = 0;

        while let Some(pos) = line_lower[col..].find(&pattern_lower) {
            let match_col = col + pos;
            let line_num = line_idx as u32;
            let col_num = match_col as u32;

            let include = match direction {
                Direction::Forward => {
                    line_num > cursor_line || (line_num == cursor_line && col_num > cursor_col)
                }
                Direction::Backward => {
                    line_num < cursor_line || (line_num == cursor_line && col_num < cursor_col)
                }
                Direction::Both => !(line_num == cursor_line && col_num == cursor_col),
            };

            if include {
                let distance = line_num.abs_diff(cursor_line) + col_num.abs_diff(cursor_col);
                positions.push((line_num, col_num, distance));
            }

            col = match_col + 1;
        }
    }

    // Sort by distance (closest first).
    positions.sort_by_key(|&(_, _, dist)| dist);

    // Cap at maximum label count.
    if positions.len() > MAX_LABELS {
        positions.truncate(MAX_LABELS);
    }

    let labels = generate_labels(positions.len());
    positions
        .into_iter()
        .zip(labels)
        .map(|((line, col, distance), label)| JumpMatch::new(line, col, label, distance))
        .collect()
}

/// Returns `true` if exactly one match (auto-jump without label selection).
#[must_use]
pub const fn should_auto_jump(match_count: usize) -> bool {
    match_count == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── generate_labels ──────────────────────────────────────────────

    #[test]
    fn test_generate_labels_zero() {
        assert!(generate_labels(0).is_empty());
    }

    #[test]
    fn test_generate_labels_one() {
        let labels = generate_labels(1);
        assert_eq!(labels, vec!["s"]);
    }

    #[test]
    fn test_generate_labels_twenty_six() {
        let labels = generate_labels(26);
        assert_eq!(labels.len(), 26);
        assert_eq!(labels[0], "s");
        assert_eq!(labels[1], "f");
        assert_eq!(labels[25], "z");
        // All single-char
        assert!(labels.iter().all(|l| l.len() == 1));
    }

    #[test]
    fn test_generate_labels_twenty_seven() {
        let labels = generate_labels(27);
        assert_eq!(labels.len(), 27);
        // All two-char (no mixing)
        assert!(labels.iter().all(|l| l.len() == 2));
        assert_eq!(labels[0], "ss");
        assert_eq!(labels[1], "sf");
        assert_eq!(labels[26], "fs");
    }

    #[test]
    fn test_generate_labels_max() {
        let labels = generate_labels(676);
        assert_eq!(labels.len(), 676);
        assert!(labels.iter().all(|l| l.len() == 2));
        assert_eq!(labels[0], "ss");
        assert_eq!(labels[675], "zz");
    }

    #[test]
    fn test_generate_labels_over_max() {
        let labels = generate_labels(700);
        assert_eq!(labels.len(), 676);
    }

    // ── find_matches ─────────────────────────────────────────────────

    fn sample_lines() -> Vec<String> {
        vec![
            "hello world".into(),
            "hello rust".into(),
            "goodbye hello".into(),
        ]
    }

    #[test]
    fn test_find_matches_forward() {
        let lines = sample_lines();
        let matches = find_matches("he", &lines, 0, 0, Direction::Forward);
        // (0,0) excluded because Forward requires col > cursor_col on same line
        // Matches: (1,0) distance=1, (2,8) distance=10
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[0].col, 0);
    }

    #[test]
    fn test_find_matches_backward() {
        let lines = sample_lines();
        let matches = find_matches("he", &lines, 2, 10, Direction::Backward);
        // All three "he" are before (2,10): (0,0), (1,0), (2,8)
        assert_eq!(matches.len(), 3);
        // Closest first: (2,8) dist=2, then (1,0) dist=11, then (0,0) dist=12
        assert_eq!(matches[0].line, 2);
        assert_eq!(matches[0].col, 8);
    }

    #[test]
    fn test_find_matches_both() {
        let lines = sample_lines();
        let matches = find_matches("he", &lines, 1, 0, Direction::Both);
        // Excludes cursor position (1,0), includes (0,0) and (2,8)
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_find_matches_forward_same_line_ahead() {
        // Cursor in the middle of the line. Matches before AND after on same line.
        let lines = vec!["he_he_he".into()];
        let matches = find_matches("he", &lines, 0, 3, Direction::Forward);
        // Match at col 0: same line, col < cursor_col → excluded
        // Match at col 3: same line, col == cursor_col → excluded (not >)
        // Match at col 6: same line, col > cursor_col → included
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].col, 6);
    }

    #[test]
    fn test_find_matches_forward_excludes_before_cursor_line() {
        // Matches on lines before cursor should be excluded in Forward mode.
        let lines = vec!["hello".into(), "world".into(), "hello".into()];
        let matches = find_matches("he", &lines, 1, 0, Direction::Forward);
        // Line 0: before cursor_line → excluded (line_num < cursor_line, not ==)
        // Line 2: after cursor_line → included
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 2);
    }

    #[test]
    fn test_find_matches_backward_excludes_after_cursor_line() {
        // Matches on lines after cursor should be excluded in Backward mode.
        let lines = vec!["hello".into(), "world".into(), "hello".into()];
        let matches = find_matches("he", &lines, 1, 0, Direction::Backward);
        // Line 0: before cursor_line → included
        // Line 2: after cursor_line → excluded (line_num > cursor_line, not ==)
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 0);
    }

    #[test]
    fn test_find_matches_backward_same_line_at_or_after_cursor() {
        // Backward: same line, match at or after cursor_col → excluded.
        let lines = vec!["he_he_he".into()];
        let matches = find_matches("he", &lines, 0, 3, Direction::Backward);
        // Match at col 0: same line, col < cursor_col → included
        // Match at col 3: same line, col == cursor_col → excluded (not <)
        // Match at col 6: same line, col > cursor_col → excluded (not <)
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].col, 0);
    }

    #[test]
    fn test_find_matches_case_insensitive() {
        let lines = vec!["Hello WORLD hello HeLLo".into()];
        let matches = find_matches("he", &lines, 0, 100, Direction::Backward);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_find_matches_empty_pattern() {
        assert!(find_matches("", &sample_lines(), 0, 0, Direction::Both).is_empty());
    }

    #[test]
    fn test_find_matches_empty_lines() {
        let empty: Vec<String> = Vec::new();
        assert!(find_matches("he", &empty, 0, 0, Direction::Both).is_empty());
    }

    #[test]
    fn test_find_matches_no_results() {
        let lines = sample_lines();
        assert!(find_matches("zz", &lines, 0, 0, Direction::Both).is_empty());
    }

    #[test]
    fn test_find_matches_distance_sorting() {
        let lines = vec![
            "ab ab ab".into(), // matches at col 0, 3, 6
        ];
        let matches = find_matches("ab", &lines, 0, 3, Direction::Both);
        // Distances from (0,3): col 0 -> 3, col 6 -> 3
        // Both distance 3, col 0 and col 6
        assert_eq!(matches.len(), 2);
        // Both have distance 3 (stable sort preserves encounter order)
        assert_eq!(matches[0].distance, 3);
        assert_eq!(matches[1].distance, 3);
    }

    #[test]
    fn test_find_matches_cap_at_max() {
        // Create a line with many repeated patterns
        let line = "ab ".repeat(700);
        let lines = vec![line];
        let matches = find_matches("ab", &lines, 0, 5000, Direction::Both);
        assert!(matches.len() <= MAX_LABELS);
    }

    #[test]
    fn test_find_matches_label_assignment() {
        let lines = vec![
            "he he he he he".into(), // 5 matches
        ];
        let matches = find_matches("he", &lines, 0, 100, Direction::Backward);
        assert_eq!(matches.len(), 5);
        // Labels assigned in home-row order to closest-first matches
        let labels: Vec<&str> = matches.iter().map(|m| m.label.as_str()).collect();
        assert_eq!(labels[0], "s");
        assert_eq!(labels[1], "f");
        assert_eq!(labels[2], "n");
        assert_eq!(labels[3], "j");
        assert_eq!(labels[4], "k");
    }

    // ── should_auto_jump ─────────────────────────────────────────────

    #[test]
    fn test_should_auto_jump_zero() {
        assert!(!should_auto_jump(0));
    }

    #[test]
    fn test_should_auto_jump_one() {
        assert!(should_auto_jump(1));
    }

    #[test]
    fn test_should_auto_jump_two() {
        assert!(!should_auto_jump(2));
    }
}
