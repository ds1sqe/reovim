//! Server configuration types.
//!
//! Contains `TransportMode` and `ServerConfig` for configuring
//! how the server listens for connections.

use std::path::PathBuf;

/// Transport configuration for the server.
///
/// Determines how the server accepts client connections.
#[derive(Debug, Clone, Default)]
pub enum TransportMode {
    /// TCP with automatic port fallback (12540-12549 for new server).
    #[default]
    TcpWithFallback,

    /// TCP on a specific port.
    Tcp {
        /// Port to bind to.
        port: u16,
    },

    /// Unix socket at a specific path.
    #[cfg(unix)]
    UnixSocket {
        /// Path to the socket file.
        path: PathBuf,
    },

    /// gRPC transport on a specific port.
    ///
    /// Uses HTTP/2 and Protocol Buffers for efficient binary communication.
    /// This is the primary transport for v0.9.3+ clients.
    Grpc {
        /// Port to bind gRPC server to.
        port: u16,
    },
}

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Transport mode.
    pub transport: TransportMode,

    /// Instance name for registry discovery.
    pub instance_name: String,

    /// Name of the default session to create on startup.
    pub default_session_name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            transport: TransportMode::TcpWithFallback,
            instance_name: String::from("default"),
            default_session_name: String::from("default"),
        }
    }
}

impl ServerConfig {
    /// Create config for gRPC transport on specific port.
    #[must_use]
    pub fn grpc(port: u16) -> Self {
        Self {
            transport: TransportMode::Grpc { port },
            ..Self::default()
        }
    }

    /// Create config for TCP transport on specific port.
    #[must_use]
    pub fn tcp(port: u16) -> Self {
        Self {
            transport: TransportMode::Tcp { port },
            ..Self::default()
        }
    }

    /// Create config for Unix socket.
    #[cfg(unix)]
    #[must_use]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self {
            transport: TransportMode::UnixSocket { path: path.into() },
            ..Self::default()
        }
    }

    /// Set the instance name.
    #[must_use]
    pub fn with_instance_name(mut self, name: impl Into<String>) -> Self {
        self.instance_name = name.into();
        self
    }

    /// Set the default session name.
    #[must_use]
    pub fn with_session_name(mut self, name: impl Into<String>) -> Self {
        self.default_session_name = name.into();
        self
    }
}

#[cfg(test)]
mod tests {
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
}
