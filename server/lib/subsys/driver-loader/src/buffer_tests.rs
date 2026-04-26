//! Stub-vtable tests for [`LoadedBuffer`] code paths that do not
//! require dlopen.
//!
//! Mirrors the `net_grpc_tests.rs` pattern: each test hand-builds a
//! `BufferVTable` whose slots route through Rust trampolines that
//! record their call counts. The `invoke_*` helpers under test take a
//! `&BufferVTable` + a sentinel `*mut c_void` instance, so no real
//! `Library` is needed — the test surface lives entirely in this
//! crate.
//!
//! The dlopen + drop-order paths are covered by the integration test
//! `tests/buffer_dlopen_roundtrip.rs`, which exercises the workspace-
//! built `reovim-driver-text-buffer` cdylib.

use {
    super::*,
    crate::validation::BufferExpectations,
    reovim_kernel::api::v1::{BufferId, Version},
    reovim_subsys_buffer::{
        BufferError,
        abi::{BufferDriverProbe, BufferVTable, FfiByteEdit, FfiCodecAttachment, FfiCodecName},
    },
    std::{
        ffi::{CString, c_char, c_int, c_void},
        sync::atomic::{AtomicUsize, Ordering},
    },
};

// ────────────────────────────────────────────────────────────────────
// Stub vtable bookkeeping. Each test resets the relevant atomics to
// avoid order dependence with the cargo test harness.
// ────────────────────────────────────────────────────────────────────

/// `0` = ok mode, `1` = error mode (writes a `CString` into `out_err`).
static STUB_MODE: AtomicUsize = AtomicUsize::new(0);
/// Counts `vtable.destroy_id_list` invocations.
static DESTROY_ID_LIST_CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn stub_probe() -> BufferDriverProbe {
    BufferDriverProbe::new("buffer", "stub")
}

unsafe extern "C" fn stub_construct(
    _out_instance: *mut *mut c_void,
    _out_err: *mut *mut c_char,
) -> c_int {
    -1
}

unsafe extern "C" fn stub_destroy(_instance: *mut c_void) {}

unsafe extern "C" fn stub_destroy_err(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: stub-allocated CString.
        unsafe { drop(CString::from_raw(ptr)) };
    }
}

fn write_err(out_err: *mut *mut c_char, msg: &str) {
    if out_err.is_null() {
        return;
    }
    let cs = CString::new(msg).expect("msg has no nul");
    // SAFETY: out_err is a writable host slot.
    unsafe { *out_err = cs.into_raw() };
}

/// Sentinel pointer for stub buffer handles. The unit tests never
/// dereference it; they only assert non-null.
static SENTINEL_BYTE: u8 = 0;

fn sentinel_handle() -> *mut c_void {
    (&raw const SENTINEL_BYTE).cast::<c_void>().cast_mut()
}

unsafe extern "C" fn stub_create_buffer(
    _instance: *mut c_void,
    _bytes_ptr: *const u8,
    _bytes_len: usize,
    _path_ptr: *const c_char,
    _path_len: usize,
    out_buffer: *mut *mut c_void,
    out_id: *mut u64,
    out_err: *mut *mut c_char,
) -> c_int {
    if STUB_MODE.load(Ordering::SeqCst) == 1 {
        write_err(out_err, "create_buffer rejected");
        return -1;
    }
    // SAFETY: writable host slots.
    unsafe {
        *out_buffer = sentinel_handle();
        *out_id = 7;
    }
    0
}

unsafe extern "C" fn stub_open_buffer(
    _instance: *mut c_void,
    _path_ptr: *const c_char,
    _path_len: usize,
    out_buffer: *mut *mut c_void,
    out_id: *mut u64,
    out_err: *mut *mut c_char,
) -> c_int {
    if STUB_MODE.load(Ordering::SeqCst) == 1 {
        write_err(out_err, "open_buffer rejected");
        return -1;
    }
    // SAFETY: writable host slots.
    unsafe {
        *out_buffer = sentinel_handle();
        *out_id = 9;
    }
    0
}

unsafe extern "C" fn stub_list_buffers(
    _instance: *mut c_void,
    out_ids: *mut *mut u64,
    out_count: *mut usize,
    _out_err: *mut *mut c_char,
) -> c_int {
    let ids: Box<[u64]> = vec![1u64, 2, 3].into_boxed_slice();
    let count = ids.len();
    let ptr = Box::into_raw(ids).cast::<u64>();
    // SAFETY: writable host slots.
    unsafe {
        *out_ids = ptr;
        *out_count = count;
    }
    0
}

unsafe extern "C" fn stub_list_buffers_panic(
    _instance: *mut c_void,
    _out_ids: *mut *mut u64,
    _out_count: *mut usize,
    _out_err: *mut *mut c_char,
) -> c_int {
    // Simulate a caught-panic return code.
    -2
}

unsafe extern "C" fn stub_get_buffer(
    _instance: *mut c_void,
    id: u64,
    out_buffer: *mut *mut c_void,
    _out_err: *mut *mut c_char,
) -> c_int {
    if id == 0 {
        // SAFETY: writable host slot.
        unsafe { *out_buffer = std::ptr::null_mut() };
        return -1;
    }
    // SAFETY: writable host slot — sentinel pointer.
    unsafe { *out_buffer = sentinel_handle() };
    0
}

unsafe extern "C" fn stub_close_buffer(
    _instance: *mut c_void,
    _id: u64,
    out_err: *mut *mut c_char,
) -> c_int {
    if STUB_MODE.load(Ordering::SeqCst) == 1 {
        write_err(out_err, "close failed");
        return -1;
    }
    0
}

unsafe extern "C" fn stub_destroy_id_list(ids_ptr: *mut u64, count: usize) {
    DESTROY_ID_LIST_CALLS.fetch_add(1, Ordering::SeqCst);
    if ids_ptr.is_null() || count == 0 {
        return;
    }
    // SAFETY: produced by `Box::<[u64]>::into_raw` in `stub_list_buffers`.
    unsafe {
        let slice_ptr: *mut [u64] = std::ptr::slice_from_raw_parts_mut(ids_ptr, count);
        drop(Box::from_raw(slice_ptr));
    }
}

// Per-buffer slots (unused by these unit tests, but must be present so
// the test vtable matches `size_of::<BufferVTable>`).
unsafe extern "C" fn unused_buffer_id(_buffer: *mut c_void) -> u64 {
    0
}
unsafe extern "C" fn unused_buffer_size(_buffer: *mut c_void) -> usize {
    0
}
unsafe extern "C" fn unused_buffer_is_modified(_buffer: *mut c_void) -> u8 {
    0
}
unsafe extern "C" fn unused_buffer_file_path(
    _buffer: *mut c_void,
    _out_ptr: *mut *mut c_char,
    _out_len: *mut usize,
) -> c_int {
    -1
}
unsafe extern "C" fn unused_buffer_set_file_path(
    _buffer: *mut c_void,
    _path_ptr: *const c_char,
    _path_len: usize,
    _out_err: *mut *mut c_char,
) -> c_int {
    0
}
unsafe extern "C" fn unused_buffer_read_bytes(
    _buffer: *mut c_void,
    _start: usize,
    _end: usize,
    _out_ptr: *mut *mut u8,
    _out_len: *mut usize,
    _out_err: *mut *mut c_char,
) -> c_int {
    0
}
unsafe extern "C" fn unused_buffer_apply_edit(
    _buffer: *mut c_void,
    _edit: *const FfiByteEdit,
    _out_err: *mut *mut c_char,
) -> c_int {
    0
}
unsafe extern "C" fn unused_buffer_write_to(
    _buffer: *mut c_void,
    _write_cb: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> c_int,
    _ctx: *mut c_void,
    _out_err: *mut *mut c_char,
) -> c_int {
    0
}
unsafe extern "C" fn unused_buffer_attach_codec(
    _buffer: *mut c_void,
    _attachment: *const FfiCodecAttachment,
    _out_id: *mut u32,
    _out_err: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn unused_buffer_detach_codec(
    _buffer: *mut c_void,
    _id: u32,
    _out_attachment: *mut FfiCodecAttachment,
    _out_err: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn unused_buffer_list_codecs(
    _buffer: *mut c_void,
    _out_ids: *mut *mut u32,
    _out_names: *mut *mut FfiCodecName,
    _out_count: *mut usize,
    _out_err: *mut *mut c_char,
) -> c_int {
    0
}
unsafe extern "C" fn unused_buffer_subscribe(
    _buffer: *mut c_void,
    _subscriber_handle: *mut c_void,
    _subscriber_cb: unsafe extern "C" fn(*mut c_void, *const FfiByteEdit) -> c_int,
    _subscriber_destroy: unsafe extern "C" fn(*mut c_void),
    _out_subscription_id: *mut u32,
    _out_err: *mut *mut c_char,
) -> c_int {
    0
}
unsafe extern "C" fn unused_buffer_unsubscribe(
    _buffer: *mut c_void,
    _subscription_id: u32,
    _out_err: *mut *mut c_char,
) -> c_int {
    0
}
unsafe extern "C" fn unused_buffer_destroy(_buffer: *mut c_void) {}
unsafe extern "C" fn unused_destroy_byte_buffer(_ptr: *mut u8, _len: usize) {}
unsafe extern "C" fn unused_destroy_file_path(_ptr: *mut c_char, _len: usize) {}
unsafe extern "C" fn unused_destroy_codec_name_array(
    _ids_ptr: *mut u32,
    _names_ptr: *mut FfiCodecName,
    _count: usize,
) {
}

fn make_vtable() -> BufferVTable {
    make_vtable_with_list(stub_list_buffers)
}

fn make_vtable_with_list(
    list_buffers: unsafe extern "C" fn(
        *mut c_void,
        *mut *mut u64,
        *mut usize,
        *mut *mut c_char,
    ) -> c_int,
) -> BufferVTable {
    BufferVTable {
        abi_version: 1,
        api_version: Version::new(1, 0, 0),
        size_of_self: std::mem::size_of::<BufferVTable>(),
        probe: stub_probe,
        construct: stub_construct,
        create_buffer: stub_create_buffer,
        open_buffer: stub_open_buffer,
        list_buffers,
        get_buffer: stub_get_buffer,
        close_buffer: stub_close_buffer,
        destroy_id_list: stub_destroy_id_list,
        buffer_id: unused_buffer_id,
        buffer_size: unused_buffer_size,
        buffer_is_modified: unused_buffer_is_modified,
        buffer_file_path: unused_buffer_file_path,
        buffer_set_file_path: unused_buffer_set_file_path,
        buffer_read_bytes: unused_buffer_read_bytes,
        buffer_apply_edit: unused_buffer_apply_edit,
        buffer_write_to: unused_buffer_write_to,
        buffer_attach_codec: unused_buffer_attach_codec,
        buffer_detach_codec: unused_buffer_detach_codec,
        buffer_list_codecs: unused_buffer_list_codecs,
        buffer_subscribe_edits: unused_buffer_subscribe,
        buffer_unsubscribe_edits: unused_buffer_unsubscribe,
        buffer_destroy: unused_buffer_destroy,
        destroy: stub_destroy,
        destroy_error_string: stub_destroy_err,
        destroy_byte_buffer: unused_destroy_byte_buffer,
        destroy_file_path: unused_destroy_file_path,
        destroy_codec_name_array: unused_destroy_codec_name_array,
    }
}

fn null_instance() -> *mut c_void {
    std::ptr::null_mut()
}

// ────────────────────────────────────────────────────────────────────
// Tests for the free-function trampoline bodies.
// ────────────────────────────────────────────────────────────────────

#[test]
fn invoke_create_buffer_ok_returns_handle() {
    STUB_MODE.store(0, Ordering::SeqCst);
    let vt = make_vtable();
    let handle = invoke_create_buffer(&vt, null_instance(), b"hello", None).expect("ok");
    assert!(!handle.is_null());
}

#[test]
fn invoke_create_buffer_with_path_routes_path_through_ffi() {
    STUB_MODE.store(0, Ordering::SeqCst);
    let vt = make_vtable();
    let handle = invoke_create_buffer(&vt, null_instance(), b"x", Some("/tmp/y")).expect("ok");
    assert!(!handle.is_null());
}

#[test]
fn invoke_create_buffer_error_translates_to_buffer_error_driver() {
    STUB_MODE.store(1, Ordering::SeqCst);
    let vt = make_vtable();
    let err = invoke_create_buffer(&vt, null_instance(), b"x", None).expect_err("err");
    match err {
        BufferError::Driver(m) => assert!(m.contains("create_buffer rejected")),
        other => panic!("expected Driver, got {other:?}"),
    }
    STUB_MODE.store(0, Ordering::SeqCst);
}

#[test]
fn invoke_open_buffer_ok_returns_handle() {
    STUB_MODE.store(0, Ordering::SeqCst);
    let vt = make_vtable();
    let handle = invoke_open_buffer(&vt, null_instance(), "/tmp/x").expect("ok");
    assert!(!handle.is_null());
}

#[test]
fn invoke_open_buffer_error_translates() {
    STUB_MODE.store(1, Ordering::SeqCst);
    let vt = make_vtable();
    let err = invoke_open_buffer(&vt, null_instance(), "/tmp/x").expect_err("err");
    assert!(matches!(err, BufferError::Driver(_)));
    STUB_MODE.store(0, Ordering::SeqCst);
}

#[test]
fn invoke_list_buffers_returns_vec_and_calls_destructor() {
    DESTROY_ID_LIST_CALLS.store(0, Ordering::SeqCst);
    let vt = make_vtable();
    let ids = invoke_list_buffers(&vt, null_instance()).expect("ok");
    let raw_ids: Vec<usize> = ids.iter().map(|id| id.as_usize()).collect();
    assert_eq!(raw_ids, vec![1, 2, 3]);
    assert_eq!(DESTROY_ID_LIST_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn invoke_list_buffers_panic_translates_to_driver_error() {
    let vt = make_vtable_with_list(stub_list_buffers_panic);
    let err = invoke_list_buffers(&vt, null_instance()).expect_err("panic");
    match err {
        BufferError::Driver(m) => assert!(m.contains("panicked")),
        other => panic!("expected Driver(_), got {other:?}"),
    }
}

#[test]
fn invoke_get_buffer_miss_returns_none() {
    let vt = make_vtable();
    let h = invoke_get_buffer(&vt, null_instance(), BufferId::from_raw(0));
    assert!(h.is_none());
}

#[test]
fn invoke_get_buffer_hit_returns_some_handle() {
    let vt = make_vtable();
    let h = invoke_get_buffer(&vt, null_instance(), BufferId::from_raw(7)).expect("hit");
    assert!(!h.is_null());
}

#[test]
fn invoke_close_buffer_ok() {
    STUB_MODE.store(0, Ordering::SeqCst);
    let vt = make_vtable();
    invoke_close_buffer(&vt, null_instance(), BufferId::from_raw(1)).expect("ok");
}

#[test]
fn invoke_close_buffer_error_translates() {
    STUB_MODE.store(1, Ordering::SeqCst);
    let vt = make_vtable();
    let err = invoke_close_buffer(&vt, null_instance(), BufferId::from_raw(1)).expect_err("err");
    assert!(matches!(err, BufferError::Driver(_)));
    STUB_MODE.store(0, Ordering::SeqCst);
}

// ────────────────────────────────────────────────────────────────────
// Validation surface — null vtable.
// ────────────────────────────────────────────────────────────────────

#[test]
fn check_buffer_rejects_null_pointer() {
    use crate::{error::ValidationError, validation::check_buffer};
    let err = unsafe { check_buffer(std::ptr::null(), BufferExpectations::from_host()) }
        .expect_err("null");
    assert!(matches!(err, ValidationError::VtablePointerNull));
}

#[test]
fn check_buffer_accepts_matching_vtable() {
    use crate::validation::check_buffer;
    let vt = make_vtable();
    unsafe { check_buffer(&raw const vt, BufferExpectations::from_host()) }.unwrap();
}

#[test]
fn check_buffer_rejects_abi_mismatch() {
    use crate::{error::ValidationError, validation::check_buffer};
    let mut vt = make_vtable();
    vt.abi_version += 1;
    let err = unsafe { check_buffer(&raw const vt, BufferExpectations::from_host()) }
        .expect_err("abi mismatch");
    assert!(matches!(err, ValidationError::AbiVersionMismatch { .. }));
}

#[test]
fn check_buffer_rejects_size_mismatch() {
    use crate::{error::ValidationError, validation::check_buffer};
    let mut vt = make_vtable();
    vt.size_of_self += 8;
    let err = unsafe { check_buffer(&raw const vt, BufferExpectations::from_host()) }
        .expect_err("size mismatch");
    assert!(matches!(err, ValidationError::SizeOfSelfMismatch { .. }));
}
