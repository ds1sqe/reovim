//! RPC method dispatcher.
//!
//! Routes incoming JSON-RPC requests to the appropriate handlers.

use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use reovim_protocol::v1::{RpcError, RpcRequest, RpcResponse};

use crate::session::Session;

/// Context passed to RPC handlers.
#[derive(Clone)]
pub struct RpcContext {
    /// The session this request is for.
    pub session: Arc<Session>,
}

/// Result type for RPC handlers.
pub type RpcResult = Result<serde_json::Value, RpcError>;

/// Boxed future type for async handlers.
/// Using 'static lifetime to avoid dyn-compatibility issues.
pub type HandlerFuture = Pin<Box<dyn Future<Output = RpcResult> + Send>>;

/// Handler function type.
/// Takes context and params, returns a boxed future.
pub type HandlerFn = fn(RpcContext, serde_json::Value) -> HandlerFuture;

/// RPC method dispatcher.
///
/// Routes requests to registered handlers by method name.
/// Uses function pointers for simple, lock-free dispatch.
///
/// # Example
///
/// ```ignore
/// use runner::rpc::{RpcDispatcher, RpcContext, RpcResult, HandlerFuture};
///
/// fn my_handler(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
///     Box::pin(async move {
///         Ok(serde_json::json!({"ok": true}))
///     })
/// }
///
/// let mut dispatcher = RpcDispatcher::new();
/// dispatcher.register("my/method", my_handler);
/// ```
pub struct RpcDispatcher {
    handlers: HashMap<&'static str, HandlerFn>,
}

impl RpcDispatcher {
    /// Create a new empty dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for a method.
    pub fn register(&mut self, method: &'static str, handler: HandlerFn) {
        self.handlers.insert(method, handler);
    }

    /// Dispatch an RPC request to the appropriate handler.
    ///
    /// Returns a response for requests with IDs, or `None` for notifications.
    pub async fn dispatch(&self, request: RpcRequest, ctx: &RpcContext) -> Option<RpcResponse> {
        // Notifications don't get responses
        let request_id = request.id?;

        let result = if let Some(handler) = self.handlers.get(request.method.as_str()) {
            handler(ctx.clone(), request.params).await
        } else {
            Err(RpcError::method_not_found(&request.method))
        };

        Some(match result {
            Ok(value) => RpcResponse::success(request_id, value),
            Err(error) => RpcResponse::error(request_id, error),
        })
    }

    /// Check if a method is registered.
    #[must_use]
    pub fn has_method(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }

    /// Get the number of registered methods.
    #[must_use]
    pub fn method_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for RpcDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for RpcDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcDispatcher")
            .field("methods", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId},
    };

    use crate::session::{Session, SessionId};

    fn echo_handler(_ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
        Box::pin(async move { Ok(params) })
    }

    fn error_handler(_ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
        Box::pin(async move { Err(RpcError::internal_error("test error")) })
    }

    fn test_session() -> Arc<Session> {
        Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            ModeId::new(ModuleId::new("test"), "normal"),
        )
    }

    #[tokio::test]
    async fn test_dispatcher_dispatch_success() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("echo", echo_handler);

        let session = test_session();
        let ctx = RpcContext { session };

        let request = RpcRequest::new(1, "echo", serde_json::json!({"key": "value"}));
        let response = dispatcher.dispatch(request, &ctx).await.unwrap();

        assert!(response.result.is_some());
        assert!(response.error.is_none());
        assert_eq!(response.result.unwrap(), serde_json::json!({"key": "value"}));
    }

    #[tokio::test]
    async fn test_dispatcher_dispatch_error() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("error", error_handler);

        let session = test_session();
        let ctx = RpcContext { session };

        let request = RpcRequest::new(1, "error", serde_json::json!({}));
        let response = dispatcher.dispatch(request, &ctx).await.unwrap();

        assert!(response.error.is_some());
        assert!(response.result.is_none());
    }

    #[tokio::test]
    async fn test_dispatcher_method_not_found() {
        let dispatcher = RpcDispatcher::new();
        let session = test_session();
        let ctx = RpcContext { session };

        let request = RpcRequest::new(1, "unknown", serde_json::json!({}));
        let response = dispatcher.dispatch(request, &ctx).await.unwrap();

        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert!(error.message.contains("Method not found"));
    }

    #[tokio::test]
    async fn test_dispatcher_notification_no_response() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("notify", echo_handler);

        let session = test_session();
        let ctx = RpcContext { session };

        let request = RpcRequest::notification("notify", serde_json::json!({}));
        let response = dispatcher.dispatch(request, &ctx).await;

        assert!(response.is_none());
    }

    #[test]
    fn test_dispatcher_has_method() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("test", echo_handler);

        assert!(dispatcher.has_method("test"));
        assert!(!dispatcher.has_method("unknown"));
    }
}
