//! Transport listeners for TCP and Unix sockets.
//!
//! Provides `TransportListener` that abstracts over different listener types,
//! enabling the server to accept connections from TCP or Unix socket transports.
//!
//! # Example
//!
//! ```ignore
//! use runner::transport::TransportListener;
//!
//! // TCP with automatic port fallback
//! let listener = TransportListener::bind_tcp_with_fallback().await?;
//! eprintln!("Listening on {}", listener.local_addr_string());
//!
//! // Or Unix socket
//! let listener = TransportListener::bind_unix("/tmp/reovim.sock").await?;
//!
//! // Accept loop
//! loop {
//!     let (reader, writer) = listener.accept().await?;
//!     // Handle client...
//! }
//! ```

use std::{io, path::PathBuf};

use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;

use super::{
    connection::{TransportReader, TransportWriter},
    tcp::{DEFAULT_HOST, DEFAULT_PORT, PORT_FALLBACK_COUNT},
};

/// Listener supporting TCP and Unix sockets.
///
/// Abstracts over different listener types to provide a unified accept interface.
/// The listener returns `TransportReader`/`TransportWriter` pairs on accept.
pub enum TransportListener {
    /// TCP listener with address info.
    Tcp {
        listener: TcpListener,
        local_addr: std::net::SocketAddr,
    },
    /// Unix socket listener with path info.
    #[cfg(unix)]
    Unix {
        listener: UnixListener,
        path: PathBuf,
    },
}

impl TransportListener {
    /// Bind to TCP with automatic port fallback.
    ///
    /// Tries ports from `DEFAULT_PORT` (12521) up to `DEFAULT_PORT + PORT_FALLBACK_COUNT - 1`.
    /// This allows multiple reovim servers to run concurrently for development.
    ///
    /// # Errors
    ///
    /// Returns an error if all ports in the range fail to bind.
    pub async fn bind_tcp_with_fallback() -> io::Result<Self> {
        let mut last_error = None;

        for port_offset in 0..PORT_FALLBACK_COUNT {
            let port = DEFAULT_PORT + port_offset;
            match Self::bind_tcp(port).await {
                Ok(listener) => return Ok(listener),
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                format!(
                    "Failed to bind to any port in range {DEFAULT_PORT}-{}",
                    DEFAULT_PORT + PORT_FALLBACK_COUNT - 1
                ),
            )
        }))
    }

    /// Bind to a specific TCP port on localhost.
    ///
    /// # Errors
    ///
    /// Returns an error if binding fails.
    pub async fn bind_tcp(port: u16) -> io::Result<Self> {
        let addr = format!("{DEFAULT_HOST}:{port}");
        let listener = TcpListener::bind(&addr).await?;
        let local_addr = listener.local_addr()?;

        Ok(Self::Tcp {
            listener,
            local_addr,
        })
    }

    /// Bind to a Unix socket at the given path.
    ///
    /// If a socket file already exists at the path, it will be removed first.
    /// This handles stale socket files from crashed processes.
    ///
    /// # Errors
    ///
    /// Returns an error if binding fails.
    #[cfg(unix)]
    pub fn bind_unix(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();

        // Remove existing socket file if present (handles stale sockets)
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        let listener = UnixListener::bind(&path)?;

        Ok(Self::Unix { listener, path })
    }

    /// Accept a new connection.
    ///
    /// Returns a `TransportReader`/`TransportWriter` pair for the new connection.
    ///
    /// # Errors
    ///
    /// Returns an error if accepting fails.
    pub async fn accept(&self) -> io::Result<(TransportReader, TransportWriter)> {
        match self {
            Self::Tcp { listener, .. } => {
                let (stream, _addr) = listener.accept().await?;
                let (read_half, write_half) = stream.into_split();
                Ok((TransportReader::from_tcp(read_half), TransportWriter::from_tcp(write_half)))
            }
            #[cfg(unix)]
            Self::Unix { listener, .. } => {
                let (stream, _addr) = listener.accept().await?;
                let (read_half, write_half) = stream.into_split();
                Ok((TransportReader::from_unix(read_half), TransportWriter::from_unix(write_half)))
            }
        }
    }

    /// Get a human-readable string describing the local address.
    ///
    /// For TCP: "127.0.0.1:12521"
    /// For Unix: "/tmp/reovim.sock"
    #[must_use]
    pub fn local_addr_string(&self) -> String {
        match self {
            Self::Tcp { local_addr, .. } => local_addr.to_string(),
            #[cfg(unix)]
            Self::Unix { path, .. } => path.display().to_string(),
        }
    }

    /// Get the local socket address for TCP listeners.
    ///
    /// Returns the bound `SocketAddr` for TCP.
    ///
    /// # Panics
    ///
    /// Panics if called on a Unix socket listener, which has no `SocketAddr`.
    /// Use `tcp_port()` or `local_addr_string()` for safer access.
    #[must_use]
    pub const fn local_addr(&self) -> std::net::SocketAddr {
        match self {
            Self::Tcp { local_addr, .. } => *local_addr,
            #[cfg(unix)]
            Self::Unix { .. } => panic!("Unix socket has no SocketAddr"),
        }
    }

    /// Get the TCP port if this is a TCP listener.
    #[must_use]
    pub const fn tcp_port(&self) -> Option<u16> {
        match self {
            Self::Tcp { local_addr, .. } => Some(local_addr.port()),
            #[cfg(unix)]
            Self::Unix { .. } => None,
        }
    }
}

impl std::fmt::Debug for TransportListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tcp { local_addr, .. } => f
                .debug_struct("TransportListener::Tcp")
                .field("local_addr", local_addr)
                .finish_non_exhaustive(),
            #[cfg(unix)]
            Self::Unix { path, .. } => f
                .debug_struct("TransportListener::Unix")
                .field("path", path)
                .finish_non_exhaustive(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_listener_bind_tcp_with_fallback() {
        let listener = TransportListener::bind_tcp_with_fallback().await.unwrap();

        match &listener {
            TransportListener::Tcp { local_addr, .. } => {
                assert_eq!(local_addr.ip().to_string(), DEFAULT_HOST);
                assert!(local_addr.port() >= DEFAULT_PORT);
                assert!(local_addr.port() < DEFAULT_PORT + PORT_FALLBACK_COUNT);
            }
            #[cfg(unix)]
            _ => panic!("Expected TCP listener"),
        }
    }

    #[tokio::test]
    async fn test_listener_bind_tcp_specific_port() {
        // Use a high port to avoid conflicts
        let port = 32700;
        let listener = TransportListener::bind_tcp(port).await.unwrap();

        assert_eq!(listener.tcp_port(), Some(port));
    }

    #[tokio::test]
    async fn test_listener_local_addr_string_tcp() {
        let listener = TransportListener::bind_tcp_with_fallback().await.unwrap();
        let addr = listener.local_addr_string();

        assert!(addr.starts_with("127.0.0.1:"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_listener_bind_unix() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");

        let listener = TransportListener::bind_unix(&sock_path).unwrap();

        match &listener {
            TransportListener::Unix { path, .. } => {
                assert_eq!(*path, sock_path);
                assert!(sock_path.exists());
            }
            TransportListener::Tcp { .. } => panic!("Expected Unix listener"),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_listener_unix_removes_existing_socket() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");

        // Create first listener
        let listener1 = TransportListener::bind_unix(&sock_path).unwrap();
        drop(listener1);

        // Second listener should succeed (removes stale socket)
        let listener2 = TransportListener::bind_unix(&sock_path).unwrap();
        assert!(matches!(listener2, TransportListener::Unix { .. }));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_listener_local_addr_string_unix() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("addr.sock");

        let listener = TransportListener::bind_unix(&sock_path).unwrap();
        let addr = listener.local_addr_string();

        assert!(addr.contains("addr.sock"));
    }

    #[tokio::test]
    async fn test_listener_tcp_accept() {
        let port = 32701;
        let listener = TransportListener::bind_tcp(port).await.unwrap();

        let addr = format!("127.0.0.1:{port}");

        // Connect from client in background
        let client_task =
            tokio::spawn(async move { tokio::net::TcpStream::connect(addr).await.unwrap() });

        // Accept on server
        let (reader, writer) = listener.accept().await.unwrap();

        // Both sides connected
        let _ = client_task.await.unwrap();

        // Verify we got valid reader/writer
        let _ = format!("{reader:?}");
        let _ = format!("{writer:?}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_listener_unix_accept() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("accept.sock");

        let listener = TransportListener::bind_unix(&sock_path).unwrap();

        // Connect from client in background
        let path = sock_path.clone();
        let client_task =
            tokio::spawn(async move { tokio::net::UnixStream::connect(path).await.unwrap() });

        // Accept on server
        let (reader, writer) = listener.accept().await.unwrap();

        let _ = client_task.await.unwrap();

        // Verify we got valid reader/writer
        let _ = format!("{reader:?}");
        let _ = format!("{writer:?}");
    }
}
