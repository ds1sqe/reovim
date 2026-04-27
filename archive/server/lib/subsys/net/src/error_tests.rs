//! Tests for `NetError` shape and conversions.

use {
    super::NetError,
    crate::transport::TransportKind,
    std::io::{Error as IoError, ErrorKind},
};

#[test]
fn display_bind_failed() {
    let err = NetError::BindFailed("address in use".into());
    assert_eq!(err.to_string(), "failed to bind: address in use");
}

#[test]
fn display_accept_failed() {
    let err = NetError::AcceptFailed("peer reset".into());
    assert_eq!(err.to_string(), "failed to accept connection: peer reset");
}

#[test]
fn display_serve_failed() {
    let err = NetError::ServeFailed("tonic error".into());
    assert_eq!(err.to_string(), "serve failed: tonic error");
}

#[test]
fn display_io() {
    let err = NetError::Io("broken pipe".into());
    assert_eq!(err.to_string(), "I/O error: broken pipe");
}

#[test]
fn display_port_exhausted() {
    assert_eq!(NetError::PortExhausted.to_string(), "no available ports in fallback range");
}

#[test]
fn display_invalid_address() {
    let err = NetError::InvalidAddress("not.a.host:xx".into());
    assert_eq!(err.to_string(), "invalid address: not.a.host:xx");
}

#[test]
fn display_unsupported_transport_stdio() {
    let err = NetError::UnsupportedTransport(TransportKind::Stdio);
    assert!(err.to_string().contains("Stdio"));
    assert!(err.to_string().contains("does not support"));
}

#[test]
fn display_unsupported_transport_unix_and_tcp() {
    let unix = NetError::UnsupportedTransport(TransportKind::UnixSocket);
    let tcp = NetError::UnsupportedTransport(TransportKind::Tcp);
    assert!(unix.to_string().contains("UnixSocket"));
    assert!(tcp.to_string().contains("Tcp"));
}

#[test]
fn debug_contains_variant_name() {
    let err = NetError::BindFailed("x".into());
    assert!(format!("{err:?}").contains("BindFailed"));
}

#[test]
fn is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(NetError::BindFailed("x".into()));
    assert!(err.to_string().starts_with("failed to bind"));
}

#[test]
fn from_io_error() {
    let io = IoError::new(ErrorKind::AddrInUse, "boom");
    let err: NetError = io.into();
    match err {
        NetError::Io(msg) => assert!(msg.contains("boom")),
        other => panic!("expected NetError::Io, got {other:?}"),
    }
}

#[test]
fn from_tonic_transport_error_conversion_shape() {
    // Bind-style runtime cover lives in the driver crate (N.2).
    // Here we only assert the From<tonic::transport::Error> impl
    // exists by forcing a monomorphization through a helper fn.
    fn _accepts_tonic_error(e: tonic::transport::Error) -> NetError {
        NetError::from(e)
    }
    let shaped = NetError::ServeFailed("simulated".into());
    assert_eq!(shaped.to_string(), "serve failed: simulated");
}
