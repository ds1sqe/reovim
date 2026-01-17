//! Vim operator-pending mode key resolver.
//!
//! After pressing an operator (d, y, c), we enter operator-pending mode
//! and wait for a motion or text object to define the range.

use std::sync::RwLock;

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeySequence, ModeKeyResolver, ModeState, ModeTransition, Modifiers,
        PopResult, ResolveResult,
    },
    reovim_kernel::api::v1::{ModeId, Position},
};

use crate::mode::EditorMode;

/// Vim operator-pending mode key resolver.
///
/// This mode is entered after pressing an operator key (d, y, c) and
/// waits for a motion or text object to complete the operation.
///
/// # Motion Types
///
/// - Character motions: h, l, w, b, e, etc.
/// - Line motions: j, k, 0, $, ^, etc.
/// - Word motions: w, b, e, W, B, E
/// - Text objects: iw, aw, i(, a[, etc.
///
/// # Count Handling
///
/// Counts can appear both before the operator and before the motion:
/// - `2dw` - delete 2 words (count on operator)
/// - `d2w` - delete 2 words (count on motion)
/// - `2d3w` - delete 6 words (counts multiply)
///
/// # Special Keys
///
/// - Escape cancels the pending operator
/// - Repeating the operator key applies to the whole line (dd, yy, cc)
///
/// # Example
///
/// ```ignore
/// let resolver = VimOperatorPendingResolver::new();
///
/// // In operator-pending mode after 'd', receive 'w'
/// // This should look up the word-forward motion and return the range
/// let result = resolver.resolve(&key('w'), &mut state);
/// // Result would be Pop with OperatorRange (after motion execution)
/// ```
pub struct VimOperatorPendingResolver {
    /// Mode ID for operator-pending mode.
    mode_id: ModeId,

    /// Parent mode ID (normal mode) for inheritance.
    parent_mode_id: ModeId,

    /// Accumulated count for the motion.
    pending_count: RwLock<Option<usize>>,

    /// Accumulated key sequence for multi-key motions.
    pending_keys: RwLock<KeySequence>,
}

#[allow(dead_code)] // Methods used in Phase 3 when wiring to EventLoop
impl VimOperatorPendingResolver {
    /// Create a new operator-pending mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: EditorMode::OPERATOR_PENDING_ID,
            parent_mode_id: EditorMode::NORMAL_ID,
            pending_count: RwLock::new(None),
            pending_keys: RwLock::new(KeySequence::new()),
        }
    }

    /// Get the accumulated count, if any.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn pending_count(&self) -> Option<usize> {
        *self.pending_count.read().expect("lock poisoned")
    }

    /// Check if a key is a count digit.
    fn is_count_digit(&self, key: &KeyEvent) -> bool {
        if key.modifiers != Modifiers::NONE {
            return false;
        }

        match key.code {
            KeyCode::Char('1'..='9') => true,
            KeyCode::Char('0') => self.pending_count().is_some(),
            _ => false,
        }
    }

    /// Accumulate a count digit.
    fn accumulate_count(&self, key: &KeyEvent) {
        if let KeyCode::Char(c @ '0'..='9') = key.code {
            let digit = c.to_digit(10).expect("valid digit") as usize;
            let mut guard = self.pending_count.write().expect("lock poisoned");
            *guard = Some(guard.unwrap_or(0) * 10 + digit);
        }
    }

    /// Take the accumulated count, clearing it.
    fn take_count(&self) -> Option<usize> {
        self.pending_count.write().expect("lock poisoned").take()
    }

    /// Clear pending keys.
    fn clear_pending_keys(&self) {
        self.pending_keys.write().expect("lock poisoned").clear();
    }

    /// Clear all internal state (for use from &self via interior mutability).
    fn clear_state(&self) {
        *self.pending_count.write().expect("lock poisoned") = None;
        self.pending_keys.write().expect("lock poisoned").clear();
    }

    /// Add a key to pending sequence.
    fn push_pending_key(&self, key: KeyEvent) {
        self.pending_keys.write().expect("lock poisoned").push(key);
    }

    /// Get a clone of pending keys.
    fn get_pending_keys(&self) -> KeySequence {
        self.pending_keys.read().expect("lock poisoned").clone()
    }

    /// Check if key is escape to cancel operator.
    fn is_escape(key: &KeyEvent) -> bool {
        key.code == KeyCode::Escape
            || (key.code == KeyCode::Char('[') && key.modifiers.contains(Modifiers::CTRL))
    }

    /// Check if key is the operator key for line operation.
    ///
    /// When the same operator key is pressed twice (dd, yy, cc), it operates
    /// on the whole current line.
    fn is_line_operator(key: &KeyEvent, state: &ModeState) -> bool {
        // Check if the key matches the pending operator
        // This requires access to the transition context
        if let Some(ctx) = &state.transition_context
            && let Some(ref op) = ctx.pending_operator
        {
            // Match operator to key
            let op_key = match op.name() {
                "delete" | "enter-delete-operator" => Some('d'),
                "yank" | "enter-yank-operator" => Some('y'),
                "change" | "enter-change-operator" => Some('c'),
                "indent" | "enter-indent-operator" => Some('>'),
                "dedent" | "enter-dedent-operator" => Some('<'),
                _ => None,
            };

            if let Some(expected) = op_key
                && key.modifiers == Modifiers::NONE
                && let KeyCode::Char(c) = key.code
            {
                return c == expected;
            }
        }
        false
    }
}

impl Default for VimOperatorPendingResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimOperatorPendingResolver {
    fn resolve(&self, key: &KeyEvent, state: &mut ModeState) -> ResolveResult {
        // Escape cancels the pending operator
        if Self::is_escape(key) {
            self.clear_state();
            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::Cancelled),
            });
        }

        // Check for count digit
        if self.is_count_digit(key) {
            self.accumulate_count(key);
            return ResolveResult::Pending;
        }

        // Check for line operator (dd, yy, cc)
        if Self::is_line_operator(key, state) {
            // Return a special result indicating line-wise operation
            // The runner will handle this by operating on the current line
            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::OperatorRange {
                    // Placeholder positions - runner will calculate actual line range
                    start: Position::new(0, 0),
                    end: Position::new(0, 0),
                    linewise: true,
                }),
            });
        }

        // Add to pending keys for motion lookup
        self.push_pending_key(*key);

        // Motion/text-object lookup happens via the runner's registry
        // Return NotHandled to delegate to keymap lookup
        ResolveResult::NotHandled
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        // Operator-pending inherits from normal mode for motion keys
        // This allows using all normal mode motions as operator arguments
        Some(&self.parent_mode_id)
    }

    fn reset(&mut self) {
        *self.pending_count.write().expect("lock poisoned") = None;
        self.pending_keys.write().expect("lock poisoned").clear();
    }
}

#[cfg(test)]
mod tests {
    use {reovim_driver_input::TransitionContext, reovim_kernel::api::v1::CommandId};

    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn key_with_mod(c: char, modifiers: Modifiers) -> KeyEvent {
        KeyEvent::with_modifiers(KeyCode::Char(c), modifiers)
    }

    fn test_state() -> ModeState {
        ModeState::new(EditorMode::OPERATOR_PENDING_ID)
    }

    fn test_state_with_operator(op_name: &'static str) -> ModeState {
        let mut state = ModeState::new(EditorMode::OPERATOR_PENDING_ID);
        state.transition_context = Some(TransitionContext::with_operator(CommandId::new(
            reovim_kernel::api::v1::ModuleId::new("editor"),
            op_name,
        )));
        state
    }

    #[test]
    fn test_new_resolver() {
        let resolver = VimOperatorPendingResolver::new();
        assert_eq!(resolver.mode_id(), &EditorMode::OPERATOR_PENDING_ID);
        assert!(resolver.pending_count().is_none());
    }

    #[test]
    fn test_escape_cancels() {
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&KeyEvent::new(KeyCode::Escape), &mut state);

        if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
            assert!(matches!(r, PopResult::Cancelled));
        } else {
            panic!("expected ModeTransition::Pop with Cancelled");
        }
    }

    #[test]
    fn test_ctrl_bracket_cancels() {
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&key_with_mod('[', Modifiers::CTRL), &mut state);

        if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
            assert!(matches!(r, PopResult::Cancelled));
        } else {
            panic!("expected ModeTransition::Pop with Cancelled for Ctrl+[");
        }
    }

    #[test]
    fn test_count_digit() {
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&key('3'), &mut state);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.pending_count(), Some(3));

        let result = resolver.resolve(&key('5'), &mut state);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.pending_count(), Some(35));
    }

    #[test]
    fn test_line_operator_dd() {
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state_with_operator("delete");

        let result = resolver.resolve(&key('d'), &mut state);

        if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
            if let PopResult::OperatorRange { linewise, .. } = r {
                assert!(linewise);
            } else {
                panic!("expected OperatorRange");
            }
        } else {
            panic!("expected ModeTransition::Pop");
        }
    }

    #[test]
    fn test_line_operator_yy() {
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state_with_operator("yank");

        let result = resolver.resolve(&key('y'), &mut state);

        if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
            if let PopResult::OperatorRange { linewise, .. } = r {
                assert!(linewise);
            } else {
                panic!("expected OperatorRange");
            }
        } else {
            panic!("expected ModeTransition::Pop");
        }
    }

    #[test]
    fn test_motion_key_not_handled() {
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();

        // 'w' (word forward) should go to keymap lookup
        let result = resolver.resolve(&key('w'), &mut state);
        assert!(matches!(result, ResolveResult::NotHandled));

        // Key should be accumulated for lookup
        assert!(!resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_inherits_from_normal() {
        let resolver = VimOperatorPendingResolver::new();
        let parent = resolver.inherits_from();
        assert!(parent.is_some());
        assert_eq!(parent.unwrap().name(), "normal");
    }

    #[test]
    fn test_reset() {
        let mut resolver = VimOperatorPendingResolver::new();

        resolver.accumulate_count(&key('5'));
        resolver.push_pending_key(key('w'));

        resolver.reset();

        assert!(resolver.pending_count().is_none());
        assert!(resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_mode_id() {
        let resolver = VimOperatorPendingResolver::new();
        assert_eq!(resolver.mode_id().name(), "operator-pending");
    }
}
