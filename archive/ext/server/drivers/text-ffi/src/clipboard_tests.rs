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
    let ret = unsafe { reovim_paste_from_clipboard(std::ptr::null_mut(), 0, std::ptr::null_mut()) };
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
    let ret = unsafe { reovim_paste_from_selection(std::ptr::null_mut(), 0, std::ptr::null_mut()) };
    assert_eq!(ret, REOVIM_ERR_NULL_PTR);
}

// ========================================================================
// With-runtime tests (RuntimeGuard + TestSessionRuntime)
// ========================================================================

#[test]
fn test_paste_from_clipboard_empty_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut result = ReovimStringResult::ok(0);
    let ret = unsafe { reovim_paste_from_selection(std::ptr::null_mut(), 0, &raw mut result) };
    assert_eq!(ret, REOVIM_ERR_NOT_FOUND);
}

#[test]
fn test_copy_to_clipboard_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let text = c"test copy";
    let ret = unsafe { reovim_copy_to_selection(text.as_ptr()) };
    assert!(ret == REOVIM_OK || ret == REOVIM_ERR_FAILED);
}
