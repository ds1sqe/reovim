//! Jump search engine - pure functions for label generation and match finding.
//!
//! This module contains no state; all functions are pure and fully testable
//! in isolation. Ported from [`archive/pre_kernel/plugins/features/range-finder/`](https://github.com/ds1sqe/reovim/tree/81806439/archive/pre_kernel/plugins/features/range-finder).

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
#[path = "search_tests.rs"]
mod tests;
