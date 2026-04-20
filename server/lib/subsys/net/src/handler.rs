//! RPC handler trait and types.
//!
//! This module provides the infrastructure for plugins to register
//! custom RPC method handlers.

use std::fmt::Debug;

use {reovim_kernel::api::v1::BufferId, serde_json::Value};

/// Result of an RPC handler execution.
#[derive(Debug)]
pub enum RpcResult {
    /// Successful response with a JSON value.
    Success(Value),
    /// Error response with code and message.
    Error {
        /// Error code.
        code: i32,
        /// Error message.
        message: String,
    },
}

impl RpcResult {
    /// Create a successful result with a JSON value.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Value is not const-constructible
    pub fn success(value: Value) -> Self {
        Self::Success(value)
    }

    /// Create an "ok" success result (empty success).
    #[must_use]
    pub fn ok() -> Self {
        Self::Success(serde_json::json!({ "ok": true }))
    }

    /// Create an error result.
    #[must_use]
    pub fn error(code: i32, message: impl Into<String>) -> Self {
        Self::Error {
            code,
            message: message.into(),
        }
    }

    /// Create an invalid params error.
    #[must_use]
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::Error {
            code: crate::INVALID_PARAMS,
            message: message.into(),
        }
    }

    /// Create an internal error.
    #[must_use]
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::Error {
            code: crate::INTERNAL_ERROR,
            message: message.into(),
        }
    }
}

/// Context provided to RPC handlers for accessing runtime state.
///
/// This is the context struct for RPC handlers. It provides
/// access to runtime state needed for handler execution.
pub struct RpcHandlerContext {
    /// Active buffer ID.
    pub active_buffer_id: BufferId,
}

impl RpcHandlerContext {
    /// Create a new minimal context.
    #[must_use]
    pub const fn new(active_buffer_id: BufferId) -> Self {
        Self { active_buffer_id }
    }

    /// Get the active buffer ID.
    #[must_use]
    pub const fn active_buffer_id(&self) -> BufferId {
        self.active_buffer_id
    }
}

/// Trait for RPC method handlers.
///
/// Plugins implement this trait to handle custom RPC methods.
/// Handlers are registered with the runtime and invoked when
/// matching RPC requests are received.
///
/// # Example
///
/// ```ignore
/// use reovim_subsys_net::{RpcHandler, RpcHandlerContext, RpcResult};
/// use serde_json::Value;
///
/// struct EchoHandler;
///
/// impl RpcHandler for EchoHandler {
///     fn method(&self) -> &'static str {
///         "echo"
///     }
///
///     fn handle(&self, params: &Value, _ctx: &RpcHandlerContext) -> RpcResult {
///         RpcResult::Success(params.clone())
///     }
/// }
/// ```
pub trait RpcHandler: Send + Sync {
    /// The RPC method name this handler responds to.
    ///
    /// Convention: Use namespaced methods like `"plugin_name/method_name"`.
    fn method(&self) -> &'static str;

    /// Handle an RPC request.
    ///
    /// # Arguments
    /// * `params` - The JSON parameters from the RPC request
    /// * `ctx` - Context for accessing runtime state
    ///
    /// # Returns
    /// The result of handling the request.
    fn handle(&self, params: &Value, ctx: &RpcHandlerContext) -> RpcResult;

    /// Human-readable description of this handler (for introspection).
    fn description(&self) -> &'static str {
        "No description available"
    }
}

impl Debug for dyn RpcHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RpcHandler({})", self.method())
    }
}

#[cfg(test)]
#[path = "handler_tests.rs"]
mod tests;
