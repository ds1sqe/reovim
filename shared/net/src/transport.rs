//! Transport layer configuration.
//!
//! Defines transport types for RPC communication:
//! - Stdio: JSON-RPC over stdin/stdout (for process piping)
//! - Unix socket: JSON-RPC over Unix domain socket
//! - TCP: JSON-RPC over TCP connection

use std::path::PathBuf;

/// Transport configuration.
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// Use stdin/stdout for communication.
    Stdio,
    /// Listen on Unix socket at the given path.
    UnixSocket {
        /// Path to the Unix socket.
        path: PathBuf,
    },
    /// Listen on TCP at the given address.
    Tcp {
        /// Host address (e.g., "127.0.0.1").
        host: String,
        /// Port number.
        port: u16,
    },
}

impl TransportConfig {
    /// Create Unix socket config.
    #[must_use]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self::UnixSocket { path: path.into() }
    }

    /// Create TCP config.
    #[must_use]
    pub fn tcp(host: impl Into<String>, port: u16) -> Self {
        Self::Tcp {
            host: host.into(),
            port,
        }
    }

    /// Create TCP config with default host (localhost).
    #[must_use]
    pub fn tcp_localhost(port: u16) -> Self {
        Self::tcp("127.0.0.1", port)
    }

    /// Default port for reovim RPC server.
    /// Calculated as: 'r' * 100 + 'e' * 10 + 'o' = 114*100 + 101*10 + 111 = 12521
    pub const DEFAULT_PORT: u16 = 12521;

    /// Maximum port in fallback range.
    pub const MAX_PORT: u16 = 12530;

}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
