//! `reovim-uapi-terminal` — product-facing terminal and raw-mode vocabulary.
//!
//! This crate is pure `uapi`: it defines the terms upper layers use to ask for
//! terminal mode changes, without exposing POSIX file descriptors, `termios`,
//! down-face `kabi` slots, providers, or arch mechanisms. The system-kernel
//! bridge maps these logical requests to whatever lower mechanism a target
//! actually has.
//!
//! ## Types
//!
//! - [`TerminalId`] — logical terminal endpoint id.
//! - [`RawModeRequest`] — request to enter raw mode for a terminal endpoint.
//! - [`RawModeToken`] — opaque successful raw-mode entry token.
//! - [`TerminalError`] — product-facing terminal refusal reasons.
//! - [`TerminalControl`] — callable up-face raw-mode operations.
#![no_std]

use reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr};

/// Logical terminal endpoint id.
///
/// Values are product-facing endpoint identifiers, not POSIX file descriptors.
/// The well-known ids below name the editor's standard terminal endpoints; any
/// other value is an opaque endpoint assigned by a future session/runtime API.
///
/// ```rust
/// use reovim_uapi_terminal::{PRIMARY_INPUT, TerminalId};
///
/// let target = TerminalId::new(9);
/// assert_eq!(target.raw(), 9);
/// assert_eq!(PRIMARY_INPUT.raw(), 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct TerminalId(u32);

impl TerminalId {
    /// Builds a logical terminal id from its raw product-facing value.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::TerminalId;
    ///
    /// assert_eq!(TerminalId::new(4).raw(), 4);
    /// ```
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw product-facing endpoint value.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::DIAGNOSTIC_OUTPUT;
    ///
    /// assert_eq!(DIAGNOSTIC_OUTPUT.raw(), 2);
    /// ```
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// The primary input terminal endpoint.
///
/// This is the endpoint a TUI asks to place in raw mode. It is intentionally
/// logical; the bridge may map it to fd 0 on POSIX, to a console device on a
/// freestanding target, or to `NotTerminal` where no terminal exists.
///
/// ```rust
/// use reovim_uapi_terminal::PRIMARY_INPUT;
///
/// assert_eq!(PRIMARY_INPUT.raw(), 0);
/// ```
pub const PRIMARY_INPUT: TerminalId = TerminalId::new(0);

/// The primary output terminal endpoint.
///
/// ```rust
/// use reovim_uapi_terminal::PRIMARY_OUTPUT;
///
/// assert_eq!(PRIMARY_OUTPUT.raw(), 1);
/// ```
pub const PRIMARY_OUTPUT: TerminalId = TerminalId::new(1);

/// The diagnostic output terminal endpoint.
///
/// ```rust
/// use reovim_uapi_terminal::DIAGNOSTIC_OUTPUT;
///
/// assert_eq!(DIAGNOSTIC_OUTPUT.raw(), 2);
/// ```
pub const DIAGNOSTIC_OUTPUT: TerminalId = TerminalId::new(2);

/// Request to enter raw mode for a logical terminal endpoint.
///
/// ```rust
/// use reovim_uapi_terminal::{PRIMARY_INPUT, RawModeRequest};
///
/// let request = RawModeRequest::primary_input();
/// assert_eq!(request.terminal, PRIMARY_INPUT);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawModeRequest {
    /// Logical terminal endpoint to place in raw mode.
    pub terminal: TerminalId,
}

impl RawModeRequest {
    /// Builds a raw-mode request for a terminal endpoint.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::{PRIMARY_OUTPUT, RawModeRequest};
    ///
    /// let request = RawModeRequest::new(PRIMARY_OUTPUT);
    /// assert_eq!(request.terminal, PRIMARY_OUTPUT);
    /// ```
    #[must_use]
    pub const fn new(terminal: TerminalId) -> Self {
        Self { terminal }
    }

    /// Builds the usual TUI raw-mode request for the primary input endpoint.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::{PRIMARY_INPUT, RawModeRequest};
    ///
    /// assert_eq!(RawModeRequest::primary_input().terminal, PRIMARY_INPUT);
    /// ```
    #[must_use]
    pub const fn primary_input() -> Self {
        Self::new(PRIMARY_INPUT)
    }
}

/// Opaque token returned after raw mode is entered.
///
/// The bridge owns token interpretation. Product code may keep and pass it back
/// for restore, but must not infer file descriptors, termios snapshots, or
/// provider state from the value.
///
/// ```rust
/// use reovim_uapi_terminal::RawModeToken;
///
/// let token = RawModeToken::new(42);
/// assert_eq!(token.raw(), 42);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RawModeToken(u64);

impl RawModeToken {
    /// Builds an opaque raw-mode token from its bridge-issued value.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::RawModeToken;
    ///
    /// assert_eq!(RawModeToken::new(7).raw(), 7);
    /// ```
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw opaque token value.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::RawModeToken;
    ///
    /// assert_eq!(RawModeToken::new(3).raw(), 3);
    /// ```
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Product-facing terminal refusal reason.
///
/// These errors describe terminal semantics, not POSIX errno values. The bridge
/// maps lower `ENOTTY`, unsupported provider slots, busy restore tables, and
/// invalid target ids into this vocabulary.
///
/// ```rust
/// use reovim_uapi_terminal::TerminalError;
///
/// assert!(TerminalError::NotTerminal.is_absent_terminal());
/// assert!(!TerminalError::Busy.is_absent_terminal());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalError {
    /// The requested endpoint exists but is not a terminal.
    NotTerminal,
    /// This target/build has no terminal raw-mode support.
    Unsupported,
    /// The endpoint id is not known to the bridge.
    InvalidTarget,
    /// Raw mode is already owned by another active token or guard.
    Busy,
    /// The bridge refused access to the endpoint.
    PermissionDenied,
}

impl TerminalError {
    /// Returns whether this error means no usable terminal exists for the
    /// request, allowing callers to continue in pipe/headless mode.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::TerminalError;
    ///
    /// assert!(TerminalError::NotTerminal.is_absent_terminal());
    /// assert!(TerminalError::Unsupported.is_absent_terminal());
    /// assert!(!TerminalError::InvalidTarget.is_absent_terminal());
    /// ```
    #[must_use]
    pub const fn is_absent_terminal(self) -> bool {
        matches!(self, Self::NotTerminal | Self::Unsupported)
    }
}

/// Function pointer for entering raw mode.
///
/// ```rust
/// use reovim_uapi_terminal::{
///     EnterRawModeFn, RawModeRequest, RawModeToken, TerminalError,
/// };
///
/// fn enter(_: RawModeRequest) -> Result<RawModeToken, TerminalError> {
///     Ok(RawModeToken::new(1))
/// }
///
/// let f: EnterRawModeFn = enter;
/// assert_eq!(f(RawModeRequest::primary_input()).unwrap().raw(), 1);
/// ```
pub type EnterRawModeFn = fn(RawModeRequest) -> Result<RawModeToken, TerminalError>;

/// Function pointer for restoring a raw-mode token.
///
/// ```rust
/// use reovim_uapi_terminal::{RawModeToken, RestoreRawModeFn, TerminalError};
///
/// fn restore(_: RawModeToken) -> Result<(), TerminalError> { Ok(()) }
///
/// let f: RestoreRawModeFn = restore;
/// assert_eq!(f(RawModeToken::new(1)), Ok(()));
/// ```
pub type RestoreRawModeFn = fn(RawModeToken) -> Result<(), TerminalError>;

/// Function pointer for panic-time best-effort primary-input restore.
///
/// This form takes no token so it can be registered directly as a pre-exit
/// panic hook.
///
/// ```rust
/// use reovim_uapi_terminal::RestorePrimaryInputRawModeFn;
///
/// fn restore_primary() {}
///
/// let f: RestorePrimaryInputRawModeFn = restore_primary;
/// f();
/// ```
pub type RestorePrimaryInputRawModeFn = fn();

/// Up-face terminal/raw-mode operations.
///
/// Products use this table without naming provider, arch, `termios`, POSIX fd,
/// or down-face `kabi` details. A composition root supplies an implementation
/// from the system-kernel bridge.
///
/// ```rust
/// use reovim_uapi_terminal::{
///     RawModeRequest, RawModeToken, TerminalControl, TerminalError,
/// };
///
/// fn enter(_: RawModeRequest) -> Result<RawModeToken, TerminalError> {
///     Ok(RawModeToken::new(7))
/// }
/// fn restore(_: RawModeToken) -> Result<(), TerminalError> { Ok(()) }
/// fn restore_primary() {}
///
/// let terminal = TerminalControl::new(enter, restore, restore_primary);
/// assert_eq!(
///     terminal.enter_raw_mode(RawModeRequest::primary_input()).unwrap().raw(),
///     7,
/// );
/// ```
#[derive(Clone, Copy)]
pub struct TerminalControl {
    /// Enters raw mode for a logical terminal endpoint.
    pub enter_raw_mode_fn: EnterRawModeFn,
    /// Restores a token returned by [`enter_raw_mode`](Self::enter_raw_mode).
    pub restore_raw_mode_fn: RestoreRawModeFn,
    /// Best-effort restore hook for the primary input terminal.
    pub restore_primary_input_raw_mode_fn: RestorePrimaryInputRawModeFn,
}

impl TerminalControl {
    /// Builds a terminal-control table from function pointers.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::{
    ///     RawModeRequest, RawModeToken, TerminalControl, TerminalError,
    /// };
    ///
    /// fn enter(_: RawModeRequest) -> Result<RawModeToken, TerminalError> {
    ///     Err(TerminalError::Unsupported)
    /// }
    /// fn restore(_: RawModeToken) -> Result<(), TerminalError> { Ok(()) }
    /// fn restore_primary() {}
    ///
    /// let terminal = TerminalControl::new(enter, restore, restore_primary);
    /// assert!(terminal
    ///     .enter_raw_mode(RawModeRequest::primary_input())
    ///     .unwrap_err()
    ///     .is_absent_terminal());
    /// ```
    #[must_use]
    pub const fn new(
        enter_raw_mode_fn: EnterRawModeFn,
        restore_raw_mode_fn: RestoreRawModeFn,
        restore_primary_input_raw_mode_fn: RestorePrimaryInputRawModeFn,
    ) -> Self {
        Self {
            enter_raw_mode_fn,
            restore_raw_mode_fn,
            restore_primary_input_raw_mode_fn,
        }
    }

    /// Enters raw mode using this control table.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::{
    ///     RawModeRequest, RawModeToken, TerminalControl, TerminalError,
    /// };
    ///
    /// fn enter(_: RawModeRequest) -> Result<RawModeToken, TerminalError> {
    ///     Ok(RawModeToken::new(5))
    /// }
    /// fn restore(_: RawModeToken) -> Result<(), TerminalError> { Ok(()) }
    /// fn restore_primary() {}
    ///
    /// let terminal = TerminalControl::new(enter, restore, restore_primary);
    /// assert_eq!(
    ///     terminal.enter_raw_mode(RawModeRequest::primary_input()),
    ///     Ok(RawModeToken::new(5)),
    /// );
    /// ```
    pub fn enter_raw_mode(self, request: RawModeRequest) -> Result<RawModeToken, TerminalError> {
        (self.enter_raw_mode_fn)(request)
    }

    /// Restores raw mode using this control table.
    ///
    /// ```rust
    /// use reovim_uapi_terminal::{
    ///     RawModeRequest, RawModeToken, TerminalControl, TerminalError,
    /// };
    ///
    /// fn enter(_: RawModeRequest) -> Result<RawModeToken, TerminalError> {
    ///     Ok(RawModeToken::new(5))
    /// }
    /// fn restore(token: RawModeToken) -> Result<(), TerminalError> {
    ///     assert_eq!(token.raw(), 5);
    ///     Ok(())
    /// }
    /// fn restore_primary() {}
    ///
    /// let terminal = TerminalControl::new(enter, restore, restore_primary);
    /// assert_eq!(terminal.restore_raw_mode(RawModeToken::new(5)), Ok(()));
    /// ```
    pub fn restore_raw_mode(self, token: RawModeToken) -> Result<(), TerminalError> {
        (self.restore_raw_mode_fn)(token)
    }

    /// Restores primary input raw mode without requiring a token.
    ///
    /// ```rust
    /// use core::sync::atomic::{AtomicBool, Ordering};
    /// use reovim_uapi_terminal::{
    ///     RawModeRequest, RawModeToken, TerminalControl, TerminalError,
    /// };
    ///
    /// static CALLED: AtomicBool = AtomicBool::new(false);
    ///
    /// fn enter(_: RawModeRequest) -> Result<RawModeToken, TerminalError> {
    ///     Err(TerminalError::Unsupported)
    /// }
    /// fn restore(_: RawModeToken) -> Result<(), TerminalError> { Ok(()) }
    /// fn restore_primary() { CALLED.store(true, Ordering::SeqCst); }
    ///
    /// let terminal = TerminalControl::new(enter, restore, restore_primary);
    /// terminal.restore_primary_input_raw_mode();
    /// assert!(CALLED.load(Ordering::SeqCst));
    /// ```
    pub fn restore_primary_input_raw_mode(self) {
        (self.restore_primary_input_raw_mode_fn)();
    }
}

/// Terminal control backed by the raw Reovim syscall transport.
///
/// This adapter keeps terminal semantics in `uapi::terminal`: callers name
/// logical terminal endpoints, while only this lowest wrapper layer packs raw
/// syscall numbers and scalar arguments.
#[derive(Debug, Clone, Copy)]
pub struct SyscallTerminalControl {
    raw: RawSyscall,
}

impl SyscallTerminalControl {
    /// Creates a terminal-control adapter over the raw syscall transport.
    #[must_use]
    pub const fn new(raw: RawSyscall) -> Self {
        Self { raw }
    }

    /// Clears a logical terminal endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError`] when the raw syscall transport reports
    /// failure or the target endpoint cannot be cleared.
    pub fn clear(self, terminal: TerminalId) -> Result<(), TerminalError> {
        self.raw
            .invoke(
                SyscallNr::TERMINAL_CLEAR,
                SyscallArgs::new([terminal.raw() as usize, 0, 0, 0, 0, 0]),
            )
            .decode()
            .map(|_| ())
            .map_err(terminal_error_from_syscall)
    }

    /// Clears the primary output endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError`] when the target endpoint cannot be cleared.
    pub fn clear_primary_output(self) -> Result<(), TerminalError> {
        self.clear(PRIMARY_OUTPUT)
    }

    /// Enters raw mode for a logical terminal endpoint.
    ///
    /// The returned token is opaque product-facing state. Callers pass it back
    /// to [`restore_raw_mode`](Self::restore_raw_mode); they must not infer a
    /// POSIX fd, termios snapshot, or provider detail from it.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError`] when the endpoint is not a terminal, the
    /// target is unknown, or the active kernel cannot provide raw mode.
    pub fn enter_raw_mode(self, request: RawModeRequest) -> Result<RawModeToken, TerminalError> {
        self.raw
            .invoke(
                SyscallNr::TERMINAL_RAW_ENTER,
                SyscallArgs::new([request.terminal.raw() as usize, 0, 0, 0, 0, 0]),
            )
            .decode()
            .map(|token| RawModeToken::new(token as u64))
            .map_err(terminal_error_from_syscall)
    }

    /// Restores raw mode from a token returned by
    /// [`enter_raw_mode`](Self::enter_raw_mode).
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError`] when the token cannot be represented by the
    /// active bridge or the kernel rejects the restore request.
    pub fn restore_raw_mode(self, token: RawModeToken) -> Result<(), TerminalError> {
        let raw_token = usize::try_from(token.raw()).map_err(|_| TerminalError::InvalidTarget)?;
        self.raw
            .invoke(SyscallNr::TERMINAL_RAW_RESTORE, SyscallArgs::new([raw_token, 0, 0, 0, 0, 0]))
            .decode()
            .map(|_| ())
            .map_err(terminal_error_from_syscall)
    }

    /// Best-effort restore for the primary input endpoint.
    ///
    /// This mirrors the panic-hook shape in [`TerminalControl`], but reports
    /// transport failure to syscall-backed callers.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError`] when the active kernel rejects the request.
    pub fn restore_primary_input_raw_mode(self) -> Result<(), TerminalError> {
        self.raw
            .invoke(SyscallNr::TERMINAL_RAW_RESTORE_PRIMARY, SyscallArgs::EMPTY)
            .decode()
            .map(|_| ())
            .map_err(terminal_error_from_syscall)
    }
}

fn terminal_error_from_syscall(error: SyscallError) -> TerminalError {
    match error {
        SyscallError::UNSUPPORTED => TerminalError::Unsupported,
        SyscallError::INVALID_ARGUMENT => TerminalError::InvalidTarget,
        SyscallError::BAD_DESCRIPTOR | SyscallError::NOT_WRITABLE => TerminalError::NotTerminal,
        SyscallError::BUSY => TerminalError::Busy,
        SyscallError::PROTECTED_PROCESS => TerminalError::PermissionDenied,
        _ => TerminalError::Unsupported,
    }
}
