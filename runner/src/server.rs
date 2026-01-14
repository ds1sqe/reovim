//! Server struct and main run loop.
//!
//! The server accepts TCP connections, manages sessions, and dispatches
//! RPC requests to handlers. Following the Linux-inspired architecture,
//! the server is a "mechanism" that provides session management - the
//! actual editing policy comes from modules.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │                       Server                                │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │ SessionRegistry                                      │   │
//! │  │ └── Session "default"                               │   │
//! │  │     └── SessionState (AppState + Registries)        │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! │                          │                                  │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │ TcpTransport                                         │   │
//! │  │ └── accept() → spawn client task                    │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! │                          │                                  │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │ RpcDispatcher                                        │   │
//! │  │ └── dispatch(request) → handler → response          │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! └────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use runner::server::{Server, ServerConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ServerConfig::default();
//!     let server = Server::new(config);
//!     server.run().await?;
//!     Ok(())
//! }
//! ```

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpStream,
};

use {
    reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId},
    reovim_protocol::v1::{RpcError, RpcRequest, RpcResponse},
};

use crate::{
    client::Client,
    rpc::{RpcContext, RpcDispatcher, create_default_dispatcher},
    session::{Session, SessionId, SessionRegistry},
    transport::TcpTransport,
};

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// TCP port to listen on. If `None`, use default with fallback.
    pub port: Option<u16>,

    /// Host to bind to. Defaults to "127.0.0.1" (localhost only).
    pub host: String,

    /// Name of the default session to create on startup.
    pub default_session_name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: None,
            host: String::from("127.0.0.1"),
            default_session_name: String::from("default"),
        }
    }
}

impl ServerConfig {
    /// Create a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the TCP port to listen on.
    #[must_use]
    pub const fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Set the host to bind to.
    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the default session name.
    #[must_use]
    pub fn session_name(mut self, name: impl Into<String>) -> Self {
        self.default_session_name = name.into();
        self
    }
}

/// The reovim headless server.
///
/// Manages sessions and handles client connections over TCP.
/// Uses tokio for async I/O with task-per-client architecture.
///
/// # Concurrency Model
///
/// Following `docs/reference/concurrency.md`:
/// - **Level 0**: Lock-free session lookup (`ArcSwap`)
/// - **Level 1**: Per-session `RwLock` for state
/// - **Level 2**: Per-client `Mutex` for response writer
pub struct Server {
    /// Server configuration.
    config: ServerConfig,

    /// Session registry (shared across all client tasks).
    ///
    /// Wrapped in Arc to allow sharing across spawned tasks.
    sessions: Arc<SessionRegistry>,

    /// RPC dispatcher (shared across all client tasks).
    dispatcher: Arc<RpcDispatcher>,

    /// Shutdown flag for graceful termination.
    shutdown: AtomicBool,
}

impl Server {
    /// Create a new server with the given configuration.
    #[must_use]
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(SessionRegistry::new()),
            dispatcher: Arc::new(create_default_dispatcher()),
            shutdown: AtomicBool::new(false),
        }
    }

    /// Create a new server with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(ServerConfig::default())
    }

    /// Run the server.
    ///
    /// This method:
    /// 1. Binds the TCP transport
    /// 2. Creates the default session
    /// 3. Accepts connections in a loop
    /// 4. Spawns a task for each client
    ///
    /// # Errors
    ///
    /// Returns an error if binding fails.
    pub async fn run(&self) -> std::io::Result<()> {
        // Bind TCP transport
        let transport = if let Some(port) = self.config.port {
            TcpTransport::bind_port(port).await?
        } else {
            TcpTransport::bind_with_fallback().await?
        };

        // Print listening address to stderr (like tmux)
        eprintln!("Listening on {}", transport.local_addr());

        // Create default session
        let default_session_id = SessionId::new(self.config.default_session_name.as_str());
        self.ensure_default_session(&default_session_id);

        // Accept loop
        while !self.shutdown.load(Ordering::Relaxed) {
            match transport.accept().await {
                Ok((stream, addr)) => {
                    tracing::info!("New connection from {addr}");

                    // Generate unique client ID
                    let client_id = self.sessions.next_client_id();

                    // Clone Arcs for the spawned task
                    let sessions = Arc::clone(&self.sessions);
                    let dispatcher = Arc::clone(&self.dispatcher);
                    let default_session_id = default_session_id.clone();

                    // Spawn client handler task
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(
                            stream,
                            client_id,
                            default_session_id,
                            sessions,
                            dispatcher,
                        )
                        .await
                        {
                            tracing::error!("Client {client_id:?} error: {e}");
                        }
                        tracing::info!("Client {client_id:?} disconnected");
                    });
                }
                Err(e) => {
                    tracing::error!("Accept error: {e}");
                }
            }
        }

        tracing::info!("Server shutting down");
        Ok(())
    }

    /// Request server shutdown.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Check if shutdown has been requested.
    #[must_use]
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    /// Get a reference to the session registry.
    #[must_use]
    pub fn sessions(&self) -> &SessionRegistry {
        &self.sessions
    }

    /// Ensure the default session exists.
    fn ensure_default_session(&self, id: &SessionId) {
        self.sessions.get_or_create(id, || {
            Session::new(id.clone(), KernelContext::default(), default_mode_id())
        });
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Default mode ID for new sessions.
///
/// Uses "editor:normal" as the initial mode.
const fn default_mode_id() -> ModeId {
    ModeId::new(ModuleId::new("editor"), "normal")
}

/// Handle a single client connection.
///
/// Reads JSON-RPC requests line by line, dispatches them, and sends responses.
/// Runs until the client disconnects or an error occurs.
async fn handle_client(
    stream: TcpStream,
    client_id: crate::session::ClientId,
    session_id: SessionId,
    sessions: Arc<SessionRegistry>,
    dispatcher: Arc<RpcDispatcher>,
) -> std::io::Result<()> {
    // Split the stream for concurrent read/write
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Get or create the session
    let session = sessions.get_or_create(&session_id, || {
        Session::new(session_id.clone(), KernelContext::default(), default_mode_id())
    });

    // Create the client (owns the writer)
    let client = Client::new(client_id, session_id, writer);

    // Create RPC context
    let ctx = RpcContext {
        session: Arc::clone(&session),
    };

    // Read loop
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            // EOF - client disconnected
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse JSON-RPC request
        let request: RpcRequest = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(e) => {
                // Send parse error response
                let error_response = RpcResponse::error(0, RpcError::parse_error());
                let response_json = serde_json::to_string(&error_response)
                    .expect("RpcResponse serialization should never fail");
                client.send_line(&response_json).await?;
                tracing::warn!("Parse error from client {client_id:?}: {e}");
                continue;
            }
        };

        tracing::debug!("Request from {client_id:?}: {} (id={:?})", request.method, request.id);

        // Dispatch to handler
        if let Some(response) = dispatcher.dispatch(request, &ctx).await {
            let response_json = serde_json::to_string(&response)
                .expect("RpcResponse serialization should never fail");
            client.send_line(&response_json).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert!(config.port.is_none());
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.default_session_name, "default");
    }

    #[test]
    fn test_server_config_builder() {
        let config = ServerConfig::new()
            .port(9000)
            .host("0.0.0.0")
            .session_name("my-session");

        assert_eq!(config.port, Some(9000));
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.default_session_name, "my-session");
    }

    #[test]
    fn test_server_new() {
        let server = Server::new(ServerConfig::default());
        assert!(!server.is_shutdown());
        assert!(server.sessions().is_empty());
    }

    #[test]
    fn test_server_shutdown_flag() {
        let server = Server::new(ServerConfig::default());
        assert!(!server.is_shutdown());

        server.shutdown();
        assert!(server.is_shutdown());
    }

    #[tokio::test]
    async fn test_server_creates_default_session() {
        let server = Server::new(ServerConfig::default());
        let session_id = SessionId::new("default");

        server.ensure_default_session(&session_id);

        assert!(server.sessions().contains(&session_id));
        assert_eq!(server.sessions().len(), 1);
    }

    #[test]
    fn test_default_mode_id() {
        let mode_id = default_mode_id();
        assert_eq!(mode_id.module().as_str(), "editor");
        assert_eq!(mode_id.name(), "normal");
    }
}
