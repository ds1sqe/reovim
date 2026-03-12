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
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::new();
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut out: ReovimBufferId = 0;
    assert_eq!(unsafe { reovim_active_buffer(&raw mut out) }, REOVIM_ERR_NOT_FOUND);
}

#[test]
fn test_buffer_queries_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

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
    assert_eq!(unsafe { reovim_buffer_line_count(bid, &raw mut count) }, REOVIM_OK);
    assert_eq!(count, 1);

    // buffer_line_len → Ok(Some)
    let mut len: u32 = 0;
    assert_eq!(unsafe { reovim_buffer_line_len(bid, 0, &raw mut len) }, REOVIM_OK);
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
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

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
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

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
    assert_eq!(unsafe { reovim_delete_buffer(original_bid) }, REOVIM_ERR_LAST_BUFFER);
}
