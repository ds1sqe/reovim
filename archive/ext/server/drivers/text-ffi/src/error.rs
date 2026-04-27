//! FFI error code constants.
//!
//! All FFI functions that interact with the session runtime return `i32` status
//! codes. These constants mirror the `REOVIM_*` defines in `reovim.h`.
//!
//! # Convention
//!
//! - `REOVIM_OK` (0): Success
//! - Negative values: Errors
//! - Positive values: Reserved for non-error status (e.g., command results)

/// Operation succeeded.
pub const REOVIM_OK: i32 = 0;

/// Called outside a command callback (no active `SessionRuntime`).
pub const REOVIM_ERR_NO_RUNTIME: i32 = -1;

/// A required pointer argument was null.
pub const REOVIM_ERR_NULL_PTR: i32 = -2;

/// Buffer, window, or mode not found.
pub const REOVIM_ERR_NOT_FOUND: i32 = -3;

/// String is not valid UTF-8.
pub const REOVIM_ERR_INVALID_UTF8: i32 = -4;

/// Line or column index out of range.
pub const REOVIM_ERR_OUT_OF_RANGE: i32 = -5;

/// Cannot delete the last buffer.
pub const REOVIM_ERR_LAST_BUFFER: i32 = -6;

/// Cannot close the last window.
pub const REOVIM_ERR_LAST_WINDOW: i32 = -7;

/// Generic failure.
pub const REOVIM_ERR_FAILED: i32 = -8;

/// A Rust panic was caught at the FFI boundary.
pub const REOVIM_ERR_PANIC: i32 = -9;

/// Called outside module init (no active `ServiceRegistry`).
pub const REOVIM_ERR_NO_INIT_CTX: i32 = -10;

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
