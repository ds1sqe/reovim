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

    /// Start a jump session with pre-computed matches (e.g., from f/t motions).
    ///
    /// Skips the two-character search and jumps directly to `ShowingLabels`.
    /// The matches must already have labels assigned via [`generate_labels`].
    ///
    /// # Panics
    ///
    /// Does not panic on empty matches — simply stays inactive.
    pub fn start_with_matches(&mut self, matches: Vec<JumpMatch>) {
        self.target = None;
        self.lines.clear();

        if matches.is_empty() {
            self.phase = JumpPhase::Inactive;
            return;
        }

        let label_len = matches[0].label.len();
        self.phase = JumpPhase::ShowingLabels { matches, label_len };
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

    /// Whether a resolved jump target is available (non-consuming).
    #[must_use]
    pub const fn has_target(&self) -> bool {
        self.target.is_some()
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
#[path = "state_tests.rs"]
mod tests;
