//! Reovim Runner - Headless Editor Server
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
//! ├── lib.rs           - Re-exports from server/
//! ├── main.rs          - Entry point
//! └── server/          - ALL server-specific code
//!     ├── mod.rs       - Server struct and config
//!     ├── app.rs       - AppState (kernel + runtime state)
//!     ├── event_loop.rs - Synchronous event loop
//!     ├── fallback.rs  - InputFallbackHandler trait
//!     ├── notification.rs - NotificationBroadcaster
//!     ├── session/     - Session management
//!     ├── client/      - Client connections
//!     ├── transport/   - Transport layer (TCP, Unix, Stdio)
//!     ├── registry/    - Mode, Command, Keymap registries
//!     ├── module/      - Module system
//!     └── rpc/         - RPC dispatcher + handlers
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
pub use server::{client, module, notification, registry, session, transport};

// Backwards compat: allow `runner::fallback::*`
pub mod fallback {
    //! Fallback handlers for unhandled input.
    pub use crate::server::fallback::*;
}
