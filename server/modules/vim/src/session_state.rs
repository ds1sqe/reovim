//! Vim-specific per-session state.
//!
//! This module provides [`VimSessionState`], which stores all vim policy state
//! per client session. This state was previously scattered in the runner, but
//! now lives in the vim module where it belongs.

#![allow(clippy::missing_const_for_fn)] // Methods may be extended later
//!
//! # Design
//!
//! The vim module owns all vim-specific state:
//! - Pending motion info (for operator completion)
//! - Pending text object range (for operator completion)
//! - Pending character operations (f, F, t, T, r)
//! - Last find for ; and , repeat
//! - Numeric count prefix
//! - Register selection
//! - Repeat state for . command
//! - Macro recording state (Epic #465 Phase 8D)
//!
//! # Note on Operators (Epic #415)
//!
//! Operator state (d, y, c) is owned by dedicated mode resolvers
//! (`VimDeleteResolver`, `VimYankResolver`, `VimChangeResolver`), not by
//! this session state. Each resolver maintains its own `OperatorState`.
//!
//! # Text Object Range (Epic #465)
//!
//! Text objects (like `iw`, `aw`, `i"`) calculate a range directly instead of
//! moving the cursor. When executed, they store the range in `pending_textobj_range`.
//! The operator resolver's `on_command_complete` consumes this range via `take()`.
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_vim::VimSessionState;
//! use reovim_driver_session::SessionRuntime;
//!
//! fn check_pending_char(runtime: &SessionRuntime) -> bool {
//!     ctx.ext::<VimSessionState>()
//!         .map(|vim| vim.pending_char.is_some())
//!         .unwrap_or(false)
//! }
//! ```

use {reovim_driver_input::KeyEvent, reovim_driver_session::SessionExtension};

// Re-export TextObjRange from session driver for convenience
pub use reovim_driver_session::TextObjRange;

/// Vim-specific per-session state.
///
/// Stored via [`SessionExtension`], accessed by vim resolvers and commands.
/// Each client session has its own independent vim state.
///
/// # Note on Operators (Epic #415)
///
/// Operator state (d, y, c) is owned by dedicated mode resolvers, not here.
/// See `VimDeleteResolver`, `VimYankResolver`, `VimChangeResolver`.
#[derive(Debug, Default)]
pub struct VimSessionState {
    /// Pending motion info (Epic #415, Issue #388).
    ///
    /// When the operator resolver dispatches a motion, it stores the motion type
    /// here. After the motion executes, `on_command_complete` uses this to
    /// complete the operator with the correct linewise flag.
    ///
    /// This replaces `CommandResult::Motion` - motion type is resolver policy,
    /// not command mechanism.
    pub pending_motion: Option<PendingMotion>,

    /// Pending text object range (Epic #465).
    ///
    /// When a text object command executes during operator-pending mode, it
    /// stores the calculated range here. The operator resolver's
    /// `on_command_complete` consumes this range via `take()`.
    ///
    /// Text object range takes priority over motion-based range calculation.
    pub pending_textobj_range: Option<TextObjRange>,

    /// Pending character operation (f, F, t, T, r).
    ///
    /// When a user presses a character-wait command, the operation type
    /// is stored here until the next character is typed.
    pub pending_char: Option<PendingCharOp>,

    /// Numeric prefix accumulator.
    ///
    /// When digits are pressed before a command (e.g., `5j`), the count
    /// is accumulated here.
    pub pending_count: Option<usize>,

    /// Register selection (e.g., `"a` sets register to 'a').
    ///
    /// When a register prefix is typed, the register is stored here
    /// until the next operator/command uses it.
    pub pending_register: Option<char>,

    /// Last change for dot repeat (Epic #465).
    ///
    /// Set by operator resolvers after completing an operation, and by
    /// insert mode exit. Used by the `.` command to replay the last change.
    pub last_change: Option<LastChange>,

    /// Insert mode text accumulator.
    ///
    /// Tracks characters inserted during insert mode. Cleared on insert mode
    /// entry, recorded as `LastChange::Insert` on insert mode exit.
    pub insert_buffer: String,

    // =========================================================================
    // Macro Recording State (Epic #465 Phase 8D)
    // =========================================================================
    /// Currently recording macro to this register (None = not recording).
    ///
    /// When set, all keys processed by the normal resolver are accumulated
    /// in `recording_keys`. Set by `q{a-z}`, cleared by `q` (stop recording).
    pub recording_register: Option<char>,

    /// Accumulated keys during macro recording.
    ///
    /// Cleared when recording starts, stored to register when recording stops.
    /// The final `q` key that stops recording is NOT included.
    pub recording_keys: Vec<KeyEvent>,

    /// Last played macro register (for `@@` repeat).
    ///
    /// Updated each time a macro is played with `@{a-z}`.
    pub last_macro_register: Option<char>,

    /// Current macro playback depth (for recursion detection).
    ///
    /// Incremented when a macro starts playing, decremented when it finishes.
    /// Playback is blocked when depth reaches `MAX_MACRO_DEPTH` (16).
    pub macro_playback_depth: usize,
}

impl SessionExtension for VimSessionState {
    fn create() -> Self {
        Self::default()
    }
}

/// Maximum macro recursion depth to prevent infinite loops.
///
/// Vim uses 1000 by default, but we use 16 for safety. If a macro
/// calls itself (or creates a cycle), this prevents stack overflow.
pub const MAX_MACRO_DEPTH: usize = 16;

impl VimSessionState {
    /// Check if there is any pending state.
    ///
    /// Returns `true` if any vim operation is waiting for input.
    ///
    /// # Note (Epic #415)
    ///
    /// This no longer checks `pending_operator` - the dedicated operator
    /// resolvers (DELETE, YANK, CHANGE) own their state directly.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending_char.is_some()
            || self.pending_count.is_some()
            || self.pending_register.is_some()
    }

    /// Check if currently recording a macro.
    #[must_use]
    pub const fn is_recording(&self) -> bool {
        self.recording_register.is_some()
    }

    /// Check if macro playback would exceed depth limit.
    #[must_use]
    pub const fn is_macro_depth_exceeded(&self) -> bool {
        self.macro_playback_depth >= MAX_MACRO_DEPTH
    }

    /// Clear all pending state.
    ///
    /// Called when an operation is cancelled (e.g., pressing Escape).
    ///
    /// # Note (Epic #415)
    ///
    /// Operator state (d, y, c) is managed by dedicated mode resolvers
    /// (`VimDeleteResolver`, `VimYankResolver`, `VimChangeResolver`),
    /// not by this method.
    ///
    /// # Note (Epic #465)
    ///
    /// `last_change` is NOT cleared here - it persists until the next
    /// change operation so dot repeat can replay it.
    pub fn clear_pending(&mut self) {
        self.pending_motion = None;
        self.pending_textobj_range = None;
        self.pending_char = None;
        self.pending_count = None;
        self.pending_register = None;
        // Note: last_change is NOT cleared - persists for dot repeat
        // Note: insert_buffer is cleared on insert mode entry, not here
    }

    /// Get the effective count, defaulting to 1 if not set.
    ///
    /// Takes the pending count, clearing it in the process.
    pub fn take_count(&mut self) -> usize {
        self.pending_count.take().unwrap_or(1)
    }

    /// Get the register, clearing it in the process.
    ///
    /// Returns `None` if no register was selected (use default register).
    pub fn take_register(&mut self) -> Option<char> {
        self.pending_register.take()
    }

    // =========================================================================
    // Macro Recording Methods (Epic #465 Phase 8D)
    // =========================================================================

    /// Start recording a macro to the given register.
    ///
    /// Clears any previously recorded keys and sets the recording register.
    /// Returns `false` if the register name is invalid (not a-z).
    pub fn start_recording(&mut self, register: char) -> bool {
        if !register.is_ascii_lowercase() {
            return false;
        }
        self.recording_register = Some(register);
        self.recording_keys.clear();
        true
    }

    /// Stop recording and return the recorded keys.
    ///
    /// Returns `None` if not currently recording.
    /// The caller is responsible for storing the keys in the register.
    pub fn stop_recording(&mut self) -> Option<(char, Vec<KeyEvent>)> {
        let register = self.recording_register.take()?;
        let keys = std::mem::take(&mut self.recording_keys);
        Some((register, keys))
    }

    /// Record a key during macro recording.
    ///
    /// Does nothing if not currently recording.
    pub fn record_key(&mut self, key: KeyEvent) {
        if self.is_recording() {
            self.recording_keys.push(key);
        }
    }

    /// Enter macro playback (increment depth counter).
    ///
    /// Returns `false` if depth limit would be exceeded.
    pub fn enter_macro_playback(&mut self) -> bool {
        if self.is_macro_depth_exceeded() {
            return false;
        }
        self.macro_playback_depth += 1;
        true
    }

    /// Exit macro playback (decrement depth counter).
    pub fn exit_macro_playback(&mut self) {
        self.macro_playback_depth = self.macro_playback_depth.saturating_sub(1);
    }
}

/// Pending motion info for operator completion (Epic #415, Issue #388).
///
/// When the operator-pending resolver dispatches a motion, it stores
/// info here. After the motion executes, `on_command_complete` uses
/// this to complete the operator.
///
/// This replaces `CommandResult::Motion` - motion type classification
/// is resolver policy knowledge, not command mechanism.
///
/// # Motion Semantics
///
/// - **Linewise**: The motion affects entire lines (j, k, gg, G)
/// - **Inclusive**: The cursor lands ON the last character of the range ($, e, f, t)
/// - **Exclusive**: The cursor lands at the START of the next unit (w, b, h, l)
/// - **Word Forward**: The motion is word-forward (w, W) - needs special handling for `c`
///
/// For operators, exclusive motions work directly with `Range.end` (which is exclusive),
/// but inclusive motions need +1 adjustment to include the character under the cursor.
#[derive(Debug, Clone, Copy)]
pub struct PendingMotion {
    /// Whether the motion is linewise (j, k, gg, G) or characterwise (w, b, h, l, $).
    pub linewise: bool,
    /// Whether the motion is inclusive (cursor lands ON last char).
    ///
    /// Inclusive motions: $, e, E, f, F, t, T, G, gg, %
    /// Exclusive motions: w, W, b, B, h, l, 0, ^
    ///
    /// For characterwise inclusive motions, the end position must be
    /// adjusted by +1 to convert to exclusive range semantics.
    pub inclusive: bool,
    /// Whether this is a word-forward motion (w, W).
    ///
    /// This is used by the change operator for the `cw` special case:
    /// `cw` behaves like `ce` (change to end of word, not to start of next word).
    /// See `:help cw` in Vim for documentation of this behavior.
    pub word_forward: bool,
}

impl PendingMotion {
    /// Create a new pending motion with explicit flags.
    #[must_use]
    pub const fn new(linewise: bool, inclusive: bool, word_forward: bool) -> Self {
        Self {
            linewise,
            inclusive,
            word_forward,
        }
    }

    /// Create a characterwise exclusive pending motion (w, b, h, l, etc.).
    #[must_use]
    pub const fn characterwise() -> Self {
        Self {
            linewise: false,
            inclusive: false,
            word_forward: false,
        }
    }

    /// Create a characterwise inclusive pending motion ($, e, f, t, etc.).
    #[must_use]
    pub const fn characterwise_inclusive() -> Self {
        Self {
            linewise: false,
            inclusive: true,
            word_forward: false,
        }
    }

    /// Create a linewise pending motion (j, k, gg, G, etc.).
    #[must_use]
    pub const fn linewise() -> Self {
        Self {
            linewise: true,
            inclusive: false,
            word_forward: false,
        }
    }

    /// Create a word-forward motion (w, W) for special cw handling.
    #[must_use]
    pub const fn word_forward() -> Self {
        Self {
            linewise: false,
            inclusive: false,
            word_forward: true,
        }
    }
}

/// Pending character operation (f, F, t, T, r).
///
/// These commands wait for a single character input to complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingCharOp {
    /// Find character forward (f).
    FindForward,
    /// Find character backward (F).
    FindBackward,
    /// Till character forward (t).
    TillForward,
    /// Till character backward (T).
    TillBackward,
    /// Replace character (r).
    Replace,
}

impl PendingCharOp {
    /// Check if this is a find operation (f or F).
    #[must_use]
    pub const fn is_find(&self) -> bool {
        matches!(self, Self::FindForward | Self::FindBackward)
    }

    /// Check if this is a till operation (t or T).
    #[must_use]
    pub const fn is_till(&self) -> bool {
        matches!(self, Self::TillForward | Self::TillBackward)
    }

    /// Check if this is a motion (f, F, t, T) vs replace (r).
    #[must_use]
    pub const fn is_motion(&self) -> bool {
        !matches!(self, Self::Replace)
    }

    /// Check if searching forward.
    #[must_use]
    pub const fn is_forward(&self) -> bool {
        matches!(self, Self::FindForward | Self::TillForward)
    }
}

// =============================================================================
// Dot Repeat Support (Epic #465)
// =============================================================================

/// Type of change recorded for dot repeat.
#[derive(Debug, Clone)]
pub enum ChangeType {
    /// Operator with motion (e.g., `dw`, `c$`).
    OperatorMotion {
        /// The operator type (Delete, Yank, Change).
        operator: OperatorType,
        /// Whether the operation was linewise.
        linewise: bool,
    },
    /// Operator with text object (e.g., `diw`, `ci"`).
    OperatorTextObject {
        /// The operator type.
        operator: OperatorType,
        /// Whether the operation was linewise.
        linewise: bool,
    },
    /// Insert mode text (e.g., `ihello<Esc>`).
    Insert {
        /// The inserted text content.
        text: String,
    },
}

/// Operator type for dot repeat tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorType {
    /// Delete operator (d).
    Delete,
    /// Yank operator (y).
    Yank,
    /// Change operator (c).
    Change,
}

/// Record of the last change for dot repeat (.).
///
/// Stores information needed to replay the last edit operation.
/// This is set by operator resolvers and insert mode exit.
#[derive(Debug, Clone)]
pub struct LastChange {
    /// Type of change that was made.
    pub change_type: ChangeType,
    /// Count used with the original command (None = no explicit count).
    pub count: Option<usize>,
    /// Register used with the original command.
    pub register: Option<char>,
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use reovim_kernel::api::v1::Position;

    use super::*;

    #[test]
    fn test_vim_session_state_default() {
        let state = VimSessionState::default();
        assert!(state.pending_char.is_none());
        assert!(state.pending_textobj_range.is_none());
        assert!(state.pending_count.is_none());
        assert!(state.pending_register.is_none());
        assert!(!state.is_pending());
    }

    #[test]
    fn test_textobj_range_characterwise() {
        let range = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(0, 5));
        assert!(!range.is_linewise);
    }

    #[test]
    fn test_textobj_range_linewise() {
        let range = TextObjRange::linewise(Position::new(1, 0), Position::new(3, 0));
        assert!(range.is_linewise);
    }

    #[test]
    fn test_textobj_range_cleared_on_clear_pending() {
        let mut state = VimSessionState {
            pending_textobj_range: Some(TextObjRange::characterwise(
                Position::new(0, 0),
                Position::new(0, 5),
            )),
            ..Default::default()
        };
        assert!(state.pending_textobj_range.is_some());

        state.clear_pending();
        assert!(state.pending_textobj_range.is_none());
    }

    #[test]
    fn test_vim_session_state_is_pending() {
        let mut state = VimSessionState::default();
        assert!(!state.is_pending());

        state.pending_count = Some(5);
        assert!(state.is_pending());

        state.clear_pending();
        assert!(!state.is_pending());
    }

    #[test]
    fn test_vim_session_state_take_count() {
        let mut state = VimSessionState::default();
        assert_eq!(state.take_count(), 1); // Default

        state.pending_count = Some(5);
        assert_eq!(state.take_count(), 5);
        assert!(state.pending_count.is_none()); // Cleared
    }

    #[test]
    fn test_pending_char_op_variants() {
        assert!(PendingCharOp::FindForward.is_find());
        assert!(PendingCharOp::FindBackward.is_find());
        assert!(!PendingCharOp::TillForward.is_find());

        assert!(PendingCharOp::TillForward.is_till());
        assert!(PendingCharOp::TillBackward.is_till());
        assert!(!PendingCharOp::FindForward.is_till());

        assert!(PendingCharOp::FindForward.is_forward());
        assert!(!PendingCharOp::FindBackward.is_forward());

        assert!(PendingCharOp::FindForward.is_motion());
        assert!(!PendingCharOp::Replace.is_motion());
    }

    // ========================================================================
    // Macro Recording Tests (Epic #465 Phase 8D)
    // ========================================================================

    use reovim_driver_input::KeyCode;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    #[test]
    fn test_macro_default_state() {
        let state = VimSessionState::default();
        assert!(!state.is_recording());
        assert!(state.recording_register.is_none());
        assert!(state.recording_keys.is_empty());
        assert!(state.last_macro_register.is_none());
        assert_eq!(state.macro_playback_depth, 0);
    }

    #[test]
    fn test_start_recording() {
        let mut state = VimSessionState::default();

        // Valid register (a-z)
        assert!(state.start_recording('a'));
        assert!(state.is_recording());
        assert_eq!(state.recording_register, Some('a'));
        assert!(state.recording_keys.is_empty());
    }

    #[test]
    fn test_start_recording_invalid_register() {
        let mut state = VimSessionState::default();

        // Invalid registers
        assert!(!state.start_recording('A')); // uppercase
        assert!(!state.start_recording('1')); // digit
        assert!(!state.start_recording('+')); // special
        assert!(!state.is_recording());
    }

    #[test]
    fn test_record_key() {
        let mut state = VimSessionState::default();
        state.start_recording('a');

        state.record_key(key('d'));
        state.record_key(key('w'));

        assert_eq!(state.recording_keys.len(), 2);
        assert_eq!(state.recording_keys[0].code, KeyCode::Char('d'));
        assert_eq!(state.recording_keys[1].code, KeyCode::Char('w'));
    }

    #[test]
    fn test_record_key_not_recording() {
        let mut state = VimSessionState::default();

        // Should be a no-op when not recording
        state.record_key(key('d'));
        assert!(state.recording_keys.is_empty());
    }

    #[test]
    fn test_stop_recording() {
        let mut state = VimSessionState::default();
        state.start_recording('a');
        state.record_key(key('d'));
        state.record_key(key('w'));

        let result = state.stop_recording();
        assert!(result.is_some());

        let (register, keys) = result.unwrap();
        assert_eq!(register, 'a');
        assert_eq!(keys.len(), 2);

        // State should be cleared
        assert!(!state.is_recording());
        assert!(state.recording_keys.is_empty());
    }

    #[test]
    fn test_stop_recording_not_recording() {
        let mut state = VimSessionState::default();
        let result = state.stop_recording();
        assert!(result.is_none());
    }

    #[test]
    fn test_start_recording_clears_previous() {
        let mut state = VimSessionState::default();
        state.start_recording('a');
        state.record_key(key('x'));

        // Starting new recording should clear previous keys
        state.start_recording('b');
        assert!(state.recording_keys.is_empty());
        assert_eq!(state.recording_register, Some('b'));
    }

    #[test]
    fn test_macro_playback_depth() {
        let mut state = VimSessionState::default();

        // Can enter playback
        assert!(state.enter_macro_playback());
        assert_eq!(state.macro_playback_depth, 1);

        // Can enter multiple levels
        for i in 2..=MAX_MACRO_DEPTH {
            assert!(state.enter_macro_playback());
            assert_eq!(state.macro_playback_depth, i);
        }

        // Cannot exceed depth limit
        assert!(state.is_macro_depth_exceeded());
        assert!(!state.enter_macro_playback());
        assert_eq!(state.macro_playback_depth, MAX_MACRO_DEPTH);
    }

    #[test]
    fn test_exit_macro_playback() {
        let mut state = VimSessionState::default();
        state.enter_macro_playback();
        state.enter_macro_playback();
        assert_eq!(state.macro_playback_depth, 2);

        state.exit_macro_playback();
        assert_eq!(state.macro_playback_depth, 1);

        state.exit_macro_playback();
        assert_eq!(state.macro_playback_depth, 0);

        // Saturating sub - doesn't go negative
        state.exit_macro_playback();
        assert_eq!(state.macro_playback_depth, 0);
    }

    // ========================================================================
    // PendingMotion tests
    // ========================================================================

    #[test]
    fn test_pending_motion_new() {
        let motion = PendingMotion::new(true, false, true);
        assert!(motion.linewise);
        assert!(!motion.inclusive);
        assert!(motion.word_forward);
    }

    #[test]
    fn test_pending_motion_characterwise() {
        let motion = PendingMotion::characterwise();
        assert!(!motion.linewise);
        assert!(!motion.inclusive);
        assert!(!motion.word_forward);
    }

    #[test]
    fn test_pending_motion_characterwise_inclusive() {
        let motion = PendingMotion::characterwise_inclusive();
        assert!(!motion.linewise);
        assert!(motion.inclusive);
        assert!(!motion.word_forward);
    }

    #[test]
    fn test_pending_motion_linewise() {
        let motion = PendingMotion::linewise();
        assert!(motion.linewise);
        assert!(!motion.inclusive);
        assert!(!motion.word_forward);
    }

    #[test]
    fn test_pending_motion_word_forward() {
        let motion = PendingMotion::word_forward();
        assert!(!motion.linewise);
        assert!(!motion.inclusive);
        assert!(motion.word_forward);
    }

    // ========================================================================
    // PendingCharOp additional tests
    // ========================================================================

    #[test]
    fn test_pending_char_op_is_find() {
        assert!(PendingCharOp::FindForward.is_find());
        assert!(PendingCharOp::FindBackward.is_find());
        assert!(!PendingCharOp::TillForward.is_find());
        assert!(!PendingCharOp::TillBackward.is_find());
        assert!(!PendingCharOp::Replace.is_find());
    }

    #[test]
    fn test_pending_char_op_is_till() {
        assert!(!PendingCharOp::FindForward.is_till());
        assert!(!PendingCharOp::FindBackward.is_till());
        assert!(PendingCharOp::TillForward.is_till());
        assert!(PendingCharOp::TillBackward.is_till());
        assert!(!PendingCharOp::Replace.is_till());
    }

    #[test]
    fn test_pending_char_op_is_motion() {
        assert!(PendingCharOp::FindForward.is_motion());
        assert!(PendingCharOp::FindBackward.is_motion());
        assert!(PendingCharOp::TillForward.is_motion());
        assert!(PendingCharOp::TillBackward.is_motion());
        assert!(!PendingCharOp::Replace.is_motion());
    }

    #[test]
    fn test_pending_char_op_is_forward() {
        assert!(PendingCharOp::FindForward.is_forward());
        assert!(PendingCharOp::TillForward.is_forward());
        assert!(!PendingCharOp::FindBackward.is_forward());
        assert!(!PendingCharOp::TillBackward.is_forward());
        assert!(!PendingCharOp::Replace.is_forward());
    }

    #[test]
    fn test_pending_char_op_equality() {
        assert_eq!(PendingCharOp::FindForward, PendingCharOp::FindForward);
        assert_ne!(PendingCharOp::FindForward, PendingCharOp::FindBackward);
    }

    // ========================================================================
    // ChangeType and LastChange tests (Epic #465)
    // ========================================================================

    #[test]
    fn test_change_type_operator_motion() {
        let ct = ChangeType::OperatorMotion {
            operator: OperatorType::Delete,
            linewise: false,
        };
        assert!(matches!(ct, ChangeType::OperatorMotion { .. }));
    }

    #[test]
    fn test_change_type_operator_text_object() {
        let ct = ChangeType::OperatorTextObject {
            operator: OperatorType::Change,
            linewise: true,
        };
        assert!(matches!(ct, ChangeType::OperatorTextObject { .. }));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_change_type_insert() {
        let ct = ChangeType::Insert {
            text: "hello".to_string(),
        };
        match ct {
            ChangeType::Insert { text } => assert_eq!(text, "hello"),
            _ => panic!("Expected Insert variant"),
        }
    }

    #[test]
    fn test_operator_type_variants() {
        assert_eq!(OperatorType::Delete, OperatorType::Delete);
        assert_ne!(OperatorType::Delete, OperatorType::Yank);
        assert_ne!(OperatorType::Yank, OperatorType::Change);
    }

    #[test]
    fn test_last_change_with_all_fields() {
        let lc = LastChange {
            change_type: ChangeType::OperatorMotion {
                operator: OperatorType::Delete,
                linewise: true,
            },
            count: Some(5),
            register: Some('a'),
        };
        assert_eq!(lc.count, Some(5));
        assert_eq!(lc.register, Some('a'));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::redundant_clone)]
    fn test_last_change_clone() {
        let lc = LastChange {
            change_type: ChangeType::Insert {
                text: "test".to_string(),
            },
            count: Some(2),
            register: None,
        };
        let cloned = lc.clone();
        assert_eq!(cloned.count, Some(2));
        match cloned.change_type {
            ChangeType::Insert { text } => assert_eq!(text, "test"),
            _ => panic!("Expected Insert"),
        }
    }

    // ========================================================================
    // VimSessionState additional tests
    // ========================================================================

    #[test]
    fn test_vim_session_state_is_pending_with_pending_char() {
        let mut state = VimSessionState::default();
        state.pending_char = Some(PendingCharOp::FindForward);
        assert!(state.is_pending());
    }

    #[test]
    fn test_vim_session_state_is_pending_with_register() {
        let mut state = VimSessionState::default();
        state.pending_register = Some('a');
        assert!(state.is_pending());
    }

    #[test]
    fn test_vim_session_state_take_register() {
        let mut state = VimSessionState::default();
        assert!(state.take_register().is_none());

        state.pending_register = Some('x');
        assert_eq!(state.take_register(), Some('x'));
        assert!(state.pending_register.is_none());
    }

    #[test]
    fn test_vim_session_state_clear_pending_preserves_last_change() {
        let mut state = VimSessionState::default();
        state.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: "test".to_string(),
            },
            count: None,
            register: None,
        });
        state.pending_count = Some(5);
        state.pending_register = Some('a');
        state.pending_char = Some(PendingCharOp::FindForward);
        state.pending_motion = Some(PendingMotion::characterwise());
        state.pending_textobj_range =
            Some(TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5)));

        state.clear_pending();

        // These should be cleared
        assert!(state.pending_count.is_none());
        assert!(state.pending_register.is_none());
        assert!(state.pending_char.is_none());
        assert!(state.pending_motion.is_none());
        assert!(state.pending_textobj_range.is_none());
        // But last_change should be preserved
        assert!(state.last_change.is_some());
    }

    #[test]
    fn test_vim_session_state_insert_buffer() {
        let mut state = VimSessionState::default();
        assert!(state.insert_buffer.is_empty());

        state.insert_buffer.push('h');
        state.insert_buffer.push('i');
        assert_eq!(state.insert_buffer, "hi");
    }

    #[test]
    fn test_vim_session_state_max_macro_depth() {
        assert_eq!(MAX_MACRO_DEPTH, 16);
    }

    #[test]
    fn test_vim_session_state_is_macro_depth_exceeded_at_zero() {
        let state = VimSessionState::default();
        assert!(!state.is_macro_depth_exceeded());
    }

    #[test]
    fn test_vim_session_state_is_macro_depth_exceeded_at_max() {
        let mut state = VimSessionState::default();
        state.macro_playback_depth = MAX_MACRO_DEPTH;
        assert!(state.is_macro_depth_exceeded());
    }

    #[test]
    fn test_vim_session_state_last_macro_register() {
        let mut state = VimSessionState::default();
        assert!(state.last_macro_register.is_none());

        state.last_macro_register = Some('q');
        assert_eq!(state.last_macro_register, Some('q'));
    }

    #[test]
    fn test_pending_motion_debug() {
        let motion = PendingMotion::characterwise();
        let debug = format!("{motion:?}");
        assert!(debug.contains("PendingMotion"));
    }

    #[test]
    fn test_pending_char_op_debug() {
        let op = PendingCharOp::FindForward;
        let debug = format!("{op:?}");
        assert!(debug.contains("FindForward"));
    }

    #[test]
    fn test_change_type_debug() {
        let ct = ChangeType::Insert {
            text: "test".to_string(),
        };
        let debug = format!("{ct:?}");
        assert!(debug.contains("Insert"));
    }

    #[test]
    fn test_vim_session_state_debug() {
        let state = VimSessionState::default();
        let debug = format!("{state:?}");
        assert!(debug.contains("VimSessionState"));
    }

    #[test]
    fn test_session_extension_create() {
        let state = <VimSessionState as reovim_driver_session::SessionExtension>::create();
        assert!(!state.is_pending());
        assert!(!state.is_recording());
        assert!(!state.is_macro_depth_exceeded());
    }

    // ========================================================================
    // Additional VimSessionState tests
    // ========================================================================

    #[test]
    fn test_vim_session_state_clear_pending_does_not_clear_insert_buffer() {
        let mut state = VimSessionState::default();
        state.insert_buffer.push_str("hello");
        state.pending_count = Some(5);

        state.clear_pending();

        assert!(state.pending_count.is_none());
        // insert_buffer is NOT cleared by clear_pending
        assert_eq!(state.insert_buffer, "hello");
    }

    #[test]
    fn test_vim_session_state_take_count_default_is_one() {
        let mut state = VimSessionState::default();
        // No pending count
        assert_eq!(state.take_count(), 1);
    }

    #[test]
    fn test_vim_session_state_take_count_clears() {
        let mut state = VimSessionState::default();
        state.pending_count = Some(42);
        assert_eq!(state.take_count(), 42);
        assert!(state.pending_count.is_none());
        // Second call should return default
        assert_eq!(state.take_count(), 1);
    }

    #[test]
    fn test_vim_session_state_take_register_clears() {
        let mut state = VimSessionState::default();
        state.pending_register = Some('z');
        assert_eq!(state.take_register(), Some('z'));
        assert!(state.pending_register.is_none());
        assert_eq!(state.take_register(), None);
    }

    #[test]
    fn test_vim_session_state_is_pending_with_all_set() {
        let mut state = VimSessionState::default();
        state.pending_char = Some(PendingCharOp::TillForward);
        state.pending_count = Some(3);
        state.pending_register = Some('a');
        assert!(state.is_pending());
    }

    #[test]
    fn test_vim_session_state_clear_pending_clears_all() {
        let mut state = VimSessionState::default();
        state.pending_char = Some(PendingCharOp::FindForward);
        state.pending_count = Some(5);
        state.pending_register = Some('x');
        state.pending_motion = Some(PendingMotion::linewise());
        state.pending_textobj_range =
            Some(TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5)));

        state.clear_pending();

        assert!(!state.is_pending());
        assert!(state.pending_char.is_none());
        assert!(state.pending_count.is_none());
        assert!(state.pending_register.is_none());
        assert!(state.pending_motion.is_none());
        assert!(state.pending_textobj_range.is_none());
    }

    #[test]
    fn test_vim_session_state_recording_lifecycle() {
        let mut state = VimSessionState::default();

        // Start
        assert!(state.start_recording('a'));
        assert!(state.is_recording());

        // Record keys
        state.record_key(key('d'));
        state.record_key(key('w'));
        assert_eq!(state.recording_keys.len(), 2);

        // Stop
        let result = state.stop_recording();
        assert!(result.is_some());
        let (reg, keys) = result.unwrap();
        assert_eq!(reg, 'a');
        assert_eq!(keys.len(), 2);

        // Verify state is clean
        assert!(!state.is_recording());
        assert!(state.recording_keys.is_empty());
    }

    #[test]
    fn test_vim_session_state_macro_playback_lifecycle() {
        let mut state = VimSessionState::default();

        // Enter
        assert!(state.enter_macro_playback());
        assert_eq!(state.macro_playback_depth, 1);

        // Enter again
        assert!(state.enter_macro_playback());
        assert_eq!(state.macro_playback_depth, 2);

        // Exit
        state.exit_macro_playback();
        assert_eq!(state.macro_playback_depth, 1);

        state.exit_macro_playback();
        assert_eq!(state.macro_playback_depth, 0);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_vim_session_state_start_recording_boundary_chars() {
        let mut state = VimSessionState::default();

        // 'a' is valid
        assert!(state.start_recording('a'));
        state.stop_recording();

        // 'z' is valid
        assert!(state.start_recording('z'));
        state.stop_recording();

        // Just before 'a' is invalid
        assert!(!state.start_recording('`'));

        // Just after 'z' is invalid
        assert!(!state.start_recording('{'));
    }

    #[test]
    fn test_pending_motion_clone() {
        let motion = PendingMotion::characterwise_inclusive();
        let cloned = motion;
        assert_eq!(cloned.inclusive, motion.inclusive);
        assert_eq!(cloned.linewise, motion.linewise);
        assert_eq!(cloned.word_forward, motion.word_forward);
    }

    #[test]
    fn test_pending_motion_all_flags_set() {
        let motion = PendingMotion::new(true, true, true);
        assert!(motion.linewise);
        assert!(motion.inclusive);
        assert!(motion.word_forward);
    }

    #[test]
    fn test_pending_motion_all_flags_unset() {
        let motion = PendingMotion::new(false, false, false);
        assert!(!motion.linewise);
        assert!(!motion.inclusive);
        assert!(!motion.word_forward);
    }

    #[test]
    fn test_pending_char_op_clone_copy() {
        let op = PendingCharOp::TillBackward;
        let copied = op;
        assert_eq!(copied, op);
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    fn test_change_type_clone() {
        let ct = ChangeType::OperatorMotion {
            operator: OperatorType::Yank,
            linewise: true,
        };
        let cloned = ct.clone();
        assert!(matches!(
            cloned,
            ChangeType::OperatorMotion {
                operator: OperatorType::Yank,
                linewise: true,
            }
        ));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::redundant_clone)]
    fn test_change_type_insert_clone() {
        let ct = ChangeType::Insert {
            text: "some text".to_string(),
        };
        let cloned = ct.clone();
        match cloned {
            ChangeType::Insert { text } => assert_eq!(text, "some text"),
            _ => panic!("Expected Insert"),
        }
    }

    #[test]
    fn test_operator_type_debug() {
        let d = format!("{:?}", OperatorType::Delete);
        assert!(d.contains("Delete"));
        let y = format!("{:?}", OperatorType::Yank);
        assert!(y.contains("Yank"));
        let c = format!("{:?}", OperatorType::Change);
        assert!(c.contains("Change"));
    }

    #[test]
    fn test_operator_type_clone_copy() {
        let op = OperatorType::Delete;
        let copied = op;
        assert_eq!(copied, OperatorType::Delete);
    }

    #[test]
    fn test_last_change_no_count_no_register() {
        let lc = LastChange {
            change_type: ChangeType::Insert {
                text: "x".to_string(),
            },
            count: None,
            register: None,
        };
        assert!(lc.count.is_none());
        assert!(lc.register.is_none());
    }

    #[test]
    fn test_last_change_debug() {
        let lc = LastChange {
            change_type: ChangeType::OperatorTextObject {
                operator: OperatorType::Change,
                linewise: false,
            },
            count: Some(1),
            register: Some('a'),
        };
        let debug = format!("{lc:?}");
        assert!(debug.contains("LastChange"));
        assert!(debug.contains("OperatorTextObject"));
    }

    #[test]
    fn test_textobj_range_characterwise_fields() {
        let range = TextObjRange::characterwise(Position::new(1, 5), Position::new(3, 10));
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.column, 5);
        assert_eq!(range.end.line, 3);
        assert_eq!(range.end.column, 10);
        assert!(!range.is_linewise);
    }

    #[test]
    fn test_textobj_range_linewise_fields() {
        let range = TextObjRange::linewise(Position::new(2, 0), Position::new(5, 0));
        assert_eq!(range.start.line, 2);
        assert_eq!(range.end.line, 5);
        assert!(range.is_linewise);
    }
}
