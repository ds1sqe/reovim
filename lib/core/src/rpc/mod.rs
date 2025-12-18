//! RPC (Remote Procedure Call) module for server mode
//!
//! Provides JSON-RPC 2.0 over multiple transport types for external tool integration.
//! Enables programmatic control of the editor for debugging and testing.
//!
//! Supported transports:
//! - Stdio: JSON-RPC over stdin/stdout (for process piping)
//! - Unix socket: JSON-RPC over Unix domain socket
//! - TCP: JSON-RPC over TCP connection

pub mod server;
pub mod state;
pub mod transport;
pub mod types;

pub use {
    server::{RpcServer, ServerConfig},
    state::*,
    transport::{TransportClient, TransportConfig, TransportConnection, TransportListener},
    types::*,
};

// Re-export key notation parser from testing module
pub use crate::testing::keys::keys_from_str;
