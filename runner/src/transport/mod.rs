//! Transport layer for the reovim server.
//!
//! This module provides network transport implementations. Currently only
//! TCP is supported (MVP), with plans for Unix socket support later.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                     TcpTransport                         │
//! │  ├── bind(addr) / bind_with_fallback()                  │
//! │  └── accept() → (TcpStream, SocketAddr)                 │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use runner::transport::TcpTransport;
//!
//! // Bind with automatic port fallback (12521-12530)
//! let transport = TcpTransport::bind_with_fallback().await?;
//! eprintln!("Listening on {}", transport.local_addr());
//!
//! // Accept loop
//! loop {
//!     let (stream, addr) = transport.accept().await?;
//!     // Spawn client handler...
//! }
//! ```

mod tcp;

pub use tcp::{DEFAULT_HOST, DEFAULT_PORT, PORT_FALLBACK_COUNT, TcpTransport};
