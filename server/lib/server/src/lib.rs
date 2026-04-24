#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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
pub mod debug;
pub mod registry;
pub mod session;

#[cfg(feature = "grpc")]
pub mod grpc;

mod server;
#[cfg(feature = "grpc")]
pub(crate) mod tick;

pub(crate) mod transport_inproc;
pub(crate) mod transport_pipe;

// Public API
pub use {
    app::AppState,
    config::{ServerConfig, TransportMode},
    registry::{
        CommandQuerySnapshot, CommandRegistry, KeymapRegistry, LookupResult, ModeEntry,
        ModeRegistry,
    },
    server::{Server, SessionFactory},
    session::{Session, SessionId, SessionRegistry, SessionState, SyntaxStreamState},
};

/// Creates a pair of bidirectional in-memory streams for inproc
/// transport.
///
/// Returns `(server_side, client_side)`. The server end is passed to
/// [`Server::run_inproc`]; the client end goes to the in-process
/// client's connect function. Capacity is the tokio default
/// `duplex()` size — enough for a keystroke-per-frame workload with
/// headroom for burstier paint cycles.
///
/// Exposed as a constructor so callers don't re-export the raw
/// `tokio::io::DuplexStream` type through the launcher's public
/// surface.
#[must_use]
pub fn inproc_channel_pair() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
    const INPROC_BUFFER_BYTES: usize = 64 * 1024;
    tokio::io::duplex(INPROC_BUFFER_BYTES)
}
