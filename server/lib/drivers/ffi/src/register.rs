//! Register API FFI functions.
//!
//! These functions expose [`RegisterApi`] methods as C-callable functions,
//! allowing external modules to read and write vim-style registers.
//!
//! All functions require an active [`RuntimeGuard`] (i.e., must be called
//! during a command callback). Returns `REOVIM_ERR_NO_RUNTIME` otherwise.
//!
//! # Register Names
//!
//! Register names are passed as `u8` values:
//! - `0` maps to the unnamed register (`None`)
//! - ASCII characters (`b'a'`, `b'"'`, etc.) map to named registers
//!
//! [`RegisterApi`]: reovim_driver_session::api::RegisterApi
//! [`RuntimeGuard`]: crate::RuntimeGuard

#![allow(unsafe_code)]

use std::ffi::CStr;

use {
    libc::c_char,
    reovim_driver_session::api::{RegisterApi, RegisterContent},
};

#[allow(clippy::wildcard_imports)]
use crate::error::*;
use std::panic::AssertUnwindSafe;

use crate::{
    buffer::write_string_to_buf,
    ffi_types::{ReovimStringResult, ReovimYankType},
    runtime::{ffi_catch_unwind, with_runtime},
};

/// Convert a `u8` register name to `Option<char>`.
///
/// `0` maps to the unnamed register (`None`), any other value is the register
/// character.
const fn register_name_from_ffi(name: u8) -> Option<char> {
    if name == 0 { None } else { Some(name as char) }
}

// ============================================================================
// Register functions
// ============================================================================

/// Get register contents.
///
/// Reads the text content of a register into the caller-owned buffer `buf`.
/// The yank type is written to `out_yank_type` (if non-null).
///
/// # Arguments
///
/// - `name`: Register character (`b'a'` for `"a`, `b'"'` for unnamed, `0` for unnamed alias)
/// - `buf`: Caller-owned buffer for the register text
/// - `buf_len`: Size of `buf` in bytes
/// - `out_result`: Receives string length and status
/// - `out_yank_type`: Receives the yank type (may be null if caller doesn't need it)
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_result` is null
/// - `REOVIM_ERR_NOT_FOUND` if register is empty
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `buf` must point to at least `buf_len` writable bytes, or be null
/// - `out_result` must be a valid pointer
/// - `out_yank_type` must be a valid pointer or null
/// - Must be called during a command callback
#[cfg_attr(coverage_nightly, coverage(off))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_get_register(
    name: u8,
    buf: *mut u8,
    buf_len: u32,
    out_result: *mut ReovimStringResult,
    out_yank_type: *mut ReovimYankType,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_result.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let reg_name = register_name_from_ffi(name);

        match with_runtime(|rt| rt.get_register(reg_name)) {
            Err(e) => e,
            Ok(None) => {
                unsafe {
                    *out_result = ReovimStringResult::err(REOVIM_ERR_NOT_FOUND);
                }
                REOVIM_ERR_NOT_FOUND
            }
            Ok(Some(content)) => {
                write_string_to_buf(&content.text, buf, buf_len, out_result);
                if !out_yank_type.is_null() {
                    unsafe {
                        *out_yank_type = ReovimYankType::from(content.yank_type);
                    }
                }
                REOVIM_OK
            }
        }
    }))
}

/// Set register contents.
///
/// # Arguments
///
/// - `name`: Register character (`b'a'` for `"a`, `0` for unnamed)
/// - `text`: Null-terminated C string to store in the register
/// - `yank_type`: Whether the content is characterwise or linewise
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `text` is null
/// - `REOVIM_ERR_INVALID_UTF8` if `text` is not valid UTF-8
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `text` must be a valid null-terminated C string or null
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_set_register(
    name: u8,
    text: *const c_char,
    yank_type: ReovimYankType,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if text.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let c_str = unsafe { CStr::from_ptr(text) };
        let Ok(text_str) = c_str.to_str() else {
            return REOVIM_ERR_INVALID_UTF8;
        };

        let reg_name = register_name_from_ffi(name);
        let content = RegisterContent::new(text_str, yank_type.into());

        match with_runtime(|rt| rt.set_register(reg_name, content)) {
            Err(e) => e,
            Ok(()) => REOVIM_OK,
        }
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "register_tests.rs"]
mod tests;
