//! Clipboard API FFI functions.
//!
//! These functions expose [`ClipboardApi`] methods as C-callable functions,
//! allowing external modules to copy/paste via the system clipboard.
//!
//! All functions require an active [`RuntimeGuard`] (i.e., must be called
//! during a command callback). Returns `REOVIM_ERR_NO_RUNTIME` otherwise.
//!
//! # Graceful Degradation
//!
//! Clipboard operations may fail silently (headless, SSH, Wayland without
//! clipboard manager). Copy functions return `REOVIM_ERR_FAILED` when the
//! clipboard is unavailable. Paste functions return `REOVIM_ERR_NOT_FOUND`
//! when the clipboard is empty or unavailable.
//!
//! [`ClipboardApi`]: reovim_driver_session::api::ClipboardApi
//! [`RuntimeGuard`]: crate::RuntimeGuard

#![allow(unsafe_code)]

use std::ffi::CStr;

use {libc::c_char, reovim_driver_session::api::ClipboardApi};

#[allow(clippy::wildcard_imports)]
use crate::error::*;
use std::panic::AssertUnwindSafe;

use crate::{
    buffer::write_string_to_buf,
    ffi_types::ReovimStringResult,
    runtime::{ffi_catch_unwind, with_runtime},
};

// ============================================================================
// Copy functions
// ============================================================================

/// Copy text to the system clipboard (`+` register).
///
/// # Returns
///
/// - `REOVIM_OK` if copy succeeded
/// - `REOVIM_ERR_NULL_PTR` if `text` is null
/// - `REOVIM_ERR_INVALID_UTF8` if `text` is not valid UTF-8
/// - `REOVIM_ERR_FAILED` if clipboard is unavailable
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `text` must be a valid null-terminated C string or null
/// - Must be called during a command callback (`RuntimeGuard` active)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_copy_to_clipboard(text: *const c_char) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if text.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let c_str = unsafe { CStr::from_ptr(text) };
        let Ok(text_str) = c_str.to_str() else {
            return REOVIM_ERR_INVALID_UTF8;
        };

        match with_runtime(|rt| rt.copy_to_clipboard(text_str)) {
            Err(e) => e,
            Ok(true) => REOVIM_OK,
            Ok(false) => REOVIM_ERR_FAILED,
        }
    }))
}

/// Copy text to the selection clipboard (`*` register, X11 primary selection).
///
/// On X11, this is the primary selection (middle-click paste).
/// On other platforms, this typically mirrors the system clipboard.
///
/// # Returns
///
/// - `REOVIM_OK` if copy succeeded
/// - `REOVIM_ERR_NULL_PTR` if `text` is null
/// - `REOVIM_ERR_INVALID_UTF8` if `text` is not valid UTF-8
/// - `REOVIM_ERR_FAILED` if selection clipboard is unavailable
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `text` must be a valid null-terminated C string or null
/// - Must be called during a command callback (`RuntimeGuard` active)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_copy_to_selection(text: *const c_char) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if text.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let c_str = unsafe { CStr::from_ptr(text) };
        let Ok(text_str) = c_str.to_str() else {
            return REOVIM_ERR_INVALID_UTF8;
        };

        match with_runtime(|rt| rt.copy_to_selection(text_str)) {
            Err(e) => e,
            Ok(true) => REOVIM_OK,
            Ok(false) => REOVIM_ERR_FAILED,
        }
    }))
}

// ============================================================================
// Paste functions
// ============================================================================

/// Paste text from the system clipboard (`+` register).
///
/// Copies the clipboard content into the caller-owned buffer `buf` of size
/// `buf_len`. The actual length is written to `out_result->length`.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_result` is null
/// - `REOVIM_ERR_NOT_FOUND` if clipboard is empty or unavailable
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `buf` must point to at least `buf_len` writable bytes, or be null
/// - `out_result` must be a valid pointer
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_paste_from_clipboard(
    buf: *mut u8,
    buf_len: u32,
    out_result: *mut ReovimStringResult,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_result.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        match with_runtime(|rt| rt.paste_from_clipboard()) {
            Err(e) => e,
            Ok(None) => {
                unsafe {
                    *out_result = ReovimStringResult::err(REOVIM_ERR_NOT_FOUND);
                }
                REOVIM_ERR_NOT_FOUND
            }
            Ok(Some(text)) => {
                write_string_to_buf(&text, buf, buf_len, out_result);
                REOVIM_OK
            }
        }
    }))
}

/// Paste text from the selection clipboard (`*` register, X11 primary selection).
///
/// Copies the selection content into the caller-owned buffer `buf` of size
/// `buf_len`. The actual length is written to `out_result->length`.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_result` is null
/// - `REOVIM_ERR_NOT_FOUND` if selection is empty or unavailable
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `buf` must point to at least `buf_len` writable bytes, or be null
/// - `out_result` must be a valid pointer
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_paste_from_selection(
    buf: *mut u8,
    buf_len: u32,
    out_result: *mut ReovimStringResult,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_result.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        match with_runtime(|rt| rt.paste_from_selection()) {
            Err(e) => e,
            Ok(None) => {
                unsafe {
                    *out_result = ReovimStringResult::err(REOVIM_ERR_NOT_FOUND);
                }
                REOVIM_ERR_NOT_FOUND
            }
            Ok(Some(text)) => {
                write_string_to_buf(&text, buf, buf_len, out_result);
                REOVIM_OK
            }
        }
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Copy function tests
    // ========================================================================

    #[test]
    fn test_copy_to_clipboard_no_runtime() {
        let text = c"hello";
        let result = unsafe { reovim_copy_to_clipboard(text.as_ptr()) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_copy_to_clipboard_null_text() {
        let result = unsafe { reovim_copy_to_clipboard(std::ptr::null()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_copy_to_clipboard_invalid_utf8() {
        let bytes: &[u8] = &[0xFF, 0xFE, 0x00];
        let result = unsafe { reovim_copy_to_clipboard(bytes.as_ptr().cast()) };
        assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
    }

    #[test]
    fn test_copy_to_selection_no_runtime() {
        let text = c"hello";
        let result = unsafe { reovim_copy_to_selection(text.as_ptr()) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_copy_to_selection_null_text() {
        let result = unsafe { reovim_copy_to_selection(std::ptr::null()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_copy_to_selection_invalid_utf8() {
        let bytes: &[u8] = &[0xFF, 0xFE, 0x00];
        let result = unsafe { reovim_copy_to_selection(bytes.as_ptr().cast()) };
        assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
    }

    // ========================================================================
    // Paste function tests
    // ========================================================================

    #[test]
    fn test_paste_from_clipboard_no_runtime() {
        let mut result = ReovimStringResult::ok(0);
        let ret = unsafe { reovim_paste_from_clipboard(std::ptr::null_mut(), 0, &raw mut result) };
        assert_eq!(ret, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_paste_from_clipboard_null_out_result() {
        let ret =
            unsafe { reovim_paste_from_clipboard(std::ptr::null_mut(), 0, std::ptr::null_mut()) };
        assert_eq!(ret, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_paste_from_selection_no_runtime() {
        let mut result = ReovimStringResult::ok(0);
        let ret = unsafe { reovim_paste_from_selection(std::ptr::null_mut(), 0, &raw mut result) };
        assert_eq!(ret, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_paste_from_selection_null_out_result() {
        let ret =
            unsafe { reovim_paste_from_selection(std::ptr::null_mut(), 0, std::ptr::null_mut()) };
        assert_eq!(ret, REOVIM_ERR_NULL_PTR);
    }

    // ========================================================================
    // With-runtime tests (RuntimeGuard + TestSessionRuntime)
    // ========================================================================

    #[test]
    fn test_paste_from_clipboard_empty_with_runtime() {
        use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut result = ReovimStringResult::ok(0);
        let ret = unsafe { reovim_paste_from_clipboard(std::ptr::null_mut(), 0, &raw mut result) };
        // Clipboard is empty in test harness → Ok(None)
        assert_eq!(ret, REOVIM_ERR_NOT_FOUND);
    }

    #[test]
    fn test_paste_from_selection_empty_with_runtime() {
        use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut result = ReovimStringResult::ok(0);
        let ret = unsafe { reovim_paste_from_selection(std::ptr::null_mut(), 0, &raw mut result) };
        assert_eq!(ret, REOVIM_ERR_NOT_FOUND);
    }

    #[test]
    fn test_copy_to_clipboard_with_runtime() {
        use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let text = c"test copy";
        let ret = unsafe { reovim_copy_to_clipboard(text.as_ptr()) };
        // Clipboard may not be available in headless test → Ok(true) or Ok(false)
        assert!(ret == REOVIM_OK || ret == REOVIM_ERR_FAILED);
    }

    #[test]
    fn test_copy_to_selection_with_runtime() {
        use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let text = c"test copy";
        let ret = unsafe { reovim_copy_to_selection(text.as_ptr()) };
        assert!(ret == REOVIM_OK || ret == REOVIM_ERR_FAILED);
    }
}
