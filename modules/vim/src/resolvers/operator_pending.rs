//! Vim operator-pending mode key resolver.
//!
//! After pressing an operator (d, y, c), we enter operator-pending mode
//! and wait for a motion or text object to define the range.

use std::{collections::HashMap, sync::RwLock};

use {
    reovim_driver_command_types::ArgValue,
    reovim_driver_input::{
        ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState,
        ModeTransition, Modifiers, PopResult, ResolveContext, ResolveInput, ResolveResult,
        SessionApiDyn,
    },
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId, Position},
};

use crate::{
    modes::VimMode,
    session_state::{PendingMotion, VimSessionState},
};

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

impl VimOperatorPendingResolver {
    /// Create a new operator-pending mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: VimMode::OPERATOR_PENDING_ID,
            parent_mode_id: VimMode::NORMAL_ID,
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
    ///
    /// # Characterwise Motions (default)
    /// - w, b, e, W, B, E (word motions)
    /// - h, l (character motions)
    /// - 0, $, ^ (line position motions)
    /// - f, F, t, T (find-char motions)
    fn is_linewise_motion(cmd: &CommandId) -> bool {
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

    /// Build a resolve context with the current state.
    fn build_context(&self, keys: KeySequence) -> ResolveContext {
        let count = self.take_count();

        ResolveContext {
            count,
            register: None, // Register was set in normal mode, not here
            keys,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Check if key is the operator key for line operation.
    ///
    /// When the same operator key is pressed twice (dd, yy, cc), it operates
    /// on the whole current line.
    ///
    /// # Note
    ///
    /// This version uses `state.transition_context` which is deprecated.
    /// Use `is_line_operator_ext` with `VimSessionState` for new code.
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

    /// Check if key is the operator key for line operation (extensions-based).
    ///
    /// Epic #415: Uses `VimSessionState` from extensions instead of
    /// `state.transition_context` which the runner ignores.
    ///
    /// When the same operator key is pressed twice (dd, yy, cc), it operates
    /// on the whole current line.
    fn is_line_operator_ext(key: &KeyEvent, extensions: &ExtensionMap) -> bool {
        if let Some(vim) = extensions.get::<VimSessionState>()
            && let Some(ref pending) = vim.pending_operator
        {
            // Match operator name to expected key
            let op_key = match pending.operator_id.name() {
                "delete" => Some('d'),
                "yank" => Some('y'),
                "change" => Some('c'),
                "indent" => Some('>'),
                "dedent" => Some('<'),
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

    /// Extract operator info from transition context (SSOT).
    ///
    /// Returns `(operator, count, register)` from the mode state's transition context.
    /// If no operator is found, returns a placeholder "noop" command.
    fn extract_operator_info(
        state: &ModeState,
    ) -> (reovim_kernel::api::v1::CommandId, Option<usize>, Option<char>) {
        if let Some(ctx) = &state.transition_context {
            let operator = ctx
                .pending_operator
                .clone()
                .unwrap_or_else(|| CommandId::new(ModuleId::new("noop"), "noop"));
            (operator, ctx.count, ctx.register)
        } else {
            // Fallback - should not happen in normal operation
            (CommandId::new(ModuleId::new("noop"), "noop"), None, None)
        }
    }

    /// Extract operator info from VimSessionState (Epic #415 approach).
    ///
    /// This is the preferred method when using `resolve_with_session()` as it
    /// accesses the canonical vim state stored by the normal resolver.
    fn extract_operator_from_vim_state(
        &self,
        extensions: &mut ExtensionMap,
    ) -> (CommandId, Option<usize>, Option<char>) {
        if let Some(vim) = extensions.get_mut::<VimSessionState>()
            && let Some(pending) = vim.pending_operator.take()
        {
            // Convert OperatorId (vim-specific) to CommandId (kernel type)
            let operator =
                CommandId::new(pending.operator_id.module().clone(), pending.operator_id.name());
            return (operator, pending.count, pending.register);
        }
        // Fallback - should not happen in normal operation
        (CommandId::new(ModuleId::new("noop"), "noop"), None, None)
    }

    /// Build a PopResult::ExecuteCommand with operator arguments.
    ///
    /// This is the vim module building the complete command context.
    /// Runner just executes - no vim knowledge needed.
    fn build_operator_pop_result(
        operator: CommandId,
        start: Position,
        end: Position,
        linewise: bool,
        count: Option<usize>,
        register: Option<char>,
    ) -> PopResult {
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

        PopResult::ExecuteCommand {
            command: operator,
            args,
        }
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
            // Extract operator info from transition context (SSOT)
            let (operator, count, register) = Self::extract_operator_info(state);
            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(Self::build_operator_pop_result(
                    operator,
                    Position::new(0, 0), // Placeholder - runner calculates line range
                    Position::new(0, 0),
                    true, // linewise
                    count,
                    register,
                )),
            });
        }

        // Add to pending keys for motion lookup
        self.push_pending_key(*key);

        // Motion/text-object lookup happens via the runner's registry
        // Return NotHandled to delegate to keymap lookup
        ResolveResult::NotHandled
    }

    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
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
            // Extract operator info from transition context (SSOT)
            let (operator, count, register) = Self::extract_operator_info(state);
            // Motion count (for lines like `d2d`) - currently unused but cleared
            let _motion_count = self.take_count();
            self.clear_pending_keys();
            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(Self::build_operator_pop_result(
                    operator,
                    Position::new(0, 0), // Placeholder - runner calculates line range
                    Position::new(0, 0),
                    true, // linewise
                    count,
                    register,
                )),
            });
        }

        // Add to pending keys for motion/text-object lookup
        self.push_pending_key(*key);
        let keys = self.get_pending_keys();

        // Query keymap for motion/text-object
        let lookup_state = input.keymap.query(input.mode, &keys);

        // Apply Vim policy for operator-pending mode
        match lookup_state {
            KeyLookupState::ExactWithLonger { exact: cmd } | KeyLookupState::ExactOnly(cmd) => {
                // Motion/text-object found - execute it
                let ctx = self.build_context(keys);
                self.clear_pending_keys();
                ResolveResult::Execute(cmd, ctx)
            }
            KeyLookupState::PrefixOnly => {
                // Wait for more keys (e.g., 'i' might become 'iw')
                ResolveResult::Pending
            }
            KeyLookupState::NotFound => {
                // Unknown motion - cancel the operator
                self.clear_state();
                ResolveResult::ModeTransition(ModeTransition::Pop {
                    result: Some(PopResult::Cancelled),
                })
            }
        }
    }

    /// Resolve key with session API access.
    ///
    /// This is the primary resolution method for operator-pending mode in Epic #415.
    /// Instead of executing motions directly (which isn't supported), it returns
    /// `Execute` and lets the runner orchestrate the two-step flow.
    ///
    /// # Flow
    ///
    /// 1. Handle special keys (Escape, counts, line operators)
    /// 2. Look up motion in keymap
    /// 3. If motion found:
    ///    - Store cursor position BEFORE in VimSessionState
    ///    - Return Execute(motion_cmd) - runner will execute it
    ///    - Operator stays pending - runner will complete after motion
    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
        session: &mut dyn SessionApiDyn,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        eprintln!("[DEBUG OP-PENDING] resolve_with_session called with key: {:?}", key);

        // Escape cancels the pending operator
        if Self::is_escape(key) {
            // Clear vim session state
            if let Some(vim) = extensions.get_mut::<VimSessionState>() {
                vim.pending_operator = None;
            }
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
        // Epic #415: Use is_line_operator_ext which reads from VimSessionState
        // (not state.transition_context which the runner ignores)
        if Self::is_line_operator_ext(key, extensions) {
            // Get operator info from VimSessionState (Epic #415 approach)
            let (operator, count, register) = self.extract_operator_from_vim_state(extensions);
            let motion_count = self.take_count().unwrap_or(1);
            self.clear_pending_keys();

            // For linewise operations, get current cursor position to determine line range
            let (start, end) = if let Some(buffer_id) = session.active_buffer()
                && let Some(cursor_pos) = session.buffer_position(buffer_id)
            {
                // Linewise range: start of current line to start of (line + count)
                // The delete operator will delete entire lines when is_linewise=true
                let start = Position::new(cursor_pos.line, 0);
                let end_line = cursor_pos.line + count.unwrap_or(1) * motion_count;
                let end = Position::new(end_line, 0);
                (start, end)
            } else {
                // Fallback if no cursor position (shouldn't happen)
                (Position::new(0, 0), Position::new(1, 0))
            };

            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(Self::build_operator_pop_result(
                    operator, start, end, true, // linewise
                    count, register,
                )),
            });
        }

        // Add to pending keys for motion/text-object lookup
        self.push_pending_key(*key);
        let keys = self.get_pending_keys();

        // Query keymap for motion/text-object
        let lookup_state = input.keymap.query(input.mode, &keys);
        eprintln!(
            "[DEBUG OP-PENDING] keymap query for {:?} in mode {:?}: {:?}",
            keys, input.mode, lookup_state
        );

        match lookup_state {
            KeyLookupState::ExactWithLonger { exact: cmd } | KeyLookupState::ExactOnly(cmd) => {
                // Motion found - store state for on_command_complete hook

                // Per #388: Store motion type in VimSessionState BEFORE dispatch.
                // The resolver knows which motions are linewise vs characterwise.
                let linewise = Self::is_linewise_motion(&cmd);

                // Take motion count ONCE - use for both storing and passing to command
                let motion_count = self.take_count();

                if let Some(vim) = extensions.get_mut::<VimSessionState>() {
                    // Store pending motion info
                    vim.pending_motion = Some(PendingMotion::new(linewise));

                    // Store start position in pending operator
                    if let Some(buffer) = session.active_buffer()
                        && let Some(start_pos) = session.buffer_position(buffer)
                        && let Some(ref mut pending) = vim.pending_operator
                    {
                        pending.start_position = Some(start_pos);
                        // Include motion count
                        if let Some(count) = motion_count {
                            pending.motion_count = Some(count);
                        }
                    }
                }

                // Build context with motion count (so motion executes with correct count)
                let ctx = ResolveContext {
                    count: motion_count,
                    register: None,
                    keys,
                    metadata: std::collections::HashMap::new(),
                };
                self.clear_pending_keys();

                // Return Execute - runner calls on_command_complete after motion
                ResolveResult::Execute(cmd, ctx)
            }
            KeyLookupState::PrefixOnly => {
                // Wait for more keys (e.g., 'i' might become 'iw')
                ResolveResult::Pending
            }
            KeyLookupState::NotFound => {
                // Unknown motion - cancel the operator
                if let Some(vim) = extensions.get_mut::<VimSessionState>() {
                    vim.pending_operator = None;
                }
                self.clear_state();
                ResolveResult::ModeTransition(ModeTransition::Pop {
                    result: Some(PopResult::Cancelled),
                })
            }
        }
    }

    /// Complete pending operator after command execution (Epic #415, Issue #388).
    ///
    /// Called by the runner after any command executes successfully.
    /// Checks `VimSessionState` for pending motion info and completes
    /// the operator if needed.
    ///
    /// # Flow
    ///
    /// 1. Check `VimSessionState` for `pending_motion` (set before dispatch)
    /// 2. Check for `pending_operator` with start position
    /// 3. Get current cursor position (end of motion) from session
    /// 4. Build `PopResult::OperatorRange` with the range
    /// 5. Return `ModeTransition::Pop` to complete the operator
    fn on_command_complete(
        &self,
        session: &mut dyn SessionApiDyn,
        extensions: &mut ExtensionMap,
    ) -> Option<ModeTransition> {
        eprintln!("[DEBUG OP-PENDING] on_command_complete called");

        // Get pending motion and operator from VimSessionState
        let vim = extensions.get_mut::<VimSessionState>()?;
        eprintln!(
            "[DEBUG OP-PENDING] Got VimSessionState, pending_motion={:?}, pending_operator={:?}",
            vim.pending_motion.is_some(),
            vim.pending_operator.is_some()
        );

        // Take pending motion - if none, this wasn't a dispatched motion
        let motion = vim.pending_motion.take()?;
        eprintln!("[DEBUG OP-PENDING] Got pending_motion: linewise={}", motion.linewise);

        // Take pending operator
        let pending = vim.pending_operator.take()?;
        eprintln!("[DEBUG OP-PENDING] Got pending_operator: {:?}", pending.operator_id);

        // Need start position to calculate range
        let start_pos = pending.start_position?;

        // Get current cursor position (end of motion)
        let buffer_id = session.active_buffer()?;
        let end_pos = session.buffer_position(buffer_id)?;

        // Normalize range (start <= end for characterwise)
        let (range_start, range_end) = if start_pos <= end_pos {
            (start_pos, end_pos)
        } else {
            (end_pos, start_pos)
        };

        // Build operator command ID
        let operator =
            CommandId::new(pending.operator_id.module().clone(), pending.operator_id.name());

        // Return mode transition to complete the operator
        Some(ModeTransition::Pop {
            result: Some(Self::build_operator_pop_result(
                operator,
                range_start,
                range_end,
                motion.linewise,
                pending.count,
                pending.register,
            )),
        })
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
        ModeState::new(VimMode::OPERATOR_PENDING_ID)
    }

    fn test_state_with_operator(op_name: &'static str) -> ModeState {
        let mut state = ModeState::new(VimMode::OPERATOR_PENDING_ID);
        state.transition_context = Some(TransitionContext::with_operator(CommandId::new(
            reovim_kernel::api::v1::ModuleId::new("editor"),
            op_name,
        )));
        state
    }

    #[test]
    fn test_new_resolver() {
        let resolver = VimOperatorPendingResolver::new();
        assert_eq!(resolver.mode_id(), &VimMode::OPERATOR_PENDING_ID);
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
            if let PopResult::ExecuteCommand { args, .. } = r {
                // Check linewise flag is set to true
                assert_eq!(args.get("linewise"), Some(&ArgValue::Bang(true)));
            } else {
                panic!("expected ExecuteCommand");
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
            if let PopResult::ExecuteCommand { args, .. } = r {
                // Check linewise flag is set to true
                assert_eq!(args.get("linewise"), Some(&ArgValue::Bang(true)));
            } else {
                panic!("expected ExecuteCommand");
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

    // =========================================================================
    // resolve_with_keymap tests
    // =========================================================================
    // These tests verify that resolve_with_keymap correctly applies Vim policy
    // based on the KeyLookupState returned by the keymap.

    use reovim_driver_input::KeymapQuery;

    use reovim_kernel::api::v1::ModuleId;

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
        static MODE: ModeId = VimMode::OPERATOR_PENDING_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    #[test]
    fn test_resolve_with_keymap_exact_only_returns_execute() {
        // When keymap returns ExactOnly, execute the motion.
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("word-forward");
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);

        if let ResolveResult::Execute(cmd, ctx) = result {
            assert_eq!(cmd.name(), "word-forward");
            assert!(ctx.count.is_none()); // No count set
        } else {
            panic!("expected Execute, got {result:?}");
        }
    }

    #[test]
    fn test_resolve_with_keymap_exact_with_longer_returns_execute() {
        // For operator-pending, ExactWithLonger should execute immediately
        // (unlike normal mode which waits). Motions don't have prefixes in Vim.
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_with_longer("word-forward");
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);

        if let ResolveResult::Execute(cmd, _) = result {
            assert_eq!(cmd.name(), "word-forward");
        } else {
            panic!("expected Execute, got {result:?}");
        }
    }

    #[test]
    fn test_resolve_with_keymap_prefix_only_returns_pending() {
        // When keymap returns PrefixOnly (e.g., 'i' might become 'iw'), wait.
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::prefix_only();
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('i'), &mut state, &input);

        assert!(matches!(result, ResolveResult::Pending));
        // Keys should be accumulated
        assert!(!resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_resolve_with_keymap_not_found_cancels() {
        // Unknown motion cancels the operator.
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);

        if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
            assert!(matches!(r, PopResult::Cancelled));
        } else {
            panic!("expected ModeTransition::Pop with Cancelled, got {result:?}");
        }
    }

    #[test]
    fn test_resolve_with_keymap_count_flows_to_context() {
        // Count accumulated in operator-pending should appear in Execute context.
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("word-forward");
        let input = resolve_input(&keymap);

        // Accumulate count first: d2w = delete 2 words
        let _ = resolver.resolve_with_keymap(&key('2'), &mut state, &input);
        assert_eq!(resolver.pending_count(), Some(2));

        // Now the motion
        let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);

        if let ResolveResult::Execute(cmd, ctx) = result {
            assert_eq!(cmd.name(), "word-forward");
            assert_eq!(ctx.count, Some(2));
        } else {
            panic!("expected Execute with count, got {result:?}");
        }
    }

    #[test]
    fn test_resolve_with_keymap_escape_cancels() {
        // Escape should cancel the operator.
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        // Accumulate some state first
        let _ = resolver.resolve_with_keymap(&key('2'), &mut state, &input);
        assert_eq!(resolver.pending_count(), Some(2));

        // Escape cancels
        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

        if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
            assert!(matches!(r, PopResult::Cancelled));
        } else {
            panic!("expected Cancelled");
        }

        // State should be cleared
        assert!(resolver.pending_count().is_none());
        assert!(resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_resolve_with_keymap_line_operator_dd() {
        // dd should return linewise Pop even with keymap-aware resolution
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state_with_operator("delete");
        let keymap = MockKeymap::not_found(); // Doesn't matter, line operator checked first
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('d'), &mut state, &input);

        if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
            if let PopResult::ExecuteCommand { args, .. } = r {
                // Check linewise flag is set to true
                assert_eq!(args.get("linewise"), Some(&ArgValue::Bang(true)));
            } else {
                panic!("expected ExecuteCommand");
            }
        } else {
            panic!("expected ModeTransition::Pop");
        }
    }

    #[test]
    fn test_resolve_with_keymap_pending_keys_cleared_after_execute() {
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("word-forward");
        let input = resolve_input(&keymap);

        let _ = resolver.resolve_with_keymap(&key('w'), &mut state, &input);

        // Pending keys should be cleared after execute
        assert!(resolver.get_pending_keys().is_empty());
    }

    #[test]
    fn test_resolve_with_keymap_pending_keys_cleared_after_cancel() {
        let resolver = VimOperatorPendingResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        let _ = resolver.resolve_with_keymap(&key('z'), &mut state, &input);

        // Pending keys should be cleared after cancel
        assert!(resolver.get_pending_keys().is_empty());
    }
}
