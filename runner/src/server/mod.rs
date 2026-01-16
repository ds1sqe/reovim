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

use clap::Args;

/// Server mode CLI arguments.
///
/// These arguments configure how the server listens for connections.
#[derive(Args, Debug, Clone)]
pub struct SrvArgs {
    /// Start server on specific TCP port.
    #[arg(short, long, value_name = "PORT")]
    pub tcp: Option<u16>,

    /// Start server on Unix socket.
    #[cfg(unix)]
    #[arg(short, long, value_name = "PATH")]
    pub socket: Option<std::path::PathBuf>,

    /// Start server in stdio mode (single client, for embedding).
    #[arg(long)]
    pub stdio: bool,
}

impl SrvArgs {
    /// Convert arguments to `ServerConfig`.
    #[must_use]
    pub fn into_config(self) -> ServerConfig {
        #[cfg(unix)]
        if let Some(path) = self.socket {
            return ServerConfig::unix_socket(path);
        }

        if self.stdio {
            ServerConfig::stdio()
        } else if let Some(port) = self.tcp {
            ServerConfig::tcp(port)
        } else {
            ServerConfig::tcp_with_fallback()
        }
    }
}

// Submodules - all server-specific code lives here
mod app;
pub mod client;
pub mod debug;
mod event_loop;
pub mod module;
pub mod notification;
pub mod registry;
pub mod rpc;
pub mod session;
pub mod transport;

// Re-exports for public API
pub use {
    app::AppState,
    event_loop::{EventLoop, EventLoopError},
    notification::NotificationBroadcaster,
    // Fallback types from driver (breaking the circular dependency)
    reovim_driver_input::{
        BeepFallback, FallbackContext, FallbackResult, InputFallbackHandler, NoOpFallback,
    },
};

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use {
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::{
        EventBus, KernelContext, MarkBank, ModeId, ModuleId, MotionEngine, OptionRegistry,
        RegisterBank, TextObjectEngine,
    },
    reovim_protocol::v1::{RpcError, RpcRequest, RpcResponse},
};

use crate::buffer_manager::SimpleBufferManager;

use {
    client::Client,
    module::ModuleConfig,
    rpc::{RpcContext, RpcDispatcher, create_default_dispatcher},
    session::{Session, SessionId, SessionRegistry},
    transport::{TransportListener, TransportReader, TransportWriter},
};

/// Transport configuration for the server.
///
/// Determines how the server accepts client connections.
#[derive(Debug, Clone, Default)]
pub enum TransportMode {
    /// TCP with automatic port fallback (12521-12530).
    ///
    /// Allows multiple reovim servers to run concurrently.
    #[default]
    TcpWithFallback,

    /// TCP on a specific port.
    Tcp {
        /// Port to bind to.
        port: u16,
    },

    /// Unix socket at a specific path.
    ///
    /// Efficient for local IPC, commonly used for editor embedding.
    #[cfg(unix)]
    UnixSocket {
        /// Path to the socket file.
        path: PathBuf,
    },

    /// Stdio transport (stdin/stdout).
    ///
    /// For process embedding - the parent process communicates
    /// directly via stdin/stdout. Single client only.
    Stdio,
}

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Transport mode (TCP, Unix socket, or Stdio).
    pub transport: TransportMode,

    /// Name of the default session to create on startup.
    pub default_session_name: String,

    /// Module configuration (search paths, auto-load).
    pub modules: ModuleConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            transport: TransportMode::TcpWithFallback,
            default_session_name: String::from("default"),
            modules: ModuleConfig::default(),
        }
    }
}

impl ServerConfig {
    /// Create a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a config for TCP with automatic port fallback.
    #[must_use]
    pub fn tcp_with_fallback() -> Self {
        Self {
            transport: TransportMode::TcpWithFallback,
            ..Self::default()
        }
    }

    /// Create a config for TCP on a specific port.
    #[must_use]
    pub fn tcp(port: u16) -> Self {
        Self {
            transport: TransportMode::Tcp { port },
            ..Self::default()
        }
    }

    /// Create a config for Unix socket.
    #[cfg(unix)]
    #[must_use]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self {
            transport: TransportMode::UnixSocket { path: path.into() },
            ..Self::default()
        }
    }

    /// Create a config for Stdio transport.
    #[must_use]
    pub fn stdio() -> Self {
        Self {
            transport: TransportMode::Stdio,
            ..Self::default()
        }
    }

    /// Set the default session name.
    #[must_use]
    pub fn session_name(mut self, name: impl Into<String>) -> Self {
        self.default_session_name = name.into();
        self
    }

    /// Set the module configuration.
    #[must_use]
    pub fn with_modules(mut self, modules: ModuleConfig) -> Self {
        self.modules = modules;
        self
    }

    /// Load module configuration from the config file.
    ///
    /// Loads `[modules]` section from `~/.config/reovim/config.toml`.
    /// Falls back to defaults if the file doesn't exist.
    ///
    /// # Panics
    ///
    /// Logs a warning and uses defaults if the config file exists
    /// but cannot be parsed.
    #[must_use]
    pub fn with_modules_from_config(mut self) -> Self {
        match ModuleConfig::load() {
            Ok(config) => self.modules = config,
            Err(e) => {
                tracing::warn!("Failed to load module config: {e}");
            }
        }
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
    /// Dispatches to the appropriate transport handler based on configuration:
    /// - `TcpWithFallback`: TCP with automatic port fallback
    /// - `Tcp`: TCP on specific port
    /// - `UnixSocket`: Unix domain socket
    /// - `Stdio`: Standard input/output
    ///
    /// # Errors
    ///
    /// Returns an error if binding fails.
    pub async fn run(&self) -> std::io::Result<()> {
        // Initialize debug infrastructure (uptime tracking, etc.)
        debug::init();

        // Create default session
        let default_session_id = SessionId::new(self.config.default_session_name.as_str());
        self.ensure_default_session(&default_session_id);

        match &self.config.transport {
            TransportMode::TcpWithFallback => {
                let listener = TransportListener::bind_tcp_with_fallback().await?;
                self.run_listener(listener, &default_session_id).await
            }
            TransportMode::Tcp { port } => {
                let listener = TransportListener::bind_tcp(*port).await?;
                self.run_listener(listener, &default_session_id).await
            }
            #[cfg(unix)]
            TransportMode::UnixSocket { path } => {
                let listener = TransportListener::bind_unix(path)?;
                self.run_listener(listener, &default_session_id).await
            }
            TransportMode::Stdio => self.run_stdio(&default_session_id).await,
        }
    }

    /// Run the server with a listener transport (TCP or Unix socket).
    ///
    /// Accepts connections in a loop and spawns a task for each client.
    async fn run_listener(
        &self,
        listener: TransportListener,
        default_session_id: &SessionId,
    ) -> std::io::Result<()> {
        // Print listening address to stderr (like tmux)
        eprintln!("Listening on {}", listener.local_addr_string());

        // Accept loop
        while !self.shutdown.load(Ordering::Relaxed) {
            match listener.accept().await {
                Ok((reader, writer)) => {
                    tracing::info!("New connection");

                    // Generate unique client ID
                    let client_id = self.sessions.next_client_id();

                    // Clone Arcs for the spawned task
                    let sessions = Arc::clone(&self.sessions);
                    let dispatcher = Arc::clone(&self.dispatcher);
                    let session_id = default_session_id.clone();

                    // Spawn client handler task
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(
                            reader, writer, client_id, session_id, sessions, dispatcher,
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

    /// Run the server with Stdio transport.
    ///
    /// Handles a single client via stdin/stdout. Returns when stdin closes (EOF).
    async fn run_stdio(&self, default_session_id: &SessionId) -> std::io::Result<()> {
        tracing::info!("Running in stdio mode");

        // Create stdio reader/writer
        let reader = TransportReader::from_stdio();
        let writer = TransportWriter::from_stdio();

        // Generate client ID
        let client_id = self.sessions.next_client_id();

        // Handle the single client directly (no spawn)
        handle_client(
            reader,
            writer,
            client_id,
            default_session_id.clone(),
            Arc::clone(&self.sessions),
            Arc::clone(&self.dispatcher),
        )
        .await?;

        tracing::info!("Stdio client disconnected");
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
            Session::new(id.clone(), real_kernel_context(), default_mode_id())
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

/// Create a real `KernelContext` with working buffer management.
///
/// Unlike `KernelContext::default()` which uses stubs, this creates
/// a fully functional kernel context suitable for actual editing.
fn real_kernel_context() -> KernelContext {
    KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(SimpleBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(RwLock::new(RegisterBank::new())),
        Arc::new(RwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
    )
}

/// Handle a single client connection.
///
/// Reads JSON-RPC requests line by line, dispatches them, and sends responses.
/// Runs until the client disconnects or an error occurs.
///
/// # Arguments
///
/// * `reader` - Transport reader for receiving requests
/// * `writer` - Transport writer for sending responses
/// * `client_id` - Unique identifier for this client
/// * `session_id` - Session to attach the client to
/// * `sessions` - Session registry for looking up sessions
/// * `dispatcher` - RPC dispatcher for handling requests
async fn handle_client(
    mut reader: TransportReader,
    writer: TransportWriter,
    client_id: crate::session::ClientId,
    session_id: SessionId,
    sessions: Arc<SessionRegistry>,
    dispatcher: Arc<RpcDispatcher>,
) -> std::io::Result<()> {
    // Get or create the session
    let session = sessions.get_or_create(&session_id, || {
        Session::new(session_id.clone(), real_kernel_context(), default_mode_id())
    });

    // Create the client (owns the writer)
    let client = Client::new(client_id, session_id, writer);

    // Register client with session for notifications
    session.clients().insert(&client);

    // Create RPC context
    let ctx = RpcContext {
        session: Arc::clone(&session),
        client_id,
        client: Arc::clone(&client),
    };

    // Read loop
    loop {
        let Some(line) = reader.read_line().await? else {
            // EOF - client disconnected
            break;
        };

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

    // Cleanup: remove client from session
    session.clients().remove(&client_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert!(matches!(config.transport, TransportMode::TcpWithFallback));
        assert_eq!(config.default_session_name, "default");
    }

    #[test]
    fn test_server_config_tcp_with_fallback() {
        let config = ServerConfig::tcp_with_fallback();
        assert!(matches!(config.transport, TransportMode::TcpWithFallback));
    }

    #[test]
    fn test_server_config_tcp() {
        let config = ServerConfig::tcp(9000);
        match config.transport {
            TransportMode::Tcp { port } => assert_eq!(port, 9000),
            _ => panic!("Expected TransportMode::Tcp"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_server_config_unix_socket() {
        let config = ServerConfig::unix_socket("/tmp/test.sock");
        match &config.transport {
            TransportMode::UnixSocket { path } => {
                assert_eq!(path.to_str().unwrap(), "/tmp/test.sock");
            }
            _ => panic!("Expected TransportMode::UnixSocket"),
        }
    }

    #[test]
    fn test_server_config_stdio() {
        let config = ServerConfig::stdio();
        assert!(matches!(config.transport, TransportMode::Stdio));
    }

    #[test]
    fn test_server_config_session_name() {
        let config = ServerConfig::tcp(9000).session_name("my-session");
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

    #[test]
    fn test_server_config_with_modules() {
        let modules = ModuleConfig::new()
            .with_search_path("/custom/modules")
            .with_autoload("my-module");

        let config = ServerConfig::tcp(9000).with_modules(modules);

        assert_eq!(config.modules.search_paths, vec!["/custom/modules"]);
        assert_eq!(config.modules.autoload, vec!["my-module"]);
    }

    #[test]
    fn test_server_config_with_modules_from_config() {
        // This should not panic even if no config file exists
        let config = ServerConfig::tcp(9000).with_modules_from_config();

        // Should have default or loaded config
        assert!(config.modules.search_paths.is_empty() || !config.modules.search_paths.is_empty());
    }

    #[test]
    fn test_server_config_default_has_module_config() {
        let config = ServerConfig::default();

        // Default config should have empty module config
        assert!(config.modules.autoload.is_empty());
    }
}
