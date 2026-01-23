//! Vim-specific per-session state.
//!
//! This module provides [`VimSessionState`], which stores all vim policy state
//! per client session. This state was previously scattered in the runner, but
//! now lives in the vim module where it belongs.
//!
//! # Design
//!
//! The vim module owns all vim-specific state:
//! - Pending operator (d, y, c waiting for motion)
//! - Pending character operations (f, F, t, T, r)
//! - Last find for ; and , repeat
//! - Numeric count prefix
//! - Register selection
//! - Repeat state for . command
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_vim::VimSessionState;
//! use reovim_driver_session::SessionRuntime;
//!
//! fn check_operator_pending(runtime: &SessionRuntime) -> bool {
//!     ctx.ext::<VimSessionState>()
//!         .map(|vim| vim.pending_operator.is_some())
//!         .unwrap_or(false)
//! }
//! ```

use {reovim_driver_session::SessionExtension, reovim_kernel::api::v1::Position};

use crate::ids::OperatorId;

/// Vim-specific per-session state.
///
/// Stored via [`SessionExtension`], accessed by vim resolvers and commands.
/// Each client session has its own independent vim state.
#[derive(Debug, Default)]
pub struct VimSessionState {
    /// Pending operator waiting for a motion (d, y, c).
    ///
    /// # Deprecated (Epic #415)
    ///
    /// The dedicated operator modes (DELETE, YANK, CHANGE) now own their state
    /// directly in the resolver. This field is only used by the deprecated
    /// `VimOperatorPendingResolver`.
    #[deprecated(
        since = "0.9.0",
        note = "Use dedicated operator modes (DELETE, YANK, CHANGE) instead. Each resolver owns its state."
    )]
    #[allow(deprecated)] // Field uses deprecated type
    pub pending_operator: Option<PendingOperator>,

    /// Pending motion info (Epic #415, Issue #388).
    ///
    /// When the operator resolver dispatches a motion, it stores the motion type
    /// here. After the motion executes, `on_command_complete` uses this to
    /// complete the operator with the correct linewise flag.
    ///
    /// This replaces `CommandResult::Motion` - motion type is resolver policy,
    /// not command mechanism.
    pub pending_motion: Option<PendingMotion>,

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
    /// This no longer clears `pending_operator` - the dedicated operator
    /// resolvers (DELETE, YANK, CHANGE) manage their own state clearing.
    #[allow(deprecated)] // We still clear it for backward compatibility
    pub fn clear_pending(&mut self) {
        self.pending_operator = None;
        self.pending_motion = None;
        self.pending_char = None;
        self.pending_count = None;
        self.pending_register = None;
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

/// Pending operator waiting for a motion.
///
/// When an operator key (d, y, c) is pressed in normal mode, the operator
/// waits for a motion to define the text range. This struct captures the
/// operator and any modifiers while waiting.
///
/// # Deprecated (Epic #415)
///
/// The dedicated operator modes (DELETE, YANK, CHANGE) now own their state
/// via `OperatorState` in their respective resolvers. This struct is only
/// used by the deprecated `VimOperatorPendingResolver`.
#[deprecated(
    since = "0.9.0",
    note = "Use dedicated operator modes. Each resolver owns its state via OperatorState."
)]
#[derive(Debug, Clone)]
pub struct PendingOperator {
    /// The operator being applied.
    ///
    /// Uses strong `OperatorId` type instead of string.
    pub operator_id: OperatorId,

    /// Count applied before the operator (e.g., `2d` in `2dw`).
    ///
    /// Combined with motion count to get total count.
    pub count: Option<usize>,

    /// Target register for the operation.
    ///
    /// If None, uses the default (unnamed) register.
    pub register: Option<char>,

    /// Start position captured when entering operator-pending mode.
    ///
    /// Epic #415: The cursor position when the operator was initiated.
    /// After motion executes, this forms the start of the range.
    pub start_position: Option<Position>,

    /// Motion count (e.g., `3` in `d3w`).
    ///
    /// Epic #415: Separate from operator count for correct multiplication.
    pub motion_count: Option<usize>,
}

#[allow(deprecated)] // Implementing methods on deprecated struct
impl PendingOperator {
    /// Create a new pending operator.
    #[must_use]
    pub const fn new(operator_id: OperatorId) -> Self {
        Self {
            operator_id,
            count: None,
            register: None,
            start_position: None,
            motion_count: None,
        }
    }

    /// Create a pending operator with count.
    #[must_use]
    pub const fn with_count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Create a pending operator with register.
    #[must_use]
    pub const fn with_register(mut self, register: char) -> Self {
        self.register = Some(register);
        self
    }

    /// Set the start position (captured when entering operator-pending mode).
    #[must_use]
    pub const fn with_start_position(mut self, pos: Position) -> Self {
        self.start_position = Some(pos);
        self
    }

    /// Set the motion count.
    #[must_use]
    pub const fn with_motion_count(mut self, count: usize) -> Self {
        self.motion_count = Some(count);
        self
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
#[derive(Debug, Clone, Copy)]
pub struct PendingMotion {
    /// Whether the motion is linewise (j, k, gg, G) or characterwise (w, b, h, l, $).
    pub linewise: bool,
}

impl PendingMotion {
    /// Create a new pending motion.
    #[must_use]
    pub const fn new(linewise: bool) -> Self {
        Self { linewise }
    }

    /// Create a characterwise pending motion (w, b, h, l, $, etc.).
    #[must_use]
    pub const fn characterwise() -> Self {
        Self { linewise: false }
    }

    /// Create a linewise pending motion (j, k, gg, G, etc.).
    #[must_use]
    pub const fn linewise() -> Self {
        Self { linewise: true }
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
#[allow(deprecated)] // Tests for deprecated PendingOperator still need to work
mod tests {
    use {super::*, crate::ids::DELETE};

    fn test_operator() -> OperatorId {
        DELETE.clone()
    }

    #[test]
    #[allow(deprecated)] // Testing deprecated field
    fn test_vim_session_state_default() {
        let state = VimSessionState::default();
        assert!(state.pending_operator.is_none());
        assert!(state.pending_char.is_none());
        assert!(state.last_find.is_none());
        assert!(state.pending_count.is_none());
        assert!(state.pending_register.is_none());
        assert!(!state.is_pending());
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
    fn test_pending_operator_new() {
        let op = PendingOperator::new(test_operator());
        assert!(op.count.is_none());
        assert!(op.register.is_none());
    }

    #[test]
    fn test_pending_operator_with_count() {
        let op = PendingOperator::new(test_operator()).with_count(3);
        assert_eq!(op.count, Some(3));
    }

    #[test]
    fn test_pending_operator_with_register() {
        let op = PendingOperator::new(test_operator()).with_register('a');
        assert_eq!(op.register, Some('a'));
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
