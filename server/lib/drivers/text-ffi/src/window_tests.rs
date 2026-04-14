use super::*;

// ========================================================================
// No-runtime error tests
// ========================================================================

#[test]
fn test_active_window_no_runtime() {
    let mut out: ReovimWindowId = 0;
    let result = unsafe { reovim_active_window(&raw mut out) };
    assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
}

#[test]
fn test_cursor_position_no_runtime() {
    let mut pos = ReovimPosition::new(0, 0);
    let result = unsafe { reovim_cursor_position(&raw mut pos) };
    assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
}

#[test]
fn test_window_count_no_runtime() {
    let mut count: u32 = 0;
    let result = unsafe { reovim_window_count(&raw mut count) };
    assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
}

#[test]
fn test_window_buffer_no_runtime() {
    let mut buf_id: ReovimBufferId = 0;
    let result = unsafe { reovim_window_buffer(1, &raw mut buf_id) };
    assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
}

#[test]
fn test_create_window_no_runtime() {
    let mut out: ReovimWindowId = 0;
    let result = unsafe { reovim_create_window(0, &raw mut out) };
    assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
}

#[test]
fn test_close_window_no_runtime() {
    let result = unsafe { reovim_close_window(1) };
    assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
}

#[test]
fn test_focus_window_no_runtime() {
    let result = unsafe { reovim_focus_window(1) };
    assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
}

// ========================================================================
// Null pointer tests
// ========================================================================

#[test]
fn test_active_window_null_out() {
    let result = unsafe { reovim_active_window(std::ptr::null_mut()) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

#[test]
fn test_cursor_position_null_out() {
    let result = unsafe { reovim_cursor_position(std::ptr::null_mut()) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

#[test]
fn test_window_count_null_out() {
    let result = unsafe { reovim_window_count(std::ptr::null_mut()) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

#[test]
fn test_window_buffer_null_out() {
    let result = unsafe { reovim_window_buffer(1, std::ptr::null_mut()) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

#[test]
fn test_create_window_null_out() {
    let result = unsafe { reovim_create_window(0, std::ptr::null_mut()) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

// ========================================================================
// Helper tests
// ========================================================================

#[test]
fn test_window_id_roundtrip() {
    let original = WindowId::from_raw(42);
    let ffi = window_id_to_ffi(original);
    let back = window_id_from_ffi(ffi);
    assert_eq!(original, back);
}

#[test]
fn test_window_id_zero() {
    let ffi = window_id_to_ffi(WindowId::from_raw(0));
    assert_eq!(ffi, 0);
    let back = window_id_from_ffi(0);
    assert_eq!(back, WindowId::from_raw(0));
}

// ========================================================================
// With-runtime tests (RuntimeGuard + TestSessionRuntime)
// ========================================================================

#[test]
fn test_active_window_found() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut out: ReovimWindowId = 0;
    assert_eq!(unsafe { reovim_active_window(&raw mut out) }, REOVIM_OK);
    assert_ne!(out, 0);
}

#[test]
fn test_active_window_not_found_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::new();
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut out: ReovimWindowId = 0;
    assert_eq!(unsafe { reovim_active_window(&raw mut out) }, REOVIM_ERR_NOT_FOUND);
}

#[test]
fn test_cursor_position_found() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut pos = ReovimPosition::new(0, 0);
    assert_eq!(unsafe { reovim_cursor_position(&raw mut pos) }, REOVIM_OK);
}

#[test]
fn test_cursor_position_not_found_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::new();
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut pos = ReovimPosition::new(0, 0);
    assert_eq!(unsafe { reovim_cursor_position(&raw mut pos) }, REOVIM_ERR_NOT_FOUND);
}

#[test]
fn test_window_count_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut count: u32 = 0;
    assert_eq!(unsafe { reovim_window_count(&raw mut count) }, REOVIM_OK);
    assert!(count >= 1);
}

#[test]
fn test_window_buffer_found_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    // Get active window first
    let mut wid: ReovimWindowId = 0;
    assert_eq!(unsafe { reovim_active_window(&raw mut wid) }, REOVIM_OK);

    let mut buf_id: ReovimBufferId = 0;
    assert_eq!(unsafe { reovim_window_buffer(wid, &raw mut buf_id) }, REOVIM_OK);
    assert_ne!(buf_id, 0);
}

#[test]
fn test_window_buffer_not_found_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut buf_id: ReovimBufferId = 0;
    assert_eq!(unsafe { reovim_window_buffer(99999, &raw mut buf_id) }, REOVIM_ERR_NOT_FOUND);
}

#[test]
fn test_create_window_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut out: ReovimWindowId = 0;
    assert_eq!(unsafe { reovim_create_window(0, &raw mut out) }, REOVIM_OK);
    assert_ne!(out, 0);
}

#[test]
fn test_close_window_not_found_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    // With only one window, kernel returns LAST_WINDOW before NOT_FOUND
    assert_eq!(unsafe { reovim_close_window(99999) }, REOVIM_ERR_LAST_WINDOW);
}

#[test]
fn test_close_window_last_window_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut wid: ReovimWindowId = 0;
    assert_eq!(unsafe { reovim_active_window(&raw mut wid) }, REOVIM_OK);
    assert_eq!(unsafe { reovim_close_window(wid) }, REOVIM_ERR_LAST_WINDOW);
}

#[test]
fn test_focus_window_success_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut wid: ReovimWindowId = 0;
    assert_eq!(unsafe { reovim_active_window(&raw mut wid) }, REOVIM_OK);
    assert_eq!(unsafe { reovim_focus_window(wid) }, REOVIM_OK);
}

#[test]
fn test_focus_window_not_found_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    assert_eq!(unsafe { reovim_focus_window(99999) }, REOVIM_ERR_NOT_FOUND);
}
