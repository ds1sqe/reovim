//! Jump input handling
//!
//! Coordinates character input with state machine and search engine.

#![allow(dead_code)] // Temporary: will be used once wired up

use super::{
    command::{Direction, JumpExecute, JumpMode},
    search::{find_matches, should_auto_jump, too_many_matches},
    state::{JumpPhase, JumpState},
};

/// Result of handling character input
#[derive(Debug)]
pub enum InputResult {
    /// Continue waiting for more input
    Continue,
    /// Execute a jump to the given target
    Jump(JumpExecute),
    /// Show labels for selection
    ShowLabels,
    /// Cancel jump mode (invalid input or too many matches)
    Cancel(CancelReason),
}

/// Reason for canceling jump mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    /// No matches found
    NoMatches,
    /// Too many matches (>20)
    TooManyMatches,
    /// Invalid input
    InvalidInput,
    /// User canceled (Escape)
    UserCanceled,
}

/// Handle character input during jump mode
///
/// # Arguments
/// * `state` - Current jump state (will be mutated)
/// * `c` - Input character
/// * `lines` - Buffer lines for searching
///
/// Returns what action should be taken next
#[allow(clippy::too_many_lines)]
pub fn handle_char_input(state: &mut JumpState, c: char, lines: &[String]) -> InputResult {
    tracing::info!("==> handle_char_input: c='{}', phase={:?}", c, state.phase);

    // Extract data we need before borrowing state mutably
    let (mode, direction) = match &state.phase {
        JumpPhase::WaitingFirstChar { .. } => {
            tracing::debug!("==> In WaitingFirstChar phase");
            // Store first character and wait for second
            let result = if state.handle_first_char(c) {
                InputResult::Continue
            } else {
                InputResult::Cancel(CancelReason::InvalidInput)
            };
            tracing::info!("==> WaitingFirstChar result: {:?}", result);
            return result;
        }

        JumpPhase::WaitingSecondChar {
            mode, direction, ..
        }
        | JumpPhase::WaitingSingleChar { mode, direction } => {
            tracing::debug!(
                "==> In WaitingSecondChar/WaitingSingleChar phase, mode={:?}, direction={:?}",
                mode,
                direction
            );
            (*mode, *direction)
        }

        JumpPhase::ShowingLabels { matches, .. } => {
            tracing::debug!("==> In ShowingLabels phase, treating input as label");

            // Check if we're using single-char or two-char labels
            let label_len = matches.first().map_or(1, |m| m.label.len());

            if label_len == 1 {
                // Single-char labels - immediate selection
                return handle_label_selection(state, &c.to_string());
            }

            // Two-char labels - handle first character
            tracing::debug!("==> Two-char labels detected, handling first char '{}'", c);
            let valid = state.handle_label_first_char(c);
            if valid {
                tracing::debug!("==> Valid first char, waiting for second char");
                return InputResult::Continue;
            }

            tracing::warn!("==> No labels start with '{}', canceling", c);
            return InputResult::Cancel(CancelReason::InvalidInput);
        }

        JumpPhase::WaitingLabelSecondChar { first_char, .. } => {
            tracing::debug!(
                "==> In WaitingLabelSecondChar phase, first_char='{}', second_char='{}'",
                first_char,
                c
            );
            // Combine first and second character into full label
            let label = format!("{first_char}{c}");
            tracing::debug!("==> Looking up label '{}'", label);
            return handle_label_selection(state, &label);
        }

        JumpPhase::Inactive => {
            tracing::debug!("==> In Inactive phase - canceling");
            // Not in jump mode - ignore
            return InputResult::Cancel(CancelReason::InvalidInput);
        }
    };

    // Now we can safely mutate state
    let is_multi_char = matches!(state.phase, JumpPhase::WaitingSecondChar { .. });
    tracing::debug!("==> is_multi_char={}", is_multi_char);

    let pattern = if is_multi_char {
        tracing::debug!("==> Calling state.handle_second_char({})", c);
        state.handle_second_char(c)
    } else {
        tracing::debug!("==> Calling state.handle_single_char({})", c);
        state.handle_single_char(c)
    };

    tracing::debug!("==> Pattern after handling char: {:?}", pattern);

    if let Some(pattern) = pattern {
        // Search for matches
        tracing::info!("==> Searching for pattern '{}' in {} lines", pattern, lines.len());
        let matches = find_matches(&pattern, lines, state.start_line, state.start_col, direction);
        tracing::info!("==> Found {} matches", matches.len());

        // Apply auto-jump logic
        if matches.is_empty() {
            tracing::info!("==> No matches, canceling");
            state.cancel();
            InputResult::Cancel(CancelReason::NoMatches)
        } else if too_many_matches(matches.len()) {
            tracing::info!("==> Too many matches ({}), canceling", matches.len());
            state.cancel();
            InputResult::Cancel(CancelReason::TooManyMatches)
        } else if should_auto_jump(matches.len()) {
            tracing::info!("==> Single match, auto-jumping");
            // Single match - jump immediately
            let m = &matches[0];
            let buffer_id = state.buffer_id.unwrap_or(0);

            // Adjust column for Till mode (cursor before/after match)
            let col = match (mode, direction) {
                (JumpMode::TillChar, Direction::Forward) => m.col.saturating_sub(1),
                (JumpMode::TillChar, Direction::Backward) => m.col.saturating_add(1),
                _ => m.col, // FindChar and MultiChar: cursor on match
            };

            // Don't reset here - let lib.rs reset AFTER getting operator_context
            InputResult::Jump(JumpExecute {
                buffer_id,
                line: m.line,
                col,
                mode,
                direction,
            })
        } else {
            // Multiple matches - show labels
            tracing::debug!(
                "Input handler: Found {} matches for pattern '{}', transitioning to ShowingLabels",
                matches.len(),
                pattern
            );
            state.show_labels(pattern, matches);
            tracing::debug!("Input handler: State is_showing_labels={}", state.is_showing_labels());
            InputResult::ShowLabels
        }
    } else {
        InputResult::Cancel(CancelReason::InvalidInput)
    }
}

/// Handle label selection
///
/// # Arguments
/// * `state` - Current jump state (will be mutated)
/// * `label` - Selected label string
///
/// Returns jump execution or cancel
pub fn handle_label_selection(state: &mut JumpState, label: &str) -> InputResult {
    // Extract data before mutating state
    let match_data = state.find_match_by_label(label).map(|m| (m.line, m.col));

    // Extract mode and direction from either ShowingLabels or WaitingLabelSecondChar
    let (mode, direction) = match &state.phase {
        JumpPhase::ShowingLabels {
            mode, direction, ..
        }
        | JumpPhase::WaitingLabelSecondChar {
            mode, direction, ..
        } => (*mode, *direction),
        _ => {
            tracing::warn!("handle_label_selection called in wrong phase: {:?}", state.phase);
            return InputResult::Cancel(CancelReason::InvalidInput);
        }
    };

    if let Some((line, col)) = match_data {
        // Found match - execute jump
        let buffer_id = state.buffer_id.unwrap_or(0);

        // Adjust column for Till mode (cursor before/after match)
        let col = match (mode, direction) {
            (JumpMode::TillChar, Direction::Forward) => col.saturating_sub(1),
            (JumpMode::TillChar, Direction::Backward) => col.saturating_add(1),
            _ => col, // FindChar and MultiChar: cursor on match
        };

        // Don't reset here - let lib.rs reset AFTER getting operator_context
        InputResult::Jump(JumpExecute {
            buffer_id,
            line,
            col,
            mode,
            direction,
        })
    } else {
        // Invalid label
        state.cancel();
        InputResult::Cancel(CancelReason::InvalidInput)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::jump::command::{Direction, JumpMode},
    };

    #[test]
    fn test_multi_char_input() {
        let mut state = JumpState::new();
        state.start_multi_char(1, 0, 0, Direction::Forward, vec![], None);

        let lines = vec![
            "hello world".to_string(),
            "hello rust".to_string(),
            "hello there".to_string(),
        ];

        // First char
        let result = handle_char_input(&mut state, 'h', &lines);
        assert!(matches!(result, InputResult::Continue));

        // Second char - should find 3 matches (lines 1 and 2, excluding cursor at line 0)
        // Actually, "he" at (0, 0) is excluded by forward search
        // So finds: (1, 0) and (2, 0) and (2, 6) = 3 matches
        let result = handle_char_input(&mut state, 'e', &lines);
        assert!(matches!(result, InputResult::ShowLabels));

        // Verify state transitioned to ShowingLabels
        assert!(state.is_showing_labels());
        let matches = state.get_matches().unwrap();
        assert!(matches.len() >= 2); // At least 2 matches to show labels
    }

    #[test]
    fn test_auto_jump() {
        let mut state = JumpState::new();
        state.start_multi_char(1, 0, 0, Direction::Forward, vec![], None);

        let lines = vec!["hello world".to_string()];

        // First char
        handle_char_input(&mut state, 'w', &lines);

        // Second char - only 1 match, should auto-jump
        let result = handle_char_input(&mut state, 'o', &lines);
        assert!(matches!(result, InputResult::Jump(_)));

        // Note: State is NOT reset here - lib.rs resets AFTER getting operator_context
        // This allows operator integration (d+s+pattern) to work correctly
        assert!(state.is_active());
    }

    #[test]
    fn test_no_matches() {
        let mut state = JumpState::new();
        state.start_multi_char(1, 0, 0, Direction::Forward, vec![], None);

        let lines = vec!["hello world".to_string()];

        handle_char_input(&mut state, 'x', &lines);

        let result = handle_char_input(&mut state, 'y', &lines);
        assert!(matches!(result, InputResult::Cancel(CancelReason::NoMatches)));
    }

    #[test]
    fn test_label_selection() {
        let mut state = JumpState::new();
        state.start_multi_char(1, 0, 0, Direction::Forward, vec![], None);

        let lines = vec![
            "hello world".to_string(),
            "hello rust".to_string(),
            "hello there".to_string(),
        ];

        // Input "he" to get labels (will find 3+ matches)
        handle_char_input(&mut state, 'h', &lines);
        handle_char_input(&mut state, 'e', &lines);

        // Should be showing labels now
        assert!(state.is_showing_labels());

        // Select first label
        let result = handle_label_selection(&mut state, "s");
        assert!(matches!(result, InputResult::Jump(_)));

        if let InputResult::Jump(jump) = result {
            assert_eq!(jump.mode, JumpMode::MultiChar);
        }
    }

    #[test]
    fn test_single_char_mode() {
        let mut state = JumpState::new();
        state.start_find_char(1, 0, 0, Direction::Forward, vec![], None);

        let lines = vec!["hello world hello".to_string(), "h is a letter".to_string()];

        // Input 'h' - should find multiple matches
        let result = handle_char_input(&mut state, 'h', &lines);
        assert!(matches!(result, InputResult::ShowLabels));

        let matches = state.get_matches().unwrap();
        assert!(matches.len() >= 2);
    }
}
