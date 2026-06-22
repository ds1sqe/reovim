//! Log sink bridge from up-face `uapi/log` to lower file helpers.
//!
//! Product/editor code names opaque log sink handles and product-facing errors.
//! This bridge owns the current mapping to host file descriptors and the panic
//! flush registration bridge.

use {
    reovim_kabi_platform::{O_APPEND, O_CLOEXEC, O_CREAT, O_WRONLY},
    reovim_uapi_log::{LogSinkControl, LogSinkError, LogSinkHandle},
    reovim_uapi_panic::{PanicConfigError, PanicFlushTarget},
};

use crate::fs::{File, SysError, close_fd, write_fd};

const DIAGNOSTIC_FD: i32 = 2;

/// Returns the up-face log-sink control table backed by this bridge.
///
/// Composition roots pass this value into editor/kernel launch arguments so
/// the editor kernel does not import raw file helpers or `kabi/panic`.
///
/// ```rust,no_run
/// use reovim_system_kernel::log::log_sink_control;
///
/// let log = log_sink_control();
/// let _ = log.write_diagnostic(b"booting\n");
/// ```
#[must_use]
pub const fn log_sink_control() -> LogSinkControl {
    LogSinkControl::new(
        open_file_sink,
        write_sink,
        close_sink,
        register_panic_flush,
        write_diagnostic,
    )
}

/// Opens the process file-backed log sink.
///
/// # Errors
///
/// Returns [`LogSinkError`] when the lower file open fails.
pub fn open_file_sink(path: &[u8]) -> Result<LogSinkHandle, LogSinkError> {
    let flags = O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC;
    File::open(path, flags, 0o644)
        .map(File::into_raw_fd)
        .map(fd_to_handle)
        .map_err(map_sys_error)
}

/// Writes all bytes to an opened log sink.
///
/// # Errors
///
/// Returns [`LogSinkError`] on the first lower write error or zero-byte stall.
pub fn write_sink(handle: LogSinkHandle, bytes: &[u8]) -> Result<(), LogSinkError> {
    write_all(handle_to_fd(handle)?, bytes)
}

/// Closes an opened log sink.
///
/// # Errors
///
/// Returns [`LogSinkError`] when the lower close fails.
pub fn close_sink(handle: LogSinkHandle) -> Result<(), LogSinkError> {
    close_fd(handle_to_fd(handle)?).map_err(map_sys_error)
}

/// Registers an opened log sink as the panic flush target.
///
/// # Errors
///
/// Returns [`PanicConfigError::AlreadyConfigured`] when another target already
/// won the write-once panic flush slot.
pub fn register_panic_flush(handle: LogSinkHandle) -> Result<(), PanicConfigError> {
    let fd = handle_to_fd(handle).map_err(|_| PanicConfigError::AlreadyConfigured)?;
    crate::panic::set_flush_target(PanicFlushTarget::from_raw_fd(fd))
}

/// Writes all bytes to the diagnostic output path.
///
/// # Errors
///
/// Returns [`LogSinkError`] on the first lower write error or zero-byte stall.
pub fn write_diagnostic(bytes: &[u8]) -> Result<(), LogSinkError> {
    write_all(DIAGNOSTIC_FD, bytes)
}

fn write_all(fd: i32, bytes: &[u8]) -> Result<(), LogSinkError> {
    let mut off = 0;
    while off < bytes.len() {
        match write_fd(fd, &bytes[off..]) {
            Ok(0) => return Err(LogSinkError::new(0)),
            Ok(n) => off += n,
            Err(err) => return Err(map_sys_error(err)),
        }
    }
    Ok(())
}

#[allow(clippy::cast_sign_loss)]
fn fd_to_handle(fd: i32) -> LogSinkHandle {
    LogSinkHandle::new(fd as u64)
}

#[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
fn handle_to_fd(handle: LogSinkHandle) -> Result<i32, LogSinkError> {
    let raw = handle.raw();
    if raw > i32::MAX as u64 {
        Err(LogSinkError::new(0))
    } else {
        Ok(raw as i32)
    }
}

fn map_sys_error(error: SysError) -> LogSinkError {
    LogSinkError::new(error.code())
}
