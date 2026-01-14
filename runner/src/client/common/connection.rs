//! Connection handling for outbound client connections.
//!
//! Supports TCP and Unix socket connections to a reovim server.

use std::{io, path::PathBuf};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
};

#[cfg(unix)]
use tokio::net::UnixStream;

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
    #[must_use]
    pub fn tcp_default() -> Self {
        Self::Tcp {
            host: "127.0.0.1".to_string(),
            port: 12521,
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
    /// Scans default ports and returns the first available server.
    /// Falls back to default port if no server found.
    #[must_use]
    pub fn auto_discover() -> Self {
        use super::discovery;

        let servers = discovery::list_servers();
        servers
            .first()
            .map_or_else(Self::tcp_default, |server| Self::Tcp {
                host: server.host.clone(),
                port: server.port,
            })
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
}
