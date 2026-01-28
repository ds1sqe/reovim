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

use reovim_driver_session::SessionExtension;

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

    /// Last find-char operation for ; and , repeat.
    ///
    /// Updated each time a find-char motion successfully moves the cursor.
    pub last_find: Option<LastFind>,

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
}

impl SessionExtension for VimSessionState {
    fn create() -> Self {
        Self::default()
    }
}

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

/// Record of the last find-char operation for ; and , repeat.
///
/// Stores the character and operation type so that `;` can repeat
/// the same find and `,` can repeat in the opposite direction.
#[derive(Debug, Clone, Copy)]
pub struct LastFind {
    /// The character that was searched for.
    pub char: char,
    /// The type of find operation (f, F, t, or T).
    pub op: PendingCharOp,
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

impl LastFind {
    /// Create a new last-find record.
    #[must_use]
    pub const fn new(char: char, op: PendingCharOp) -> Self {
        Self { char, op }
    }

    /// Get the reversed operation for , (repeat in opposite direction).
    #[must_use]
    pub const fn reversed(&self) -> Self {
        let op = match self.op {
            PendingCharOp::FindForward => PendingCharOp::FindBackward,
            PendingCharOp::FindBackward => PendingCharOp::FindForward,
            PendingCharOp::TillForward => PendingCharOp::TillBackward,
            PendingCharOp::TillBackward => PendingCharOp::TillForward,
            PendingCharOp::Replace => PendingCharOp::Replace, // Replace doesn't reverse
        };
        Self {
            char: self.char,
            op,
        }
    }
}

#[cfg(test)]
mod tests {
    use reovim_kernel::api::v1::Position;

    use super::*;

    #[test]
    fn test_vim_session_state_default() {
        let state = VimSessionState::default();
        assert!(state.pending_char.is_none());
        assert!(state.pending_textobj_range.is_none());
        assert!(state.last_find.is_none());
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

    #[test]
    fn test_last_find_reversed() {
        let find = LastFind::new('x', PendingCharOp::FindForward);
        let reversed = find.reversed();
        assert_eq!(reversed.char, 'x');
        assert_eq!(reversed.op, PendingCharOp::FindBackward);

        let till = LastFind::new('y', PendingCharOp::TillBackward);
        let reversed = till.reversed();
        assert_eq!(reversed.op, PendingCharOp::TillForward);
    }
}
