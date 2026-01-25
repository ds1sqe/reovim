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
        ArgValue, ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver,
        ModeState, ModeTransition, Modifiers, ResolveContext, ResolveInput, ResolveResult,
        TransitionContext,
    },
    reovim_kernel::api::v1::ModeId,
    reovim_module_editor as editor,
};

use crate::{
    ids::EXECUTE_FIND_CHAR,
    modes::VimMode,
    session_state::{PendingCharOp, VimSessionState},
};

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

impl VimNormalResolver {
    /// Create a new normal mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: VimMode::NORMAL_ID,
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

    // ========================================================================
    // Extension-based helpers (Epic #385 - use VimSessionState)
    // ========================================================================

    /// Check if a key is a count digit (extension-based version).
    fn is_count_digit_ext(&self, key: &KeyEvent, vim: &VimSessionState) -> bool {
        if key.modifiers != Modifiers::NONE {
            return false;
        }

        match key.code {
            KeyCode::Char('1'..='9') => true,
            KeyCode::Char('0') => vim.pending_count.is_some(),
            _ => false,
        }
    }

    /// Accumulate a count digit (extension-based version).
    fn accumulate_count_ext(&self, key: &KeyEvent, vim: &mut VimSessionState) {
        if let KeyCode::Char(c @ '0'..='9') = key.code {
            let digit = c.to_digit(10).expect("valid digit") as usize;
            vim.pending_count = Some(vim.pending_count.unwrap_or(0) * 10 + digit);
        }
    }

    /// Handle register character after `"` prefix (extension-based version).
    fn handle_register_char_ext(&self, key: &KeyEvent, vim: &mut VimSessionState) -> ResolveResult {
        if let KeyCode::Char(c) = key.code {
            // Valid register characters: a-z, A-Z, 0-9, and special registers
            if c.is_ascii_alphanumeric() || "+-*/.%#:".contains(c) {
                vim.pending_register = Some(c);
                return ResolveResult::Pending;
            }
        }

        // Invalid register character - cancel and pass key through
        vim.pending_register = None;
        ResolveResult::NotHandled
    }

    /// Build resolve context with count and register (extension-based version).
    fn build_context_ext(&self, keys: KeySequence, vim: &mut VimSessionState) -> ResolveContext {
        let mut ctx = ResolveContext::new().keys(keys);

        if let Some(count) = vim.pending_count.take() {
            ctx = ctx.count(count);
        }

        // Don't return the sentinel
        if let Some(reg) = vim.pending_register.take()
            && reg != '"'
        {
            ctx = ctx.register(reg);
        }

        ctx
    }

    /// Classify a command as a find-char operation, if applicable.
    ///
    /// Returns the corresponding `PendingCharOp` if the command is one of the
    /// find-char commands (f, F, t, T), otherwise returns `None`.
    ///
    /// This enables the resolver to intercept these commands and handle the
    /// character wait internally, rather than relying on the runner.
    fn classify_find_char_command(
        cmd: &reovim_kernel::api::v1::CommandId,
    ) -> Option<PendingCharOp> {
        // Check if this is a motions module command
        if cmd.module().as_str() != "motions" {
            return None;
        }

        // Match by command name within the motions module
        match cmd.name() {
            "find-char-forward" => Some(PendingCharOp::FindForward),
            "find-char-backward" => Some(PendingCharOp::FindBackward),
            "till-char-forward" => Some(PendingCharOp::TillForward),
            "till-char-backward" => Some(PendingCharOp::TillBackward),
            _ => None,
        }
    }

    /// Classify an operator entry command and return the target mode.
    ///
    /// Returns the ModeId for the dedicated operator mode (DELETE, YANK, CHANGE)
    /// if the command is an operator entry command, otherwise returns `None`.
    ///
    /// # Epic #415 - Dedicated Operator Modes
    ///
    /// Instead of routing all operators to a generic `operator-pending` mode,
    /// we now push to dedicated modes where:
    /// - The MODE itself carries operator semantics (no runtime lookup needed)
    /// - Each resolver is focused (~300 lines) and easier to debug
    /// - Statusline shows "DELETE" instead of "OP-PENDING"
    ///
    /// The enter-*-operator commands in the editor module are "shell" commands
    /// (they do nothing). The real work is done by:
    /// 1. This resolver: intercepts and pushes to the specific operator mode
    /// 2. Dedicated operator resolver (delete/yank/change): captures motion, returns range
    /// 3. Runner: executes the operator on the range
    ///
    /// # Compile-time Safety
    ///
    /// This function uses hard-typed comparisons against the editor module's
    /// `CommandId` constants. If those constants are renamed or removed, this
    /// code will fail to compile rather than silently break at runtime.
    fn classify_operator_mode(cmd: &reovim_kernel::api::v1::CommandId) -> Option<ModeId> {
        // Hard-typed check using editor module constants (compile-time safe)
        // Returns the dedicated operator mode (not generic operator-pending)
        if *cmd == editor::ids::ENTER_DELETE_OPERATOR {
            Some(VimMode::DELETE_ID)
        } else if *cmd == editor::ids::ENTER_YANK_OPERATOR {
            Some(VimMode::YANK_ID)
        } else if *cmd == editor::ids::ENTER_CHANGE_OPERATOR {
            Some(VimMode::CHANGE_ID)
        } else {
            None
        }
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
    /// This method queries the keymap and applies Vim policy to determine
    /// whether to execute immediately or wait for more keys.
    ///
    /// # Vim Policy
    ///
    /// | Lookup State | Vim Behavior |
    /// |--------------|--------------|
    /// | `ExactWithLonger` | `Pending` - wait for more keys (d might become dd) |
    /// | `ExactOnly` | `Execute` - run the command immediately |
    /// | `PrefixOnly` | `Pending` - wait for more keys |
    /// | `NotFound` | `NotHandled` - delegate to fallback handler |
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Handle escape - reset state and return NotHandled
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
        let keys = self.get_pending_keys();

        // Query keymap for facts about what bindings exist
        let lookup_state = input.keymap.query(input.mode, &keys);

        // Apply Vim policy (inline match - no separate trait needed)
        match lookup_state {
            KeyLookupState::ExactWithLonger { .. } => {
                // Wait for longer sequence (d might become dd)
                // Keep pending_keys for next lookup
                ResolveResult::Pending
            }
            KeyLookupState::ExactOnly(cmd) => {
                // Execute with context containing count and register
                let ctx = self.build_context(keys);
                self.clear_pending_keys();
                ResolveResult::Execute(cmd, ctx)
            }
            KeyLookupState::PrefixOnly => {
                // Wait for more keys (g waiting for gg, etc.)
                // Keep pending_keys for next lookup
                ResolveResult::Pending
            }
            KeyLookupState::NotFound => {
                // No binding found - clear keys and let runner handle
                self.clear_pending_keys();
                ResolveResult::NotHandled
            }
        }
    }

    /// Vim-style key resolution with keymap AND session extensions access.
    ///
    /// This is the new architecture (Epic #385) that uses `VimSessionState` from
    /// extensions instead of the runner's `AppState`. The resolver owns the vim
    /// policy, the runner is pure mechanism.
    ///
    /// # State Management
    ///
    /// | State | Source | Notes |
    /// |-------|--------|-------|
    /// | `pending_count` | `VimSessionState` | From extensions |
    /// | `pending_register` | `VimSessionState` | From extensions |
    /// | `pending_keys` | Internal `RwLock` | Multi-key sequence |
    ///
    /// Note: For backward compatibility, we still use internal `pending_keys`.
    /// Once all resolvers are migrated, these can move to `VimSessionState` too.
    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Get vim session state from extensions
        let vim = extensions.get_or_insert::<VimSessionState>();

        // Handle escape - reset state and return NotHandled
        if key.code == KeyCode::Escape {
            vim.clear_pending();
            self.clear_pending_keys();
            return ResolveResult::NotHandled;
        }

        // =====================================================================
        // Epic #385 - Handle pending find-char operation
        // =====================================================================
        // If pending_char is set, the next character completes the find motion.
        // We create an Execute result with EXECUTE_FIND_CHAR and the char in metadata.
        // Note: take() consumes pending_char regardless of whether the key is a char,
        // which correctly cancels the operation if a non-char key is pressed.
        if let Some(pending_op) = vim.pending_char.take()
            && let KeyCode::Char(c) = key.code
        {
            // Build context with find-char metadata
            let mut ctx = self.build_context_ext(KeySequence::new(), vim);

            // Set the target character
            ctx.metadata
                .insert("find_char".to_string(), ArgValue::Char(c));

            // Set direction based on operation type
            let direction = if pending_op.is_forward() {
                "forward"
            } else {
                "backward"
            };
            ctx.metadata
                .insert("find_direction".to_string(), ArgValue::String(direction.to_string()));

            // Set inclusive flag: f/F are inclusive (on char), t/T are not (before char)
            let inclusive = pending_op.is_find();
            ctx.metadata
                .insert("find_inclusive".to_string(), ArgValue::Bool(inclusive));

            self.clear_pending_keys();
            return ResolveResult::Execute(EXECUTE_FIND_CHAR, ctx);
        }

        // Check for register prefix waiting for character
        if vim.pending_register == Some('"') {
            return self.handle_register_char_ext(key, vim);
        }

        // Check for register prefix start
        if Self::is_register_prefix(key) {
            vim.pending_register = Some('"'); // Sentinel
            return ResolveResult::Pending;
        }

        // Check for count digit
        if self.is_count_digit_ext(key, vim) {
            self.accumulate_count_ext(key, vim);
            return ResolveResult::Pending;
        }

        // Add to pending keys for lookup
        self.push_pending_key(*key);
        let keys = self.get_pending_keys();

        // Query keymap for facts about what bindings exist
        let lookup_state = input.keymap.query(input.mode, &keys);
        tracing::warn!(?key.code, %input.mode, %keys, ?lookup_state, "Normal resolver query");

        // Apply Vim policy
        match lookup_state {
            KeyLookupState::ExactWithLonger { exact: cmd } => {
                // Epic #415: Operators push to dedicated modes (DELETE, YANK, CHANGE)
                // Even though dd exists, we don't wait - the dedicated mode handles dd
                // via the is_line_operator check when the second 'd' is pressed.
                //
                // Key insight: we DON'T take pending_count/pending_register here!
                // The dedicated resolver reads them on its first key press.
                // This simplifies the flow and eliminates vim.pending_operator.
                if let Some(target_mode) = Self::classify_operator_mode(&cmd) {
                    self.clear_pending_keys();
                    return ResolveResult::ModeTransition(ModeTransition::Push {
                        mode: target_mode,
                        context: TransitionContext::new(),
                    });
                }

                // Not an operator - wait for longer sequence
                ResolveResult::Pending
            }
            KeyLookupState::ExactOnly(cmd) => {
                // Epic #385 - Intercept find-char commands
                // Instead of executing commands that return WaitingForChar,
                // set pending_char in VimSessionState and return Pending.
                if let Some(pending_op) = Self::classify_find_char_command(&cmd) {
                    vim.pending_char = Some(pending_op);
                    self.clear_pending_keys();
                    return ResolveResult::Pending;
                }

                // Epic #415 - Push to dedicated operator modes (DELETE, YANK, CHANGE)
                // Instead of executing enter-*-operator commands (which do nothing),
                // push to the specific operator mode. The resolver reads pending_count
                // and pending_register from VimSessionState on its first key press.
                if let Some(target_mode) = Self::classify_operator_mode(&cmd) {
                    self.clear_pending_keys();
                    return ResolveResult::ModeTransition(ModeTransition::Push {
                        mode: target_mode,
                        context: TransitionContext::new(),
                    });
                }

                // Execute with context containing count and register
                let ctx = self.build_context_ext(keys, vim);
                self.clear_pending_keys();
                ResolveResult::Execute(cmd, ctx)
            }
            KeyLookupState::PrefixOnly => {
                // Wait for more keys
                ResolveResult::Pending
            }
            KeyLookupState::NotFound => {
                // No binding found - clear keys and let runner handle
                self.clear_pending_keys();
                ResolveResult::NotHandled
            }
        }
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
        ModeState::new(VimMode::NORMAL_ID)
    }

    #[test]
    fn test_new_resolver() {
        let resolver = VimNormalResolver::new();
        assert_eq!(resolver.mode_id(), &VimMode::NORMAL_ID);
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
    // These tests verify that resolve_with_keymap correctly applies Vim policy
    // based on the KeyLookupState returned by the keymap.

    use reovim_driver_input::KeymapQuery;

    use reovim_kernel::api::v1::{CommandId, ModuleId};

    /// Test module ID for creating command IDs.
    const TEST_MODULE: ModuleId = ModuleId::new("test");

    /// Mock keymap that returns a configurable `KeyLookupState`.
    struct MockKeymap {
        response: KeyLookupState,
    }

    impl MockKeymap {
        fn exact_only(cmd: &'static str) -> Self {
            Self {
                response: KeyLookupState::ExactOnly(CommandId::new(TEST_MODULE, cmd)),
            }
        }

        fn exact_with_longer(cmd: &'static str) -> Self {
            Self {
                response: KeyLookupState::ExactWithLonger {
                    exact: CommandId::new(TEST_MODULE, cmd),
                },
            }
        }

        fn prefix_only() -> Self {
            Self {
                response: KeyLookupState::PrefixOnly,
            }
        }

        fn not_found() -> Self {
            Self {
                response: KeyLookupState::NotFound,
            }
        }
    }

    impl KeymapQuery for MockKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            self.response.clone()
        }
    }

    fn resolve_input(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
        // Keys are managed by resolver, so we pass empty sequence here
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::NORMAL_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    // ------------------------------------------------------------------------
    // Vim Policy Tests: KeyLookupState -> ResolveResult mapping
    // ------------------------------------------------------------------------

    #[test]
    fn test_exact_with_longer_returns_pending() {
        // When keymap reports ExactWithLonger (d exists and dd exists),
        // Vim policy says wait for more keys.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_with_longer("delete");
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('d'), &mut state, &input);

        assert!(matches!(result, ResolveResult::Pending));
        // pending_keys should contain the key
        assert!(!resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_exact_only_returns_execute() {
        // When keymap reports ExactOnly (x exists, no xx),
        // Vim policy says execute immediately.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("delete-char");
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('x'), &mut state, &input);

        match result {
            ResolveResult::Execute(cmd, _ctx) => {
                assert_eq!(cmd.name(), "delete-char");
            }
            _ => panic!("Expected Execute, got {result:?}"),
        }
        // pending_keys should be cleared after execute
        assert!(resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_prefix_only_returns_pending() {
        // When keymap reports PrefixOnly (g is prefix for gg, but no binding for just g),
        // Vim policy says wait for more keys.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::prefix_only();
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('g'), &mut state, &input);

        assert!(matches!(result, ResolveResult::Pending));
        // pending_keys should contain the key
        assert!(!resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_not_found_clears_and_returns_not_handled() {
        // When keymap reports NotFound (no binding for z),
        // Vim policy clears pending keys and returns NotHandled.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);

        assert!(matches!(result, ResolveResult::NotHandled));
        // pending_keys should be cleared
        assert!(resolver.get_pending_keys().is_empty());
    }

    // ------------------------------------------------------------------------
    // Context building tests: count and register flow to Execute
    // ------------------------------------------------------------------------

    #[test]
    fn test_count_flows_to_context() {
        // Count accumulated before command should appear in Execute context.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("cursor-down");
        let input = resolve_input(&keymap);

        // Accumulate count first
        let _ = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
        assert_eq!(resolver.pending_count(), Some(3));

        // Now press 'j' which executes
        let result = resolver.resolve_with_keymap(&key('j'), &mut state, &input);

        match result {
            ResolveResult::Execute(_cmd, ctx) => {
                assert_eq!(ctx.count, Some(3));
            }
            _ => panic!("Expected Execute, got {result:?}"),
        }
        // Count should be cleared after execute
        assert!(resolver.pending_count().is_none());
    }

    #[test]
    fn test_register_flows_to_context() {
        // Register selected before command should appear in Execute context.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("paste");
        let input = resolve_input(&keymap);

        // Select register first
        let _ = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
        assert!(resolver.is_waiting_for_register());
        let _ = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
        assert_eq!(resolver.pending_register(), Some('a'));

        // Now press 'p' which executes
        let result = resolver.resolve_with_keymap(&key('p'), &mut state, &input);

        match result {
            ResolveResult::Execute(_cmd, ctx) => {
                assert_eq!(ctx.register, Some('a'));
            }
            _ => panic!("Expected Execute, got {result:?}"),
        }
        // Register should be cleared after execute
        assert!(resolver.pending_register().is_none());
    }

    #[test]
    fn test_count_and_register_together() {
        // Both count and register should flow to context.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("delete-word");
        let input = resolve_input(&keymap);

        // "a3 - select register 'a', then count 3
        let _ = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
        let _ = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
        let _ = resolver.resolve_with_keymap(&key('3'), &mut state, &input);

        assert_eq!(resolver.pending_register(), Some('a'));
        assert_eq!(resolver.pending_count(), Some(3));

        // Press 'w' which executes
        let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);

        match result {
            ResolveResult::Execute(_cmd, ctx) => {
                assert_eq!(ctx.count, Some(3));
                assert_eq!(ctx.register, Some('a'));
            }
            _ => panic!("Expected Execute, got {result:?}"),
        }
    }

    // ------------------------------------------------------------------------
    // State management tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_escape_clears_all_state() {
        // Escape should clear count, register, and pending keys.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        // Accumulate some state
        let _ = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
        let _ = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
        let _ = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
        resolver.push_pending_key(key('d'));

        assert!(resolver.pending_count().is_some());
        assert!(resolver.pending_register().is_some());
        assert!(!resolver.get_pending_keys().is_empty());

        // Press escape
        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

        assert!(matches!(result, ResolveResult::NotHandled));
        assert!(resolver.pending_count().is_none());
        assert!(resolver.pending_register().is_none());
        assert!(resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_pending_keys_cleared_after_execute() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("delete-char");
        let input = resolve_input(&keymap);

        let _ = resolver.resolve_with_keymap(&key('x'), &mut state, &input);

        assert!(resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_pending_keys_cleared_after_not_found() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        let _ = resolver.resolve_with_keymap(&key('z'), &mut state, &input);

        assert!(resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_register_sentinel_not_in_context() {
        // The sentinel value '"' should not leak into the execute context.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("some-cmd");
        let input = resolve_input(&keymap);

        // Start register prefix but don't complete it
        let _ = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
        assert!(resolver.is_waiting_for_register());

        // Directly test take_register filters sentinel
        let reg = resolver.take_register();
        assert!(reg.is_none(), "Sentinel should not be returned");
    }

    // ========================================================================
    // Operator interception tests (Epic #415 - Operator-pending mode)
    // ========================================================================
    //
    // These tests verify that operator commands (d, y, c) are intercepted and
    // return ModeTransition::Push to operator-pending mode.

    use reovim_driver_session::ExtensionMap;

    /// Module ID for editor commands.
    const EDITOR_MODULE: ModuleId = ModuleId::new("editor");

    /// Mock keymap that returns operator commands.
    impl MockKeymap {
        fn enter_delete_operator() -> Self {
            Self {
                response: KeyLookupState::ExactOnly(CommandId::new(
                    EDITOR_MODULE,
                    "enter-delete-operator",
                )),
            }
        }

        fn enter_yank_operator() -> Self {
            Self {
                response: KeyLookupState::ExactOnly(CommandId::new(
                    EDITOR_MODULE,
                    "enter-yank-operator",
                )),
            }
        }

        fn enter_change_operator() -> Self {
            Self {
                response: KeyLookupState::ExactOnly(CommandId::new(
                    EDITOR_MODULE,
                    "enter-change-operator",
                )),
            }
        }
    }

    fn resolve_with_ext(
        resolver: &VimNormalResolver,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        resolver.resolve_with_extensions(key, state, input, extensions)
    }

    #[test]
    fn test_d_returns_mode_transition_push_to_delete_mode() {
        // Epic #415: Pressing 'd' should push to dedicated DELETE mode.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::enter_delete_operator();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);

        match result {
            ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
                assert_eq!(mode, VimMode::DELETE_ID, "Should push to DELETE mode");
            }
            _ => panic!("Expected ModeTransition::Push, got {result:?}"),
        }
    }

    #[test]
    fn test_y_returns_mode_transition_push_to_yank_mode() {
        // Epic #415: Pressing 'y' should push to dedicated YANK mode.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::enter_yank_operator();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('y'), &mut state, &input, &mut extensions);

        match result {
            ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
                assert_eq!(mode, VimMode::YANK_ID, "Should push to YANK mode");
            }
            _ => panic!("Expected ModeTransition::Push, got {result:?}"),
        }
    }

    #[test]
    fn test_c_returns_mode_transition_push_to_change_mode() {
        // Epic #415: Pressing 'c' should push to dedicated CHANGE mode.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::enter_change_operator();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('c'), &mut state, &input, &mut extensions);

        match result {
            ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
                assert_eq!(mode, VimMode::CHANGE_ID, "Should push to CHANGE mode");
            }
            _ => panic!("Expected ModeTransition::Push, got {result:?}"),
        }
    }

    #[test]
    fn test_count_preserved_for_operator_resolver() {
        // Epic #415: Normal mode does NOT consume pending_count.
        // The dedicated operator resolver reads it on first key.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::enter_delete_operator();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Initialize VimSessionState with a count
        {
            let vim = extensions.get_or_insert::<VimSessionState>();
            vim.pending_count = Some(3);
        }

        let _ = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);

        // Count should still be in VimSessionState (not consumed by normal resolver)
        let vim = extensions
            .get::<VimSessionState>()
            .expect("VimSessionState should exist");
        assert_eq!(
            vim.pending_count,
            Some(3),
            "pending_count should be preserved for operator resolver"
        );
    }

    #[test]
    fn test_register_preserved_for_operator_resolver() {
        // Epic #415: Normal mode does NOT consume pending_register.
        // The dedicated operator resolver reads it on first key.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::enter_delete_operator();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Initialize VimSessionState with a register
        {
            let vim = extensions.get_or_insert::<VimSessionState>();
            vim.pending_register = Some('a');
        }

        let _ = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);

        // Register should still be in VimSessionState (not consumed by normal resolver)
        let vim = extensions
            .get::<VimSessionState>()
            .expect("VimSessionState should exist");
        assert_eq!(
            vim.pending_register,
            Some('a'),
            "pending_register should be preserved for operator resolver"
        );
    }

    #[test]
    fn test_count_and_register_preserved_together() {
        // Epic #415: Both count and register should remain for the dedicated resolver.
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::enter_delete_operator();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Initialize VimSessionState with both count and register
        {
            let vim = extensions.get_or_insert::<VimSessionState>();
            vim.pending_count = Some(3);
            vim.pending_register = Some('a');
        }

        let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);

        // Verify we push to DELETE mode
        if let ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) = result {
            assert_eq!(mode, VimMode::DELETE_ID, "Should push to DELETE mode");
        } else {
            panic!("Expected ModeTransition::Push");
        }

        // Both should still be in VimSessionState
        let vim = extensions
            .get::<VimSessionState>()
            .expect("VimSessionState should exist");
        assert_eq!(vim.pending_count, Some(3), "pending_count should be preserved");
        assert_eq!(vim.pending_register, Some('a'), "pending_register should be preserved");
    }
}
