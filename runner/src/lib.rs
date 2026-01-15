//! Reovim Runner - Headless Editor Server with TUI and CLI Clients
//!
//! This crate provides a headless editor server using the kernel-driver
//! type system from issue #213. It implements the "mechanism vs policy"
//! principle where:
//!
//! - **Mechanism** (this crate): Server, sessions, registries
//! - **Policy** (modules): Mode implementations, commands, keybindings
//!
//! # Architecture
//!
//! ```text
//! runner/
//! ├── lib.rs           - Re-exports from server/ and client/
//! ├── main.rs          - Entry point (server/tui/cli dispatcher)
//! ├── server/          - Server-side code
//! │   ├── mod.rs       - Server struct, config, SrvArgs
//! │   ├── app.rs       - AppState (kernel + runtime state)
//! │   ├── event_loop.rs - Synchronous event loop
//! │   ├── fallback.rs  - InputFallbackHandler trait
//! │   ├── notification.rs - NotificationBroadcaster
//! │   ├── session/     - Session management
//! │   ├── client/      - Inbound client connections (server-side)
//! │   ├── transport/   - Transport layer (TCP, Unix, Stdio)
//! │   ├── registry/    - Mode, Command, Keymap registries
//! │   ├── module/      - Module system
//! │   └── rpc/         - RPC dispatcher + handlers
//! └── client/          - Outbound client code (TUI/CLI)
//!     ├── mod.rs       - Re-exports
//!     ├── common/      - Shared connection, RPC, discovery
//!     │   ├── connection.rs - TCP/Unix socket connections
//!     │   ├── discovery.rs  - Find running servers
//!     │   └── rpc.rs        - JSON-RPC client
//!     ├── tui/         - Terminal UI client
//!     │   ├── mod.rs   - TuiArgs
//!     │   ├── app.rs   - Main loop
//!     │   ├── input.rs - Key handling
//!     │   └── render.rs - Screen rendering
//!     └── cli/         - Command-line client
//!         ├── mod.rs   - CliArgs, CliAction
//!         ├── commands.rs - Command handlers
//!         ├── output.rs - Formatting
//!         └── repl.rs  - Interactive mode
//! ```
//!
//! # Example - Server Mode
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
//!
//! # Example - TUI Client
//!
//! ```ignore
//! use runner::client::{common::ConnectionConfig, tui::TuiApp};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ConnectionConfig::auto_discover();
//!     let mut app = TuiApp::connect(&config).await?;
//!     app.run().await?;
//!     Ok(())
//! }
//! ```
//!
//! # Example - CLI Client
//!
//! ```ignore
//! use runner::client::common::{ConnectionConfig, RpcClient};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ConnectionConfig::auto_discover();
//!     let mut client = RpcClient::connect(&config).await?;
//!     let result = client.call("input/keys", json!({ "keys": "iHello<Esc>" })).await?;
//!     println!("{result}");
//!     Ok(())
//! }
//! ```
//!
//! # Example - Embedded Mode (Synchronous)
//!
//! ```ignore
//! use runner::{AppState, EventLoop, registry::*};
//! use runner::fallback::NoOpFallback;
//!
//! let app = AppState::new(kernel_context, initial_mode);
//! let mut event_loop = EventLoop::new(
//!     app,
//!     mode_registry,
//!     command_registry,
//!     keymap_registry,
//!     NoOpFallback,
//! );
//! event_loop.run()?;
//! ```

// All server-specific code lives in the server module
pub mod server;

// Client modules (TUI and CLI)
pub mod client;

// Buffer manager implementation
pub mod buffer_manager;

// Re-export SimpleBufferManager
pub use buffer_manager::SimpleBufferManager;

// Re-exports for backwards compatibility and convenience
pub use server::{
    // Core types
    AppState,
    // Fallback handlers
    BeepFallback,
    EventLoop,
    EventLoopError,
    FallbackResult,
    InputFallbackHandler,
    NoOpFallback,
    NotificationBroadcaster,
    Server,
    ServerConfig,
    TransportMode,
};

// Re-export submodules for backwards compatibility
// Note: server::client is NOT re-exported to avoid conflict with runner::client (TUI/CLI)
pub use server::{module, notification, registry, session, transport};

// Backwards compat: allow `runner::fallback::*`
pub mod fallback {
    //! Fallback handlers for unhandled input.
    pub use crate::server::fallback::*;
}
