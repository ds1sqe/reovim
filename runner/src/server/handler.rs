//! Client connection handler.
//!
//! Contains the main request/response loop for a single client connection.

use std::sync::Arc;

use {
    reovim_kernel::api::v1::ModeId,
    reovim_protocol::v1::{RpcError, RpcRequest, RpcResponse},
};

use crate::server::{
    bootstrap,
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
pub async fn handle_client(
    mut reader: TransportReader,
    writer: TransportWriter,
    client_id: crate::session::ClientId,
    session_id: SessionId,
    sessions: Arc<SessionRegistry>,
    dispatcher: Arc<RpcDispatcher>,
    _default_mode: ModeId,
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

        // Parse JSON-RPC request
        let request: RpcRequest = match serde_json::from_str(line) {
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
