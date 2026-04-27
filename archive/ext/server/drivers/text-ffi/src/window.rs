//! Window and cursor API FFI functions.
//!
//! These functions expose [`WindowApi`] methods as C-callable functions,
//! allowing external modules to manage windows and query cursor position.
//!
//! All functions require an active [`RuntimeGuard`] (i.e., must be called
//! during a command callback). Returns `REOVIM_ERR_NO_RUNTIME` otherwise.
//!
//! [`WindowApi`]: reovim_driver_text_session::api::WindowApi
//! [`RuntimeGuard`]: crate::RuntimeGuard

#![allow(unsafe_code)]

use {
    reovim_driver_text_session::api::WindowApi,
    reovim_kernel::api::v1::{BufferId, WindowId},
};

#[allow(clippy::wildcard_imports)]
use crate::error::*;
use std::panic::AssertUnwindSafe;

use crate::{
    buffer::ReovimBufferId,
    ffi_types::ReovimPosition,
    runtime::{ffi_catch_unwind, with_runtime},
};

/// Opaque window ID for FFI. Maps to kernel `WindowId(usize)`.
pub type ReovimWindowId = u64;

/// Convert a kernel `WindowId` to the FFI representation.
#[allow(clippy::cast_possible_truncation)]
const fn window_id_to_ffi(id: WindowId) -> ReovimWindowId {
    id.as_usize() as ReovimWindowId
}

/// Convert an FFI window ID to a kernel `WindowId`.
#[allow(clippy::cast_possible_truncation)]
const fn window_id_from_ffi(id: ReovimWindowId) -> WindowId {
    WindowId::from_raw(id as usize)
}

// ============================================================================
// Query functions
// ============================================================================

/// Get the active window ID.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_id` is null
/// - `REOVIM_ERR_NOT_FOUND` if no active window
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_id` must be a valid pointer or null
/// - Must be called during a command callback
#[cfg_attr(coverage_nightly, coverage(off))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_active_window(out_id: *mut ReovimWindowId) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_id.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        match with_runtime(|rt| rt.active_window()) {
            Err(e) => e,
            Ok(None) => REOVIM_ERR_NOT_FOUND,
            Ok(Some(id)) => {
                unsafe { *out_id = window_id_to_ffi(id) };
                REOVIM_OK
            }
        }
    }))
}

/// Get the cursor position from the active window.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_pos` is null
/// - `REOVIM_ERR_NOT_FOUND` if no active window
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_pos` must be a valid pointer or null
/// - Must be called during a command callback
#[cfg_attr(coverage_nightly, coverage(off))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_cursor_position(out_pos: *mut ReovimPosition) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_pos.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        match with_runtime(|rt| rt.cursor_position()) {
            Err(e) => e,
            Ok(None) => REOVIM_ERR_NOT_FOUND,
            Ok(Some(pos)) => {
                unsafe { *out_pos = ReovimPosition::from(pos) };
                REOVIM_OK
            }
        }
    }))
}

/// Get the number of windows.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_count` is null
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_count` must be a valid pointer or null
/// - Must be called during a command callback
#[cfg_attr(coverage_nightly, coverage(off))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_window_count(out_count: *mut u32) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_count.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        match with_runtime(|rt| rt.window_count()) {
            Err(e) => e,
            Ok(count) => {
                #[allow(clippy::cast_possible_truncation)]
                unsafe {
                    *out_count = count as u32;
                }
                REOVIM_OK
            }
        }
    }))
}

/// Get the buffer displayed in a window.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_buffer` is null
/// - `REOVIM_ERR_NOT_FOUND` if window doesn't exist or has no buffer
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_buffer` must be a valid pointer or null
/// - Must be called during a command callback
#[cfg_attr(coverage_nightly, coverage(off))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_window_buffer(
    window_id: ReovimWindowId,
    out_buffer: *mut ReovimBufferId,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_buffer.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let wid = window_id_from_ffi(window_id);

        match with_runtime(|rt| rt.window_buffer(wid)) {
            Err(e) => e,
            Ok(None) => REOVIM_ERR_NOT_FOUND,
            Ok(Some(bid)) => {
                unsafe { *out_buffer = bid.as_usize() as ReovimBufferId };
                REOVIM_OK
            }
        }
    }))
}

// ============================================================================
// Mutation functions
// ============================================================================

/// Create a new window.
///
/// If `buffer_id` is 0, the window starts without a buffer.
///
/// # Returns
///
/// - `REOVIM_OK` on success (window ID written to `out_id`)
/// - `REOVIM_ERR_NULL_PTR` if `out_id` is null
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_id` must be a valid pointer or null
/// - Must be called during a command callback
#[cfg_attr(coverage_nightly, coverage(off))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_create_window(
    buffer_id: ReovimBufferId,
    out_id: *mut ReovimWindowId,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_id.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let buffer = if buffer_id == 0 {
            None
        } else {
            #[allow(clippy::cast_possible_truncation)]
            Some(BufferId::from_raw(buffer_id as usize))
        };

        match with_runtime(|rt| rt.create_window(buffer)) {
            Err(e) => e,
            Ok(id) => {
                unsafe { *out_id = window_id_to_ffi(id) };
                REOVIM_OK
            }
        }
    }))
}

/// Close a window.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NOT_FOUND` if window doesn't exist
/// - `REOVIM_ERR_LAST_WINDOW` if this is the last window
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - Must be called during a command callback
#[cfg_attr(coverage_nightly, coverage(off))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_close_window(window_id: ReovimWindowId) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        let wid = window_id_from_ffi(window_id);

        match with_runtime(|rt| rt.close_window(wid)) {
            Err(e) => e,
            Ok(Ok(())) => REOVIM_OK,
            Ok(Err(
                reovim_driver_text_session::api::WindowError::NotFound(_)
                | reovim_driver_text_session::api::WindowError::BufferNotFound(_),
            )) => REOVIM_ERR_NOT_FOUND,
            Ok(Err(reovim_driver_text_session::api::WindowError::CannotCloseLastWindow)) => {
                REOVIM_ERR_LAST_WINDOW
            }
        }
    }))
}

/// Focus a window.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NOT_FOUND` if window doesn't exist
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - Must be called during a command callback
#[cfg_attr(coverage_nightly, coverage(off))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_focus_window(window_id: ReovimWindowId) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        let wid = window_id_from_ffi(window_id);

        match with_runtime(|rt| rt.focus_window(wid)) {
            Err(e) => e,
            Ok(Ok(())) => REOVIM_OK,
            Ok(Err(
                reovim_driver_text_session::api::WindowError::NotFound(_)
                | reovim_driver_text_session::api::WindowError::BufferNotFound(_),
            )) => REOVIM_ERR_NOT_FOUND,
            Ok(Err(reovim_driver_text_session::api::WindowError::CannotCloseLastWindow)) => {
                REOVIM_ERR_LAST_WINDOW
            }
        }
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
