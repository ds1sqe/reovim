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
/// use reovim_driver_net::{RpcHandler, RpcHandlerContext, RpcResult};
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

    #[test]
    fn test_handler_trait() {
        let handler = TestHandler;
        assert_eq!(handler.method(), "test/echo");
        assert_eq!(handler.description(), "Echo back the params");

        let ctx = RpcHandlerContext::new(BufferId::from_raw(0));
        let result = handler.handle(&serde_json::json!({"test": 1}), &ctx);
        assert!(matches!(result, RpcResult::Success(_)));
    }

    #[test]
    fn test_handler_debug() {
        let handler: &dyn RpcHandler = &TestHandler;
        let debug = format!("{handler:?}");
        assert!(debug.contains("test/echo"));
    }

    #[test]
    fn test_handler_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TestHandler>();
    }

    #[test]
    fn test_rpc_handler_context_new() {
        let ctx = RpcHandlerContext::new(BufferId::from_raw(42));
        assert_eq!(ctx.active_buffer_id(), BufferId::from_raw(42));
    }

    #[test]
    fn test_rpc_handler_context_active_buffer_id() {
        let ctx = RpcHandlerContext::new(BufferId::from_raw(0));
        assert_eq!(ctx.active_buffer_id(), BufferId::from_raw(0));

        let ctx2 = RpcHandlerContext::new(BufferId::from_raw(99));
        assert_eq!(ctx2.active_buffer_id(), BufferId::from_raw(99));
    }

    #[test]
    fn test_handler_default_description() {
        struct NoDescHandler;
        impl RpcHandler for NoDescHandler {
            fn method(&self) -> &'static str {
                "test/no_desc"
            }
            fn handle(&self, _params: &Value, _ctx: &RpcHandlerContext) -> RpcResult {
                RpcResult::ok()
            }
            // description() not overridden - uses default
        }

        let handler = NoDescHandler;
        assert_eq!(handler.description(), "No description available");
    }

    #[test]
    fn test_rpc_result_success_value() {
        let result = RpcResult::success(serde_json::json!({"key": "val"}));
        match result {
            RpcResult::Success(v) => assert_eq!(v["key"], "val"),
            RpcResult::Error { .. } => panic!("Expected Success"),
        }
    }

    #[test]
    fn test_rpc_result_ok_content() {
        let result = RpcResult::ok();
        match result {
            RpcResult::Success(v) => {
                assert_eq!(v["ok"], true);
            }
            RpcResult::Error { .. } => panic!("Expected Success"),
        }
    }

    #[test]
    fn test_rpc_result_error_content() {
        let result = RpcResult::error(-32000, "custom");
        match result {
            RpcResult::Error { code, message } => {
                assert_eq!(code, -32000);
                assert_eq!(message, "custom");
            }
            RpcResult::Success(_) => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_rpc_result_invalid_params_content() {
        let result = RpcResult::invalid_params("field 'x' missing");
        match result {
            RpcResult::Error { code, message } => {
                assert_eq!(code, crate::INVALID_PARAMS);
                assert_eq!(message, "field 'x' missing");
            }
            RpcResult::Success(_) => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_rpc_result_internal_error_content() {
        let result = RpcResult::internal_error("panic in handler");
        match result {
            RpcResult::Error { code, message } => {
                assert_eq!(code, crate::INTERNAL_ERROR);
                assert_eq!(message, "panic in handler");
            }
            RpcResult::Success(_) => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_rpc_result_debug() {
        let success = RpcResult::success(serde_json::json!(1));
        let debug_str = format!("{success:?}");
        assert!(debug_str.contains("Success"));

        let error = RpcResult::error(-1, "err");
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("Error"));
    }

    #[test]
    fn test_handler_handle_returns_echo() {
        let handler = TestHandler;
        let ctx = RpcHandlerContext::new(BufferId::from_raw(0));
        let params = serde_json::json!({"echo": "test"});
        let result = handler.handle(&params, &ctx);
        match result {
            RpcResult::Success(v) => assert_eq!(v["echo"], "test"),
            RpcResult::Error { .. } => panic!("Expected Success"),
        }
    }

    #[test]
    fn test_rpc_result_error_string_conversion() {
        // Test Into<String> conversion for error message
        let result = RpcResult::error(-100, String::from("owned string"));
        match result {
            RpcResult::Error { code, message } => {
                assert_eq!(code, -100);
                assert_eq!(message, "owned string");
            }
            RpcResult::Success(_) => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_rpc_result_invalid_params_string_conversion() {
        let result = RpcResult::invalid_params(String::from("owned message"));
        match result {
            RpcResult::Error { code, message } => {
                assert_eq!(code, crate::INVALID_PARAMS);
                assert_eq!(message, "owned message");
            }
            RpcResult::Success(_) => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_rpc_result_internal_error_string_conversion() {
        let result = RpcResult::internal_error(String::from("owned internal"));
        match result {
            RpcResult::Error { code, message } => {
                assert_eq!(code, crate::INTERNAL_ERROR);
                assert_eq!(message, "owned internal");
            }
            RpcResult::Success(_) => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_handler_debug_format() {
        let handler: &dyn RpcHandler = &TestHandler;
        let debug = format!("{handler:?}");
        assert_eq!(debug, "RpcHandler(test/echo)");
    }

    #[test]
    fn test_rpc_handler_context_field_access() {
        let buf_id = BufferId::from_raw(12345);
        let ctx = RpcHandlerContext::new(buf_id);
        assert_eq!(ctx.active_buffer_id, buf_id);
        assert_eq!(ctx.active_buffer_id(), buf_id);
    }

    #[test]
    fn test_rpc_result_success_with_null() {
        let result = RpcResult::success(serde_json::Value::Null);
        match result {
            RpcResult::Success(v) => assert!(v.is_null()),
            RpcResult::Error { .. } => panic!("Expected Success"),
        }
    }

    #[test]
    fn test_rpc_result_success_with_array() {
        let result = RpcResult::success(serde_json::json!([1, 2, 3]));
        match result {
            RpcResult::Success(v) => {
                assert!(v.is_array());
                assert_eq!(v.as_array().unwrap().len(), 3);
            }
            RpcResult::Error { .. } => panic!("Expected Success"),
        }
    }
}
