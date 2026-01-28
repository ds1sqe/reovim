//! Common client infrastructure for outbound connections.
//!
//! Shared functionality for TUI and CLI clients.

pub mod connection;
pub mod discovery;
pub mod rpc;

pub use {
    connection::{Connection, ConnectionConfig, ConnectionReader, ConnectionWriter},
    discovery::{ServerInfo, list_servers},
    rpc::{RpcClient, RpcClientError, RpcWriter, ServerMessage},
};
