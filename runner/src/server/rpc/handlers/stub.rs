//! Stub handlers for unimplemented methods.
//!
//! These handlers return explicit "not implemented" errors to make missing
//! implementations obvious. This is preferable to silently failing or
//! returning misleading responses.

use reovim_protocol::v1::RpcError;

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Create a stub handler that returns a "not implemented" error.
macro_rules! stub_handler {
    ($name:ident, $method:literal) => {
        /// Stub handler for unimplemented method.
        ///
        /// Returns an RPC error indicating the method is not yet implemented.
        #[must_use]
        pub fn $name(_ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
            Box::pin(async move {
                Err(RpcError::method_not_found(concat!(
                    "Method '",
                    $method,
                    "' is not yet implemented"
                )))
            })
        }
    };
}

// State methods (not yet implemented)
stub_handler!(state_windows, "state/windows");
stub_handler!(state_telescope, "state/telescope");
stub_handler!(state_microscope, "state/microscope");
stub_handler!(state_visual_snapshot, "state/visual_snapshot");
stub_handler!(state_ascii_art, "state/ascii_art");
stub_handler!(state_layer_info, "state/layer_info");

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
    async fn test_stub_handlers_return_error() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        // Test one stub handler to verify the pattern works
        let result = state_windows(ctx, serde_json::json!({})).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("not yet implemented"));
    }
}
