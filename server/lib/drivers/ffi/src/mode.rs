//! Mode API FFI functions.
//!
//! These functions expose [`ModeApi`] methods as C-callable functions,
//! allowing external modules to query and manipulate the mode stack.
//!
//! # Mode ID Format
//!
//! Mode IDs are strings in `"module:name"` format (e.g., `"vim:normal"`,
//! `"vim:insert"`). The module part identifies the owning module, and the
//! name part is the display name.
//!
//! [`ModeApi`]: reovim_driver_session::api::ModeApi

#![allow(unsafe_code)]

use std::ffi::CStr;

use {
    libc::c_char,
    reovim_driver_session::api::ModeApi,
    reovim_kernel::api::v1::{ModeId, ModuleId},
};

#[allow(clippy::wildcard_imports)]
use crate::error::*;
use crate::{ffi_types::ReovimStringResult, runtime::with_runtime};

/// Parse a "module:name" mode ID string into a `ModeId`.
///
/// Uses `Box::leak` to convert owned strings to `&'static str`.
/// This is intentional and acceptable because:
/// - Mode registrations are few and live for the program's lifetime
/// - Same pattern as `ffi-python/src/types.rs`
fn parse_mode_id(s: &str) -> Option<ModeId> {
    let (module_str, name_str) = s.split_once(':')?;
    if module_str.is_empty() || name_str.is_empty() {
        return None;
    }

    let module = ModuleId::from_string(module_str.to_string());
    // SAFETY: intentional leak — mode names live for program lifetime
    let name: &'static str = Box::leak(name_str.to_string().into_boxed_str());
    Some(ModeId::new(module, name))
}

// ============================================================================
// Query functions
// ============================================================================

/// Get the current mode as a string.
///
/// Writes the mode ID string (e.g., `"vim:normal"`) to the caller-owned buffer.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_result` is null
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `buf` must point to at least `buf_len` writable bytes, or be null
/// - `out_result` must be a valid pointer
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_current_mode(
    buf: *mut u8,
    buf_len: u32,
    out_result: *mut ReovimStringResult,
) -> i32 {
    if out_result.is_null() {
        return REOVIM_ERR_NULL_PTR;
    }

    match with_runtime(|rt| rt.current_mode().to_string()) {
        Err(e) => e,
        Ok(mode_str) => {
            crate::buffer::write_string_to_buf(&mode_str, buf, buf_len, out_result);
            REOVIM_OK
        }
    }
}

/// Get the mode stack depth.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_depth` is null
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_depth` must be a valid pointer or null
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_mode_depth(out_depth: *mut u32) -> i32 {
    if out_depth.is_null() {
        return REOVIM_ERR_NULL_PTR;
    }

    match with_runtime(|rt| rt.mode_depth()) {
        Err(e) => e,
        Ok(depth) => {
            #[allow(clippy::cast_possible_truncation)]
            unsafe { *out_depth = depth as u32 };
            REOVIM_OK
        }
    }
}

// ============================================================================
// Mutation functions
// ============================================================================

/// Push a mode onto the mode stack.
///
/// The `mode_id` string must be in `"module:name"` format.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `mode_id` is null
/// - `REOVIM_ERR_INVALID_UTF8` if `mode_id` is not valid UTF-8
/// - `REOVIM_ERR_FAILED` if `mode_id` is not in `"module:name"` format
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `mode_id` must be a valid null-terminated C string or null
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_push_mode(mode_id: *const c_char) -> i32 {
    if mode_id.is_null() {
        return REOVIM_ERR_NULL_PTR;
    }

    let c_str = unsafe { CStr::from_ptr(mode_id) };
    let Ok(mode_str) = c_str.to_str() else {
        return REOVIM_ERR_INVALID_UTF8;
    };

    let Some(mode) = parse_mode_id(mode_str) else {
        return REOVIM_ERR_FAILED;
    };

    match with_runtime(|rt| {
        rt.push_mode(mode.clone(), reovim_driver_session::TransitionContext::new());
    }) {
        Err(e) => e,
        Ok(()) => REOVIM_OK,
    }
}

/// Pop the current mode from the stack.
///
/// Returns to the previous mode. Cannot pop the home mode.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_FAILED` if trying to pop the home mode
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_pop_mode() -> i32 {
    match with_runtime(|rt| rt.pop_mode(None)) {
        Err(e) => e,
        Ok(Ok(())) => REOVIM_OK,
        Ok(Err(_)) => REOVIM_ERR_FAILED,
    }
}

/// Replace the current mode (pop + push atomically).
///
/// The `mode_id` string must be in `"module:name"` format.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `mode_id` is null
/// - `REOVIM_ERR_INVALID_UTF8` if `mode_id` is not valid UTF-8
/// - `REOVIM_ERR_FAILED` if `mode_id` is not in `"module:name"` format
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `mode_id` must be a valid null-terminated C string or null
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_set_mode(mode_id: *const c_char) -> i32 {
    if mode_id.is_null() {
        return REOVIM_ERR_NULL_PTR;
    }

    let c_str = unsafe { CStr::from_ptr(mode_id) };
    let Ok(mode_str) = c_str.to_str() else {
        return REOVIM_ERR_INVALID_UTF8;
    };

    let Some(mode) = parse_mode_id(mode_str) else {
        return REOVIM_ERR_FAILED;
    };

    match with_runtime(|rt| {
        rt.set_mode(mode.clone(), reovim_driver_session::TransitionContext::new());
    }) {
        Err(e) => e,
        Ok(()) => REOVIM_OK,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // parse_mode_id tests
    // ========================================================================

    #[test]
    fn test_parse_mode_id_valid() {
        let mode = parse_mode_id("vim:normal").unwrap();
        assert_eq!(mode.module().as_str(), "vim");
        assert_eq!(mode.name(), "normal");
    }

    #[test]
    fn test_parse_mode_id_no_colon() {
        assert!(parse_mode_id("vimnormal").is_none());
    }

    #[test]
    fn test_parse_mode_id_empty_module() {
        assert!(parse_mode_id(":normal").is_none());
    }

    #[test]
    fn test_parse_mode_id_empty_name() {
        assert!(parse_mode_id("vim:").is_none());
    }

    #[test]
    fn test_parse_mode_id_multiple_colons() {
        // "a:b:c" splits at first colon -> module="a", name="b:c"
        let mode = parse_mode_id("a:b:c").unwrap();
        assert_eq!(mode.module().as_str(), "a");
        assert_eq!(mode.name(), "b:c");
    }

    // ========================================================================
    // No-runtime error tests
    // ========================================================================

    #[test]
    fn test_current_mode_no_runtime() {
        let mut buf = [0u8; 64];
        let mut result = ReovimStringResult::err(0);
        let status = unsafe { reovim_current_mode(buf.as_mut_ptr(), 64, &raw mut result) };
        assert_eq!(status, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_mode_depth_no_runtime() {
        let mut depth: u32 = 0;
        let result = unsafe { reovim_mode_depth(&raw mut depth) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_push_mode_no_runtime() {
        let mode = std::ffi::CString::new("vim:insert").unwrap();
        let result = unsafe { reovim_push_mode(mode.as_ptr()) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_pop_mode_no_runtime() {
        let result = unsafe { reovim_pop_mode() };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_set_mode_no_runtime() {
        let mode = std::ffi::CString::new("vim:visual").unwrap();
        let result = unsafe { reovim_set_mode(mode.as_ptr()) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    // ========================================================================
    // Null pointer tests
    // ========================================================================

    #[test]
    fn test_current_mode_null_result() {
        let mut buf = [0u8; 64];
        let result = unsafe { reovim_current_mode(buf.as_mut_ptr(), 64, std::ptr::null_mut()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_mode_depth_null_out() {
        let result = unsafe { reovim_mode_depth(std::ptr::null_mut()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_push_mode_null() {
        let result = unsafe { reovim_push_mode(std::ptr::null()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_set_mode_null() {
        let result = unsafe { reovim_set_mode(std::ptr::null()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    // ========================================================================
    // Invalid format tests
    // ========================================================================

    #[test]
    fn test_push_mode_invalid_format() {
        let mode = std::ffi::CString::new("nocolon").unwrap();
        let result = unsafe { reovim_push_mode(mode.as_ptr()) };
        assert_eq!(result, REOVIM_ERR_FAILED);
    }

    #[test]
    fn test_set_mode_invalid_format() {
        let mode = std::ffi::CString::new("nocolon").unwrap();
        let result = unsafe { reovim_set_mode(mode.as_ptr()) };
        assert_eq!(result, REOVIM_ERR_FAILED);
    }

    #[test]
    fn test_push_mode_invalid_utf8() {
        let invalid_bytes: &[u8] = &[0x80, 0x00];
        let result = unsafe { reovim_push_mode(invalid_bytes.as_ptr().cast()) };
        assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
    }

    #[test]
    fn test_set_mode_invalid_utf8() {
        let invalid_bytes: &[u8] = &[0x80, 0x00];
        let result = unsafe { reovim_set_mode(invalid_bytes.as_ptr().cast()) };
        assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
    }
}
