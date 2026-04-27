use super::*;

#[test]
fn no_flags_defaults_to_grpc_12540() {
    let mode = determine_transport(
        None,
        None,
        #[cfg(unix)]
        None,
        None,
    )
    .expect("no-flag path must succeed");
    assert!(
        matches!(mode, TransportMode::Grpc { port: 12540 }),
        "expected Grpc {{ port: 12540 }}, got {mode:?}"
    );
}

#[test]
fn grpc_flag_routes_to_grpc_variant() {
    let mode = determine_transport(
        None,
        Some(1234),
        #[cfg(unix)]
        None,
        None,
    )
    .expect("--grpc path must succeed");
    assert!(
        matches!(mode, TransportMode::Grpc { port: 1234 }),
        "expected Grpc {{ port: 1234 }}, got {mode:?}"
    );
}

#[test]
fn tcp_flag_routes_to_tcp_variant() {
    let mode = determine_transport(
        Some(5678),
        None,
        #[cfg(unix)]
        None,
        None,
    )
    .expect("--tcp path must succeed");
    assert!(
        matches!(mode, TransportMode::Tcp { port: 5678 }),
        "expected Tcp {{ port: 5678 }}, got {mode:?}"
    );
}

#[cfg(unix)]
#[test]
fn socket_flag_routes_to_unix_socket_variant() {
    let path = std::path::PathBuf::from("/tmp/reovim-test.sock");
    let mode = determine_transport(None, None, Some(path.clone()), None)
        .expect("--socket path must succeed");
    match mode {
        TransportMode::UnixSocket { path: got } => assert_eq!(got, path),
        other => panic!("expected UnixSocket, got {other:?}"),
    }
}

#[test]
fn grpc_flag_wins_over_tcp_flag() {
    let mode = determine_transport(
        Some(5678),
        Some(1234),
        #[cfg(unix)]
        None,
        None,
    )
    .expect("grpc+tcp path must succeed");
    assert!(
        matches!(mode, TransportMode::Grpc { port: 1234 }),
        "expected Grpc to win over Tcp, got {mode:?}"
    );
}

#[test]
fn transport_pipe_maps_to_pipe_variant() {
    let mode = determine_transport(
        None,
        None,
        #[cfg(unix)]
        None,
        Some(TransportArg::Pipe),
    )
    .expect("--transport pipe must succeed");
    assert!(matches!(mode, TransportMode::Pipe), "expected Pipe, got {mode:?}");
}

#[test]
fn transport_inproc_is_rejected_by_standalone_bin() {
    let err = determine_transport(
        None,
        None,
        #[cfg(unix)]
        None,
        Some(TransportArg::Inproc),
    )
    .expect_err("--transport inproc must be rejected");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("inproc"), "error message should mention inproc: {err}");
}

#[test]
fn transport_pipe_wins_over_port_flags() {
    let mode = determine_transport(
        Some(5678),
        Some(1234),
        #[cfg(unix)]
        None,
        Some(TransportArg::Pipe),
    )
    .expect("--transport pipe should take priority");
    assert!(
        matches!(mode, TransportMode::Pipe),
        "expected Pipe to win over port flags, got {mode:?}"
    );
}
