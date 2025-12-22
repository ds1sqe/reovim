//! RPC handler trait and registry for plugin-based RPC method handling
//!
//! This module provides the infrastructure for plugins to register
//! custom RPC method handlers without modifying the core event loop.
//!
//! # Example
//!
//! ```ignore
//! use reovim_core::rpc::{RpcHandler, RpcHandlerContext, RpcResult};
//! use serde_json::Value;
//!
//! struct MyPluginHandler;
//!
//! impl RpcHandler for MyPluginHandler {
//!     fn method(&self) -> &'static str {
//!         "my_plugin/custom_method"
//!     }
//!
//!     fn handle(&self, params: &Value, ctx: &mut RpcHandlerContext) -> RpcResult {
//!         // Handle the RPC request
//!         RpcResult::Success(serde_json::json!({ "status": "ok" }))
//!     }
//! }
//! ```

use std::{collections::HashMap, fmt::Debug, sync::Arc};

use serde_json::Value;

/// Result of an RPC handler execution
#[derive(Debug)]
pub enum RpcResult {
    /// Successful response with a JSON value
    Success(Value),
    /// Error response with code and message
    Error { code: i32, message: String },
}

impl RpcResult {
    /// Create a successful result with a JSON value
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Value is not const-constructible
    pub fn success(value: Value) -> Self {
        Self::Success(value)
    }

    /// Create an "ok" success result (empty success)
    #[must_use]
    pub fn ok() -> Self {
        Self::Success(serde_json::json!({ "ok": true }))
    }

    /// Create an error result
    #[must_use]
    pub fn error(code: i32, message: impl Into<String>) -> Self {
        Self::Error {
            code,
            message: message.into(),
        }
    }

    /// Create an invalid params error
    #[must_use]
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::Error {
            code: -32602,
            message: message.into(),
        }
    }

    /// Create an internal error
    #[must_use]
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::Error {
            code: -32603,
            message: message.into(),
        }
    }
}

/// Context provided to RPC handlers for accessing runtime state
///
/// This provides read-only access to runtime state. Handlers that need
/// to modify state should return an action or use the event bus.
pub struct RpcHandlerContext<'a> {
    /// Access to plugin state registry (read-only)
    plugin_state: &'a crate::plugin::PluginStateRegistry,
    /// Current mode state
    mode_state: &'a crate::modd::ModeState,
    /// Active buffer ID
    active_buffer_id: usize,
}

impl<'a> RpcHandlerContext<'a> {
    /// Create a new RPC handler context
    #[must_use]
    pub const fn new(
        plugin_state: &'a crate::plugin::PluginStateRegistry,
        mode_state: &'a crate::modd::ModeState,
        active_buffer_id: usize,
    ) -> Self {
        Self {
            plugin_state,
            mode_state,
            active_buffer_id,
        }
    }

    /// Get access to the plugin state registry
    #[must_use]
    pub const fn plugin_state(&self) -> &crate::plugin::PluginStateRegistry {
        self.plugin_state
    }

    /// Get the current mode state
    #[must_use]
    pub const fn mode_state(&self) -> &crate::modd::ModeState {
        self.mode_state
    }

    /// Get the active buffer ID
    #[must_use]
    pub const fn active_buffer_id(&self) -> usize {
        self.active_buffer_id
    }

    /// Access plugin state by type
    pub fn with_state<S, F, R>(&self, f: F) -> Option<R>
    where
        S: Send + Sync + 'static,
        F: FnOnce(&S) -> R,
    {
        self.plugin_state.with::<S, _, _>(f)
    }
}

/// Trait for RPC method handlers
///
/// Plugins implement this trait to handle custom RPC methods.
/// Handlers are registered with the `RpcHandlerRegistry` and are
/// invoked when matching RPC requests are received.
pub trait RpcHandler: Send + Sync {
    /// The RPC method name this handler responds to
    ///
    /// Convention: Use namespaced methods like `"plugin_name/method_name"`
    fn method(&self) -> &'static str;

    /// Handle an RPC request
    ///
    /// # Arguments
    /// * `params` - The JSON parameters from the RPC request
    /// * `ctx` - Context for accessing runtime state
    ///
    /// # Returns
    /// The result of handling the request
    fn handle(&self, params: &Value, ctx: &RpcHandlerContext) -> RpcResult;

    /// Human-readable description of this handler (for introspection)
    fn description(&self) -> &'static str {
        "No description available"
    }
}

impl Debug for dyn RpcHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RpcHandler({})", self.method())
    }
}

/// Registry for RPC handlers
///
/// Stores registered handlers and dispatches RPC requests to them.
/// Used by the runtime to extend RPC functionality via plugins.
#[derive(Default)]
pub struct RpcHandlerRegistry {
    handlers: HashMap<&'static str, Arc<dyn RpcHandler>>,
}

impl RpcHandlerRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an RPC handler
    ///
    /// If a handler for the same method already exists, it will be replaced.
    pub fn register(&mut self, handler: Arc<dyn RpcHandler>) {
        let method = handler.method();
        tracing::debug!("Registering RPC handler for method: {}", method);
        self.handlers.insert(method, handler);
    }

    /// Unregister an RPC handler by method name
    ///
    /// Returns true if a handler was removed.
    pub fn unregister(&mut self, method: &str) -> bool {
        self.handlers.remove(method).is_some()
    }

    /// Check if a handler is registered for a method
    #[must_use]
    pub fn has_handler(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }

    /// Get a handler for a method
    #[must_use]
    pub fn get(&self, method: &str) -> Option<&Arc<dyn RpcHandler>> {
        self.handlers.get(method)
    }

    /// Dispatch an RPC request to the appropriate handler
    ///
    /// Returns `None` if no handler is registered for the method.
    #[must_use]
    pub fn dispatch(
        &self,
        method: &str,
        params: &Value,
        ctx: &RpcHandlerContext,
    ) -> Option<RpcResult> {
        self.handlers
            .get(method)
            .map(|handler| handler.handle(params, ctx))
    }

    /// List all registered methods (for introspection)
    #[must_use]
    pub fn methods(&self) -> Vec<&'static str> {
        self.handlers.keys().copied().collect()
    }

    /// Get method info for all handlers (for introspection)
    #[must_use]
    pub fn method_info(&self) -> Vec<(&'static str, &'static str)> {
        self.handlers
            .values()
            .map(|h| (h.method(), h.description()))
            .collect()
    }
}

impl Debug for RpcHandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcHandlerRegistry")
            .field("handler_count", &self.handlers.len())
            .field("methods", &self.methods())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHandler;

    impl RpcHandler for TestHandler {
        fn method(&self) -> &'static str {
            "test/echo"
        }

        fn handle(&self, params: &Value, _ctx: &RpcHandlerContext) -> RpcResult {
            RpcResult::Success(params.clone())
        }

        fn description(&self) -> &'static str {
            "Echo back the params"
        }
    }

    #[test]
    fn test_registry_register_and_dispatch() {
        let mut registry = RpcHandlerRegistry::new();
        registry.register(Arc::new(TestHandler));

        assert!(registry.has_handler("test/echo"));
        assert!(!registry.has_handler("test/unknown"));

        let methods = registry.methods();
        assert!(methods.contains(&"test/echo"));
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = RpcHandlerRegistry::new();
        registry.register(Arc::new(TestHandler));

        assert!(registry.has_handler("test/echo"));
        assert!(registry.unregister("test/echo"));
        assert!(!registry.has_handler("test/echo"));
        assert!(!registry.unregister("test/echo")); // Already removed
    }

    #[test]
    fn test_rpc_result_variants() {
        let success = RpcResult::success(serde_json::json!({ "value": 42 }));
        assert!(matches!(success, RpcResult::Success(_)));

        let ok = RpcResult::ok();
        assert!(matches!(ok, RpcResult::Success(_)));

        let error = RpcResult::error(-1, "test error");
        assert!(matches!(error, RpcResult::Error { code: -1, .. }));

        let invalid = RpcResult::invalid_params("missing field");
        assert!(matches!(invalid, RpcResult::Error { code: -32602, .. }));

        let internal = RpcResult::internal_error("something went wrong");
        assert!(matches!(internal, RpcResult::Error { code: -32603, .. }));
    }
}
