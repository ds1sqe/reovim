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
use std::panic::AssertUnwindSafe;

use crate::{
    ffi_types::ReovimStringResult,
    runtime::{ffi_catch_unwind, with_runtime},
};

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
    ffi_catch_unwind(AssertUnwindSafe(|| {
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
    }))
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
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_depth.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        match with_runtime(|rt| rt.mode_depth()) {
            Err(e) => e,
            Ok(depth) => {
                #[allow(clippy::cast_possible_truncation)]
                unsafe {
                    *out_depth = depth as u32;
                }
                REOVIM_OK
            }
        }
    }))
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
    ffi_catch_unwind(AssertUnwindSafe(|| {
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
    }))
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
    ffi_catch_unwind(AssertUnwindSafe(|| match with_runtime(|rt| rt.pop_mode(None)) {
        Err(e) => e,
        Ok(Ok(())) => REOVIM_OK,
        Ok(Err(_)) => REOVIM_ERR_FAILED,
    }))
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
    ffi_catch_unwind(AssertUnwindSafe(|| {
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
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "mode_tests.rs"]
mod tests;
