// Methods made `pub` for test access from `resolvers::tests::normal`.
// This module is private, so `pub` is effectively crate-internal.
#![allow(clippy::missing_panics_doc, clippy::must_use_candidate)]

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
    ids,
    macros::notation_to_keys,
    modes::VimMode,
    session_state::{PendingCharOp, VimSessionState},
};

use reovim_module_cmdline::CmdlineState;

/// Coordinator command dispatched for find-char motions (#563).
/// Defined locally to reference the motions module's command without
/// compile-time dependency.
const DISPATCH_FIND_CHAR: reovim_kernel::api::v1::CommandId =
    reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("motions"),
        "dispatch-find-char",
    );

/// Pending macro operation.
///
/// Tracks what macro-related action we're waiting for after pressing `q` or `@`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingMacroOp {
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
    pub pending_register: RwLock<Option<char>>,

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
    pub fn is_count_digit(&self, key: &KeyEvent) -> bool {
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
    pub fn accumulate_count(&self, key: &KeyEvent) {
        if let KeyCode::Char(c @ '0'..='9') = key.code {
            let digit = c.to_digit(10).expect("valid digit") as usize;
            let mut guard = self.pending_count.write().expect("lock poisoned");
            *guard = Some(guard.unwrap_or(0) * 10 + digit);
        }
    }

    /// Check if a key is the register prefix (`"`).
    pub fn is_register_prefix(key: &KeyEvent) -> bool {
        key.modifiers == Modifiers::NONE && key.code == KeyCode::Char('"')
    }

    /// Handle register character after `"` prefix.
    pub fn handle_register_char(&self, key: &KeyEvent) -> ResolveResult {
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
    pub fn take_count(&self) -> Option<usize> {
        self.pending_count.write().expect("lock poisoned").take()
    }

    /// Take the pending register, clearing it.
    pub fn take_register(&self) -> Option<char> {
        let reg = self.pending_register.write().expect("lock poisoned").take();
        // Don't return the sentinel
        reg.filter(|&r| r != '"')
    }

    /// Clear pending keys.
    fn clear_pending_keys(&self) {
        self.pending_keys.write().expect("lock poisoned").clear();
    }

    /// Add a key to pending sequence.
    pub fn push_pending_key(&self, key: KeyEvent) {
        self.pending_keys.write().expect("lock poisoned").push(key);
    }

    /// Get a clone of pending keys for lookup.
    pub fn get_pending_keys(&self) -> KeySequence {
        self.pending_keys.read().expect("lock poisoned").clone()
    }

    /// Clear all internal state (for use from &self via interior mutability).
    pub fn clear_state(&self) {
        *self.pending_count.write().expect("lock poisoned") = None;
        *self.pending_register.write().expect("lock poisoned") = None;
        self.pending_keys.write().expect("lock poisoned").clear();
        *self.pending_macro.write().expect("lock poisoned") = None;
    }

    // ========================================================================
    // Macro Recording/Playback Helpers (Epic #465 Phase 8D)
    // ========================================================================

    /// Check if we're waiting for a macro operation register.
    pub fn pending_macro_op(&self) -> Option<PendingMacroOp> {
        *self.pending_macro.read().expect("lock poisoned")
    }

    /// Set pending macro operation.
    pub fn set_pending_macro(&self, op: PendingMacroOp) {
        *self.pending_macro.write().expect("lock poisoned") = Some(op);
    }

    /// Clear pending macro operation.
    pub fn clear_pending_macro(&self) {
        *self.pending_macro.write().expect("lock poisoned") = None;
    }

    /// Check if a key is the macro record key (`q` without modifiers).
    pub fn is_macro_record_key(key: &KeyEvent) -> bool {
        key.modifiers == Modifiers::NONE && key.code == KeyCode::Char('q')
    }

    /// Check if a key is the macro play key (`@` without modifiers).
    pub fn is_macro_play_key(key: &KeyEvent) -> bool {
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
                    use reovim_domain_text::RegisterContent;
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    pub fn build_context(&self, keys: KeySequence) -> ResolveContext {
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
    pub fn is_count_digit_ext(&self, key: &KeyEvent, vim: &VimSessionState) -> bool {
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
    pub fn accumulate_count_ext(&self, key: &KeyEvent, vim: &mut VimSessionState) {
        if let KeyCode::Char(c @ '0'..='9') = key.code {
            let digit = c.to_digit(10).expect("valid digit") as usize;
            vim.pending_count = Some(vim.pending_count.unwrap_or(0) * 10 + digit);
        }
    }

    /// Handle register character after `"` prefix (extension-based version).
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn handle_register_char_ext(
        &self,
        key: &KeyEvent,
        vim: &mut VimSessionState,
    ) -> ResolveResult {
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
    pub fn build_context_ext(
        &self,
        keys: KeySequence,
        vim: &mut VimSessionState,
    ) -> ResolveContext {
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
    pub fn classify_find_char_command(
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

    /// Classify a command as a mark operation, if applicable (#654).
    ///
    /// Returns the corresponding `PendingCharOp` if the command is one of the
    /// mark commands (m, ', `` ` ``), otherwise returns `None`.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn classify_mark_command(cmd: &reovim_kernel::api::v1::CommandId) -> Option<PendingCharOp> {
        if *cmd == editor::ids::SET_MARK {
            Some(PendingCharOp::SetMark)
        } else if *cmd == editor::ids::GOTO_MARK_LINE {
            Some(PendingCharOp::GotoMarkLine)
        } else if *cmd == editor::ids::GOTO_MARK_EXACT {
            Some(PendingCharOp::GotoMarkExact)
        } else {
            None
        }
    }

    /// Check if a command is an insert entry command (#577).
    ///
    /// These commands start a change that should be recorded for dot repeat.
    /// The recording starts here and continues through insert mode until
    /// `ExitToNormal` finishes it.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn is_insert_entry_command(cmd: &reovim_kernel::api::v1::CommandId) -> bool {
        *cmd == ids::ENTER_INSERT
            || *cmd == ids::ENTER_INSERT_AFTER
            || *cmd == ids::ENTER_INSERT_EOL
            || *cmd == ids::ENTER_INSERT_BOL
            || *cmd == ids::OPEN_LINE_BELOW
            || *cmd == ids::OPEN_LINE_ABOVE
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn classify_operator_mode(cmd: &reovim_kernel::api::v1::CommandId) -> Option<ModeId> {
        // Hard-typed check using editor module constants (compile-time safe)
        // Returns the dedicated operator mode (not generic operator-pending)
        if *cmd == editor::ids::ENTER_DELETE_OPERATOR {
            Some(VimMode::DELETE_ID)
        } else if *cmd == editor::ids::ENTER_YANK_OPERATOR {
            Some(VimMode::YANK_ID)
        } else if *cmd == editor::ids::ENTER_CHANGE_OPERATOR {
            Some(VimMode::CHANGE_ID)
        } else if *cmd == editor::ids::ENTER_LOWERCASE_OPERATOR {
            Some(VimMode::LOWERCASE_ID)
        } else if *cmd == editor::ids::ENTER_UPPERCASE_OPERATOR {
            Some(VimMode::UPPERCASE_ID)
        } else if *cmd == editor::ids::ENTER_TOGGLE_CASE_OPERATOR {
            Some(VimMode::TOGGLE_CASE_ID)
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
        // Clear any pending cmdline message on next keypress (#558)
        if let Some(cmdline) = client_extensions.get_mut::<CmdlineState>() {
            cmdline.clear_message();
        }

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
                return ResolveResult::Execute(DISPATCH_FIND_CHAR, ctx);
            }

            // Replace operation (r) — dispatch to editor::REPLACE_CHAR
            if matches!(pending_op, PendingCharOp::Replace) {
                ctx.metadata
                    .insert("replace_char".to_string(), ArgValue::Char(c));
                return ResolveResult::Execute(editor::ids::REPLACE_CHAR, ctx);
            }

            // #654 - Mark operations (m, ', `)
            let cmd_id = match pending_op {
                PendingCharOp::SetMark => editor::ids::SET_MARK,
                PendingCharOp::GotoMarkLine => editor::ids::GOTO_MARK_LINE,
                PendingCharOp::GotoMarkExact => editor::ids::GOTO_MARK_EXACT,
                _ => unreachable!("All pending ops handled above"),
            };
            ctx.metadata
                .insert("mark_char".to_string(), ArgValue::Char(c));
            return ResolveResult::Execute(cmd_id, ctx);
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
                    // #577: Start recording keys for dot repeat
                    vim.start_repeat_recording();
                    vim.record_repeat_key(*key);
                    return ResolveResult::ModeTransition(ModeTransition::Push {
                        mode: target_mode,
                        context: TransitionContext::new(),
                    });
                }

                // Not an operator - wait for longer sequence
                ResolveResult::Pending
            }
            KeyLookupState::ExactOnly(cmd) => {
                // #577 - Intercept dot repeat and replay recorded keys
                if cmd == ids::DOT_REPEAT
                    && let Some(ref lc) = vim.last_change
                    && !lc.keys.is_empty()
                {
                    let replay_keys = lc.keys.clone();
                    self.clear_pending_keys();
                    return ResolveResult::InjectKeys {
                        keys: replay_keys,
                        exit_macro_playback: false,
                    };
                }

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

                // #654 - Intercept mark commands (m, ', `)
                if let Some(mark_op) = Self::classify_mark_command(&cmd) {
                    vim.pending_char = Some(mark_op);
                    self.clear_pending_keys();
                    return ResolveResult::Pending;
                }

                // Epic #415 - Push to dedicated operator modes (DELETE, YANK, CHANGE)
                // Instead of executing enter-*-operator commands (which do nothing),
                // push to the specific operator mode. The resolver reads pending_count
                // and pending_register from VimSessionState on its first key press.
                if let Some(target_mode) = Self::classify_operator_mode(&cmd) {
                    self.clear_pending_keys();
                    // #577: Start recording keys for dot repeat
                    vim.start_repeat_recording();
                    vim.record_repeat_key(*key);
                    return ResolveResult::ModeTransition(ModeTransition::Push {
                        mode: target_mode,
                        context: TransitionContext::new(),
                    });
                }

                // #577: Start recording on insert entry commands
                if Self::is_insert_entry_command(&cmd) {
                    vim.start_repeat_recording();
                    vim.record_repeat_key(*key);
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn pending_keys(&self) -> KeySequence {
        self.get_pending_keys()
    }

    fn reset(&mut self) {
        *self.pending_count.write().expect("lock poisoned") = None;
        *self.pending_register.write().expect("lock poisoned") = None;
        self.pending_keys.write().expect("lock poisoned").clear();
        *self.pending_macro.write().expect("lock poisoned") = None;
    }
}

#[cfg(test)]
impl VimNormalResolver {
    /// Get a clone of pending keys (for testing).
    pub fn pending_keys(&self) -> KeySequence {
        self.get_pending_keys()
    }
}
