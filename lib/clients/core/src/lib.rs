//! Reovim Client Core - connection, discovery, and RPC.
//!
//! This crate provides the core client functionality for connecting
//! to reovim servers. It handles:
//!
//! - **Connection**: TCP and Unix socket transport abstraction
//! - **Discovery**: Server enumeration and auto-discovery
//! - **RPC**: JSON-RPC v1 protocol client
//!
//! # Architecture
//!
//! This crate is part of the Epic #465 server/client split:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  lib/clients/tui/                  (Phase 6 - Future)       │
//! │  lib/clients/cli/                  (Phase 7 - Future)       │
//! ├─────────────────────────────────────────────────────────────┤
//! │  lib/clients/core/                 (THIS CRATE - Phase 5)   │
//! │    Connection, Discovery, RpcClient                         │
//! ├─────────────────────────────────────────────────────────────┤
//! │  lib/protocol/                     (Shared RPC types)       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_client_core::{ConnectionConfig, RpcClient};
//!
//! // Auto-discover a running server
//! let config = ConnectionConfig::auto_discover();
//!
//! // Or specify explicitly
//! let config = ConnectionConfig::tcp("127.0.0.1", 12522);
//!
//! // Connect and make RPC calls
//! let mut client = RpcClient::connect(&config).await?;
//! let result = client.call("state/mode", serde_json::json!({})).await?;
//! ```
//!
//! # Concurrent Operation
//!
//! For TUI clients that need to handle notifications while sending requests,
//! use [`RpcClient::into_split`]:
//!
//! ```ignore
//! let client = RpcClient::connect(&config).await?;
//! let (reader, mut writer) = client.into_split();
//!
//! // Spawn notification listener
//! tokio::spawn(async move {
//!     loop {
//!         let line = reader.read_line().await?;
//!         // Handle notification...
//!     }
//! });
//!
//! // Send requests from main task
//! writer.send_request("input/keys", json!({"keys": "j"})).await?;
//! ```

pub mod connection;
pub mod discovery;
pub mod output;
pub mod rpc;

// Re-exports for convenience
pub use {
    connection::{Connection, ConnectionConfig, ConnectionReader, ConnectionWriter},
    discovery::{ServerInfo, list_servers},
    output::{OutputFormat, format_output, format_output_for_command},
    rpc::{RpcClient, RpcClientError, RpcWriter, ServerMessage},
};
