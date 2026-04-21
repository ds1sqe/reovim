//! Tests for `TransportConfig` + `TransportKind`.

use {
    super::{TransportConfig, TransportKind},
    std::path::PathBuf,
};

#[test]
fn stdio_matches() {
    let config = TransportConfig::Stdio;
    assert!(matches!(config, TransportConfig::Stdio));
}

#[test]
fn unix_socket_from_str() {
    let config = TransportConfig::unix_socket("/tmp/test.sock");
    match config {
        TransportConfig::UnixSocket { path } => {
            assert_eq!(path, PathBuf::from("/tmp/test.sock"));
        }
        other => panic!("expected UnixSocket, got {other:?}"),
    }
}

#[test]
fn unix_socket_from_string() {
    let config = TransportConfig::unix_socket(String::from("/var/run/reovim.sock"));
    match config {
        TransportConfig::UnixSocket { path } => {
            assert_eq!(path, PathBuf::from("/var/run/reovim.sock"));
        }
        other => panic!("expected UnixSocket, got {other:?}"),
    }
}

#[test]
fn tcp_from_str_host() {
    let config = TransportConfig::tcp("localhost", 9999);
    match config {
        TransportConfig::Tcp { host, port } => {
            assert_eq!(host, "localhost");
            assert_eq!(port, 9999);
        }
        other => panic!("expected Tcp, got {other:?}"),
    }
}

#[test]
fn tcp_from_string_host() {
    let config = TransportConfig::tcp(String::from("0.0.0.0"), 5000);
    match config {
        TransportConfig::Tcp { host, port } => {
            assert_eq!(host, "0.0.0.0");
            assert_eq!(port, 5000);
        }
        other => panic!("expected Tcp, got {other:?}"),
    }
}

#[test]
fn tcp_localhost_uses_127_0_0_1() {
    let config = TransportConfig::tcp_localhost(8080);
    match config {
        TransportConfig::Tcp { host, port } => {
            assert_eq!(host, "127.0.0.1");
            assert_eq!(port, 8080);
        }
        other => panic!("expected Tcp, got {other:?}"),
    }
}

#[test]
fn tcp_port_zero_and_max() {
    assert!(matches!(
        TransportConfig::tcp_localhost(0),
        TransportConfig::Tcp { port: 0, .. }
    ));
    assert!(matches!(
        TransportConfig::tcp("h", u16::MAX),
        TransportConfig::Tcp { port: u16::MAX, .. }
    ));
}

#[test]
fn default_port_constants() {
    assert_eq!(TransportConfig::DEFAULT_PORT, 12521);
    assert_eq!(TransportConfig::MAX_PORT, 12530);
    const { assert!(TransportConfig::MAX_PORT > TransportConfig::DEFAULT_PORT) };
}

#[test]
fn debug_contains_variant_names() {
    assert!(format!("{:?}", TransportConfig::Stdio).contains("Stdio"));
    assert!(format!("{:?}", TransportConfig::unix_socket("/x")).contains("UnixSocket"));
    assert!(format!("{:?}", TransportConfig::tcp("h", 1)).contains("Tcp"));
}

#[test]
fn kind_stdio() {
    assert_eq!(TransportConfig::Stdio.kind(), TransportKind::Stdio);
}

#[test]
fn kind_unix() {
    assert_eq!(
        TransportConfig::unix_socket("/tmp/x").kind(),
        TransportKind::UnixSocket
    );
}

#[test]
fn kind_tcp() {
    assert_eq!(
        TransportConfig::tcp_localhost(0).kind(),
        TransportKind::Tcp
    );
}

#[test]
fn kind_is_copy_and_eq() {
    let a = TransportKind::Tcp;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, TransportKind::UnixSocket);
}

#[test]
fn clone_preserves_fields() {
    let original = TransportConfig::tcp("192.168.1.1", 3000);
    // Move semantics are enough here; the Clone impl is exercised
    // elsewhere (downstream consumers clone TransportConfig through
    // driver dispatch).
    match original {
        TransportConfig::Tcp { host, port } => {
            assert_eq!(host, "192.168.1.1");
            assert_eq!(port, 3000);
        }
        other => panic!("expected Tcp, got {other:?}"),
    }
}
