//! Typed error kinds for the server runtime (no `&'static str` errors on
//! public APIs — code-style rule).

use core::fmt;

/// Errors that can arise in the server runtime.
///
/// Each variant is a distinct failure kind; callers can match exhaustively.
///
/// # Examples
///
/// ```rust
/// use reovim_server_rt::error::RuntimeError;
///
/// let e = RuntimeError::Io(42);
/// assert!(matches!(e, RuntimeError::Io(_)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeError {
    /// An OS-level I/O error; the inner value is the raw `errno`.
    Io(i32),
    /// A protocol-codec error; the inner value is the raw `ErrorCode` i32.
    Protocol(i32),
    /// An editor-core dispatch error (alloc or missing handler/projector).
    Dispatch,
    /// Allocation failure inside the runtime.
    Alloc,
    /// The listener path is too long or otherwise invalid.
    InvalidPath,
    /// A second `Attach` arrived on a connection that already has one.
    AlreadyAttached,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(errno) => write!(f, "io error (errno {errno})"),
            Self::Protocol(code) => write!(f, "protocol error (code {code})"),
            Self::Dispatch => f.write_str("editor core dispatch failed"),
            Self::Alloc => f.write_str("allocation failed"),
            Self::InvalidPath => f.write_str("listener path invalid"),
            Self::AlreadyAttached => f.write_str("second Attach on one connection (SP1)"),
        }
    }
}
