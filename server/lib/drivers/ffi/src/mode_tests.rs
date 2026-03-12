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

// ========================================================================
// With-runtime tests (RuntimeGuard + TestSessionRuntime)
// ========================================================================

#[test]
fn test_current_mode_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut buf = [0u8; 64];
    let mut result = ReovimStringResult::err(0);
    assert_eq!(unsafe { reovim_current_mode(buf.as_mut_ptr(), 64, &raw mut result) }, REOVIM_OK);
    assert!(result.length > 0);
}

#[test]
fn test_mode_depth_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut depth: u32 = 0;
    assert_eq!(unsafe { reovim_mode_depth(&raw mut depth) }, REOVIM_OK);
    assert!(depth >= 1);
}

#[test]
fn test_push_mode_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mode = std::ffi::CString::new("test:custom").unwrap();
    assert_eq!(unsafe { reovim_push_mode(mode.as_ptr()) }, REOVIM_OK);
}

#[test]
fn test_pop_mode_home_fails_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    assert_eq!(unsafe { reovim_pop_mode() }, REOVIM_ERR_FAILED);
}

#[test]
fn test_pop_mode_success_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mode = std::ffi::CString::new("test:temp").unwrap();
    assert_eq!(unsafe { reovim_push_mode(mode.as_ptr()) }, REOVIM_OK);
    assert_eq!(unsafe { reovim_pop_mode() }, REOVIM_OK);
}

#[test]
fn test_set_mode_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mode = std::ffi::CString::new("test:replaced").unwrap();
    assert_eq!(unsafe { reovim_set_mode(mode.as_ptr()) }, REOVIM_OK);
}
