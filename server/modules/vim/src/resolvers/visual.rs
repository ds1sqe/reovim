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
        ExtensionMap, KeyCode, KeyEvent, KeySequence, ModeKeyResolver, ModeState, ModeTransition,
        Modifiers, ResolveContext, ResolveInput, ResolveResult, SessionApiDyn, TransitionContext,
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
struct VisualState {
    /// Pending count for motions (e.g., `3j` in visual mode).
    motion_count: Option<usize>,
    /// Accumulated key sequence for multi-key motions.
    pending_keys: KeySequence,
    /// Whether the resolver has been initialized for this mode entry.
    initialized: bool,
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
    const fn has_motion_count(&self) -> bool {
        self.motion_count.is_some()
    }

    /// Accumulate a count digit.
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
    fn state(&self) -> VisualState {
        self.state.read().expect("lock poisoned").clone()
    }
}

impl ModeKeyResolver for VimVisualResolver {
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Escape or Ctrl-C exits visual mode
        if is_escape(key)
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(Modifiers::CTRL))
        {
            self.clear_state();
            return ResolveResult::ModeTransition(ModeTransition::Set {
                mode: VimMode::NORMAL_ID,
                context: TransitionContext::new(),
            });
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

    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        _mstate: &mut ModeState,
        input: &ResolveInput<'_>,
        _session: &mut dyn SessionApiDyn,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        tracing::debug!(key = ?key, mode = ?self.mode_id, "visual resolver: resolve_with_session");

        // Escape or Ctrl-C exits visual mode
        if is_escape(key)
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(Modifiers::CTRL))
        {
            tracing::debug!("visual resolver: escape/ctrl-c - exiting visual mode");
            self.clear_state();
            return ResolveResult::ModeTransition(ModeTransition::Set {
                mode: VimMode::NORMAL_ID,
                context: TransitionContext::new(),
            });
        }

        let mut state = self.state.write().expect("lock poisoned");

        // Initialize state on first key (read any pending count from normal mode)
        if !state.initialized {
            if let Some(vim) = extensions.get_mut::<crate::VimSessionState>() {
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

#[cfg(test)]
mod tests {
    use {
        reovim_driver_input::{KeyCode, KeyLookupState, KeySequence, KeymapQuery},
        reovim_kernel::api::v1::{CommandId, ModuleId},
    };

    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn test_state() -> ModeState {
        ModeState::new(VimMode::VISUAL_ID)
    }

    struct MockKeymap {
        response: KeyLookupState,
    }

    impl MockKeymap {
        fn exact_only(cmd: &'static str) -> Self {
            Self {
                response: KeyLookupState::ExactOnly(CommandId::new(ModuleId::new("test"), cmd)),
            }
        }

        fn not_found() -> Self {
            Self {
                response: KeyLookupState::NotFound,
            }
        }

        fn prefix_only() -> Self {
            Self {
                response: KeyLookupState::PrefixOnly,
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
        static MODE: ModeId = VimMode::VISUAL_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    // =============================================================================
    // Mode Identity Tests
    // =============================================================================

    #[test]
    fn test_mode_id_for_visual() {
        let resolver = VimVisualResolver::character_wise();
        assert_eq!(resolver.mode_id(), &VimMode::VISUAL_ID);
    }

    #[test]
    fn test_mode_id_for_visual_line() {
        let resolver = VimVisualResolver::line_wise();
        assert_eq!(resolver.mode_id(), &VimMode::VISUAL_LINE_ID);
    }

    #[test]
    fn test_mode_id_for_visual_block() {
        let resolver = VimVisualResolver::block_wise();
        assert_eq!(resolver.mode_id(), &VimMode::VISUAL_BLOCK_ID);
    }

    #[test]
    fn test_inherits_from_normal() {
        let resolver = VimVisualResolver::character_wise();
        assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
    }

    // =============================================================================
    // Escape Handling Tests
    // =============================================================================

    #[test]
    fn test_escape_exits_visual_mode() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

        if let ResolveResult::ModeTransition(ModeTransition::Set { mode, .. }) = result {
            assert_eq!(mode, VimMode::NORMAL_ID);
        } else {
            panic!("expected Set to Normal, got {result:?}");
        }
    }

    #[test]
    fn test_ctrl_c_exits_visual_mode() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(
            &KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL),
            &mut state,
            &input,
        );

        if let ResolveResult::ModeTransition(ModeTransition::Set { mode, .. }) = result {
            assert_eq!(mode, VimMode::NORMAL_ID);
        } else {
            panic!("expected Set to Normal, got {result:?}");
        }
    }

    #[test]
    fn test_escape_clears_pending_state() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        // Build up some state
        resolver.resolve_with_keymap(&key('3'), &mut state, &input);
        assert!(resolver.state().has_motion_count());

        // Escape should clear it
        resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        assert!(!resolver.state().has_motion_count());
    }

    // =============================================================================
    // Count Accumulation Tests
    // =============================================================================

    #[test]
    fn test_count_digit_accumulates() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.state().motion_count, Some(3));

        let result = resolver.resolve_with_keymap(&key('5'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.state().motion_count, Some(35));
    }

    #[test]
    fn test_first_zero_not_count_digit() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("line-start");
        let input = resolve_input(&keymap);

        // First 0 is not a count, it's a motion
        let result = resolver.resolve_with_keymap(&key('0'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Execute(..)));
    }

    #[test]
    fn test_subsequent_zero_is_count_digit() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        // Start a count
        resolver.resolve_with_keymap(&key('3'), &mut state, &input);
        // Now 0 is a count digit
        let result = resolver.resolve_with_keymap(&key('0'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert_eq!(resolver.state().motion_count, Some(30));
    }

    #[test]
    fn test_count_flows_to_motion() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("cursor-down");
        let input = resolve_input(&keymap);

        // Enter count
        resolver.resolve_with_keymap(&key('3'), &mut state, &input);

        // Execute motion
        let result = resolver.resolve_with_keymap(&key('j'), &mut state, &input);
        if let ResolveResult::Execute(_cmd, ctx) = result {
            assert_eq!(ctx.count, Some(3));
        } else {
            panic!("expected Execute, got {result:?}");
        }
    }

    // =============================================================================
    // Motion Delegation Tests
    // =============================================================================

    #[test]
    fn test_motion_h_executes() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("cursor-left");
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('h'), &mut state, &input);
        if let ResolveResult::Execute(cmd, _) = result {
            assert_eq!(cmd.name(), "cursor-left");
        } else {
            panic!("expected Execute, got {result:?}");
        }
    }

    #[test]
    fn test_motion_j_executes() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("cursor-down");
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('j'), &mut state, &input);
        if let ResolveResult::Execute(cmd, _) = result {
            assert_eq!(cmd.name(), "cursor-down");
        } else {
            panic!("expected Execute, got {result:?}");
        }
    }

    #[test]
    fn test_motion_w_executes() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::exact_only("word-forward");
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);
        if let ResolveResult::Execute(cmd, _) = result {
            assert_eq!(cmd.name(), "word-forward");
        } else {
            panic!("expected Execute, got {result:?}");
        }
    }

    #[test]
    fn test_motion_gg_multi_key() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();

        // First g should be pending
        let keymap_prefix = MockKeymap::prefix_only();
        let input_prefix = resolve_input(&keymap_prefix);
        let result = resolver.resolve_with_keymap(&key('g'), &mut state, &input_prefix);
        assert!(matches!(result, ResolveResult::Pending));

        // Second g should execute
        let keymap_exact = MockKeymap::exact_only("document-start");
        let input_exact = resolve_input(&keymap_exact);
        let result = resolver.resolve_with_keymap(&key('g'), &mut state, &input_exact);
        if let ResolveResult::Execute(cmd, _) = result {
            assert_eq!(cmd.name(), "document-start");
        } else {
            panic!("expected Execute, got {result:?}");
        }
    }

    #[test]
    fn test_unknown_key_returns_not_handled() {
        let resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    // =============================================================================
    // State Reset Tests
    // =============================================================================

    #[test]
    fn test_reset_clears_state() {
        let mut resolver = VimVisualResolver::character_wise();
        let mut state = test_state();
        let keymap = MockKeymap::not_found();
        let input = resolve_input(&keymap);

        // Build up state
        resolver.resolve_with_keymap(&key('3'), &mut state, &input);
        assert!(resolver.state().has_motion_count());

        // Reset
        resolver.reset();
        assert!(!resolver.state().has_motion_count());
        assert!(!resolver.state().initialized);
    }
}
