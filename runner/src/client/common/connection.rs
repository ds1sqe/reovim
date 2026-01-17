//! Connection handling for outbound client connections.
//!
//! Supports TCP and Unix socket connections to a reovim server.
//!
//! For concurrent reading and writing (e.g., TUI notification handling),
//! use [`Connection::split`] to get separate reader/writer handles.

use std::{
    io,
    path::{Path, PathBuf},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
};

#[cfg(unix)]
use tokio::net::UnixStream;

use crate::server::instance::{InstanceRegistry, TransportInfo};

/// Connection configuration.
#[derive(Debug, Clone)]
pub enum ConnectionConfig {
    /// TCP connection to host:port.
    Tcp { host: String, port: u16 },
    /// Unix socket connection.
    #[cfg(unix)]
    UnixSocket(PathBuf),
}

impl ConnectionConfig {
    /// Create TCP config with default host and port.
    ///
    /// Uses port 12522 (port 12521 is reserved for manager daemon).
    #[must_use]
    pub fn tcp_default() -> Self {
        Self::Tcp {
            host: "127.0.0.1".to_string(),
            port: 12522,
        }
    }

    /// Create TCP config with specific host and port.
    #[must_use]
    pub fn tcp(host: impl Into<String>, port: u16) -> Self {
        Self::Tcp {
            host: host.into(),
            port,
        }
    }

    /// Create Unix socket config.
    #[cfg(unix)]
    #[must_use]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self::UnixSocket(path.into())
    }

    /// Parse from CLI argument (host:port format).
    ///
    /// # Errors
    ///
    /// Returns error if format is invalid.
    pub fn parse_tcp(addr: &str) -> Result<Self, String> {
        let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid address format: {addr}. Expected host:port"));
        }
        let port: u16 = parts[0]
            .parse()
            .map_err(|_| format!("Invalid port: {}", parts[0]))?;
        let host = parts[1].to_string();
        Ok(Self::Tcp { host, port })
    }

    /// Create config from a TCP address string (host:port format).
    ///
    /// If parsing fails, defaults to localhost:12521.
    #[must_use]
    pub fn tcp_from_addr(addr: &str) -> Self {
        Self::parse_tcp(addr).unwrap_or_else(|_| Self::tcp_default())
    }

    /// Auto-discover a running server.
    ///
    /// Discovery order:
    /// 1. Check instance registry for "default" instance
    /// 2. Scan default ports (12522-12531)
    /// 3. Fall back to default port
    #[must_use]
    pub fn auto_discover() -> Self {
        // First, try to find "default" instance in registry
        if let Ok(config) = Self::from_instance("default") {
            return config;
        }

        // Fall back to port scanning
        use super::discovery;
        let servers = discovery::list_servers();
        servers
            .first()
            .map_or_else(Self::tcp_default, |server| Self::Tcp {
                host: server.host.clone(),
                port: server.port,
            })
    }

    /// Resolve connection from CLI flags.
    ///
    /// Flag precedence (highest to lowest):
    /// 1. `--tcp` - explicit TCP connection, bypass registry
    /// 2. `-S path` - explicit socket/pipe path
    /// 3. `-L name` - named instance lookup
    /// 4. Default: `-L default`
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - TCP address parsing fails
    /// - Instance lookup fails (not found, invalid name)
    pub fn from_flags(
        tcp: Option<&str>,
        socket_path: Option<&Path>,
        instance: Option<&str>,
    ) -> Result<Self, String> {
        // 1. Explicit TCP wins
        if let Some(addr) = tcp {
            return Self::parse_tcp(addr);
        }

        // 2. Explicit socket path
        if let Some(path) = socket_path {
            #[cfg(unix)]
            return Ok(Self::unix_socket(path));
            #[cfg(not(unix))]
            return Err(format!("Unix sockets not supported on this platform: {}", path.display()));
        }

        // 3. Named instance lookup (or default)
        let name = instance.unwrap_or("default");
        Self::from_instance(name)
    }

    /// Look up a named instance from the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Instance name is invalid
    /// - Instance not found in registry
    /// - Registry cannot be read
    pub fn from_instance(name: &str) -> Result<Self, String> {
        // Validate instance name first
        InstanceRegistry::validate_name(name).map_err(|e| format!("Invalid instance name: {e}"))?;

        let registry = InstanceRegistry::new();
        match registry.get(name) {
            Ok(Some(info)) => Self::from_transport_info(&info.transport),
            Ok(None) => {
                Err(format!("Instance '{name}' not found. Run 'reovim server -L {name}' first."))
            }
            Err(e) => Err(format!("Failed to read instance registry: {e}")),
        }
    }

    /// Create config from transport info.
    ///
    /// Returns `Err` on non-Unix platforms for local transport.
    #[allow(clippy::unnecessary_wraps)] // Result needed for Windows local transport error
    fn from_transport_info(transport: &TransportInfo) -> Result<Self, String> {
        match transport {
            TransportInfo::Tcp { host, port } => Ok(Self::Tcp {
                host: host.clone(),
                port: *port,
            }),
            TransportInfo::Local { path } => {
                #[cfg(unix)]
                return Ok(Self::UnixSocket(PathBuf::from(path)));
                #[cfg(not(unix))]
                return Err(format!("Local transport not supported on this platform: {path}"));
            }
        }
    }
}

/// Connection to a reovim server.
///
/// Wraps either TCP or Unix socket with buffered I/O.
pub struct Connection {
    inner: ConnectionInner,
}

enum ConnectionInner {
    Tcp {
        reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
        writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    },
    #[cfg(unix)]
    Unix {
        reader: BufReader<tokio::net::unix::OwnedReadHalf>,
        writer: BufWriter<tokio::net::unix::OwnedWriteHalf>,
    },
}

impl Connection {
    /// Connect to a server using the given config.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect(config: &ConnectionConfig) -> io::Result<Self> {
        match config {
            ConnectionConfig::Tcp { host, port } => {
                let stream = TcpStream::connect((host.as_str(), *port)).await?;
                let (read_half, write_half) = stream.into_split();
                Ok(Self {
                    inner: ConnectionInner::Tcp {
                        reader: BufReader::new(read_half),
                        writer: BufWriter::new(write_half),
                    },
                })
            }
            #[cfg(unix)]
            ConnectionConfig::UnixSocket(path) => {
                let stream = UnixStream::connect(path).await?;
                let (read_half, write_half) = stream.into_split();
                Ok(Self {
                    inner: ConnectionInner::Unix {
                        reader: BufReader::new(read_half),
                        writer: BufWriter::new(write_half),
                    },
                })
            }
        }
    }

    /// Write a line to the server (appends newline).
    ///
    /// # Errors
    ///
    /// Returns error if write fails.
    pub async fn write_line(&mut self, line: &str) -> io::Result<()> {
        match &mut self.inner {
            ConnectionInner::Tcp { writer, .. } => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            #[cfg(unix)]
            ConnectionInner::Unix { writer, .. } => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }
        Ok(())
    }

    /// Read a line from the server.
    ///
    /// # Errors
    ///
    /// Returns error if read fails or connection closed.
    pub async fn read_line(&mut self) -> io::Result<String> {
        let mut line = String::new();
        let bytes_read = match &mut self.inner {
            ConnectionInner::Tcp { reader, .. } => reader.read_line(&mut line).await?,
            #[cfg(unix)]
            ConnectionInner::Unix { reader, .. } => reader.read_line(&mut line).await?,
        };

        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Connection closed"));
        }

        // Remove trailing newline
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        Ok(line)
    }

    /// Split connection into separate reader and writer handles.
    ///
    /// Use this for concurrent reading and writing, e.g., when the TUI
    /// needs to listen for notifications while also sending requests.
    #[must_use]
    pub fn split(self) -> (ConnectionReader, ConnectionWriter) {
        match self.inner {
            ConnectionInner::Tcp { reader, writer } => (
                ConnectionReader {
                    inner: ReaderInner::Tcp(reader),
                },
                ConnectionWriter {
                    inner: WriterInner::Tcp(writer),
                },
            ),
            #[cfg(unix)]
            ConnectionInner::Unix { reader, writer } => (
                ConnectionReader {
                    inner: ReaderInner::Unix(reader),
                },
                ConnectionWriter {
                    inner: WriterInner::Unix(writer),
                },
            ),
        }
    }
}

/// Reader half of a split connection.
///
/// Created by [`Connection::split`]. Can be moved to a separate task
/// for concurrent notification listening.
pub struct ConnectionReader {
    inner: ReaderInner,
}

enum ReaderInner {
    Tcp(BufReader<tokio::net::tcp::OwnedReadHalf>),
    #[cfg(unix)]
    Unix(BufReader<tokio::net::unix::OwnedReadHalf>),
}

impl ConnectionReader {
    /// Read a line from the server.
    ///
    /// # Errors
    ///
    /// Returns error if read fails or connection closed.
    pub async fn read_line(&mut self) -> io::Result<String> {
        let mut line = String::new();
        let bytes_read = match &mut self.inner {
            ReaderInner::Tcp(reader) => reader.read_line(&mut line).await?,
            #[cfg(unix)]
            ReaderInner::Unix(reader) => reader.read_line(&mut line).await?,
        };

        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Connection closed"));
        }

        // Remove trailing newline
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        Ok(line)
    }
}

/// Writer half of a split connection.
///
/// Created by [`Connection::split`]. Used for sending requests
/// while a separate task handles incoming notifications.
pub struct ConnectionWriter {
    inner: WriterInner,
}

enum WriterInner {
    Tcp(BufWriter<tokio::net::tcp::OwnedWriteHalf>),
    #[cfg(unix)]
    Unix(BufWriter<tokio::net::unix::OwnedWriteHalf>),
}

impl ConnectionWriter {
    /// Write a line to the server (appends newline).
    ///
    /// # Errors
    ///
    /// Returns error if write fails.
    pub async fn write_line(&mut self, line: &str) -> io::Result<()> {
        match &mut self.inner {
            WriterInner::Tcp(writer) => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            #[cfg(unix)]
            WriterInner::Unix(writer) => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tcp_valid() {
        let config = ConnectionConfig::parse_tcp("127.0.0.1:12521").unwrap();
        match config {
            ConnectionConfig::Tcp { host, port } => {
                assert_eq!(host, "127.0.0.1");
                assert_eq!(port, 12521);
            }
            #[cfg(unix)]
            _ => panic!("Expected TCP config"),
        }
    }

    #[test]
    fn test_parse_tcp_ipv6() {
        let config = ConnectionConfig::parse_tcp("[::1]:9000").unwrap();
        match config {
            ConnectionConfig::Tcp { host, port } => {
                assert_eq!(host, "[::1]");
                assert_eq!(port, 9000);
            }
            #[cfg(unix)]
            _ => panic!("Expected TCP config"),
        }
    }

    #[test]
    fn test_parse_tcp_invalid() {
        assert!(ConnectionConfig::parse_tcp("invalid").is_err());
        assert!(ConnectionConfig::parse_tcp("host:notaport").is_err());
    }

    #[test]
    fn test_from_flags_tcp_wins() {
        // TCP flag takes highest precedence
        let config = ConnectionConfig::from_flags(
            Some("127.0.0.1:9000"),
            Some(std::path::Path::new("/tmp/sock")),
            Some("instance"),
        )
        .unwrap();
        match config {
            ConnectionConfig::Tcp { host, port } => {
                assert_eq!(host, "127.0.0.1");
                assert_eq!(port, 9000);
            }
            #[cfg(unix)]
            _ => panic!("Expected TCP config"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_from_flags_socket_wins_over_instance() {
        // Socket path takes precedence over instance
        let config = ConnectionConfig::from_flags(
            None,
            Some(std::path::Path::new("/tmp/test.sock")),
            Some("myinstance"),
        )
        .unwrap();
        match config {
            ConnectionConfig::UnixSocket(path) => {
                assert_eq!(path, std::path::PathBuf::from("/tmp/test.sock"));
            }
            ConnectionConfig::Tcp { .. } => panic!("Expected UnixSocket config"),
        }
    }

    #[test]
    fn test_from_flags_instance_not_found() {
        // Instance lookup fails for non-existent instance
        let result = ConnectionConfig::from_flags(None, None, Some("nonexistent-instance-12345"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not found"));
        assert!(err.contains("reovim server -L"));
    }

    #[test]
    fn test_from_flags_instance_invalid_name() {
        // Invalid instance name should fail validation
        let result = ConnectionConfig::from_flags(None, None, Some("../etc/passwd"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Invalid instance name"));
    }

    #[test]
    fn test_from_instance_invalid_name_empty() {
        let result = ConnectionConfig::from_instance("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid instance name"));
    }

    #[test]
    fn test_from_instance_invalid_name_too_long() {
        let long_name = "a".repeat(64);
        let result = ConnectionConfig::from_instance(&long_name);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid instance name"));
    }

    #[test]
    fn test_from_instance_invalid_name_special_chars() {
        let result = ConnectionConfig::from_instance("has spaces");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid instance name"));
    }

    #[test]
    fn test_from_transport_info_tcp() {
        let transport = TransportInfo::Tcp {
            host: "192.168.1.1".to_string(),
            port: 8080,
        };
        let config = ConnectionConfig::from_transport_info(&transport).unwrap();
        match config {
            ConnectionConfig::Tcp { host, port } => {
                assert_eq!(host, "192.168.1.1");
                assert_eq!(port, 8080);
            }
            #[cfg(unix)]
            _ => panic!("Expected TCP config"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_from_transport_info_local() {
        let transport = TransportInfo::Local {
            path: "/tmp/reovim.sock".to_string(),
        };
        let config = ConnectionConfig::from_transport_info(&transport).unwrap();
        match config {
            ConnectionConfig::UnixSocket(path) => {
                assert_eq!(path, std::path::PathBuf::from("/tmp/reovim.sock"));
            }
            ConnectionConfig::Tcp { .. } => panic!("Expected UnixSocket config"),
        }
    }
}
