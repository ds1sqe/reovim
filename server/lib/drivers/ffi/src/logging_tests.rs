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
