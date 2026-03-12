//! Undo API FFI functions.
//!
//! These functions expose [`UndoApi`] methods as C-callable functions,
//! allowing external modules to perform undo/redo operations.
//!
//! All functions require an active [`RuntimeGuard`] (i.e., must be called
//! during a command callback). Returns `REOVIM_ERR_NO_RUNTIME` otherwise.
//!
//! # Design Note
//!
//! Only `undo`, `redo`, `can_undo`, and `can_redo` are exposed.
//! `record_edit` is deferred to v3 because it requires `Vec<Edit>` input
//! which is complex to expose across FFI. FFI modules that make edits via
//! `reovim_insert_text`/`reovim_delete_range` get automatic undo recording.
//!
//! [`UndoApi`]: reovim_driver_session::api::UndoApi
//! [`RuntimeGuard`]: crate::RuntimeGuard

#![allow(unsafe_code)]

use reovim_driver_session::api::UndoApi;

#[allow(clippy::wildcard_imports)]
use crate::error::*;
use std::panic::AssertUnwindSafe;

use crate::{
    buffer::{ReovimBufferId, buffer_id_from_ffi},
    ffi_types::ReovimPosition,
    runtime::{ffi_catch_unwind, with_runtime},
};

// ============================================================================
// Undo/Redo functions
// ============================================================================

/// Undo the last change for a buffer.
///
/// If undo succeeds, writes the restored cursor position to `out_cursor`
/// (if non-null). The edits are automatically applied by the undo system.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NOT_FOUND` if nothing to undo
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_cursor` must be a valid pointer or null
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_undo(
    buffer_id: ReovimBufferId,
    out_cursor: *mut ReovimPosition,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        let bid = buffer_id_from_ffi(buffer_id);

        match with_runtime(|rt| rt.undo(bid)) {
            Err(e) => e,
            Ok(None) => REOVIM_ERR_NOT_FOUND,
            Ok(Some(result)) => {
                if !out_cursor.is_null() {
                    unsafe {
                        *out_cursor = ReovimPosition::from(result.cursor);
                    }
                }
                REOVIM_OK
            }
        }
    }))
}

/// Redo the last undone change for a buffer.
///
/// If redo succeeds, writes the restored cursor position to `out_cursor`
/// (if non-null). The edits are automatically applied by the undo system.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NOT_FOUND` if nothing to redo
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_cursor` must be a valid pointer or null
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_redo(
    buffer_id: ReovimBufferId,
    out_cursor: *mut ReovimPosition,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        let bid = buffer_id_from_ffi(buffer_id);

        match with_runtime(|rt| rt.redo(bid)) {
            Err(e) => e,
            Ok(None) => REOVIM_ERR_NOT_FOUND,
            Ok(Some(result)) => {
                if !out_cursor.is_null() {
                    unsafe {
                        *out_cursor = ReovimPosition::from(result.cursor);
                    }
                }
                REOVIM_OK
            }
        }
    }))
}

// ============================================================================
// Query functions
// ============================================================================

/// Check if undo is available for a buffer.
///
/// Writes `1` (true) or `0` (false) to `out_bool`.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_bool` is null
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_bool` must be a valid, aligned pointer to `i32`
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_can_undo(buffer_id: ReovimBufferId, out_bool: *mut i32) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_bool.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let bid = buffer_id_from_ffi(buffer_id);

        match with_runtime(|rt| rt.can_undo(bid)) {
            Err(e) => e,
            Ok(can) => {
                unsafe {
                    *out_bool = i32::from(can);
                }
                REOVIM_OK
            }
        }
    }))
}

/// Check if redo is available for a buffer.
///
/// Writes `1` (true) or `0` (false) to `out_bool`.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_bool` is null
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_bool` must be a valid, aligned pointer to `i32`
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_can_redo(buffer_id: ReovimBufferId, out_bool: *mut i32) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_bool.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let bid = buffer_id_from_ffi(buffer_id);

        match with_runtime(|rt| rt.can_redo(bid)) {
            Err(e) => e,
            Ok(can) => {
                unsafe {
                    *out_bool = i32::from(can);
                }
                REOVIM_OK
            }
        }
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "undo_tests.rs"]
mod tests;
