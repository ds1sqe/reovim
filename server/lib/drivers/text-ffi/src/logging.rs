//! FFI-safe logging functions for external modules.
//!
//! These functions allow external modules written in C (or any FFI-compatible
//! language) to log messages through the kernel's tracing infrastructure.
//!
//! # Thread Safety
//!
//! All logging functions are thread-safe. Multiple threads can log concurrently.
//!
//! # Null Pointer Handling
//!
//! All functions handle null message pointers gracefully by logging a
//! placeholder message. This prevents undefined behavior from FFI callers.
//!
//! # C Usage
//!
//! ```c
//! #include <reovim.h>
//!
//! void my_module_init(void) {
//!     reovim_log_info("Module initialized");
//!     reovim_log_debug("Debug details here");
//! }
//! ```

use std::ffi::CStr;

use libc::c_char;

/// Log an info-level message.
///
/// # Safety
///
/// - `msg` must be a valid null-terminated C string, or null
/// - The string must remain valid for the duration of this call
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_log_info(msg: *const c_char) {
    // Safety: caller guarantees msg is valid C string or null
    let message = unsafe { c_str_to_rust(msg) };
    tracing::info!("{}", message);
}

/// Log a warning-level message.
///
/// # Safety
///
/// - `msg` must be a valid null-terminated C string, or null
/// - The string must remain valid for the duration of this call
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_log_warn(msg: *const c_char) {
    // Safety: caller guarantees msg is valid C string or null
    let message = unsafe { c_str_to_rust(msg) };
    tracing::warn!("{}", message);
}

/// Log an error-level message.
///
/// # Safety
///
/// - `msg` must be a valid null-terminated C string, or null
/// - The string must remain valid for the duration of this call
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_log_error(msg: *const c_char) {
    // Safety: caller guarantees msg is valid C string or null
    let message = unsafe { c_str_to_rust(msg) };
    tracing::error!("{}", message);
}

/// Log a debug-level message.
///
/// # Safety
///
/// - `msg` must be a valid null-terminated C string, or null
/// - The string must remain valid for the duration of this call
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_log_debug(msg: *const c_char) {
    // Safety: caller guarantees msg is valid C string or null
    let message = unsafe { c_str_to_rust(msg) };
    tracing::debug!("{}", message);
}

/// Convert a C string pointer to a Rust string slice.
///
/// Returns a placeholder if the pointer is null or the string is invalid UTF-8.
///
/// # Safety
///
/// The caller must ensure that `ptr` is either null or points to a valid
/// null-terminated C string.
unsafe fn c_str_to_rust(ptr: *const c_char) -> &'static str {
    if ptr.is_null() {
        return "(null message)";
    }

    // Safety: caller guarantees ptr is valid
    // Leak the string to get 'static lifetime (intentional for logging)
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_or("(invalid UTF-8)", |s| Box::leak(s.to_string().into_boxed_str()))
}

#[cfg(test)]
#[path = "logging_tests.rs"]
mod tests;
