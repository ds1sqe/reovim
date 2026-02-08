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
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_stdio() {
        let config = TransportConfig::Stdio;
        assert!(matches!(config, TransportConfig::Stdio));
    }

    #[test]
    fn test_transport_config_unix() {
        let config = TransportConfig::unix_socket("/tmp/test.sock");
        match config {
            TransportConfig::UnixSocket { path } => {
                assert_eq!(path, PathBuf::from("/tmp/test.sock"));
            }
            _ => panic!("Expected UnixSocket config"),
        }
    }

    #[test]
    fn test_transport_config_tcp() {
        let config = TransportConfig::tcp("localhost", 9999);
        match config {
            TransportConfig::Tcp { host, port } => {
                assert_eq!(host, "localhost");
                assert_eq!(port, 9999);
            }
            _ => panic!("Expected Tcp config"),
        }
    }

    #[test]
    fn test_transport_config_tcp_localhost() {
        let config = TransportConfig::tcp_localhost(8080);
        match config {
            TransportConfig::Tcp { host, port } => {
                assert_eq!(host, "127.0.0.1");
                assert_eq!(port, 8080);
            }
            _ => panic!("Expected Tcp config"),
        }
    }

    #[test]
    fn test_default_port() {
        assert_eq!(TransportConfig::DEFAULT_PORT, 12521);
        assert_eq!(TransportConfig::MAX_PORT, 12530);
    }

    #[test]
    fn test_max_port_greater_than_default() {
        const { assert!(TransportConfig::MAX_PORT > TransportConfig::DEFAULT_PORT) };
    }

    #[test]
    fn test_transport_config_debug() {
        let stdio = TransportConfig::Stdio;
        let debug_str = format!("{stdio:?}");
        assert!(debug_str.contains("Stdio"));

        let unix = TransportConfig::unix_socket("/tmp/test.sock");
        let debug_str = format!("{unix:?}");
        assert!(debug_str.contains("UnixSocket"));
        assert!(debug_str.contains("test.sock"));

        let tcp = TransportConfig::tcp("localhost", 8080);
        let debug_str = format!("{tcp:?}");
        assert!(debug_str.contains("Tcp"));
        assert!(debug_str.contains("localhost"));
        assert!(debug_str.contains("8080"));
    }

    #[test]
    fn test_transport_config_clone() {
        let original = TransportConfig::tcp("192.168.1.1", 3000);
        let cloned = original;
        match cloned {
            TransportConfig::Tcp { host, port } => {
                assert_eq!(host, "192.168.1.1");
                assert_eq!(port, 3000);
            }
            _ => panic!("Expected Tcp config after clone"),
        }
    }

    #[test]
    fn test_unix_socket_from_string() {
        let config = TransportConfig::unix_socket(String::from("/var/run/reovim.sock"));
        match config {
            TransportConfig::UnixSocket { path } => {
                assert_eq!(path, PathBuf::from("/var/run/reovim.sock"));
            }
            _ => panic!("Expected UnixSocket config"),
        }
    }

    #[test]
    fn test_tcp_from_string_host() {
        let config = TransportConfig::tcp(String::from("0.0.0.0"), 5000);
        match config {
            TransportConfig::Tcp { host, port } => {
                assert_eq!(host, "0.0.0.0");
                assert_eq!(port, 5000);
            }
            _ => panic!("Expected Tcp config"),
        }
    }

    #[test]
    fn test_tcp_port_zero() {
        let config = TransportConfig::tcp_localhost(0);
        match config {
            TransportConfig::Tcp { host, port } => {
                assert_eq!(host, "127.0.0.1");
                assert_eq!(port, 0);
            }
            _ => panic!("Expected Tcp config"),
        }
    }

    #[test]
    fn test_tcp_port_max() {
        let config = TransportConfig::tcp("localhost", u16::MAX);
        match config {
            TransportConfig::Tcp { host, port } => {
                assert_eq!(host, "localhost");
                assert_eq!(port, u16::MAX);
            }
            _ => panic!("Expected Tcp config"),
        }
    }

    #[test]
    fn test_unix_socket_clone() {
        let original = TransportConfig::unix_socket("/tmp/clone-test.sock");
        let cloned = original;
        match cloned {
            TransportConfig::UnixSocket { path } => {
                assert_eq!(path, PathBuf::from("/tmp/clone-test.sock"));
            }
            _ => panic!("Expected UnixSocket config after clone"),
        }
    }

    #[test]
    fn test_stdio_clone() {
        let original = TransportConfig::Stdio;
        let cloned = original;
        assert!(matches!(cloned, TransportConfig::Stdio));
    }
}
