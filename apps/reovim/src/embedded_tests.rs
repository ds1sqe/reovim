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
#[tokio::test]
async fn run_embedded_uds_is_not_yet_wired() {
    let args = EmbeddedArgs {
        client: ClientKind::Tui,
        transport: TransportChoice::Uds {
            path: std::path::PathBuf::from("/tmp/reovim-test.sock"),
        },
    };
    let err = run_embedded(args).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
    assert!(err.to_string().contains("uds/tcp"));
}

#[tokio::test]
async fn run_embedded_tcp_is_not_yet_wired() {
    let args = EmbeddedArgs {
        client: ClientKind::Tui,
        transport: TransportChoice::Tcp {
            addr: "127.0.0.1:0".parse().unwrap(),
        },
    };
    let err = run_embedded(args).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
    assert!(err.to_string().contains("uds/tcp"));
}

#[cfg(feature = "embedded-tui")]
#[tokio::test]
async fn run_embedded_inproc_tui_client_is_2b_e_followup() {
    let args = EmbeddedArgs {
        client: ClientKind::Tui,
        transport: TransportChoice::Inproc,
    };
    let err = run_embedded(args).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
    // The server side is already live; the Unsupported message must
    // point at the 2b.E follow-up so a reader of the error can locate
    // the next sub-phase.
    assert!(err.to_string().contains("2b.E"));
    assert!(err.to_string().contains("transport_inproc"));
}
