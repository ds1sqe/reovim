//! Stub-vtable tests for loader code paths that do not require dlopen.
//!
//! The real `load_from_path` + observer-drop paths are covered by the
//! integration test `tests/debug_dlopen_roundtrip.rs`, which exercises
//! a real cdylib built as part of the workspace.

use {
    super::*,
    crate::rc::{read_and_free_error, translate_rc},
    reovim_client_subsys_debug::{
        abi::{ClientDebugVTable, DebugObserverVTable},
        client_debug::DebugProbe,
    },
    reovim_kernel::api::v1::Version,
    std::{
        ffi::{CString, c_char, c_int, c_void},
        sync::atomic::{AtomicUsize, Ordering},
    },
};

static FREE_ERROR_COUNTER: AtomicUsize = AtomicUsize::new(0);
static FREE_BYTES_COUNTER: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn counting_free_err(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: ptr is a driver-allocated CString.
        unsafe { drop(CString::from_raw(ptr)) };
        FREE_ERROR_COUNTER.fetch_add(1, Ordering::SeqCst);
    }
}

unsafe extern "C" fn counting_free_bytes(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        // SAFETY: sibling stub allocators always produced (ptr, len)
        // via `Vec::into_raw_parts()` after `shrink_to_fit()`, so
        // capacity == len. This is the ABI-required reconstruction
        // idiom for the two-arg `destroy_bytes(ptr, len)` slot.
        #[allow(clippy::same_length_and_capacity)]
        unsafe {
            drop(Vec::from_raw_parts(ptr, len, len));
        }
        FREE_BYTES_COUNTER.fetch_add(1, Ordering::SeqCst);
    }
}

unsafe extern "C" fn stub_probe() -> DebugProbe {
    DebugProbe::new("stub", "stub", &[], &[])
}
unsafe extern "C" fn stub_construct(
    _: *mut c_void,
    _: *mut *mut c_void,
    _: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn stub_observe_start(
    _: *mut c_void,
    _: *const u8,
    _: usize,
    _: *mut *const DebugObserverVTable,
    _: *mut *mut c_void,
    _: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn stub_drive(
    _: *mut c_void,
    _: *const u8,
    _: usize,
    _: *mut *mut u8,
    _: *mut usize,
    _: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn stub_shutdown(_: *mut c_void, _: *mut *mut c_char) -> c_int {
    0
}
unsafe extern "C" fn stub_destroy(_: *mut c_void) {}

fn driver_vtable() -> ClientDebugVTable {
    ClientDebugVTable {
        abi_version: 1,
        api_version: Version::new(1, 0, 0),
        size_of_self: std::mem::size_of::<ClientDebugVTable>(),
        probe: stub_probe,
        construct: stub_construct,
        observe_start: stub_observe_start,
        drive: stub_drive,
        shutdown: stub_shutdown,
        destroy: stub_destroy,
        destroy_error_string: counting_free_err,
        destroy_bytes: counting_free_bytes,
    }
}

#[test]
fn read_and_free_error_on_debug_vtable_returns_placeholder_on_null() {
    let vt = driver_vtable();
    let msg = read_and_free_error(&vt, std::ptr::null_mut());
    assert_eq!(msg, "<no error message>");
}

#[test]
fn read_and_free_error_on_debug_vtable_round_trips_cstring() {
    let before = FREE_ERROR_COUNTER.load(Ordering::SeqCst);
    let vt = driver_vtable();
    let owned = CString::new("boom").unwrap().into_raw();
    let msg = read_and_free_error(&vt, owned);
    assert_eq!(msg, "boom");
    assert_eq!(FREE_ERROR_COUNTER.load(Ordering::SeqCst), before + 1);
}

#[test]
fn translate_rc_success_on_debug_vtable() {
    let vt = driver_vtable();
    translate_rc(0, std::ptr::null_mut(), &vt).unwrap();
}

#[test]
fn translate_rc_panic_on_debug_vtable() {
    let vt = driver_vtable();
    let err = translate_rc(-2, std::ptr::null_mut(), &vt).unwrap_err();
    assert!(matches!(err, LoadError::DriverPanicked));
}

#[test]
fn translate_rc_error_on_debug_vtable() {
    let before = FREE_ERROR_COUNTER.load(Ordering::SeqCst);
    let vt = driver_vtable();
    let owned = CString::new("drive failed").unwrap().into_raw();
    let err = translate_rc(-1, owned, &vt).unwrap_err();
    match err {
        LoadError::DriverError(m) => assert_eq!(m, "drive failed"),
        other => panic!("expected DriverError, got {other:?}"),
    }
    assert_eq!(FREE_ERROR_COUNTER.load(Ordering::SeqCst), before + 1);
}

#[test]
fn copy_and_free_bytes_on_null_pointer_returns_empty_vec() {
    let vt = driver_vtable();
    let out = copy_and_free_bytes(&vt, std::ptr::null_mut(), 0);
    assert!(out.is_empty());
}

#[test]
fn copy_and_free_bytes_round_trips_driver_allocation() {
    let before = FREE_BYTES_COUNTER.load(Ordering::SeqCst);
    let vt = driver_vtable();
    let mut source = vec![0xAA_u8, 0xBB, 0xCC];
    source.shrink_to_fit();
    let (ptr, len) = {
        let mut md = std::mem::ManuallyDrop::new(source);
        (md.as_mut_ptr(), md.len())
    };
    let out = copy_and_free_bytes(&vt, ptr, len);
    assert_eq!(out, vec![0xAA, 0xBB, 0xCC]);
    assert_eq!(FREE_BYTES_COUNTER.load(Ordering::SeqCst), before + 1);
}

unsafe extern "C" fn observe_start_returns_null_vtable(
    _instance: *mut c_void,
    _selector: *const u8,
    _selector_len: usize,
    out_vtable: *mut *const DebugObserverVTable,
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
fn call_observe_start_null_sub_vtable_surfaces_validation_error() {
    let mut vt = driver_vtable();
    vt.observe_start = observe_start_returns_null_vtable;
    let err = call_observe_start(&vt, std::ptr::null_mut(), b"schema").unwrap_err();
    assert!(matches!(
        err,
        LoadError::Validation(crate::error::ValidationError::VtablePointerNull)
    ));
}
