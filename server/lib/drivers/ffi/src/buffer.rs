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
mod tests {
    use super::*;

    // ========================================================================
    // No-runtime error tests (all functions should return REOVIM_ERR_NO_RUNTIME)
    // ========================================================================

    #[test]
    fn test_active_buffer_no_runtime() {
        let mut out: ReovimBufferId = 0;
        let result = unsafe { reovim_active_buffer(&raw mut out) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_buffer_line_no_runtime() {
        let mut buf = [0u8; 64];
        let mut result = ReovimStringResult::err(0);
        let status = unsafe { reovim_buffer_line(1, 0, buf.as_mut_ptr(), 64, &raw mut result) };
        assert_eq!(status, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_buffer_line_count_no_runtime() {
        let mut count: u32 = 0;
        let result = unsafe { reovim_buffer_line_count(1, &raw mut count) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_buffer_line_len_no_runtime() {
        let mut len: u32 = 0;
        let result = unsafe { reovim_buffer_line_len(1, 0, &raw mut len) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_buffer_text_range_no_runtime() {
        let mut buf = [0u8; 64];
        let mut result = ReovimStringResult::err(0);
        let status = unsafe {
            reovim_buffer_text_range(
                1,
                ReovimPosition::new(0, 0),
                ReovimPosition::new(0, 5),
                buf.as_mut_ptr(),
                64,
                &raw mut result,
            )
        };
        assert_eq!(status, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_buffer_content_no_runtime() {
        let mut buf = [0u8; 64];
        let mut result = ReovimStringResult::err(0);
        let status = unsafe { reovim_buffer_content(1, buf.as_mut_ptr(), 64, &raw mut result) };
        assert_eq!(status, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_insert_text_no_runtime() {
        let text = std::ffi::CString::new("hello").unwrap();
        let result = unsafe { reovim_insert_text(1, ReovimPosition::new(0, 0), text.as_ptr()) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_delete_range_no_runtime() {
        let result =
            unsafe { reovim_delete_range(1, ReovimPosition::new(0, 0), ReovimPosition::new(0, 5)) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_create_buffer_no_runtime() {
        let mut out_id: ReovimBufferId = 0;
        let result =
            unsafe { reovim_create_buffer(std::ptr::null(), std::ptr::null(), &raw mut out_id) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    #[test]
    fn test_delete_buffer_no_runtime() {
        let result = unsafe { reovim_delete_buffer(1) };
        assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
    }

    // ========================================================================
    // Null pointer tests
    // ========================================================================

    #[test]
    fn test_active_buffer_null_out() {
        let result = unsafe { reovim_active_buffer(std::ptr::null_mut()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_buffer_line_null_result() {
        let mut buf = [0u8; 64];
        let result =
            unsafe { reovim_buffer_line(1, 0, buf.as_mut_ptr(), 64, std::ptr::null_mut()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_buffer_line_count_null_out() {
        let result = unsafe { reovim_buffer_line_count(1, std::ptr::null_mut()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_buffer_line_len_null_out() {
        let result = unsafe { reovim_buffer_line_len(1, 0, std::ptr::null_mut()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_buffer_text_range_null_result() {
        let mut buf = [0u8; 64];
        let result = unsafe {
            reovim_buffer_text_range(
                1,
                ReovimPosition::new(0, 0),
                ReovimPosition::new(0, 5),
                buf.as_mut_ptr(),
                64,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_buffer_content_null_result() {
        let mut buf = [0u8; 64];
        let result =
            unsafe { reovim_buffer_content(1, buf.as_mut_ptr(), 64, std::ptr::null_mut()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_insert_text_null_text() {
        let result = unsafe { reovim_insert_text(1, ReovimPosition::new(0, 0), std::ptr::null()) };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    #[test]
    fn test_create_buffer_null_out_id() {
        let result = unsafe {
            reovim_create_buffer(std::ptr::null(), std::ptr::null(), std::ptr::null_mut())
        };
        assert_eq!(result, REOVIM_ERR_NULL_PTR);
    }

    // ========================================================================
    // UTF-8 validation tests
    // ========================================================================

    #[test]
    fn test_insert_text_invalid_utf8() {
        // Even without runtime, null-ptr check happens first, then UTF-8 check,
        // then runtime check. So we get REOVIM_ERR_INVALID_UTF8 before NO_RUNTIME.
        let invalid_bytes: &[u8] = &[0x48, 0x65, 0x80, 0x00]; // "He\x80\0"
        let result = unsafe {
            reovim_insert_text(1, ReovimPosition::new(0, 0), invalid_bytes.as_ptr().cast())
        };
        assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
    }

    #[test]
    fn test_create_buffer_invalid_utf8_name() {
        let mut out_id: ReovimBufferId = 0;
        let invalid_bytes: &[u8] = &[0x80, 0x00];
        let result = unsafe {
            reovim_create_buffer(invalid_bytes.as_ptr().cast(), std::ptr::null(), &raw mut out_id)
        };
        assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
    }

    #[test]
    fn test_create_buffer_invalid_utf8_content() {
        let mut out_id: ReovimBufferId = 0;
        let valid_name = std::ffi::CString::new("test").unwrap();
        let invalid_bytes: &[u8] = &[0x80, 0x00];
        let result = unsafe {
            reovim_create_buffer(
                valid_name.as_ptr(),
                invalid_bytes.as_ptr().cast(),
                &raw mut out_id,
            )
        };
        assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
    }

    // ========================================================================
    // Helper tests
    // ========================================================================

    #[test]
    fn test_buffer_id_roundtrip() {
        let original = BufferId::from_raw(42);
        let ffi = buffer_id_to_ffi(original);
        let back = buffer_id_from_ffi(ffi);
        assert_eq!(original, back);
    }

    #[test]
    fn test_buffer_id_zero() {
        let ffi = buffer_id_to_ffi(BufferId::from_raw(0));
        assert_eq!(ffi, 0);
        let back = buffer_id_from_ffi(0);
        assert_eq!(back, BufferId::from_raw(0));
    }

    #[test]
    fn test_write_string_to_buf_full() {
        let mut buf = [0u8; 16];
        let mut result = ReovimStringResult::err(0);
        write_string_to_buf("hello", buf.as_mut_ptr(), 16, &raw mut result);
        assert_eq!(result.status, REOVIM_OK);
        assert_eq!(result.length, 5);
        assert_eq!(&buf[..5], b"hello");
    }

    #[test]
    fn test_write_string_to_buf_truncated() {
        let mut buf = [0u8; 3];
        let mut result = ReovimStringResult::err(0);
        write_string_to_buf("hello", buf.as_mut_ptr(), 3, &raw mut result);
        assert_eq!(result.status, REOVIM_OK);
        assert_eq!(result.length, 5); // reports full length
        assert_eq!(&buf[..3], b"hel"); // truncated
    }

    #[test]
    fn test_write_string_to_buf_null_buf() {
        let mut result = ReovimStringResult::err(0);
        write_string_to_buf("hello", std::ptr::null_mut(), 0, &raw mut result);
        assert_eq!(result.status, REOVIM_OK);
        assert_eq!(result.length, 5);
    }

    #[test]
    fn test_write_string_to_buf_null_result() {
        let mut buf = [0u8; 16];
        // Should not crash with null result pointer
        write_string_to_buf("hello", buf.as_mut_ptr(), 16, std::ptr::null_mut());
        assert_eq!(&buf[..5], b"hello");
    }

    #[test]
    fn test_write_string_to_buf_empty() {
        let mut buf = [0u8; 16];
        let mut result = ReovimStringResult::err(0);
        write_string_to_buf("", buf.as_mut_ptr(), 16, &raw mut result);
        assert_eq!(result.status, REOVIM_OK);
        assert_eq!(result.length, 0);
    }

    // ========================================================================
    // With-runtime tests (RuntimeGuard + TestSessionRuntime)
    // ========================================================================

    #[test]
    fn test_active_buffer_found() {
        use crate::runtime::RuntimeGuard;
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let expected = buffer_id_to_ffi(harness.active_buffer().unwrap());
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut out: ReovimBufferId = 0;
        assert_eq!(unsafe { reovim_active_buffer(&raw mut out) }, REOVIM_OK);
        assert_eq!(out, expected);
    }

    #[test]
    fn test_active_buffer_not_found_with_runtime() {
        use crate::runtime::RuntimeGuard;
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut out: ReovimBufferId = 0;
        assert_eq!(
            unsafe { reovim_active_buffer(&raw mut out) },
            REOVIM_ERR_NOT_FOUND
        );
    }

    #[test]
    fn test_buffer_queries_with_runtime() {
        use crate::runtime::RuntimeGuard;
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let bid = buffer_id_to_ffi(harness.active_buffer().unwrap());
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        // buffer_line → Ok(Some)
        let mut buf = [0u8; 64];
        let mut str_result = ReovimStringResult::err(0);
        assert_eq!(
            unsafe { reovim_buffer_line(bid, 0, buf.as_mut_ptr(), 64, &raw mut str_result) },
            REOVIM_OK
        );
        assert_eq!(&buf[..str_result.length as usize], b"hello world");

        // buffer_line_count → Ok(Some)
        let mut count: u32 = 0;
        assert_eq!(
            unsafe { reovim_buffer_line_count(bid, &raw mut count) },
            REOVIM_OK
        );
        assert_eq!(count, 1);

        // buffer_line_len → Ok(Some)
        let mut len: u32 = 0;
        assert_eq!(
            unsafe { reovim_buffer_line_len(bid, 0, &raw mut len) },
            REOVIM_OK
        );
        assert_eq!(len, 11);

        // buffer_content → Ok(Some)
        let mut buf2 = [0u8; 64];
        let mut str_result2 = ReovimStringResult::err(0);
        assert_eq!(
            unsafe { reovim_buffer_content(bid, buf2.as_mut_ptr(), 64, &raw mut str_result2) },
            REOVIM_OK
        );
        let content = std::str::from_utf8(&buf2[..str_result2.length as usize]).unwrap();
        assert!(content.contains("hello world"));

        // buffer_text_range → Ok(Some)
        let mut buf3 = [0u8; 64];
        let mut str_result3 = ReovimStringResult::err(0);
        assert_eq!(
            unsafe {
                reovim_buffer_text_range(
                    bid,
                    ReovimPosition::new(0, 0),
                    ReovimPosition::new(0, 5),
                    buf3.as_mut_ptr(),
                    64,
                    &raw mut str_result3,
                )
            },
            REOVIM_OK
        );
    }

    #[test]
    fn test_buffer_line_out_of_range_with_runtime() {
        use crate::runtime::RuntimeGuard;
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let bid = buffer_id_to_ffi(harness.active_buffer().unwrap());
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut buf = [0u8; 64];
        let mut result = ReovimStringResult::err(0);
        assert_eq!(
            unsafe { reovim_buffer_line(bid, 99, buf.as_mut_ptr(), 64, &raw mut result) },
            REOVIM_ERR_OUT_OF_RANGE
        );
    }

    #[test]
    fn test_buffer_queries_invalid_buffer() {
        use crate::runtime::RuntimeGuard;
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("x");
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let bad_id: ReovimBufferId = 99999;

        // buffer_line → Ok(None) → NOT_FOUND
        let mut buf = [0u8; 64];
        let mut result = ReovimStringResult::err(0);
        assert_eq!(
            unsafe { reovim_buffer_line(bad_id, 0, buf.as_mut_ptr(), 64, &raw mut result) },
            REOVIM_ERR_NOT_FOUND
        );

        // buffer_line_count → Ok(None)
        let mut count: u32 = 0;
        assert_eq!(
            unsafe { reovim_buffer_line_count(bad_id, &raw mut count) },
            REOVIM_ERR_NOT_FOUND
        );

        // buffer_line_len → Ok(None) → NOT_FOUND
        let mut len: u32 = 0;
        assert_eq!(
            unsafe { reovim_buffer_line_len(bad_id, 0, &raw mut len) },
            REOVIM_ERR_NOT_FOUND
        );

        // buffer_content → Ok(None)
        let mut result2 = ReovimStringResult::err(0);
        assert_eq!(
            unsafe { reovim_buffer_content(bad_id, buf.as_mut_ptr(), 64, &raw mut result2) },
            REOVIM_ERR_NOT_FOUND
        );

        // buffer_text_range → Ok(None)
        let mut result3 = ReovimStringResult::err(0);
        assert_eq!(
            unsafe {
                reovim_buffer_text_range(
                    bad_id,
                    ReovimPosition::new(0, 0),
                    ReovimPosition::new(0, 5),
                    buf.as_mut_ptr(),
                    64,
                    &raw mut result3,
                )
            },
            REOVIM_ERR_NOT_FOUND
        );
    }

    #[test]
    fn test_buffer_line_len_out_of_range_with_runtime() {
        use crate::runtime::RuntimeGuard;
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let bid = buffer_id_to_ffi(harness.active_buffer().unwrap());
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        let mut len: u32 = 0;
        assert_eq!(
            unsafe { reovim_buffer_line_len(bid, 99, &raw mut len) },
            REOVIM_ERR_OUT_OF_RANGE
        );
    }

    #[test]
    fn test_buffer_mutations_with_runtime() {
        use crate::runtime::RuntimeGuard;
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let bid = buffer_id_to_ffi(harness.active_buffer().unwrap());
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        // insert_text → Ok(())
        let text = std::ffi::CString::new("!").unwrap();
        assert_eq!(
            unsafe { reovim_insert_text(bid, ReovimPosition::new(0, 5), text.as_ptr()) },
            REOVIM_OK
        );

        // delete_range → Ok(())
        assert_eq!(
            unsafe {
                reovim_delete_range(bid, ReovimPosition::new(0, 0), ReovimPosition::new(0, 1))
            },
            REOVIM_OK
        );
    }

    #[test]
    fn test_buffer_lifecycle_with_runtime() {
        use crate::runtime::RuntimeGuard;
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("existing");
        let original_bid = buffer_id_to_ffi(harness.active_buffer().unwrap());
        let mut rt = harness.runtime();
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };

        // create_buffer → Ok(id)
        let name = std::ffi::CString::new("new").unwrap();
        let mut new_id: ReovimBufferId = 0;
        assert_eq!(
            unsafe { reovim_create_buffer(name.as_ptr(), std::ptr::null(), &raw mut new_id) },
            REOVIM_OK
        );
        assert_ne!(new_id, 0);

        // delete_buffer (new buffer) → Ok(Ok(()))
        assert_eq!(unsafe { reovim_delete_buffer(new_id) }, REOVIM_OK);

        // delete_buffer (last buffer) → Ok(Err(CannotDeleteLastBuffer))
        // Note: kernel checks "last buffer" before "not found"
        assert_eq!(
            unsafe { reovim_delete_buffer(original_bid) },
            REOVIM_ERR_LAST_BUFFER
        );
    }
}
