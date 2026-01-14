//! Transport layer for the reovim server.
//!
//! This module provides network transport implementations supporting:
//! - **TCP**: Default transport with port fallback (12521-12530)
//! - **Unix Socket**: For local IPC (`--listen-socket /tmp/reovim.sock`)
//! - **Stdio**: For process embedding (`--stdio`)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                   TransportListener                      │
//! │  ├── bind_tcp(port) / bind_tcp_with_fallback()          │
//! │  ├── bind_unix(path)                                     │
//! │  └── accept() → (TransportReader, TransportWriter)       │
//! └─────────────────────────────────────────────────────────┘
//! ┌─────────────────────────────────────────────────────────┐
//! │              TransportReader / TransportWriter           │
//! │  ├── from_tcp(half)                                      │
//! │  ├── from_unix(half)                                     │
//! │  ├── from_stdio()                                        │
//! │  └── read_line() / write_line()                          │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use runner::transport::TransportListener;
//!
//! // Bind with automatic port fallback (12521-12530)
//! let listener = TransportListener::bind_tcp_with_fallback().await?;
//! eprintln!("Listening on {}", listener.local_addr_string());
//!
//! // Accept loop
//! loop {
//!     let (reader, writer) = listener.accept().await?;
//!     // Spawn client handler with reader/writer...
//! }
//! ```

mod connection;
mod listener;
mod tcp;

pub use {
    connection::{TransportReader, TransportWriter},
    listener::TransportListener,
    tcp::{DEFAULT_HOST, DEFAULT_PORT, PORT_FALLBACK_COUNT, TcpTransport},
};
