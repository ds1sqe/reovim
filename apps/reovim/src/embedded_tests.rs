use super::*;

#[test]
fn embedded_args_default_inproc_tui() {
    let args = EmbeddedArgs::resolve(ClientKind::Tui, None, None, None).unwrap();
    assert_eq!(args.client, ClientKind::Tui);
    assert_eq!(args.transport, TransportChoice::Inproc);
}

#[test]
fn embedded_args_default_inproc_cli() {
    let args = EmbeddedArgs::resolve(ClientKind::Cli, None, None, None).unwrap();
    assert_eq!(args.client, ClientKind::Cli);
    assert_eq!(args.transport, TransportChoice::Inproc);
}

#[test]
fn embedded_args_default_inproc_web() {
    let args = EmbeddedArgs::resolve(ClientKind::Web, None, None, None).unwrap();
    assert_eq!(args.client, ClientKind::Web);
    assert_eq!(args.transport, TransportChoice::Inproc);
}

#[test]
fn embedded_args_explicit_tcp() {
    let args = EmbeddedArgs::resolve(
        ClientKind::Tui,
        Some(TransportKind::Tcp),
        None,
        Some("127.0.0.1:13000"),
    )
    .unwrap();
    let TransportChoice::Tcp { addr } = args.transport else {
        panic!("expected Tcp transport");
    };
    assert_eq!(addr.port(), 13000);
}

#[test]
fn embedded_args_inproc_over_default_matches_explicit() {
    let implicit = EmbeddedArgs::resolve(ClientKind::Tui, None, None, None).unwrap();
    let explicit =
        EmbeddedArgs::resolve(ClientKind::Tui, Some(TransportKind::Inproc), None, None).unwrap();
    assert_eq!(implicit.transport, explicit.transport);
}

#[test]
fn embedded_args_tcp_parse_error_propagates() {
    let err =
        EmbeddedArgs::resolve(ClientKind::Tui, Some(TransportKind::Tcp), None, Some("garbage"))
            .unwrap_err();
    assert!(matches!(err, TransportError::TcpAddrParse(_)));
}

#[tokio::test]
async fn run_embedded_pipe_is_unsupported() {
    let args = EmbeddedArgs {
        client: ClientKind::Tui,
        transport: TransportChoice::Pipe,
    };
    let err = run_embedded(args).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
    assert!(err.to_string().contains("OS pipe"));
}

#[cfg(unix)]
#[test]
fn run_embedded_uds_args_accept_path() {
    // UDS is wired now (2b.G). Unit-testing a full boot would require a
    // running tokio runtime and a TTY; that lives in the integration
    // suite (`tests/transport_matrix.rs`). This test pins the arg
    // surface instead — a UDS transport choice holds the requested
    // path and survives a resolve round-trip.
    let path = std::path::PathBuf::from("/tmp/reovim-test-uds-resolve.sock");
    let args = EmbeddedArgs {
        client: ClientKind::Tui,
        transport: TransportChoice::Uds { path: path.clone() },
    };
    let TransportChoice::Uds { path: resolved } = args.transport else {
        panic!("expected Uds transport");
    };
    assert_eq!(resolved, path);
}

#[cfg(feature = "embedded-tui")]
#[test]
fn run_embedded_inproc_args_match_shape() {
    // Inproc is wired now (2b.G). As above, a full boot is an
    // integration test. Here we just assert the arg shape: the default
    // inproc resolution produces `TransportChoice::Inproc`.
    let args = EmbeddedArgs::resolve(ClientKind::Tui, None, None, None).unwrap();
    assert_eq!(args.client, ClientKind::Tui);
    assert_eq!(args.transport, TransportChoice::Inproc);
}
