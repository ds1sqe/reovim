//! Terminal/raw-mode bridge from up-face `uapi/terminal` to down-face KABI.
//!
//! Product code names logical terminal endpoints and product-facing errors.
//! This bridge owns the current mapping from logical endpoints to the lower
//! standard stream fds consumed by `kabi/platform` terminal slots. Providers
//! still own all raw `termios` mechanism and saved-state storage.

use {
    reovim_kabi_platform::{ENOTTY, Errno, handle},
    reovim_uapi_terminal::{
        DIAGNOSTIC_OUTPUT, PRIMARY_INPUT, PRIMARY_OUTPUT, RawModeRequest, RawModeToken,
        TerminalControl, TerminalError, TerminalId,
    },
};

pub(crate) const PRIMARY_INPUT_FD: i32 = 0;
pub(crate) const PRIMARY_OUTPUT_FD: i32 = 1;
pub(crate) const DIAGNOSTIC_OUTPUT_FD: i32 = 2;

/// Guard for a successful raw-mode entry through the system-kernel bridge.
///
/// The guard exposes only the opaque up-face [`RawModeToken`]. Drop restores
/// through the lower provider slot; the saved terminal state itself never
/// crosses the bridge.
///
/// ```rust,no_run
/// use reovim_system_kernel::terminal::enter_raw_mode;
/// use reovim_uapi_terminal::RawModeRequest;
///
/// let guard = enter_raw_mode(RawModeRequest::primary_input()).expect("terminal raw mode");
/// let _token = guard.token();
/// ```
pub struct RawModeGuard {
    fd: i32,
    token: RawModeToken,
    restored: bool,
}

impl RawModeGuard {
    /// Returns the opaque token issued by the bridge.
    ///
    /// ```rust,no_run
    /// use reovim_system_kernel::terminal::enter_raw_mode;
    /// use reovim_uapi_terminal::RawModeRequest;
    ///
    /// let guard = enter_raw_mode(RawModeRequest::primary_input()).unwrap();
    /// let _token = guard.token();
    /// ```
    #[must_use]
    pub const fn token(&self) -> RawModeToken {
        self.token
    }

    /// Restores cooked mode before the guard is dropped.
    ///
    /// Calling this explicitly makes the later `Drop` a no-op.
    ///
    /// ```rust,no_run
    /// use reovim_system_kernel::terminal::enter_raw_mode;
    /// use reovim_uapi_terminal::RawModeRequest;
    ///
    /// let guard = enter_raw_mode(RawModeRequest::primary_input()).unwrap();
    /// guard.restore();
    /// ```
    pub fn restore(mut self) {
        self.restore_once();
    }

    fn restore_once(&mut self) {
        if !self.restored {
            handle().term_restore(self.fd);
            self.restored = true;
        }
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        self.restore_once();
    }
}

/// Returns the up-face terminal-control table backed by this bridge.
///
/// Composition roots pass this value into product-facing runtime code that
/// should not import `system/lib/kernel` directly.
///
/// ```rust,no_run
/// use reovim_system_kernel::terminal::terminal_control;
/// use reovim_uapi_terminal::RawModeRequest;
///
/// let terminal = terminal_control();
/// let _ = terminal.enter_raw_mode(RawModeRequest::primary_input());
/// ```
#[must_use]
pub const fn terminal_control() -> TerminalControl {
    TerminalControl::new(enter_raw_mode_token, restore_raw_mode, restore_primary_input_raw_mode)
}

/// Enters raw mode for a logical terminal endpoint.
///
/// The request is up-face vocabulary. The bridge maps known logical endpoints
/// to lower standard-stream fds and maps lower terminal-slot errors back to
/// [`TerminalError`].
///
/// ```rust,no_run
/// use reovim_system_kernel::terminal::enter_raw_mode;
/// use reovim_uapi_terminal::{RawModeRequest, TerminalError};
///
/// match enter_raw_mode(RawModeRequest::primary_input()) {
///     Ok(_guard) => {}
///     Err(e) if e.is_absent_terminal() => {}
///     Err(e) => panic!("terminal setup failed: {e:?}"),
/// }
/// ```
///
/// # Errors
///
/// Returns [`TerminalError::InvalidTarget`] for unknown logical terminal ids,
/// [`TerminalError::NotTerminal`] when the provider reports `ENOTTY`, and
/// [`TerminalError::PermissionDenied`] for other lower terminal refusals.
pub fn enter_raw_mode(request: RawModeRequest) -> Result<RawModeGuard, TerminalError> {
    let fd = terminal_fd(request.terminal)?;
    let token = handle()
        .term_set_raw(fd)
        .map_err(map_terminal_error)
        .map(raw_token)?;
    Ok(RawModeGuard {
        fd,
        token,
        restored: false,
    })
}

fn enter_raw_mode_token(request: RawModeRequest) -> Result<RawModeToken, TerminalError> {
    let guard = enter_raw_mode(request)?;
    let token = guard.token();
    core::mem::forget(guard);
    Ok(token)
}

/// Restores cooked mode by opaque raw-mode token.
///
/// This is the explicit restore entry for callers that stored the token rather
/// than the guard. Normal scoped callers should prefer [`RawModeGuard`].
///
/// ```rust,no_run
/// use reovim_system_kernel::terminal::{enter_raw_mode, restore_raw_mode};
/// use reovim_uapi_terminal::RawModeRequest;
///
/// let guard = enter_raw_mode(RawModeRequest::primary_input()).unwrap();
/// let token = guard.token();
/// restore_raw_mode(token).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`TerminalError::InvalidTarget`] when the token cannot be represented
/// by the bridge's current lower restore key.
pub fn restore_raw_mode(token: RawModeToken) -> Result<(), TerminalError> {
    let fd = token_to_fd(token)?;
    handle().term_restore(fd);
    Ok(())
}

/// Best-effort restore for the primary input endpoint.
///
/// This is the token-free function registered as the TUI pre-exit panic hook.
///
/// ```rust,no_run
/// use reovim_system_kernel::terminal::restore_primary_input_raw_mode;
///
/// restore_primary_input_raw_mode();
/// ```
pub fn restore_primary_input_raw_mode() {
    handle().term_restore(PRIMARY_INPUT_FD);
}

pub(crate) fn terminal_fd(terminal: TerminalId) -> Result<i32, TerminalError> {
    if terminal == PRIMARY_INPUT {
        Ok(PRIMARY_INPUT_FD)
    } else if terminal == PRIMARY_OUTPUT {
        Ok(PRIMARY_OUTPUT_FD)
    } else if terminal == DIAGNOSTIC_OUTPUT {
        Ok(DIAGNOSTIC_OUTPUT_FD)
    } else {
        Err(TerminalError::InvalidTarget)
    }
}

pub(crate) fn map_terminal_error(err: Errno) -> TerminalError {
    if err == ENOTTY {
        TerminalError::NotTerminal
    } else {
        TerminalError::PermissionDenied
    }
}

#[allow(clippy::cast_possible_truncation)]
fn raw_token(raw: usize) -> RawModeToken {
    RawModeToken::new(raw as u64)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub(crate) fn token_to_fd(token: RawModeToken) -> Result<i32, TerminalError> {
    let raw = token.raw();
    if raw > i32::MAX as u64 {
        Err(TerminalError::InvalidTarget)
    } else {
        Ok(raw as i32)
    }
}
