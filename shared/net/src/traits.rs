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
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_default_port_allocator() {
        struct DummyAllocator;
        impl PortAllocator for DummyAllocator {
            fn allocate(&self) -> Result<u16, NetError> {
                Ok(12521)
            }
            fn is_port_available(&self, _port: u16) -> bool {
                true
            }
        }

        let allocator = DummyAllocator;
        assert_eq!(allocator.default_port(), 12521);
        assert_eq!(allocator.port_range(), (12521, 12530));
    }

    #[test]
    fn test_trait_bounds() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<dyn NetDriver>();
        assert_send_sync::<dyn TransportListener>();
        assert_send_sync::<dyn TransportConnection>();
        assert_send_sync::<dyn PortAllocator>();
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_port_allocator_allocate_success() {
        struct SuccessAllocator;
        impl PortAllocator for SuccessAllocator {
            fn allocate(&self) -> Result<u16, NetError> {
                Ok(12525)
            }
            fn is_port_available(&self, _port: u16) -> bool {
                true
            }
        }

        let allocator = SuccessAllocator;
        assert_eq!(allocator.allocate().unwrap(), 12525);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_port_allocator_allocate_exhausted() {
        struct ExhaustedAllocator;
        impl PortAllocator for ExhaustedAllocator {
            fn allocate(&self) -> Result<u16, NetError> {
                Err(NetError::PortExhausted)
            }
            fn is_port_available(&self, _port: u16) -> bool {
                false
            }
        }

        let allocator = ExhaustedAllocator;
        let result = allocator.allocate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NetError::PortExhausted));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_port_allocator_is_port_available() {
        struct SelectiveAllocator;
        impl PortAllocator for SelectiveAllocator {
            fn allocate(&self) -> Result<u16, NetError> {
                Ok(12521)
            }
            fn is_port_available(&self, port: u16) -> bool {
                port == 12521
            }
        }

        let allocator = SelectiveAllocator;
        assert!(allocator.is_port_available(12521));
        assert!(!allocator.is_port_available(12522));
        assert!(!allocator.is_port_available(0));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_port_allocator_custom_port_range() {
        struct CustomAllocator;
        impl PortAllocator for CustomAllocator {
            fn default_port(&self) -> u16 {
                9000
            }
            fn port_range(&self) -> (u16, u16) {
                (9000, 9010)
            }
            fn allocate(&self) -> Result<u16, NetError> {
                Ok(9000)
            }
            fn is_port_available(&self, _port: u16) -> bool {
                true
            }
        }

        let allocator = CustomAllocator;
        assert_eq!(allocator.default_port(), 9000);
        assert_eq!(allocator.port_range(), (9000, 9010));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_port_allocator_default_implementations() {
        // Verify default trait methods return the correct constants
        struct MinimalAllocator;
        impl PortAllocator for MinimalAllocator {
            fn allocate(&self) -> Result<u16, NetError> {
                Ok(self.default_port())
            }
            fn is_port_available(&self, _port: u16) -> bool {
                true
            }
        }

        let allocator = MinimalAllocator;
        let (start, end) = allocator.port_range();
        assert_eq!(start, allocator.default_port());
        assert!(end > start);
        assert_eq!(allocator.allocate().unwrap(), 12521);
    }

    // =========================================================================
    // Mock NetDriver
    // =========================================================================

    struct MockNetDriver {
        initialized: bool,
        listening: bool,
    }

    impl MockNetDriver {
        fn new() -> Self {
            Self {
                initialized: false,
                listening: false,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl NetDriver for MockNetDriver {
        fn init(&mut self) -> Result<(), NetError> {
            self.initialized = true;
            Ok(())
        }
        fn listen(&mut self, _config: TransportConfig) -> Result<(), NetError> {
            if !self.initialized {
                return Err(NetError::NotInitialized);
            }
            if self.listening {
                return Err(NetError::AlreadyListening);
            }
            self.listening = true;
            Ok(())
        }
        fn shutdown(&mut self) -> Result<(), NetError> {
            self.listening = false;
            self.initialized = false;
            Ok(())
        }
        fn is_listening(&self) -> bool {
            self.listening
        }
        fn local_addr(&self) -> Option<SocketAddr> {
            if self.listening {
                Some("127.0.0.1:12521".parse().unwrap())
            } else {
                None
            }
        }
        fn register_handler(
            &mut self,
            _handler: Box<dyn crate::RpcHandler>,
        ) -> Result<(), NetError> {
            if !self.initialized {
                return Err(NetError::NotInitialized);
            }
            Ok(())
        }
    }

    #[test]
    fn test_mock_net_driver_lifecycle() {
        let mut driver = MockNetDriver::new();
        assert!(!driver.is_listening());
        assert!(driver.local_addr().is_none());

        driver.init().unwrap();

        driver
            .listen(TransportConfig::tcp_localhost(12521))
            .unwrap();
        assert!(driver.is_listening());
        assert!(driver.local_addr().is_some());

        driver.shutdown().unwrap();
        assert!(!driver.is_listening());
    }

    #[test]
    fn test_mock_net_driver_listen_before_init() {
        let mut driver = MockNetDriver::new();
        let result = driver.listen(TransportConfig::tcp_localhost(12521));
        assert!(matches!(result.unwrap_err(), NetError::NotInitialized));
    }

    #[test]
    fn test_mock_net_driver_already_listening() {
        let mut driver = MockNetDriver::new();
        driver.init().unwrap();
        driver
            .listen(TransportConfig::tcp_localhost(12521))
            .unwrap();
        let result = driver.listen(TransportConfig::tcp_localhost(12522));
        assert!(matches!(result.unwrap_err(), NetError::AlreadyListening));
    }

    struct DummyHandler;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl crate::RpcHandler for DummyHandler {
        fn method(&self) -> &'static str {
            "test"
        }
        fn handle(
            &self,
            _params: &serde_json::Value,
            _ctx: &crate::RpcHandlerContext,
        ) -> crate::RpcResult {
            crate::RpcResult::ok()
        }
    }

    #[test]
    fn test_mock_net_driver_register_handler_before_init() {
        let mut driver = MockNetDriver::new();
        let result = driver.register_handler(Box::new(DummyHandler));
        assert!(matches!(result.unwrap_err(), NetError::NotInitialized));
    }

    // =========================================================================
    // Mock TransportListener and TransportConnection
    // =========================================================================

    struct MockConnection {
        lines: Vec<String>,
        closed: bool,
    }

    impl MockConnection {
        fn new(lines: Vec<String>) -> Self {
            Self {
                lines,
                closed: false,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl TransportConnection for MockConnection {
        fn read_line(&mut self) -> Result<Option<String>, NetError> {
            if self.lines.is_empty() {
                Ok(None) // EOF
            } else {
                Ok(Some(self.lines.remove(0)))
            }
        }
        fn write_line(&mut self, line: &str) -> Result<(), NetError> {
            self.lines.push(line.to_string());
            Ok(())
        }
        fn close(&mut self) -> Result<(), NetError> {
            self.closed = true;
            Ok(())
        }
    }

    #[test]
    fn test_mock_transport_connection() {
        let mut conn = MockConnection::new(vec!["hello".to_string(), "world".to_string()]);

        assert_eq!(conn.read_line().unwrap(), Some("hello".to_string()));
        assert_eq!(conn.read_line().unwrap(), Some("world".to_string()));
        assert_eq!(conn.read_line().unwrap(), None); // EOF

        conn.write_line("response").unwrap();
        assert_eq!(conn.read_line().unwrap(), Some("response".to_string()));

        conn.close().unwrap();
        assert!(conn.closed);
    }

    struct MockListener {
        connections: Vec<MockConnection>,
        addr: Option<SocketAddr>,
    }

    impl MockListener {
        fn new() -> Self {
            Self {
                connections: Vec::new(),
                addr: None,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl TransportListener for MockListener {
        fn bind(&mut self, config: TransportConfig) -> Result<(), NetError> {
            if let TransportConfig::Tcp { host, port } = config {
                self.addr = Some(
                    format!("{host}:{port}")
                        .parse()
                        .map_err(|_| NetError::BindFailed("invalid address".to_string()))?,
                );
                Ok(())
            } else {
                Err(NetError::BindFailed("unsupported transport".to_string()))
            }
        }
        fn accept(&mut self) -> Result<Box<dyn TransportConnection>, NetError> {
            if self.connections.is_empty() {
                Err(NetError::AcceptFailed("no pending connections".to_string()))
            } else {
                Ok(Box::new(self.connections.remove(0)))
            }
        }
        fn local_addr(&self) -> Option<SocketAddr> {
            self.addr
        }
    }

    #[test]
    fn test_mock_transport_listener_bind() {
        let mut listener = MockListener::new();
        listener.bind(TransportConfig::tcp_localhost(9999)).unwrap();
        assert!(listener.local_addr().is_some());
        assert_eq!(listener.local_addr().unwrap().port(), 9999);
    }

    #[test]
    fn test_mock_transport_listener_accept_empty() {
        let mut listener = MockListener::new();
        listener.bind(TransportConfig::tcp_localhost(9999)).unwrap();
        let result = listener.accept();
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_net_driver_register_handler_after_init() {
        let mut driver = MockNetDriver::new();
        driver.init().unwrap();
        let result = driver.register_handler(Box::new(DummyHandler));
        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_net_driver_shutdown_without_listen() {
        let mut driver = MockNetDriver::new();
        driver.init().unwrap();
        // Shutting down without ever listening should succeed
        let result = driver.shutdown();
        assert!(result.is_ok());
        assert!(!driver.is_listening());
        assert!(driver.local_addr().is_none());
    }

    #[test]
    fn test_mock_net_driver_local_addr_none_before_listen() {
        let mut driver = MockNetDriver::new();
        assert!(driver.local_addr().is_none());
        driver.init().unwrap();
        assert!(driver.local_addr().is_none());
    }

    #[test]
    fn test_mock_net_driver_local_addr_some_after_listen() {
        let mut driver = MockNetDriver::new();
        driver.init().unwrap();
        driver
            .listen(TransportConfig::tcp_localhost(12521))
            .unwrap();
        let addr = driver.local_addr().unwrap();
        assert_eq!(addr.port(), 12521);
        driver.shutdown().unwrap();
    }

    #[test]
    fn test_mock_connection_write_then_read() {
        let mut conn = MockConnection::new(vec![]);
        conn.write_line("first").unwrap();
        conn.write_line("second").unwrap();
        assert_eq!(conn.read_line().unwrap(), Some("first".to_string()));
        assert_eq!(conn.read_line().unwrap(), Some("second".to_string()));
        assert_eq!(conn.read_line().unwrap(), None);
    }

    #[test]
    fn test_mock_connection_close_idempotent() {
        let mut conn = MockConnection::new(vec![]);
        conn.close().unwrap();
        assert!(conn.closed);
        // Close again should be fine
        conn.close().unwrap();
        assert!(conn.closed);
    }

    #[test]
    fn test_mock_listener_no_addr_before_bind() {
        let listener = MockListener::new();
        assert!(listener.local_addr().is_none());
    }

    #[test]
    fn test_mock_listener_bind_non_tcp_fails() {
        let mut listener = MockListener::new();
        let result = listener.bind(TransportConfig::Stdio);
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_listener_bind_unix_socket_fails() {
        let mut listener = MockListener::new();
        let result = listener.bind(TransportConfig::unix_socket("/tmp/test.sock"));
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_listener_accept_with_connection() {
        let mut listener = MockListener::new();
        listener.bind(TransportConfig::tcp_localhost(9999)).unwrap();
        listener
            .connections
            .push(MockConnection::new(vec!["hello".to_string()]));

        let mut conn = listener.accept().unwrap();
        assert_eq!(conn.read_line().unwrap(), Some("hello".to_string()));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_port_allocator_uses_default_port_in_allocate() {
        struct DefaultPortAllocator;
        impl PortAllocator for DefaultPortAllocator {
            fn allocate(&self) -> Result<u16, NetError> {
                Ok(self.default_port())
            }
            fn is_port_available(&self, port: u16) -> bool {
                port >= self.default_port() && port <= self.port_range().1
            }
        }

        let allocator = DefaultPortAllocator;
        let port = allocator.allocate().unwrap();
        assert_eq!(port, 12521);
        assert!(allocator.is_port_available(12521));
        assert!(allocator.is_port_available(12530));
        assert!(!allocator.is_port_available(12531));
    }
}
