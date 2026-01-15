//! Editor-level RPC handlers (application lifecycle, display).
//!
//! Handlers for `editor/resize`, `editor/quit`, and related methods.

use {reovim_protocol::v1::RpcError, serde::Deserialize};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Parameters for `editor/resize` method.
#[derive(Debug, Deserialize)]
pub struct EditorResizeParams {
    /// New terminal width in columns.
    pub width: u16,
    /// New terminal height in rows.
    pub height: u16,
}

/// Handler for `editor/resize` method.
///
/// Updates the terminal dimensions in the application state.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "editor/resize", "params": {"width": 120, "height": 40}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}
/// ```
///
/// # Errors
///
/// Returns `invalid_params` if width or height is zero.
#[must_use]
pub fn editor_resize(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: EditorResizeParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        // Validate dimensions
        if params.width == 0 || params.height == 0 {
            return Err(RpcError::invalid_params("Width and height must be positive"));
        }

        ctx.session
            .with_state_mut(|state| {
                state.app.terminal_width = params.width;
                state.app.terminal_height = params.height;
            })
            .await;

        Ok(serde_json::json!({"ok": true}))
    })
}

/// Handler for `editor/quit` method.
///
/// Requests a graceful shutdown by setting the running flag to false.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "editor/quit", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}
/// ```
#[must_use]
pub fn editor_quit(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        ctx.session
            .with_state_mut(|state| {
                state.app.running = false;
            })
            .await;

        Ok(serde_json::json!({"ok": true}))
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::session::{ClientId, Session, SessionId},
        reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId},
        std::sync::Arc,
    };

    fn test_session() -> Arc<Session> {
        Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            ModeId::new(ModuleId::new("test"), "normal"),
        )
    }

    #[tokio::test]
    async fn test_editor_resize_success() {
        let session = test_session();
        let ctx = RpcContext {
            session: Arc::clone(&session),
            client_id: ClientId::new(1),
        };

        let result = editor_resize(ctx, serde_json::json!({"width": 120, "height": 40})).await;
        assert!(result.is_ok());

        // Verify dimensions were updated
        let (width, height) = session
            .with_state(|state| (state.app.terminal_width, state.app.terminal_height))
            .await;
        assert_eq!(width, 120);
        assert_eq!(height, 40);
    }

    #[tokio::test]
    async fn test_editor_resize_zero_width() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let result = editor_resize(ctx, serde_json::json!({"width": 0, "height": 24})).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("positive"));
    }

    #[tokio::test]
    async fn test_editor_resize_zero_height() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let result = editor_resize(ctx, serde_json::json!({"width": 80, "height": 0})).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("positive"));
    }

    #[tokio::test]
    async fn test_editor_resize_invalid_params() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let result = editor_resize(ctx, serde_json::json!({"invalid": "params"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_editor_resize_boundary_values() {
        let session = test_session();
        let ctx = RpcContext {
            session: Arc::clone(&session),
            client_id: ClientId::new(1),
        };

        // Test max u16 values
        let result =
            editor_resize(ctx, serde_json::json!({"width": u16::MAX, "height": u16::MAX})).await;
        assert!(result.is_ok());

        let (width, height) = session
            .with_state(|state| (state.app.terminal_width, state.app.terminal_height))
            .await;
        assert_eq!(width, u16::MAX);
        assert_eq!(height, u16::MAX);
    }

    #[tokio::test]
    async fn test_editor_quit_sets_running_false() {
        let session = test_session();

        // Verify initial state
        let running = session.with_state(|state| state.app.running).await;
        assert!(running);

        let ctx = RpcContext {
            session: Arc::clone(&session),
            client_id: ClientId::new(1),
        };

        let result = editor_quit(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        // Verify running flag is now false
        let running = session.with_state(|state| state.app.running).await;
        assert!(!running);
    }

    #[tokio::test]
    async fn test_editor_resize_persistence() {
        let session = test_session();

        // Resize multiple times
        let ctx1 = RpcContext {
            session: Arc::clone(&session),
            client_id: ClientId::new(1),
        };
        let _ = editor_resize(ctx1, serde_json::json!({"width": 100, "height": 30})).await;

        let ctx2 = RpcContext {
            session: Arc::clone(&session),
            client_id: ClientId::new(1),
        };
        let _ = editor_resize(ctx2, serde_json::json!({"width": 150, "height": 45})).await;

        // Verify only the final state persists
        let (width, height) = session
            .with_state(|state| (state.app.terminal_width, state.app.terminal_height))
            .await;
        assert_eq!(width, 150);
        assert_eq!(height, 45);
    }

    #[tokio::test]
    async fn test_editor_quit_returns_ok() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let result = editor_quit(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(
            value
                .get("ok")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        );
    }
}
