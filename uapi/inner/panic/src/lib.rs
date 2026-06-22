//! `reovim-uapi-panic` — panic disposition, records, and up-face control types.
//!
//! Pure uapi-tier definitions mirroring the shapes owned by `arch/src/panic.rs`.
//! This crate is a vocabulary layer: no registration storage, no atomics, no
//! platform state. Function tables here let upper code express panic/log policy
//! through a composition-root-supplied implementation without importing the
//! down-face `kabi/panic` atom registry directly.
//!
//! ## Types
//!
//! - [`Disposition`] — what the panic handler does after recording the fault.
//! - [`PanicRecord`] — the record handed to the state-record hook.
//! - [`RingTailProviderFn`] — fn-pointer type for the ring-tail provider hook.
//! - [`StateRecordHookFn`] — fn-pointer type for the state-record hook.
//! - [`PreExitHookFn`] — fn-pointer type for the pre-exit callback.
//! - [`PanicFlushTarget`] — opaque panic flush target key.
//! - [`PanicControl`] — callable up-face panic/log configuration operations.
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

// ---- Up-face panic/log configuration ----------------------------------------

/// Product-facing panic configuration refusal reason.
///
/// The current panic atoms are process-global and write-once. The up-face
/// vocabulary preserves that semantic without exposing the lower atom registry.
///
/// ```rust
/// use reovim_uapi_panic::PanicConfigError;
///
/// assert!(PanicConfigError::AlreadyConfigured.is_already_configured());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanicConfigError {
    /// A process-global panic/log slot has already been configured.
    AlreadyConfigured,
}

impl PanicConfigError {
    /// Returns whether this error means the first configuration already won.
    ///
    /// ```rust
    /// use reovim_uapi_panic::PanicConfigError;
    ///
    /// assert!(PanicConfigError::AlreadyConfigured.is_already_configured());
    /// ```
    #[must_use]
    pub const fn is_already_configured(self) -> bool {
        matches!(self, Self::AlreadyConfigured)
    }
}

/// Opaque panic flush target key.
///
/// The bridge owns interpretation. In the current host runtime this key maps to
/// the opened log sink fd; later system log/fs services can issue the same
/// up-face type without exposing fd lifecycle to editor code.
///
/// ```rust
/// use reovim_uapi_panic::PanicFlushTarget;
///
/// let target = PanicFlushTarget::from_raw_fd(7);
/// assert_eq!(target.raw_fd(), 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PanicFlushTarget(i32);

impl PanicFlushTarget {
    /// Builds a panic flush target from the current bridge's raw fd key.
    ///
    /// ```rust
    /// use reovim_uapi_panic::PanicFlushTarget;
    ///
    /// assert_eq!(PanicFlushTarget::from_raw_fd(2).raw_fd(), 2);
    /// ```
    #[must_use]
    pub const fn from_raw_fd(raw_fd: i32) -> Self {
        Self(raw_fd)
    }

    /// Returns the current bridge raw fd key.
    ///
    /// ```rust
    /// use reovim_uapi_panic::PanicFlushTarget;
    ///
    /// assert_eq!(PanicFlushTarget::from_raw_fd(3).raw_fd(), 3);
    /// ```
    #[must_use]
    pub const fn raw_fd(self) -> i32 {
        self.0
    }
}

/// Function pointer for configuring the panic disposition.
///
/// ```rust
/// use reovim_uapi_panic::{Disposition, PanicConfigError, SetPanicDispositionFn};
///
/// fn set(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
///
/// let f: SetPanicDispositionFn = set;
/// assert_eq!(f(Disposition::Recover), Ok(()));
/// ```
pub type SetPanicDispositionFn = fn(Disposition) -> Result<(), PanicConfigError>;

/// Function pointer for configuring the panic ring-tail provider.
///
/// ```rust
/// use reovim_uapi_panic::{
///     PanicConfigError, RingTailProviderFn, SetPanicRingTailProviderFn,
/// };
///
/// fn tail() -> &'static [u8] { b"" }
/// fn set(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
///
/// let f: SetPanicRingTailProviderFn = set;
/// assert_eq!(f(tail), Ok(()));
/// ```
pub type SetPanicRingTailProviderFn = fn(RingTailProviderFn) -> Result<(), PanicConfigError>;

/// Function pointer for configuring the panic state-record hook.
///
/// ```rust
/// use reovim_uapi_panic::{
///     PanicConfigError, PanicRecord, SetPanicStateRecordHookFn, StateRecordHookFn,
/// };
///
/// fn hook(_: PanicRecord) {}
/// fn set(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
///
/// let f: SetPanicStateRecordHookFn = set;
/// assert_eq!(f(hook), Ok(()));
/// ```
pub type SetPanicStateRecordHookFn = fn(StateRecordHookFn) -> Result<(), PanicConfigError>;

/// Function pointer for configuring the panic flush target.
///
/// ```rust
/// use reovim_uapi_panic::{
///     PanicConfigError, PanicFlushTarget, SetPanicFlushTargetFn,
/// };
///
/// fn set(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
///
/// let f: SetPanicFlushTargetFn = set;
/// assert_eq!(f(PanicFlushTarget::from_raw_fd(2)), Ok(()));
/// ```
pub type SetPanicFlushTargetFn = fn(PanicFlushTarget) -> Result<(), PanicConfigError>;

/// Function pointer for configuring the panic pre-exit hook.
///
/// ```rust
/// use reovim_uapi_panic::{PanicConfigError, PreExitHookFn, SetPanicPreExitHookFn};
///
/// fn restore_terminal() {}
/// fn set(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
///
/// let f: SetPanicPreExitHookFn = set;
/// assert_eq!(f(restore_terminal), Ok(()));
/// ```
pub type SetPanicPreExitHookFn = fn(PreExitHookFn) -> Result<(), PanicConfigError>;

/// Up-face panic/log configuration operations.
///
/// Products use this table without naming `kabi/panic`, arch, platform, or the
/// lower write-once atom storage. A composition root supplies an implementation
/// from the system-kernel bridge.
///
/// ```rust
/// use reovim_uapi_panic::{
///     Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PanicRecord,
///     PreExitHookFn, RingTailProviderFn, StateRecordHookFn,
/// };
///
/// fn set_disposition(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_tail(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_record(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_flush(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_pre_exit(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
/// fn tail() -> &'static [u8] { b"" }
/// fn record(_: PanicRecord) {}
/// fn pre_exit() {}
///
/// let panic = PanicControl::new(
///     set_disposition,
///     set_tail,
///     set_record,
///     set_flush,
///     set_pre_exit,
/// );
/// assert_eq!(panic.set_disposition(Disposition::Halt), Ok(()));
/// assert_eq!(panic.set_ring_tail_provider(tail), Ok(()));
/// assert_eq!(panic.set_state_record_hook(record), Ok(()));
/// assert_eq!(panic.set_flush_target(PanicFlushTarget::from_raw_fd(2)), Ok(()));
/// assert_eq!(panic.set_pre_exit_hook(pre_exit), Ok(()));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PanicControl {
    /// Configures the process panic disposition.
    pub set_disposition_fn: SetPanicDispositionFn,
    /// Configures the panic ring-tail provider.
    pub set_ring_tail_provider_fn: SetPanicRingTailProviderFn,
    /// Configures the panic state-record hook.
    pub set_state_record_hook_fn: SetPanicStateRecordHookFn,
    /// Configures the panic flush target.
    pub set_flush_target_fn: SetPanicFlushTargetFn,
    /// Configures the panic pre-exit hook.
    pub set_pre_exit_hook_fn: SetPanicPreExitHookFn,
}

impl PanicControl {
    /// Builds a panic-control table from function pointers.
    ///
    /// ```rust
    /// use reovim_uapi_panic::{
    ///     Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PanicRecord,
    ///     PreExitHookFn, RingTailProviderFn, StateRecordHookFn,
    /// };
    ///
    /// fn set_disposition(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_tail(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_record(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_flush(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_pre_exit(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    ///
    /// let _panic = PanicControl::new(
    ///     set_disposition,
    ///     set_tail,
    ///     set_record,
    ///     set_flush,
    ///     set_pre_exit,
    /// );
    /// ```
    #[must_use]
    pub const fn new(
        set_disposition_fn: SetPanicDispositionFn,
        set_ring_tail_provider_fn: SetPanicRingTailProviderFn,
        set_state_record_hook_fn: SetPanicStateRecordHookFn,
        set_flush_target_fn: SetPanicFlushTargetFn,
        set_pre_exit_hook_fn: SetPanicPreExitHookFn,
    ) -> Self {
        Self {
            set_disposition_fn,
            set_ring_tail_provider_fn,
            set_state_record_hook_fn,
            set_flush_target_fn,
            set_pre_exit_hook_fn,
        }
    }

    /// Configures the panic disposition.
    ///
    /// ```rust
    /// use reovim_uapi_panic::{
    ///     Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PreExitHookFn,
    ///     RingTailProviderFn, StateRecordHookFn,
    /// };
    ///
    /// fn set_disposition(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_tail(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_record(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_flush(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_pre_exit(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    ///
    /// let panic = PanicControl::new(set_disposition, set_tail, set_record, set_flush, set_pre_exit);
    /// assert_eq!(panic.set_disposition(Disposition::Recover), Ok(()));
    /// ```
    pub fn set_disposition(self, disposition: Disposition) -> Result<(), PanicConfigError> {
        (self.set_disposition_fn)(disposition)
    }

    /// Configures the panic ring-tail provider.
    ///
    /// ```rust
    /// use reovim_uapi_panic::{
    ///     Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PreExitHookFn,
    ///     RingTailProviderFn, StateRecordHookFn,
    /// };
    ///
    /// fn tail() -> &'static [u8] { b"" }
    /// fn set_disposition(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_tail(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_record(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_flush(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_pre_exit(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    ///
    /// let panic = PanicControl::new(set_disposition, set_tail, set_record, set_flush, set_pre_exit);
    /// assert_eq!(panic.set_ring_tail_provider(tail), Ok(()));
    /// ```
    pub fn set_ring_tail_provider(
        self,
        provider: RingTailProviderFn,
    ) -> Result<(), PanicConfigError> {
        (self.set_ring_tail_provider_fn)(provider)
    }

    /// Configures the panic state-record hook.
    ///
    /// ```rust
    /// use reovim_uapi_panic::{
    ///     Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PanicRecord,
    ///     PreExitHookFn, RingTailProviderFn, StateRecordHookFn,
    /// };
    ///
    /// fn record(_: PanicRecord) {}
    /// fn set_disposition(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_tail(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_record(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_flush(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_pre_exit(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    ///
    /// let panic = PanicControl::new(set_disposition, set_tail, set_record, set_flush, set_pre_exit);
    /// assert_eq!(panic.set_state_record_hook(record), Ok(()));
    /// ```
    pub fn set_state_record_hook(self, hook: StateRecordHookFn) -> Result<(), PanicConfigError> {
        (self.set_state_record_hook_fn)(hook)
    }

    /// Configures the panic flush target.
    ///
    /// ```rust
    /// use reovim_uapi_panic::{
    ///     Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PreExitHookFn,
    ///     RingTailProviderFn, StateRecordHookFn,
    /// };
    ///
    /// fn set_disposition(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_tail(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_record(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_flush(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_pre_exit(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    ///
    /// let panic = PanicControl::new(set_disposition, set_tail, set_record, set_flush, set_pre_exit);
    /// assert_eq!(panic.set_flush_target(PanicFlushTarget::from_raw_fd(2)), Ok(()));
    /// ```
    pub fn set_flush_target(self, target: PanicFlushTarget) -> Result<(), PanicConfigError> {
        (self.set_flush_target_fn)(target)
    }

    /// Configures the panic pre-exit hook.
    ///
    /// ```rust
    /// use reovim_uapi_panic::{
    ///     Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PreExitHookFn,
    ///     RingTailProviderFn, StateRecordHookFn,
    /// };
    ///
    /// fn pre_exit() {}
    /// fn set_disposition(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_tail(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_record(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_flush(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn set_pre_exit(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
    ///
    /// let panic = PanicControl::new(set_disposition, set_tail, set_record, set_flush, set_pre_exit);
    /// assert_eq!(panic.set_pre_exit_hook(pre_exit), Ok(()));
    /// ```
    pub fn set_pre_exit_hook(self, hook: PreExitHookFn) -> Result<(), PanicConfigError> {
        (self.set_pre_exit_hook_fn)(hook)
    }
}
