//! Stub-vtable tests for loader code paths that do not require dlopen.
//!
//! The real `load_from_path` + drop-order paths are covered by the
//! integration test `tests/dlopen_roundtrip.rs`, which exercises a
//! real cdylib built as part of the workspace.

use {
    super::*,
    reovim_client_subsys_render::{
        abi::{ClientRenderVTable, RenderTargetVTable},
        client_render::ClientRenderDriverProbe,
    },
    reovim_kernel::api::v1::Version,
    std::{
        ffi::{CString, c_char, c_int, c_void},
        sync::atomic::{AtomicUsize, Ordering},
    },
};

static FREE_COUNTER: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn counting_free_err(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: ptr is a driver-allocated CString.
        unsafe { drop(CString::from_raw(ptr)) };
        FREE_COUNTER.fetch_add(1, Ordering::SeqCst);
    }
}

unsafe extern "C" fn dummy_probe() -> ClientRenderDriverProbe {
    ClientRenderDriverProbe::new("x", "x")
}
unsafe extern "C" fn dummy_construct(
    _: *mut c_void,
    _: *mut *mut c_void,
    _: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn dummy_target(
    _: *mut c_void,
    _: *mut *const RenderTargetVTable,
    _: *mut *mut c_void,
    _: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn dummy_shutdown(_: *mut c_void, _: *mut *mut c_char) -> c_int {
    0
}
unsafe extern "C" fn dummy_destroy(_: *mut c_void) {}

fn driver_vtable() -> ClientRenderVTable {
    ClientRenderVTable {
        abi_version: 1,
        api_version: Version::new(1, 0, 0),
        size_of_self: std::mem::size_of::<ClientRenderVTable>(),
        probe: dummy_probe,
        construct: dummy_construct,
        target: dummy_target,
        shutdown: dummy_shutdown,
        destroy: dummy_destroy,
        destroy_error_string: counting_free_err,
    }
}

#[test]
fn read_and_free_error_returns_placeholder_on_null() {
    let vt = driver_vtable();
    let msg = read_and_free_error(&vt, std::ptr::null_mut());
    assert_eq!(msg, "<no error message>");
}

#[test]
fn read_and_free_error_round_trips_driver_cstring() {
    let before = FREE_COUNTER.load(Ordering::SeqCst);
    let vt = driver_vtable();
    let owned = CString::new("boom").unwrap().into_raw();
    let msg = read_and_free_error(&vt, owned);
    assert_eq!(msg, "boom");
    assert_eq!(FREE_COUNTER.load(Ordering::SeqCst), before + 1);
}

unsafe extern "C" fn ok_submit(
    _: *mut c_void,
    _: *const u8,
    _: usize,
    _: *mut *mut c_char,
) -> c_int {
    0
}

unsafe extern "C" fn err_submit(
    _: *mut c_void,
    _: *const u8,
    _: usize,
    out_err: *mut *mut c_char,
) -> c_int {
    let owned = CString::new("submit failed").unwrap().into_raw();
    // SAFETY: caller guarantees out_err is a writable `*mut *mut c_char`.
    unsafe { *out_err = owned };
    -1
}

unsafe extern "C" fn panic_submit(
    _: *mut c_void,
    _: *const u8,
    _: usize,
    _: *mut *mut c_char,
) -> c_int {
    -2
}

fn target_vtable(
    submit: unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut *mut c_char) -> c_int,
) -> RenderTargetVTable {
    RenderTargetVTable {
        abi_version: 1,
        size_of_self: std::mem::size_of::<RenderTargetVTable>(),
        submit,
    }
}

#[test]
fn loaded_render_target_submit_ok() {
    let drv = Box::leak(Box::new(driver_vtable()));
    let tvt = target_vtable(ok_submit);
    let mut t = LoadedRenderTarget {
        vtable: &raw const tvt,
        handle: std::ptr::null_mut(),
        driver_vtable: drv,
        _borrow: PhantomData,
    };
    t.submit(b"frame").unwrap();
}

#[test]
fn loaded_render_target_submit_err_round_trips_string() {
    let before = FREE_COUNTER.load(Ordering::SeqCst);
    let drv = Box::leak(Box::new(driver_vtable()));
    let tvt = target_vtable(err_submit);
    let mut t = LoadedRenderTarget {
        vtable: &raw const tvt,
        handle: std::ptr::null_mut(),
        driver_vtable: drv,
        _borrow: PhantomData,
    };
    let err = t.submit(b"frame").unwrap_err();
    assert!(matches!(err, RenderError::InvalidData(ref m) if m == "submit failed"));
    assert_eq!(FREE_COUNTER.load(Ordering::SeqCst), before + 1);
}

#[test]
fn loaded_render_target_submit_panic_surfaces_render_error() {
    let drv = Box::leak(Box::new(driver_vtable()));
    let tvt = target_vtable(panic_submit);
    let mut t = LoadedRenderTarget {
        vtable: &raw const tvt,
        handle: std::ptr::null_mut(),
        driver_vtable: drv,
        _borrow: PhantomData,
    };
    let err = t.submit(b"frame").unwrap_err();
    assert!(matches!(err, RenderError::InvalidData(ref m) if m.contains("panicked")));
}

#[test]
fn translate_rc_success_with_null_err_returns_ok() {
    let vt = driver_vtable();
    translate_rc(0, std::ptr::null_mut(), &vt).unwrap();
}

#[test]
fn translate_rc_success_with_spurious_err_frees_string() {
    let before = FREE_COUNTER.load(Ordering::SeqCst);
    let vt = driver_vtable();
    let owned = CString::new("spurious").unwrap().into_raw();
    translate_rc(0, owned, &vt).unwrap();
    assert_eq!(FREE_COUNTER.load(Ordering::SeqCst), before + 1);
}

#[test]
fn translate_rc_panic_with_null_err_returns_driver_panicked() {
    let vt = driver_vtable();
    let err = translate_rc(-2, std::ptr::null_mut(), &vt).unwrap_err();
    assert!(matches!(err, LoadError::DriverPanicked));
}

#[test]
fn translate_rc_panic_with_non_null_err_frees_and_returns_panicked() {
    let before = FREE_COUNTER.load(Ordering::SeqCst);
    let vt = driver_vtable();
    let owned = CString::new("panic-with-msg").unwrap().into_raw();
    let err = translate_rc(-2, owned, &vt).unwrap_err();
    assert!(matches!(err, LoadError::DriverPanicked));
    assert_eq!(FREE_COUNTER.load(Ordering::SeqCst), before + 1);
}

#[test]
fn translate_rc_error_with_msg_round_trips_string() {
    let before = FREE_COUNTER.load(Ordering::SeqCst);
    let vt = driver_vtable();
    let owned = CString::new("construct failed").unwrap().into_raw();
    let err = translate_rc(-1, owned, &vt).unwrap_err();
    match err {
        LoadError::DriverError(m) => assert_eq!(m, "construct failed"),
        other => panic!("expected DriverError, got {other:?}"),
    }
    assert_eq!(FREE_COUNTER.load(Ordering::SeqCst), before + 1);
}

#[test]
fn translate_rc_error_with_null_err_returns_placeholder() {
    let vt = driver_vtable();
    let err = translate_rc(-1, std::ptr::null_mut(), &vt).unwrap_err();
    match err {
        LoadError::DriverError(m) => assert_eq!(m, "<no error message>"),
        other => panic!("expected DriverError, got {other:?}"),
    }
}

#[test]
fn translate_rc_contract_violation_returns_driver_error() {
    let vt = driver_vtable();
    let err = translate_rc(-3, std::ptr::null_mut(), &vt).unwrap_err();
    assert!(matches!(err, LoadError::DriverError(_)));
}

unsafe extern "C" fn target_returns_null_vtable(
    _instance: *mut c_void,
    out_vtable: *mut *const RenderTargetVTable,
    out_handle: *mut *mut c_void,
    _out_err: *mut *mut c_char,
) -> c_int {
    // SAFETY: caller guarantees writable out-params.
    unsafe {
        *out_vtable = std::ptr::null();
        *out_handle = std::ptr::null_mut();
    }
    0
}

#[test]
fn call_target_null_sub_vtable_surfaces_validation_error() {
    let mut vt = driver_vtable();
    vt.target = target_returns_null_vtable;
    let err = call_target(&vt, std::ptr::null_mut()).unwrap_err();
    assert!(matches!(
        err,
        LoadError::Validation(crate::error::ValidationError::VtablePointerNull)
    ));
}
