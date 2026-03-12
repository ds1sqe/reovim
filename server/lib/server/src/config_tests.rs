use super::*;

#[test]
fn transport_mode_default_is_tcp_with_fallback() {
    assert!(matches!(TransportMode::default(), TransportMode::TcpWithFallback));
}

#[test]
fn server_config_default() {
    let config = ServerConfig::default();
    assert!(matches!(config.transport, TransportMode::TcpWithFallback));
    assert_eq!(config.instance_name, "default");
    assert_eq!(config.default_session_name, "default");
}

#[test]
fn server_config_grpc() {
    let config = ServerConfig::grpc(50051);
    assert!(matches!(config.transport, TransportMode::Grpc { port: 50051 }));
    assert_eq!(config.instance_name, "default");
    assert_eq!(config.default_session_name, "default");
}

#[test]
fn server_config_tcp() {
    let config = ServerConfig::tcp(8080);
    assert!(matches!(config.transport, TransportMode::Tcp { port: 8080 }));
}

#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn server_config_unix_socket() {
    let config = ServerConfig::unix_socket("/tmp/reovim.sock");
    let TransportMode::UnixSocket { path } = &config.transport else {
        panic!("Expected UnixSocket transport");
    };
    assert_eq!(path, &PathBuf::from("/tmp/reovim.sock"));
}

#[test]
fn server_config_with_instance_name() {
    let config = ServerConfig::default().with_instance_name("my-server");
    assert_eq!(config.instance_name, "my-server");
}

#[test]
fn server_config_with_session_name() {
    let config = ServerConfig::default().with_session_name("my-session");
    assert_eq!(config.default_session_name, "my-session");
}

#[test]
fn server_config_builder_chain() {
    let config = ServerConfig::grpc(9000)
        .with_instance_name("prod")
        .with_session_name("main");
    assert!(matches!(config.transport, TransportMode::Grpc { port: 9000 }));
    assert_eq!(config.instance_name, "prod");
    assert_eq!(config.default_session_name, "main");
}

#[test]
fn transport_mode_debug() {
    let mode = TransportMode::Grpc { port: 50051 };
    let debug = format!("{mode:?}");
    assert!(debug.contains("Grpc"));
    assert!(debug.contains("50051"));
}

#[test]
fn server_config_clone() {
    let config = ServerConfig::grpc(50051).with_instance_name("test");
    #[allow(clippy::redundant_clone)]
    let cloned = config.clone();
    assert_eq!(cloned.instance_name, "test");
}
