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
//! │                       Server                               │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │ SessionRegistry                                     │   │
//! │  │ └── Session "default"                               │   │
//! │  │     └── SessionState (AppState + Registries)        │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! │                          │                                 │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │ TcpTransport                                        │   │
//! │  │ └── accept() → spawn client task                    │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! │                          │                                 │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │ RpcDispatcher                                       │   │
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

// SrvArgs moved to args.rs (Epic #417 Part 2)

// Submodules - all server-specific code lives here
mod app;
mod args;
mod bootstrap;
pub mod capture;
pub mod client;
pub mod config;
pub mod debug;
mod event_loop;
mod handler;
pub mod instance;
pub mod module;
pub mod notification;
pub mod registry;
pub mod rpc;
#[allow(clippy::module_inception)] // Intentional: Server struct in server.rs
mod server;
mod server_config;
pub mod session;
pub mod transport;

// Re-exports for public API
pub use {
    app::AppState,
    // CLI arguments (Epic #417 Part 2)
    args::SrvArgs,
    event_loop::{EventLoop, EventLoopError},
    notification::NotificationBroadcaster,
    // Fallback types from driver (breaking the circular dependency)
    reovim_driver_input::{
        BeepFallback, FallbackContext, FallbackResult, InputFallbackHandler, NoOpFallback,
    },
    // Server struct (Epic #417 Part 2)
    server::Server,
    // Server configuration (Epic #417 Part 2)
    server_config::{ServerConfig, TransportMode},
};

// Server struct moved to server.rs (Epic #417 Part 2)
// Handler moved to handler.rs (Epic #417 Part 2)
// Bootstrap functions moved to bootstrap/ (Epic #417 Part 2)
// Tests moved to their respective modules (server.rs, server_config.rs, args.rs)
