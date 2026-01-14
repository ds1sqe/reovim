//! TCP transport for the reovim server.
//!
//! Handles TCP socket binding with port fallback and connection acceptance.

use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};

/// Default TCP port for reovim server.
pub const DEFAULT_PORT: u16 = 12521;

/// Number of ports to try when the default is busy.
pub const PORT_FALLBACK_COUNT: u16 = 10;

/// Default bind address (localhost only for security).
pub const DEFAULT_HOST: &str = "127.0.0.1";

/// TCP listener with port fallback.
///
/// Wraps `tokio::net::TcpListener` with automatic port fallback when the
/// default port is busy. This allows multiple reovim servers to run
/// concurrently for development/debugging.
///
/// # Example
///
/// ```ignore
/// use runner::transport::TcpTransport;
///
/// // Bind with automatic port fallback
/// let transport = TcpTransport::bind_with_fallback().await?;
/// println!("Listening on {}", transport.local_addr());
///
/// // Accept connections
/// loop {
///     let (stream, addr) = transport.accept().await?;
///     // Handle connection...
/// }
/// ```
pub struct TcpTransport {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl TcpTransport {
    /// Bind to a specific address.
    ///
    /// # Errors
    ///
    /// Returns an error if binding fails.
    pub async fn bind(addr: SocketAddr) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        Ok(Self {
            listener,
            local_addr,
        })
    }

    /// Bind to the default address with port fallback.
    ///
    /// Tries ports starting from [`DEFAULT_PORT`] up to
    /// `DEFAULT_PORT + PORT_FALLBACK_COUNT - 1`.
    ///
    /// # Errors
    ///
    /// Returns an error if all ports in the range fail to bind.
    pub async fn bind_with_fallback() -> std::io::Result<Self> {
        Self::bind_with_fallback_on(DEFAULT_HOST).await
    }

    /// Bind to a specific host with port fallback.
    ///
    /// # Errors
    ///
    /// Returns an error if all ports in the range fail to bind.
    pub async fn bind_with_fallback_on(host: &str) -> std::io::Result<Self> {
        let mut last_error = None;

        for port_offset in 0..PORT_FALLBACK_COUNT {
            let port = DEFAULT_PORT + port_offset;
            let addr_str = format!("{host}:{port}");

            match addr_str.parse::<SocketAddr>() {
                Ok(addr) => match Self::bind(addr).await {
                    Ok(transport) => return Ok(transport),
                    Err(e) => {
                        last_error = Some(e);
                    }
                },
                Err(e) => {
                    last_error = Some(std::io::Error::new(std::io::ErrorKind::InvalidInput, e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                format!(
                    "Failed to bind to any port in range {DEFAULT_PORT}-{}",
                    DEFAULT_PORT + PORT_FALLBACK_COUNT - 1
                ),
            )
        }))
    }

    /// Bind to a specific port on localhost.
    ///
    /// # Errors
    ///
    /// Returns an error if binding fails.
    pub async fn bind_port(port: u16) -> std::io::Result<Self> {
        let addr: SocketAddr = format!("{DEFAULT_HOST}:{port}")
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        Self::bind(addr).await
    }

    /// Get the local address this transport is bound to.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Get the port this transport is bound to.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.local_addr.port()
    }

    /// Accept a new connection.
    ///
    /// Returns the TCP stream and the remote address.
    ///
    /// # Errors
    ///
    /// Returns an error if accepting fails.
    pub async fn accept(&self) -> std::io::Result<(TcpStream, SocketAddr)> {
        self.listener.accept().await
    }
}

impl std::fmt::Debug for TcpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpTransport")
            .field("local_addr", &self.local_addr)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tcp_transport_bind_with_fallback() {
        // Should successfully bind to some port
        let transport = TcpTransport::bind_with_fallback().await.unwrap();

        // Port should be in the expected range
        let port = transport.port();
        assert!(
            (DEFAULT_PORT..DEFAULT_PORT + PORT_FALLBACK_COUNT).contains(&port),
            "Port {port} not in expected range"
        );

        // Address should be localhost
        assert_eq!(transport.local_addr().ip().to_string(), DEFAULT_HOST);
    }

    #[tokio::test]
    async fn test_tcp_transport_bind_specific_port() {
        // Use a high port to avoid conflicts
        let port = 32521;
        let transport = TcpTransport::bind_port(port).await.unwrap();

        assert_eq!(transport.port(), port);
    }

    #[tokio::test]
    async fn test_tcp_transport_local_addr() {
        let transport = TcpTransport::bind_with_fallback().await.unwrap();
        let addr = transport.local_addr();

        assert_eq!(addr.ip().to_string(), DEFAULT_HOST);
    }
}
