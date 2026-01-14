//! Wire protocol types for reovim client-server communication.
//!
//! This crate defines the JSON-RPC 2.0 message types and shared data structures
//! used for communication between the reovim server and its clients (TUI, CLI).
//!
//! # Architecture
//!
//! The protocol crate is designed to be:
//! - **Minimal**: Only serde dependencies, no runtime logic
//! - **Versioned**: Types are organized under version modules (v1, v2, etc.)
//! - **Shared**: Used by both server and client crates
//!
//! # Example
//!
//! ```
//! use reovim_protocol::{RpcRequest, methods};
//!
//! // Create a typed request
//! let request = RpcRequest::new(1, methods::STATE_CURSOR, serde_json::json!({}));
//! ```

pub mod codec;
pub mod v1;

// Re-export v1 as the default API
pub use v1::*;
