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
mod tests {
    use super::*;

    // ========================================================================
    // undo/redo tests
    // ========================================================================

    #[test]
    fn test_undo_no_runtime() {
        let mut cursor = ReovimPosition::new(0, 0);
        let ret = unsafe { reovim_undo(1, &raw mut cursor) };
        assert_eq!(ret, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_undo_null_cursor_no_runtime() {
        let ret = unsafe { reovim_undo(1, std::ptr::null_mut()) };
        assert_eq!(ret, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_redo_no_runtime() {
        let mut cursor = ReovimPosition::new(0, 0);
        let ret = unsafe { reovim_redo(1, &raw mut cursor) };
        assert_eq!(ret, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_redo_null_cursor_no_runtime() {
        let ret = unsafe { reovim_redo(1, std::ptr::null_mut()) };
        assert_eq!(ret, REOVIM_ERR_NO_RUNTIME);
    }

    // ========================================================================
    // can_undo/can_redo tests
    // ========================================================================

    #[test]
    fn test_can_undo_no_runtime() {
        let mut out = 0;
        let ret = unsafe { reovim_can_undo(1, &raw mut out) };
        assert_eq!(ret, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_can_undo_null_out() {
        let ret = unsafe { reovim_can_undo(1, std::ptr::null_mut()) };
        assert_eq!(ret, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_can_redo_no_runtime() {
        let mut out = 0;
        let ret = unsafe { reovim_can_redo(1, &raw mut out) };
        assert_eq!(ret, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_can_redo_null_out() {
        let ret = unsafe { reovim_can_redo(1, std::ptr::null_mut()) };
        assert_eq!(ret, REOVIM_ERR_NULL_PTR);
    }

    // ========================================================================
    // buffer_id_from_ffi tests
    // ========================================================================

    #[test]
    fn test_buffer_id_from_ffi_zero() {
        let bid = buffer_id_from_ffi(0);
        assert_eq!(bid.as_usize(), 0);
    }

    #[test]
    fn test_buffer_id_from_ffi_nonzero() {
        let bid = buffer_id_from_ffi(42);
        assert_eq!(bid.as_usize(), 42);
    }

    // ========================================================================
    // With-runtime tests (RuntimeGuard + TestSessionRuntime)
    // ========================================================================

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn test_can_undo_false_with_runtime() {
        use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let bid = harness.active_buffer().unwrap().as_usize() as ReovimBufferId;
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut out = -1;
        assert_eq!(unsafe { reovim_can_undo(bid, &raw mut out) }, REOVIM_OK);
        assert_eq!(out, 0);
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn test_can_redo_false_with_runtime() {
        use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let bid = harness.active_buffer().unwrap().as_usize() as ReovimBufferId;
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut out = -1;
        assert_eq!(unsafe { reovim_can_redo(bid, &raw mut out) }, REOVIM_OK);
        assert_eq!(out, 0);
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn test_undo_nothing_to_undo() {
        use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let bid = harness.active_buffer().unwrap().as_usize() as ReovimBufferId;
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut cursor = ReovimPosition::new(0, 0);
        assert_eq!(unsafe { reovim_undo(bid, &raw mut cursor) }, REOVIM_ERR_NOT_FOUND);
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn test_redo_nothing_to_redo() {
        use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let bid = harness.active_buffer().unwrap().as_usize() as ReovimBufferId;
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut cursor = ReovimPosition::new(0, 0);
        assert_eq!(unsafe { reovim_redo(bid, &raw mut cursor) }, REOVIM_ERR_NOT_FOUND);
    }
}
