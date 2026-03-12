//! Buffer API FFI functions.
//!
//! These functions expose [`BufferApi`] methods as C-callable functions,
//! allowing external modules to query and modify buffer content.
//!
//! All functions require an active [`RuntimeGuard`] (i.e., must be called
//! during a command callback). Returns `REOVIM_ERR_NO_RUNTIME` otherwise.
//!
//! # Memory Model
//!
//! String reads use **caller-owned buffers**: the caller provides a `*mut u8`
//! buffer and its length. The callee fills the buffer and reports the actual
//! string length via [`ReovimStringResult`]. If the actual length exceeds
//! `buf_len`, the string is truncated but the full length is still reported,
//! allowing the caller to retry with a larger buffer.
//!
//! [`BufferApi`]: reovim_driver_session::api::BufferApi
//! [`RuntimeGuard`]: crate::RuntimeGuard
//! [`ReovimStringResult`]: crate::ReovimStringResult

#![allow(unsafe_code)]

use std::ffi::CStr;

use {
    libc::c_char,
    reovim_driver_session::api::BufferApi,
    reovim_kernel::api::v1::{BufferId, Position},
};

#[allow(clippy::wildcard_imports)]
use crate::error::*;
use std::panic::AssertUnwindSafe;

use crate::{
    ffi_types::{ReovimPosition, ReovimStringResult},
    runtime::{ffi_catch_unwind, with_runtime},
};

/// Opaque buffer ID for FFI. Maps to kernel `BufferId(usize)`.
pub type ReovimBufferId = u64;

/// Convert a kernel `BufferId` to the FFI representation.
#[allow(clippy::cast_possible_truncation)]
const fn buffer_id_to_ffi(id: BufferId) -> ReovimBufferId {
    id.as_usize() as ReovimBufferId
}

/// Convert an FFI buffer ID to a kernel `BufferId`.
#[allow(clippy::cast_possible_truncation)]
pub(crate) const fn buffer_id_from_ffi(id: ReovimBufferId) -> BufferId {
    BufferId::from_raw(id as usize)
}

/// Helper: write a Rust string into a caller-owned buffer, returning the result.
///
/// If `buf` is null, writes only the length (no copy). If `buf_len` is smaller
/// than the string, truncates but reports the full length.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn write_string_to_buf(
    s: &str,
    buf: *mut u8,
    buf_len: u32,
    out_result: *mut ReovimStringResult,
) {
    let actual_len = s.len() as u32;
    let copy_len = actual_len.min(buf_len) as usize;

    if !buf.is_null() && copy_len > 0 {
        // Safety: caller guarantees buf points to at least buf_len bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(s.as_ptr(), buf, copy_len);
        }
    }

    if !out_result.is_null() {
        // Safety: caller guarantees out_result is valid.
        unsafe {
            *out_result = ReovimStringResult::ok(actual_len);
        }
    }
}

// ============================================================================
// Query functions
// ============================================================================

/// Get the active buffer ID.
///
/// Writes the active buffer ID to `out_id`.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_id` is null
/// - `REOVIM_ERR_NOT_FOUND` if no active buffer
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_id` must be a valid pointer or null
/// - Must be called during a command callback (`RuntimeGuard` active)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_active_buffer(out_id: *mut ReovimBufferId) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_id.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        match with_runtime(|rt| rt.active_buffer()) {
            Err(e) => e,
            Ok(None) => REOVIM_ERR_NOT_FOUND,
            Ok(Some(id)) => {
                unsafe { *out_id = buffer_id_to_ffi(id) };
                REOVIM_OK
            }
        }
    }))
}

/// Get a line from a buffer.
///
/// Copies the line content into the caller-owned buffer `buf` of size `buf_len`.
/// The actual length is written to `out_result->length`. If the line is longer
/// than `buf_len`, it is truncated but the full length is reported.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_result` is null
/// - `REOVIM_ERR_NOT_FOUND` if buffer doesn't exist
/// - `REOVIM_ERR_OUT_OF_RANGE` if line is out of bounds
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `buf` must point to at least `buf_len` writable bytes, or be null
/// - `out_result` must be a valid pointer
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_buffer_line(
    buffer_id: ReovimBufferId,
    line: u32,
    buf: *mut u8,
    buf_len: u32,
    out_result: *mut ReovimStringResult,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_result.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let bid = buffer_id_from_ffi(buffer_id);

        match with_runtime(|rt| rt.buffer_line(bid, line as usize)) {
            Err(e) => e,
            Ok(None) => {
                // Could be invalid buffer or out-of-range line.
                // Check if buffer exists by querying line count.
                match with_runtime(|rt| rt.buffer_line_count(bid)) {
                    Ok(Some(count)) if (line as usize) >= count => {
                        unsafe { *out_result = ReovimStringResult::err(REOVIM_ERR_OUT_OF_RANGE) };
                        REOVIM_ERR_OUT_OF_RANGE
                    }
                    _ => {
                        unsafe { *out_result = ReovimStringResult::err(REOVIM_ERR_NOT_FOUND) };
                        REOVIM_ERR_NOT_FOUND
                    }
                }
            }
            Ok(Some(content)) => {
                write_string_to_buf(&content, buf, buf_len, out_result);
                REOVIM_OK
            }
        }
    }))
}

/// Get the line count of a buffer.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_count` is null
/// - `REOVIM_ERR_NOT_FOUND` if buffer doesn't exist
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_count` must be a valid pointer or null
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_buffer_line_count(
    buffer_id: ReovimBufferId,
    out_count: *mut u32,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_count.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let bid = buffer_id_from_ffi(buffer_id);

        match with_runtime(|rt| rt.buffer_line_count(bid)) {
            Err(e) => e,
            Ok(None) => REOVIM_ERR_NOT_FOUND,
            Ok(Some(count)) => {
                #[allow(clippy::cast_possible_truncation)]
                unsafe {
                    *out_count = count as u32;
                }
                REOVIM_OK
            }
        }
    }))
}

/// Get the length of a line in bytes.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_len` is null
/// - `REOVIM_ERR_NOT_FOUND` if buffer doesn't exist
/// - `REOVIM_ERR_OUT_OF_RANGE` if line is out of bounds
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `out_len` must be a valid pointer or null
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_buffer_line_len(
    buffer_id: ReovimBufferId,
    line: u32,
    out_len: *mut u32,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_len.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let bid = buffer_id_from_ffi(buffer_id);

        match with_runtime(|rt| rt.buffer_line_len(bid, line as usize)) {
            Err(e) => e,
            Ok(None) => match with_runtime(|rt| rt.buffer_line_count(bid)) {
                Ok(Some(count)) if (line as usize) >= count => REOVIM_ERR_OUT_OF_RANGE,
                _ => REOVIM_ERR_NOT_FOUND,
            },
            Ok(Some(len)) => {
                #[allow(clippy::cast_possible_truncation)]
                unsafe {
                    *out_len = len as u32;
                }
                REOVIM_OK
            }
        }
    }))
}

/// Extract text from a range in the buffer.
///
/// The range is `[start, end)` (start-inclusive, end-exclusive).
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_result` is null
/// - `REOVIM_ERR_NOT_FOUND` if buffer doesn't exist
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `buf` must point to at least `buf_len` writable bytes, or be null
/// - `out_result` must be a valid pointer
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_buffer_text_range(
    buffer_id: ReovimBufferId,
    start: ReovimPosition,
    end: ReovimPosition,
    buf: *mut u8,
    buf_len: u32,
    out_result: *mut ReovimStringResult,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_result.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let bid = buffer_id_from_ffi(buffer_id);
        let start_pos = Position::from(start);
        let end_pos = Position::from(end);

        match with_runtime(|rt| rt.buffer_text_range(bid, start_pos, end_pos)) {
            Err(e) => e,
            Ok(None) => {
                unsafe { *out_result = ReovimStringResult::err(REOVIM_ERR_NOT_FOUND) };
                REOVIM_ERR_NOT_FOUND
            }
            Ok(Some(content)) => {
                write_string_to_buf(&content, buf, buf_len, out_result);
                REOVIM_OK
            }
        }
    }))
}

/// Get full buffer content as a string.
///
/// Useful for small buffers (config files, etc.). For large buffers, prefer
/// `reovim_buffer_line` or `reovim_buffer_text_range`.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `out_result` is null
/// - `REOVIM_ERR_NOT_FOUND` if buffer doesn't exist
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `buf` must point to at least `buf_len` writable bytes, or be null
/// - `out_result` must be a valid pointer
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_buffer_content(
    buffer_id: ReovimBufferId,
    buf: *mut u8,
    buf_len: u32,
    out_result: *mut ReovimStringResult,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_result.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let bid = buffer_id_from_ffi(buffer_id);

        match with_runtime(|rt| rt.buffer_content(bid)) {
            Err(e) => e,
            Ok(None) => {
                unsafe { *out_result = ReovimStringResult::err(REOVIM_ERR_NOT_FOUND) };
                REOVIM_ERR_NOT_FOUND
            }
            Ok(Some(content)) => {
                write_string_to_buf(&content, buf, buf_len, out_result);
                REOVIM_OK
            }
        }
    }))
}

// ============================================================================
// Mutation functions
// ============================================================================

/// Insert text at a position in a buffer.
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
pub unsafe extern "C" fn reovim_insert_text(
    buffer_id: ReovimBufferId,
    pos: ReovimPosition,
    text: *const c_char,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if text.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        // Safety: caller guarantees text is a valid C string.
        let c_str = unsafe { CStr::from_ptr(text) };
        let Ok(text_str) = c_str.to_str() else {
            return REOVIM_ERR_INVALID_UTF8;
        };

        let bid = buffer_id_from_ffi(buffer_id);
        let position = Position::from(pos);

        match with_runtime(|rt| rt.insert_text(bid, position, text_str)) {
            Err(e) => e,
            Ok(()) => REOVIM_OK,
        }
    }))
}

/// Delete a range from a buffer.
///
/// The range is `[start, end)` (start-inclusive, end-exclusive).
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_delete_range(
    buffer_id: ReovimBufferId,
    start: ReovimPosition,
    end: ReovimPosition,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        let bid = buffer_id_from_ffi(buffer_id);
        let start_pos = Position::from(start);
        let end_pos = Position::from(end);

        match with_runtime(|rt| rt.delete_range(bid, start_pos, end_pos)) {
            Err(e) => e,
            Ok(()) => REOVIM_OK,
        }
    }))
}

// ============================================================================
// Lifecycle functions
// ============================================================================

/// Create a new buffer.
///
/// `name` may be null for an unnamed buffer. `content` may be null for an
/// empty buffer.
///
/// # Returns
///
/// - `REOVIM_OK` on success (buffer ID written to `out_id`)
/// - `REOVIM_ERR_NULL_PTR` if `out_id` is null
/// - `REOVIM_ERR_INVALID_UTF8` if `name` or `content` is not valid UTF-8
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `name` must be a valid null-terminated C string, or null
/// - `content` must be a valid null-terminated C string, or null
/// - `out_id` must be a valid pointer
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_create_buffer(
    name: *const c_char,
    content: *const c_char,
    out_id: *mut ReovimBufferId,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if out_id.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let name_str = if name.is_null() {
            None
        } else {
            // Safety: caller guarantees name is a valid C string.
            match unsafe { CStr::from_ptr(name) }.to_str() {
                Ok(s) => Some(s),
                Err(_) => return REOVIM_ERR_INVALID_UTF8,
            }
        };

        let content_str = if content.is_null() {
            ""
        } else {
            // Safety: caller guarantees content is a valid C string.
            match unsafe { CStr::from_ptr(content) }.to_str() {
                Ok(s) => s,
                Err(_) => return REOVIM_ERR_INVALID_UTF8,
            }
        };

        match with_runtime(|rt| rt.create_buffer(name_str, content_str)) {
            Err(e) => e,
            Ok(id) => {
                unsafe { *out_id = buffer_id_to_ffi(id) };
                REOVIM_OK
            }
        }
    }))
}

/// Delete a buffer.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NOT_FOUND` if buffer doesn't exist
/// - `REOVIM_ERR_LAST_BUFFER` if this is the last buffer
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - Must be called during a command callback
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_delete_buffer(buffer_id: ReovimBufferId) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        let bid = buffer_id_from_ffi(buffer_id);

        match with_runtime(|rt| rt.delete_buffer(bid)) {
            Err(e) => e,
            Ok(Ok(())) => REOVIM_OK,
            Ok(Err(reovim_driver_session::api::BufferError::NotFound(_))) => REOVIM_ERR_NOT_FOUND,
            Ok(Err(reovim_driver_session::api::BufferError::CannotDeleteLastBuffer)) => {
                REOVIM_ERR_LAST_BUFFER
            }
        }
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
