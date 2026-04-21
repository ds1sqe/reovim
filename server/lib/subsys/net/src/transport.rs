//! Transport configuration for the gRPC server.
//!
//! The subsys expresses transport intent as a plain data enum;
//! drivers translate the variant into a concrete listener.

use std::path::PathBuf;

/// Lightweight tag for the transport variant, used by
/// `NetError::UnsupportedTransport` so error values don't have to
/// re-carry path/host/port payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// Communicate over stdin/stdout (no driver supports this for gRPC).
    Stdio,
    /// Unix domain socket.
    UnixSocket,
    /// TCP listener.
    Tcp,
}

/// Transport configuration for the gRPC server.
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// Communicate over stdin/stdout.
    Stdio,
    /// Listen on a Unix domain socket at the given path.
    UnixSocket {
        /// Filesystem path for the socket.
        path: PathBuf,
    },
    /// Listen on TCP at the given host and port.
    Tcp {
        /// Host address (e.g., "127.0.0.1").
        host: String,
        /// Port number (0 to let the OS pick one).
        port: u16,
    },
}

impl TransportConfig {
    /// Default port: 'r'*100 + 'e'*10 + 'o' = 12521.
    pub const DEFAULT_PORT: u16 = 12521;

    /// Last port in the default fallback range.
    pub const MAX_PORT: u16 = 12530;

    /// Build a Unix-socket config.
    #[must_use]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self::UnixSocket { path: path.into() }
    }

    /// Build a TCP config.
    #[must_use]
    pub fn tcp(host: impl Into<String>, port: u16) -> Self {
        Self::Tcp {
            host: host.into(),
            port,
        }
    }

    /// Build a TCP config bound to `127.0.0.1`.
    #[must_use]
    pub fn tcp_localhost(port: u16) -> Self {
        Self::tcp("127.0.0.1", port)
    }

    /// Lightweight tag for use in error values.
    #[must_use]
    pub const fn kind(&self) -> TransportKind {
        match self {
            Self::Stdio => TransportKind::Stdio,
            Self::UnixSocket { .. } => TransportKind::UnixSocket,
            Self::Tcp { .. } => TransportKind::Tcp,
        }
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
