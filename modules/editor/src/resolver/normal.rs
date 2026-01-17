//! Vim normal mode key resolver.
//!
//! Handles normal mode key interpretation including:
//! - Count prefix accumulation (1-9, then 0)
//! - Register prefix (") handling
//! - Command key lookup via keymap registry
//! - Operator entry transitions

use std::sync::RwLock;

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeySequence, ModeKeyResolver, ModeState, ModeTransition, Modifiers,
        ResolveContext, ResolveInput, ResolveResult, TransitionContext,
    },
    reovim_kernel::api::v1::{CommandId, ModeId},
};

use crate::mode::EditorMode;

/// Vim normal mode key resolver.
///
/// In normal mode:
/// - Digits 1-9 (and 0 after other digits) accumulate as count prefix
/// - `"` followed by a character selects a register
/// - Other keys are looked up in the keymap
/// - Operators (d, y, c) trigger transition to operator-pending mode
///
/// # State Management
///
/// The resolver owns its state (counts, pending register) rather than
/// storing it externally. This enables:
/// - Unit testing without full runner
/// - Different editing styles with different state needs
/// - Clean hot-reload (replace resolver, state resets)
///
/// # Example
///
/// ```ignore
/// let resolver = VimNormalResolver::new();
///
/// // Process '3' - accumulates count
/// let result = resolver.resolve(&key_3, &mut state);
/// assert!(matches!(result, ResolveResult::Pending));
///
/// // Process 'j' - executes cursor-down with count=3
/// let result = resolver.resolve(&key_j, &mut state);
/// assert!(matches!(result, ResolveResult::Execute(..)));
/// ```
pub struct VimNormalResolver {
    /// Mode ID for normal mode.
    mode_id: ModeId,

    /// Accumulated count prefix.
    ///
    /// - `None`: No count yet
    /// - `Some(n)`: Count is n
    ///
    /// Reset after command execution or escape.
    pending_count: RwLock<Option<usize>>,

    /// Pending register selection.
    ///
    /// - `None`: No register prefix
    /// - `Some('"')`: Waiting for register character (sentinel)
    /// - `Some('a'..'z')`: Register selected
    ///
    /// Reset after command execution or escape.
    pending_register: RwLock<Option<char>>,

    /// Accumulated key sequence for multi-key commands.
    pending_keys: RwLock<KeySequence>,
}

#[allow(dead_code)] // Methods used in Phase 3 when wiring to EventLoop
impl VimNormalResolver {
    /// Create a new normal mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: EditorMode::NORMAL_ID,
            pending_count: RwLock::new(None),
            pending_register: RwLock::new(None),
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

    /// Get the pending register, if any.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn pending_register(&self) -> Option<char> {
        *self.pending_register.read().expect("lock poisoned")
    }

    /// Check if waiting for register character.
    #[must_use]
    pub fn is_waiting_for_register(&self) -> bool {
        self.pending_register() == Some('"')
    }

    /// Check if a key is a count digit.
    ///
    /// - First digit must be 1-9 (not 0, since 0 is a motion)
    /// - Subsequent digits can be 0-9
    fn is_count_digit(&self, key: &KeyEvent) -> bool {
        // Only plain digits (no modifiers) are count digits
        if key.modifiers != Modifiers::NONE {
            return false;
        }

        match key.code {
            KeyCode::Char('1'..='9') => true,
            KeyCode::Char('0') => {
                // 0 is only a count digit if we already have a count
                self.pending_count().is_some()
            }
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

    /// Check if a key is the register prefix (`"`).
    fn is_register_prefix(key: &KeyEvent) -> bool {
        key.modifiers == Modifiers::NONE && key.code == KeyCode::Char('"')
    }

    /// Handle register character after `"` prefix.
    fn handle_register_char(&self, key: &KeyEvent) -> ResolveResult {
        if let KeyCode::Char(c) = key.code {
            // Valid register characters: a-z, A-Z, 0-9, and special registers
            if c.is_ascii_alphanumeric() || "+-*/.%#:".contains(c) {
                *self.pending_register.write().expect("lock poisoned") = Some(c);
                // Don't execute yet - wait for command
                return ResolveResult::Pending;
            }
        }

        // Invalid register character - cancel and pass key through
        *self.pending_register.write().expect("lock poisoned") = None;
        ResolveResult::NotHandled
    }

    /// Take the accumulated count, clearing it.
    fn take_count(&self) -> Option<usize> {
        self.pending_count.write().expect("lock poisoned").take()
    }

    /// Take the pending register, clearing it.
    fn take_register(&self) -> Option<char> {
        let reg = self.pending_register.write().expect("lock poisoned").take();
        // Don't return the sentinel
        reg.filter(|&r| r != '"')
    }

    /// Clear pending keys.
    fn clear_pending_keys(&self) {
        self.pending_keys.write().expect("lock poisoned").clear();
    }

    /// Add a key to pending sequence.
    fn push_pending_key(&self, key: KeyEvent) {
        self.pending_keys.write().expect("lock poisoned").push(key);
    }

    /// Get a clone of pending keys for lookup.
    fn get_pending_keys(&self) -> KeySequence {
        self.pending_keys.read().expect("lock poisoned").clone()
    }

    /// Clear all internal state (for use from &self via interior mutability).
    fn clear_state(&self) {
        *self.pending_count.write().expect("lock poisoned") = None;
        *self.pending_register.write().expect("lock poisoned") = None;
        self.pending_keys.write().expect("lock poisoned").clear();
    }

    /// Build resolve context with count and register.
    fn build_context(&self, keys: KeySequence) -> ResolveContext {
        let mut ctx = ResolveContext::new().keys(keys);

        if let Some(count) = self.take_count() {
            ctx = ctx.count(count);
        }

        if let Some(reg) = self.take_register() {
            ctx = ctx.register(reg);
        }

        ctx
    }

    /// Create a mode transition to operator-pending mode.
    fn enter_operator_pending(&self, operator: CommandId) -> ResolveResult {
        let ctx = TransitionContext::with_operator(operator)
            .count(self.take_count().unwrap_or(1))
            .register(self.take_register().unwrap_or('\0'));

        ResolveResult::ModeTransition(ModeTransition::Push {
            mode: EditorMode::OPERATOR_PENDING_ID,
            context: ctx,
        })
    }
}

impl Default for VimNormalResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimNormalResolver {
    fn resolve(&self, key: &KeyEvent, _state: &mut ModeState) -> ResolveResult {
        // Handle escape - reset state and stay in normal mode
        if key.code == KeyCode::Escape {
            self.clear_state();
            return ResolveResult::NotHandled;
        }

        // Check for register prefix waiting for character
        if self.is_waiting_for_register() {
            return self.handle_register_char(key);
        }

        // Check for register prefix start
        if Self::is_register_prefix(key) {
            *self.pending_register.write().expect("lock poisoned") = Some('"'); // Sentinel
            return ResolveResult::Pending;
        }

        // Check for count digit
        if self.is_count_digit(key) {
            self.accumulate_count(key);
            return ResolveResult::Pending;
        }

        // Add to pending keys for lookup
        self.push_pending_key(*key);
        let _keys = self.get_pending_keys();

        // Legacy: return NotHandled to let the runner do the lookup
        // Use resolve_with_keymap for keymap-aware resolution
        ResolveResult::NotHandled
    }

    /// Vim-style key resolution with keymap access.
    ///
    /// **Note**: Currently delegates to `resolve()` for backward compatibility.
    /// The keymap-aware resolution infrastructure is in place for future use,
    /// but the runner's existing multi-key handling is used for now.
    ///
    /// # Future Vim Policy (not yet active)
    ///
    /// When fully implemented, this will:
    /// - Query the keymap to get FACTS about what bindings exist
    /// - Apply Vim policy: prefer longer sequences (wait for `dd` after `d`)
    ///
    /// | Lookup State | Vim Behavior |
    /// |--------------|--------------|
    /// | `ExactWithLonger` | `Pending` - wait for more keys |
    /// | `ExactOnly` | `Execute` - run the command |
    /// | `PrefixOnly` | `Pending` - wait for more keys |
    /// | `NotFound` | `NotHandled` - delegate to fallback |
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        _input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Delegate to legacy resolve() for backward compatibility.
        //
        // The keymap-aware infrastructure (KeymapQuery, ResolveInput, etc.) is
        // in place, but full integration requires unifying the runner's pending
        // key management with the resolver's internal state.
        //
        // TODO(Epic #353 Phase 3): Enable full keymap-aware resolution once the
        // runner's pending key management is unified with the resolver.
        self.resolve(key, state)
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        None
    }

    fn reset(&mut self) {
        *self.pending_count.write().expect("lock poisoned") = None;
        *self.pending_register.write().expect("lock poisoned") = None;
        self.pending_keys.write().expect("lock poisoned").clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn key_with_mod(c: char, modifiers: Modifiers) -> KeyEvent {
        KeyEvent::with_modifiers(KeyCode::Char(c), modifiers)
    }

    fn test_state() -> ModeState {
        ModeState::new(EditorMode::NORMAL_ID)
    }

    #[test]
    fn test_new_resolver() {
        let resolver = VimNormalResolver::new();
        assert_eq!(resolver.mode_id(), &EditorMode::NORMAL_ID);
        assert!(resolver.pending_count().is_none());
        assert!(resolver.pending_register().is_none());
    }

    #[test]
    fn test_is_count_digit_first() {
        let resolver = VimNormalResolver::new();

        // 1-9 are count digits when no count yet
        for c in '1'..='9' {
            assert!(resolver.is_count_digit(&key(c)), "'{c}' should be count digit");
        }

        // 0 is NOT a count digit when no count (it's a motion)
        assert!(!resolver.is_count_digit(&key('0')));
    }

    #[test]
    fn test_is_count_digit_subsequent() {
        let resolver = VimNormalResolver::new();

        // Accumulate a count first
        resolver.accumulate_count(&key('3'));
        assert_eq!(resolver.pending_count(), Some(3));

        // Now 0 IS a count digit
        assert!(resolver.is_count_digit(&key('0')));
    }

    #[test]
    fn test_count_digit_requires_no_modifiers() {
        let resolver = VimNormalResolver::new();

        // Ctrl+3 is NOT a count digit
        assert!(!resolver.is_count_digit(&key_with_mod('3', Modifiers::CTRL)));

        // Alt+5 is NOT a count digit
        assert!(!resolver.is_count_digit(&key_with_mod('5', Modifiers::ALT)));
    }

    #[test]
    fn test_accumulate_count() {
        let resolver = VimNormalResolver::new();

        resolver.accumulate_count(&key('2'));
        assert_eq!(resolver.pending_count(), Some(2));

        resolver.accumulate_count(&key('5'));
        assert_eq!(resolver.pending_count(), Some(25));

        resolver.accumulate_count(&key('0'));
        assert_eq!(resolver.pending_count(), Some(250));
    }

    #[test]
    fn test_register_prefix() {
        assert!(VimNormalResolver::is_register_prefix(&key('"')));
        assert!(!VimNormalResolver::is_register_prefix(&key('a')));
        assert!(!VimNormalResolver::is_register_prefix(&key_with_mod('"', Modifiers::SHIFT)));
    }

    #[test]
    fn test_waiting_for_register() {
        let resolver = VimNormalResolver::new();

        assert!(!resolver.is_waiting_for_register());

        // Set sentinel
        *resolver.pending_register.write().unwrap() = Some('"');
        assert!(resolver.is_waiting_for_register());

        // Set actual register
        *resolver.pending_register.write().unwrap() = Some('a');
        assert!(!resolver.is_waiting_for_register());
    }

    #[test]
    fn test_handle_register_char_valid() {
        let resolver = VimNormalResolver::new();
        *resolver.pending_register.write().unwrap() = Some('"');

        let result = resolver.handle_register_char(&key('a'));
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.pending_register(), Some('a'));
    }

    #[test]
    fn test_handle_register_char_invalid() {
        let resolver = VimNormalResolver::new();
        *resolver.pending_register.write().unwrap() = Some('"');

        // Space is not a valid register
        let result = resolver.handle_register_char(&key(' '));
        assert!(matches!(result, ResolveResult::NotHandled));
        assert!(resolver.pending_register().is_none());
    }

    #[test]
    fn test_take_count() {
        let resolver = VimNormalResolver::new();

        resolver.accumulate_count(&key('5'));
        assert_eq!(resolver.take_count(), Some(5));
        assert!(resolver.pending_count().is_none());
    }

    #[test]
    fn test_take_register() {
        let resolver = VimNormalResolver::new();

        *resolver.pending_register.write().unwrap() = Some('a');
        assert_eq!(resolver.take_register(), Some('a'));
        assert!(resolver.pending_register().is_none());
    }

    #[test]
    fn test_take_register_ignores_sentinel() {
        let resolver = VimNormalResolver::new();

        *resolver.pending_register.write().unwrap() = Some('"');
        assert!(resolver.take_register().is_none());
    }

    #[test]
    fn test_reset() {
        let mut resolver = VimNormalResolver::new();

        resolver.accumulate_count(&key('3'));
        *resolver.pending_register.write().unwrap() = Some('a');
        resolver.push_pending_key(key('d'));

        resolver.reset();

        assert!(resolver.pending_count().is_none());
        assert!(resolver.pending_register().is_none());
        assert!(resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_resolve_escape_resets() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();

        resolver.accumulate_count(&key('3'));
        *resolver.pending_register.write().unwrap() = Some('a');

        let result = resolver.resolve(&KeyEvent::new(KeyCode::Escape), &mut state);

        assert!(matches!(result, ResolveResult::NotHandled));
        assert!(resolver.pending_count().is_none());
        assert!(resolver.pending_register().is_none());
    }

    #[test]
    fn test_resolve_count_digit() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&key('3'), &mut state);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.pending_count(), Some(3));

        let result = resolver.resolve(&key('5'), &mut state);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.pending_count(), Some(35));
    }

    #[test]
    fn test_resolve_register_prefix() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&key('"'), &mut state);
        assert!(matches!(result, ResolveResult::Pending));
        assert!(resolver.is_waiting_for_register());

        let result = resolver.resolve(&key('a'), &mut state);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.pending_register(), Some('a'));
    }

    #[test]
    fn test_mode_id() {
        let resolver = VimNormalResolver::new();
        assert_eq!(resolver.mode_id().name(), "normal");
    }

    #[test]
    fn test_inherits_from() {
        let resolver = VimNormalResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    // ========================================================================
    // Keymap-aware resolution tests (Epic #353 - Mechanism/Policy separation)
    // ========================================================================
    //
    // Note: resolve_with_keymap currently delegates to resolve() for backward
    // compatibility. Full keymap-aware resolution is planned for Phase 3.
    //
    // The following tests verify the basic infrastructure works:

    use reovim_driver_input::KeymapQuery;

    /// Mock keymap for testing the infrastructure.
    struct MockKeymap;

    impl KeymapQuery for MockKeymap {
        fn query(
            &self,
            _mode: &ModeId,
            _keys: &KeySequence,
        ) -> reovim_driver_input::KeyLookupState {
            reovim_driver_input::KeyLookupState::NotFound
        }
    }

    #[test]
    fn test_resolve_with_keymap_delegates_to_resolve() {
        // Verify that resolve_with_keymap delegates to resolve() for backward compatibility.
        // The keymap is not actually queried in the current implementation.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap;
        let keys = KeySequence::new();
        let mode = EditorMode::NORMAL_ID;
        let input = ResolveInput::new(&keys, &mode, &keymap);

        // Test count accumulation works through resolve_with_keymap
        let result = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.pending_count(), Some(3));

        // Test that regular keys return NotHandled (delegated to runner's keymap lookup)
        resolver.clear_state();
        let result = resolver.resolve_with_keymap(&key('j'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }
}
