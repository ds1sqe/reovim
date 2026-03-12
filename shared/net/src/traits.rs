//! Network driver traits.

use {
    crate::{NetError, RpcHandler, TransportConfig},
    std::net::SocketAddr,
};

/// Network driver for RPC server lifecycle management.
///
/// This trait manages the network subsystem lifecycle:
/// initialization, listening for connections, and clean shutdown.
pub trait NetDriver: Send + Sync {
    /// Initialize the network driver.
    ///
    /// # Errors
    /// Returns [`NetError::NotInitialized`] if initialization fails.
    fn init(&mut self) -> Result<(), NetError>;

    /// Start listening for connections on the specified transport.
    ///
    /// # Errors
    /// Returns [`NetError::BindFailed`] if binding fails, or
    /// [`NetError::AlreadyListening`] if already listening.
    fn listen(&mut self, config: TransportConfig) -> Result<(), NetError>;

    /// Shutdown the network driver.
    ///
    /// # Errors
    /// Returns [`NetError::Io`] if shutdown fails.
    fn shutdown(&mut self) -> Result<(), NetError>;

    /// Check if the driver is currently listening.
    fn is_listening(&self) -> bool;

    /// Get the local address if listening on TCP.
    fn local_addr(&self) -> Option<SocketAddr>;

    /// Register an RPC handler for a custom method.
    ///
    /// # Errors
    /// Returns [`NetError::NotInitialized`] if driver not initialized.
    fn register_handler(&mut self, handler: Box<dyn RpcHandler>) -> Result<(), NetError>;
}

/// Transport listener for accepting connections.
pub trait TransportListener: Send + Sync {
    /// Bind to the configured address and start listening.
    ///
    /// # Errors
    /// Returns [`NetError::BindFailed`] if binding fails.
    fn bind(&mut self, config: TransportConfig) -> Result<(), NetError>;

    /// Accept a new connection (blocking).
    ///
    /// # Errors
    /// Returns [`NetError::AcceptFailed`] if accepting fails.
    fn accept(&mut self) -> Result<Box<dyn TransportConnection>, NetError>;

    /// Get the local address (TCP only).
    fn local_addr(&self) -> Option<SocketAddr>;
}

/// Active transport connection for reading/writing messages.
pub trait TransportConnection: Send + Sync {
    /// Read a line from the connection.
    /// Returns `Ok(None)` on EOF.
    ///
    /// # Errors
    /// Returns [`NetError::ReadFailed`] if reading fails.
    fn read_line(&mut self) -> Result<Option<String>, NetError>;

    /// Write a line to the connection (appends newline, flushes).
    ///
    /// # Errors
    /// Returns [`NetError::WriteFailed`] if writing fails.
    fn write_line(&mut self, line: &str) -> Result<(), NetError>;

    /// Close the connection gracefully.
    ///
    /// # Errors
    /// Returns [`NetError::Io`] if closing fails.
    fn close(&mut self) -> Result<(), NetError>;
}

/// Port allocator for multi-instance TCP server support.
pub trait PortAllocator: Send + Sync {
    /// Get the default port. Default: 12521 ('r'*100 + 'e'*10 + 'o')
    fn default_port(&self) -> u16 {
        crate::transport::TransportConfig::DEFAULT_PORT
    }

    /// Get the port fallback range. Default: (12521, 12530)
    fn port_range(&self) -> (u16, u16) {
        (
            crate::transport::TransportConfig::DEFAULT_PORT,
            crate::transport::TransportConfig::MAX_PORT,
        )
    }

    /// Try to allocate a port, starting with default and falling back.
    ///
    /// # Errors
    /// Returns [`NetError::PortExhausted`] if no ports are available.
    fn allocate(&self) -> Result<u16, NetError>;

    /// Check if a port is available.
    fn is_port_available(&self, port: u16) -> bool;
}

#[cfg(test)]
#[path = "traits_tests.rs"]
mod tests;
