use std::{net::SocketAddr, path::PathBuf};

use super::*;

#[test]
fn inproc_ok_embedded() {
    let got =
        TransportChoice::resolve(TransportKind::Inproc, LaunchMode::Embedded, None, None).unwrap();
    assert_eq!(got, TransportChoice::Inproc);
}

#[test]
fn inproc_rejected_subprocess() {
    let err = TransportChoice::resolve(TransportKind::Inproc, LaunchMode::Subprocess, None, None)
        .unwrap_err();
    assert_eq!(err, TransportError::InprocRequiresEmbedded);
}

#[test]
fn inproc_rejected_external_grpc() {
    let err = TransportChoice::resolve(TransportKind::Inproc, LaunchMode::ExternalGrpc, None, None)
        .unwrap_err();
    assert_eq!(err, TransportError::InprocRequiresEmbedded);
}

#[test]
fn pipe_rejected_external_grpc() {
    let err = TransportChoice::resolve(TransportKind::Pipe, LaunchMode::ExternalGrpc, None, None)
        .unwrap_err();
    assert_eq!(err, TransportError::ExternalGrpcRequiresTcp);
}

#[test]
fn uds_rejected_external_grpc() {
    let err = TransportChoice::resolve(TransportKind::Uds, LaunchMode::ExternalGrpc, None, None)
        .unwrap_err();
    assert_eq!(err, TransportError::ExternalGrpcRequiresTcp);
}

#[test]
fn tcp_external_grpc_ok() {
    let got = TransportChoice::resolve(
        TransportKind::Tcp,
        LaunchMode::ExternalGrpc,
        None,
        Some("127.0.0.1:8080"),
    )
    .unwrap();
    let TransportChoice::Tcp { addr } = got else {
        panic!("expected Tcp");
    };
    assert_eq!(addr, "127.0.0.1:8080".parse::<SocketAddr>().unwrap());
}

#[test]
fn tcp_default_addr_embedded() {
    let got =
        TransportChoice::resolve(TransportKind::Tcp, LaunchMode::Embedded, None, None).unwrap();
    let TransportChoice::Tcp { addr } = got else {
        panic!("expected Tcp");
    };
    assert_eq!(addr.ip().to_string(), "127.0.0.1");
    assert_eq!(addr.port(), 0);
}

#[test]
fn tcp_bad_addr_error() {
    let err = TransportChoice::resolve(
        TransportKind::Tcp,
        LaunchMode::Embedded,
        None,
        Some("not-an-addr"),
    )
    .unwrap_err();
    assert!(matches!(err, TransportError::TcpAddrParse(_)));
}

#[cfg(unix)]
#[test]
fn uds_ok_embedded_default_path() {
    let got =
        TransportChoice::resolve(TransportKind::Uds, LaunchMode::Embedded, None, None).unwrap();
    let TransportChoice::Uds { path } = got else {
        panic!("expected Uds");
    };
    assert!(path.to_string_lossy().contains("reovim-"));
    assert!(path.extension().is_some_and(|e| e == "sock"));
}

#[cfg(unix)]
#[test]
fn uds_ok_embedded_custom_path() {
    let custom = PathBuf::from("/tmp/reovim-custom.sock");
    let got =
        TransportChoice::resolve(TransportKind::Uds, LaunchMode::Embedded, Some(&custom), None)
            .unwrap();
    let TransportChoice::Uds { path } = got else {
        panic!("expected Uds");
    };
    assert_eq!(path, custom);
}

#[test]
fn pipe_ok_embedded() {
    let got =
        TransportChoice::resolve(TransportKind::Pipe, LaunchMode::Embedded, None, None).unwrap();
    assert_eq!(got, TransportChoice::Pipe);
}

#[test]
fn pipe_ok_subprocess() {
    let got =
        TransportChoice::resolve(TransportKind::Pipe, LaunchMode::Subprocess, None, None).unwrap();
    assert_eq!(got, TransportChoice::Pipe);
}

#[test]
fn to_server_mode_inproc() {
    assert!(matches!(TransportChoice::Inproc.to_server_mode(), TransportMode::Inproc));
}

#[test]
fn to_server_mode_pipe() {
    assert!(matches!(TransportChoice::Pipe.to_server_mode(), TransportMode::Pipe));
}

#[test]
fn to_server_mode_tcp_preserves_port() {
    let choice = TransportChoice::Tcp {
        addr: "127.0.0.1:12340".parse().unwrap(),
    };
    let TransportMode::Grpc { port } = choice.to_server_mode() else {
        panic!("expected Grpc");
    };
    assert_eq!(port, 12340);
}

#[cfg(unix)]
#[test]
fn to_server_mode_uds_preserves_path() {
    let choice = TransportChoice::Uds {
        path: PathBuf::from("/tmp/reovim-x.sock"),
    };
    let TransportMode::UnixSocket { path } = choice.to_server_mode() else {
        panic!("expected UnixSocket");
    };
    assert_eq!(path, PathBuf::from("/tmp/reovim-x.sock"));
}

#[test]
fn to_client_connect_inproc_is_none() {
    assert!(TransportChoice::Inproc.to_client_connect_string().is_none());
}

#[test]
fn to_client_connect_pipe_is_none() {
    assert!(TransportChoice::Pipe.to_client_connect_string().is_none());
}

#[test]
fn to_client_connect_tcp_roundtrip() {
    let choice = TransportChoice::Tcp {
        addr: "127.0.0.1:54321".parse().unwrap(),
    };
    assert_eq!(choice.to_client_connect_string().as_deref(), Some("127.0.0.1:54321"),);
}

#[cfg(unix)]
#[test]
fn to_client_connect_uds_exposes_path() {
    let choice = TransportChoice::Uds {
        path: PathBuf::from("/tmp/reovim-y.sock"),
    };
    assert_eq!(choice.to_client_connect_string().as_deref(), Some("/tmp/reovim-y.sock"),);
}

#[test]
fn transport_error_into_io_error() {
    let err: std::io::Error = TransportError::InprocRequiresEmbedded.into();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("inproc"));
}

#[test]
fn transport_error_display_external_grpc() {
    assert!(
        TransportError::ExternalGrpcRequiresTcp
            .to_string()
            .contains("--external-grpc")
    );
}

#[test]
fn transport_error_display_uds_platform() {
    assert!(
        TransportError::UdsUnsupportedOnPlatform
            .to_string()
            .contains("uds")
    );
}

#[test]
fn transport_error_display_tcp_parse() {
    let msg = TransportError::TcpAddrParse("bad".into()).to_string();
    assert!(msg.contains("bad"));
}

#[test]
fn transport_kind_roundtrips_through_clap() {
    use clap::ValueEnum;
    let variants = TransportKind::value_variants();
    assert_eq!(variants.len(), 4);
    let spellings: Vec<_> = variants
        .iter()
        .map(|v| v.to_possible_value().unwrap().get_name().to_string())
        .collect();
    assert_eq!(spellings, vec!["inproc", "pipe", "uds", "tcp"]);
}
