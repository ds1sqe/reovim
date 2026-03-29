// Methods made `pub` for test access from `resolvers::tests::delete`.
// This module is private, so `pub` is effectively crate-internal.
#![allow(clippy::missing_panics_doc)]

//! Delete operator mode key resolver.
//!
//! This resolver handles the `vim:delete` mode, which is entered when `d` is pressed
//! in normal mode. It waits for a motion or text object to define the deletion range.
//!
//! # Key Features
//!
//! - Mode itself carries operator semantics (no runtime lookup needed)
//! - Resolver owns its state (not stored in `VimSessionState`)
//! - Statusline shows "DELETE" instead of "OP-PENDING"
//! - Focused ~300 lines vs 1000+ line generic resolver
//!
//! # Examples
//!
//! - `dw` - Delete word (motion completes operator)
//! - `dd` - Delete line (doubled key detection)
//! - `d2j` - Delete 3 lines (count + motion)
//! - `d3w` - Delete 3 words (motion count)
//! - `2d3w` - Delete 6 words (operator count * motion count)

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
        is_line_operator_key, is_linewise_motion, is_word_forward_motion,
    },
    crate::{
        modes::VimMode,
        session_state::{
            ChangeType, LastChange, OperatorType as SessionOperatorType, PendingMotion,
        },
    },
};

/// Vim delete mode key resolver.
///
/// This resolver handles the `vim:delete` mode for the `d` operator.
/// Unlike the generic operator-pending resolver, this resolver:
/// - Knows it's specifically for delete (no runtime operator lookup)
/// - Owns its state directly (cleaner than `VimSessionState.pending_operator`)
/// - Provides clear statusline display ("DELETE")
pub struct VimDeleteResolver {
    /// Mode ID for delete mode.
    mode_id: ModeId,
    /// Parent mode ID (normal mode) for inheritance.
    parent_mode_id: ModeId,
    /// Operator state owned by this resolver.
    pub state: RwLock<OperatorState>,
}

impl VimDeleteResolver {
    /// Create a new delete mode resolver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode_id: VimMode::DELETE_ID,
            parent_mode_id: VimMode::NORMAL_ID,
            state: RwLock::new(OperatorState::new(OperatorType::Delete)),
        }
    }

    /// Create a delete resolver with initial context (count, register).
    #[must_use]
    pub fn with_context(count: Option<usize>, register: Option<char>) -> Self {
        Self {
            mode_id: VimMode::DELETE_ID,
            parent_mode_id: VimMode::NORMAL_ID,
            state: RwLock::new(OperatorState::with_context(OperatorType::Delete, count, register)),
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

impl Default for VimDeleteResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ModeKeyResolver for VimDeleteResolver {
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
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

        // Check for line operator (dd)
        if is_line_operator_key(key, OperatorType::Delete) {
            let count = state.operator_count;
            let motion_count = state.take_motion_count().unwrap_or(1);
            let register = state.register;
            state.clear_keys();
            drop(state);

            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(build_operator_execute(
                    OperatorType::Delete,
                    Position::new(0, 0),
                    Position::new(0, 0),
                    true,
                    Some(count.unwrap_or(1) * motion_count),
                    register,
                )),
            });
        }

        // Add to pending keys
        state.push_key(*key);
        let keys = state.keys();

        // Query keymap for motion/text-object
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

                // Build context with explicit count (None if not specified)
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

    /// Resolve key with session API access.
    ///
    /// This is the primary resolution method. It:
    /// 1. Initializes state from `VimSessionState` on first key (count, register)
    /// 2. Handles special keys (Escape, counts, line operator dd)
    /// 3. Looks up motion in keymap
    /// 4. If motion found, stores start position and returns Execute
    /// 5. Runner calls `on_command_complete` after motion executes
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
        tracing::debug!(key = ?key, "delete resolver: resolve_with_session");

        // #577: Record key for dot repeat
        if let Some(vim) = client_extensions.get_mut::<crate::VimSessionState>() {
            vim.record_repeat_key(*key);
        }

        // Escape cancels the operator
        if is_escape(key) {
            self.clear_state();
            return ResolveResult::ModeTransition(build_cancelled());
        }

        let mut state = self.state.write().expect("lock poisoned");

        // Initialize state from VimSessionState on first key in this mode entry
        if !state.initialized {
            if let Some(vim) = client_extensions.get_mut::<crate::VimSessionState>() {
                state.operator_count = vim.pending_count.take();
                state.register = vim.pending_register.take();
                tracing::debug!(
                    operator_count = ?state.operator_count,
                    register = ?state.register,
                    "delete resolver: initialized from VimSessionState"
                );
            }
            state.initialized = true;
        }

        // Check for count digit
        if is_count_digit(key, state.has_motion_count()) {
            state.accumulate_motion_count(key);
            return ResolveResult::Pending;
        }

        // Check for line operator (dd)
        if is_line_operator_key(key, OperatorType::Delete) {
            let count = state.operator_count;
            let motion_count = state.take_motion_count().unwrap_or(1);
            let register = state.register;
            state.clear_keys();
            drop(state);

            // Get cursor position to calculate line range
            // For linewise delete, range is [start_line..=end_line] (inclusive)
            // So dd on line 0 with count=1: start=0, end=0 (delete 1 line)
            // And 2dd on line 0: start=0, end=1 (delete 2 lines)
            let (start, end) = session.cursor_position().map_or_else(
                || {
                    // Fallback: delete line 0
                    (Position::new(0, 0), Position::new(0, 0))
                },
                |cursor_pos| {
                    let start = Position::new(cursor_pos.line, 0);
                    let total_count = count.unwrap_or(1) * motion_count;
                    // end_line is inclusive, so subtract 1 from count
                    let end_line = cursor_pos.line + total_count - 1;
                    let end = Position::new(end_line, 0);
                    (start, end)
                },
            );

            // #577: Finish dot repeat recording for dd (line operator shortcut)
            if let Some(vim) = client_extensions.get_mut::<crate::VimSessionState>() {
                vim.last_change = Some(LastChange {
                    change_type: ChangeType::OperatorMotion {
                        operator: SessionOperatorType::Delete,
                        linewise: true,
                    },
                    count,
                    register,
                    keys: Vec::new(),
                });
                vim.finish_repeat_recording();
            }

            return ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(build_operator_execute(
                    OperatorType::Delete,
                    start,
                    end,
                    true, // linewise
                    Some(count.unwrap_or(1) * motion_count),
                    register,
                )),
            });
        }

        // Add to pending keys
        state.push_key(*key);
        let keys = state.keys();

        // Query keymap for motion/text-object
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
                // Get explicit count - None if no count specified, Some(n) if specified.
                // This preserves "no count" semantics for motions like G/gg where
                // "G" (no count) means last line, but "1G" means line 1.
                let explicit_count = state.explicit_count();
                tracing::debug!(
                    operator_count = ?state.operator_count,
                    motion_count = ?state.motion_count,
                    explicit_count = ?explicit_count,
                    cmd = %cmd,
                    linewise,
                    "delete resolver: executing motion"
                );
                let _motion_count = state.take_motion_count();
                state.clear_keys();

                // Store start position before motion
                if let Some(start_pos) = session.cursor_position() {
                    state.set_start_position(start_pos);
                }

                // Store motion info in VimSessionState for on_command_complete
                // Inclusive motions need end position adjustment ($ returns ON last char)
                let inclusive = !linewise && is_inclusive_motion(&cmd);
                let word_forward = !linewise && is_word_forward_motion(&cmd);
                if let Some(vim) = client_extensions.get_mut::<crate::VimSessionState>() {
                    vim.pending_motion =
                        Some(PendingMotion::new(linewise, inclusive, word_forward));
                }

                drop(state);

                // Build context with explicit count (None if not specified)
                // This allows motions like G to distinguish "no count" from "count=1"
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

    /// Complete delete operator after motion/text object execution.
    ///
    /// Called by the runner after the motion/text object command executes.
    /// Builds the final delete command with the calculated range.
    ///
    /// # Text Object Priority (Epic #465)
    ///
    /// Text object ranges take priority over motion-based ranges. Text objects
    /// calculate ranges directly (stored in `OperatorPendingState`), while motions
    /// require position-based calculation (`start_position` → `cursor_position`).
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

        // Check for text object range first (Epic #465)
        // Text objects set this range directly, so it takes priority over motion-based calculation
        if let Some(op_state) = client_extensions.get_mut::<OperatorPendingState>()
            && let Some(textobj_range) = op_state.take_textobj_range()
        {
            tracing::debug!(
                range_start = ?textobj_range.start,
                range_end = ?textobj_range.end,
                linewise = textobj_range.is_linewise,
                "delete resolver: using text object range"
            );

            // Record for dot repeat (Epic #465)
            if let Some(vim) = client_extensions.get_mut::<crate::VimSessionState>() {
                vim.last_change = Some(LastChange {
                    change_type: ChangeType::OperatorTextObject {
                        operator: SessionOperatorType::Delete,
                        linewise: textobj_range.is_linewise,
                    },
                    count,
                    register,
                    keys: Vec::new(),
                });
                vim.finish_repeat_recording();
            }

            // Clear state for next operation
            self.clear_state();

            return Some(ModeTransition::Pop {
                result: Some(build_operator_execute(
                    OperatorType::Delete,
                    textobj_range.start,
                    textobj_range.end,
                    textobj_range.is_linewise,
                    count,
                    register,
                )),
            });
        }

        // Fall back to motion-based range calculation
        let vim = client_extensions.get_mut::<crate::VimSessionState>()?;

        // Peek first: if a multi-step motion (e.g., jump search) pushed a
        // mode for label selection, the cursor hasn't moved yet.  Return
        // None so the motion state is preserved for the second
        // on_command_complete call that fires after the cursor actually moves.
        let _ = vim.pending_motion.as_ref()?;

        let state = self.state.read().expect("lock poisoned");
        let start_pos = state.start_position?;
        drop(state);

        let end_pos = session.cursor_position()?;

        if start_pos == end_pos {
            return None;
        }

        // Now consume — the motion has actually completed.
        let motion = vim.pending_motion.take()?;

        // Normalize range and adjust for motion type
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
            } else {
                (start, end)
            }
        };

        // Record for dot repeat (Epic #465)
        if let Some(vim) = client_extensions.get_mut::<crate::VimSessionState>() {
            vim.last_change = Some(LastChange {
                change_type: ChangeType::OperatorMotion {
                    operator: SessionOperatorType::Delete,
                    linewise: motion.linewise,
                },
                count,
                register,
                keys: Vec::new(),
            });
            vim.finish_repeat_recording();
        }

        // Clear state for next operation
        self.clear_state();

        Some(ModeTransition::Pop {
            result: Some(build_operator_execute(
                OperatorType::Delete,
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
