//! `reovim-uapi-panic` — panic disposition and record types.
//!
//! Pure uapi-tier definitions mirroring the shapes owned by `arch/src/panic.rs`.
//! This crate is a vocabulary layer: no registration machinery, no atomics,
//! no platform state. The shapes here match exactly so Phase 3 can re-home
//! the canonical definitions from `arch` into `uapi/panic` without changing
//! any call sites that name the types.
//!
//! ## Types
//!
//! - [`Disposition`] — what the panic handler does after recording the fault.
//! - [`PanicRecord`] — the record handed to the state-record hook.
//! - [`RingTailProviderFn`] — fn-pointer type for the ring-tail provider hook.
//! - [`StateRecordHookFn`] — fn-pointer type for the state-record hook.
//! - [`PreExitHookFn`] — fn-pointer type for the pre-exit callback.
//!
//! ## Source of truth
//!
//! Shapes mirrored from `arch/src/panic.rs`. The type aliases for hook
//! fn-pointers are mirrored from the private type aliases in that module
//! (lines 226-230): `RingTailProvider`, `StateRecordHook`, `PreExitHook`.
#![no_std]

// ---- Disposition -------------------------------------------------------------

/// What the panic handler does after recording the fault (6.2 §5, §5.1).
///
/// The two dispositions exit with fixed `sysexits` status codes that the
/// supervising launcher treats as a contract: exactly `75` is
/// restart-requested; any other non-zero exit is a no-restart fault.
///
/// ```rust
/// use reovim_uapi_panic::Disposition;
///
/// assert_eq!(Disposition::Recover.exit_code(), 75);
/// assert_eq!(Disposition::Halt.exit_code(), 70);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// `EX_TEMPFAIL` (75): transient failure — restart requested. The
    /// supervisor restarts and quarantines the attributed owner.
    Recover,
    /// `EX_SOFTWARE` (70): internal software error — stop for analysis. The
    /// flushed log and process state are preserved (dev/test posture).
    Halt,
}

impl Disposition {
    /// The normative process exit code for this disposition (6.2 §5.1).
    ///
    /// ```rust
    /// use reovim_uapi_panic::Disposition;
    ///
    /// // EX_TEMPFAIL (75) signals a restart-requested transient failure.
    /// assert_eq!(Disposition::Recover.exit_code(), 75);
    /// // EX_SOFTWARE (70) signals a halt-for-analysis internal error.
    /// assert_eq!(Disposition::Halt.exit_code(), 70);
    /// ```
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            // EX_TEMPFAIL: the launcher treats exactly 75 as restart-requested.
            Self::Recover => 75,
            // EX_SOFTWARE: any non-75 non-zero exit is a no-restart fault.
            Self::Halt => 70,
        }
    }
}

// ---- PanicRecord -------------------------------------------------------------

/// The record handed to the state-record hook (6.2 §5 step 3, AB13).
///
/// Arch fills the disposition and the AB13 cleanup marker; the persistence
/// consumer (a later phase) maps it onto lifecycle + quarantine state. The
/// shape is intentionally minimal — no kernel DS is referenced here.
///
/// ```rust
/// use reovim_uapi_panic::{Disposition, PanicRecord};
///
/// let r = PanicRecord { disposition: Disposition::Halt, rollback_failed: false };
/// assert_eq!(r.disposition.exit_code(), 70);
/// assert!(!r.rollback_failed);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanicRecord {
    /// The disposition the handler will terminate under.
    pub disposition: Disposition,
    /// Whether the panic happened in a cleanup context (AB13). When `true`
    /// the flushed line carries `rollback = failed` and the persisted state
    /// records `TombstonedFailedUnload`.
    pub rollback_failed: bool,
}

// ---- Hook fn-pointer type aliases -------------------------------------------
// Mirrored from the private type aliases in arch/src/panic.rs (lines 226-230).
// Placing them here allows upper layers to name hook types without depending
// on arch.

/// The signature of a ring-tail provider.
///
/// Returns one contiguous pre-rendered LOG2 byte slice the panic handler writes
/// verbatim ahead of the panic line (9.5 §9.1). Arch writes the tail; it never
/// parses ring entries.
///
/// ```rust
/// use reovim_uapi_panic::RingTailProviderFn;
///
/// fn empty_tail() -> &'static [u8] { b"" }
///
/// // The type alias is a bare fn pointer — coercible from a fn item.
/// let _p: RingTailProviderFn = empty_tail;
/// ```
pub type RingTailProviderFn = fn() -> &'static [u8];

/// The signature of a state-record hook: called with the [`PanicRecord`] so
/// the persistence consumer can record lifecycle + quarantine state before
/// the process terminates.
///
/// ```rust
/// use reovim_uapi_panic::{PanicRecord, StateRecordHookFn};
///
/// fn noop_hook(_: PanicRecord) {}
///
/// let _h: StateRecordHookFn = noop_hook;
/// ```
pub type StateRecordHookFn = fn(PanicRecord);

/// The signature of the pre-exit callback.
///
/// Invoked by the panic handler BEFORE rendering its output (gap-7, 8.2 §2).
/// A platform runtime registers a terminal/console-restore function here so the
/// panic line is written in cooked mode.
///
/// ```rust
/// use reovim_uapi_panic::PreExitHookFn;
///
/// fn noop_pre_exit() {}
///
/// let _h: PreExitHookFn = noop_pre_exit;
/// ```
pub type PreExitHookFn = fn();
