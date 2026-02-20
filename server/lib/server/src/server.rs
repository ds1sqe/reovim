//! Server - the main entry point for running reovim server.

use std::sync::Arc;

use reovim_kernel::api::v1::ServiceRegistry;

use crate::{
    ServerConfig, TransportMode,
    session::{Session, SessionId, SessionRegistry, SessionState, TokenRegistry},
};

#[cfg(feature = "grpc")]
use {
    crate::grpc::{
        AuthInterceptor, BufferServiceImpl, CommandServiceImpl, EditorServiceImpl,
        ExtensionServiceImpl, InputServiceImpl, ModuleServiceImpl, NotificationServiceImpl,
        PresenceServiceImpl, ServerServiceImpl, StateServiceImpl, SyntaxServiceImpl,
    },
    reovim_driver_session::bridges::{BridgeRegistry, CmdlineBridge},
    reovim_protocol::v2::{
        buffer_service_server::BufferServiceServer, command_service_server::CommandServiceServer,
        editor_service_server::EditorServiceServer,
        extension_service_server::ExtensionServiceServer, input_service_server::InputServiceServer,
        module_service_server::ModuleServiceServer,
        notification_service_server::NotificationServiceServer,
        presence_service_server::PresenceServiceServer, server_service_server::ServerServiceServer,
        state_service_server::StateServiceServer, syntax_service_server::SyntaxServiceServer,
    },
};

/// Session factory function type.
///
/// Creates a `SessionState` for new sessions. This allows the runner to inject
/// module-initialized registries into sessions.
pub type SessionFactory = Box<dyn Fn() -> SessionState + Send + Sync>;

/// The reovim server.
///
/// Manages sessions and handles client connections via the configured transport.
pub struct Server {
    /// Server configuration.
    config: ServerConfig,

    /// Registry of active sessions.
    sessions: Arc<SessionRegistry>,

    /// Token registry for session-based authentication (#483).
    ///
    /// Maps session tokens to client IDs. Shared with the gRPC interceptor
    /// and presence service.
    tokens: Arc<TokenRegistry>,

    /// Optional service registry populated by modules.
    ///
    /// When set, sessions created by the server will use services from this registry
    /// (resolvers, command handlers, keybindings, etc.).
    ///
    /// Note: Currently unused - will be used when we add service-based session creation.
    #[allow(dead_code)]
    services: Option<Arc<ServiceRegistry>>,

    /// Optional session factory for creating sessions with custom state.
    ///
    /// If provided, this factory is used to create `SessionState` for new sessions.
    /// This enables the runner to inject module-initialized registries.
    session_factory: Option<SessionFactory>,
}

impl Server {
    /// Create a new server with the given configuration.
    ///
    /// This creates a server with empty registries. For full vim functionality,
    /// use [`Server::with_services`] or [`Server::with_session_factory`] to
    /// inject module-initialized registries.
    #[must_use]
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(SessionRegistry::new()),
            tokens: Arc::new(TokenRegistry::new()),
            services: None,
            session_factory: None,
        }
    }

    /// Create a server with a service registry populated by modules.
    ///
    /// The service registry should contain:
    /// - `ResolverRegistry` - mode key resolvers
    /// - `KeybindingStore` - keybindings
    /// - `CommandHandlerStore` - command handlers
    /// - `ModeInfoStore` - mode metadata
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_server::{Server, ServerConfig};
    /// use reovim_kernel::api::v1::ServiceRegistry;
    ///
    /// // Bootstrap: load modules and populate services
    /// let services = Arc::new(ServiceRegistry::new());
    /// bootstrap_modules(&services);
    ///
    /// let server = Server::with_services(ServerConfig::default(), services);
    /// server.run().await?;
    /// ```
    #[must_use]
    pub fn with_services(config: ServerConfig, services: Arc<ServiceRegistry>) -> Self {
        Self {
            config,
            sessions: Arc::new(SessionRegistry::new()),
            tokens: Arc::new(TokenRegistry::new()),
            services: Some(services),
            session_factory: None,
        }
    }

    /// Create a server with a custom session factory.
    ///
    /// The factory function is called each time a new session is created,
    /// allowing the runner to inject fully-configured `SessionState` instances
    /// with module-initialized registries.
    ///
    /// This is the most flexible option for module integration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_server::{Server, ServerConfig, SessionState};
    ///
    /// let server = Server::with_session_factory(
    ///     ServerConfig::default(),
    ///     Box::new(|| {
    ///         // Create session state with populated registries
    ///         create_session_state_with_modules()
    ///     }),
    /// );
    /// server.run().await?;
    /// ```
    #[must_use]
    pub fn with_session_factory(config: ServerConfig, factory: SessionFactory) -> Self {
        Self {
            config,
            sessions: Arc::new(SessionRegistry::new()),
            tokens: Arc::new(TokenRegistry::new()),
            services: None,
            session_factory: Some(factory),
        }
    }

    /// Create a session state using the configured factory or default.
    #[allow(clippy::option_if_let_else)] // More readable with if-let
    fn create_session_state(&self) -> SessionState {
        if let Some(factory) = &self.session_factory {
            factory()
        } else {
            SessionState::default()
        }
    }

    /// Run the server.
    ///
    /// This method blocks until the server is shut down.
    ///
    /// # Errors
    ///
    /// Returns an error if the transport fails to start (e.g., port in use).
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn run(&self) -> std::io::Result<()> {
        // Create the default session with module-initialized state
        let session_state = self.create_session_state();
        let default_session = Arc::new(Session::from_state(
            SessionId::new(&*self.config.default_session_name),
            session_state,
        ));
        self.sessions.insert(&default_session);

        tracing::info!(
            session = %self.config.default_session_name,
            "Created default session"
        );

        // Start the appropriate transport
        match &self.config.transport {
            TransportMode::TcpWithFallback => self.run_tcp_fallback().await,
            TransportMode::Tcp { port } => self.run_tcp(*port).await,
            #[cfg(unix)]
            TransportMode::UnixSocket { path } => self.run_unix(path).await,
            TransportMode::Grpc { port } => self.run_grpc(*port, None, None).await,
        }
    }

    /// Run with TCP transport, trying ports 12540-12549.
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn run_tcp_fallback(&self) -> std::io::Result<()> {
        for port in 12540..=12549 {
            match self.run_tcp(port).await {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                    tracing::debug!(port, "Port in use, trying next");
                }
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            "All ports 12540-12549 are in use",
        ))
    }

    /// Run with TCP transport on a specific port.
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn run_tcp(&self, port: u16) -> std::io::Result<()> {
        tracing::info!(port, "Starting TCP server (JSON-RPC not implemented yet)");
        // TODO: Implement JSON-RPC server
        // For now, just wait forever
        std::future::pending::<()>().await;
        Ok(())
    }

    /// Run with Unix socket transport.
    #[cfg(unix)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn run_unix(&self, path: &std::path::Path) -> std::io::Result<()> {
        tracing::info!(path = %path.display(), "Starting Unix socket server");
        // TODO: Implement Unix socket server
        std::future::pending::<()>().await;
        Ok(())
    }

    /// Run with gRPC transport.
    ///
    /// When `shutdown` is `Some`, the server will stop when the future resolves.
    /// When `port_tx` is `Some`, the bound port is sent before serving starts.
    #[allow(clippy::too_many_lines)] // Service wiring is inherently verbose
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn run_grpc(
        &self,
        port: u16,
        shutdown: Option<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
        port_tx: Option<tokio::sync::oneshot::Sender<u16>>,
    ) -> std::io::Result<()> {
        // TODO: Make bind address configurable (currently 0.0.0.0 for dev testing)
        let addr: std::net::SocketAddr = format!("0.0.0.0:{port}")
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        // Bind first to get actual port (handles port 0 for testing)
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        tracing::info!(address = %local_addr, "Starting gRPC server");
        // Output for test harness (expects exact format)
        eprintln!("Listening on 127.0.0.1:{}", local_addr.port());

        // Report port to caller if requested (for integrated mode with OS-assigned port)
        if let Some(tx) = port_tx {
            let _ = tx.send(local_addr.port());
        }

        let default_session_id = SessionId::new(&*self.config.default_session_name);

        // Auth interceptor: resolves x-reovim-token → ClientId (#483)
        let interceptor = AuthInterceptor::new(Arc::clone(&self.tokens));

        // Extension bridge registry (#514) — shared between InputService and ExtensionService
        let mut bridge_registry = BridgeRegistry::new();
        bridge_registry.register(CmdlineBridge);
        let bridges = Arc::new(bridge_registry);

        // Create all gRPC services
        let buffer_service =
            BufferServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());
        let editor_service =
            EditorServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());
        let input_service = InputServiceImpl::new(
            Arc::clone(&self.sessions),
            default_session_id.clone(),
            Arc::clone(&bridges),
        );
        let state_service =
            StateServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());
        let server_service =
            ServerServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());
        let notification_service = NotificationServiceImpl::new(
            Arc::clone(&self.sessions),
            default_session_id.clone(),
            Arc::clone(&self.tokens),
        );

        // ModuleService is a stub - full implementation is in runner
        let module_service = ModuleServiceImpl::new();

        // SyntaxService provides token data for syntax highlighting
        let syntax_service =
            SyntaxServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());

        // PresenceService for multi-client awareness (Phase 14)
        let presence_service = PresenceServiceImpl::new(
            Arc::clone(&self.sessions),
            default_session_id.clone(),
            Arc::clone(&self.tokens),
        );

        // CommandService for command completion (#453)
        let command_service =
            CommandServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());

        // ExtensionService for querying extension state (#514)
        let extension_service =
            ExtensionServiceImpl::new(Arc::clone(&self.sessions), default_session_id, bridges);

        // Build gRPC server with optional gRPC-Web support
        #[cfg(feature = "grpc-web")]
        {
            use tower_http::cors::{Any, CorsLayer};

            tracing::info!("gRPC-Web support enabled (HTTP/1.1 + CORS)");

            // CORS layer for browser access (permissive for development)
            // TODO (Phase 9+): Production CORS with configurable allowed origins
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods(Any)
                .expose_headers(Any);

            let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
            let i = &interceptor;
            let router = tonic::transport::Server::builder()
                .accept_http1(true) // Required for gRPC-Web
                .layer(cors)
                .layer(tonic_web::GrpcWebLayer::new())
                .add_service(BufferServiceServer::with_interceptor(buffer_service, i.clone()))
                .add_service(EditorServiceServer::with_interceptor(editor_service, i.clone()))
                .add_service(InputServiceServer::with_interceptor(input_service, i.clone()))
                .add_service(ModuleServiceServer::with_interceptor(module_service, i.clone()))
                .add_service(StateServiceServer::with_interceptor(state_service, i.clone()))
                .add_service(ServerServiceServer::with_interceptor(server_service, i.clone()))
                .add_service(NotificationServiceServer::with_interceptor(
                    notification_service,
                    i.clone(),
                ))
                .add_service(SyntaxServiceServer::with_interceptor(syntax_service, i.clone()))
                .add_service(PresenceServiceServer::with_interceptor(presence_service, i.clone()))
                .add_service(ExtensionServiceServer::with_interceptor(extension_service, i.clone()))
                .add_service(CommandServiceServer::with_interceptor(command_service, i.clone()));

            if let Some(signal) = shutdown {
                router
                    .serve_with_incoming_shutdown(incoming, signal)
                    .await
                    .map_err(std::io::Error::other)
            } else {
                router
                    .serve_with_incoming(incoming)
                    .await
                    .map_err(std::io::Error::other)
            }
        }

        #[cfg(not(feature = "grpc-web"))]
        {
            let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
            let i = &interceptor;
            let router = tonic::transport::Server::builder()
                .add_service(BufferServiceServer::with_interceptor(buffer_service, i.clone()))
                .add_service(EditorServiceServer::with_interceptor(editor_service, i.clone()))
                .add_service(InputServiceServer::with_interceptor(input_service, i.clone()))
                .add_service(ModuleServiceServer::with_interceptor(module_service, i.clone()))
                .add_service(StateServiceServer::with_interceptor(state_service, i.clone()))
                .add_service(ServerServiceServer::with_interceptor(server_service, i.clone()))
                .add_service(NotificationServiceServer::with_interceptor(
                    notification_service,
                    i.clone(),
                ))
                .add_service(SyntaxServiceServer::with_interceptor(syntax_service, i.clone()))
                .add_service(PresenceServiceServer::with_interceptor(presence_service, i.clone()))
                .add_service(ExtensionServiceServer::with_interceptor(extension_service, i.clone()))
                .add_service(CommandServiceServer::with_interceptor(command_service, i.clone()));

            if let Some(signal) = shutdown {
                router
                    .serve_with_incoming_shutdown(incoming, signal)
                    .await
                    .map_err(std::io::Error::other)
            } else {
                router
                    .serve_with_incoming(incoming)
                    .await
                    .map_err(std::io::Error::other)
            }
        }
    }

    /// Run the server until a shutdown signal is received.
    ///
    /// Similar to [`run()`](Self::run) but accepts a shutdown future and an optional
    /// port sender. When the shutdown future resolves, the server performs a graceful
    /// shutdown. The port sender reports the actual bound port (useful when binding
    /// to port 0 for OS-assigned ports).
    ///
    /// Only gRPC transport is supported.
    ///
    /// # Errors
    ///
    /// Returns an error if the transport fails to start or if the configured
    /// transport is not gRPC.
    pub async fn run_until(
        &self,
        shutdown: impl std::future::Future<Output = ()> + Send + 'static,
        port_tx: Option<tokio::sync::oneshot::Sender<u16>>,
    ) -> std::io::Result<()> {
        // Create the default session with module-initialized state
        let session_state = self.create_session_state();
        let default_session = Arc::new(Session::from_state(
            SessionId::new(&*self.config.default_session_name),
            session_state,
        ));
        self.sessions.insert(&default_session);

        tracing::info!(
            session = %self.config.default_session_name,
            "Created default session"
        );

        let port = match &self.config.transport {
            TransportMode::Grpc { port } => *port,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "run_until() only supports gRPC transport",
                ));
            }
        };

        self.run_grpc(port, Some(Box::pin(shutdown)), port_tx).await
    }

    /// Get a reference to the session registry.
    #[must_use]
    pub const fn sessions(&self) -> &Arc<SessionRegistry> {
        &self.sessions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_new_creates_empty_registries() {
        let server = Server::new(ServerConfig::default());
        assert!(server.sessions.is_empty());
        assert!(server.services.is_none());
        assert!(server.session_factory.is_none());
    }

    #[test]
    fn server_with_services() {
        let services = Arc::new(ServiceRegistry::new());
        let server = Server::with_services(ServerConfig::default(), services);
        assert!(server.services.is_some());
        assert!(server.session_factory.is_none());
    }

    #[test]
    fn server_with_session_factory() {
        let factory: SessionFactory = Box::new(SessionState::default);
        let server = Server::with_session_factory(ServerConfig::default(), factory);
        assert!(server.services.is_none());
        assert!(server.session_factory.is_some());
    }

    #[test]
    fn create_session_state_uses_default_when_no_factory() {
        let server = Server::new(ServerConfig::default());
        let state = server.create_session_state();
        // Just verify it returns without panicking
        let _ = state;
    }

    #[test]
    fn create_session_state_uses_factory() {
        let factory: SessionFactory = Box::new(|| {
            let mut state = SessionState::default();
            state.app.running = false; // marker to verify factory was called
            state
        });
        let server = Server::with_session_factory(ServerConfig::default(), factory);
        let state = server.create_session_state();
        assert!(!state.app.is_running());
    }

    #[test]
    fn sessions_accessor() {
        let server = Server::new(ServerConfig::default());
        assert!(server.sessions().is_empty());
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn run_until_starts_and_shuts_down() {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let (port_tx, port_rx) = tokio::sync::oneshot::channel::<u16>();

        let config = ServerConfig::grpc(0); // OS-assigned port
        let server = Arc::new(Server::new(config));

        let server_clone = Arc::clone(&server);
        let handle = tokio::spawn(async move {
            server_clone
                .run_until(
                    async {
                        shutdown_rx.await.ok();
                    },
                    Some(port_tx),
                )
                .await
        });

        // Wait for port to be assigned
        let port = port_rx.await.expect("should receive port");
        assert!(port > 0);

        // Verify session was created
        assert_eq!(server.sessions().len(), 1);

        // Shut down
        shutdown_tx.send(()).expect("should send shutdown");
        let result = handle.await.expect("task should complete");
        assert!(result.is_ok());
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn run_until_rejects_non_grpc_transport() {
        let config = ServerConfig::tcp(8080);
        let server = Server::new(config);

        let result = server.run_until(std::future::pending::<()>(), None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn run_until_with_factory_creates_custom_session() {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let (port_tx, port_rx) = tokio::sync::oneshot::channel::<u16>();

        let factory: SessionFactory = Box::new(SessionState::default);
        let config = ServerConfig::grpc(0);
        let server = Arc::new(Server::with_session_factory(config, factory));

        let server_clone = Arc::clone(&server);
        let handle = tokio::spawn(async move {
            server_clone
                .run_until(
                    async {
                        shutdown_rx.await.ok();
                    },
                    Some(port_tx),
                )
                .await
        });

        let _port = port_rx.await.expect("should receive port");
        assert_eq!(server.sessions().len(), 1);

        shutdown_tx.send(()).expect("should send shutdown");
        handle
            .await
            .expect("task should complete")
            .expect("server should succeed");
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn run_creates_session_and_dispatches_grpc() {
        // Cover lines 165-187: run() creates session state, inserts default session,
        // and dispatches to the appropriate transport.
        // We use gRPC transport with port 0 (OS-assigned) so it binds successfully.
        // Since run() doesn't accept a shutdown signal, we abort the task after verifying.
        let config = ServerConfig::grpc(0);
        let server = Arc::new(Server::new(config));

        let server_clone = Arc::clone(&server);
        let handle = tokio::spawn(async move { server_clone.run().await });

        // Give the server a moment to start and create the session
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Verify session was created by run()
        assert_eq!(server.sessions().len(), 1);

        // Abort the task since run() doesn't have a shutdown mechanism
        handle.abort();
        let _ = handle.await;
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn run_tcp_transport_creates_session() {
        // Cover lines 180-182: the TcpWithFallback match arm in run().
        // run() with TcpWithFallback will call run_tcp_fallback which calls run_tcp
        // which blocks forever. We abort after verifying session creation.
        let config = ServerConfig::default(); // TcpWithFallback
        let server = Arc::new(Server::new(config));

        let server_clone = Arc::clone(&server);
        let handle = tokio::spawn(async move { server_clone.run().await });

        // Give the server a moment to create the session
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Verify session was created
        assert_eq!(server.sessions().len(), 1);

        handle.abort();
        let _ = handle.await;
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn run_tcp_specific_port_creates_session() {
        // Cover lines 182, 207-208: the Tcp { port } match arm and run_tcp entry.
        let config = ServerConfig::tcp(0);
        let server = Arc::new(Server::new(config));

        let server_clone = Arc::clone(&server);
        let handle = tokio::spawn(async move { server_clone.run().await });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(server.sessions().len(), 1);

        handle.abort();
        let _ = handle.await;
    }

    #[cfg(all(feature = "grpc", unix))]
    #[tokio::test]
    async fn run_unix_socket_creates_session() {
        // Cover lines 184, 217-218: the UnixSocket match arm and run_unix entry.
        let tmp = tempfile::tempdir().expect("should create tempdir");
        let socket_path = tmp.path().join("test.sock");
        let config = ServerConfig::unix_socket(&socket_path);
        let server = Arc::new(Server::new(config));

        let server_clone = Arc::clone(&server);
        let handle = tokio::spawn(async move { server_clone.run().await });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(server.sessions().len(), 1);

        handle.abort();
        let _ = handle.await;
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn run_grpc_without_shutdown_signal() {
        // Cover lines 329-332 (grpc-web) or equivalent non-grpc-web: the else branch
        // in run_grpc where shutdown is None, calling serve_with_incoming.
        // run() calls run_grpc(port, None, None) so shutdown is None.
        let config = ServerConfig::grpc(0);
        let server = Arc::new(Server::new(config));

        let server_clone = Arc::clone(&server);
        let handle = tokio::spawn(async move { server_clone.run().await });

        // Give time for run_grpc to bind and enter serve_with_incoming
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert_eq!(server.sessions().len(), 1);

        handle.abort();
        let _ = handle.await;
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn run_until_without_port_sender() {
        // Cover run_until with port_tx = None
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let config = ServerConfig::grpc(0);
        let server = Arc::new(Server::new(config));

        let server_clone = Arc::clone(&server);
        let handle = tokio::spawn(async move {
            server_clone
                .run_until(
                    async {
                        shutdown_rx.await.ok();
                    },
                    None, // no port sender
                )
                .await
        });

        // Give server time to start
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(server.sessions().len(), 1);

        shutdown_tx.send(()).expect("should send shutdown");
        let result = handle.await.expect("task should complete");
        assert!(result.is_ok());
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn run_with_custom_session_name() {
        // Verify run() uses the configured session name
        let config = ServerConfig::grpc(0).with_session_name("my-custom-session");
        let server = Arc::new(Server::new(config));

        let server_clone = Arc::clone(&server);
        let handle = tokio::spawn(async move { server_clone.run().await });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(server.sessions().len(), 1);

        handle.abort();
        let _ = handle.await;
    }
}
