//! Network driver for reovim.
//!
//! Linux equivalent: `net/`, `include/linux/net.h`
//!
//! # Architecture
//!
//! This crate defines types and traits for RPC server infrastructure.
//! Concrete implementations in `lib/core/src/rpc/` import from this driver.
//!
//! ```text
//! lib/drivers/net/          <-- Types + Traits (this crate)
//!        ^
//!        |  imports from
//!        |
//! lib/core/src/rpc/         <-- Implementations
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
//! use reovim_driver_net::{
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

mod codes;
mod error;
mod handler;
mod rpc;
mod traits;
pub mod transport;

// ============================================================================
// Re-exports
// ============================================================================

// Error types
pub use error::NetError;

// JSON-RPC error codes
pub use codes::{
    BUFFER_NOT_FOUND, COMMAND_NOT_FOUND, INTERNAL_ERROR, INVALID_KEY_NOTATION, INVALID_PARAMS,
    INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR,
};

// Transport configuration
pub use transport::TransportConfig;

// RPC message types
pub use rpc::{RpcError, RpcNotification, RpcRequest, RpcResponse};

// Handler types
pub use handler::{RpcHandler, RpcHandlerContext, RpcResult};

// Core traits
pub use traits::{NetDriver, PortAllocator, TransportConnection, TransportListener};

// Method name constants (for discoverability)
pub mod methods {
    //! Standard RPC method names.
    pub use crate::rpc::methods::*;
}

// Notification name constants
pub mod notifications {
    //! Standard notification names.
    pub use crate::rpc::notifications::*;
}
