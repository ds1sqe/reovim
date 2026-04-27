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
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_text_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let bid = harness.active_buffer().unwrap().as_usize() as ReovimBufferId;
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut cursor = ReovimPosition::new(0, 0);
    assert_eq!(unsafe { reovim_redo(bid, &raw mut cursor) }, REOVIM_ERR_NOT_FOUND);
}
