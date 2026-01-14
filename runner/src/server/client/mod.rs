//! Client management for the reovim server.
//!
//! This module provides client connection handling. Each client represents
//! a connected UI (terminal, GUI, etc.) that attaches to a session.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    ClientRegistry                        │
//! │  ├── Client 1 (id=1, session="default")                 │
//! │  ├── Client 2 (id=2, session="default")                 │
//! │  └── Client 3 (id=3, session="project-x")               │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Concurrency Model
//!
//! Following `docs/reference/concurrency.md`:
//!
//! | Level | Component | Lock Type |
//! |-------|-----------|-----------|
//! | 0 | `ClientRegistry` | Lock-free (`ArcSwap`) |
//! | 2 | Client writer | `tokio::sync::Mutex` |
//!
//! # Example
//!
//! ```ignore
//! use runner::client::{Client, ClientRegistry};
//! use runner::session::{ClientId, SessionId};
//!
//! // Create client from TCP connection
//! let client = Client::new(ClientId::new(1), SessionId::default(), writer);
//!
//! // Add to registry
//! let registry = ClientRegistry::new();
//! registry.insert(client.clone());
//!
//! // Send response
//! client.send_line(r#"{"jsonrpc":"2.0","result":"ok","id":1}"#).await?;
//!
//! // Broadcast to all clients in session
//! for c in registry.iter() {
//!     c.send_line("event").await?;
//! }
//! ```

#[allow(clippy::module_inception)]
mod client;
mod registry;

pub use {client::Client, registry::ClientRegistry};
