//! Shared logic for operator mode resolvers (delete, yank, change).
//!
//! This module contains common patterns extracted from the operator resolution flow:
//! - Escape handling (cancel operator)
//! - Count digit accumulation
//! - Linewise motion detection
//! - Building `PopResult::ExecuteCommand` with operator arguments
//! - Key sequence management

use std::collections::HashMap;

use {
    reovim_driver_command_types::ArgValue,
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeTransition, Modifiers, PopResult,
    },
    reovim_kernel::api::v1::{CommandId, ModeId, Position},
};

use crate::{ids::OperatorId, modes::VimMode};

// =============================================================================
// Operator Type
// =============================================================================

/// The type of operator being executed.
///
/// This enum identifies which operator mode we're in, enabling the resolver
/// to know what command to execute when the motion completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorType {
    /// Delete operator (d)
    Delete,
    /// Yank operator (y)
    Yank,
    /// Change operator (c)
    Change,
}

impl OperatorType {
    /// Get the mode ID for this operator type.
    #[must_use]
    pub const fn mode_id(&self) -> ModeId {
        match self {
            Self::Delete => VimMode::DELETE_ID,
            Self::Yank => VimMode::YANK_ID,
            Self::Change => VimMode::CHANGE_ID,
        }
    }

    /// Get the operator ID for this operator type.
    #[must_use]
    pub fn operator_id(&self) -> OperatorId {
        match self {
            Self::Delete => crate::ids::DELETE.clone(),
            Self::Yank => crate::ids::YANK.clone(),
            Self::Change => crate::ids::CHANGE.clone(),
        }
    }

    /// Get the key that triggers linewise operation (doubled operator).
    #[must_use]
    pub const fn line_key(&self) -> char {
        match self {
            Self::Delete => 'd',
            Self::Yank => 'y',
            Self::Change => 'c',
        }
    }

    /// Get the display name for this operator.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Delete => "DELETE",
            Self::Yank => "YANK",
            Self::Change => "CHANGE",
        }
    }
}

// =============================================================================
// Common Helper Functions
// =============================================================================

/// Check if key is escape (cancels operator).
#[must_use]
pub fn is_escape(key: &KeyEvent) -> bool {
    key.code == KeyCode::Escape
        || (key.code == KeyCode::Char('[') && key.modifiers.contains(Modifiers::CTRL))
}

/// Check if a key is a count digit.
///
/// - First digit must be 1-9 (0 is a motion in Vim)
/// - Subsequent digits can be 0-9
#[must_use]
pub fn is_count_digit(key: &KeyEvent, has_count: bool) -> bool {
    if key.modifiers != Modifiers::NONE {
        return false;
    }

    match key.code {
        KeyCode::Char('1'..='9') => true,
        KeyCode::Char('0') => has_count, // 0 is only count if we already have a count
        _ => false,
    }
}

/// Accumulate a count digit into the existing count.
///
/// Returns the new count value.
#[must_use]
pub fn accumulate_count(key: &KeyEvent, current: Option<usize>) -> Option<usize> {
    if let KeyCode::Char(c @ '0'..='9') = key.code {
        let digit = c.to_digit(10).expect("valid digit") as usize;
        Some(current.unwrap_or(0) * 10 + digit)
    } else {
        current
    }
}

/// Check if key is the operator's line key (for dd, yy, cc).
#[must_use]
pub fn is_line_operator_key(key: &KeyEvent, operator: OperatorType) -> bool {
    key.modifiers == Modifiers::NONE && key.code == KeyCode::Char(operator.line_key())
}

/// Determine if a motion command is linewise.
///
/// This is vim policy knowledge - the resolver knows which motions
/// are linewise vs characterwise based on the command name.
///
/// # Linewise Motions
/// - j, k (line up/down)
/// - gg, G (document start/end)
/// - H, M, L (screen positions)
/// - {, } (paragraph motions)
/// - +, - (line motions)
/// - whole-line (for dd, yy, cc)
///
/// # Characterwise Motions (default)
/// - w, b, e, W, B, E (word motions)
/// - h, l (character motions)
/// - 0, $, ^ (line position motions)
/// - f, F, t, T (find-char motions)
#[must_use]
pub fn is_linewise_motion(cmd: &CommandId) -> bool {
    let name = cmd.name();
    matches!(
        name,
        "move-down"
            | "move-up"
            | "document-start"
            | "document-end"
            | "screen-top"
            | "screen-middle"
            | "screen-bottom"
            | "paragraph-forward"
            | "paragraph-backward"
            | "next-line"
            | "prev-line"
            | "whole-line"
    )
}

/// Build a `PopResult::ExecuteCommand` with operator arguments.
///
/// This constructs the complete command context for executing an operator
/// with the given range. The runner just executes - no vim knowledge needed.
#[must_use]
pub fn build_operator_execute(
    operator: OperatorType,
    start: Position,
    end: Position,
    linewise: bool,
    count: Option<usize>,
    register: Option<char>,
) -> PopResult {
    let operator_id = operator.operator_id();
    let command = CommandId::new(operator_id.module().clone(), operator_id.name());

    let mut args = HashMap::new();

    // Set linewise flag
    args.insert("linewise".to_string(), ArgValue::Bang(linewise));

    // Set range positions
    args.insert("range_start".to_string(), ArgValue::Position(start.line, start.column));
    args.insert("range_end".to_string(), ArgValue::Position(end.line, end.column));

    // Set count (defaults to 1)
    args.insert("count".to_string(), ArgValue::Count(count.unwrap_or(1)));

    // Set register if specified
    if let Some(reg) = register {
        args.insert("register".to_string(), ArgValue::Register(reg));
    }

    PopResult::ExecuteCommand { command, args }
}

/// Build a cancellation mode transition.
#[must_use]
pub fn build_cancelled() -> ModeTransition {
    ModeTransition::Pop {
        result: Some(PopResult::Cancelled),
    }
}

/// Apply Vim policy to determine the next action based on keymap lookup.
///
/// This is reusable across all operator resolvers since they share the same policy:
/// - `ExactOnly` or `ExactWithLonger` → Execute the motion
/// - `PrefixOnly` → Wait for more keys
/// - `NotFound` → Cancel the operator
pub fn apply_keymap_policy(lookup: &KeyLookupState) -> KeymapAction {
    match lookup {
        KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
            KeymapAction::Execute(cmd.clone())
        }
        KeyLookupState::PrefixOnly => KeymapAction::Pending,
        KeyLookupState::NotFound => KeymapAction::Cancel,
    }
}

/// Result of applying keymap policy.
#[derive(Debug, Clone)]
pub enum KeymapAction {
    /// Execute the found command (motion/text-object).
    Execute(CommandId),
    /// Wait for more keys (prefix match).
    Pending,
    /// Cancel the operator (no match).
    Cancel,
}

// =============================================================================
// Operator State
// =============================================================================

/// State owned by an operator resolver.
///
/// Unlike `VimSessionState.pending_operator`, this state is owned by the resolver
/// itself. This makes testing easier and eliminates runtime lookup of operator type.
///
/// # State Lifecycle
///
/// When a mode is pushed (e.g., user presses 'd' in normal mode):
/// 1. `initialized` is false
/// 2. First `resolve_with_session` call reads `pending_count`/`pending_register` from VimSessionState
/// 3. Sets `initialized = true`
/// 4. Subsequent keys use the cached state
/// 5. When mode is popped or cancelled, `reset()` clears the state
#[derive(Debug, Clone)]
pub struct OperatorState {
    /// The type of operator (delete, yank, change).
    pub operator: OperatorType,
    /// Start position captured when entering operator mode.
    pub start_position: Option<Position>,
    /// Count applied before the operator (e.g., `2d` in `2dw`).
    pub operator_count: Option<usize>,
    /// Count applied to the motion (e.g., `3` in `d3w`).
    pub motion_count: Option<usize>,
    /// Target register for the operation.
    pub register: Option<char>,
    /// Accumulated key sequence for multi-key motions.
    pub pending_keys: KeySequence,
    /// Whether the state has been initialized with context from normal mode.
    ///
    /// On first key press in the mode, the resolver reads `pending_count` and
    /// `pending_register` from VimSessionState and sets this to true.
    pub initialized: bool,
}

impl OperatorState {
    /// Create a new operator state.
    ///
    /// The state starts uninitialized. On first key press, the resolver
    /// will read `pending_count` and `pending_register` from VimSessionState.
    #[must_use]
    pub fn new(operator: OperatorType) -> Self {
        Self {
            operator,
            start_position: None,
            operator_count: None,
            motion_count: None,
            register: None,
            pending_keys: KeySequence::new(),
            initialized: false,
        }
    }

    /// Create operator state with initial count and register.
    ///
    /// Used in tests to pre-initialize state without going through VimSessionState.
    #[must_use]
    pub fn with_context(
        operator: OperatorType,
        count: Option<usize>,
        register: Option<char>,
    ) -> Self {
        Self {
            operator,
            start_position: None,
            operator_count: count,
            motion_count: None,
            register,
            pending_keys: KeySequence::new(),
            initialized: true, // Pre-initialized for testing
        }
    }

    /// Set the start position.
    pub fn set_start_position(&mut self, pos: Position) {
        self.start_position = Some(pos);
    }

    /// Get the effective count (operator_count * motion_count, defaulting to 1).
    #[must_use]
    pub fn effective_count(&self) -> usize {
        let op = self.operator_count.unwrap_or(1);
        let motion = self.motion_count.unwrap_or(1);
        op * motion
    }

    /// Check if we have a motion count.
    #[must_use]
    pub fn has_motion_count(&self) -> bool {
        self.motion_count.is_some()
    }

    /// Accumulate a motion count digit.
    pub fn accumulate_motion_count(&mut self, key: &KeyEvent) {
        self.motion_count = accumulate_count(key, self.motion_count);
    }

    /// Take the motion count, clearing it.
    pub fn take_motion_count(&mut self) -> Option<usize> {
        self.motion_count.take()
    }

    /// Add a key to pending sequence.
    pub fn push_key(&mut self, key: KeyEvent) {
        self.pending_keys.push(key);
    }

    /// Get a clone of pending keys.
    #[must_use]
    pub fn keys(&self) -> KeySequence {
        self.pending_keys.clone()
    }

    /// Clear pending keys.
    pub fn clear_keys(&mut self) {
        self.pending_keys.clear();
    }

    /// Reset all state for mode re-entry.
    ///
    /// Called when the mode is exited (cancelled or completed). Clears all
    /// state including count and register so they can be re-read from
    /// VimSessionState on the next mode entry.
    pub fn reset(&mut self) {
        self.start_position = None;
        self.operator_count = None;
        self.motion_count = None;
        self.register = None;
        self.pending_keys.clear();
        self.initialized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    #[test]
    fn test_is_escape() {
        assert!(is_escape(&KeyEvent::new(KeyCode::Escape)));
        assert!(is_escape(&KeyEvent::with_modifiers(KeyCode::Char('['), Modifiers::CTRL)));
        assert!(!is_escape(&key('d')));
    }

    #[test]
    fn test_is_count_digit() {
        // First digit
        assert!(is_count_digit(&key('1'), false));
        assert!(is_count_digit(&key('9'), false));
        assert!(!is_count_digit(&key('0'), false)); // 0 is motion when no count

        // Subsequent digit
        assert!(is_count_digit(&key('0'), true));

        // With modifiers - not a count digit
        assert!(!is_count_digit(
            &KeyEvent::with_modifiers(KeyCode::Char('3'), Modifiers::CTRL),
            false
        ));
    }

    #[test]
    fn test_accumulate_count() {
        assert_eq!(accumulate_count(&key('3'), None), Some(3));
        assert_eq!(accumulate_count(&key('5'), Some(3)), Some(35));
        assert_eq!(accumulate_count(&key('0'), Some(35)), Some(350));
    }

    #[test]
    fn test_is_line_operator_key() {
        assert!(is_line_operator_key(&key('d'), OperatorType::Delete));
        assert!(!is_line_operator_key(&key('y'), OperatorType::Delete));
        assert!(is_line_operator_key(&key('y'), OperatorType::Yank));
        assert!(is_line_operator_key(&key('c'), OperatorType::Change));
    }

    #[test]
    fn test_is_linewise_motion() {
        use reovim_kernel::api::v1::ModuleId;

        let editor = ModuleId::new("editor");

        assert!(is_linewise_motion(&CommandId::new(editor.clone(), "move-down")));
        assert!(is_linewise_motion(&CommandId::new(editor.clone(), "whole-line")));
        assert!(!is_linewise_motion(&CommandId::new(editor.clone(), "cursor-left")));
    }

    #[test]
    fn test_operator_state_new() {
        let state = OperatorState::new(OperatorType::Delete);
        assert_eq!(state.operator, OperatorType::Delete);
        assert!(state.start_position.is_none());
        assert!(state.operator_count.is_none());
        assert!(state.motion_count.is_none());
        assert!(!state.initialized, "new state should not be initialized");
    }

    #[test]
    fn test_operator_state_with_context() {
        let state = OperatorState::with_context(OperatorType::Delete, Some(3), Some('a'));
        assert_eq!(state.operator, OperatorType::Delete);
        assert_eq!(state.operator_count, Some(3));
        assert_eq!(state.register, Some('a'));
        assert!(state.initialized, "with_context should be pre-initialized");
    }

    #[test]
    fn test_operator_state_reset() {
        let mut state = OperatorState::with_context(OperatorType::Yank, Some(2), Some('b'));
        state.start_position = Some(Position::new(5, 10));
        state.motion_count = Some(3);
        state.push_key(KeyEvent::new(KeyCode::Char('w')));

        state.reset();

        assert!(state.start_position.is_none());
        assert!(state.operator_count.is_none());
        assert!(state.motion_count.is_none());
        assert!(state.register.is_none());
        assert!(state.pending_keys.is_empty());
        assert!(!state.initialized, "reset should clear initialized flag");
    }

    #[test]
    fn test_operator_state_effective_count() {
        let mut state = OperatorState::new(OperatorType::Yank);
        assert_eq!(state.effective_count(), 1); // No counts

        state.operator_count = Some(2);
        assert_eq!(state.effective_count(), 2);

        state.motion_count = Some(3);
        assert_eq!(state.effective_count(), 6); // 2 * 3
    }

    #[test]
    fn test_operator_type_mode_id() {
        assert_eq!(OperatorType::Delete.mode_id(), VimMode::DELETE_ID);
        assert_eq!(OperatorType::Yank.mode_id(), VimMode::YANK_ID);
        assert_eq!(OperatorType::Change.mode_id(), VimMode::CHANGE_ID);
    }
}
