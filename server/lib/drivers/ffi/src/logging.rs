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
mod tests {
    use super::*;

    // ========================================================================
    // Internal helper tests
    // ========================================================================

    #[test]
    fn test_c_str_null_handling() {
        let result = unsafe { c_str_to_rust(std::ptr::null()) };
        assert_eq!(result, "(null message)");
    }

    #[test]
    fn test_c_str_valid() {
        let s = std::ffi::CString::new("hello").unwrap();
        let result = unsafe { c_str_to_rust(s.as_ptr()) };
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_c_str_empty() {
        let s = std::ffi::CString::new("").unwrap();
        let result = unsafe { c_str_to_rust(s.as_ptr()) };
        assert_eq!(result, "");
    }

    #[test]
    fn test_c_str_invalid_utf8() {
        // Create a byte sequence with invalid UTF-8
        let invalid_bytes: &[u8] = &[0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x80, 0x00]; // "Hello\x80\0"
        let result = unsafe { c_str_to_rust(invalid_bytes.as_ptr().cast()) };
        assert_eq!(result, "(invalid UTF-8)");
    }

    // ========================================================================
    // Exported FFI function tests
    // ========================================================================

    #[test]
    fn test_reovim_log_info_valid_string() {
        // Should not panic with valid string
        let msg = std::ffi::CString::new("test info message").unwrap();
        unsafe { reovim_log_info(msg.as_ptr()) };
    }

    #[test]
    fn test_reovim_log_warn_valid_string() {
        // Should not panic with valid string
        let msg = std::ffi::CString::new("test warning message").unwrap();
        unsafe { reovim_log_warn(msg.as_ptr()) };
    }

    #[test]
    fn test_reovim_log_error_valid_string() {
        // Should not panic with valid string
        let msg = std::ffi::CString::new("test error message").unwrap();
        unsafe { reovim_log_error(msg.as_ptr()) };
    }

    #[test]
    fn test_reovim_log_debug_valid_string() {
        // Should not panic with valid string
        let msg = std::ffi::CString::new("test debug message").unwrap();
        unsafe { reovim_log_debug(msg.as_ptr()) };
    }

    #[test]
    fn test_reovim_log_info_null_pointer() {
        // Should not panic with null pointer
        unsafe { reovim_log_info(std::ptr::null()) };
    }

    #[test]
    fn test_reovim_log_warn_null_pointer() {
        // Should not panic with null pointer
        unsafe { reovim_log_warn(std::ptr::null()) };
    }

    #[test]
    fn test_reovim_log_error_null_pointer() {
        // Should not panic with null pointer
        unsafe { reovim_log_error(std::ptr::null()) };
    }

    #[test]
    fn test_reovim_log_debug_null_pointer() {
        // Should not panic with null pointer
        unsafe { reovim_log_debug(std::ptr::null()) };
    }

    #[test]
    fn test_reovim_log_info_invalid_utf8() {
        // Should not panic with invalid UTF-8 (graceful degradation)
        let invalid_bytes: &[u8] = &[0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x80, 0x00];
        unsafe { reovim_log_info(invalid_bytes.as_ptr().cast()) };
    }

    #[test]
    fn test_reovim_log_info_empty_string() {
        // Should not panic with empty string
        let msg = std::ffi::CString::new("").unwrap();
        unsafe { reovim_log_info(msg.as_ptr()) };
    }
}
