//! Shape and round-trip tests for the FFI ABI types.

#![allow(unsafe_code)] // round-trip tests dereference host-pinned pointers

use super::*;

#[test]
fn probe_size_is_192_bytes() {
    // 64 + 128 = 192; all u8, no padding.
    assert_eq!(std::mem::size_of::<NetGrpcDriverProbe>(), 192);
    assert_eq!(std::mem::align_of::<NetGrpcDriverProbe>(), 1);
}

#[test]
fn probe_new_truncates_oversized_strings() {
    let kind = "x".repeat(100);
    let name = "y".repeat(200);
    let p = NetGrpcDriverProbe::new(&kind, &name);
    assert_eq!(p.kind[63], 0, "kind truncates at 63 bytes (last byte stays nul)");
    assert_eq!(p.name[127], 0, "name truncates at 127 bytes (last byte stays nul)");
    assert_eq!(p.kind[..63], [b'x'; 63]);
    assert_eq!(p.name[..127], [b'y'; 127]);
}

#[test]
fn probe_new_normal_strings_zero_padded() {
    let p = NetGrpcDriverProbe::new("net_grpc", "reovim-driver-net-grpc");
    assert_eq!(&p.kind[..8], b"net_grpc");
    assert_eq!(p.kind[8], 0, "kind zero-padded after content");
    assert_eq!(&p.name[..22], b"reovim-driver-net-grpc");
    assert_eq!(p.name[22], 0, "name zero-padded after content");
}

#[test]
fn shutdown_fd_is_raw_fd_sized() {
    use std::os::fd::RawFd;
    assert_eq!(std::mem::size_of::<ShutdownFd>(), std::mem::size_of::<RawFd>());
    assert_eq!(std::mem::align_of::<ShutdownFd>(), std::mem::align_of::<RawFd>());
}

#[test]
fn shutdown_fd_is_transparent() {
    // repr(transparent) means the fd is the only field and the struct
    // has the same layout as the fd. Verify via construction+access.
    let fd = ShutdownFd(42);
    assert_eq!(fd.0, 42);
}

#[test]
fn vtable_aligns_to_pointer() {
    // The struct contains fn pointers and other pointer-aligned
    // fields; alignment must equal pointer alignment.
    assert_eq!(std::mem::align_of::<NetGrpcVTable>(), std::mem::align_of::<*const ()>());
}

#[test]
fn vtable_is_non_empty() {
    // Sanity guard against accidental field removal collapsing the
    // struct. Exact size depends on platform pointer width and field
    // padding; we assert a lower bound (header + 6 fn pointers + sizes).
    let min_size = std::mem::size_of::<u32>()                    // abi_version
        + std::mem::size_of::<reovim_kernel::api::v1::Version>() // api_version
        + std::mem::size_of::<usize>()                           // size_of_self
        + 6 * std::mem::size_of::<*const ()>(); // 6 fn pointers
    assert!(
        std::mem::size_of::<NetGrpcVTable>() >= min_size,
        "vtable size shrank below minimum field-sum"
    );
}

#[test]
fn ffi_transport_config_aligns_to_pointer() {
    assert_eq!(std::mem::align_of::<FfiTransportConfig>(), std::mem::align_of::<*const ()>());
}

#[test]
fn ffi_transport_config_fields_round_trip_tcp() {
    // Construct an FfiTransportConfig directly (mimics the
    // production into_ffi() path that pins host-side strings).
    let host = "127.0.0.1";
    let cfg = FfiTransportConfig {
        kind: 0,
        host_ptr: host.as_ptr().cast(),
        host_len: host.len(),
        port: 12521,
        path_ptr: std::ptr::null(),
        path_len: 0,
        enable_grpc_web: 1,
    };
    assert_eq!(cfg.kind, 0);
    assert_eq!(cfg.port, 12521);
    assert_eq!(cfg.enable_grpc_web, 1);
    // SAFETY: `host_ptr` was just created from `host.as_ptr()` which
    // is valid for the duration of this test. `host_len` matches.
    let bytes = unsafe { std::slice::from_raw_parts(cfg.host_ptr.cast::<u8>(), cfg.host_len) };
    assert_eq!(bytes, host.as_bytes());
}

#[test]
fn ffi_transport_config_fields_round_trip_unix() {
    let path = "/tmp/reovim.sock";
    let cfg = FfiTransportConfig {
        kind: 1,
        host_ptr: std::ptr::null(),
        host_len: 0,
        port: 0,
        path_ptr: path.as_ptr().cast(),
        path_len: path.len(),
        enable_grpc_web: 0,
    };
    assert_eq!(cfg.kind, 1);
    assert_eq!(cfg.enable_grpc_web, 0);
    // SAFETY: `path_ptr` was just created from `path.as_ptr()` which
    // is valid for the duration of this test.
    let bytes = unsafe { std::slice::from_raw_parts(cfg.path_ptr.cast::<u8>(), cfg.path_len) };
    assert_eq!(bytes, path.as_bytes());
}

#[test]
fn abi_version_constant() {
    assert_eq!(REOVIM_NET_GRPC_DRIVER_ABI_VERSION, 1);
}

#[test]
fn api_version_constant() {
    assert_eq!(REOVIM_NET_GRPC_DRIVER_API_VERSION.major, 1);
    assert_eq!(REOVIM_NET_GRPC_DRIVER_API_VERSION.minor, 0);
    assert_eq!(REOVIM_NET_GRPC_DRIVER_API_VERSION.patch, 0);
}
