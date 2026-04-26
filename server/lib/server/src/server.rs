//! Server - the main entry point for running reovim server.

use std::sync::Arc;

use {parking_lot::Mutex, reovim_kernel::api::v1::ServiceRegistry};

use crate::{
    ClientDebugRegistry, ServerConfig, TransportMode,
    session::{Session, SessionId, SessionRegistry, SessionState, TokenRegistry},
};

#[cfg(feature = "grpc")]
use {
    crate::grpc::{
        AuthInterceptor, BufferServiceImpl, ClientDebugServiceImpl, CommandServiceImpl,
        DebugServiceImpl, EditorServiceImpl, ExtensionServiceImpl, InputServiceImpl,
        ModuleServiceImpl, NotificationServiceImpl, PresenceServiceImpl, ServerServiceImpl,
        StateServiceImpl,
    },
    reovim_protocol::v3::{
        buffer_service_server::BufferServiceServer,
        client_debug_service_server::ClientDebugServiceServer,
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
    },
    reovim_subsys_session::bridges::BridgeRegistry,
};

#[cfg(feature = "grpc")]
type DefaultModuleService = ModuleServiceImpl;

#[cfg(not(feature = "grpc"))]
type DefaultModuleService = ();

/// Fully-instantiated gRPC service implementations ready to be
/// stacked into a tonic router. Produced by
/// [`Server::build_grpc_services`] and consumed by both the plain and
/// gRPC-Web router assemblies.
#[cfg(feature = "grpc")]
struct GrpcServicesBundle<M> {
    buffer: BufferServiceImpl,
    editor: EditorServiceImpl,
    input: InputServiceImpl,
    state: StateServiceImpl,
    server: ServerServiceImpl,
    notification: NotificationServiceImpl,
    module: M,
    presence: PresenceServiceImpl,
    command: CommandServiceImpl,
    extension: ExtensionServiceImpl,
    debug: DebugServiceImpl,
    client_debug: ClientDebugServiceImpl,
}

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

    /// Optional domain driver for domain-neutral dispatch (#753).
    ///
    /// When set, the default session is wired with this driver on startup.
    /// Dispatch routes through the domain driver when wired.
    domain_driver: Option<Arc<dyn reovim_subsys_session::DomainDriver>>,

    /// Extension bridge registry for gRPC notification emission (#468).
    ///
    /// Bridges are collected from `BridgeProvider` in bootstrap.
    /// Defaults to empty registry (no extension notifications).
    #[cfg(feature = "grpc")]
    bridge_registry: Arc<BridgeRegistry>,

    /// Concrete gRPC module service implementation.
    #[cfg(feature = "grpc")]
    module_service: M,

    /// Shutdown signal. Flipped `true` by [`Server::shutdown`]; serve
    /// loops subscribe via [`Server::subscribe_shutdown`] and thread
    /// the receiver into `serve_with_incoming_shutdown`, so draining
    /// in-flight RPCs is the tonic router's concern — not ours.
    shutdown_tx: tokio::sync::watch::Sender<bool>,

    /// Registry of client-debug drivers routed by `ClientDebugService`
    /// (#770). Populated by the composition root (empty by default).
    client_debug_registry: Arc<ClientDebugRegistry>,
}

impl Server<DefaultModuleService> {
    /// Create a new server with the given configuration.
    ///
    /// This creates a server with empty registries. For full vim functionality,
    /// use [`Server::with_services`] or [`Server::with_session_factory`] to
    /// inject module-initialized registries.
    #[must_use]
    pub fn new(config: ServerConfig) -> Self {
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);
        Self {
            config,
            sessions: Arc::new(SessionRegistry::new()),
            tokens: Arc::new(TokenRegistry::new()),
            services: None,
            session_factory: None,
            initial_session_state: None,
            domain_driver: None,
            #[cfg(feature = "grpc")]
            bridge_registry: Arc::new(BridgeRegistry::default()),
            #[cfg(feature = "grpc")]
            module_service: ModuleServiceImpl::new(),
            shutdown_tx,
            client_debug_registry: Arc::new(ClientDebugRegistry::new()),
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
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);
        Self {
            config,
            sessions: Arc::new(SessionRegistry::new()),
            tokens: Arc::new(TokenRegistry::new()),
            services: Some(services),
            session_factory: None,
            initial_session_state: None,
            domain_driver: None,
            #[cfg(feature = "grpc")]
            bridge_registry: Arc::new(BridgeRegistry::default()),
            #[cfg(feature = "grpc")]
            module_service: ModuleServiceImpl::new(),
            shutdown_tx,
            client_debug_registry: Arc::new(ClientDebugRegistry::new()),
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
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);
        Self {
            config,
            sessions: Arc::new(SessionRegistry::new()),
            tokens: Arc::new(TokenRegistry::new()),
            services: None,
            session_factory: Some(factory),
            initial_session_state: None,
            domain_driver: None,
            #[cfg(feature = "grpc")]
            bridge_registry: Arc::new(BridgeRegistry::default()),
            #[cfg(feature = "grpc")]
            module_service: ModuleServiceImpl::new(),
            shutdown_tx,
            client_debug_registry: Arc::new(ClientDebugRegistry::new()),
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
            domain_driver: self.domain_driver,
            bridge_registry: self.bridge_registry,
            module_service,
            shutdown_tx: self.shutdown_tx,
            client_debug_registry: self.client_debug_registry,
        }
    }

    /// Inject a populated client-debug registry (#770 Phase 1). The
    /// default is an empty registry; composition roots that host
    /// client-debug drivers call this before `run*()` to wire them up.
    #[must_use]
    pub fn with_client_debug_registry(mut self, registry: Arc<ClientDebugRegistry>) -> Self {
        self.client_debug_registry = registry;
        self
    }

    /// Inject a prebuilt default session state for one-time consumption.
    #[must_use]
    pub fn with_initial_session_state(mut self, session_state: SessionState) -> Self {
        self.initial_session_state = Some(Mutex::new(Some(session_state)));
        self
    }

    /// Inject a domain driver for domain-neutral dispatch (#753).
    ///
    /// The driver is wired into the default session at startup.
    /// Dispatch routes through the domain driver automatically.
    #[must_use]
    pub fn with_domain_driver(
        mut self,
        driver: Arc<dyn reovim_subsys_session::DomainDriver>,
    ) -> Self {
        self.domain_driver = Some(driver);
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
        // Start the appropriate transport.
        //
        // `run_grpc` expects the caller to have bootstrapped the default
        // session (it derives the session id locally). `run_unix` self-
        // bootstraps — mirrors `run_inproc`/`run_pipe` — so the launcher
        // can call it directly. The placeholder tcp paths do not consume
        // the default session today but still bootstrap here to match
        // the 0.9 behaviour.
        match &self.config.transport {
            TransportMode::TcpWithFallback => {
                let _ = self.bootstrap_default_session();
                self.run_tcp_fallback().await
            }
            TransportMode::Tcp { port } => {
                let _ = self.bootstrap_default_session();
                self.run_tcp(*port).await
            }
            #[cfg(unix)]
            TransportMode::UnixSocket { path } => self.run_unix(path).await,
            TransportMode::Grpc { port } => {
                let _ = self.bootstrap_default_session();
                self.run_grpc(*port, None, None).await
            }
            TransportMode::Inproc => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "TransportMode::Inproc requires Server::run_inproc(stream) — \
                 call it directly from the embedded launcher instead of run()",
            )),
            TransportMode::Pipe => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "TransportMode::Pipe requires Server::run_pipe(read, write) — \
                 call it directly from the subprocess pipe launcher instead of run()",
            )),
        }
    }

    /// Signal every subscribed serve loop to shut down gracefully.
    ///
    /// Flips the internal shutdown watch to `true`; any serve loop
    /// currently awaiting the signal (via `serve_with_incoming_shutdown`)
    /// drains its in-flight RPCs and returns `Ok(())`. Idempotent —
    /// calling `shutdown` after the serve loop has already exited is a
    /// no-op.
    ///
    /// # Errors
    ///
    /// Never fails today. The result type is retained so a future
    /// drain-await can propagate an error without breaking callers.
    #[allow(clippy::unused_async)] // Async today for symmetry with a future drain-await.
    pub async fn shutdown(&self) -> std::io::Result<()> {
        // `send` errors only when every receiver has been dropped,
        // which means the serve loop already exited — equivalent to a
        // successful shutdown from the caller's perspective.
        let _ = self.shutdown_tx.send(true);
        Ok(())
    }

    /// Subscribe to the shutdown signal. Serve loops await the
    /// returned receiver and exit when [`Server::shutdown`] fires.
    fn subscribe_shutdown(&self) -> tokio::sync::watch::Receiver<bool> {
        self.shutdown_tx.subscribe()
    }

    /// Run the server over the in-process `DuplexStream` handed in by
    /// an embedded launcher.
    ///
    /// Bypasses `Server::run`'s transport dispatch; call this method
    /// directly when `config.transport == TransportMode::Inproc`. The
    /// body does the same session-bootstrap as `Server::run` and then
    /// serves the full tonic gRPC stack over the caller's duplex. The
    /// loop returns when the caller fires [`Server::shutdown`] or the
    /// peer drops the duplex.
    ///
    /// # Errors
    ///
    /// Propagates any transport error from the inproc gRPC loop.
    #[cfg(feature = "grpc")]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn run_inproc(&self, stream: tokio::io::DuplexStream) -> std::io::Result<()>
    where
        M: ModuleService + Clone + Send + Sync + 'static,
    {
        let default_session_id = self.bootstrap_default_session();
        let router = self.assemble_grpc_router(&default_session_id);
        let mut rx = self.subscribe_shutdown();
        let shutdown_fut =
            async move { while !*rx.borrow_and_update() && rx.changed().await.is_ok() {} };
        crate::transport_inproc::run(stream, router, shutdown_fut).await
    }

    /// Run the server over an OS-pipe (`AsyncRead` + `AsyncWrite`) pair.
    ///
    /// Bypasses `Server::run`'s transport dispatch; call this method
    /// directly when `config.transport == TransportMode::Pipe`.
    ///
    /// # Errors
    ///
    /// Propagates any transport error from the pipe gRPC loop.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn run_pipe<R, W>(&self, read: R, write: W) -> std::io::Result<()>
    where
        M: ModuleService + Clone + Send + Sync + 'static,
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
        W: tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        crate::transport_pipe::run(read, write).await
    }

    /// Create and insert the default session.
    ///
    /// Shared between [`Server::run`] (before transport dispatch) and
    /// [`Server::run_inproc`] (which bypasses `run`). Consumes the
    /// one-shot `initial_session_state` if present.
    fn bootstrap_default_session(&self) -> SessionId {
        let session_state = self.create_session_state();
        let session_id = SessionId::new(&*self.config.default_session_name);
        let default_session = Arc::new(Session::from_state(session_id.clone(), session_state));

        if let Some(ref driver) = self.domain_driver {
            default_session.set_domain_driver(Arc::clone(driver));
        }

        self.sessions.insert(&default_session);

        tracing::info!(
            session = %self.config.default_session_name,
            "Created default session"
        );

        session_id
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

    /// Run the gRPC stack over a Unix-domain socket listener bound at
    /// `path`.
    ///
    /// Self-bootstraps the default session and serves tonic through
    /// [`Server::assemble_grpc_router`] until [`Server::shutdown`]
    /// fires on the shared watch. The caller (embedded launcher or the
    /// `run()` dispatcher) owns the shutdown trigger; this method does
    /// not install a ctrl-c handler of its own.
    ///
    /// # Errors
    ///
    /// Propagates any `std::io::Error` from binding the listener or
    /// from the tonic serve loop.
    #[cfg(all(unix, feature = "grpc"))]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn run_unix(&self, path: &std::path::Path) -> std::io::Result<()>
    where
        M: ModuleService + Clone + Send + Sync + 'static,
    {
        tracing::info!(path = %path.display(), "Starting Unix socket gRPC server");

        // Pre-remove a stale socket file so repeated launches don't
        // fail with EADDRINUSE. Ignore a NotFound result.
        if let Err(e) = tokio::fs::remove_file(path).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(e);
        }

        let listener = tokio::net::UnixListener::bind(path)?;

        let default_session_id = self.bootstrap_default_session();
        let router = self.assemble_grpc_router(&default_session_id);

        let mut rx = self.subscribe_shutdown();
        let shutdown_fut =
            async move { while !*rx.borrow_and_update() && rx.changed().await.is_ok() {} };

        let incoming = tokio_stream::wrappers::UnixListenerStream::new(listener);
        router
            .serve_with_incoming_shutdown(incoming, shutdown_fut)
            .await
            .map_err(std::io::Error::other)
    }

    /// Fallback stub for builds without the `grpc` feature or on
    /// non-unix platforms. Preserves the `run()` dispatch signature
    /// without dragging in the tonic router.
    #[cfg(all(unix, not(feature = "grpc")))]
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::unused_async)] // Async reserved for the grpc-feature body.
    async fn run_unix(&self, path: &std::path::Path) -> std::io::Result<()> {
        tracing::warn!(
            path = %path.display(),
            "run_unix called without the `grpc` feature; no service exposed"
        );
        std::future::pending::<()>().await;
        Ok(())
    }

    /// Run with gRPC transport.
    ///
    /// When `shutdown` is `Some`, the server will stop when the future resolves.
    /// When `port_tx` is `Some`, the bound port is sent before serving starts.
    /// Maximum gRPC message size (encoding and decoding) in bytes.
    ///
    /// Tonic defaults to 4 MB, which is too small for large buffer content
    /// (e.g., hex-encoded binary files). 64 MB accommodates codec output
    /// with room for normal large text files.
    const GRPC_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

    #[cfg_attr(coverage_nightly, coverage(off))]
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

        #[cfg(feature = "grpc-web")]
        {
            use tower_http::cors::{Any, CorsLayer};

            tracing::info!("gRPC-Web support enabled (HTTP/1.1 + CORS)");

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods(Any)
                .expose_headers(Any);

            let interceptor = AuthInterceptor::new(Arc::clone(&self.tokens));
            let bridges = Arc::clone(&self.bridge_registry);
            self.wire_tick_scheduler(&default_session_id, &bridges);
            let services = self.build_grpc_services(&default_session_id, &bridges);

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
                .accept_http1(true)
                .layer(cors)
                .layer(tonic_web::GrpcWebLayer::new())
                .add_service(svc!(BufferServiceServer, services.buffer, i.clone()))
                .add_service(svc!(EditorServiceServer, services.editor, i.clone()))
                .add_service(svc!(InputServiceServer, services.input, i.clone()))
                .add_service(svc!(ModuleServiceServer, services.module, i.clone()))
                .add_service(svc!(StateServiceServer, services.state, i.clone()))
                .add_service(svc!(ServerServiceServer, services.server, i.clone()))
                .add_service(svc!(NotificationServiceServer, services.notification, i.clone()))
                .add_service(svc!(PresenceServiceServer, services.presence, i.clone()))
                .add_service(svc!(ExtensionServiceServer, services.extension, i.clone()))
                .add_service(svc!(CommandServiceServer, services.command, i.clone()))
                .add_service(svc!(DebugServiceServer, services.debug, i.clone()))
                .add_service(svc!(ClientDebugServiceServer, services.client_debug, i.clone()));

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
            let router = self.assemble_grpc_router(&default_session_id);

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

    /// Build the plain (no-layer) tonic router with every reovim gRPC
    /// service wired to the shared auth interceptor.
    ///
    /// Shared between [`Server::run_grpc`] (non-gRPC-Web path) and
    /// [`Server::run_inproc`]. Session creation must have already run;
    /// call [`Server::bootstrap_default_session`] first. The gRPC-Web
    /// branch of `run_grpc` builds its own layered router inline
    /// because the layer-stack changes `Router<L>`'s generic
    /// parameter.
    #[cfg(feature = "grpc")]
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::too_many_lines)] // Service wiring is inherently verbose
    fn assemble_grpc_router(
        &self,
        default_session_id: &SessionId,
    ) -> tonic::transport::server::Router
    where
        M: ModuleService + Clone + Send + Sync + 'static,
    {
        let interceptor = AuthInterceptor::new(Arc::clone(&self.tokens));
        let bridges = Arc::clone(&self.bridge_registry);
        self.wire_tick_scheduler(default_session_id, &bridges);
        let services = self.build_grpc_services(default_session_id, &bridges);

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

        let i = &interceptor;
        tonic::transport::Server::builder()
            .add_service(svc!(BufferServiceServer, services.buffer, i.clone()))
            .add_service(svc!(EditorServiceServer, services.editor, i.clone()))
            .add_service(svc!(InputServiceServer, services.input, i.clone()))
            .add_service(svc!(ModuleServiceServer, services.module, i.clone()))
            .add_service(svc!(StateServiceServer, services.state, i.clone()))
            .add_service(svc!(ServerServiceServer, services.server, i.clone()))
            .add_service(svc!(NotificationServiceServer, services.notification, i.clone()))
            .add_service(svc!(PresenceServiceServer, services.presence, i.clone()))
            .add_service(svc!(ExtensionServiceServer, services.extension, i.clone()))
            .add_service(svc!(CommandServiceServer, services.command, i.clone()))
            .add_service(svc!(DebugServiceServer, services.debug, i.clone()))
            .add_service(svc!(ClientDebugServiceServer, services.client_debug, i.clone()))
    }

    /// Install the server-driven tick scheduler into the default
    /// session (#546). Idempotent: no-op if the session has already
    /// picked up a scheduler.
    #[cfg(feature = "grpc")]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn wire_tick_scheduler(&self, default_session_id: &SessionId, bridges: &Arc<BridgeRegistry>) {
        use reovim_subsys_session::{TickSchedulerHandle, tick::TickScheduler};

        let tick_scheduler = Arc::new(crate::tick::TokioTickScheduler::new(
            Arc::clone(&self.sessions),
            default_session_id.clone(),
            Arc::clone(bridges),
        ));

        if let Some(session) = self.sessions.get(default_session_id) {
            session.with_state_mut_sync(|state| {
                let handle = state.app.services.get_or_create::<TickSchedulerHandle>();
                handle.set(tick_scheduler as Arc<dyn TickScheduler>);
            });
        }
    }

    /// Instantiate the 11 gRPC service implementations bound to the
    /// server's shared state. Shared between the plain and gRPC-Web
    /// router assemblies.
    #[cfg(feature = "grpc")]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn build_grpc_services(
        &self,
        default_session_id: &SessionId,
        bridges: &Arc<BridgeRegistry>,
    ) -> GrpcServicesBundle<M>
    where
        M: Clone,
    {
        GrpcServicesBundle {
            buffer: BufferServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone()),
            editor: EditorServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone()),
            input: InputServiceImpl::new(
                Arc::clone(&self.sessions),
                default_session_id.clone(),
                Arc::clone(bridges),
            ),
            state: StateServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone()),
            server: ServerServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone()),
            notification: NotificationServiceImpl::new(
                Arc::clone(&self.sessions),
                default_session_id.clone(),
                Arc::clone(&self.tokens),
            ),
            module: self.module_service.clone(),
            presence: PresenceServiceImpl::new(
                Arc::clone(&self.sessions),
                default_session_id.clone(),
                Arc::clone(&self.tokens),
            ),
            command: CommandServiceImpl::new(
                Arc::clone(&self.sessions),
                default_session_id.clone(),
            ),
            extension: ExtensionServiceImpl::new(
                Arc::clone(&self.sessions),
                default_session_id.clone(),
                Arc::clone(bridges),
            ),
            debug: DebugServiceImpl::with_sessions(
                Arc::clone(&self.sessions),
                default_session_id.clone(),
                Arc::clone(&self.bridge_registry),
            ),
            client_debug: ClientDebugServiceImpl::new(Arc::clone(&self.client_debug_registry)),
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
        let _default_session_id = self.bootstrap_default_session();

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

    /// Run the server through a `GrpcServerDriver` abstraction.
    ///
    /// This is the driver-routed path introduced in Plan 15 Phase N.
    /// The composition root (apps/bin) provides a concrete driver
    /// (e.g. `reovim-driver-net-grpc::GrpcServerDriverImpl`) that
    /// owns the TCP bind + tonic serve lifecycle. Server code here
    /// touches only the `reovim-subsys-net` contract; the driver
    /// implementation is not a production dep of this crate.
    ///
    /// Currently supports base gRPC only. gRPC-Web layer stacking
    /// changes the concrete `tonic::transport::server::Router<L>`
    /// generic parameter, which the `GrpcServerDriver::serve` trait
    /// method cannot accept through its object-safe signature. gRPC-
    /// Web traffic continues to flow through [`run_grpc`][Self::run_grpc];
    /// migrating it to the driver abstraction is tracked as a
    /// follow-on (Plan 15 Phase V or later).
    ///
    /// # Errors
    /// Propagates any [`reovim_subsys_net::NetError`] from the driver,
    /// plus a synthetic `NetError::Io` when the configured transport
    /// is not gRPC.
    #[cfg(feature = "grpc")]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn serve_with_driver(
        &self,
        driver: Box<dyn reovim_subsys_net::GrpcServerDriver>,
        shutdown_fd: reovim_subsys_net::abi::ShutdownFd,
        bind_ready_fd: reovim_subsys_net::abi::ShutdownFd,
        port_writeback: &'static std::sync::atomic::AtomicU16,
    ) -> Result<(), reovim_subsys_net::NetError>
    where
        M: ModuleService + Clone + Send + Sync + 'static,
    {
        use reovim_subsys_net::{NetError, TransportConfig};

        let default_session_id = self.bootstrap_default_session();

        let port = match &self.config.transport {
            TransportMode::Grpc { port } => *port,
            _ => {
                return Err(NetError::Io(
                    "serve_with_driver() only supports gRPC transport".into(),
                ));
            }
        };

        let descriptors = self.build_service_descriptors(&default_session_id);
        let config = TransportConfig::tcp("0.0.0.0", port);
        driver
            .serve(config, descriptors, shutdown_fd, bind_ready_fd, port_writeback)
            .await
    }

    /// Build the typed service-descriptor list handed to a
    /// [`reovim_subsys_net::GrpcServerDriver`] implementation.
    ///
    /// Each descriptor pairs the canonical proto service name with a
    /// type-erased `tower::util::BoxCloneService` produced by
    /// `tonic::service::interceptor::InterceptedService::new(<X>ServiceServer::new(...), interceptor)`.
    /// SP02 Phase 6 redesign — replaces the prior `assemble_grpc_router`
    /// path for the driver-routed flow. `assemble_grpc_router` stays
    /// for the in-process `run_grpc` / `run_inproc` paths which never
    /// go through a driver.
    #[cfg(feature = "grpc")]
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::too_many_lines)]
    fn build_service_descriptors(
        &self,
        default_session_id: &SessionId,
    ) -> Vec<reovim_subsys_net::ServiceDescriptor>
    where
        M: ModuleService + Clone + Send + Sync + 'static,
    {
        use {reovim_subsys_net::ServiceDescriptor, tower::util::BoxCloneService};

        let interceptor = AuthInterceptor::new(Arc::clone(&self.tokens));
        let bridges = Arc::clone(&self.bridge_registry);
        self.wire_tick_scheduler(default_session_id, &bridges);
        let services = self.build_grpc_services(default_session_id, &bridges);

        macro_rules! desc {
            ($server:ident, $impl:expr, $name:literal) => {{
                let svc = tonic::service::interceptor::InterceptedService::new(
                    $server::new($impl)
                        .max_decoding_message_size(Self::GRPC_MAX_MESSAGE_SIZE)
                        .max_encoding_message_size(Self::GRPC_MAX_MESSAGE_SIZE),
                    interceptor.clone(),
                );
                ServiceDescriptor::new($name, BoxCloneService::new(svc))
            }};
        }

        vec![
            desc!(BufferServiceServer, services.buffer, "reovim.v3.BufferService"),
            desc!(EditorServiceServer, services.editor, "reovim.v3.EditorService"),
            desc!(InputServiceServer, services.input, "reovim.v3.InputService"),
            desc!(ModuleServiceServer, services.module, "reovim.v3.ModuleService"),
            desc!(StateServiceServer, services.state, "reovim.v3.StateService"),
            desc!(ServerServiceServer, services.server, "reovim.v3.ServerService"),
            desc!(
                NotificationServiceServer,
                services.notification,
                "reovim.v3.NotificationService"
            ),
            desc!(PresenceServiceServer, services.presence, "reovim.v3.PresenceService"),
            desc!(ExtensionServiceServer, services.extension, "reovim.v3.ExtensionService"),
            desc!(CommandServiceServer, services.command, "reovim.v3.CommandService"),
            desc!(DebugServiceServer, services.debug, "reovim.v3.DebugService"),
            desc!(ClientDebugServiceServer, services.client_debug, "reovim.v3.ClientDebugService"),
        ]
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
