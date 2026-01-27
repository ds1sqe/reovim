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
    #[cfg(feature = "grpc")]
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
    #[cfg(feature = "grpc")]
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
