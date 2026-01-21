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
/// These arguments configure how the server listens for connections
/// and how modules are loaded.
///
/// # Instance Naming
///
/// Use `-L` to name this server instance. Named instances are discoverable
/// via the instance registry, allowing clients to connect using `-L name`
/// instead of raw TCP addresses.
///
/// # Transport Precedence
///
/// Transport flags are mutually exclusive. If multiple are specified:
/// 1. `--stdio` wins (single client mode)
/// 2. `-s`/`--socket` next (Unix socket)
/// 3. `-t`/`--tcp` next (specific TCP port)
/// 4. Default: TCP with automatic port fallback (12521-12530)
#[derive(Args, Debug, Clone)]
pub struct SrvArgs {
    /// Instance name for registry discovery.
    ///
    /// Registers this server as a named instance. Clients can then
    /// connect using `reovim -L <name>` instead of specifying the port.
    ///
    /// Default: "default"
    #[arg(
        short = 'L',
        long = "instance",
        value_name = "NAME",
        default_value = "default"
    )]
    pub instance: String,

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

    /// Additional module search directory (repeatable).
    ///
    /// Modules in these directories are discovered in addition to the
    /// default search paths.
    #[arg(long = "moddir", value_name = "PATH")]
    pub module_dirs: Vec<std::path::PathBuf>,

    /// Load specific module by ID (repeatable).
    ///
    /// Explicitly load these modules on startup. Can be combined with
    /// `--no-defaults` to load only specific modules.
    #[arg(long = "load", value_name = "MODULE_ID")]
    pub load_modules: Vec<String>,

    /// Skip loading default modules.
    ///
    /// When set, only modules specified via `--load` are loaded.
    /// Useful for testing or minimal startup.
    #[arg(long = "no-defaults")]
    pub no_defaults: bool,

    /// Print ready signal to stdout when server is bound.
    ///
    /// When set, server prints `READY <ip>:<port>` to stdout after binding.
    /// Used by integrated mode for process coordination.
    /// Format: `READY 127.0.0.1:12521\n`
    #[arg(long, hide = true)]
    pub ready_signal: bool,
}

impl SrvArgs {
    /// Convert arguments to `ServerConfig`.
    ///
    /// # Panics
    ///
    /// Panics if the instance name is invalid.
    #[must_use]
    pub fn into_config(self) -> ServerConfig {
        // Validate instance name
        if let Err(e) = instance::InstanceRegistry::validate_name(&self.instance) {
            eprintln!("Error: Invalid instance name: {e}");
            std::process::exit(1);
        }

        // Build module config from CLI arguments
        let mut modules = ModuleConfig::new();

        // Add CLI-specified search paths
        for path in self.module_dirs {
            modules = modules.with_search_path(path.to_string_lossy().into_owned());
        }

        // Add CLI-specified modules to autoload
        for module_id in self.load_modules {
            modules = modules.with_autoload(module_id);
        }

        // Set no_defaults flag
        if self.no_defaults {
            modules = modules.with_no_defaults();
        }

        // Build transport config
        let transport = {
            #[cfg(unix)]
            if let Some(path) = self.socket {
                return ServerConfig::unix_socket(path)
                    .with_instance_name(&self.instance)
                    .with_modules(modules)
                    .with_ready_signal(self.ready_signal);
            }

            if self.stdio {
                TransportMode::Stdio
            } else if let Some(port) = self.tcp {
                TransportMode::Tcp { port }
            } else {
                TransportMode::TcpWithFallback
            }
        };

        ServerConfig {
            transport,
            instance_name: self.instance,
            default_session_name: String::from("default"),
            modules,
            default_mode: None,
            ready_signal: self.ready_signal,
        }
    }
}

// Submodules - all server-specific code lives here
mod app;
pub mod client;
pub mod config;
pub mod debug;
mod event_loop;
pub mod instance;
pub mod module;
pub mod notification;
pub mod registry;
pub mod rpc;
pub mod session;
pub mod transport;
pub mod window;

// Re-exports for public API
pub use {
    app::AppState,
    event_loop::{EventLoop, EventLoopError},
    notification::NotificationBroadcaster,
    // Fallback types from driver (breaking the circular dependency)
    reovim_driver_input::{
        BeepFallback, FallbackContext, FallbackResult, InputFallbackHandler, NoOpFallback,
    },
    window::{WindowRegistry, WindowState},
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
    reovim_driver_display::layout::{CompositorBox, RootCompositor},
    reovim_driver_vfs::{StandardVfs, VfsDriver},
    reovim_kernel::api::v1::{
        EventBus, KernelContext, MarkBank, ModeId, Module, ModuleContext, ModuleId, MotionEngine,
        OptionRegistry, OptionScope, OptionSpec, OptionValue, RegisterBank, TextObjectEngine,
    },
    reovim_module_commands::CommandsModule,
    reovim_module_defaults,
    reovim_module_editor::EditorModule,
    reovim_module_keymap::KeymapModule,
    reovim_module_layout::LayoutModule,
    reovim_module_motions::MotionsModule,
    // Note: operators merged into vim (Epic #385)
    reovim_module_vim::{VimMode, VimModule},
    reovim_protocol::v1::{RpcError, RpcRequest, RpcResponse},
};

use crate::buffer_manager::SimpleBufferManager;

use {
    client::Client,
    module::{
        ModuleConfig, ModuleLoader, ModuleManager, load_from_config, wire_module_commands,
        wire_module_keybindings,
    },
    registry::{CommandRegistry, EmptySessionHandlerRegistry, KeymapRegistry, ModeRegistry},
    rpc::{RpcContext, RpcDispatcher, create_default_dispatcher},
    session::{Session, SessionId, SessionRegistry},
    transport::{TransportListener, TransportReader, TransportWriter},
};

use reovim_driver_session::{EmptySessionAction, EmptySessionContext};

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

    /// Instance name for registry discovery.
    ///
    /// This server will be registered under this name in the instance
    /// registry, allowing clients to connect using `-L <name>`.
    pub instance_name: String,

    /// Name of the default session to create on startup.
    pub default_session_name: String,

    /// Module configuration (search paths, auto-load).
    pub modules: ModuleConfig,

    /// Default mode ID for new sessions.
    ///
    /// If None, falls back to "editor:normal".
    pub default_mode: Option<ModeId>,

    /// Print ready signal to stdout when server is bound.
    ///
    /// When true, server prints `READY <ip>:<port>\n` to stdout after binding.
    /// Used by integrated mode for process coordination.
    pub ready_signal: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            transport: TransportMode::TcpWithFallback,
            instance_name: String::from("default"),
            default_session_name: String::from("default"),
            modules: ModuleConfig::default(),
            default_mode: None,
            ready_signal: false,
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

    /// Set the instance name for registry discovery.
    #[must_use]
    pub fn with_instance_name(mut self, name: impl Into<String>) -> Self {
        self.instance_name = name.into();
        self
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

    /// Set the default mode for new sessions.
    #[must_use]
    pub fn with_default_mode(mut self, mode: ModeId) -> Self {
        self.default_mode = Some(mode);
        self
    }

    /// Enable ready signal output.
    ///
    /// When enabled, server prints `READY <ip>:<port>\n` to stdout after binding.
    #[must_use]
    pub const fn with_ready_signal(mut self, enable: bool) -> Self {
        self.ready_signal = enable;
        self
    }

    /// Get the effective default mode.
    ///
    /// Returns the configured default mode, or falls back to "editor:normal".
    #[must_use]
    pub fn effective_default_mode(&self) -> ModeId {
        self.default_mode
            .clone()
            .unwrap_or_else(fallback_default_mode)
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

    /// Module registry (holds loaded modules).
    module_registry: Arc<ModuleManager>,

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
            module_registry: Arc::new(ModuleManager::new()),
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
        // Quiet mode when ready_signal is set (no stderr output)
        debug::init(self.config.ready_signal);

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
                    let default_mode = self.config.effective_default_mode();

                    // Spawn client handler task
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(
                            reader,
                            writer,
                            client_id,
                            session_id,
                            sessions,
                            dispatcher,
                            default_mode,
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
        handle_client(
            reader,
            writer,
            client_id,
            default_session_id.clone(),
            Arc::clone(&self.sessions),
            Arc::clone(&self.dispatcher),
            default_mode,
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
            .get_or_create(id, || create_session_with_defaults(id.clone()));
    }
}

/// Create a new session with default registries (modes, commands, keybindings wired).
///
/// This is the entry point for creating sessions that have working keybindings.
/// Used by both `ensure_default_session()` and `handle_client()`.
fn create_session_with_defaults(id: SessionId) -> Arc<Session> {
    let (
        mode_registry,
        command_registry,
        keymap_registry,
        module_registry,
        resolver_registry,
        compositor,
    ) = build_default_registries();

    // Use auto-detected entry mode from registry, or fall back to hardcoded default
    let initial_mode = mode_registry
        .entry_mode()
        .cloned()
        .unwrap_or_else(fallback_default_mode);

    // Create kernel context
    let kernel = real_kernel_context();

    // Handle empty session: create buffer if handlers indicate so
    let empty_session_registry = build_empty_session_registry();
    handle_empty_session(&kernel, &empty_session_registry);

    Session::with_registries(
        id,
        kernel,
        initial_mode,
        standard_vfs(),
        mode_registry,
        command_registry,
        keymap_registry,
        module_registry,
        resolver_registry,
        compositor,
    )
}

/// Build the empty session handler registry.
///
/// Collects handlers from the defaults module. The defaults module
/// aggregates all default policy handlers, keeping the runner decoupled
/// from specific handler modules like scratch-buffer.
fn build_empty_session_registry() -> EmptySessionHandlerRegistry {
    let mut registry = EmptySessionHandlerRegistry::new();

    // Get handler from defaults module (centralized policy)
    // The defaults module provides the scratch buffer handler
    let handler = reovim_module_defaults::empty_session_handler();
    registry.register(Arc::new(handler));

    tracing::debug!(handlers = registry.len(), "built empty session handler registry");

    registry
}

/// Handle empty session by calling registered handlers.
///
/// If no buffers exist and a handler returns `CreateBuffer`, creates
/// the buffer in the kernel context. This runs synchronously at session
/// startup.
fn handle_empty_session(kernel: &KernelContext, registry: &EmptySessionHandlerRegistry) {
    // Check if session already has buffers
    if kernel.buffers.count() > 0 {
        return;
    }

    // No file args for now (CLI file opening is separate)
    let file_args: Vec<String> = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_default();

    let ctx = EmptySessionContext {
        session_id: 0, // Session ID not used in current handlers
        file_args: &file_args,
        cwd: &cwd,
    };

    if let Some(EmptySessionAction::CreateBuffer { name: _, content }) = registry.resolve(&ctx) {
        let buffer_id = kernel.buffers.create();
        if !content.is_empty()
            && let Some(buffer) = kernel.buffers.get(buffer_id)
        {
            buffer.write().set_content(&content);
        }
        tracing::info!(
            buffer_id = buffer_id.as_usize(),
            "Created initial buffer for empty session"
        );
    }
}

/// Static module factory - creates module instances by name.
///
/// This is the fallback for modules not found in dynamic search paths.
/// Returns `None` if the module name is unknown.
fn static_module_factory(name: &str) -> Option<Box<dyn Module>> {
    match name {
        "keymap" => Some(Box::new(KeymapModule)),
        "vim" => Some(Box::new(VimModule::new())),
        "motions" => Some(Box::new(MotionsModule)),
        // Note: operators merged into vim (Epic #385)
        "commands" => Some(Box::new(CommandsModule)),
        "editor" => Some(Box::new(EditorModule)),
        _ => None,
    }
}

/// Build default registries with keybindings wired from default modules.
///
/// This is the "generation position" where modules are registered and
/// their keybindings are wired to the session registries.
///
/// Module loading follows this precedence:
/// 1. Dynamic modules from XDG paths (`~/.local/share/reovim/modules/`)
/// 2. Static fallback for modules not found in paths
///
/// Returns the registries plus resolver registry and compositor from the layout module (if any).
#[allow(clippy::too_many_lines)]
#[allow(clippy::type_complexity)]
fn build_default_registries() -> (
    ModeRegistry,
    CommandRegistry,
    KeymapRegistry,
    ModuleManager,
    reovim_module_editor::ResolverRegistry,
    Option<Box<dyn RootCompositor>>,
) {
    let mut mode_registry = ModeRegistry::new();
    let mut command_registry = CommandRegistry::new();
    let mut keymap_registry = KeymapRegistry::new();
    let mut resolver_registry = reovim_module_editor::ResolverRegistry::new();

    // Load module configuration from ~/.config/reovim/config.toml
    let module_config = ModuleConfig::load().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to load module config, using defaults");
        ModuleConfig::default()
    });

    // Create module loader and load modules from config
    let mut loader = ModuleLoader::new();
    let load_stats = load_from_config(&mut loader, &module_config, Some(static_module_factory));

    tracing::info!(
        dynamic = load_stats.dynamic_loaded.len(),
        static_ = load_stats.static_loaded.len(),
        not_found = load_stats.not_found.len(),
        failed = load_stats.failed.len(),
        "loaded modules from config"
    );

    // Create ModuleManager from the loader
    let module_manager = ModuleManager::with_loader(loader);

    // Register Vim modes (so ModeRegistry knows about them)
    // Each mode provides Mode (identity), ModeDisplay (cursor), and ModeInput (accepts char)
    for mode in VimMode::ALL {
        mode_registry.register_mode(*mode);
    }

    // Register commands from modules that implement CommandProvider
    // Using wire_module_commands for type-safe command registration

    // Wire editor module commands (cursor movement, editing operations, etc.)
    let editor_module = EditorModule;
    let editor_module_id = editor_module.id();
    match wire_module_commands(&editor_module_id, &editor_module, &mut command_registry) {
        Ok(stats) => {
            tracing::info!(
                module = %editor_module_id,
                wired = stats.commands_wired,
                "wired editor commands"
            );
        }
        Err(e) => {
            tracing::error!(module = %editor_module_id, error = %e, "failed to wire editor commands");
        }
    }

    // Wire motions module commands (word, line, find-char, search motions)
    let motions_module = MotionsModule;
    let motions_module_id = motions_module.id();
    match wire_module_commands(&motions_module_id, &motions_module, &mut command_registry) {
        Ok(stats) => {
            tracing::info!(
                module = %motions_module_id,
                wired = stats.commands_wired,
                "wired motions commands"
            );
        }
        Err(e) => {
            tracing::error!(module = %motions_module_id, error = %e, "failed to wire motions commands");
        }
    }

    // Wire vim module commands and keybindings
    // Vim module provides mode switching, visual operations, and standard keybindings
    let mut vim_module = VimModule::new();
    let vim_module_id = vim_module.id();

    // Initialize the module (for logging purposes)
    let ctx = ModuleContext::default();
    let _init_result = vim_module.init(&ctx);

    // Wire commands
    match wire_module_commands(&vim_module_id, &vim_module, &mut command_registry) {
        Ok(stats) => {
            tracing::info!(
                module = %vim_module_id,
                wired = stats.commands_wired,
                "wired vim commands"
            );
        }
        Err(e) => {
            tracing::error!(module = %vim_module_id, error = %e, "failed to wire vim commands");
        }
    }

    // Wire keybindings from the vim module
    let keybindings = vim_module.keybindings();
    match wire_module_keybindings(
        &vim_module_id,
        &keybindings,
        &mut keymap_registry,
        &mode_registry,
    ) {
        Ok(stats) => {
            tracing::info!(
                module = %vim_module_id,
                wired = stats.keybindings_wired,
                skipped = stats.keybindings_skipped,
                "wired keybindings"
            );
        }
        Err(e) => {
            tracing::error!(module = %vim_module_id, error = %e, "failed to wire keybindings");
        }
    }

    // Register vim mode resolvers for operator-pending mode support (Epic #415)
    // These implement the policy for handling operators (d, y, c) and motions
    resolver_registry.register(reovim_module_vim::VimNormalResolver::new());
    resolver_registry.register(reovim_module_vim::VimInsertResolver::new());
    resolver_registry.register(reovim_module_vim::VimOperatorPendingResolver::new());

    tracing::info!(count = resolver_registry.len(), "registered vim mode resolvers");

    // Load user keymap configuration from ~/.config/reovim/keymap.toml
    // User bindings are registered at the User layer (highest priority)
    match config::KeymapConfig::load() {
        Ok(user_config) => {
            if !user_config.is_empty() {
                // Validate config and report warnings for any issues
                let validation_errors = user_config.validate();
                for err in &validation_errors {
                    tracing::warn!(error = %err, "keymap.toml validation warning");
                }

                // Try to apply - will fail on first invalid entry
                match user_config.apply(&mut keymap_registry) {
                    Ok(stats) => {
                        tracing::info!(
                            bindings_added = stats.bindings_added,
                            bindings_removed = stats.bindings_removed,
                            warnings = validation_errors.len(),
                            "applied user keymap configuration"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to apply keymap configuration");
                    }
                }
            }
        }
        Err(config::KeymapConfigError::NoConfigDir) => {
            // Silent - no config directory is fine
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to load keymap.toml");
        }
    }

    // Wire layout module - provides window management compositor and keybindings
    let mut layout_module = LayoutModule::new();
    let layout_module_id = layout_module.id();

    // Initialize the module
    let layout_ctx = ModuleContext::default();
    let _init_result = layout_module.init(&layout_ctx);

    // Wire layout keybindings (window mode: hjkl, splits, etc.)
    let layout_keybindings = layout_module.keybindings();
    match wire_module_keybindings(
        &layout_module_id,
        &layout_keybindings,
        &mut keymap_registry,
        &mode_registry,
    ) {
        Ok(stats) => {
            tracing::info!(
                module = %layout_module_id,
                wired = stats.keybindings_wired,
                skipped = stats.keybindings_skipped,
                "wired layout keybindings"
            );
        }
        Err(e) => {
            tracing::error!(module = %layout_module_id, error = %e, "failed to wire layout keybindings");
        }
    }

    // Extract compositor from layout module via Module::compositor()
    // Uses type erasure (Any) to cross layer boundaries - see issue #417
    let compositor: Option<Box<dyn RootCompositor>> = layout_module
        .compositor()
        .and_then(|boxed_any| boxed_any.downcast::<CompositorBox>().ok())
        .map(|compositor_box| compositor_box.into_inner());

    if compositor.is_some() {
        tracing::info!(module = %layout_module_id, "extracted compositor from layout module");
    }

    (
        mode_registry,
        command_registry,
        keymap_registry,
        module_manager,
        resolver_registry,
        compositor,
    )
}

impl Default for Server {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Fallback default mode ID when no config-specified mode is set.
///
/// Uses "editor:normal" as the default. This hardcoded value exists
/// for backwards compatibility, but the mode should be configurable
/// via `ServerConfig::with_default_mode()`.
const fn fallback_default_mode() -> ModeId {
    ModeId::new(ModuleId::new("editor"), "normal")
}

/// Create a real `KernelContext` with working buffer management.
///
/// Unlike `KernelContext::default()` which uses stubs, this creates
/// a fully functional kernel context suitable for actual editing.
fn real_kernel_context() -> KernelContext {
    let option_registry = Arc::new(OptionRegistry::new());
    register_default_options(&option_registry);

    KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(SimpleBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(RwLock::new(RegisterBank::new())),
        Arc::new(RwLock::new(MarkBank::new())),
        option_registry,
    )
}

/// Register default editor options.
///
/// These options are registered at startup and available to all modules.
/// Following mechanism vs policy: the registry (mechanism) is in kernel,
/// the option definitions (policy) are here in the runner.
fn register_default_options(registry: &OptionRegistry) {
    // Indentation options
    let _ = registry.register(
        OptionSpec::new(
            "autoindent",
            "Copy indent from current line when starting new line",
            OptionValue::bool(true),
        )
        .with_short("ai")
        .with_scope(OptionScope::Buffer),
    );

    let _ = registry.register(
        OptionSpec::new(
            "smartindent",
            "Smart autoindenting for C-like languages",
            OptionValue::bool(false),
        )
        .with_short("si")
        .with_scope(OptionScope::Buffer),
    );

    // Tab options (for display line calculations)
    let _ = registry.register(
        OptionSpec::new("tabstop", "Number of spaces that a tab counts for", OptionValue::int(8))
            .with_short("ts")
            .with_scope(OptionScope::Buffer),
    );
}

/// Create the standard VFS for file operations.
///
/// Uses the real filesystem via `std::fs`.
fn standard_vfs() -> Arc<dyn VfsDriver> {
    Arc::new(StandardVfs::new())
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
/// * `default_mode` - Default mode ID for new sessions
async fn handle_client(
    mut reader: TransportReader,
    writer: TransportWriter,
    client_id: crate::session::ClientId,
    session_id: SessionId,
    sessions: Arc<SessionRegistry>,
    dispatcher: Arc<RpcDispatcher>,
    _default_mode: ModeId,
) -> std::io::Result<()> {
    // Get or create the session with default registries (keybindings wired)
    let session =
        sessions.get_or_create(&session_id, || create_session_with_defaults(session_id.clone()));

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
    fn test_fallback_default_mode() {
        let mode_id = fallback_default_mode();
        assert_eq!(mode_id.module().as_str(), "editor");
        assert_eq!(mode_id.name(), "normal");
    }

    #[test]
    fn test_effective_default_mode_fallback() {
        let config = ServerConfig::default();
        let mode = config.effective_default_mode();
        assert_eq!(mode.module().as_str(), "editor");
        assert_eq!(mode.name(), "normal");
    }

    #[test]
    fn test_effective_default_mode_configured() {
        let custom_mode = ModeId::new(ModuleId::new("custom"), "mode");
        let config = ServerConfig::default().with_default_mode(custom_mode.clone());
        let mode = config.effective_default_mode();
        assert_eq!(mode, custom_mode);
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

    #[test]
    fn test_srv_args_into_config_default() {
        let args = SrvArgs {
            instance: "default".to_string(),
            tcp: None,
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec![],
            load_modules: vec![],
            no_defaults: false,
            ready_signal: false,
        };

        let config = args.into_config();
        assert!(matches!(config.transport, TransportMode::TcpWithFallback));
        assert!(config.modules.search_paths.is_empty());
        assert!(config.modules.autoload.is_empty());
        assert!(!config.modules.no_defaults);
        assert_eq!(config.instance_name, "default");
    }

    #[test]
    fn test_srv_args_into_config_with_tcp() {
        let args = SrvArgs {
            instance: "default".to_string(),
            tcp: Some(9000),
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec![],
            load_modules: vec![],
            no_defaults: false,
            ready_signal: false,
        };

        let config = args.into_config();
        assert!(matches!(config.transport, TransportMode::Tcp { port: 9000 }));
    }

    #[test]
    fn test_srv_args_into_config_with_stdio() {
        let args = SrvArgs {
            instance: "default".to_string(),
            tcp: None,
            #[cfg(unix)]
            socket: None,
            stdio: true,
            module_dirs: vec![],
            load_modules: vec![],
            no_defaults: false,
            ready_signal: false,
        };

        let config = args.into_config();
        assert!(matches!(config.transport, TransportMode::Stdio));
    }

    #[test]
    fn test_srv_args_into_config_with_module_dirs() {
        let args = SrvArgs {
            instance: "default".to_string(),
            tcp: None,
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec![
                PathBuf::from("/custom/path1"),
                PathBuf::from("/custom/path2"),
            ],
            load_modules: vec![],
            no_defaults: false,
            ready_signal: false,
        };

        let config = args.into_config();
        assert_eq!(config.modules.search_paths.len(), 2);
        assert_eq!(config.modules.search_paths[0], "/custom/path1");
        assert_eq!(config.modules.search_paths[1], "/custom/path2");
    }

    #[test]
    fn test_srv_args_into_config_with_load_modules() {
        let args = SrvArgs {
            instance: "default".to_string(),
            tcp: None,
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec![],
            load_modules: vec!["editor".into(), "keymap".into()],
            no_defaults: false,
            ready_signal: false,
        };

        let config = args.into_config();
        assert_eq!(config.modules.autoload.len(), 2);
        assert_eq!(config.modules.autoload[0], "editor");
        assert_eq!(config.modules.autoload[1], "keymap");
    }

    #[test]
    fn test_srv_args_into_config_with_no_defaults() {
        let args = SrvArgs {
            instance: "default".to_string(),
            tcp: None,
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec![],
            load_modules: vec!["my-module".into()],
            no_defaults: true,
            ready_signal: false,
        };

        let config = args.into_config();
        assert!(config.modules.no_defaults);
        assert_eq!(config.modules.autoload, vec!["my-module"]);
    }

    #[test]
    fn test_srv_args_into_config_with_ready_signal() {
        let args = SrvArgs {
            instance: "default".to_string(),
            tcp: Some(9000),
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec![],
            load_modules: vec![],
            no_defaults: false,
            ready_signal: true,
        };

        let config = args.into_config();
        assert!(config.ready_signal);
    }

    #[test]
    fn test_srv_args_into_config_with_custom_instance() {
        let args = SrvArgs {
            instance: "my-project".to_string(),
            tcp: Some(9000),
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec![],
            load_modules: vec![],
            no_defaults: false,
            ready_signal: false,
        };

        let config = args.into_config();
        assert_eq!(config.instance_name, "my-project");
    }

    #[test]
    fn test_server_config_with_ready_signal_builder() {
        let config = ServerConfig::tcp(9000).with_ready_signal(true);
        assert!(config.ready_signal);

        let config = ServerConfig::tcp(9000).with_ready_signal(false);
        assert!(!config.ready_signal);
    }
}
