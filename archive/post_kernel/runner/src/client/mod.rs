//! Outbound client infrastructure for connecting TO a reovim server.
//!
//! This module provides TUI and CLI clients that connect to a running
//! reovim server via TCP, Unix socket, or stdio.
//!
//! # Architecture
//!
//! ```text
//! client/
//! ├── common/           # Shared connection/discovery/rpc
//! │   ├── connection.rs # TCP, socket connections
//! │   ├── discovery.rs  # Find running servers
//! │   └── rpc.rs        # JSON-RPC client
//! ├── tui/              # Terminal UI client
//! │   ├── app.rs        # Main loop
//! │   ├── input.rs      # Key handling
//! │   └── render.rs     # Screen rendering
//! └── cli/              # Command-line client
//!     ├── commands.rs   # Command handlers
//!     ├── output.rs     # Formatting
//!     └── repl.rs       # Interactive mode
//! ```

pub mod cli;
pub mod common;
pub mod tui;

pub use common::{Connection, ConnectionConfig, RpcClient, ServerMessage};
