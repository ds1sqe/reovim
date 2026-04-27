//! Tests for `GrpcServerDriverImpl`.
//!
//! Covers the inherent lifecycle methods (`probe`, `construct`,
//! `runtime_handle`) plus a smoke serve test for `Stdio` (rejected).
//! Full FFI roundtrip lives in
//! `server/lib/subsys/driver-loader/tests/net_grpc_dlopen_roundtrip.rs`
//! (SP02 Phase 5).

#![allow(unsafe_code)] // pipe2() + raw fd manipulation in shutdown helpers

use {
    super::GrpcServerDriverImpl,
    reovim_subsys_net::{
        GrpcServerDriver, NetError, ServiceDescriptor, TransportConfig, abi::ShutdownFd,
    },
    std::sync::atomic::AtomicU16,
};

#[test]
fn probe_returns_kind_and_name() {
    let p = GrpcServerDriverImpl::probe();
    assert_eq!(&p.kind[..8], b"net_grpc");
    let name_str = std::str::from_utf8(&p.name).unwrap_or("");
    assert!(name_str.starts_with("reovim-driver-net-grpc"));
}

#[test]
fn construct_succeeds() {
    let driver = GrpcServerDriverImpl::construct().expect("construct");
    let _handle = driver.runtime_handle();
}

#[test]
fn driver_is_object_safe() {
    let driver = GrpcServerDriverImpl::construct().expect("construct");
    let _: Box<dyn GrpcServerDriver> = Box::new(driver);
}

#[tokio::test]
async fn serve_rejects_stdio() {
    let driver = GrpcServerDriverImpl::construct().expect("construct");
    let writeback: &'static AtomicU16 = Box::leak(Box::new(AtomicU16::new(0)));
    let result = driver
        .serve(
            TransportConfig::Stdio,
            Vec::<ServiceDescriptor>::new(),
            ShutdownFd(-1),
            ShutdownFd(-1),
            writeback,
        )
        .await;
    assert!(matches!(result, Err(NetError::Io(_))));
}

#[tokio::test]
async fn serve_rejects_unix_socket_for_now() {
    // Phase 4 minimum: only TCP. UnixSocket support lands in a Phase
    // 4 follow-on (driver.rs serve body explicitly returns
    // NetError::Io for non-Tcp configs).
    let driver = GrpcServerDriverImpl::construct().expect("construct");
    let writeback: &'static AtomicU16 = Box::leak(Box::new(AtomicU16::new(0)));
    let result = driver
        .serve(
            TransportConfig::unix_socket("/tmp/reovim-test.sock"),
            Vec::<ServiceDescriptor>::new(),
            ShutdownFd(-1),
            ShutdownFd(-1),
            writeback,
        )
        .await;
    assert!(matches!(result, Err(NetError::Io(_))));
}
