use {super::*, reovim_driver_session::api::YankType};

// ========================================================================
// register_name_from_ffi tests
// ========================================================================

#[test]
fn test_register_name_zero_is_unnamed() {
    assert_eq!(register_name_from_ffi(0), None);
}

#[test]
fn test_register_name_letter() {
    assert_eq!(register_name_from_ffi(b'a'), Some('a'));
}

#[test]
fn test_register_name_quote() {
    assert_eq!(register_name_from_ffi(b'"'), Some('"'));
}

#[test]
fn test_register_name_plus() {
    assert_eq!(register_name_from_ffi(b'+'), Some('+'));
}

#[test]
fn test_register_name_star() {
    assert_eq!(register_name_from_ffi(b'*'), Some('*'));
}

// ========================================================================
// ReovimYankType conversion tests
// ========================================================================

#[test]
fn test_yank_type_from_kernel_characterwise() {
    let ffi = ReovimYankType::from(YankType::Characterwise);
    assert_eq!(ffi, ReovimYankType::Characterwise);
}

#[test]
fn test_yank_type_from_kernel_linewise() {
    let ffi = ReovimYankType::from(YankType::Linewise);
    assert_eq!(ffi, ReovimYankType::Linewise);
}

#[test]
fn test_yank_type_to_kernel_characterwise() {
    let kernel: YankType = ReovimYankType::Characterwise.into();
    assert_eq!(kernel, YankType::Characterwise);
}

#[test]
fn test_yank_type_to_kernel_linewise() {
    let kernel: YankType = ReovimYankType::Linewise.into();
    assert_eq!(kernel, YankType::Linewise);
}

#[test]
fn test_yank_type_roundtrip() {
    for yt in [YankType::Characterwise, YankType::Linewise] {
        let ffi = ReovimYankType::from(yt);
        let back: YankType = ffi.into();
        assert_eq!(yt, back);
    }
}

// ========================================================================
// get_register tests
// ========================================================================

#[test]
fn test_get_register_no_runtime() {
    let mut result = ReovimStringResult::ok(0);
    let ret = unsafe {
        reovim_get_register(
            b'a',
            std::ptr::null_mut(),
            0,
            &raw mut result,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(ret, REOVIM_ERR_NO_RUNTIME);
}

#[test]
fn test_get_register_null_out_result() {
    let ret = unsafe {
        reovim_get_register(
            b'a',
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(ret, REOVIM_ERR_NULL_PTR);
}

// ========================================================================
// set_register tests
// ========================================================================

#[test]
fn test_set_register_no_runtime() {
    let text = c"hello";
    let result =
        unsafe { reovim_set_register(b'a', text.as_ptr(), ReovimYankType::Characterwise) };
    assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
}

#[test]
fn test_set_register_null_text() {
    let result =
        unsafe { reovim_set_register(b'a', std::ptr::null(), ReovimYankType::Characterwise) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

#[test]
fn test_set_register_invalid_utf8() {
    let bytes: &[u8] = &[0xFF, 0xFE, 0x00];
    let result = unsafe {
        reovim_set_register(b'a', bytes.as_ptr().cast(), ReovimYankType::Characterwise)
    };
    assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
}

// ========================================================================
// With-runtime tests (RuntimeGuard + TestSessionRuntime)
// ========================================================================

#[test]
fn test_get_register_empty_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let mut result = ReovimStringResult::ok(0);
    let ret = unsafe {
        reovim_get_register(
            b'a',
            std::ptr::null_mut(),
            0,
            &raw mut result,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(ret, REOVIM_ERR_NOT_FOUND);
}

#[test]
fn test_set_register_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let text = c"register content";
    let ret =
        unsafe { reovim_set_register(b'a', text.as_ptr(), ReovimYankType::Characterwise) };
    assert_eq!(ret, REOVIM_OK);
}

#[test]
fn test_get_register_after_set_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    // Set register 'a'
    let text = c"hello reg";
    assert_eq!(
        unsafe { reovim_set_register(b'a', text.as_ptr(), ReovimYankType::Linewise) },
        REOVIM_OK
    );

    // Get register 'a'
    let mut buf = [0u8; 64];
    let mut result = ReovimStringResult::err(0);
    let mut yank_type = ReovimYankType::Characterwise;
    let ret = unsafe {
        reovim_get_register(b'a', buf.as_mut_ptr(), 64, &raw mut result, &raw mut yank_type)
    };
    assert_eq!(ret, REOVIM_OK);
    assert_eq!(&buf[..result.length as usize], b"hello reg");
    assert_eq!(yank_type, ReovimYankType::Linewise);
}
