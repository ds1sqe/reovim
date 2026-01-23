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

use super::operator_common::{
    KeymapAction, OperatorState, OperatorType, apply_keymap_policy, build_cancelled,
    build_operator_execute, is_count_digit, is_escape, is_line_operator_key, is_linewise_motion,
};
use crate::{modes::VimMode, session_state::PendingMotion};

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
            state: RwLock::new(OperatorState::with_context(
                OperatorType::Change,
                count,
                register,
            )),
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
    fn resolve(&self, key: &KeyEvent, _state: &mut ModeState) -> ResolveResult {
        // Escape cancels the operator
        if is_escape(key) {
            self.clear_state();
            return ResolveResult::ModeTransition(build_cancelled());
        }

        let mut state = self.state.write().expect("lock poisoned");

        // Check for count digit
        if is_count_digit(key, state.has_motion_count()) {
            state.accumulate_motion_count(key);
            return ResolveResult::Pending;
        }

        // Check for line operator (cc)
        if is_line_operator_key(key, OperatorType::Change) {
            let count = state.operator_count;
            let motion_count = state.take_motion_count().unwrap_or(1);
            let register = state.register;
            drop(state);

            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(build_operator_execute(
                    OperatorType::Change,
                    Position::new(0, 0),
                    Position::new(0, 0),
                    true, // linewise
                    Some(count.unwrap_or(1) * motion_count),
                    register,
                )),
            });
        }

        // Add to pending keys for motion lookup
        state.push_key(*key);
        drop(state);

        ResolveResult::NotHandled
    }

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

        let lookup_state = input.keymap.query(input.mode, &keys);

        match apply_keymap_policy(&lookup_state) {
            KeymapAction::Execute(cmd) => {
                let motion_count = state.take_motion_count();
                state.clear_keys();
                drop(state);

                let ctx = ResolveContext {
                    count: motion_count,
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

            let (start, end) = if let Some(buffer_id) = session.active_buffer()
                && let Some(cursor_pos) = session.buffer_position(buffer_id)
            {
                let start = Position::new(cursor_pos.line, 0);
                let total_count = count.unwrap_or(1) * motion_count;
                let end_line = cursor_pos.line + total_count;
                let end = Position::new(end_line, 0);
                (start, end)
            } else {
                (Position::new(0, 0), Position::new(1, 0))
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

        let lookup_state = input.keymap.query(input.mode, &keys);

        match apply_keymap_policy(&lookup_state) {
            KeymapAction::Execute(cmd) => {
                let linewise = is_linewise_motion(&cmd);
                let motion_count = state.take_motion_count();
                state.clear_keys();

                if let Some(buffer) = session.active_buffer()
                    && let Some(start_pos) = session.buffer_position(buffer)
                {
                    state.set_start_position(start_pos);
                }

                if let Some(vim) = extensions.get_mut::<crate::VimSessionState>() {
                    vim.pending_motion = Some(PendingMotion::new(linewise));
                }

                drop(state);

                let ctx = ResolveContext {
                    count: motion_count,
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

        let (range_start, range_end) = if start_pos <= end_pos {
            (start_pos, end_pos)
        } else {
            (end_pos, start_pos)
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
        reovim_driver_input::{KeyCode, PopResult},
        reovim_kernel::api::v1::CommandId,
    };

    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn test_state() -> ModeState {
        ModeState::new(VimMode::CHANGE_ID)
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

        let result = resolver.resolve(&KeyEvent::new(KeyCode::Escape), &mut state);

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

        let result = resolver.resolve(&key('c'), &mut state);

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

        let result = resolver.resolve(&key('3'), &mut state);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.state().motion_count, Some(3));
    }

    // =========================================================================
    // Keymap-aware tests
    // =========================================================================

    use reovim_driver_input::{KeySequence, KeymapQuery};

    use reovim_kernel::api::v1::ModuleId;

    const TEST_MODULE: ModuleId = ModuleId::new("test");

    struct MockKeymap {
        response: reovim_driver_input::KeyLookupState,
    }

    impl MockKeymap {
        fn exact_only(cmd: &'static str) -> Self {
            Self {
                response: reovim_driver_input::KeyLookupState::ExactOnly(CommandId::new(
                    TEST_MODULE,
                    cmd,
                )),
            }
        }
    }

    impl KeymapQuery for MockKeymap {
        fn query(
            &self,
            _mode: &ModeId,
            _keys: &KeySequence,
        ) -> reovim_driver_input::KeyLookupState {
            self.response.clone()
        }
    }

    fn resolve_input(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::CHANGE_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
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
}
