//! TUI-related RPC handlers.
//!
//! Handler for `tui/capture` method that relays capture requests to TUI clients.
//!
//! # Capture Flow
//!
//! ```text
//! CLI                    Server                     TUI
//!  |                       |                         |
//!  |-- tui/capture ------->|                         |
//!  |                       |-- tui/capture-request ->|
//!  |                       |<- tui/capture-response -|
//!  |<----- response -------|                         |
//! ```

use reovim_protocol::v1::{
    RpcError, StateScreenContentParams,
    notifications::{CAPTURE_REQUEST, CaptureRequestPayload},
};

use {
    super::super::dispatcher::{HandlerFuture, RpcContext},
    crate::server::capture::{CaptureError, CaptureTracker, wait_for_capture},
};

/// Handler for `tui/capture` method.
///
/// Relays capture requests to a connected TUI client and waits for the response.
/// The TUI client must be running (interactive or headless mode) to respond.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "tui/capture", "params": {"format": "raw_ansi"}}
/// ```
///
/// # Response (success)
///
/// ```json
/// {
///     "jsonrpc": "2.0",
///     "id": 1,
///     "result": {
///         "width": 80,
///         "height": 24,
///         "format": "raw_ansi",
///         "content": "=== FRAME CAPTURE ===\n..."
///     }
/// }
/// ```
///
/// # Response (error)
///
/// - No TUI client connected: `{"error": {"code": -32000, "message": "No TUI client connected..."}}`
/// - Timeout: `{"error": {"code": -32000, "message": "TUI capture timeout..."}}`
///
/// # Panics
///
/// This function will not panic as `ScreenContentResult` serialization is infallible.
#[must_use]
pub fn tui_capture(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: StateScreenContentParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        // Get capture tracker from session ServiceRegistry
        let tracker = ctx
            .session
            .with_state(|state| state.app.services.get::<CaptureTracker>())
            .await;

        let Some(tracker) = tracker else {
            return Err(RpcError::internal_error("Capture tracker not initialized"));
        };

        // Find a TUI client (any client other than the requesting one)
        // In the future, we could track client capabilities explicitly
        let tui_client = find_tui_client(&ctx);

        let Some(tui_client) = tui_client else {
            let err = CaptureError::NoTuiClient;
            return Err(RpcError::internal_error(err.to_string()));
        };

        // Create pending request
        let (request_id, rx) = tracker.create_pending();

        // Build capture request notification
        let request_payload = CaptureRequestPayload {
            request_id,
            format: params.format,
        };
        let notification = reovim_protocol::v1::RpcNotification::new(
            CAPTURE_REQUEST,
            serde_json::to_value(&request_payload)
                .expect("CaptureRequestPayload serialization cannot fail"),
        );
        let notification_json = serde_json::to_string(&notification)
            .expect("RpcNotification serialization cannot fail");

        // Send capture request to TUI
        tracing::info!(
            "Sending capture request {} to TUI client {:?}",
            request_id,
            tui_client.id()
        );
        if let Err(e) = tui_client.send_line(&notification_json).await {
            tracker.cancel(request_id);
            return Err(RpcError::internal_error(format!("Failed to send capture request: {e}")));
        }

        // Wait for response
        match wait_for_capture(rx).await {
            Ok(result) => {
                tracing::info!("Capture {} completed", request_id);
                Ok(serde_json::to_value(result)
                    .expect("ScreenContentResult serialization cannot fail"))
            }
            Err(e) => {
                tracker.cancel(request_id);
                tracing::warn!("Capture {} failed: {}", request_id, e);
                Err(RpcError::internal_error(e.to_string()))
            }
        }
    })
}

/// Find a TUI client that can handle capture requests.
///
/// Returns the client with the lowest ID that is not the requesting client.
/// This works because TUI clients typically connect before CLI clients,
/// so they have lower IDs.
///
/// In the future, this could use explicit client capabilities to identify TUI clients.
fn find_tui_client(ctx: &RpcContext) -> Option<std::sync::Arc<crate::server::client::Client>> {
    // Find the client with the lowest ID that is not the requesting client
    // TUI clients connect first, so they have lower IDs than CLI clients
    ctx.session
        .clients()
        .iter()
        .filter(|client| client.id() != ctx.client_id)
        .min_by_key(|client| client.id().as_usize())
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_protocol::v1::ScreenFormat};

    #[test]
    fn test_capture_request_payload_serialization() {
        let payload = CaptureRequestPayload {
            request_id: 42,
            format: ScreenFormat::RawAnsi,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("42"));
        assert!(json.contains("raw_ansi"));
    }
}
