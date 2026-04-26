//! Stub-vtable tests for the net-grpc validation paths.
//!
//! Mirrors the client-render validation coverage: every failure
//! variant of [`ValidationError`] gets an explicit test, plus the two
//! happy paths (matching vtable + api-minor-above-required).

use {
    super::*,
    crate::error::ValidationError,
    reovim_kernel::api::v1::Version,
    reovim_subsys_net::abi::{
        FfiServiceDescriptor, FfiTransportConfig, NetGrpcDriverProbe, NetGrpcVTable, ShutdownFd,
    },
    std::{
        ffi::{c_char, c_int, c_void},
        ptr,
        sync::atomic::AtomicU16,
    },
};

unsafe extern "C" fn stub_probe() -> NetGrpcDriverProbe {
    NetGrpcDriverProbe::new("stub", "stub")
}
unsafe extern "C" fn stub_construct(_: *mut *mut c_void, _: *mut *mut c_char) -> c_int {
    -1
}
unsafe extern "C" fn stub_serve(
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
unsafe extern "C" fn stub_shutdown(_: *mut c_void, _: *mut *mut c_char) -> c_int {
    0
}
unsafe extern "C" fn stub_destroy(_: *mut c_void) {}
unsafe extern "C" fn stub_destroy_err(_: *mut c_char) {}

fn vtable(abi: u32, api: Version, size: usize) -> NetGrpcVTable {
    NetGrpcVTable {
        abi_version: abi,
        api_version: api,
        size_of_self: size,
        probe: stub_probe,
        construct: stub_construct,
        serve: stub_serve,
        shutdown: stub_shutdown,
        destroy: stub_destroy,
        destroy_error_string: stub_destroy_err,
    }
}

fn host() -> NetGrpcExpectations {
    NetGrpcExpectations::from_host()
}

#[test]
fn rejects_null_pointer() {
    let err = unsafe { check_net_grpc(ptr::null(), host()) }.unwrap_err();
    assert!(matches!(err, ValidationError::VtablePointerNull));
}

#[test]
fn accepts_matching_vtable() {
    let h = host();
    let vt = vtable(h.abi_version, h.api_version, h.size_of_self);
    unsafe { check_net_grpc(&raw const vt, h) }.unwrap();
}

#[test]
fn rejects_abi_mismatch() {
    let h = host();
    let vt = vtable(h.abi_version + 1, h.api_version, h.size_of_self);
    let err = unsafe { check_net_grpc(&raw const vt, h) }.unwrap_err();
    assert!(matches!(err, ValidationError::AbiVersionMismatch { .. }));
}

#[test]
fn rejects_api_major_mismatch() {
    let h = host();
    let vt = vtable(h.abi_version, Version::new(h.api_version.major + 1, 0, 0), h.size_of_self);
    let err = unsafe { check_net_grpc(&raw const vt, h) }.unwrap_err();
    assert!(matches!(err, ValidationError::ApiVersionIncompatible { .. }));
}

#[test]
fn rejects_api_minor_below_required() {
    let h = NetGrpcExpectations {
        abi_version: 1,
        api_version: Version::new(1, 2, 0),
        size_of_self: std::mem::size_of::<NetGrpcVTable>(),
    };
    let vt = vtable(1, Version::new(1, 1, 0), h.size_of_self);
    let err = unsafe { check_net_grpc(&raw const vt, h) }.unwrap_err();
    assert!(matches!(err, ValidationError::ApiVersionIncompatible { .. }));
}

#[test]
fn accepts_api_minor_above_required() {
    let h = NetGrpcExpectations {
        abi_version: 1,
        api_version: Version::new(1, 0, 0),
        size_of_self: std::mem::size_of::<NetGrpcVTable>(),
    };
    let vt = vtable(1, Version::new(1, 9, 3), h.size_of_self);
    unsafe { check_net_grpc(&raw const vt, h) }.unwrap();
}

#[test]
fn rejects_size_mismatch() {
    let h = host();
    let vt = vtable(h.abi_version, h.api_version, h.size_of_self + 8);
    let err = unsafe { check_net_grpc(&raw const vt, h) }.unwrap_err();
    assert!(matches!(err, ValidationError::SizeOfSelfMismatch { .. }));
}
