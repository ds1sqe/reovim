// Methods made `pub` for test access from `resolvers::tests::case`.
// This module is private, so `pub` is effectively crate-internal.
#![allow(clippy::missing_panics_doc)]

//! Case operator mode key resolver.
//!
//! This resolver handles the `vim:lowercase`, `vim:uppercase`, and
//! `vim:toggle-case` modes. These are entered via `gu`, `gU`, `g~` in
//! normal mode. The resolver waits for a motion or text object to define
//! the range, then dispatches the case transformation operator.
//!
//! A single `VimCaseResolver` struct is parameterized by `OperatorType`
//! to avoid duplicating the entire resolver for each case variant.

use std::sync::RwLock;

use {
    reovim_driver_input::{
        ExtensionMap, KeyEvent, KeySequence, ModeKeyResolver, ModeState, ModeTransition,
        ResolveContext, ResolveInput, ResolveResult, SessionApiDyn,
    },
    reovim_driver_session::OperatorPendingState,
    reovim_kernel::api::v1::{ModeId, Position},
};

use {
    super::operator_common::{
        KeymapAction, OperatorState, OperatorType, apply_keymap_policy, build_cancelled,
        build_operator_execute, is_count_digit, is_escape, is_inclusive_motion,
        is_line_operator_key, is_linewise_motion,
    },
    crate::{modes::VimMode, session_state::PendingMotion},
};

/// Parameterized case operator resolver.
///
/// Handles `gu{motion}`, `gU{motion}`, `g~{motion}` and their
/// linewise forms `guu`, `gUU`, `g~~`.
pub struct VimCaseResolver {
    /// The specific case operator type.
    operator_type: OperatorType,
    /// Mode ID for this operator mode.
    mode_id: ModeId,
    /// Parent mode ID (normal mode) for inheritance.
    parent_mode_id: ModeId,
    /// Operator state owned by this resolver.
    pub state: RwLock<OperatorState>,
}

impl VimCaseResolver {
    /// Create a new case resolver for the given operator type.
    #[must_use]
    pub fn new(operator_type: OperatorType) -> Self {
        let mode_id = operator_type.mode_id();
        Self {
            operator_type,
            mode_id,
            parent_mode_id: VimMode::NORMAL_ID,
            state: RwLock::new(OperatorState::new(operator_type)),
        }
    }

    /// Get a clone of the current state (for testing).
    #[cfg(test)]
    pub fn state(&self) -> OperatorState {
        self.state.read().expect("lock poisoned").clone()
    }

    /// Clear all internal state.
    fn clear_state(&self) {
        self.state.write().expect("lock poisoned").reset();
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ModeKeyResolver for VimCaseResolver {
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

        // Check for line operator (guu, gUU, g~~)
        if is_line_operator_key(key, self.operator_type) {
            let count = state.operator_count;
            let motion_count = state.take_motion_count().unwrap_or(1);
            let register = state.register;
            state.clear_keys();
            drop(state);

            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(build_operator_execute(
                    self.operator_type,
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

        let lookup_state = {
            let state = input.keymap.query(input.mode, &keys);
            if matches!(state, reovim_driver_input::KeyLookupState::NotFound) {
                input.keymap.query(&self.parent_mode_id, &keys)
            } else {
                state
            }
        };

        match apply_keymap_policy(&lookup_state) {
            KeymapAction::Execute(cmd) => {
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

    #[allow(clippy::too_many_lines)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        _mstate: &mut ModeState,
        input: &ResolveInput<'_>,
        session: &mut dyn SessionApiDyn,
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        tracing::debug!(key = ?key, operator = ?self.operator_type, "case resolver: resolve_with_session");

        if is_escape(key) {
            self.clear_state();
            return ResolveResult::ModeTransition(build_cancelled());
        }

        let mut state = self.state.write().expect("lock poisoned");

        // Initialize from VimSessionState on first key
        if !state.initialized {
            if let Some(vim) = client_extensions.get_mut::<crate::VimSessionState>() {
                state.operator_count = vim.pending_count.take();
                state.register = vim.pending_register.take();
            }
            state.initialized = true;
        }

        if is_count_digit(key, state.has_motion_count()) {
            state.accumulate_motion_count(key);
            return ResolveResult::Pending;
        }

        // Check for line operator (guu, gUU, g~~)
        if is_line_operator_key(key, self.operator_type) {
            let count = state.operator_count;
            let motion_count = state.take_motion_count().unwrap_or(1);
            let register = state.register;
            state.clear_keys();
            drop(state);

            let (start, end) = session.cursor_position().map_or_else(
                || (Position::new(0, 0), Position::new(0, 0)),
                |cursor_pos| {
                    let start = Position::new(cursor_pos.line, 0);
                    let total_count = count.unwrap_or(1) * motion_count;
                    let end_line = cursor_pos.line + total_count - 1;
                    let end = Position::new(end_line, 0);
                    (start, end)
                },
            );

            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(build_operator_execute(
                    self.operator_type,
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

        let lookup_state = {
            let state = input.keymap.query(input.mode, &keys);
            if matches!(state, reovim_driver_input::KeyLookupState::NotFound) {
                input.keymap.query(&self.parent_mode_id, &keys)
            } else {
                state
            }
        };

        match apply_keymap_policy(&lookup_state) {
            KeymapAction::Execute(cmd) => {
                let linewise = is_linewise_motion(&cmd);
                let explicit_count = state.explicit_count();
                let _motion_count = state.take_motion_count();
                state.clear_keys();

                if let Some(start_pos) = session.cursor_position() {
                    state.set_start_position(start_pos);
                }

                let inclusive = !linewise && is_inclusive_motion(&cmd);
                if let Some(vim) = client_extensions.get_mut::<crate::VimSessionState>() {
                    vim.pending_motion = Some(PendingMotion::new(linewise, inclusive, false));
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn on_command_complete(
        &self,
        session: &mut dyn SessionApiDyn,
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> Option<ModeTransition> {
        let state = self.state.read().expect("lock poisoned");
        let count = state.operator_count;
        let register = state.register;
        drop(state);

        // Check for text object range first
        if let Some(op_state) = client_extensions.get_mut::<OperatorPendingState>()
            && let Some(textobj_range) = op_state.take_textobj_range()
        {
            self.clear_state();
            return Some(ModeTransition::Pop {
                result: Some(build_operator_execute(
                    self.operator_type,
                    textobj_range.start,
                    textobj_range.end,
                    textobj_range.is_linewise,
                    count,
                    register,
                )),
            });
        }

        // Motion-based range
        let vim = client_extensions.get_mut::<crate::VimSessionState>()?;

        // Peek first: multi-step motions (jump search) push a mode for
        // label selection before the cursor moves.  Defer completion.
        let _ = vim.pending_motion.as_ref()?;

        let state = self.state.read().expect("lock poisoned");
        let start_pos = state.start_position?;
        drop(state);

        let end_pos = session.cursor_position()?;

        if start_pos == end_pos {
            return None;
        }

        let motion = vim.pending_motion.take()?;

        let (range_start, range_end) = {
            let (start, end) = if start_pos <= end_pos {
                (start_pos, end_pos)
            } else {
                (end_pos, start_pos)
            };

            if !motion.linewise && motion.inclusive {
                (start, Position::new(end.line, end.column + 1))
            } else {
                (start, end)
            }
        };

        self.clear_state();

        Some(ModeTransition::Pop {
            result: Some(build_operator_execute(
                self.operator_type,
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

    fn pending_keys(&self) -> KeySequence {
        self.state.read().expect("lock poisoned").keys()
    }

    fn reset(&mut self) {
        self.state.write().expect("lock poisoned").reset();
    }
}
