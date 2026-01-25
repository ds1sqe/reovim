//! Change operator mode key resolver.
//!
//! This resolver handles the `vim:change` mode, which is entered when `c` is pressed
//! in normal mode. It waits for a motion or text object to define the change range.
//!
//! # Key Features
//!
//! - Mode itself carries operator semantics (no runtime lookup needed)
//! - Resolver owns its state (not stored in `VimSessionState`)
//! - Statusline shows "CHANGE" instead of "OP-PENDING"
//! - After change completes, enters INSERT mode
//!
//! # Examples
//!
//! - `cw` - Change word (deletes word, enters insert)
//! - `cc` - Change line (deletes line content, enters insert)
//! - `c2j` - Change 3 lines (count + motion)

use std::sync::RwLock;

use {
    reovim_driver_input::{
        ExtensionMap, KeyEvent, ModeKeyResolver, ModeState, ModeTransition, ResolveContext,
        ResolveInput, ResolveResult, SessionApiDyn,
    },
    reovim_kernel::api::v1::{ModeId, Position},
};

use {
    super::operator_common::{
        KeymapAction, OperatorState, OperatorType, apply_keymap_policy, build_cancelled,
        build_operator_execute, is_count_digit, is_escape, is_inclusive_motion,
        is_line_operator_key, is_linewise_motion, is_word_forward_motion,
    },
    crate::{modes::VimMode, session_state::PendingMotion},
};

/// Vim change mode key resolver.
///
/// This resolver handles the `vim:change` mode for the `c` operator.
/// After the change operation completes, it enters INSERT mode.
pub struct VimChangeResolver {
    /// Mode ID for change mode.
    mode_id: ModeId,
    /// Parent mode ID (normal mode) for inheritance.
    parent_mode_id: ModeId,
    /// Operator state owned by this resolver.
    state: RwLock<OperatorState>,
}

impl VimChangeResolver {
    /// Create a new change mode resolver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode_id: VimMode::CHANGE_ID,
            parent_mode_id: VimMode::NORMAL_ID,
            state: RwLock::new(OperatorState::new(OperatorType::Change)),
        }
    }

    /// Create a change resolver with initial context (count, register).
    #[must_use]
    pub fn with_context(count: Option<usize>, register: Option<char>) -> Self {
        Self {
            mode_id: VimMode::CHANGE_ID,
            parent_mode_id: VimMode::NORMAL_ID,
            state: RwLock::new(OperatorState::with_context(OperatorType::Change, count, register)),
        }
    }

    /// Get a clone of the current state (for testing).
    #[cfg(test)]
    fn state(&self) -> OperatorState {
        self.state.read().expect("lock poisoned").clone()
    }

    /// Clear all internal state.
    fn clear_state(&self) {
        self.state.write().expect("lock poisoned").reset();
    }
}

impl Default for VimChangeResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimChangeResolver {
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        if is_escape(key) {
            self.clear_state();
            return ResolveResult::ModeTransition(build_cancelled());
        }

        let mut state = self.state.write().expect("lock poisoned");

        if is_count_digit(key, state.has_motion_count()) {
            state.accumulate_motion_count(key);
            return ResolveResult::Pending;
        }

        if is_line_operator_key(key, OperatorType::Change) {
            let count = state.operator_count;
            let motion_count = state.take_motion_count().unwrap_or(1);
            let register = state.register;
            state.clear_keys();
            drop(state);

            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(build_operator_execute(
                    OperatorType::Change,
                    Position::new(0, 0),
                    Position::new(0, 0),
                    true,
                    Some(count.unwrap_or(1) * motion_count),
                    register,
                )),
            });
        }

        state.push_key(*key);
        let keys = state.keys();

        // First try the current mode, then fall back to parent mode for motions
        let lookup_state = {
            let state = input.keymap.query(input.mode, &keys);
            if matches!(state, reovim_driver_input::KeyLookupState::NotFound) {
                // Motion bindings are in normal mode, not operator modes
                input.keymap.query(&self.parent_mode_id, &keys)
            } else {
                state
            }
        };

        match apply_keymap_policy(&lookup_state) {
            KeymapAction::Execute(cmd) => {
                // Get explicit count - None if no count specified
                let explicit_count = state.explicit_count();
                let _motion_count = state.take_motion_count();
                state.clear_keys();
                drop(state);

                let ctx = ResolveContext {
                    count: explicit_count,
                    register: None,
                    keys,
                    metadata: std::collections::HashMap::new(),
                };

                ResolveResult::Execute(cmd, ctx)
            }
            KeymapAction::Pending => {
                drop(state);
                ResolveResult::Pending
            }
            KeymapAction::Cancel => {
                state.clear_keys();
                drop(state);
                self.clear_state();
                ResolveResult::ModeTransition(build_cancelled())
            }
        }
    }

    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        _mstate: &mut ModeState,
        input: &ResolveInput<'_>,
        session: &mut dyn SessionApiDyn,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        tracing::debug!(key = ?key, "change resolver: resolve_with_session");

        if is_escape(key) {
            self.clear_state();
            return ResolveResult::ModeTransition(build_cancelled());
        }

        let mut state = self.state.write().expect("lock poisoned");

        // Initialize state from VimSessionState on first key in this mode entry
        if !state.initialized {
            if let Some(vim) = extensions.get_mut::<crate::VimSessionState>() {
                state.operator_count = vim.pending_count.take();
                state.register = vim.pending_register.take();
                tracing::debug!(
                    operator_count = ?state.operator_count,
                    register = ?state.register,
                    "change resolver: initialized from VimSessionState"
                );
            } else {
                tracing::warn!("change resolver: VimSessionState not found in extensions");
            }
            state.initialized = true;
        }

        if is_count_digit(key, state.has_motion_count()) {
            state.accumulate_motion_count(key);
            return ResolveResult::Pending;
        }

        if is_line_operator_key(key, OperatorType::Change) {
            let count = state.operator_count;
            let motion_count = state.take_motion_count().unwrap_or(1);
            let register = state.register;
            state.clear_keys();
            drop(state);

            // For linewise change, range is [start_line..=end_line] (inclusive)
            // So cc on line 0 with count=1: start=0, end=0 (change 1 line)
            // And 2cc on line 0: start=0, end=1 (change 2 lines)
            let (start, end) = if let Some(buffer_id) = session.active_buffer()
                && let Some(cursor_pos) = session.buffer_position(buffer_id)
            {
                let start = Position::new(cursor_pos.line, 0);
                let total_count = count.unwrap_or(1) * motion_count;
                // end_line is inclusive, so subtract 1 from count
                let end_line = cursor_pos.line + total_count - 1;
                let end = Position::new(end_line, 0);
                (start, end)
            } else {
                // Fallback: change line 0
                (Position::new(0, 0), Position::new(0, 0))
            };

            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(build_operator_execute(
                    OperatorType::Change,
                    start,
                    end,
                    true,
                    Some(count.unwrap_or(1) * motion_count),
                    register,
                )),
            });
        }

        state.push_key(*key);
        let keys = state.keys();

        // First try the current mode, then fall back to parent mode for motions
        let lookup_state = {
            let state = input.keymap.query(input.mode, &keys);
            if matches!(state, reovim_driver_input::KeyLookupState::NotFound) {
                // Motion bindings are in normal mode, not operator modes
                input.keymap.query(&self.parent_mode_id, &keys)
            } else {
                state
            }
        };

        match apply_keymap_policy(&lookup_state) {
            KeymapAction::Execute(cmd) => {
                let linewise = is_linewise_motion(&cmd);
                // Get explicit count - None if no count specified.
                // This preserves "no count" semantics for motions like G/gg.
                let explicit_count = state.explicit_count();
                tracing::debug!(
                    operator_count = ?state.operator_count,
                    motion_count = ?state.motion_count,
                    explicit_count = ?explicit_count,
                    cmd = %cmd,
                    "change resolver: executing motion"
                );
                let _motion_count = state.take_motion_count();
                state.clear_keys();

                if let Some(buffer) = session.active_buffer()
                    && let Some(start_pos) = session.buffer_position(buffer)
                {
                    state.set_start_position(start_pos);
                }

                // Inclusive motions need end position adjustment ($ returns ON last char)
                let inclusive = !linewise && is_inclusive_motion(&cmd);
                let word_forward = !linewise && is_word_forward_motion(&cmd);
                if let Some(vim) = extensions.get_mut::<crate::VimSessionState>() {
                    vim.pending_motion =
                        Some(PendingMotion::new(linewise, inclusive, word_forward));
                }

                drop(state);

                let ctx = ResolveContext {
                    count: explicit_count,
                    register: None,
                    keys,
                    metadata: std::collections::HashMap::new(),
                };

                ResolveResult::Execute(cmd, ctx)
            }
            KeymapAction::Pending => {
                drop(state);
                ResolveResult::Pending
            }
            KeymapAction::Cancel => {
                state.clear_keys();
                drop(state);
                self.clear_state();
                ResolveResult::ModeTransition(build_cancelled())
            }
        }
    }

    fn on_command_complete(
        &self,
        session: &mut dyn SessionApiDyn,
        extensions: &mut ExtensionMap,
    ) -> Option<ModeTransition> {
        let vim = extensions.get_mut::<crate::VimSessionState>()?;
        let motion = vim.pending_motion.take()?;

        let state = self.state.read().expect("lock poisoned");
        let start_pos = state.start_position?;

        let buffer_id = session.active_buffer()?;
        let end_pos = session.buffer_position(buffer_id)?;

        // Normalize range and adjust for motion type
        //
        // Special case: `cw` and `cW` in Vim behave like `ce` and `cE` respectively.
        // They change to the END of the current word, not to the start of the next word.
        // This is documented Vim behavior (`:help cw`).
        let (range_start, range_end) = {
            // First normalize direction (start <= end)
            let (start, end) = if start_pos <= end_pos {
                (start_pos, end_pos)
            } else {
                (end_pos, start_pos)
            };

            // For inclusive characterwise motions, make end exclusive
            // ($ returns cursor ON last char, but Range.end is exclusive)
            if !motion.linewise && motion.inclusive {
                (start, Position::new(end.line, end.column + 1))
            } else if motion.word_forward {
                // For change operator, `cw` should behave like `ce` - change to
                // end of word, not including trailing whitespace. This means we
                // need to subtract 1 from exclusive word-forward motions.
                // The motion puts cursor at start of next word (col 6 for "hello world"),
                // but we only want to change "hello" (cols 0-4), so end should be col 5.
                //
                // This is documented Vim behavior (`:help cw`).
                (start, Position::new(end.line, end.column.saturating_sub(1)))
            } else {
                (start, end)
            }
        };

        let count = state.operator_count;
        let register = state.register;
        drop(state);

        self.clear_state();

        // Change operator: delete text and enter insert mode
        // The change command handler is responsible for entering insert mode
        Some(ModeTransition::Pop {
            result: Some(build_operator_execute(
                OperatorType::Change,
                range_start,
                range_end,
                motion.linewise,
                count,
                register,
            )),
        })
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        Some(&self.parent_mode_id)
    }

    fn reset(&mut self) {
        self.state.write().expect("lock poisoned").reset();
    }
}

#[cfg(test)]
mod tests {
    use {
        reovim_driver_input::{KeyCode, KeyLookupState, KeySequence, KeymapQuery, PopResult},
        reovim_kernel::api::v1::{CommandId, ModuleId},
    };

    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn test_state() -> ModeState {
        ModeState::new(VimMode::CHANGE_ID)
    }

    /// Mock keymap that always returns NotFound (no bindings).
    struct NotFoundKeymap;

    impl KeymapQuery for NotFoundKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::NotFound
        }
    }

    const TEST_MODULE: ModuleId = ModuleId::new("test");
    const EDITOR_MODULE: ModuleId = ModuleId::new("editor");

    struct MockKeymap {
        response: KeyLookupState,
    }

    impl MockKeymap {
        fn exact_only(cmd: &'static str) -> Self {
            Self {
                response: KeyLookupState::ExactOnly(CommandId::new(TEST_MODULE, cmd)),
            }
        }

        /// Create a keymap that returns ExactWithLonger (simulating 'c' with 'cc' as longer match)
        fn exact_with_longer_editor(cmd: &'static str) -> Self {
            Self {
                response: KeyLookupState::ExactWithLonger {
                    exact: CommandId::new(EDITOR_MODULE, cmd),
                },
            }
        }
    }

    impl KeymapQuery for MockKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            self.response.clone()
        }
    }

    fn resolve_input(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::CHANGE_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    #[test]
    fn test_new_resolver() {
        let resolver = VimChangeResolver::new();
        assert_eq!(resolver.mode_id(), &VimMode::CHANGE_ID);
        assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
    }

    #[test]
    fn test_escape_cancels() {
        let resolver = VimChangeResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

        if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
            assert!(matches!(r, PopResult::Cancelled));
        } else {
            panic!("expected ModeTransition::Pop with Cancelled");
        }
    }

    #[test]
    fn test_line_operator_cc() {
        let resolver = VimChangeResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('c'), &mut state, &input);

        if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
            if let PopResult::ExecuteCommand { args, .. } = r {
                assert_eq!(
                    args.get("linewise"),
                    Some(&reovim_driver_command_types::ArgValue::Bang(true))
                );
            } else {
                panic!("expected ExecuteCommand");
            }
        } else {
            panic!("expected ModeTransition::Pop");
        }
    }

    #[test]
    fn test_count_digit() {
        let resolver = VimChangeResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.state().motion_count, Some(3));
    }

    #[test]
    fn test_resolve_with_keymap_executes_motion() {
        let resolver = VimChangeResolver::new();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("word-forward");
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);

        if let ResolveResult::Execute(cmd, _) = result {
            assert_eq!(cmd.name(), "word-forward");
        } else {
            panic!("expected Execute, got {:?}", result);
        }
    }

    // =========================================================================
    // Test operator_count with effective_count (Epic #415)
    // =========================================================================

    #[test]
    fn test_operator_count_with_context() {
        // This test verifies: when operator_count=2, effective_count should be 2
        // Simulates the flow after entering change mode with a count
        let resolver = VimChangeResolver::with_context(Some(2), None);
        let mut mstate = test_state();
        let keymap = MockKeymap::exact_only("word-forward");
        let input = resolve_input(&keymap);

        // Resolver processes 'w' with pre-set operator_count=2
        let result = resolver.resolve_with_keymap(&key('w'), &mut mstate, &input);

        // Verify: should execute motion with count=2
        if let ResolveResult::Execute(cmd, ctx) = result {
            assert_eq!(cmd.name(), "word-forward");
            assert_eq!(ctx.count, Some(2), "effective_count should be 2 for 2cw");
        } else {
            panic!("expected Execute, got {:?}", result);
        }
    }

    #[test]
    fn test_operator_count_multiplied_with_motion_count() {
        // This test verifies: 2c3w should have effective_count=6
        let resolver = VimChangeResolver::with_context(Some(2), None);
        let mut mstate = test_state();
        let keymap = MockKeymap::exact_only("word-forward");
        let input = resolve_input(&keymap);

        // Process '3' (motion count)
        let result = resolver.resolve_with_keymap(&key('3'), &mut mstate, &input);
        assert!(matches!(result, ResolveResult::Pending));

        // Process 'w' (motion)
        let result = resolver.resolve_with_keymap(&key('w'), &mut mstate, &input);

        // Verify: should execute motion with count=6 (2*3)
        if let ResolveResult::Execute(cmd, ctx) = result {
            assert_eq!(cmd.name(), "word-forward");
            assert_eq!(ctx.count, Some(6), "effective_count should be 6 for 2c3w");
        } else {
            panic!("expected Execute, got {:?}", result);
        }
    }

    // =========================================================================
    // Test VimSessionState flow (the actual 2cw flow)
    // =========================================================================

    #[test]
    fn test_full_2cw_flow() {
        // This test simulates the FULL flow of typing "2cw":
        // 1. Normal resolver processes '2' -> stores pending_count=2
        // 2. Normal resolver processes 'c' -> mode transition to CHANGE
        // 3. Change resolver processes 'w' -> should read pending_count=2

        use {crate::resolvers::VimNormalResolver, reovim_driver_session::ExtensionMap};

        // Shared extensions map (like in the runner)
        let mut extensions = ExtensionMap::new();

        // Step 1: Normal resolver processes '2'
        {
            let normal_resolver = VimNormalResolver::new();
            let mut state = reovim_driver_input::ModeState::new(VimMode::NORMAL_ID);
            let keymap = MockKeymap::exact_only("word-forward"); // Doesn't matter for count digit
            let input = resolve_input(&keymap);

            let result = normal_resolver.resolve_with_extensions(
                &key('2'),
                &mut state,
                &input,
                &mut extensions,
            );
            assert!(
                matches!(result, ResolveResult::Pending),
                "Expected Pending after '2', got {:?}",
                result
            );
        }

        // Verify pending_count is set after step 1
        {
            let vim = extensions
                .get::<crate::VimSessionState>()
                .expect("VimSessionState should exist");
            assert_eq!(vim.pending_count, Some(2), "After '2', pending_count should be Some(2)");
        }

        // Step 2: Normal resolver processes 'c'
        // Note: Real keymap returns ExactWithLonger because 'cc' exists as longer match
        // Also uses 'editor' module, not 'test' module
        {
            let normal_resolver = VimNormalResolver::new();
            let mut state = reovim_driver_input::ModeState::new(VimMode::NORMAL_ID);
            let keymap = MockKeymap::exact_with_longer_editor("enter-change-operator");
            let input = resolve_input(&keymap);

            let result = normal_resolver.resolve_with_extensions(
                &key('c'),
                &mut state,
                &input,
                &mut extensions,
            );
            assert!(
                matches!(result, ResolveResult::ModeTransition(ModeTransition::Push { .. })),
                "Expected ModeTransition::Push after 'c', got {:?}",
                result
            );
        }

        // Verify pending_count is STILL set after step 2 (not consumed by normal resolver)
        {
            let vim = extensions
                .get::<crate::VimSessionState>()
                .expect("VimSessionState should exist");
            assert_eq!(
                vim.pending_count,
                Some(2),
                "After 'c', pending_count should STILL be Some(2)"
            );
        }

        // Step 3: Verify pending_count is still available for change resolver
        // Note: We can't fully test resolve_with_session without a complex mock,
        // but we verify the state is correct. The test_operator_count_with_context
        // test verifies that the resolver works correctly when operator_count is set.
        {
            // Verify pending_count is still Some(2) after mode transition
            let vim = extensions
                .get::<crate::VimSessionState>()
                .expect("VimSessionState");
            assert_eq!(
                vim.pending_count,
                Some(2),
                "After mode transition to CHANGE, pending_count should still be Some(2)"
            );

            // The change resolver's resolve_with_session will:
            // 1. Check state.initialized (false for new resolver)
            // 2. Call extensions.get_mut::<VimSessionState>() - should find it
            // 3. Take pending_count - should get Some(2)
            // 4. Store in state.operator_count
            // 5. Calculate effective_count = operator_count * motion_count = 2 * 1 = 2

            // The test_operator_count_with_context test verifies this logic works
            // when operator_count is pre-set. This test verifies the normal resolver
            // correctly preserves pending_count through the mode transition.
        }
    }
}
