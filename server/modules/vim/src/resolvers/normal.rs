//! Vim normal mode key resolver.
//!
//! Handles normal mode key interpretation including:
//! - Count prefix accumulation (1-9, then 0)
//! - Register prefix (") handling
//! - Command key lookup via keymap registry
//! - Operator entry transitions
//! - Macro recording (q) and playback (@) - Epic #465 Phase 8D

#![allow(clippy::unused_self)] // Methods may need self for future extensibility

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
    macros::notation_to_keys,
    modes::VimMode,
    session_state::{PendingCharOp, VimSessionState},
};

/// Pending macro operation.
///
/// Tracks what macro-related action we're waiting for after pressing `q` or `@`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingMacroOp {
    /// Waiting for register character after `q` (to start recording).
    StartRecording,
    /// Waiting for register character after `@` (to play macro).
    PlayMacro,
}

/// Vim normal mode key resolver.
///
/// In normal mode:
/// - Digits 1-9 (and 0 after other digits) accumulate as count prefix
/// - `"` followed by a character selects a register
/// - Other keys are looked up in the keymap
/// - Operators (d, y, c) trigger transition to operator-pending mode
/// - `q` starts/stops macro recording (Epic #465 Phase 8D)
/// - `@` plays macro from register (Epic #465 Phase 8D)
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

    /// Pending macro operation (Epic #465 Phase 8D).
    ///
    /// - `None`: No pending macro operation
    /// - `Some(StartRecording)`: Waiting for register after `q`
    /// - `Some(PlayMacro)`: Waiting for register after `@`
    pending_macro: RwLock<Option<PendingMacroOp>>,
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
            pending_macro: RwLock::new(None),
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
        *self.pending_macro.write().expect("lock poisoned") = None;
    }

    // ========================================================================
    // Macro Recording/Playback Helpers (Epic #465 Phase 8D)
    // ========================================================================

    /// Check if we're waiting for a macro operation register.
    fn pending_macro_op(&self) -> Option<PendingMacroOp> {
        *self.pending_macro.read().expect("lock poisoned")
    }

    /// Set pending macro operation.
    fn set_pending_macro(&self, op: PendingMacroOp) {
        *self.pending_macro.write().expect("lock poisoned") = Some(op);
    }

    /// Clear pending macro operation.
    fn clear_pending_macro(&self) {
        *self.pending_macro.write().expect("lock poisoned") = None;
    }

    /// Check if a key is the macro record key (`q` without modifiers).
    fn is_macro_record_key(key: &KeyEvent) -> bool {
        key.modifiers == Modifiers::NONE && key.code == KeyCode::Char('q')
    }

    /// Check if a key is the macro play key (`@` without modifiers).
    fn is_macro_play_key(key: &KeyEvent) -> bool {
        key.modifiers == Modifiers::NONE && key.code == KeyCode::Char('@')
    }

    /// Handle `q` key for macro recording.
    ///
    /// - If currently recording: stop recording, store to register
    /// - If not recording: set pending macro state to wait for register
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handle_macro_record_key(
        &self,
        vim: &mut VimSessionState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        if vim.is_recording() {
            // Stop recording - store keys to register
            if let Some((register, keys)) = vim.stop_recording() {
                // Convert keys to notation string and store in register
                let notation = crate::macros::keys_to_notation(&keys);
                tracing::debug!(
                    register = %register,
                    key_count = keys.len(),
                    notation = %notation,
                    "Stopped macro recording"
                );

                // Store in register via input's register access
                // Note: We store as text - macros are just key notation strings
                if let Some(registers) = input.registers {
                    use reovim_kernel::api::v1::RegisterContent;
                    registers
                        .write()
                        .set_named(register, RegisterContent::characterwise(&notation));
                }
            }
            ResolveResult::Completed
        } else {
            // Start recording - wait for register character
            self.set_pending_macro(PendingMacroOp::StartRecording);
            ResolveResult::Pending
        }
    }

    /// Handle register character after `q` (start recording).
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handle_macro_record_register(
        &self,
        key: &KeyEvent,
        vim: &mut VimSessionState,
    ) -> ResolveResult {
        self.clear_pending_macro();

        if let KeyCode::Char(c) = key.code
            && c.is_ascii_lowercase()
            && vim.start_recording(c)
        {
            tracing::debug!(register = %c, "Started macro recording");
            return ResolveResult::Completed;
        }

        // Invalid register - cancel
        tracing::debug!(?key.code, "Invalid macro register");
        ResolveResult::NotHandled
    }

    /// Handle `@` key for macro playback.
    fn handle_macro_play_key(&self, vim: &VimSessionState) -> ResolveResult {
        // Check if we can enter playback (depth limit)
        if vim.is_macro_depth_exceeded() {
            tracing::warn!(depth = vim.macro_playback_depth, "Macro playback depth exceeded");
            return ResolveResult::NotHandled;
        }

        // Wait for register character
        self.set_pending_macro(PendingMacroOp::PlayMacro);
        ResolveResult::Pending
    }

    /// Handle register character after `@` (play macro).
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handle_macro_play_register(
        &self,
        key: &KeyEvent,
        vim: &mut VimSessionState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        self.clear_pending_macro();

        let register = if key.code == KeyCode::Char('@') {
            // @@ - repeat last macro
            vim.last_macro_register
        } else if let KeyCode::Char(c) = key.code {
            if c.is_ascii_lowercase() {
                Some(c)
            } else {
                None
            }
        } else {
            None
        };

        let Some(register) = register else {
            tracing::debug!(?key.code, "Invalid macro playback register");
            return ResolveResult::NotHandled;
        };

        // Get macro content from register
        let Some(registers) = input.registers else {
            tracing::warn!("No register access for macro playback");
            return ResolveResult::NotHandled;
        };

        let content = {
            let guard = registers.read();
            guard.get_by_name(Some(register)).cloned()
        };

        let Some(content) = content else {
            tracing::debug!(register = %register, "Macro register is empty");
            return ResolveResult::NotHandled;
        };

        // Parse the notation string to keys
        let Some(keys) = notation_to_keys(&content.text) else {
            tracing::warn!(
                register = %register,
                content = %content.text,
                "Failed to parse macro content"
            );
            return ResolveResult::NotHandled;
        };

        if keys.is_empty() {
            return ResolveResult::Completed;
        }

        // Update last macro register for @@ support
        vim.last_macro_register = Some(register);

        // Get count for playback
        let count = vim.pending_count.take().unwrap_or(1);

        // Enter playback
        if !vim.enter_macro_playback() {
            return ResolveResult::NotHandled;
        }

        // Build full key sequence (count repetitions)
        let mut all_keys = Vec::with_capacity(keys.len() * count);
        for _ in 0..count {
            all_keys.extend(keys.iter().copied());
        }

        tracing::debug!(
            register = %register,
            count,
            key_count = all_keys.len(),
            "Playing macro"
        );

        // Return keys to be injected
        // The runner will inject these and call exit_macro_playback when done
        ResolveResult::InjectKeys {
            keys: all_keys,
            exit_macro_playback: true,
        }
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn accumulate_count_ext(&self, key: &KeyEvent, vim: &mut VimSessionState) {
        if let KeyCode::Char(c @ '0'..='9') = key.code {
            let digit = c.to_digit(10).expect("valid digit") as usize;
            vim.pending_count = Some(vim.pending_count.unwrap_or(0) * 10 + digit);
        }
    }

    /// Handle register character after `"` prefix (extension-based version).
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    /// Returns the `ModeId` for the dedicated operator mode (DELETE, YANK, CHANGE)
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
    #[allow(clippy::too_many_lines)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Get vim session state from client extensions (per-client state)
        let vim = client_extensions.get_or_insert::<VimSessionState>();

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
            let mut ctx = self.build_context_ext(KeySequence::new(), vim);
            self.clear_pending_keys();

            if pending_op.is_motion() {
                // Find-char motion (f/F/t/T)
                ctx.metadata
                    .insert("find_char".to_string(), ArgValue::Char(c));
                let direction = if pending_op.is_forward() {
                    "forward"
                } else {
                    "backward"
                };
                ctx.metadata
                    .insert("find_direction".to_string(), ArgValue::String(direction.to_string()));
                let inclusive = pending_op.is_find();
                ctx.metadata
                    .insert("find_inclusive".to_string(), ArgValue::Bool(inclusive));
                return ResolveResult::Execute(EXECUTE_FIND_CHAR, ctx);
            }

            // Replace operation (r) — dispatch to editor::REPLACE_CHAR
            ctx.metadata
                .insert("replace_char".to_string(), ArgValue::Char(c));
            return ResolveResult::Execute(editor::ids::REPLACE_CHAR, ctx);
        }

        // =====================================================================
        // Epic #465 Phase 8D - Macro Recording/Playback
        // =====================================================================

        // Handle pending macro operations first (waiting for register after q or @)
        if let Some(pending_op) = self.pending_macro_op() {
            match pending_op {
                PendingMacroOp::StartRecording => {
                    // Record key if we're recording (except q that stops)
                    // Note: We're about to potentially start recording, so don't record this key
                    return self.handle_macro_record_register(key, vim);
                }
                PendingMacroOp::PlayMacro => {
                    return self.handle_macro_play_register(key, vim, input);
                }
            }
        }

        // Check for macro record key (q)
        if Self::is_macro_record_key(key) {
            return self.handle_macro_record_key(vim, input);
        }

        // Check for macro play key (@)
        if Self::is_macro_play_key(key) {
            return self.handle_macro_play_key(vim);
        }

        // Record key if we're recording (before normal processing)
        // The key will be recorded regardless of what it does
        if vim.is_recording() {
            vim.record_key(*key);
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

                // #554 - Intercept replace-char-start (r)
                // Like find-char, this sets pending_char and waits for the next char.
                if cmd == editor::ids::REPLACE_CHAR_START {
                    vim.pending_char = Some(PendingCharOp::Replace);
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
        *self.pending_macro.write().expect("lock poisoned") = None;
    }

    fn pending_keys(&self) -> KeySequence {
        self.get_pending_keys()
    }
}

#[cfg(test)]
mod tests {
    use reovim_driver_input::{KeyLookupState, KeySequence, KeymapQuery};

    use reovim_kernel::api::v1::{CommandId, ModuleId};

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

    /// Mock keymap that always returns `NotFound` (no bindings).
    struct NotFoundKeymap;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for NotFoundKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::NotFound
        }
    }

    fn resolve_input_notfound() -> ResolveInput<'static> {
        static KEYMAP: NotFoundKeymap = NotFoundKeymap;
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::NORMAL_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, &KEYMAP)
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
        let input = resolve_input_notfound();

        resolver.accumulate_count(&key('3'));
        *resolver.pending_register.write().unwrap() = Some('a');

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

        assert!(matches!(result, ResolveResult::NotHandled));
        assert!(resolver.pending_count().is_none());
        assert!(resolver.pending_register().is_none());
    }

    #[test]
    fn test_resolve_count_digit() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let input = resolve_input_notfound();

        let result = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.pending_count(), Some(3));

        let result = resolver.resolve_with_keymap(&key('5'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.pending_count(), Some(35));
    }

    #[test]
    fn test_resolve_register_prefix() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let input = resolve_input_notfound();

        let result = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert!(resolver.is_waiting_for_register());

        let result = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
        let mut shared_ext = ExtensionMap::new();
        resolver.resolve_with_extensions(key, state, input, &mut shared_ext, extensions)
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    // ========================================================================
    // Extension-based resolution tests (resolve_with_extensions)
    // ========================================================================

    #[test]
    fn test_ext_escape_clears_vim_state() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Set up VimSessionState with pending values
        {
            let vim = extensions.get_or_insert::<VimSessionState>();
            vim.pending_count = Some(5);
            vim.pending_register = Some('b');
        }

        let result = resolve_with_ext(
            &resolver,
            &KeyEvent::new(KeyCode::Escape),
            &mut state,
            &input,
            &mut extensions,
        );

        assert!(matches!(result, ResolveResult::NotHandled));

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert!(vim.pending_count.is_none());
        assert!(vim.pending_register.is_none());
    }

    #[test]
    fn test_ext_count_digit_accumulates() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('3'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::Pending));

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.pending_count, Some(3));

        let result = resolve_with_ext(&resolver, &key('5'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::Pending));

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.pending_count, Some(35));
    }

    #[test]
    fn test_ext_register_prefix() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Press " to start register prefix
        let result = resolve_with_ext(&resolver, &key('"'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::Pending));

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.pending_register, Some('"'));

        // Press 'a' to select register
        let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::Pending));

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.pending_register, Some('a'));
    }

    #[test]
    fn test_ext_register_prefix_invalid_char() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Set sentinel
        {
            let vim = extensions.get_or_insert::<VimSessionState>();
            vim.pending_register = Some('"');
        }

        // Space is not a valid register
        let result = resolve_with_ext(&resolver, &key(' '), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::NotHandled));

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert!(vim.pending_register.is_none());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_count_flows_to_execute() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap_nf = MockKeymap::not_found();
        let input_nf = resolve_input(&keymap_nf);
        let mut extensions = ExtensionMap::new();

        // Accumulate count
        let _ = resolve_with_ext(&resolver, &key('4'), &mut state, &input_nf, &mut extensions);

        // Execute command
        let keymap_exec = MockKeymap::exact_only("cursor-down");
        let input_exec = resolve_input(&keymap_exec);

        let result =
            resolve_with_ext(&resolver, &key('j'), &mut state, &input_exec, &mut extensions);

        match result {
            ResolveResult::Execute(cmd, ctx) => {
                assert_eq!(cmd.name(), "cursor-down");
                assert_eq!(ctx.count, Some(4));
            }
            _ => panic!("Expected Execute, got {result:?}"),
        }
    }

    #[test]
    fn test_ext_not_found_returns_not_handled() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('z'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_ext_prefix_only_returns_pending() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::prefix_only();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('g'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::Pending));
    }

    #[test]
    fn test_ext_non_operator_exact_with_longer_returns_pending() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        // Create a non-operator ExactWithLonger (not enter-delete-operator)
        let keymap = MockKeymap::exact_with_longer("some-non-operator");
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('g'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::Pending));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_zero_not_count_without_pending() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("line-start");
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // 0 without pending count should be a command (line-start), not a count
        let result = resolve_with_ext(&resolver, &key('0'), &mut state, &input, &mut extensions);
        match result {
            ResolveResult::Execute(cmd, _) => {
                assert_eq!(cmd.name(), "line-start");
            }
            _ => panic!("Expected Execute, got {result:?}"),
        }
    }

    #[test]
    fn test_ext_zero_is_count_with_pending() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // First digit to start count
        let _ = resolve_with_ext(&resolver, &key('1'), &mut state, &input, &mut extensions);

        // Now 0 should be a count digit
        let result = resolve_with_ext(&resolver, &key('0'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::Pending));

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.pending_count, Some(10));
    }

    // ========================================================================
    // Classify helpers tests
    // ========================================================================

    #[test]
    fn test_classify_find_char_command_forward() {
        let cmd = CommandId::new(ModuleId::new("motions"), "find-char-forward");
        let result = VimNormalResolver::classify_find_char_command(&cmd);
        assert!(result.is_some());
    }

    #[test]
    fn test_classify_find_char_command_backward() {
        let cmd = CommandId::new(ModuleId::new("motions"), "find-char-backward");
        let result = VimNormalResolver::classify_find_char_command(&cmd);
        assert!(result.is_some());
    }

    #[test]
    fn test_classify_find_char_command_till_forward() {
        let cmd = CommandId::new(ModuleId::new("motions"), "till-char-forward");
        let result = VimNormalResolver::classify_find_char_command(&cmd);
        assert!(result.is_some());
    }

    #[test]
    fn test_classify_find_char_command_till_backward() {
        let cmd = CommandId::new(ModuleId::new("motions"), "till-char-backward");
        let result = VimNormalResolver::classify_find_char_command(&cmd);
        assert!(result.is_some());
    }

    #[test]
    fn test_classify_find_char_command_non_motion_module() {
        let cmd = CommandId::new(ModuleId::new("editor"), "find-char-forward");
        let result = VimNormalResolver::classify_find_char_command(&cmd);
        assert!(result.is_none());
    }

    #[test]
    fn test_classify_find_char_command_unknown() {
        let cmd = CommandId::new(ModuleId::new("motions"), "cursor-down");
        let result = VimNormalResolver::classify_find_char_command(&cmd);
        assert!(result.is_none());
    }

    #[test]
    fn test_classify_operator_mode_delete() {
        let result = VimNormalResolver::classify_operator_mode(&editor::ids::ENTER_DELETE_OPERATOR);
        assert_eq!(result, Some(VimMode::DELETE_ID));
    }

    #[test]
    fn test_classify_operator_mode_yank() {
        let result = VimNormalResolver::classify_operator_mode(&editor::ids::ENTER_YANK_OPERATOR);
        assert_eq!(result, Some(VimMode::YANK_ID));
    }

    #[test]
    fn test_classify_operator_mode_change() {
        let result = VimNormalResolver::classify_operator_mode(&editor::ids::ENTER_CHANGE_OPERATOR);
        assert_eq!(result, Some(VimMode::CHANGE_ID));
    }

    #[test]
    fn test_classify_operator_mode_non_operator() {
        let cmd = CommandId::new(ModuleId::new("editor"), "cursor-down");
        let result = VimNormalResolver::classify_operator_mode(&cmd);
        assert!(result.is_none());
    }

    // ========================================================================
    // Macro-related helper tests
    // ========================================================================

    #[test]
    fn test_is_macro_record_key() {
        assert!(VimNormalResolver::is_macro_record_key(&key('q')));
        assert!(!VimNormalResolver::is_macro_record_key(&key('w')));
        assert!(!VimNormalResolver::is_macro_record_key(&key_with_mod('q', Modifiers::CTRL)));
    }

    #[test]
    fn test_is_macro_play_key() {
        assert!(VimNormalResolver::is_macro_play_key(&key('@')));
        assert!(!VimNormalResolver::is_macro_play_key(&key('a')));
        assert!(!VimNormalResolver::is_macro_play_key(&key_with_mod('@', Modifiers::CTRL)));
    }

    #[test]
    fn test_pending_macro_starts_none() {
        let resolver = VimNormalResolver::new();
        assert!(resolver.pending_macro_op().is_none());
    }

    #[test]
    fn test_set_and_clear_pending_macro() {
        let resolver = VimNormalResolver::new();
        resolver.set_pending_macro(PendingMacroOp::StartRecording);
        assert_eq!(resolver.pending_macro_op(), Some(PendingMacroOp::StartRecording));

        resolver.clear_pending_macro();
        assert!(resolver.pending_macro_op().is_none());
    }

    #[test]
    fn test_clear_state_clears_macro() {
        let resolver = VimNormalResolver::new();
        resolver.set_pending_macro(PendingMacroOp::PlayMacro);
        resolver.clear_state();
        assert!(resolver.pending_macro_op().is_none());
    }

    #[test]
    fn test_register_special_chars_valid() {
        let resolver = VimNormalResolver::new();
        // Set sentinel
        *resolver.pending_register.write().unwrap() = Some('"');

        // Special register characters should be accepted
        for c in ['+', '-', '*', '/', '.', '%', '#', ':'] {
            *resolver.pending_register.write().unwrap() = Some('"');
            let result = resolver.handle_register_char(&key(c));
            assert!(
                matches!(result, ResolveResult::Pending),
                "register char '{c}' should be valid"
            );
        }
    }

    #[test]
    fn test_build_context_no_count_no_register() {
        let resolver = VimNormalResolver::new();
        let ctx = resolver.build_context(KeySequence::new());
        assert!(ctx.count.is_none());
        assert!(ctx.register.is_none());
    }

    #[test]
    fn test_build_context_with_count() {
        let resolver = VimNormalResolver::new();
        resolver.accumulate_count(&key('7'));
        let ctx = resolver.build_context(KeySequence::new());
        assert_eq!(ctx.count, Some(7));
    }

    #[test]
    fn test_build_context_with_register() {
        let resolver = VimNormalResolver::new();
        *resolver.pending_register.write().unwrap() = Some('z');
        let ctx = resolver.build_context(KeySequence::new());
        assert_eq!(ctx.register, Some('z'));
    }

    // ========================================================================
    // Additional coverage tests
    // ========================================================================

    #[test]
    fn test_default_resolver() {
        let resolver = VimNormalResolver::default();
        assert_eq!(resolver.mode_id(), &VimMode::NORMAL_ID);
        assert!(resolver.pending_count().is_none());
    }

    #[test]
    fn test_handle_register_char_non_char_key() {
        // handle_register_char with non-Char key (e.g., Enter) should cancel
        let resolver = VimNormalResolver::new();
        *resolver.pending_register.write().unwrap() = Some('"');

        let result = resolver.handle_register_char(&KeyEvent::new(KeyCode::Enter));
        assert!(matches!(result, ResolveResult::NotHandled));
        assert!(resolver.pending_register().is_none());
    }

    #[test]
    fn test_build_context_ext_no_count_no_register() {
        let resolver = VimNormalResolver::new();
        let mut vim = VimSessionState::default();
        let ctx = resolver.build_context_ext(KeySequence::new(), &mut vim);
        assert!(ctx.count.is_none());
        assert!(ctx.register.is_none());
    }

    #[test]
    fn test_build_context_ext_with_count() {
        let resolver = VimNormalResolver::new();
        let mut vim = VimSessionState {
            pending_count: Some(4),
            ..VimSessionState::default()
        };
        let ctx = resolver.build_context_ext(KeySequence::new(), &mut vim);
        assert_eq!(ctx.count, Some(4));
        assert!(vim.pending_count.is_none()); // consumed
    }

    #[test]
    fn test_build_context_ext_with_register() {
        let resolver = VimNormalResolver::new();
        let mut vim = VimSessionState {
            pending_register: Some('z'),
            ..VimSessionState::default()
        };
        let ctx = resolver.build_context_ext(KeySequence::new(), &mut vim);
        assert_eq!(ctx.register, Some('z'));
        assert!(vim.pending_register.is_none()); // consumed
    }

    #[test]
    fn test_build_context_ext_sentinel_register_filtered() {
        // The sentinel '"' should NOT appear in context
        let resolver = VimNormalResolver::new();
        let mut vim = VimSessionState {
            pending_register: Some('"'),
            ..VimSessionState::default()
        };
        let ctx = resolver.build_context_ext(KeySequence::new(), &mut vim);
        assert!(ctx.register.is_none(), "sentinel should be filtered");
    }

    #[test]
    fn test_ext_find_char_forward() {
        // Test find-char interception in resolve_with_extensions
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let motions_module = ModuleId::new("motions");
        let keymap = MockKeymap {
            response: KeyLookupState::ExactOnly(CommandId::new(
                motions_module,
                "find-char-forward",
            )),
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Press 'f' - should intercept and set pending_char, return Pending
        let result = resolve_with_ext(&resolver, &key('f'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::Pending),
            "find-char should return Pending, got {result:?}"
        );

        // Check that pending_char was set
        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.pending_char, Some(PendingCharOp::FindForward));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_find_char_completes_with_character() {
        // After f is pressed and pending_char is set, the next character should complete
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let motions_module = ModuleId::new("motions");
        let keymap = MockKeymap {
            response: KeyLookupState::ExactOnly(CommandId::new(
                motions_module,
                "find-char-forward",
            )),
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Press 'f' to set pending_char
        let _ = resolve_with_ext(&resolver, &key('f'), &mut state, &input, &mut extensions);

        // Now press 'a' - should execute find-char with char='a'
        let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);

        if let ResolveResult::Execute(cmd, ctx) = result {
            assert_eq!(cmd.name(), "execute-find-char");
            assert_eq!(ctx.metadata.get("find_char"), Some(&ArgValue::Char('a')));
            assert_eq!(
                ctx.metadata.get("find_direction"),
                Some(&ArgValue::String("forward".to_string()))
            );
            assert_eq!(ctx.metadata.get("find_inclusive"), Some(&ArgValue::Bool(true)));
        } else {
            panic!("Expected Execute, got {result:?}");
        }
    }

    #[test]
    fn test_ext_find_char_backward() {
        // Test find-char-backward interception
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let motions_module = ModuleId::new("motions");
        let keymap = MockKeymap {
            response: KeyLookupState::ExactOnly(CommandId::new(
                motions_module,
                "find-char-backward",
            )),
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let _ = resolve_with_ext(&resolver, &key('F'), &mut state, &input, &mut extensions);

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.pending_char, Some(PendingCharOp::FindBackward));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_till_char_forward() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let motions_module = ModuleId::new("motions");
        let keymap = MockKeymap {
            response: KeyLookupState::ExactOnly(CommandId::new(
                motions_module,
                "till-char-forward",
            )),
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let _ = resolve_with_ext(&resolver, &key('t'), &mut state, &input, &mut extensions);

        // Complete with 'x'
        let result = resolve_with_ext(&resolver, &key('x'), &mut state, &input, &mut extensions);

        if let ResolveResult::Execute(cmd, ctx) = result {
            assert_eq!(cmd.name(), "execute-find-char");
            assert_eq!(ctx.metadata.get("find_char"), Some(&ArgValue::Char('x')));
            assert_eq!(
                ctx.metadata.get("find_direction"),
                Some(&ArgValue::String("forward".to_string()))
            );
            // till is NOT inclusive (is_find() returns false for Till)
            assert_eq!(ctx.metadata.get("find_inclusive"), Some(&ArgValue::Bool(false)));
        } else {
            panic!("Expected Execute, got {result:?}");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_till_char_backward() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let motions_module = ModuleId::new("motions");
        let keymap = MockKeymap {
            response: KeyLookupState::ExactOnly(CommandId::new(
                motions_module,
                "till-char-backward",
            )),
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let _ = resolve_with_ext(&resolver, &key('T'), &mut state, &input, &mut extensions);

        let result = resolve_with_ext(&resolver, &key('z'), &mut state, &input, &mut extensions);

        if let ResolveResult::Execute(cmd, ctx) = result {
            assert_eq!(cmd.name(), "execute-find-char");
            assert_eq!(
                ctx.metadata.get("find_direction"),
                Some(&ArgValue::String("backward".to_string()))
            );
        } else {
            panic!("Expected Execute, got {result:?}");
        }
    }

    // ========================================================================
    // Replace char interception tests (#554)
    // ========================================================================

    #[test]
    fn test_ext_replace_char_sets_pending() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let editor_module = reovim_kernel::api::v1::ModuleId::new("editor");
        let keymap = MockKeymap {
            response: KeyLookupState::ExactOnly(CommandId::new(
                editor_module,
                "replace-char-start",
            )),
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('r'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::Pending));

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.pending_char, Some(PendingCharOp::Replace));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_replace_char_dispatches() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let editor_module = reovim_kernel::api::v1::ModuleId::new("editor");
        let keymap = MockKeymap {
            response: KeyLookupState::ExactOnly(CommandId::new(
                editor_module,
                "replace-char-start",
            )),
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Press 'r' to set pending_char=Replace
        let _ = resolve_with_ext(&resolver, &key('r'), &mut state, &input, &mut extensions);

        // Now press 'x' - should dispatch replace-char with char='x'
        let result = resolve_with_ext(&resolver, &key('x'), &mut state, &input, &mut extensions);

        if let ResolveResult::Execute(cmd, ctx) = result {
            assert_eq!(cmd.name(), "replace-char");
            assert_eq!(cmd.module().as_str(), "editor");
            assert_eq!(ctx.metadata.get("replace_char"), Some(&ArgValue::Char('x')));
        } else {
            panic!("Expected Execute, got {result:?}");
        }
    }

    #[test]
    fn test_ext_macro_record_start_and_stop() {
        use {
            reovim_kernel::api::v1::{RegisterBank, RwLock},
            std::sync::Arc,
        };
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::NORMAL_ID;

        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();

        let registers = Arc::new(RwLock::new(RegisterBank::new()));
        let input = ResolveInput::with_registers(&EMPTY_KEYS, &MODE, &keymap, &registers);
        let mut extensions = ExtensionMap::new();

        // Press 'q' to start recording
        let result = resolve_with_ext(&resolver, &key('q'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::Pending),
            "q should return Pending (waiting for register), got {result:?}"
        );

        // Press 'a' to select register 'a'
        let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::Completed),
            "qa should start recording, got {result:?}"
        );

        // Verify recording started
        let vim = extensions.get::<VimSessionState>().unwrap();
        assert!(vim.is_recording());

        // Record some keys
        let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);
        // 'd' is NotFound, so it becomes NotHandled after being recorded
        assert!(matches!(result, ResolveResult::NotHandled));

        // Press 'q' again to stop recording
        let result = resolve_with_ext(&resolver, &key('q'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::Completed),
            "q should stop recording, got {result:?}"
        );

        // Verify recording stopped
        let vim = extensions.get::<VimSessionState>().unwrap();
        assert!(!vim.is_recording());
    }

    #[test]
    fn test_ext_macro_record_invalid_register() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Press 'q' to start recording
        let _ = resolve_with_ext(&resolver, &key('q'), &mut state, &input, &mut extensions);

        // Press 'A' (uppercase, invalid register for macro recording)
        let result = resolve_with_ext(&resolver, &key('A'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::NotHandled),
            "Invalid register should return NotHandled, got {result:?}"
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_macro_play_basic() {
        use {
            reovim_kernel::api::v1::{RegisterBank, RegisterContent, RwLock},
            std::sync::Arc,
        };
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::NORMAL_ID;

        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();

        let registers = Arc::new(RwLock::new(RegisterBank::new()));
        // Store a macro in register 'a'
        registers
            .write()
            .set_named('a', RegisterContent::characterwise("dw"));

        let input = ResolveInput::with_registers(&EMPTY_KEYS, &MODE, &keymap, &registers);
        let mut extensions = ExtensionMap::new();

        // Press '@' to play macro
        let result = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::Pending),
            "@ should return Pending (waiting for register), got {result:?}"
        );

        // Press 'a' to select register
        let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
        if let ResolveResult::InjectKeys {
            keys,
            exit_macro_playback,
        } = result
        {
            assert!(!keys.is_empty());
            assert!(exit_macro_playback);
        } else {
            panic!("Expected InjectKeys, got {result:?}");
        }
    }

    #[test]
    fn test_ext_macro_play_depth_exceeded() {
        use crate::session_state::MAX_MACRO_DEPTH;

        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Set depth to max
        {
            let vim = extensions.get_or_insert::<VimSessionState>();
            vim.macro_playback_depth = MAX_MACRO_DEPTH;
        }

        // Press '@' - should be blocked due to depth limit
        let result = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::NotHandled),
            "@ with exceeded depth should return NotHandled, got {result:?}"
        );
    }

    #[test]
    fn test_ext_macro_play_invalid_register() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Press '@' to start playback
        let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);

        // Press '5' (not a valid lowercase register)
        let result = resolve_with_ext(&resolver, &key('5'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::NotHandled),
            "Invalid register for @ should return NotHandled, got {result:?}"
        );
    }

    #[test]
    fn test_ext_macro_play_empty_register() {
        use {
            reovim_kernel::api::v1::{RegisterBank, RwLock},
            std::sync::Arc,
        };
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::NORMAL_ID;

        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();

        let registers = Arc::new(RwLock::new(RegisterBank::new()));
        // Register 'a' is empty (no content set)

        let input = ResolveInput::with_registers(&EMPTY_KEYS, &MODE, &keymap, &registers);
        let mut extensions = ExtensionMap::new();

        // Press '@' then 'a'
        let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
        let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::NotHandled),
            "Empty register should return NotHandled, got {result:?}"
        );
    }

    #[test]
    fn test_ext_macro_play_no_registers() {
        // When input.registers is None, macro playback should return NotHandled
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap); // no registers field
        let mut extensions = ExtensionMap::new();

        // Press '@' then 'a'
        let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
        let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
        assert!(
            matches!(result, ResolveResult::NotHandled),
            "No registers should return NotHandled, got {result:?}"
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_macro_play_at_at_repeat() {
        use {
            reovim_kernel::api::v1::{RegisterBank, RegisterContent, RwLock},
            std::sync::Arc,
        };
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::NORMAL_ID;

        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();

        let registers = Arc::new(RwLock::new(RegisterBank::new()));
        registers
            .write()
            .set_named('b', RegisterContent::characterwise("j"));

        let input = ResolveInput::with_registers(&EMPTY_KEYS, &MODE, &keymap, &registers);
        let mut extensions = ExtensionMap::new();

        // First play @b
        let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
        let result = resolve_with_ext(&resolver, &key('b'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::InjectKeys { .. }));

        // Reset playback depth so we can repeat
        {
            let vim = extensions.get_mut::<VimSessionState>().unwrap();
            vim.macro_playback_depth = 0;
        }

        // Now @@ should repeat last macro (register 'b')
        let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
        let result = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
        if let ResolveResult::InjectKeys { keys, .. } = result {
            assert!(!keys.is_empty());
        } else {
            panic!("Expected InjectKeys for @@, got {result:?}");
        }
    }

    #[test]
    fn test_ext_macro_play_non_char_key() {
        // When pending PlayMacro and a non-Char key (like Enter) is pressed
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Press '@' to start playback
        let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);

        // Press Enter (non-Char key, invalid register)
        let result = resolve_with_ext(
            &resolver,
            &KeyEvent::new(KeyCode::Enter),
            &mut state,
            &input,
            &mut extensions,
        );
        assert!(
            matches!(result, ResolveResult::NotHandled),
            "Non-char key for @ should return NotHandled, got {result:?}"
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_operator_exact_only_delete() {
        // Test operator interception via ExactOnly (not ExactWithLonger)
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::enter_delete_operator();
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
                assert_eq!(mode, VimMode::DELETE_ID);
            }
            _ => panic!("Expected ModeTransition::Push, got {result:?}"),
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_operator_exact_with_longer_delete() {
        // Test operator interception via ExactWithLonger
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap {
            response: KeyLookupState::ExactWithLonger {
                exact: CommandId::new(EDITOR_MODULE, "enter-delete-operator"),
            },
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
                assert_eq!(mode, VimMode::DELETE_ID);
            }
            _ => panic!("Expected ModeTransition::Push, got {result:?}"),
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_operator_exact_with_longer_yank() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap {
            response: KeyLookupState::ExactWithLonger {
                exact: CommandId::new(EDITOR_MODULE, "enter-yank-operator"),
            },
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('y'), &mut state, &input, &mut extensions);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
                assert_eq!(mode, VimMode::YANK_ID);
            }
            _ => panic!("Expected ModeTransition::Push, got {result:?}"),
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ext_operator_exact_with_longer_change() {
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap {
            response: KeyLookupState::ExactWithLonger {
                exact: CommandId::new(EDITOR_MODULE, "enter-change-operator"),
            },
        };
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        let result = resolve_with_ext(&resolver, &key('c'), &mut state, &input, &mut extensions);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
                assert_eq!(mode, VimMode::CHANGE_ID);
            }
            _ => panic!("Expected ModeTransition::Push, got {result:?}"),
        }
    }

    #[test]
    fn test_ext_recording_records_keys() {
        // When recording, keys should be recorded in VimSessionState
        let resolver = VimNormalResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("cursor-down");
        let input = resolve_input(&keymap);
        let mut extensions = ExtensionMap::new();

        // Start recording
        {
            let vim = extensions.get_or_insert::<VimSessionState>();
            vim.start_recording('a');
        }

        // Process a key while recording
        let result = resolve_with_ext(&resolver, &key('j'), &mut state, &input, &mut extensions);
        assert!(matches!(result, ResolveResult::Execute(..)));

        // Check the key was recorded
        let vim = extensions.get::<VimSessionState>().unwrap();
        assert!(!vim.recording_keys.is_empty());
    }

    #[test]
    fn test_ext_register_char_ext_valid() {
        let resolver = VimNormalResolver::new();
        let mut vim = VimSessionState {
            pending_register: Some('"'),
            ..VimSessionState::default()
        };

        let result = resolver.handle_register_char_ext(&key('z'), &mut vim);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(vim.pending_register, Some('z'));
    }

    #[test]
    fn test_ext_register_char_ext_invalid_non_char() {
        // handle_register_char_ext with non-Char key
        let resolver = VimNormalResolver::new();
        let mut vim = VimSessionState {
            pending_register: Some('"'),
            ..VimSessionState::default()
        };

        let result = resolver.handle_register_char_ext(&KeyEvent::new(KeyCode::Enter), &mut vim);
        assert!(matches!(result, ResolveResult::NotHandled));
        assert!(vim.pending_register.is_none());
    }

    #[test]
    fn test_pending_keys_empty_initially() {
        use reovim_driver_input::ModeKeyResolver;
        let resolver = VimNormalResolver::new();
        assert!(resolver.pending_keys().is_empty());
    }

    #[test]
    fn test_pending_keys_after_push() {
        use reovim_driver_input::ModeKeyResolver;
        let resolver = VimNormalResolver::new();
        resolver.push_pending_key(key('g'));
        let keys = resolver.pending_keys();
        assert_eq!(keys.len(), 1);
    }
}
