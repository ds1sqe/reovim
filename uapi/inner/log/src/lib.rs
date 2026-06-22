//! `reovim-uapi-log` — product-facing log sink control vocabulary.
//!
//! This crate defines the up-face terms editor code uses to open, write,
//! close, and panic-register a process log sink without importing raw fd
//! helpers, providers, arch, or down-face KABI atoms. A composition root
//! supplies an implementation table from the system-kernel bridge.
#![no_std]

use reovim_uapi_panic::PanicConfigError;

/// Opaque handle for an opened log sink.
///
/// The bridge owns interpretation. The current host bridge maps this key to an
/// fd, but editor code must treat it as an opaque sink handle.
///
/// ```rust
/// use reovim_uapi_log::LogSinkHandle;
///
/// let handle = LogSinkHandle::new(9);
/// assert_eq!(handle.raw(), 9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct LogSinkHandle(u64);

impl LogSinkHandle {
    /// Builds an opaque log sink handle from its bridge-issued value.
    ///
    /// ```rust
    /// use reovim_uapi_log::LogSinkHandle;
    ///
    /// assert_eq!(LogSinkHandle::new(3).raw(), 3);
    /// ```
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the opaque raw handle value.
    ///
    /// ```rust
    /// use reovim_uapi_log::LogSinkHandle;
    ///
    /// assert_eq!(LogSinkHandle::new(4).raw(), 4);
    /// ```
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Product-facing log sink I/O error.
///
/// The numeric code is a bridge/provider error code for diagnostics. It is not
/// a permission to call POSIX directly from editor code.
///
/// ```rust
/// use reovim_uapi_log::LogSinkError;
///
/// let error = LogSinkError::new(2);
/// assert_eq!(error.code(), 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct LogSinkError(i32);

impl LogSinkError {
    /// Builds a log sink error from a bridge/provider code.
    ///
    /// ```rust
    /// use reovim_uapi_log::LogSinkError;
    ///
    /// assert_eq!(LogSinkError::new(-1).code(), -1);
    /// ```
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    /// Returns the bridge/provider error code.
    ///
    /// ```rust
    /// use reovim_uapi_log::LogSinkError;
    ///
    /// assert_eq!(LogSinkError::new(13).code(), 13);
    /// ```
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// Function pointer for opening the process log sink at a NUL-terminated path.
///
/// ```rust
/// use reovim_uapi_log::{LogSinkError, LogSinkHandle, OpenLogSinkFn};
///
/// fn open(_: &[u8]) -> Result<LogSinkHandle, LogSinkError> {
///     Ok(LogSinkHandle::new(1))
/// }
///
/// let f: OpenLogSinkFn = open;
/// assert_eq!(f(b"/tmp/reovim.log\0").unwrap().raw(), 1);
/// ```
pub type OpenLogSinkFn = fn(&[u8]) -> Result<LogSinkHandle, LogSinkError>;

/// Function pointer for writing all bytes to an opened log sink.
///
/// ```rust
/// use reovim_uapi_log::{LogSinkError, LogSinkHandle, WriteLogSinkFn};
///
/// fn write(_: LogSinkHandle, _: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
///
/// let f: WriteLogSinkFn = write;
/// assert_eq!(f(LogSinkHandle::new(1), b"line"), Ok(()));
/// ```
pub type WriteLogSinkFn = fn(LogSinkHandle, &[u8]) -> Result<(), LogSinkError>;

/// Function pointer for closing an opened log sink.
///
/// ```rust
/// use reovim_uapi_log::{CloseLogSinkFn, LogSinkError, LogSinkHandle};
///
/// fn close(_: LogSinkHandle) -> Result<(), LogSinkError> { Ok(()) }
///
/// let f: CloseLogSinkFn = close;
/// assert_eq!(f(LogSinkHandle::new(1)), Ok(()));
/// ```
pub type CloseLogSinkFn = fn(LogSinkHandle) -> Result<(), LogSinkError>;

/// Function pointer for registering a log sink as the panic flush target.
///
/// ```rust
/// use reovim_uapi_log::{LogSinkHandle, RegisterPanicFlushFn};
/// use reovim_uapi_panic::PanicConfigError;
///
/// fn register(_: LogSinkHandle) -> Result<(), PanicConfigError> { Ok(()) }
///
/// let f: RegisterPanicFlushFn = register;
/// assert_eq!(f(LogSinkHandle::new(1)), Ok(()));
/// ```
pub type RegisterPanicFlushFn = fn(LogSinkHandle) -> Result<(), PanicConfigError>;

/// Function pointer for best-effort diagnostic output.
///
/// Used for early stderr-style log echo before a file sink opens and for
/// headless mirror output after it opens.
///
/// ```rust
/// use reovim_uapi_log::{LogSinkError, WriteDiagnosticLogFn};
///
/// fn write(_: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
///
/// let f: WriteDiagnosticLogFn = write;
/// assert_eq!(f(b"line"), Ok(()));
/// ```
pub type WriteDiagnosticLogFn = fn(&[u8]) -> Result<(), LogSinkError>;

/// Up-face log sink operations.
///
/// ```rust
/// use reovim_uapi_log::{
///     LogSinkControl, LogSinkError, LogSinkHandle,
/// };
/// use reovim_uapi_panic::PanicConfigError;
///
/// fn open(_: &[u8]) -> Result<LogSinkHandle, LogSinkError> { Ok(LogSinkHandle::new(7)) }
/// fn write(_: LogSinkHandle, _: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
/// fn close(_: LogSinkHandle) -> Result<(), LogSinkError> { Ok(()) }
/// fn register(_: LogSinkHandle) -> Result<(), PanicConfigError> { Ok(()) }
/// fn diagnostic(_: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
///
/// let log = LogSinkControl::new(open, write, close, register, diagnostic);
/// let handle = log.open_file_sink(b"/tmp/reovim.log\0").unwrap();
/// assert_eq!(handle.raw(), 7);
/// assert_eq!(log.write_sink(handle, b"line"), Ok(()));
/// assert_eq!(log.register_panic_flush(handle), Ok(()));
/// assert_eq!(log.write_diagnostic(b"line"), Ok(()));
/// assert_eq!(log.close_sink(handle), Ok(()));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct LogSinkControl {
    /// Opens the process file-backed log sink.
    pub open_file_sink_fn: OpenLogSinkFn,
    /// Writes all bytes to an opened sink.
    pub write_sink_fn: WriteLogSinkFn,
    /// Closes an opened sink.
    pub close_sink_fn: CloseLogSinkFn,
    /// Registers an opened sink as the panic flush target.
    pub register_panic_flush_fn: RegisterPanicFlushFn,
    /// Writes bytes to the diagnostic output path.
    pub write_diagnostic_fn: WriteDiagnosticLogFn,
}

impl LogSinkControl {
    /// Builds a log-sink control table from function pointers.
    ///
    /// ```rust
    /// use reovim_uapi_log::{LogSinkControl, LogSinkError, LogSinkHandle};
    /// use reovim_uapi_panic::PanicConfigError;
    ///
    /// fn open(_: &[u8]) -> Result<LogSinkHandle, LogSinkError> { Ok(LogSinkHandle::new(1)) }
    /// fn write(_: LogSinkHandle, _: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    /// fn close(_: LogSinkHandle) -> Result<(), LogSinkError> { Ok(()) }
    /// fn register(_: LogSinkHandle) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn diagnostic(_: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    ///
    /// let _log = LogSinkControl::new(open, write, close, register, diagnostic);
    /// ```
    #[must_use]
    pub const fn new(
        open_file_sink_fn: OpenLogSinkFn,
        write_sink_fn: WriteLogSinkFn,
        close_sink_fn: CloseLogSinkFn,
        register_panic_flush_fn: RegisterPanicFlushFn,
        write_diagnostic_fn: WriteDiagnosticLogFn,
    ) -> Self {
        Self {
            open_file_sink_fn,
            write_sink_fn,
            close_sink_fn,
            register_panic_flush_fn,
            write_diagnostic_fn,
        }
    }

    /// Opens the process file-backed log sink.
    ///
    /// ```rust
    /// use reovim_uapi_log::{LogSinkControl, LogSinkError, LogSinkHandle};
    /// use reovim_uapi_panic::PanicConfigError;
    ///
    /// fn open(_: &[u8]) -> Result<LogSinkHandle, LogSinkError> { Ok(LogSinkHandle::new(8)) }
    /// fn write(_: LogSinkHandle, _: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    /// fn close(_: LogSinkHandle) -> Result<(), LogSinkError> { Ok(()) }
    /// fn register(_: LogSinkHandle) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn diagnostic(_: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    ///
    /// let log = LogSinkControl::new(open, write, close, register, diagnostic);
    /// assert_eq!(log.open_file_sink(b"/tmp/reovim.log\0").unwrap().raw(), 8);
    /// ```
    pub fn open_file_sink(self, path: &[u8]) -> Result<LogSinkHandle, LogSinkError> {
        (self.open_file_sink_fn)(path)
    }

    /// Writes all bytes to an opened sink.
    ///
    /// ```rust
    /// use reovim_uapi_log::{LogSinkControl, LogSinkError, LogSinkHandle};
    /// use reovim_uapi_panic::PanicConfigError;
    ///
    /// fn open(_: &[u8]) -> Result<LogSinkHandle, LogSinkError> { Ok(LogSinkHandle::new(1)) }
    /// fn write(_: LogSinkHandle, _: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    /// fn close(_: LogSinkHandle) -> Result<(), LogSinkError> { Ok(()) }
    /// fn register(_: LogSinkHandle) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn diagnostic(_: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    ///
    /// let log = LogSinkControl::new(open, write, close, register, diagnostic);
    /// assert_eq!(log.write_sink(LogSinkHandle::new(1), b"line"), Ok(()));
    /// ```
    pub fn write_sink(self, handle: LogSinkHandle, bytes: &[u8]) -> Result<(), LogSinkError> {
        (self.write_sink_fn)(handle, bytes)
    }

    /// Closes an opened sink.
    ///
    /// ```rust
    /// use reovim_uapi_log::{LogSinkControl, LogSinkError, LogSinkHandle};
    /// use reovim_uapi_panic::PanicConfigError;
    ///
    /// fn open(_: &[u8]) -> Result<LogSinkHandle, LogSinkError> { Ok(LogSinkHandle::new(1)) }
    /// fn write(_: LogSinkHandle, _: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    /// fn close(_: LogSinkHandle) -> Result<(), LogSinkError> { Ok(()) }
    /// fn register(_: LogSinkHandle) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn diagnostic(_: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    ///
    /// let log = LogSinkControl::new(open, write, close, register, diagnostic);
    /// assert_eq!(log.close_sink(LogSinkHandle::new(1)), Ok(()));
    /// ```
    pub fn close_sink(self, handle: LogSinkHandle) -> Result<(), LogSinkError> {
        (self.close_sink_fn)(handle)
    }

    /// Registers an opened sink as the panic flush target.
    ///
    /// ```rust
    /// use reovim_uapi_log::{LogSinkControl, LogSinkError, LogSinkHandle};
    /// use reovim_uapi_panic::PanicConfigError;
    ///
    /// fn open(_: &[u8]) -> Result<LogSinkHandle, LogSinkError> { Ok(LogSinkHandle::new(1)) }
    /// fn write(_: LogSinkHandle, _: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    /// fn close(_: LogSinkHandle) -> Result<(), LogSinkError> { Ok(()) }
    /// fn register(_: LogSinkHandle) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn diagnostic(_: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    ///
    /// let log = LogSinkControl::new(open, write, close, register, diagnostic);
    /// assert_eq!(log.register_panic_flush(LogSinkHandle::new(1)), Ok(()));
    /// ```
    pub fn register_panic_flush(self, handle: LogSinkHandle) -> Result<(), PanicConfigError> {
        (self.register_panic_flush_fn)(handle)
    }

    /// Writes bytes to the diagnostic output path.
    ///
    /// ```rust
    /// use reovim_uapi_log::{LogSinkControl, LogSinkError, LogSinkHandle};
    /// use reovim_uapi_panic::PanicConfigError;
    ///
    /// fn open(_: &[u8]) -> Result<LogSinkHandle, LogSinkError> { Ok(LogSinkHandle::new(1)) }
    /// fn write(_: LogSinkHandle, _: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    /// fn close(_: LogSinkHandle) -> Result<(), LogSinkError> { Ok(()) }
    /// fn register(_: LogSinkHandle) -> Result<(), PanicConfigError> { Ok(()) }
    /// fn diagnostic(_: &[u8]) -> Result<(), LogSinkError> { Ok(()) }
    ///
    /// let log = LogSinkControl::new(open, write, close, register, diagnostic);
    /// assert_eq!(log.write_diagnostic(b"line"), Ok(()));
    /// ```
    pub fn write_diagnostic(self, bytes: &[u8]) -> Result<(), LogSinkError> {
        (self.write_diagnostic_fn)(bytes)
    }
}
