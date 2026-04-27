#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Wire protocol types for reovim client-server communication.
//!
//! This crate defines the message types and shared data structures
//! used for communication between the reovim server and its clients (TUI, CLI, GUI).
//!
//! # Architecture
//!
//! The protocol crate is designed to be:
//! - **Minimal**: Only serde dependencies (+ tonic/prost with `grpc` feature)
//! - **Versioned**: Types are organized under version modules (v3, etc.)
//! - **Shared**: Used by both server and client crates
//!
//! # Protocol Versions
//!
//! - **v1**: JSON-RPC 2.0 with cell-grid based rendering (default)
//! - **v3**: gRPC with domain-neutral projection model (requires `grpc` feature)
//!
//! # Example (v1 JSON-RPC)
//!
//! ```
//! use reovim_protocol::{RpcRequest, methods};
//!
//! // Create a typed request
//! let request = RpcRequest::new(1, methods::STATE_CURSOR, serde_json::json!({}));
//! ```
//!
//! # Example (v3 gRPC)
//!
//! ```ignore
//! // Requires `grpc` feature
//! use reovim_protocol::v3::state_service_client::StateServiceClient;
//!
//! let mut client = StateServiceClient::connect("http://127.0.0.1:12523").await?;
//! let response = client.get_projections(GetProjectionsRequest::default()).await?;
//! ```

pub mod codec;
pub mod instance;
pub mod v1;

/// Protocol v3: gRPC-based domain-neutral model.
///
/// Requires the `grpc` feature to be enabled.
#[cfg(feature = "grpc")]
pub mod v3;

// Re-export v1 as the default API (backward compatibility)
pub use v1::*;
