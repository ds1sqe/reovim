#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Network subsystem contracts for reovim.
//!
//! Linux equivalent: `net/`, `include/linux/net.h`
//!
//! # Architecture
//!
//! This crate defines subsys-level trait contracts for RPC transport
//! infrastructure and re-exports shared RPC types from `shared/protocol/`.
//! Driver implementations (e.g., gRPC via tonic) live in the drivers layer
//! and are tracked as a separate follow-on plan.
//!
//! ```text
//! shared/protocol/              <-- Shared RPC types (messages, params, results)
//!        ^
//!        |  re-exports from
//!        |
//! server/lib/subsys/net/        <-- Traits + re-exports (this crate)
//!        ^
//!        |  implemented by
//!        |
//! ext/server/drivers/net-*/     <-- Transport driver implementations (future)
//! ```
//!
//! # Components
//!
//! ## Core Traits
//!
//! - [`NetDriver`] - Network driver lifecycle management
//! - [`TransportListener`] - Bind and accept connections
//! - [`TransportConnection`] - Read/write over a transport
//! - [`PortAllocator`] - Port allocation for multi-instance support
//!
//! ## RPC Types
//!
//! - [`RpcRequest`], [`RpcResponse`], [`RpcNotification`] - JSON-RPC 2.0 messages
//! - [`RpcError`] - Error responses with standard codes
//! - [`RpcHandler`] - Plugin-based RPC method handlers
//! - [`RpcResult`] - Handler return type
//! - [`TransportConfig`] - Transport configuration enum
//!
//! ## Error Handling
//!
//! - [`NetError`] - Network operation errors
//! - Error code constants: [`PARSE_ERROR`], [`INVALID_REQUEST`], etc.
//!
//! # Example
//!
//! ```
//! use reovim_subsys_net::{
//!     TransportConfig, RpcRequest, RpcResponse, RpcError,
//!     NetError,
//! };
//!
//! // Create transport config
//! let config = TransportConfig::tcp_localhost(12521);
//!
//! // Create RPC request
//! let request = RpcRequest::new(1, "state/cursor", serde_json::json!({}));
//!
//! // Create error response
//! let error = RpcError::method_not_found("unknown");
//! ```

// ============================================================================
// Modules
// ============================================================================

mod error;
mod handler;
pub mod local;
mod traits;
pub mod transport;

// ============================================================================
// Re-exports from protocol crate (shared RPC types)
// ============================================================================

// JSON-RPC error codes
pub use reovim_protocol::v1::codes::{
    BUFFER_NOT_FOUND, COMMAND_NOT_FOUND, INTERNAL_ERROR, INVALID_KEY_NOTATION, INVALID_PARAMS,
    INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR,
};

// RPC message types
pub use reovim_protocol::v1::messages::{RpcError, RpcNotification, RpcRequest, RpcResponse};

// Method name constants (for discoverability)
pub mod methods {
    //! Standard RPC method names.
    pub use reovim_protocol::v1::methods::*;
}

// Notification name constants
pub mod notifications {
    //! Standard notification names.
    //!
    //! This module re-exports notification constants from the protocol crate.
    //! Note: The notification payload types are in `reovim_protocol::v1::notifications`.
    pub use reovim_protocol::v1::notifications::{
        BUFFER_MODIFIED, CURSOR_MOVED, MODE_CHANGED, RENDER_COMPLETE,
    };
}

// Protocol types (re-export full protocol module for advanced usage)
pub use reovim_protocol;

// ============================================================================
// Re-exports from local modules (net-specific types)
// ============================================================================

// Error types
pub use error::NetError;

// Transport configuration
pub use transport::TransportConfig;

// Local transport types
pub use local::LocalAddr;

// Handler types
pub use handler::{RpcHandler, RpcHandlerContext, RpcResult};

// Core traits
pub use traits::{NetDriver, PortAllocator, TransportConnection, TransportListener};
