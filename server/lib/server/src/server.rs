//! Server - the main entry point for running reovim server.

use std::sync::Arc;

use {parking_lot::Mutex, reovim_kernel::api::v1::ServiceRegistry};

use crate::{
    ServerConfig, TransportMode,
    session::{Session, SessionId, SessionRegistry, SessionState, TokenRegistry},
};

#[cfg(feature = "grpc")]
use {
    crate::grpc::{
        AuthInterceptor, BufferServiceImpl, CommandServiceImpl, DebugServiceImpl,
        EditorServiceImpl, ExtensionServiceImpl, InputServiceImpl, ModuleServiceImpl,
        NotificationServiceImpl, PresenceServiceImpl, ServerServiceImpl, StateServiceImpl,
        SyntaxServiceImpl,
    },
    reovim_driver_session::bridges::BridgeRegistry,
    reovim_protocol::v2::{
        buffer_service_server::BufferServiceServer,
        command_service_server::CommandServiceServer,
        debug_service_server::DebugServiceServer,
        editor_service_server::EditorServiceServer,
        extension_service_server::ExtensionServiceServer,
        input_service_server::InputServiceServer,
        module_service_server::{ModuleService, ModuleServiceServer},
        notification_service_server::NotificationServiceServer,
        presence_service_server::PresenceServiceServer,
        server_service_server::ServerServiceServer,
        state_service_server::StateServiceServer,
        syntax_service_server::SyntaxServiceServer,
    },
};

#[cfg(feature = "grpc")]
type DefaultModuleService = ModuleServiceImpl;

#[cfg(not(feature = "grpc"))]
type DefaultModuleService = ();

/// Session factory function type.
///
/// Creates a `SessionState` for new sessions. This allows the runner to inject
/// module-initialized registries into sessions.
pub type SessionFactory = Box<dyn Fn() -> SessionState + Send + Sync>;

/// The reovim server.
///
/// Manages sessions and handles client connections via the configured transport.
pub struct Server<M = DefaultModuleService> {
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

    /// Optional one-shot initial session state for the default server session.
    ///
    /// This is separate from `session_factory`: the runner can hand the server
    /// an already-bootstrapped default session without weakening the reusable
    /// `SessionFactory` contract for future session creation paths.
    initial_session_state: Option<Mutex<Option<SessionState>>>,

    /// Extension bridge registry for gRPC notification emission (#468).
    ///
    /// Bridges are collected from `BridgeProvider` in bootstrap.
    /// Defaults to empty registry (no extension notifications).
    #[cfg(feature = "grpc")]
    bridge_registry: Arc<BridgeRegistry>,

    /// Concrete gRPC module service implementation.
    #[cfg(feature = "grpc")]
    module_service: M,
}

impl Server<DefaultModuleService> {
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
            initial_session_state: None,
            #[cfg(feature = "grpc")]
            bridge_registry: Arc::new(BridgeRegistry::default()),
            #[cfg(feature = "grpc")]
            module_service: ModuleServiceImpl::new(),
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
            initial_session_state: None,
            #[cfg(feature = "grpc")]
            bridge_registry: Arc::new(BridgeRegistry::default()),
            #[cfg(feature = "grpc")]
            module_service: ModuleServiceImpl::new(),
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
            initial_session_state: None,
            #[cfg(feature = "grpc")]
            bridge_registry: Arc::new(BridgeRegistry::default()),
            #[cfg(feature = "grpc")]
            module_service: ModuleServiceImpl::new(),
        }
    }
}

impl<M: Send + Sync> Server<M> {
    /// Set the extension bridge registry (#468).
    ///
    /// Bridges are collected from `BridgeProvider` in bootstrap.
    /// Must be called before `run()`.
    #[cfg(feature = "grpc")]
    #[must_use]
    pub fn with_bridges(mut self, registry: BridgeRegistry) -> Self {
        self.bridge_registry = Arc::new(registry);
        self
    }

    /// Replace the default stub module service with a concrete runner-owned implementation.
    #[cfg(feature = "grpc")]
    #[must_use]
    pub fn with_module_service<M2>(self, module_service: M2) -> Server<M2>
    where
        M2: ModuleService + Clone + Send + Sync + 'static,
    {
        Server {
            config: self.config,
            sessions: self.sessions,
            tokens: self.tokens,
            services: self.services,
            session_factory: self.session_factory,
            initial_session_state: self.initial_session_state,
            bridge_registry: self.bridge_registry,
            module_service,
        }
    }

    /// Inject a prebuilt default session state for one-time consumption.
    #[must_use]
    pub fn with_initial_session_state(mut self, session_state: SessionState) -> Self {
        self.initial_session_state = Some(Mutex::new(Some(session_state)));
        self
    }

    /// Create a session state using the configured factory or default.
    #[allow(clippy::option_if_let_else)] // More readable with if-let
    fn create_session_state(&self) -> SessionState {
        if let Some(state) = &self.initial_session_state
            && let Some(session_state) = state.lock().take()
        {
            return session_state;
        }

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
    pub async fn run(&self) -> std::io::Result<()>
    where
        M: ModuleService + Clone + Send + Sync + 'static,
    {
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
    /// Maximum gRPC message size (encoding and decoding) in bytes.
    ///
    /// Tonic defaults to 4 MB, which is too small for large buffer content
    /// (e.g., hex-encoded binary files). 64 MB accommodates codec output
    /// with room for normal large text files.
    const GRPC_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

    #[allow(clippy::too_many_lines)]
    async fn run_grpc(
        &self,
        port: u16,
        shutdown: Option<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
        port_tx: Option<tokio::sync::oneshot::Sender<u16>>,
    ) -> std::io::Result<()>
    where
        M: ModuleService + Clone + Send + Sync + 'static,
    {
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

        // Extension bridge registry (#514/#468) — shared between InputService and ExtensionService.
        // Bridges are now collected from BridgeProvider by bootstrap, not hardcoded here.
        let bridges = Arc::clone(&self.bridge_registry);

        // Tick scheduler for server-driven state advancement (#546).
        // Modules call TickSchedulerHandle.start() to begin periodic ticking.
        {
            use reovim_driver_session::TickSchedulerHandle;

            let tick_scheduler = Arc::new(crate::tick::TokioTickScheduler::new(
                Arc::clone(&self.sessions),
                default_session_id.clone(),
                Arc::clone(&bridges),
            ));

            if let Some(session) = self.sessions.get(&default_session_id) {
                session.with_state_mut_sync(|state| {
                    let handle = state.app.services.get_or_create::<TickSchedulerHandle>();
                    handle
                        .set(tick_scheduler as Arc<dyn reovim_driver_session::tick::TickScheduler>);
                });
            }
        }

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

        let module_service = self.module_service.clone();

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
        let extension_service = ExtensionServiceImpl::new(
            Arc::clone(&self.sessions),
            default_session_id.clone(),
            bridges,
        );

        // DebugService for CLI client-targeting operations (#468)
        let debug_service = DebugServiceImpl::with_sessions(
            Arc::clone(&self.sessions),
            default_session_id,
            Arc::clone(&self.bridge_registry),
        );

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

            // Helper: configure message size limits on a service server, then wrap with interceptor.
            macro_rules! svc {
                ($server:ident, $impl:expr, $i:expr) => {
                    tonic::service::interceptor::InterceptedService::new(
                        $server::new($impl)
                            .max_decoding_message_size(Self::GRPC_MAX_MESSAGE_SIZE)
                            .max_encoding_message_size(Self::GRPC_MAX_MESSAGE_SIZE),
                        $i,
                    )
                };
            }

            let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
            let i = &interceptor;
            let router = tonic::transport::Server::builder()
                .accept_http1(true) // Required for gRPC-Web
                .layer(cors)
                .layer(tonic_web::GrpcWebLayer::new())
                .add_service(svc!(BufferServiceServer, buffer_service, i.clone()))
                .add_service(svc!(EditorServiceServer, editor_service, i.clone()))
                .add_service(svc!(InputServiceServer, input_service, i.clone()))
                .add_service(svc!(ModuleServiceServer, module_service, i.clone()))
                .add_service(svc!(StateServiceServer, state_service, i.clone()))
                .add_service(svc!(ServerServiceServer, server_service, i.clone()))
                .add_service(svc!(NotificationServiceServer, notification_service, i.clone()))
                .add_service(svc!(SyntaxServiceServer, syntax_service, i.clone()))
                .add_service(svc!(PresenceServiceServer, presence_service, i.clone()))
                .add_service(svc!(ExtensionServiceServer, extension_service, i.clone()))
                .add_service(svc!(CommandServiceServer, command_service, i.clone()))
                .add_service(svc!(DebugServiceServer, debug_service, i.clone()));

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
                .add_service(svc!(BufferServiceServer, buffer_service, i.clone()))
                .add_service(svc!(EditorServiceServer, editor_service, i.clone()))
                .add_service(svc!(InputServiceServer, input_service, i.clone()))
                .add_service(svc!(ModuleServiceServer, module_service, i.clone()))
                .add_service(svc!(StateServiceServer, state_service, i.clone()))
                .add_service(svc!(ServerServiceServer, server_service, i.clone()))
                .add_service(svc!(NotificationServiceServer, notification_service, i.clone()))
                .add_service(svc!(SyntaxServiceServer, syntax_service, i.clone()))
                .add_service(svc!(PresenceServiceServer, presence_service, i.clone()))
                .add_service(svc!(ExtensionServiceServer, extension_service, i.clone()))
                .add_service(svc!(CommandServiceServer, command_service, i.clone()))
                .add_service(svc!(DebugServiceServer, debug_service, i.clone()));
            // svc! macro defined in grpc-web block above

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
    ) -> std::io::Result<()>
    where
        M: ModuleService + Clone + Send + Sync + 'static,
    {
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
#[path = "server_tests.rs"]
mod tests;
