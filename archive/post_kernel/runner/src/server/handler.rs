//! Client connection handler.
//!
//! Contains the main request/response loop for a single client connection.

use std::sync::Arc;

use {
    reovim_kernel::api::v1::ModeId,
    reovim_protocol::v1::{
        RpcError, RpcNotification, RpcRequest, RpcResponse,
        notifications::{CAPTURE_RESPONSE, CaptureResponsePayload},
    },
    tokio::sync::watch,
};

use crate::server::{
    bootstrap,
    capture::CaptureTracker,
    client::Client,
    rpc::{RpcContext, RpcDispatcher},
    session::{SessionId, SessionRegistry},
    transport::{TransportReader, TransportWriter},
};

/// Handle a single client connection.
///
/// Reads JSON-RPC requests line by line, dispatches them, and sends responses.
/// Runs until the client disconnects or an error occurs.
///
/// # Arguments
///
/// * `reader` - Transport reader for receiving requests
/// * `writer` - Transport writer for sending responses
/// * `client_id` - Unique identifier for this client
/// * `session_id` - Session to attach the client to
/// * `sessions` - Session registry for looking up sessions
/// * `dispatcher` - RPC dispatcher for handling requests
/// * `default_mode` - Default mode ID for new sessions
/// * `shutdown_tx` - Watch channel sender to trigger server shutdown (for `server/kill`)
#[allow(clippy::too_many_arguments)]
pub async fn handle_client(
    mut reader: TransportReader,
    writer: TransportWriter,
    client_id: crate::session::ClientId,
    session_id: SessionId,
    sessions: Arc<SessionRegistry>,
    dispatcher: Arc<RpcDispatcher>,
    _default_mode: ModeId,
    shutdown_tx: watch::Sender<bool>,
) -> std::io::Result<()> {
    // Get or create the session with default registries (keybindings wired)
    let session = sessions
        .get_or_create(&session_id, || bootstrap::create_session_with_defaults(session_id.clone()));

    // Create the client (owns the writer)
    let client = Client::new(client_id, session_id, writer);

    // Register client with session for notifications
    session.clients().insert(&client);

    // Create RPC context
    let ctx = RpcContext {
        session: Arc::clone(&session),
        client_id,
        client: Arc::clone(&client),
        shutdown_tx,
    };

    // Read loop
    loop {
        let Some(line) = reader.read_line().await? else {
            // EOF - client disconnected
            break;
        };

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try to determine if this is a request (has id) or notification (no id)
        // by checking for the "id" field in the JSON
        let parsed: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                // Send parse error response
                let error_response = RpcResponse::error(0, RpcError::parse_error());
                let response_json = serde_json::to_string(&error_response)
                    .expect("RpcResponse serialization should never fail");
                client.send_line(&response_json).await?;
                tracing::warn!("Parse error from client {client_id:?}: {e}");
                continue;
            }
        };

        // Check if this is a notification (no "id" field)
        if parsed.get("id").is_none() {
            // This is a notification
            if let Ok(notification) = serde_json::from_value::<RpcNotification>(parsed) {
                handle_notification(&session, &notification).await;
            }
            continue;
        }

        // Parse as RPC request
        let request: RpcRequest = match serde_json::from_value(parsed) {
            Ok(req) => req,
            Err(e) => {
                // Send parse error response
                let error_response = RpcResponse::error(0, RpcError::parse_error());
                let response_json = serde_json::to_string(&error_response)
                    .expect("RpcResponse serialization should never fail");
                client.send_line(&response_json).await?;
                tracing::warn!("Parse error from client {client_id:?}: {e}");
                continue;
            }
        };

        tracing::debug!("Request from {client_id:?}: {} (id={:?})", request.method, request.id);

        // Dispatch to handler
        if let Some(response) = dispatcher.dispatch(request, &ctx).await {
            let response_json = serde_json::to_string(&response)
                .expect("RpcResponse serialization should never fail");
            client.send_line(&response_json).await?;
        }
    }

    // Cleanup: remove client from session
    session.clients().remove(&client_id);

    Ok(())
}

/// Handle incoming notifications from clients.
///
/// Currently handles:
/// - `tui/capture-response` - Response from TUI client with captured frame (#447)
async fn handle_notification(
    session: &std::sync::Arc<crate::server::session::Session>,
    notification: &RpcNotification,
) {
    match notification.method.as_str() {
        CAPTURE_RESPONSE => {
            // Parse capture response payload
            if let Ok(payload) =
                serde_json::from_value::<CaptureResponsePayload>(notification.params.clone())
            {
                // Deliver to capture tracker
                let delivered = session
                    .with_state(|state| {
                        state.app.services.get::<CaptureTracker>().map_or_else(
                            || {
                                tracing::warn!(
                                    "Received capture response but no CaptureTracker registered"
                                );
                                false
                            },
                            |tracker| tracker.deliver_response(payload),
                        )
                    })
                    .await;

                if !delivered {
                    tracing::warn!("Capture response not delivered (no matching request)");
                }
            } else {
                tracing::warn!("Failed to parse capture response payload");
            }
        }
        _ => {
            // Ignore unknown notifications
            tracing::debug!("Received unknown notification: {}", notification.method);
        }
    }
}
