use super::*;

#[test]
fn test_determine_transport_grpc() {
    let result = determine_transport(
        None,
        Some(12540),
        #[cfg(unix)]
        None,
    );
    assert!(matches!(result, TransportMode::Grpc { port: 12540 }));
}

#[test]
fn test_determine_transport_tcp() {
    let result = determine_transport(
        Some(12522),
        None,
        #[cfg(unix)]
        None,
    );
    assert!(matches!(result, TransportMode::Tcp { port: 12522 }));
}

#[test]
fn test_determine_transport_fallback() {
    let result = determine_transport(
        None,
        None,
        #[cfg(unix)]
        None,
    );
    assert!(matches!(result, TransportMode::TcpWithFallback));
}

#[test]
fn test_determine_transport_grpc_over_tcp() {
    // gRPC takes priority over TCP
    let result = determine_transport(
        Some(12522),
        Some(12540),
        #[cfg(unix)]
        None,
    );
    assert!(matches!(result, TransportMode::Grpc { port: 12540 }));
}

#[cfg(unix)]
#[test]
fn test_determine_transport_socket() {
    let result = determine_transport(None, None, Some("/tmp/reovim.sock".into()));
    assert!(matches!(result, TransportMode::UnixSocket { .. }));
}

#[cfg(unix)]
#[test]
fn test_determine_transport_grpc_over_socket() {
    // gRPC takes priority over socket
    let result = determine_transport(None, Some(12540), Some("/tmp/reovim.sock".into()));
    assert!(matches!(result, TransportMode::Grpc { port: 12540 }));
}

#[test]
fn test_cli_output_format_debug() {
    assert!(format!("{:?}", CliOutputFormat::Plain).contains("Plain"));
    assert!(format!("{:?}", CliOutputFormat::Json).contains("Json"));
}

#[test]
fn test_cli_output_format_clone_eq() {
    let a = CliOutputFormat::Plain;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn test_cli_extension_state_parse() {
    let cli = Cli::parse_from([
        "reovim",
        "cli",
        "extension-state",
        "whichkey",
        "--client",
        "1",
    ]);
    match cli.command {
        Some(Commands::Cli { command, .. }) => {
            assert!(matches!(
                command,
                CliSubcommand::ExtensionState { ref kind, client: 1 } if kind == "whichkey"
            ));
        }
        _ => panic!("Expected Cli command"),
    }
}

#[test]
fn test_cli_extensions_parse() {
    let cli = Cli::parse_from(["reovim", "cli", "extensions"]);
    match cli.command {
        Some(Commands::Cli { command, .. }) => {
            assert!(matches!(command, CliSubcommand::Extensions));
        }
        _ => panic!("Expected Cli command"),
    }
}
