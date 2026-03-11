// Methods made `pub` for test access from `resolvers::tests::visual`.
// This module is private, so `pub` is effectively crate-internal.
#![allow(clippy::missing_panics_doc)]

//! Visual mode key resolver.
//!
//! This resolver handles all three visual modes:
//! - `vim:visual` - Character-wise selection (v)
//! - `vim:visual-line` - Line-wise selection (V)
//! - `vim:visual-block` - Block/rectangular selection (Ctrl-V)
//!
//! # Key Differences from Normal Mode
//!
//! Visual mode differs from normal mode in how motions and operators behave:
//! - **Motions**: Extend the selection rather than just moving the cursor
//! - **Operators (d, y, c)**: Operate on the current selection immediately
//! - **Escape**: Exits visual mode and clears selection
//!
//! # Resolution Flow
//!
//! 1. Escape → exit visual mode, clear selection
//! 2. Count digits → accumulate for motion repeat
//! 3. Motion keys → look up in Normal mode keymap, execute to extend selection
//! 4. Visual operators (d, y, c) → execute selection-based commands via keybindings
//!
//! # Selection Model
//!
//! Selection tracking is handled by the kernel's `Selection` API:
//! - When visual mode is entered, `selection.start()` is called with cursor position
//! - Each motion extends the selection by moving the cursor
//! - The selection range is from anchor (start) to cursor (current position)

use std::sync::RwLock;

use {
    reovim_driver_input::{
        ExtensionMap, KeyCode, KeyEvent, KeySequence, ModeKeyResolver, ModeState, Modifiers,
        ResolveContext, ResolveInput, ResolveResult, SessionApiDyn,
    },
    reovim_kernel::api::v1::ModeId,
};

use {
    super::operator_common::{KeymapAction, apply_keymap_policy, is_count_digit, is_escape},
    crate::modes::VimMode,
};

/// Visual mode state owned by the resolver.
///
/// Unlike operator modes (delete/yank/change), visual mode doesn't track
/// operator state. It only tracks:
/// - Pending count for motion repetition
/// - Pending keys for multi-key motions (e.g., gg, G)
#[derive(Debug, Clone)]
pub struct VisualState {
    /// Pending count for motions (e.g., `3j` in visual mode).
    pub motion_count: Option<usize>,
    /// Accumulated key sequence for multi-key motions.
    pub pending_keys: KeySequence,
    /// Whether the resolver has been initialized for this mode entry.
    pub initialized: bool,
}

impl VisualState {
    /// Create new visual mode state.
    const fn new() -> Self {
        Self {
            motion_count: None,
            pending_keys: KeySequence::new(),
            initialized: false,
        }
    }

    /// Check if we have a motion count.
    pub const fn has_motion_count(&self) -> bool {
        self.motion_count.is_some()
    }

    /// Accumulate a count digit.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn accumulate_motion_count(&mut self, key: &KeyEvent) {
        if let KeyCode::Char(c @ '0'..='9') = key.code {
            let digit = c.to_digit(10).expect("valid digit") as usize;
            self.motion_count = Some(self.motion_count.unwrap_or(0) * 10 + digit);
        }
    }

    /// Take the motion count, returning it and clearing it.
    #[allow(clippy::missing_const_for_fn)] // Option::take is not const
    fn take_motion_count(&mut self) -> Option<usize> {
        self.motion_count.take()
    }

    /// Get the explicit count (None if not specified).
    const fn explicit_count(&self) -> Option<usize> {
        self.motion_count
    }

    /// Push a key to the pending sequence.
    fn push_key(&mut self, key: KeyEvent) {
        self.pending_keys.push(key);
    }

    /// Get a clone of pending keys.
    fn keys(&self) -> KeySequence {
        self.pending_keys.clone()
    }

    /// Clear pending keys.
    fn clear_keys(&mut self) {
        self.pending_keys.clear();
    }

    /// Reset all state for mode re-entry.
    fn reset(&mut self) {
        self.motion_count = None;
        self.pending_keys.clear();
        self.initialized = false;
    }
}

/// Vim visual mode key resolver.
///
/// Handles all three visual mode variants:
/// - `vim:visual` - Character-wise selection
/// - `vim:visual-line` - Line-wise selection
/// - `vim:visual-block` - Block selection
///
/// # Example
///
/// ```ignore
/// let resolver = VimVisualResolver::new(VimMode::VISUAL_ID);
/// resolver_registry.register(resolver);
/// ```
pub struct VimVisualResolver {
    /// Mode ID for this visual mode variant.
    mode_id: ModeId,
    /// Parent mode ID for motion lookup (always Normal).
    parent_mode_id: ModeId,
    /// Resolver state.
    state: RwLock<VisualState>,
}

impl VimVisualResolver {
    /// Create a new visual mode resolver for the given mode.
    ///
    /// # Arguments
    ///
    /// * `mode_id` - The visual mode variant (`VISUAL_ID`, `VISUAL_LINE_ID`, or `VISUAL_BLOCK_ID`)
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new is not const
    pub fn new(mode_id: ModeId) -> Self {
        Self {
            mode_id,
            parent_mode_id: VimMode::NORMAL_ID,
            state: RwLock::new(VisualState::new()),
        }
    }

    /// Create a character-wise visual mode resolver.
    #[must_use]
    pub fn character_wise() -> Self {
        Self::new(VimMode::VISUAL_ID)
    }

    /// Create a line-wise visual mode resolver.
    #[must_use]
    pub fn line_wise() -> Self {
        Self::new(VimMode::VISUAL_LINE_ID)
    }

    /// Create a block visual mode resolver.
    #[must_use]
    pub fn block_wise() -> Self {
        Self::new(VimMode::VISUAL_BLOCK_ID)
    }

    /// Clear all internal state.
    fn clear_state(&self) {
        self.state.write().expect("lock poisoned").reset();
    }

    /// Get a clone of the current state (for testing).
    #[cfg(test)]
    pub fn state(&self) -> VisualState {
        self.state.read().expect("lock poisoned").clone()
    }
}

impl ModeKeyResolver for VimVisualResolver {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Escape or Ctrl-C exits visual mode via ExitVisualMode command.
        // The command clears selection, records selection_changed, and sets NORMAL mode.
        if is_escape(key)
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(Modifiers::CTRL))
        {
            self.clear_state();
            return ResolveResult::Execute(crate::ids::EXIT_VISUAL, ResolveContext::new());
        }

        let mut state = self.state.write().expect("lock poisoned");

        // Check for count digit
        if is_count_digit(key, state.has_motion_count()) {
            state.accumulate_motion_count(key);
            return ResolveResult::Pending;
        }

        // Add to pending keys
        state.push_key(*key);
        let keys = state.keys();

        // Look up in visual mode first, then fall back to normal mode for motions
        let lookup_state = {
            let visual_lookup = input.keymap.query(input.mode, &keys);
            if matches!(visual_lookup, reovim_driver_input::KeyLookupState::NotFound) {
                // Motion bindings are in normal mode
                input.keymap.query(&self.parent_mode_id, &keys)
            } else {
                visual_lookup
            }
        };

        match apply_keymap_policy(&lookup_state) {
            KeymapAction::Execute(cmd) => {
                let explicit_count = state.explicit_count();
                let _motion_count = state.take_motion_count();
                state.clear_keys();
                drop(state);

                // Build context with explicit count
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
                // For unknown keys in visual mode, just ignore them (don't exit)
                ResolveResult::NotHandled
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        _mstate: &mut ModeState,
        input: &ResolveInput<'_>,
        _session: &mut dyn SessionApiDyn,
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        tracing::debug!(key = ?key, mode = ?self.mode_id, "visual resolver: resolve_with_session");

        // Escape or Ctrl-C exits visual mode via ExitVisualMode command.
        // The command clears selection, records selection_changed, and sets NORMAL mode.
        if is_escape(key)
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(Modifiers::CTRL))
        {
            tracing::debug!("visual resolver: escape/ctrl-c - executing EXIT_VISUAL command");
            self.clear_state();
            return ResolveResult::Execute(crate::ids::EXIT_VISUAL, ResolveContext::new());
        }

        let mut state = self.state.write().expect("lock poisoned");

        // Initialize state on first key (read any pending count from normal mode)
        if !state.initialized {
            if let Some(vim) = client_extensions.get_mut::<crate::VimSessionState>() {
                // Transfer any pending count from normal mode
                if let Some(count) = vim.pending_count.take() {
                    state.motion_count = Some(count);
                    tracing::debug!(count, "visual resolver: inherited count from normal mode");
                }
            }
            state.initialized = true;
        }

        // Check for count digit
        if is_count_digit(key, state.has_motion_count()) {
            state.accumulate_motion_count(key);
            tracing::debug!(count = ?state.motion_count, "visual resolver: count digit");
            return ResolveResult::Pending;
        }

        // Add to pending keys
        state.push_key(*key);
        let keys = state.keys();

        // Look up in visual mode first, then fall back to normal mode for motions
        let lookup_state = {
            let visual_lookup = input.keymap.query(input.mode, &keys);
            if matches!(visual_lookup, reovim_driver_input::KeyLookupState::NotFound) {
                // Motion bindings are in normal mode
                input.keymap.query(&self.parent_mode_id, &keys)
            } else {
                visual_lookup
            }
        };

        tracing::debug!(?lookup_state, ?keys, "visual resolver: keymap lookup");

        match apply_keymap_policy(&lookup_state) {
            KeymapAction::Execute(cmd) => {
                let explicit_count = state.explicit_count();
                let _motion_count = state.take_motion_count();
                state.clear_keys();
                drop(state);

                tracing::debug!(
                    cmd = %cmd,
                    explicit_count = ?explicit_count,
                    "visual resolver: executing command"
                );

                // Build context with explicit count
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
                tracing::debug!("visual resolver: waiting for more keys");
                ResolveResult::Pending
            }
            KeymapAction::Cancel => {
                state.clear_keys();
                drop(state);
                tracing::debug!("visual resolver: key not found, returning NotHandled");
                // For unknown keys in visual mode, return NotHandled
                // This allows inheritance to try parent modes
                ResolveResult::NotHandled
            }
        }
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

