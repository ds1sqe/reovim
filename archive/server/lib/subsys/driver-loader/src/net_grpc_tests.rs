//! Stub-vtable tests for [`LoadedNetGrpc`] code paths that do not
//! require dlopen.
//!
//! The real `load_from_path` + drop-order paths are covered by the
//! integration test `tests/net_grpc_dlopen_roundtrip.rs`, which
//! exercises the workspace-built `reovim-driver-net-grpc` cdylib.
//!
//! These tests target [`invoke_serve`] (the lifetime-free helper that
//! [`LoadedNetGrpc::serve`] delegates to) so the test surface does not
//! need to fabricate a `Library` handle.

use {
    super::*,
    reovim_kernel::api::v1::Version,
    reovim_subsys_net::{
        NetError, ServiceDescriptor, TransportConfig,
        abi::{
            FfiServiceDescriptor, FfiTransportConfig, NetGrpcDriverProbe, NetGrpcVTable, ShutdownFd,
        },
    },
    std::{
        ffi::{CString, c_char, c_int, c_void},
        sync::atomic::{AtomicU16, Ordering},
    },
};

// ────────────────────────────────────────────────────────────────────
// Stub trampolines. Destroy and shutdown are no-ops because the unit
// tests exercise `invoke_serve` directly without a `LoadedNetGrpc`
// wrapper, so neither slot is invoked.
// ────────────────────────────────────────────────────────────────────

static OK_SERVE_PORT_WRITEBACK: AtomicU16 = AtomicU16::new(0);

unsafe extern "C" fn stub_probe() -> NetGrpcDriverProbe {
    NetGrpcDriverProbe::new("net_grpc", "stub")
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
        // SAFETY: ptr is a stub-allocated CString (via CString::into_raw).
        unsafe { drop(CString::from_raw(ptr)) };
    }
}

unsafe extern "C" fn stub_shutdown(_instance: *mut c_void, _out_err: *mut *mut c_char) -> c_int {
    0
}

unsafe extern "C" fn ok_serve(
    _instance: *mut c_void,
    _config: *const FfiTransportConfig,
    _descs_ptr: *const FfiServiceDescriptor,
    _descs_len: usize,
    _shutdown_fd: ShutdownFd,
    _bind_ready_fd: ShutdownFd,
    port_writeback: *const AtomicU16,
    _out_err: *mut *mut c_char,
) -> c_int {
    if !port_writeback.is_null() {
        // SAFETY: caller guarantees the AtomicU16 is `'static`.
        unsafe { (*port_writeback).store(54321, Ordering::Release) };
    }
    0
}

unsafe extern "C" fn err_serve(
    _instance: *mut c_void,
    _config: *const FfiTransportConfig,
    _descs_ptr: *const FfiServiceDescriptor,
    _descs_len: usize,
    _shutdown_fd: ShutdownFd,
    _bind_ready_fd: ShutdownFd,
    _port_writeback: *const AtomicU16,
    out_err: *mut *mut c_char,
) -> c_int {
    let owned = CString::new("serve failed").unwrap().into_raw();
    // SAFETY: caller guarantees out_err is a writable `*mut *mut c_char`.
    unsafe { *out_err = owned };
    -1
}

unsafe extern "C" fn panic_serve(
    _instance: *mut c_void,
    _config: *const FfiTransportConfig,
    _descs_ptr: *const FfiServiceDescriptor,
    _descs_len: usize,
    _shutdown_fd: ShutdownFd,
    _bind_ready_fd: ShutdownFd,
    _port_writeback: *const AtomicU16,
    _out_err: *mut *mut c_char,
) -> c_int {
    -2
}

fn make_vtable(
    serve: unsafe extern "C" fn(
        *mut c_void,
        *const FfiTransportConfig,
        *const FfiServiceDescriptor,
        usize,
        ShutdownFd,
        ShutdownFd,
        *const AtomicU16,
        *mut *mut c_char,
    ) -> c_int,
) -> NetGrpcVTable {
    NetGrpcVTable {
        abi_version: 1,
        api_version: Version::new(1, 0, 0),
        size_of_self: std::mem::size_of::<NetGrpcVTable>(),
        probe: stub_probe,
        construct: stub_construct,
        serve,
        shutdown: stub_shutdown,
        destroy: stub_destroy,
        destroy_error_string: stub_destroy_err,
    }
}

// ────────────────────────────────────────────────────────────────────
// Tests for `invoke_serve`.
// ────────────────────────────────────────────────────────────────────

#[test]
fn invoke_serve_ok_path_writes_port_and_returns_unit() {
    OK_SERVE_PORT_WRITEBACK.store(0, Ordering::SeqCst);
    let vt = make_vtable(ok_serve);
    let cfg = TransportConfig::tcp_localhost(0);
    let writeback: &'static AtomicU16 = &OK_SERVE_PORT_WRITEBACK;
    let result = invoke_serve(
        &vt,
        std::ptr::null_mut(),
        &cfg,
        Vec::<ServiceDescriptor>::new(),
        ShutdownFd(-1),
        ShutdownFd(-1),
        writeback,
    );
    result.expect("ok serve should return Ok(())");
    assert_eq!(OK_SERVE_PORT_WRITEBACK.load(Ordering::Acquire), 54321);
}

#[test]
fn invoke_serve_driver_error_round_trips_message() {
    let vt = make_vtable(err_serve);
    let cfg = TransportConfig::tcp_localhost(0);
    let writeback: &'static AtomicU16 = Box::leak(Box::new(AtomicU16::new(0)));
    let err = invoke_serve(
        &vt,
        std::ptr::null_mut(),
        &cfg,
        Vec::<ServiceDescriptor>::new(),
        ShutdownFd(-1),
        ShutdownFd(-1),
        writeback,
    )
    .unwrap_err();
    match err {
        NetError::ServeFailed(m) => assert_eq!(m, "serve failed"),
        other => panic!("expected ServeFailed, got {other:?}"),
    }
}

#[test]
fn invoke_serve_driver_panicked_returns_io_error() {
    let vt = make_vtable(panic_serve);
    let cfg = TransportConfig::tcp_localhost(0);
    let writeback: &'static AtomicU16 = Box::leak(Box::new(AtomicU16::new(0)));
    let err = invoke_serve(
        &vt,
        std::ptr::null_mut(),
        &cfg,
        Vec::<ServiceDescriptor>::new(),
        ShutdownFd(-1),
        ShutdownFd(-1),
        writeback,
    )
    .unwrap_err();
    match err {
        NetError::Io(m) => assert!(m.contains("panicked"), "expected panic message, got: {m}"),
        other => panic!("expected NetError::Io, got {other:?}"),
    }
}

#[test]
fn invoke_serve_with_non_empty_descriptors_rejects_with_sp02b_marker() {
    // Build a single ServiceDescriptor — content does not matter; the
    // empty-list guard must reject before any FFI call happens.
    let svc = tower::util::BoxCloneService::new(DummyService);
    let desc = ServiceDescriptor::new("reovim.v3.BufferService", svc);

    let vt = make_vtable(ok_serve);
    let cfg = TransportConfig::tcp_localhost(0);
    let writeback: &'static AtomicU16 = Box::leak(Box::new(AtomicU16::new(0)));
    let err = invoke_serve(
        &vt,
        std::ptr::null_mut(),
        &cfg,
        vec![desc],
        ShutdownFd(-1),
        ShutdownFd(-1),
        writeback,
    )
    .unwrap_err();
    match err {
        NetError::Io(m) => {
            assert!(m.contains("SP02b"), "expected SP02b marker, got: {m}");
            assert!(m.contains("dispatch_call"), "expected dispatch_call mention, got: {m}");
        }
        other => panic!("expected NetError::Io, got {other:?}"),
    }
}

/// Trivial tower service used only to satisfy `BoxCloneService::new`'s
/// `Service` bound. Never called — the non-empty-descriptor guard in
/// `invoke_serve` rejects before any FFI dispatch happens.
#[derive(Clone)]
struct DummyService;

impl tower::Service<http::Request<tonic::body::BoxBody>> for DummyService {
    type Response = http::Response<tonic::body::BoxBody>;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: http::Request<tonic::body::BoxBody>) -> Self::Future {
        Box::pin(async {
            use http_body_util::{BodyExt, Empty};
            let empty = Empty::<bytes::Bytes>::new();
            let mapped = BodyExt::map_err(empty, |never| match never {});
            Ok(http::Response::new(tonic::body::BoxBody::new(mapped)))
        })
    }
}

// ────────────────────────────────────────────────────────────────────
// Deferred dispatch-panic test (tracked at SP02b).
//
// Once dispatch_call marshalling lands in a follow-on, the empty-list
// guard in `invoke_serve` is replaced with a translation step. At that
// point the test below exercises a vtable whose `dispatch_call` panics
// to verify the host doesn't crash. Today there is no dispatch FFI to
// test, so the test stays `#[ignore]`d as a placeholder.
// ────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "TODO(SP02b): re-enable once dispatch_call marshalling lands"]
fn dispatch_panic_does_not_unwind_across_ffi_boundary() {
    // Once SP02b lands, this test should:
    //   1. Build a stub `FfiServiceDescriptor` whose `dispatch_call`
    //      trampoline calls `panic!()`.
    //   2. Invoke `LoadedNetGrpc::serve` with that descriptor list and
    //      a `serve` slot that synchronously calls `dispatch_call`
    //      once before returning 0.
    //   3. Assert the host call returns `Ok(())` — the
    //      `catch_unwind` in the driver-side dispatch trampoline
    //      converts the panic into a `tonic::Status::internal` and
    //      keeps the host alive.
}
