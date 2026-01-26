//! Server struct and main run loop.
//!
//! Contains the main `Server` type that manages sessions and handles
//! client connections.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::watch;

use crate::server::{
    bootstrap, debug, handler, instance,
    module::{ModuleConfig, ModuleManager},
    rpc::{RpcDispatcher, create_default_dispatcher},
    server_config::ServerConfig,
    session::{SessionId, SessionRegistry},
    transport::{TransportListener, TransportReader, TransportWriter},
};

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

    /// Module registry (holds loaded modules).
    module_registry: Arc<ModuleManager>,

    /// Shutdown flag for graceful termination.
    shutdown: AtomicBool,

    /// Shutdown signal sender for waking the accept loop from RPC handlers.
    ///
    /// This channel bridges RPC handlers (like `server/kill`) to the accept loop.
    /// When `true` is sent, the accept loop exits gracefully.
    /// Using `watch` channel because it preserves state even if sent while no one is waiting.
    shutdown_tx: watch::Sender<bool>,

    /// Shutdown signal receiver (cloned for each client handler).
    shutdown_rx: watch::Receiver<bool>,
}

impl Server {
    /// Create a new server with the given configuration.
    #[must_use]
    pub fn new(config: ServerConfig) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            config,
            sessions: Arc::new(SessionRegistry::new()),
            dispatcher: Arc::new(create_default_dispatcher()),
            module_registry: Arc::new(ModuleManager::new()),
            shutdown: AtomicBool::new(false),
            shutdown_tx,
            shutdown_rx,
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
        use crate::server::server_config::TransportMode;

        // Initialize debug infrastructure (uptime tracking, etc.)
        // Quiet mode when ready_signal is set (no stderr output)
        debug::init(self.config.ready_signal);

        // Log version info at startup (#419)
        let git_hash = option_env!("REOVIM_GIT_HASH").unwrap_or("unknown");
        let git_commit = option_env!("REOVIM_GIT_COMMIT").unwrap_or("unknown");
        let git_dirty = option_env!("REOVIM_GIT_DIRTY") == Some("true");
        let build_timestamp = option_env!("REOVIM_BUILD_TIMESTAMP").unwrap_or("unknown");
        let rust_version = option_env!("REOVIM_RUST_VERSION").unwrap_or("unknown");
        let target = option_env!("REOVIM_TARGET").unwrap_or("unknown");

        tracing::info!(
            version = env!("CARGO_PKG_VERSION"),
            git_hash = git_hash,
            git_commit = git_commit,
            git_dirty = git_dirty,
            build_timestamp = build_timestamp,
            rust = rust_version,
            target = target,
            "reovim server starting"
        );

        // Load modules from configuration
        self.load_modules();

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
    #[allow(clippy::too_many_lines)]
    async fn run_listener(
        &self,
        listener: TransportListener,
        default_session_id: &SessionId,
    ) -> std::io::Result<()> {
        // Print ready signal or listening message
        if self.config.ready_signal {
            // Ready signal for process coordination (stdout)
            // Format: READY <ip>:<port>\n
            use std::io::Write;
            let addr = listener.local_addr();
            println!("READY {addr}");
            std::io::stdout().flush().ok();
        } else {
            // Normal mode: print to stderr (like tmux)
            eprintln!("Listening on {}", listener.local_addr_string());
        }

        // Build instance info for registration
        let transport_info = Self::transport_info_from_listener(&listener);
        let instance_info = instance::InstanceInfo::new(
            self.config.instance_name.clone(),
            std::process::id(),
            transport_info,
        );

        // Register with manager if available (auto-starts if needed)
        let manager_registered = self.register_with_manager(&instance_info).await;

        // Also register in file-based registry as fallback
        let registry = instance::InstanceRegistry::new();
        if let Err(e) = registry.register(&instance_info) {
            tracing::warn!(
                instance = %self.config.instance_name,
                error = %e,
                "Failed to register instance in file registry"
            );
        } else {
            tracing::debug!(
                instance = %self.config.instance_name,
                "Registered instance in file registry"
            );
        }

        if manager_registered {
            tracing::info!(
                instance = %self.config.instance_name,
                transport = %instance_info.transport,
                "Registered instance with manager"
            );
        } else {
            tracing::info!(
                instance = %self.config.instance_name,
                transport = %instance_info.transport,
                "Registered instance (file registry only, manager unavailable)"
            );
        }

        // Accept loop with shutdown signal
        let mut shutdown_rx = self.shutdown_rx.clone();
        loop {
            tokio::select! {
                // Check for shutdown signal from RPC handlers (e.g., server/kill)
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Shutdown signal received");
                        break;
                    }
                }
                // Check manual shutdown flag
                () = async {}, if self.shutdown.load(Ordering::Relaxed) => {
                    tracing::info!("Shutdown flag set");
                    break;
                }
                // Accept new connections
                result = listener.accept() => {
                    match result {
                        Ok((reader, writer)) => {
                            tracing::info!("New connection");

                            // Generate unique client ID
                            let client_id = self.sessions.next_client_id();

                            // Clone Arcs for the spawned task
                            let sessions = Arc::clone(&self.sessions);
                            let dispatcher = Arc::clone(&self.dispatcher);
                            let session_id = default_session_id.clone();
                            let default_mode = self.config.effective_default_mode();
                            let shutdown_tx = self.shutdown_tx.clone();

                            // Spawn client handler task
                            tokio::spawn(async move {
                                if let Err(e) = handler::handle_client(
                                    reader,
                                    writer,
                                    client_id,
                                    session_id,
                                    sessions,
                                    dispatcher,
                                    default_mode,
                                    shutdown_tx,
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
            }
        }

        // Unregister from manager
        self.unregister_from_manager().await;

        // Unregister from file registry
        if let Err(e) = registry.unregister(&self.config.instance_name) {
            tracing::warn!(
                instance = %self.config.instance_name,
                error = %e,
                "Failed to unregister instance from file registry"
            );
        } else {
            tracing::debug!(
                instance = %self.config.instance_name,
                "Unregistered instance from file registry"
            );
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
        let default_mode = self.config.effective_default_mode();
        handler::handle_client(
            reader,
            writer,
            client_id,
            default_session_id.clone(),
            Arc::clone(&self.sessions),
            Arc::clone(&self.dispatcher),
            default_mode,
            self.shutdown_tx.clone(),
        )
        .await?;

        tracing::info!("Stdio client disconnected");
        Ok(())
    }

    /// Create transport info from a listener.
    ///
    /// Extracts the address information from the listener to create
    /// a `TransportInfo` for registry registration.
    fn transport_info_from_listener(listener: &TransportListener) -> instance::TransportInfo {
        match listener {
            TransportListener::Tcp { local_addr, .. } => {
                instance::TransportInfo::tcp(local_addr.ip().to_string(), local_addr.port())
            }
            #[cfg(unix)]
            TransportListener::Unix { path, .. } => {
                instance::TransportInfo::local(path.display().to_string())
            }
        }
    }

    /// Register this server instance with the manager daemon.
    ///
    /// Auto-starts the manager if not running. Returns true if registration
    /// succeeded, false if manager is unavailable.
    async fn register_with_manager(&self, info: &instance::InstanceInfo) -> bool {
        use crate::manager::{ManagerClient, ensure_manager_running};

        // Try to ensure manager is running (auto-starts if needed)
        if let Err(e) = ensure_manager_running().await {
            tracing::debug!(error = %e, "Manager not available, using file registry only");
            return false;
        }

        // Connect and register
        match ManagerClient::connect().await {
            Ok(mut client) => match client.register(info.clone()).await {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to register with manager");
                    false
                }
            },
            Err(e) => {
                tracing::debug!(error = %e, "Failed to connect to manager");
                false
            }
        }
    }

    /// Unregister this server instance from the manager daemon.
    ///
    /// Called on shutdown. Silently ignores failures.
    async fn unregister_from_manager(&self) {
        use crate::manager::{ManagerClient, is_manager_alive};

        // Only try if manager is running
        if !is_manager_alive().await {
            return;
        }

        if let Ok(mut client) = ManagerClient::connect().await {
            let _ = client.unregister(&self.config.instance_name).await;
        }
    }

    /// Request server shutdown.
    ///
    /// Sets the shutdown flag AND sends via watch channel.
    /// Both mechanisms are used for reliability:
    /// - Flag: Checked on next loop iteration
    /// - Channel: Wakes up if waiting on `accept()`
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = self.shutdown_tx.send(true);
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

    /// Get a reference to the module registry.
    #[must_use]
    pub fn module_registry(&self) -> &ModuleManager {
        &self.module_registry
    }

    /// Load modules from configuration.
    ///
    /// This method:
    /// 1. Loads config file (respecting `REOVIM_CONFIG_DIR`)
    /// 2. Merges file config with CLI args (CLI takes precedence)
    /// 3. Calculates effective module list (defaults + extra - skip)
    /// 4. Loads each module, logging warnings for failures
    ///
    /// Called automatically by `run()` before creating sessions.
    ///
    /// # Loading Precedence
    ///
    /// 1. CLI `--load` flags (highest priority)
    /// 2. CLI `--no-defaults` flag
    /// 3. Config file `[modules].autoload` (overrides defaults)
    /// 4. Config file `[modules].extra` (adds to defaults)
    /// 5. Config file `[modules].skip` (removes from defaults)
    /// 6. `DEFAULT_MODULES` constant (lowest priority)
    fn load_modules(&self) {
        // Load config file with environment override
        let file_config = match ModuleConfig::load_with_env() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to load module config: {e}");
                ModuleConfig::default()
            }
        };

        // Merge: CLI args override file config
        let merged = self.merge_module_config(&file_config);

        // Get effective list
        let modules_to_load = if merged.should_skip_defaults() {
            // Only CLI-specified modules when --no-defaults is set
            self.config.modules.autoload.clone()
        } else {
            merged.effective_modules()
        };

        if modules_to_load.is_empty() {
            tracing::info!("No modules to load");
            return;
        }

        tracing::info!(
            count = modules_to_load.len(),
            modules = ?modules_to_load,
            "Loading modules"
        );

        // Note: Actual module loading now happens in `build_default_registries()`
        // via `load_from_config()`, which tries dynamic loading first and falls
        // back to static modules. This function just logs the config-based intent.
        let search_paths = merged.all_search_paths_with_env();
        tracing::debug!(paths = ?search_paths, "Module search paths");
    }

    /// Merge file config with CLI arguments.
    ///
    /// CLI arguments take precedence over file config:
    /// - CLI search paths are appended
    /// - CLI autoload modules are appended
    /// - CLI `--no-defaults` overrides file config
    fn merge_module_config(&self, file_config: &ModuleConfig) -> ModuleConfig {
        let mut merged = file_config.clone();

        // CLI search paths are appended
        for path in &self.config.modules.search_paths {
            merged.search_paths.push(path.clone());
        }

        // CLI autoload modules are appended
        for module in &self.config.modules.autoload {
            if !merged.autoload.contains(module) {
                merged.autoload.push(module.clone());
            }
        }

        // CLI --no-defaults overrides
        if self.config.modules.no_defaults {
            merged.no_defaults = true;
        }

        merged
    }

    /// Ensure the default session exists.
    ///
    /// Creates the session with pre-wired keybindings from default modules.
    /// This is where module loading "meets" session creation (#265).
    fn ensure_default_session(&self, id: &SessionId) {
        self.sessions
            .get_or_create(id, || bootstrap::create_session_with_defaults(id.clone()));
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
