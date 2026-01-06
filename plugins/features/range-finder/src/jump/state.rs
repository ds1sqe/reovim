//! Jump navigation state machine
//!
//! Manages the state of jump navigation including:
//! - Phase tracking (waiting for input, showing labels, etc.)
//! - Match storage
//! - Label generation

#![allow(dead_code)] // Temporary: will be used once wired up

use super::command::{Direction, JumpMode};

/// Jump navigation phase
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum JumpPhase {
    /// Jump mode is not active
    #[default]
    Inactive,

    /// Multi-char search: waiting for first character
    WaitingFirstChar {
        /// Jump mode
        mode: JumpMode,
        /// Search direction
        direction: Direction,
    },

    /// Multi-char search: waiting for second character
    WaitingSecondChar {
        /// Jump mode
        mode: JumpMode,
        /// Search direction
        direction: Direction,
        /// First character entered
        first_char: char,
    },

    /// Single-char search: waiting for character
    WaitingSingleChar {
        /// Jump mode (`FindChar` or `TillChar`)
        mode: JumpMode,
        /// Search direction
        direction: Direction,
    },

    /// Showing labels, waiting for selection
    ShowingLabels {
        /// Jump mode
        mode: JumpMode,
        /// Search direction
        direction: Direction,
        /// Search pattern (1 or 2 chars)
        pattern: String,
        /// Matches with labels
        matches: Vec<JumpMatch>,
    },

    /// Waiting for second character of two-char label
    WaitingLabelSecondChar {
        /// Jump mode
        mode: JumpMode,
        /// Search direction
        direction: Direction,
        /// Search pattern (1 or 2 chars)
        pattern: String,
        /// Matches with labels
        matches: Vec<JumpMatch>,
        /// First character of label
        first_char: char,
    },
}

/// A single jump match with position and label
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpMatch {
    /// Buffer line number
    pub line: u32,
    /// Column position
    pub col: u32,
    /// Assigned label (e.g., "s", "f", "sf")
    pub label: String,
    /// Distance from cursor (for sorting)
    pub distance: u32,
}

impl JumpMatch {
    /// Create a new jump match
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

/// Jump navigation state for a buffer
#[derive(Debug, Clone, Default)]
pub struct JumpState {
    /// Current phase
    pub phase: JumpPhase,
    /// Buffer ID where jump is active
    pub buffer_id: Option<usize>,
    /// Buffer lines (cached when jump starts)
    pub lines: Option<Vec<String>>,
    /// Cursor position when jump started
    pub start_line: u32,
    pub start_col: u32,
    /// Operator context if jump was started from `OperatorPending` mode
    pub operator_context: Option<OperatorContext>,
}

/// Operator context for jump operations
#[derive(Debug, Clone, Copy)]
pub struct OperatorContext {
    /// Operator type (Delete, Yank, Change)
    pub operator: OperatorType,
    /// Optional repeat count
    pub count: Option<usize>,
}

impl From<reovim_core::command::traits::OperatorContext> for OperatorContext {
    fn from(ctx: reovim_core::command::traits::OperatorContext) -> Self {
        Self {
            operator: ctx.operator.into(),
            count: ctx.count,
        }
    }
}

/// Operator type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorType {
    Delete,
    Yank,
    Change,
}

impl From<reovim_core::modd::OperatorType> for OperatorType {
    fn from(op: reovim_core::modd::OperatorType) -> Self {
        match op {
            reovim_core::modd::OperatorType::Delete => Self::Delete,
            reovim_core::modd::OperatorType::Yank => Self::Yank,
            reovim_core::modd::OperatorType::Change => Self::Change,
        }
    }
}

impl From<OperatorType> for reovim_core::modd::OperatorType {
    fn from(op: OperatorType) -> Self {
        match op {
            OperatorType::Delete => Self::Delete,
            OperatorType::Yank => Self::Yank,
            OperatorType::Change => Self::Change,
        }
    }
}

impl JumpState {
    /// Create a new inactive jump state
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phase: JumpPhase::Inactive,
            buffer_id: None,
            lines: None,
            start_line: 0,
            start_col: 0,
            operator_context: None,
        }
    }

    /// Check if jump mode is active
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !matches!(self.phase, JumpPhase::Inactive)
    }

    /// Check if currently showing labels
    #[must_use]
    pub const fn is_showing_labels(&self) -> bool {
        matches!(
            self.phase,
            JumpPhase::ShowingLabels { .. } | JumpPhase::WaitingLabelSecondChar { .. }
        )
    }

    /// Get current matches if showing labels
    #[must_use]
    pub fn get_matches(&self) -> Option<&[JumpMatch]> {
        match &self.phase {
            JumpPhase::ShowingLabels { matches, .. }
            | JumpPhase::WaitingLabelSecondChar { matches, .. } => Some(matches),
            _ => None,
        }
    }

    /// Start multi-char search
    pub fn start_multi_char(
        &mut self,
        buffer_id: usize,
        line: u32,
        col: u32,
        direction: Direction,
        lines: Vec<String>,
        operator_context: Option<OperatorContext>,
    ) {
        self.phase = JumpPhase::WaitingFirstChar {
            mode: JumpMode::MultiChar,
            direction,
        };
        self.buffer_id = Some(buffer_id);
        self.lines = Some(lines);
        self.start_line = line;
        self.start_col = col;
        self.operator_context = operator_context;
    }

    /// Start single-char find
    pub fn start_find_char(
        &mut self,
        buffer_id: usize,
        line: u32,
        col: u32,
        direction: Direction,
        lines: Vec<String>,
        operator_context: Option<OperatorContext>,
    ) {
        self.phase = JumpPhase::WaitingSingleChar {
            mode: JumpMode::FindChar,
            direction,
        };
        self.buffer_id = Some(buffer_id);
        self.lines = Some(lines);
        self.start_line = line;
        self.start_col = col;
        self.operator_context = operator_context;
    }

    /// Start single-char till
    pub fn start_till_char(
        &mut self,
        buffer_id: usize,
        line: u32,
        col: u32,
        direction: Direction,
        lines: Vec<String>,
        operator_context: Option<OperatorContext>,
    ) {
        self.phase = JumpPhase::WaitingSingleChar {
            mode: JumpMode::TillChar,
            direction,
        };
        self.buffer_id = Some(buffer_id);
        self.lines = Some(lines);
        self.start_line = line;
        self.start_col = col;
        self.operator_context = operator_context;
    }

    /// Handle first character input (multi-char mode)
    pub fn handle_first_char(&mut self, c: char) -> bool {
        if let JumpPhase::WaitingFirstChar { mode, direction } = self.phase {
            self.phase = JumpPhase::WaitingSecondChar {
                mode,
                direction,
                first_char: c,
            };
            true
        } else {
            false
        }
    }

    /// Handle second character input (multi-char mode)
    ///
    /// Returns the complete pattern if successful
    pub fn handle_second_char(&self, c: char) -> Option<String> {
        if let JumpPhase::WaitingSecondChar {
            mode: _,
            direction: _,
            first_char,
        } = self.phase
        {
            let pattern = format!("{first_char}{c}");
            // State will be updated to ShowingLabels by the search handler
            Some(pattern)
        } else {
            None
        }
    }

    /// Handle character input (single-char mode)
    ///
    /// Returns the pattern if successful
    pub fn handle_single_char(&self, c: char) -> Option<String> {
        if let JumpPhase::WaitingSingleChar {
            mode: _,
            direction: _,
        } = self.phase
        {
            // State will be updated to ShowingLabels by the search handler
            Some(c.to_string())
        } else {
            None
        }
    }

    /// Transition to showing labels
    pub fn show_labels(&mut self, pattern: String, matches: Vec<JumpMatch>) {
        if let JumpPhase::WaitingSecondChar {
            mode, direction, ..
        }
        | JumpPhase::WaitingSingleChar { mode, direction } = self.phase
        {
            self.phase = JumpPhase::ShowingLabels {
                mode,
                direction,
                pattern,
                matches,
            };
        }
    }

    /// Find match by label
    #[must_use]
    pub fn find_match_by_label(&self, label: &str) -> Option<&JumpMatch> {
        match &self.phase {
            JumpPhase::ShowingLabels { matches, .. } => matches.iter().find(|m| m.label == label),
            JumpPhase::WaitingLabelSecondChar { matches, .. } => {
                matches.iter().find(|m| m.label == label)
            }
            _ => None,
        }
    }

    /// Check if any labels start with the given character
    #[must_use]
    pub fn has_labels_starting_with(&self, c: char) -> bool {
        if let JumpPhase::ShowingLabels { matches, .. } = &self.phase {
            matches.iter().any(|m| m.label.starts_with(c))
        } else {
            false
        }
    }

    /// Handle first character of two-char label (when showing labels)
    ///
    /// Returns true if transitioned to `WaitingLabelSecondChar`, false if invalid
    pub fn handle_label_first_char(&mut self, c: char) -> bool {
        if let JumpPhase::ShowingLabels {
            mode,
            direction,
            pattern,
            matches,
        } = std::mem::take(&mut self.phase)
        {
            // Check if any labels start with this character
            if matches.iter().any(|m| m.label.starts_with(c)) {
                self.phase = JumpPhase::WaitingLabelSecondChar {
                    mode,
                    direction,
                    pattern,
                    matches,
                    first_char: c,
                };
                true
            } else {
                // No labels start with this character - restore state and return false
                self.phase = JumpPhase::ShowingLabels {
                    mode,
                    direction,
                    pattern,
                    matches,
                };
                false
            }
        } else {
            false
        }
    }

    /// Cancel jump mode and return to inactive
    pub fn cancel(&mut self) {
        self.phase = JumpPhase::Inactive;
        self.buffer_id = None;
        self.lines = None;
        self.operator_context = None;
    }

    /// Reset to inactive after successful jump
    pub fn reset(&mut self) {
        self.cancel();
    }
}

// === Shared state wrapper ===

use std::sync::RwLock;

/// Thread-safe shared jump state
pub struct SharedJumpState(pub RwLock<JumpState>);

impl SharedJumpState {
    /// Create a new shared jump state
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new is not const
    pub fn new() -> Self {
        Self(RwLock::new(JumpState::new()))
    }

    /// Access the inner `JumpState` for mutation
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut JumpState) -> R,
    {
        let mut state = self.0.write().expect("JumpState lock poisoned");
        f(&mut state)
    }

    /// Access the inner `JumpState` for reading
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&JumpState) -> R,
    {
        let state = self.0.read().expect("JumpState lock poisoned");
        f(&state)
    }
}

impl Default for SharedJumpState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let mut state = JumpState::new();
        assert!(!state.is_active());

        // Start multi-char search
        state.start_multi_char(1, 10, 5, Direction::Forward, vec![], None);
        assert!(state.is_active());
        assert!(!state.is_showing_labels());

        // First char
        assert!(state.handle_first_char('a'));

        // Second char
        let pattern = state.handle_second_char('b');
        assert_eq!(pattern, Some("ab".to_string()));

        // Show labels
        let matches = vec![
            JumpMatch::new(15, 10, "s".to_string(), 5),
            JumpMatch::new(20, 5, "f".to_string(), 10),
        ];
        state.show_labels("ab".to_string(), matches);
        assert!(state.is_showing_labels());

        // Find match
        let m = state.find_match_by_label("s");
        assert!(m.is_some());
        assert_eq!(m.unwrap().line, 15);

        // Reset
        state.reset();
        assert!(!state.is_active());
    }

    #[test]
    fn test_single_char_mode() {
        let mut state = JumpState::new();

        // Start find char
        state.start_find_char(1, 10, 5, Direction::Forward, vec![], None);
        assert!(state.is_active());

        // Handle char
        let pattern = state.handle_single_char('x');
        assert_eq!(pattern, Some("x".to_string()));

        // Show labels
        state.show_labels("x".to_string(), vec![JumpMatch::new(12, 3, "s".to_string(), 2)]);
        assert!(state.is_showing_labels());
    }

    #[test]
    fn test_cancel() {
        let mut state = JumpState::new();
        state.start_multi_char(1, 10, 5, Direction::Forward, vec![], None);
        assert!(state.is_active());

        state.cancel();
        assert!(!state.is_active());
    }
}
