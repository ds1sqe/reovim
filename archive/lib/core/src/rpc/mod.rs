//! RPC (Remote Procedure Call) module for server mode
//!
//! Provides JSON-RPC 2.0 over multiple transport types for external tool integration.
//! Enables programmatic control of the editor for debugging and testing.
//!
//! Supported transports:
//! - Stdio: JSON-RPC over stdin/stdout (for process piping)
//! - Unix socket: JSON-RPC over Unix domain socket
//! - TCP: JSON-RPC over TCP connection

pub mod handler;
pub mod server;
pub mod state;
pub mod transport;

// Re-export types from driver (Phase 4-6 migration)
pub use reovim_driver_net::{
    BUFFER_NOT_FOUND, COMMAND_NOT_FOUND, INTERNAL_ERROR, INVALID_KEY_NOTATION, INVALID_PARAMS,
    INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR, RpcError, RpcNotification, RpcRequest,
    RpcResponse, TransportConfig, methods, notifications,
};

// Keep core-specific types (richer context for plugins)
pub use handler::{RpcHandler, RpcHandlerContext, RpcHandlerRegistry, RpcResult};

pub use {
    server::{RpcServer, ServerConfig},
    state::*,
    transport::{TransportClient, TransportConnection, TransportListener},
};

// Re-export key notation parser from testing module
pub use crate::testing::keys::keys_from_str;
