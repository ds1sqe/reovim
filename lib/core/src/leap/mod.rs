//! Leap motion module for quick cursor jumps
//!
//! Provides leap.nvim/hop.nvim style navigation:
//! - Press `s` + two characters to search forward
//! - Press `S` + two characters to search backward
//! - Labels appear on all matches, press a label to jump

use crate::modd::OperatorType;
use crate::screen::Position;

/// Direction of leap search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeapDirection {
    #[default]
    Forward,
    Backward,
}

/// Phase of the leap interaction
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LeapPhase {
    /// Not active
    #[default]
    Inactive,
    /// Waiting for first character
    WaitingFirstChar,
    /// Waiting for second character (first char stored)
    WaitingSecondChar { first: char },
    /// Showing labels for matches
    ShowingLabels,
}

/// A single match position with its assigned label
#[derive(Debug, Clone)]
pub struct LeapMatch {
    /// Line number (0-indexed)
    pub line: u16,
    /// Column number (0-indexed)
    pub col: u16,
    /// Assigned label character(s)
    pub label: String,
}

impl LeapMatch {
    /// Create a new leap match
    #[must_use]
    pub const fn new(line: u16, col: u16) -> Self {
        Self {
            line,
            col,
            label: String::new(),
        }
    }

    /// Create a leap match with a label
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String can't be const constructed
    pub fn with_label(line: u16, col: u16, label: String) -> Self {
        Self { line, col, label }
    }

    /// Get the position as a Position struct
    #[must_use]
    pub const fn position(&self) -> Position {
        Position {
            x: self.col,
            y: self.line,
        }
    }
}

/// State for the leap motion feature
#[derive(Debug, Clone, Default)]
pub struct LeapState {
    /// Search direction
    pub direction: LeapDirection,
    /// Current phase of interaction
    pub phase: LeapPhase,
    /// The two-character search pattern
    pub search_chars: String,
    /// All matches with their labels
    pub matches: Vec<LeapMatch>,
    /// Pending operator (if called from operator-pending mode)
    pub operator: Option<OperatorType>,
    /// Operator count from before leap was triggered
    pub operator_count: Option<usize>,
}

impl LeapState {
    /// Create a new inactive leap state
    #[must_use]
    pub const fn new() -> Self {
        Self {
            direction: LeapDirection::Forward,
            phase: LeapPhase::Inactive,
            search_chars: String::new(),
            matches: Vec::new(),
            operator: None,
            operator_count: None,
        }
    }

    /// Check if leap mode is active
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !matches!(self.phase, LeapPhase::Inactive)
    }

    /// Check if we have matches to display
    #[must_use]
    pub const fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    /// Check if we're showing labels
    #[must_use]
    pub const fn is_showing_labels(&self) -> bool {
        matches!(self.phase, LeapPhase::ShowingLabels)
    }

    /// Activate leap mode with given direction
    pub fn activate(&mut self, direction: LeapDirection) {
        self.direction = direction;
        self.phase = LeapPhase::WaitingFirstChar;
        self.search_chars.clear();
        self.matches.clear();
        self.operator = None;
        self.operator_count = None;
    }

    /// Activate leap mode with operator context
    pub fn activate_with_operator(
        &mut self,
        direction: LeapDirection,
        operator: OperatorType,
        count: Option<usize>,
    ) {
        self.activate(direction);
        self.operator = Some(operator);
        self.operator_count = count;
    }

    /// Set the first character
    pub fn set_first_char(&mut self, c: char) {
        self.search_chars.clear();
        self.search_chars.push(c);
        self.phase = LeapPhase::WaitingSecondChar { first: c };
    }

    /// Set the second character and transition to showing labels
    pub fn set_second_char(&mut self, c: char) {
        self.search_chars.push(c);
        self.phase = LeapPhase::ShowingLabels;
    }

    /// Set matches with generated labels
    pub fn set_matches(&mut self, matches: Vec<LeapMatch>) {
        self.matches = matches;
    }

    /// Find a match by its label
    #[must_use]
    pub fn find_by_label(&self, label: &str) -> Option<&LeapMatch> {
        self.matches.iter().find(|m| m.label == label)
    }

    /// Deactivate leap mode
    pub fn deactivate(&mut self) {
        self.phase = LeapPhase::Inactive;
        self.search_chars.clear();
        self.matches.clear();
        self.operator = None;
        self.operator_count = None;
    }

    /// Start leap mode (alias for activate with optional operator)
    pub fn start(
        &mut self,
        direction: LeapDirection,
        operator: Option<OperatorType>,
        count: Option<usize>,
    ) {
        self.direction = direction;
        self.phase = LeapPhase::WaitingFirstChar;
        self.search_chars.clear();
        self.matches.clear();
        self.operator = operator;
        self.operator_count = count;
    }

    /// Reset leap state (alias for deactivate)
    pub fn reset(&mut self) {
        self.deactivate();
    }

    /// Set matches with pre-generated labels
    pub fn set_matches_with_labels(&mut self, mut matches: Vec<LeapMatch>, labels: Vec<String>) {
        for (m, label) in matches.iter_mut().zip(labels) {
            m.label = label;
        }
        self.matches = matches;
    }

    /// Find a match by its label (returns cloned match)
    #[must_use]
    pub fn find_match_by_label(&self, label: &str) -> Option<LeapMatch> {
        self.matches.iter().find(|m| m.label == label).cloned()
    }

    /// Get the prompt text to display
    #[must_use]
    pub fn prompt_text(&self) -> String {
        match &self.phase {
            LeapPhase::Inactive => String::new(),
            LeapPhase::WaitingFirstChar => "Leap: _".to_string(),
            LeapPhase::WaitingSecondChar { first } => format!("Leap: {first}_"),
            LeapPhase::ShowingLabels => format!("Leap: {}", self.search_chars),
        }
    }
}

// ============================================================================
// Label Generation
// ============================================================================

/// Label characters in priority order (home row first, then others)
const LABEL_CHARS: &[char] = &[
    's', 'f', 'n', 'j', 'k', 'l', 'h', 'o', 'd', 'w', 'e', 'i', 'm', 'b', 'u', 'y', 'v', 'r', 'g',
    't', 'a', 'q', 'p', 'c', 'x', 'z',
];

/// Generate labels for a given number of matches
#[must_use]
pub fn generate_labels(count: usize) -> Vec<String> {
    let mut labels = Vec::with_capacity(count);
    let chars = LABEL_CHARS;
    let char_count = chars.len();

    if count <= char_count {
        // Single character labels
        for c in chars.iter().take(count) {
            labels.push(c.to_string());
        }
    } else {
        // Two character labels for overflow
        for (i, c) in chars.iter().enumerate().take(count) {
            if i < char_count {
                labels.push(c.to_string());
            }
        }
        // Generate two-character labels for remaining
        for i in char_count..count {
            let first_idx = (i - char_count) / char_count;
            let second_idx = (i - char_count) % char_count;
            if first_idx < char_count {
                labels.push(format!("{}{}", chars[first_idx], chars[second_idx]));
            } else {
                // Fallback for very large counts
                labels.push(format!("{i}"));
            }
        }
    }

    labels
}

// ============================================================================
// Pattern Matching
// ============================================================================

/// Find all matches of a two-character pattern in buffer lines
///
/// # Arguments
/// * `lines` - Buffer lines as string slices
/// * `pattern` - Two-character pattern to find
/// * `direction` - Search direction
/// * `cursor_line` - Current cursor line (0-indexed)
/// * `cursor_col` - Current cursor column (0-indexed)
/// * `visible_start` - First visible line (0-indexed)
/// * `visible_end` - Last visible line (exclusive, 0-indexed)
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn find_matches(
    lines: &[&str],
    pattern: &str,
    direction: LeapDirection,
    cursor_line: u16,
    cursor_col: u16,
    visible_start: u16,
    visible_end: u16,
) -> Vec<LeapMatch> {
    if pattern.len() < 2 {
        return Vec::new();
    }

    let pattern_lower = pattern.to_lowercase();
    let mut matches = Vec::new();

    // Determine search range based on direction
    let (start_line, end_line) = match direction {
        LeapDirection::Forward => (cursor_line, visible_end),
        LeapDirection::Backward => (visible_start, cursor_line.saturating_add(1)),
    };

    for line_idx in start_line..end_line.min(lines.len() as u16) {
        let line_str = lines[line_idx as usize];
        let line_lower = line_str.to_lowercase();

        // Find all occurrences of pattern in this line
        let mut search_start = 0;
        while let Some(pos) = line_lower[search_start..].find(&pattern_lower) {
            let col = (search_start + pos) as u16;

            // Skip matches before cursor on cursor line (for forward)
            // Skip matches after cursor on cursor line (for backward)
            let should_include = match direction {
                LeapDirection::Forward => {
                    line_idx > cursor_line || (line_idx == cursor_line && col > cursor_col)
                }
                LeapDirection::Backward => {
                    line_idx < cursor_line || (line_idx == cursor_line && col < cursor_col)
                }
            };

            if should_include {
                matches.push(LeapMatch::new(line_idx, col));
            }

            search_start += pos + 1;
            if search_start >= line_str.len() {
                break;
            }
        }
    }

    // Sort matches by distance from cursor
    match direction {
        LeapDirection::Forward => {
            // Already in forward order from iteration
        }
        LeapDirection::Backward => {
            // Reverse for backward direction (closest first)
            matches.reverse();
        }
    }

    // Assign labels
    let labels = generate_labels(matches.len());
    for (m, label) in matches.iter_mut().zip(labels) {
        m.label = label;
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_labels_small() {
        let labels = generate_labels(5);
        assert_eq!(labels.len(), 5);
        assert_eq!(labels[0], "s");
        assert_eq!(labels[1], "f");
        assert_eq!(labels[4], "k");
    }

    #[test]
    fn test_generate_labels_overflow() {
        let labels = generate_labels(30);
        assert_eq!(labels.len(), 30);
        // First 26 are single chars
        assert_eq!(labels[0], "s");
        assert_eq!(labels[25], "z");
        // After 26, two-char labels (first char + cycling second char)
        assert_eq!(labels[26], "ss"); // chars[0] + chars[0]
        assert_eq!(labels[27], "sf"); // chars[0] + chars[1]
    }

    #[test]
    fn test_leap_state_lifecycle() {
        let mut state = LeapState::new();
        assert!(!state.is_active());

        state.activate(LeapDirection::Forward);
        assert!(state.is_active());
        assert!(matches!(state.phase, LeapPhase::WaitingFirstChar));

        state.set_first_char('a');
        assert!(matches!(state.phase, LeapPhase::WaitingSecondChar { first: 'a' }));

        state.set_second_char('b');
        assert!(matches!(state.phase, LeapPhase::ShowingLabels));
        assert_eq!(state.search_chars, "ab");

        state.deactivate();
        assert!(!state.is_active());
    }
}
