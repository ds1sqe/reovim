//! 6.3 §2.3–2.4, 6.2 §3 — `ErrorCode` and `LogLevel`.
//!
//! `ErrorCode` is the **sole fallible-return type** at the ABI boundary
//! (AB14).  Every fallible vtable slot and `HostApi` function returns it.
//! Bare `c_int` returns are forbidden in v4 vtables.
//!
//! Enum repr convention (6.3 §1 resolved #782): fieldless enums use a
//! primitive-only repr — `#[repr(i32)]` here.

/// ABI error codes (6.3 §2.3, 6.2 §3, AB14).
///
/// `#[repr(i32)]` — the sole fallible-return type at every vtable slot.
/// Values ≥ 240 are reserved for future codes.
///
/// A cdylib returning an `i32` value outside the defined discriminants is
/// treated as [`ErrorCode::Generic`] by the loader (6.2 §6, AB14).
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::error::ErrorCode;
///
/// assert_eq!(ErrorCode::Ok as i32, 0);
/// assert_eq!(ErrorCode::BufferTooSmall as i32, 26);
/// ```
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum ErrorCode {
    /// Success.
    Ok = 0,
    /// Unspecified error; also used for out-of-range discriminants on decode.
    Generic = 1,
    /// ABI major/minor mismatch at load (6.2 §2.1 rules 2–3).
    IncompatibleAbi = 2,
    /// API semantic version mismatch.
    IncompatibleApi = 3,
    /// Requested resource not found.
    NotFound = 4,
    /// Conflicting registration.
    Conflict = 5,
    /// Caller-supplied argument is invalid.
    InvalidArgument = 6,
    /// A resource limit was exceeded.
    ResourceExhausted = 7,
    /// Framing or wire-protocol violation.
    ProtocolViolation = 8,
    /// Resource temporarily unavailable.
    Busy = 9,
    /// Referenced state is stale.
    Stale = 10,
    /// Caller lacks permission.
    PermissionDenied = 11,
    /// A panic occurred in the attributed cdylib (AB12).
    Panic = 12,
    /// Operation was cancelled.
    Cancelled = 13,
    /// Deadline or timeout exceeded.
    Timeout = 14,
    /// Config schema is invalid.
    SchemaInvalid = 15,
    /// Namespace collision during registration.
    NamespaceConflict = 16,
    /// Trust class assignment is illegal.
    IllegalTrustClass = 17,
    /// [`crate::config::ConfigSlice`] total byte cap exceeded (CFG6).
    ConfigSliceTooLarge = 18,
    /// A string failed UTF-8 validation.
    Utf8Invalid = 19,
    /// Illegal project host section.
    IllegalProjectHostSection = 20,
    /// Manifest kind is illegal or mismatched (6.2 §2.1 rule 4).
    IllegalKind = 21,
    /// Vtable `size_of_self` is shorter than the kernel-known minimum (6.2
    /// §2.1 rule 5).
    ShortVtable = 22,
    /// Content codec has been unregistered.
    CodecGone = 23,
    /// Target is not in an active lifecycle state.
    NotActive = 24,
    /// Rollback or undo operation failed.
    RollbackFailed = 25,
    /// Caller-provided buffer is smaller than `encoded_size()` (7.3 SP13).
    ///
    /// When `encode` returns this code, **nothing has been written** to the
    /// buffer.
    BufferTooSmall = 26,
    // Future codes appended here; values >= 240 are reserved.
}

/// Log severity level used by `hostapi_log_emit` and DS12 events (6.3 §2.4).
///
/// `Unknown = 0` is reserved-invalid; it is rejected at emission.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::error::LogLevel;
///
/// assert_eq!(LogLevel::Info as u8, 3);
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Reserved-invalid; rejected at emission.
    Unknown = 0,
    /// Verbose trace output.
    Trace = 1,
    /// Debug output.
    Debug = 2,
    /// Informational.
    Info = 3,
    /// Warning.
    Warn = 4,
    /// Error condition.
    Error = 5,
}
