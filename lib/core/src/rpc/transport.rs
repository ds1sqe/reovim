//! Transport layer abstraction for RPC communication
//!
//! Supports multiple transport types:
//! - Stdio: JSON-RPC over stdin/stdout (for process piping)
//! - Unix socket: JSON-RPC over Unix domain socket
//! - TCP: JSON-RPC over TCP connection

use std::{io, path::PathBuf};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
};

/// Transport configuration
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// Use stdin/stdout for communication
    Stdio,
    /// Listen on Unix socket at the given path
    UnixSocket { path: PathBuf },
    /// Listen on TCP at the given address
    Tcp { host: String, port: u16 },
}

impl TransportConfig {
    /// Create Unix socket config
    #[must_use]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self::UnixSocket { path: path.into() }
    }

    /// Create TCP config
    #[must_use]
    pub fn tcp(host: impl Into<String>, port: u16) -> Self {
        Self::Tcp {
            host: host.into(),
            port,
        }
    }

    /// Create TCP config with default host (localhost)
    #[must_use]
    pub fn tcp_localhost(port: u16) -> Self {
        Self::tcp("127.0.0.1", port)
    }
}

/// Read half of a transport connection
pub struct TransportReader {
    inner: ReaderInner,
}

enum ReaderInner {
    Stdio(BufReader<tokio::io::Stdin>),
    UnixSocket(BufReader<tokio::net::unix::OwnedReadHalf>),
    Tcp(BufReader<tokio::net::tcp::OwnedReadHalf>),
}

impl TransportReader {
    /// Read a line from the transport
    ///
    /// Returns `Ok(None)` on EOF.
    ///
    /// # Errors
    ///
    /// Returns an error if reading from the underlying transport fails.
    pub async fn read_line(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        let bytes_read = match &mut self.inner {
            ReaderInner::Stdio(reader) => reader.read_line(&mut line).await?,
            ReaderInner::UnixSocket(reader) => reader.read_line(&mut line).await?,
            ReaderInner::Tcp(reader) => reader.read_line(&mut line).await?,
        };

        if bytes_read == 0 {
            Ok(None) // EOF
        } else {
            Ok(Some(line))
        }
    }
}

/// Write half of a transport connection
pub struct TransportWriter {
    inner: WriterInner,
}

enum WriterInner {
    Stdio(BufWriter<tokio::io::Stdout>),
    UnixSocket(BufWriter<tokio::net::unix::OwnedWriteHalf>),
    Tcp(BufWriter<tokio::net::tcp::OwnedWriteHalf>),
}

impl TransportWriter {
    /// Write a line to the transport (appends newline and flushes)
    ///
    /// # Errors
    ///
    /// Returns an error if writing to or flushing the underlying transport fails.
    pub async fn write_line(&mut self, line: &str) -> io::Result<()> {
        match &mut self.inner {
            WriterInner::Stdio(writer) => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            WriterInner::UnixSocket(writer) => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            WriterInner::Tcp(writer) => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }
        Ok(())
    }
}

/// A transport connection (read + write halves)
pub struct TransportConnection {
    pub reader: TransportReader,
    pub writer: TransportWriter,
}

impl TransportConnection {
    /// Create stdio transport (for server spawned by parent process)
    #[must_use]
    pub fn stdio() -> Self {
        let reader = TransportReader {
            inner: ReaderInner::Stdio(BufReader::new(tokio::io::stdin())),
        };
        let writer = TransportWriter {
            inner: WriterInner::Stdio(BufWriter::new(tokio::io::stdout())),
        };
        Self { reader, writer }
    }

    /// Create from Unix socket stream
    fn from_unix_stream(stream: UnixStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        let reader = TransportReader {
            inner: ReaderInner::UnixSocket(BufReader::new(read_half)),
        };
        let writer = TransportWriter {
            inner: WriterInner::UnixSocket(BufWriter::new(write_half)),
        };
        Self { reader, writer }
    }

    /// Create from TCP stream
    fn from_tcp_stream(stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        let reader = TransportReader {
            inner: ReaderInner::Tcp(BufReader::new(read_half)),
        };
        let writer = TransportWriter {
            inner: WriterInner::Tcp(BufWriter::new(write_half)),
        };
        Self { reader, writer }
    }
}

/// Listener for accepting transport connections
pub enum TransportListener {
    /// Unix socket listener
    UnixSocket {
        listener: UnixListener,
        path: PathBuf,
    },
    /// TCP listener
    Tcp { listener: TcpListener },
}

impl TransportListener {
    /// Bind to TCP with port fallback
    ///
    /// Tries ports starting from `start_port` up to `start_port + max_attempts - 1`.
    /// Returns the listener and the actual port that was bound.
    ///
    /// # Errors
    ///
    /// Returns an error if all ports in the range are unavailable or if binding fails
    /// for a reason other than "address in use".
    pub async fn bind_tcp_with_fallback(
        host: &str,
        start_port: u16,
        max_attempts: u16,
    ) -> io::Result<(Self, u16)> {
        for offset in 0..max_attempts {
            let port = start_port.saturating_add(offset);
            let addr = format!("{host}:{port}");
            match TcpListener::bind(&addr).await {
                Ok(listener) => {
                    if offset > 0 {
                        tracing::info!("Port {} in use, bound to {} instead", start_port, port);
                    } else {
                        tracing::info!("Listening on TCP: {}", addr);
                    }
                    return Ok((Self::Tcp { listener }, port));
                }
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
                    tracing::debug!("Port {} in use, trying next", port);
                }
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!(
                "No available port in range {}-{}",
                start_port,
                start_port.saturating_add(max_attempts - 1)
            ),
        ))
    }

    /// Bind to the given transport configuration
    ///
    /// Returns `None` for stdio (no listener needed).
    ///
    /// # Errors
    ///
    /// Returns an error if binding to the socket or TCP address fails.
    pub async fn bind(config: &TransportConfig) -> io::Result<Option<Self>> {
        match config {
            TransportConfig::Stdio => Ok(None),
            TransportConfig::UnixSocket { path } => {
                // Remove existing socket file if present
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
                let listener = UnixListener::bind(path)?;
                tracing::info!("Listening on Unix socket: {}", path.display());
                Ok(Some(Self::UnixSocket {
                    listener,
                    path: path.clone(),
                }))
            }
            TransportConfig::Tcp { host, port } => {
                let addr = format!("{host}:{port}");
                let listener = TcpListener::bind(&addr).await?;
                tracing::info!("Listening on TCP: {}", addr);
                Ok(Some(Self::Tcp { listener }))
            }
        }
    }

    /// Accept a new connection
    ///
    /// # Errors
    ///
    /// Returns an error if accepting the connection fails.
    pub async fn accept(&self) -> io::Result<TransportConnection> {
        match self {
            Self::UnixSocket { listener, .. } => {
                let (stream, _addr) = listener.accept().await?;
                tracing::debug!("Accepted Unix socket connection");
                Ok(TransportConnection::from_unix_stream(stream))
            }
            Self::Tcp { listener } => {
                let (stream, addr) = listener.accept().await?;
                tracing::debug!("Accepted TCP connection from {}", addr);
                Ok(TransportConnection::from_tcp_stream(stream))
            }
        }
    }

    /// Get the address/path this listener is bound to
    #[must_use]
    pub fn local_addr_string(&self) -> String {
        match self {
            Self::UnixSocket { path, .. } => path.display().to_string(),
            Self::Tcp { listener } => listener
                .local_addr()
                .map_or_else(|_| "unknown".to_string(), |addr| addr.to_string()),
        }
    }
}

impl Drop for TransportListener {
    fn drop(&mut self) {
        // Clean up Unix socket file on drop
        if let Self::UnixSocket { path, .. } = self {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Client-side transport for connecting to a server
pub struct TransportClient;

impl TransportClient {
    /// Connect to a server via Unix socket
    ///
    /// # Errors
    ///
    /// Returns an error if connecting to the Unix socket fails.
    pub async fn connect_unix(
        path: impl AsRef<std::path::Path>,
    ) -> io::Result<TransportConnection> {
        let stream = UnixStream::connect(path).await?;
        Ok(TransportConnection::from_unix_stream(stream))
    }

    /// Connect to a server via TCP
    ///
    /// # Errors
    ///
    /// Returns an error if connecting to the TCP address fails.
    pub async fn connect_tcp(host: &str, port: u16) -> io::Result<TransportConnection> {
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect(&addr).await?;
        Ok(TransportConnection::from_tcp_stream(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
