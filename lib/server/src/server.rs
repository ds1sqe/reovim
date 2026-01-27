//! Server - the main entry point for running reovim server.

use std::sync::Arc;

use crate::{
    ServerConfig, TransportMode,
    session::{Session, SessionId, SessionRegistry},
};

/// The reovim server.
///
/// Manages sessions and handles client connections via the configured transport.
pub struct Server {
    /// Server configuration.
    config: ServerConfig,

    /// Registry of active sessions.
    sessions: Arc<SessionRegistry>,
}

impl Server {
    /// Create a new server with the given configuration.
    #[must_use]
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(SessionRegistry::new()),
        }
    }

    /// Run the server.
    ///
    /// This method blocks until the server is shut down.
    ///
    /// # Errors
    ///
    /// Returns an error if the transport fails to start (e.g., port in use).
    pub async fn run(&self) -> std::io::Result<()> {
        // Create the default session
        let default_session =
            Arc::new(Session::new(SessionId::new(&*self.config.default_session_name)));
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
            #[cfg(feature = "grpc")]
            TransportMode::Grpc { port } => self.run_grpc(*port).await,
        }
    }

    /// Run with TCP transport, trying ports 12540-12549.
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
    async fn run_tcp(&self, port: u16) -> std::io::Result<()> {
        tracing::info!(port, "Starting TCP server (JSON-RPC not implemented yet)");
        // TODO: Implement JSON-RPC server
        // For now, just wait forever
        std::future::pending::<()>().await;
        Ok(())
    }

    /// Run with Unix socket transport.
    #[cfg(unix)]
    async fn run_unix(&self, path: &std::path::Path) -> std::io::Result<()> {
        tracing::info!(path = %path.display(), "Starting Unix socket server");
        // TODO: Implement Unix socket server
        std::future::pending::<()>().await;
        Ok(())
    }

    /// Run with gRPC transport.
    #[cfg(feature = "grpc")]
    async fn run_grpc(&self, port: u16) -> std::io::Result<()> {
        use {
            crate::grpc::{
                BufferServiceImpl, InputServiceImpl, NotificationServiceImpl, ServerServiceImpl,
                StateServiceImpl,
            },
            reovim_protocol::v2::{
                buffer_service_server::BufferServiceServer,
                input_service_server::InputServiceServer,
                notification_service_server::NotificationServiceServer,
                server_service_server::ServerServiceServer,
                state_service_server::StateServiceServer,
            },
        };

        let addr: std::net::SocketAddr = format!("127.0.0.1:{port}")
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        tracing::info!(address = %addr, "Starting gRPC server");

        let default_session_id = SessionId::new(&*self.config.default_session_name);

        // Create all gRPC services
        let buffer_service =
            BufferServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());
        let input_service =
            InputServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());
        let state_service =
            StateServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());
        let server_service =
            ServerServiceImpl::new(Arc::clone(&self.sessions), default_session_id.clone());
        let notification_service =
            NotificationServiceImpl::new(Arc::clone(&self.sessions), default_session_id);

        tonic::transport::Server::builder()
            .add_service(BufferServiceServer::new(buffer_service))
            .add_service(InputServiceServer::new(input_service))
            .add_service(StateServiceServer::new(state_service))
            .add_service(ServerServiceServer::new(server_service))
            .add_service(NotificationServiceServer::new(notification_service))
            .serve(addr)
            .await
            .map_err(std::io::Error::other)
    }

    /// Get a reference to the session registry.
    #[must_use]
    pub const fn sessions(&self) -> &Arc<SessionRegistry> {
        &self.sessions
    }
}
