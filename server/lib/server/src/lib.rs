//! Reovim Server - the editing engine.
//!
//! This crate provides the server-side implementation of reovim:
//! - Session management with shared state
//! - Buffer operations via kernel
//! - gRPC v2 protocol services
//!
//! # Architecture
//!
//! The server follows mechanism/policy separation (Unix philosophy):
//! - **Server provides WHAT**: raw buffer data, cursor position, options
//! - **Client decides HOW**: rendering, gutters, decorations, themes
//!
//! # Example
//!
//! ```ignore
//! use reovim_server::{Server, ServerConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ServerConfig::grpc(12540);
//!     let server = Server::new(config);
//!     server.run().await?;
//!     Ok(())
//! }
//! ```

pub mod app;
pub mod config;
pub mod registry;
pub mod session;

#[cfg(feature = "grpc")]
pub mod grpc;

mod server;

// Public API
pub use {
    app::{AppState, CmdlineBuffer},
    config::{ServerConfig, TransportMode},
    registry::{
        CommandQuerySnapshot, CommandRegistry, KeyLookupResult, KeymapRegistry, ModeEntry,
        ModeRegistry,
    },
    server::{Server, SessionFactory},
    session::{Session, SessionId, SessionRegistry, SessionState},
};
