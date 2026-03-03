//! Jump state machine - per-client session extension.
//!
//! Implements `SessionExtension` + `TextInputSink` for the jump navigation
//! lifecycle. Characters flow through `insert_char()` to drive state transitions.
//!
//! # State Machine
//!
//! ```text
//! Inactive
//!   -> start(lines) -> WaitingFirstChar
//!   -> insert_char(c1) -> WaitingSecondChar
//!   -> insert_char(c2) -> find_matches(c1+c2)
//!      -> 0 matches: Inactive
//!      -> 1 match: auto-jump, target set, Inactive
//!      -> 2-26: ShowingLabels (single-char)
//!      -> 27-676: ShowingLabels (two-char)
//!   -> label selection -> target set -> Inactive
//!   -> cancel() at any phase -> Inactive
//! ```

use reovim_driver_session::{SessionExtension, TextInputSink};

use super::search::{Direction, JumpMatch, find_matches, should_auto_jump};

/// Jump target: where the cursor should move after label selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpTarget {
    /// Buffer line (0-indexed).
    pub line: u32,
    /// Column position (0-indexed).
    pub col: u32,
}

/// Internal phase of the jump state machine.
#[derive(Debug)]
enum JumpPhase {
    /// Not active.
    Inactive,
    /// Waiting for first search character.
    WaitingFirstChar,
    /// Have first char, waiting for second.
    WaitingSecondChar { first_char: char },
    /// Labels are displayed, waiting for label selection.
    ShowingLabels {
        matches: Vec<JumpMatch>,
        label_len: usize,
    },
    /// First char of a two-char label typed, waiting for second.
    WaitingLabelSecondChar {
        matches: Vec<JumpMatch>,
        first_label_char: char,
    },
}

/// Per-client jump navigation state.
///
/// Implements `SessionExtension` for automatic lifecycle management and
/// `TextInputSink` for character routing from the jump-input mode resolver.
#[derive(Debug)]
pub struct JumpSessionState {
    phase: JumpPhase,
    /// Resolved jump target (consumed by command after state machine completes).
    target: Option<JumpTarget>,
    /// Buffer lines for search (set by `start()`, cleared on cancel/completion).
    lines: Vec<String>,
    /// Cursor position when search started.
    cursor_line: u32,
    cursor_col: u32,
    /// Search direction.
    direction: Direction,
}

impl Default for JumpSessionState {
    fn default() -> Self {
        Self {
            phase: JumpPhase::Inactive,
            target: None,
            lines: Vec::new(),
            cursor_line: 0,
            cursor_col: 0,
            direction: Direction::Both,
        }
    }
}

impl SessionExtension for JumpSessionState {
    fn create() -> Self {
        Self::default()
    }

    fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
        Some(self)
    }
}

impl TextInputSink for JumpSessionState {
    fn insert_char(&mut self, ch: char) {
        Self::handle_char(self, ch);
    }
}

impl JumpSessionState {
    /// Start a jump search session with the given buffer context.
    pub fn start(
        &mut self,
        lines: Vec<String>,
        cursor_line: u32,
        cursor_col: u32,
        direction: Direction,
    ) {
        self.phase = JumpPhase::WaitingFirstChar;
        self.target = None;
        self.lines = lines;
        self.cursor_line = cursor_line;
        self.cursor_col = cursor_col;
        self.direction = direction;
    }

    /// Cancel the current jump search, returning to inactive.
    pub fn cancel(&mut self) {
        self.phase = JumpPhase::Inactive;
        self.target = None;
        self.lines.clear();
    }

    /// Take the resolved jump target (consumes it).
    ///
    /// Returns `Some(JumpTarget)` if a label was successfully selected,
    /// `None` otherwise.
    pub const fn take_target(&mut self) -> Option<JumpTarget> {
        self.target.take()
    }

    /// Whether the jump state machine is active (not Inactive).
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !matches!(self.phase, JumpPhase::Inactive)
    }

    /// Get the current matches (only valid during `ShowingLabels` or
    /// `WaitingLabelSecondChar` phases).
    #[must_use]
    pub fn get_matches(&self) -> Option<&[JumpMatch]> {
        match &self.phase {
            JumpPhase::ShowingLabels { matches, .. }
            | JumpPhase::WaitingLabelSecondChar { matches, .. } => Some(matches),
            _ => None,
        }
    }

    /// Drive the state machine with a character input.
    fn handle_char(&mut self, ch: char) {
        match std::mem::replace(&mut self.phase, JumpPhase::Inactive) {
            JumpPhase::Inactive => {
                // No-op: can't type when inactive.
                self.phase = JumpPhase::Inactive;
            }
            JumpPhase::WaitingFirstChar => {
                self.phase = JumpPhase::WaitingSecondChar { first_char: ch };
            }
            JumpPhase::WaitingSecondChar { first_char } => {
                let pattern = format!("{first_char}{ch}");
                let matches = find_matches(
                    &pattern,
                    &self.lines,
                    self.cursor_line,
                    self.cursor_col,
                    self.direction,
                );

                if matches.is_empty() {
                    self.cancel();
                } else if should_auto_jump(matches.len()) {
                    self.target = Some(JumpTarget {
                        line: matches[0].line,
                        col: matches[0].col,
                    });
                    self.phase = JumpPhase::Inactive;
                    self.lines.clear();
                } else {
                    let label_len = matches[0].label.len();
                    self.phase = JumpPhase::ShowingLabels { matches, label_len };
                }
            }
            JumpPhase::ShowingLabels { matches, label_len } => {
                if label_len == 1 {
                    // Single-char labels: immediate selection.
                    let label = ch.to_string();
                    if let Some(m) = matches.iter().find(|m| m.label == label) {
                        self.target = Some(JumpTarget {
                            line: m.line,
                            col: m.col,
                        });
                        self.phase = JumpPhase::Inactive;
                        self.lines.clear();
                    } else {
                        self.cancel();
                    }
                } else {
                    // Two-char labels: need second char.
                    // Check if any label starts with this char.
                    let has_prefix = matches.iter().any(|m| m.label.starts_with(ch));
                    if has_prefix {
                        self.phase = JumpPhase::WaitingLabelSecondChar {
                            matches,
                            first_label_char: ch,
                        };
                    } else {
                        self.cancel();
                    }
                }
            }
            JumpPhase::WaitingLabelSecondChar {
                matches,
                first_label_char,
            } => {
                let label = format!("{first_label_char}{ch}");
                if let Some(m) = matches.iter().find(|m| m.label == label) {
                    self.target = Some(JumpTarget {
                        line: m.line,
                        col: m.col,
                    });
                    self.phase = JumpPhase::Inactive;
                    self.lines.clear();
                } else {
                    self.cancel();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lines_with_matches(count: usize) -> Vec<String> {
        // Create lines with exactly `count` occurrences of "he".
        let mut lines = Vec::new();
        let mut remaining = count;
        while remaining > 0 {
            let per_line = remaining.min(10);
            let line = "he ".repeat(per_line);
            lines.push(line.trim_end().to_string());
            remaining -= per_line;
        }
        lines
    }

    #[test]
    fn test_default_inactive() {
        let state = JumpSessionState::default();
        assert!(!state.is_active());
        assert!(state.get_matches().is_none());
    }

    #[test]
    fn test_start_transitions_to_waiting_first() {
        let mut state = JumpSessionState::default();
        state.start(vec!["hello".into()], 0, 0, Direction::Both);
        assert!(state.is_active());
    }

    #[test]
    fn test_first_char_transitions_to_waiting_second() {
        let mut state = JumpSessionState::default();
        state.start(vec!["hello world".into()], 0, 100, Direction::Both);
        state.insert_char('h');
        // Should be in WaitingSecondChar now (still active, no matches shown).
        assert!(state.is_active());
        assert!(state.get_matches().is_none());
    }

    #[test]
    fn test_second_char_zero_matches_cancels() {
        let mut state = JumpSessionState::default();
        state.start(vec!["hello world".into()], 0, 0, Direction::Both);
        state.insert_char('z');
        state.insert_char('z');
        assert!(!state.is_active());
        assert!(state.take_target().is_none());
    }

    #[test]
    fn test_second_char_one_match_auto_jumps() {
        let mut state = JumpSessionState::default();
        state.start(vec!["hello world".into()], 0, 100, Direction::Both);
        state.insert_char('w');
        state.insert_char('o');
        // "wo" only matches once at col 6.
        assert!(!state.is_active());
        let target = state.take_target().expect("should have target");
        assert_eq!(target.line, 0);
        assert_eq!(target.col, 6);
    }

    #[test]
    fn test_second_char_few_matches_shows_single_labels() {
        let mut state = JumpSessionState::default();
        // 3 matches of "he": need to be careful with cursor position.
        let lines = vec!["he he he".into()];
        state.start(lines, 0, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');
        // Should be ShowingLabels with single-char labels.
        assert!(state.is_active());
        let matches = state.get_matches().expect("should have matches");
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].label.len(), 1); // single-char labels
    }

    #[test]
    fn test_second_char_many_matches_shows_two_char_labels() {
        let mut state = JumpSessionState::default();
        let lines = make_lines_with_matches(30);
        state.start(lines, 100, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');
        assert!(state.is_active());
        let matches = state.get_matches().expect("should have matches");
        assert!(matches.len() >= 27);
        assert_eq!(matches[0].label.len(), 2); // two-char labels
    }

    #[test]
    fn test_second_char_over_max_shows_capped_labels() {
        // >676 matches should be capped at 676 by find_matches.
        let mut state = JumpSessionState::default();
        let lines = make_lines_with_matches(700);
        state.start(lines, 10000, 10000, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');
        // Should show labels (capped at 676), not cancel.
        assert!(state.is_active());
        let matches = state.get_matches().expect("should have matches");
        assert!(matches.len() <= 676);
    }

    #[test]
    fn test_label_select_single_char_match() {
        let mut state = JumpSessionState::default();
        let lines = vec!["he he he".into()];
        state.start(lines, 0, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');

        // First label is "s".
        let matches = state.get_matches().unwrap();
        let first_label = matches[0].label.clone();
        let expected_line = matches[0].line;
        let expected_col = matches[0].col;

        state.insert_char(first_label.chars().next().unwrap());
        assert!(!state.is_active());
        let target = state.take_target().expect("should have target");
        assert_eq!(target.line, expected_line);
        assert_eq!(target.col, expected_col);
    }

    #[test]
    fn test_label_select_single_char_no_match() {
        let mut state = JumpSessionState::default();
        let lines = vec!["he he he".into()];
        state.start(lines, 0, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');

        // Type a char that's not a valid label.
        state.insert_char('1');
        assert!(!state.is_active());
        assert!(state.take_target().is_none());
    }

    #[test]
    fn test_label_select_two_char_first() {
        let mut state = JumpSessionState::default();
        let lines = make_lines_with_matches(30);
        state.start(lines, 100, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');

        // First label should be "ss". Type 's' first.
        assert!(state.is_active());
        state.insert_char('s');
        // Should be in WaitingLabelSecondChar now.
        assert!(state.is_active());
        assert!(state.get_matches().is_some());
    }

    #[test]
    fn test_label_select_two_char_complete() {
        let mut state = JumpSessionState::default();
        let lines = make_lines_with_matches(30);
        state.start(lines, 100, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');

        let matches = state.get_matches().unwrap();
        let first_label = matches[0].label.clone();
        let expected_line = matches[0].line;
        let expected_col = matches[0].col;
        assert_eq!(first_label.len(), 2);

        let mut chars = first_label.chars();
        state.insert_char(chars.next().unwrap());
        state.insert_char(chars.next().unwrap());

        assert!(!state.is_active());
        let target = state.take_target().expect("should have target");
        assert_eq!(target.line, expected_line);
        assert_eq!(target.col, expected_col);
    }

    #[test]
    fn test_label_select_two_char_no_match() {
        let mut state = JumpSessionState::default();
        let lines = make_lines_with_matches(30);
        state.start(lines, 100, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');

        // Type valid first char, invalid second char.
        state.insert_char('s');
        state.insert_char('1'); // Not a valid label char.
        assert!(!state.is_active());
        assert!(state.take_target().is_none());
    }

    #[test]
    fn test_cancel_from_waiting_first() {
        let mut state = JumpSessionState::default();
        state.start(vec!["hello".into()], 0, 0, Direction::Both);
        state.cancel();
        assert!(!state.is_active());
    }

    #[test]
    fn test_cancel_from_waiting_second() {
        let mut state = JumpSessionState::default();
        state.start(vec!["hello".into()], 0, 0, Direction::Both);
        state.insert_char('h');
        state.cancel();
        assert!(!state.is_active());
    }

    #[test]
    fn test_cancel_from_showing_labels() {
        let mut state = JumpSessionState::default();
        let lines = vec!["he he he".into()];
        state.start(lines, 0, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');
        assert!(state.is_active());
        state.cancel();
        assert!(!state.is_active());
    }

    #[test]
    fn test_cancel_from_waiting_label_second() {
        let mut state = JumpSessionState::default();
        let lines = make_lines_with_matches(30);
        state.start(lines, 100, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');
        state.insert_char('s'); // First char of two-char label.
        assert!(state.is_active());
        state.cancel();
        assert!(!state.is_active());
    }

    #[test]
    fn test_insert_char_when_inactive_noop() {
        let mut state = JumpSessionState::default();
        state.insert_char('a');
        assert!(!state.is_active());
        assert!(state.take_target().is_none());
    }

    #[test]
    fn test_take_target_consumes() {
        let mut state = JumpSessionState::default();
        state.start(vec!["hello world".into()], 0, 100, Direction::Both);
        state.insert_char('w');
        state.insert_char('o');
        assert!(state.take_target().is_some());
        assert!(state.take_target().is_none());
    }

    #[test]
    fn test_take_target_none_when_no_jump() {
        let state = JumpSessionState::default();
        // Can't call take_target on non-mut default without starting.
        let mut state2 = JumpSessionState::default();
        assert!(state2.take_target().is_none());
        // Suppress unused warning.
        let _ = state;
    }

    #[test]
    fn test_is_active_states() {
        let mut state = JumpSessionState::default();
        assert!(!state.is_active()); // Inactive

        state.start(vec!["he he he".into()], 0, 100, Direction::Both);
        assert!(state.is_active()); // WaitingFirstChar

        state.insert_char('h');
        assert!(state.is_active()); // WaitingSecondChar

        state.insert_char('e');
        assert!(state.is_active()); // ShowingLabels
    }

    #[test]
    fn test_get_matches_showing_labels() {
        let mut state = JumpSessionState::default();
        let lines = vec!["he he he".into()];
        state.start(lines, 0, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');

        let matches = state.get_matches();
        assert!(matches.is_some());
        assert_eq!(matches.unwrap().len(), 3);
    }

    #[test]
    fn test_get_matches_inactive() {
        let state = JumpSessionState::default();
        assert!(state.get_matches().is_none());
    }

    #[test]
    fn test_session_extension_create() {
        let state = JumpSessionState::create();
        assert!(!state.is_active());
    }

    #[test]
    fn test_text_input_sink_available() {
        let mut state = JumpSessionState::default();
        assert!(SessionExtension::as_text_input_sink(&mut state).is_some());
    }

    #[test]
    fn test_two_char_label_invalid_first_char() {
        let mut state = JumpSessionState::default();
        let lines = make_lines_with_matches(30);
        state.start(lines, 100, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');

        // Labels are two-char. Type '1' which isn't a valid label prefix.
        state.insert_char('1');
        assert!(!state.is_active());
        assert!(state.take_target().is_none());
    }

    #[test]
    fn test_generate_labels_called_correctly() {
        // Verify generate_labels is consistent with what state machine expects.
        let labels = crate::jump::search::generate_labels(5);
        assert_eq!(labels.len(), 5);
        assert_eq!(labels[0], "s");
    }
}
