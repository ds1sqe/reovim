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

use {
    reovim_domain_text::Position, reovim_driver_input::KeyEvent,
    reovim_driver_session::SessionExtension,
};

// Re-export TextObjRange from session driver for convenience
pub use reovim_driver_session::TextObjRange;

// =============================================================================
// Replace Mode Types (#666)
// =============================================================================

/// Entry in the replace mode restore stack.
///
/// Records the position and original character that was overwritten.
/// Used by backspace to restore the original.
#[derive(Debug, Clone, Copy)]
pub struct ReplaceRestoreEntry {
    /// The position where the replacement occurred.
    pub position: Position,
    /// The original character (`None` if cursor was at EOL — char was inserted).
    pub original: Option<char>,
}

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

    // =========================================================================
    // Dot Repeat Key Recording (#577)
    // =========================================================================
    /// Whether we are currently recording keys for dot repeat.
    ///
    /// Set when an operator or insert-entry command starts, cleared when
    /// the change completes. Independent from macro recording — both can
    /// be active simultaneously.
    pub recording_repeat: bool,

    /// Accumulated key sequence during the current change.
    ///
    /// Cleared when recording starts, saved into `last_change.keys` when
    /// recording finishes. Used by `.` to replay the exact key sequence.
    pub repeat_keys: Vec<KeyEvent>,

    // =========================================================================
    // Replace Mode State (#666)
    // =========================================================================
    /// Restore stack for replace mode backspace.
    ///
    /// Each entry records the position and original character that was
    /// overwritten. Backspace pops from this stack to restore the original.
    /// Cleared on replace mode entry.
    pub replace_restore_stack: Vec<ReplaceRestoreEntry>,
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

    // =========================================================================
    // Dot Repeat Key Recording (#577)
    // =========================================================================

    /// Start recording keys for dot repeat.
    ///
    /// Called when an operator or insert-entry command begins a change.
    /// Clears any previously accumulated keys and sets the recording flag.
    pub fn start_repeat_recording(&mut self) {
        self.recording_repeat = true;
        self.repeat_keys.clear();
    }

    /// Record a key for dot repeat.
    ///
    /// Does nothing if not currently recording for repeat.
    pub fn record_repeat_key(&mut self, key: KeyEvent) {
        if self.recording_repeat {
            self.repeat_keys.push(key);
        }
    }

    /// Finish recording and save keys into `last_change`.
    ///
    /// Copies the accumulated `repeat_keys` into `last_change.keys` and
    /// clears the recording flag. If `last_change` is `None`, does nothing
    /// (the caller should have set `last_change` before calling this).
    pub fn finish_repeat_recording(&mut self) {
        self.recording_repeat = false;
        if let Some(ref mut lc) = self.last_change {
            lc.keys.clone_from(&self.repeat_keys);
        }
        self.repeat_keys.clear();
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
    /// Set mark (m).
    SetMark,
    /// Go to mark line (').
    GotoMarkLine,
    /// Go to mark exact position (`` ` ``).
    GotoMarkExact,
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
        !matches!(self, Self::Replace | Self::SetMark | Self::GotoMarkLine | Self::GotoMarkExact)
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
    /// Recorded key sequence for replay via `InjectKeys` (#577).
    ///
    /// When non-empty, `.` replays these keys instead of using the
    /// metadata-based approach. This handles all cases including
    /// operator+motion, operator+textobj, and change+insert.
    pub keys: Vec<KeyEvent>,
}
