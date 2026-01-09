//! Jump search engine
//!
//! Pattern matching and label generation for jump navigation.

#![allow(dead_code)] // Temporary: will be used once wired up

use super::{command::Direction, state::JumpMatch};

/// Home row priority labels (ergonomic order)
const LABELS: &str = "sfnjklhodweimbuyvrgtaqpcxz";

/// Generate labels for matches
///
/// - 1-26 matches: Single-char labels (s, f, n, ...)
/// - 27+ matches: Two-char labels ONLY (sf, sj, sk, ...) to avoid ambiguity
/// - Maximum 676 labels (26×26)
///
/// We don't mix single and two-char labels to avoid ambiguity:
/// If we had both "s" and "sf", typing "s" would be ambiguous.
#[must_use]
pub fn generate_labels(count: usize) -> Vec<String> {
    // Cap at maximum to prevent index out of bounds
    let safe_count = count.min(MAX_LABELS);
    if count > MAX_LABELS {
        tracing::warn!("generate_labels: requested {} labels, capping at {}", count, MAX_LABELS);
    }

    let mut labels = Vec::with_capacity(safe_count);
    let label_chars: Vec<char> = LABELS.chars().collect();

    // Use single-char labels ONLY if we have <=26 matches
    // Otherwise use two-char labels for ALL to avoid ambiguity
    if safe_count <= 26 {
        // Single-char labels
        labels.extend(label_chars.iter().take(safe_count).map(|&c| c.to_string()));
    } else {
        // Two-char labels for ALL matches (no mixing)
        for i in 0..safe_count {
            let first_idx = i / 26;
            let second_idx = i % 26;
            let label = format!("{}{}", label_chars[first_idx], label_chars[second_idx]);
            labels.push(label);
        }
    }

    labels
}

/// Find all matches of a pattern in buffer lines
///
/// # Arguments
/// * `pattern` - Search pattern (1 or 2 chars)
/// * `lines` - Buffer lines to search
/// * `cursor_line` - Current cursor line (0-indexed)
/// * `cursor_col` - Current cursor column (0-indexed)
/// * `direction` - Search direction
///
/// Returns matches sorted by distance from cursor
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn find_matches(
    pattern: &str,
    lines: &[String],
    cursor_line: u32,
    cursor_col: u32,
    direction: Direction,
) -> Vec<JumpMatch> {
    tracing::info!(
        "==> find_matches: pattern='{}', lines={}, cursor=({}, {}), direction={:?}",
        pattern,
        lines.len(),
        cursor_line,
        cursor_col,
        direction
    );

    if pattern.is_empty() || lines.is_empty() {
        tracing::debug!("==> Empty pattern or no lines, returning empty");
        return Vec::new();
    }

    let pattern_lower = pattern.to_lowercase();
    let mut positions = Vec::new();

    tracing::debug!("==> Starting line-by-line search");
    // Search all lines for pattern
    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx % 500 == 0 {
            tracing::debug!("==> Processed {} / {} lines", line_idx, lines.len());
        }

        let line_lower = line.to_lowercase();
        let mut col = 0;

        while let Some(pos) = line_lower[col..].find(&pattern_lower) {
            let match_col = col + pos;

            // Calculate position
            let line_num = u32::try_from(line_idx).unwrap_or(u32::MAX);
            let col_num = u32::try_from(match_col).unwrap_or(u32::MAX);

            // Filter by direction and cursor position
            let include = match direction {
                Direction::Forward => {
                    line_num > cursor_line || (line_num == cursor_line && col_num > cursor_col)
                }
                Direction::Backward => {
                    line_num < cursor_line || (line_num == cursor_line && col_num < cursor_col)
                }
                Direction::Both => {
                    // Include all matches except cursor position
                    !(line_num == cursor_line && col_num == cursor_col)
                }
            };

            if include {
                // Calculate distance (Manhattan distance)
                let line_dist = line_num.abs_diff(cursor_line);
                let col_dist = col_num.abs_diff(cursor_col);
                let distance = line_dist + col_dist;

                positions.push((line_num, col_num, distance));
            }

            col = match_col + 1; // Move past this match
        }
    }

    tracing::info!("==> Finished search, found {} matches", positions.len());

    // Sort by distance (closest first)
    tracing::debug!("==> Sorting {} matches by distance", positions.len());
    positions.sort_by_key(|(_, _, dist)| *dist);
    tracing::debug!("==> Sort complete");

    // Limit to maximum labels we can generate
    if positions.len() > MAX_LABELS {
        tracing::warn!(
            "==> Too many matches ({}), limiting to {} closest matches",
            positions.len(),
            MAX_LABELS
        );
        positions.truncate(MAX_LABELS);
    }

    // Generate labels and create matches
    tracing::debug!("==> Generating labels for {} matches", positions.len());
    let labels = generate_labels(positions.len());
    tracing::debug!("==> Creating JumpMatch structs");
    let result: Vec<JumpMatch> = positions
        .into_iter()
        .zip(labels)
        .map(|((line, col, distance), label)| JumpMatch::new(line, col, label, distance))
        .collect();
    tracing::info!("==> find_matches returning {} matches", result.len());
    result
}

/// Auto-jump decision based on match count
///
/// Returns true if should auto-jump (exactly 1 match)
#[must_use]
pub const fn should_auto_jump(match_count: usize) -> bool {
    match_count == 1
}

/// Maximum number of labels we can generate
/// We don't mix single-char and two-char labels to avoid ambiguity.
/// - For <=26 matches: use 26 single-char labels (s, f, n, ...)
/// - For >26 matches: use 676 two-char labels (26×26 combinations: ss, sf, sn, ..., zz)
pub const MAX_LABELS: usize = 26 * 26; // 676

/// Check if match count exceeds threshold (too many to show labels)
///
/// Returns true if exceeds `MAX_LABELS` (can't generate enough labels)
/// Since `find_matches()` already caps at `MAX_LABELS`, this should never trigger
#[must_use]
pub const fn too_many_matches(match_count: usize) -> bool {
    match_count > MAX_LABELS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_labels() {
        // Test single-char labels (<=26 matches)
        let labels_single = generate_labels(26);
        assert_eq!(labels_single.len(), 26);
        assert_eq!(labels_single[0], "s");
        assert_eq!(labels_single[1], "f");
        assert_eq!(labels_single[25], "z");

        // Test two-char labels (>26 matches) - ALL two-char, no mixing
        let labels_two = generate_labels(30);
        assert_eq!(labels_two.len(), 30);
        assert_eq!(labels_two[0], "ss"); // 0 / 26 = 0, 0 % 26 = 0
        assert_eq!(labels_two[1], "sf"); // 1 / 26 = 0, 1 % 26 = 1
        assert_eq!(labels_two[2], "sn"); // 2 / 26 = 0, 2 % 26 = 2
        assert_eq!(labels_two[25], "sz"); // 25 / 26 = 0, 25 % 26 = 25
        assert_eq!(labels_two[26], "fs"); // 26 / 26 = 1, 26 % 26 = 0
        assert_eq!(labels_two[27], "ff"); // 27 / 26 = 1, 27 % 26 = 1
        assert_eq!(labels_two[29], "fj"); // 29 / 26 = 1, 29 % 26 = 3
    }

    #[test]
    fn test_find_matches_forward() {
        let lines = vec![
            "hello world".to_string(),
            "hello rust".to_string(),
            "goodbye hello".to_string(),
        ];

        // Search "he" from line 0, col 0, forward
        let matches = find_matches("he", &lines, 0, 0, Direction::Forward);

        // Should find: "hello" at (0, 0), "hello" at (1, 0), "hello" at (2, 8)
        // But (0, 0) is at cursor, so skip it
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 1); // Closest
        assert_eq!(matches[0].col, 0);
        assert_eq!(matches[0].label, "s");

        assert_eq!(matches[1].line, 2);
        assert_eq!(matches[1].col, 8);
        assert_eq!(matches[1].label, "f");
    }

    #[test]
    fn test_find_matches_backward() {
        let lines = vec![
            "hello world".to_string(),
            "hello rust".to_string(),
            "goodbye hello".to_string(),
        ];

        // Search "he" from line 2, col 10, backward
        let matches = find_matches("he", &lines, 2, 10, Direction::Backward);

        // Should find: "hello" at (2, 8), "hello" at (1, 0), "hello" at (0, 0)
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].line, 2); // Closest (same line)
        assert_eq!(matches[0].col, 8);
    }

    #[test]
    fn test_case_insensitive() {
        let lines = vec!["Hello WORLD".to_string()];

        let matches = find_matches("wo", &lines, 0, 0, Direction::Forward);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].col, 6); // Matches "WO" in "WORLD"
    }

    #[test]
    fn test_auto_jump() {
        assert!(should_auto_jump(1));
        assert!(!should_auto_jump(0));
        assert!(!should_auto_jump(2));
    }

    #[test]
    fn test_too_many_matches() {
        assert!(!too_many_matches(26)); // Single-char labels OK
        assert!(!too_many_matches(100)); // Two-char labels OK
        assert!(!too_many_matches(676)); // Exactly at MAX_LABELS - OK
        assert!(too_many_matches(677)); // Exceeds MAX_LABELS - too many
        assert!(too_many_matches(1000)); // Way too many
    }
}
