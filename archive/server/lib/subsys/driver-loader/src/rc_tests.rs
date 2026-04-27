//! Stub-vtable tests for the FFI-rc helpers.
//!
//! Mirrors the client-render `client_render_tests.rs` rc coverage:
//! every code path through [`classify_rc`], [`translate_rc`],
//! [`read_and_free_error`], and [`load_to_scan_error`] gets exercised
//! against a stub `NetGrpcVTable` whose `destroy_error_string` slot
//! records call counts in a static `AtomicUsize`.

use {
    super::*,
    crate::error::{LoadError, ScanEntryError, ValidationError},
    reovim_kernel::api::v1::Version,
    reovim_subsys_net::abi::{
        FfiServiceDescriptor, FfiTransportConfig, NetGrpcDriverProbe, NetGrpcVTable, ShutdownFd,
    },
    std::{
        ffi::{CString, c_char, c_int, c_void},
        sync::atomic::{AtomicU16, AtomicUsize, Ordering},
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

unsafe extern "C" fn dummy_probe() -> NetGrpcDriverProbe {
    NetGrpcDriverProbe::new("x", "x")
}
unsafe extern "C" fn dummy_construct(_: *mut *mut c_void, _: *mut *mut c_char) -> c_int {
    -1
}
unsafe extern "C" fn dummy_serve(
    _: *mut c_void,
    _: *const FfiTransportConfig,
    _: *const FfiServiceDescriptor,
    _: usize,
    _: ShutdownFd,
    _: ShutdownFd,
    _: *const AtomicU16,
    _: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn dummy_shutdown(_: *mut c_void, _: *mut *mut c_char) -> c_int {
    0
}
unsafe extern "C" fn dummy_destroy(_: *mut c_void) {}

fn driver_vtable() -> NetGrpcVTable {
    NetGrpcVTable {
        abi_version: 1,
        api_version: Version::new(1, 0, 0),
        size_of_self: std::mem::size_of::<NetGrpcVTable>(),
        probe: dummy_probe,
        construct: dummy_construct,
        serve: dummy_serve,
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
    // Any rc outside {0, -2} routes through the error-message branch.
    let vt = driver_vtable();
    let err = translate_rc(-3, std::ptr::null_mut(), &vt).unwrap_err();
    assert!(matches!(err, LoadError::DriverError(_)));
}

#[test]
fn classify_rc_ok_path_returns_ok_variant() {
    let vt = driver_vtable();
    assert!(matches!(classify_rc(0, std::ptr::null_mut(), &vt), RcOutcome::Ok));
}

#[test]
fn classify_rc_panic_path_returns_panicked_variant() {
    let vt = driver_vtable();
    assert!(matches!(classify_rc(-2, std::ptr::null_mut(), &vt), RcOutcome::Panicked));
}

#[test]
fn classify_rc_error_path_returns_error_variant() {
    let vt = driver_vtable();
    let owned = CString::new("oops").unwrap().into_raw();
    match classify_rc(-1, owned, &vt) {
        RcOutcome::Error(m) => assert_eq!(m, "oops"),
        _ => panic!("expected RcOutcome::Error"),
    }
}

#[test]
fn load_to_scan_error_library_open_routes_to_abi_mismatch_null() {
    let e = load_to_scan_error(LoadError::LibraryOpen("missing symbol".into()));
    assert!(matches!(e, ScanEntryError::AbiMismatch(ValidationError::VtablePointerNull)));
}

#[test]
fn load_to_scan_error_validation_routes_to_abi_mismatch_inner() {
    let e = load_to_scan_error(LoadError::Validation(ValidationError::AbiVersionMismatch {
        found: 2,
        expected: 1,
    }));
    assert!(matches!(
        e,
        ScanEntryError::AbiMismatch(ValidationError::AbiVersionMismatch { .. })
    ));
}

#[test]
fn load_to_scan_error_driver_error_routes_to_driver_error() {
    let e = load_to_scan_error(LoadError::DriverError("boom".into()));
    match e {
        ScanEntryError::DriverError(m) => assert_eq!(m, "boom"),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn load_to_scan_error_driver_panicked_routes_to_driver_panicked() {
    let e = load_to_scan_error(LoadError::DriverPanicked);
    assert!(matches!(e, ScanEntryError::DriverPanicked));
}
