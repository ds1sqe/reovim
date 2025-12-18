//! Server mode coordinator for JSON-RPC over multiple transports
//!
//! Manages:
//! - Reading JSON-RPC requests from transport (stdio/socket/TCP)
//! - Writing responses and notifications to transport
//! - Key injection via `ChannelKeySource`
//! - Screen content capture via `DualOutput`
//!
//! Supports three transport modes:
//! - Stdio: For process piping (spawned by parent)
//! - Unix socket: For local IPC
//! - TCP: For network access

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use {
    crate::{
        event::InnerEvent,
        io::output::CaptureHandle,
        rpc::{
            RpcError, RpcNotification, RpcRequest, RpcResponse, ScreenContentSnapshot,
            ScreenFormat, keys_from_str, methods,
            transport::{TransportReader, TransportWriter},
        },
    },
    reovim_sys::event::KeyEvent,
};

/// Server mode configuration
pub struct ServerConfig {
    /// Whether to render to terminal (dual output mode)
    pub render_to_terminal: bool,
    /// Initial screen width
    pub width: u16,
    /// Initial screen height
    pub height: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            render_to_terminal: false,
            width: 80,
            height: 24,
        }
    }
}

impl ServerConfig {
    /// Create headless server config
    #[must_use]
    pub const fn headless(width: u16, height: u16) -> Self {
        Self {
            render_to_terminal: false,
            width,
            height,
        }
    }

    /// Create dual output server config (terminal + capture)
    #[must_use]
    pub const fn dual(width: u16, height: u16) -> Self {
        Self {
            render_to_terminal: true,
            width,
            height,
        }
    }
}

/// Server coordinator that bridges stdio JSON-RPC with the runtime
pub struct RpcServer {
    /// Channel to send events to runtime
    event_tx: mpsc::Sender<InnerEvent>,
    /// Channel to inject keys
    key_tx: mpsc::Sender<KeyEvent>,
    /// Handle to read captured screen output
    capture_handle: Option<CaptureHandle>,
    /// Notification output channel
    #[allow(dead_code)]
    notification_tx: mpsc::Sender<RpcNotification>,
}

impl RpcServer {
    /// Create a new RPC server
    #[must_use]
    pub const fn new(
        event_tx: mpsc::Sender<InnerEvent>,
        key_tx: mpsc::Sender<KeyEvent>,
        capture_handle: Option<CaptureHandle>,
        notification_tx: mpsc::Sender<RpcNotification>,
    ) -> Self {
        Self {
            event_tx,
            key_tx,
            capture_handle,
            notification_tx,
        }
    }

    /// Handle a single RPC request
    ///
    /// Returns the response to send back to the client.
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::missing_panics_doc)]
    pub async fn handle_request(&self, request: RpcRequest) -> Option<RpcResponse> {
        let id = request.id?; // Notification has no id, no response needed

        // Handle methods that the server manages directly
        match request.method.as_str() {
            methods::INPUT_KEYS => {
                // Parse and inject keys
                let keys_str = request
                    .params
                    .get("keys")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");

                if keys_str.is_empty() {
                    return Some(RpcResponse::error(
                        id,
                        RpcError::invalid_params("missing or empty 'keys' field"),
                    ));
                }

                let key_events = keys_from_str(keys_str);
                if key_events.is_empty() {
                    return Some(RpcResponse::error(
                        id,
                        RpcError::invalid_key_notation("no valid keys parsed"),
                    ));
                }

                // Inject keys via channel with minimal delay between each
                // to allow mode changes to propagate before next key is processed.
                // 1ms is enough for async task switching while keeping latency low.
                let mut injected = 0;
                for key_event in key_events {
                    if self.key_tx.send(key_event).await.is_ok() {
                        injected += 1;
                        // Minimal delay for runtime to process mode changes
                        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    }
                }

                Some(RpcResponse::success(id, serde_json::json!({ "injected": injected })))
            }
            methods::STATE_SCREEN_CONTENT => {
                // Return captured screen content
                let format_str = request
                    .params
                    .get("format")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("plain_text");

                let format = match format_str {
                    "raw_ansi" => ScreenFormat::RawAnsi,
                    "cell_grid" => ScreenFormat::CellGrid,
                    _ => ScreenFormat::PlainText,
                };

                let content = self
                    .capture_handle
                    .as_ref()
                    .map_or_else(String::new, |handle| match format {
                        ScreenFormat::RawAnsi => handle.to_string_lossy(),
                        ScreenFormat::PlainText | ScreenFormat::CellGrid => handle.strip_ansi(),
                    });

                // Get dimensions from runtime via event
                // For now, return placeholder - full impl needs state query
                let snapshot = ScreenContentSnapshot {
                    width: 80,  // TODO: Get from runtime
                    height: 24, // TODO: Get from runtime
                    format,
                    content,
                };

                Some(RpcResponse::success(id, serde_json::to_value(snapshot).unwrap()))
            }
            methods::SERVER_KILL => {
                // Kill the server - send KillSignal to runtime
                tracing::info!("Kill command received, shutting down server");
                let _ = self.event_tx.send(InnerEvent::KillSignal).await;
                Some(RpcResponse::success(id, serde_json::json!({ "status": "shutting_down" })))
            }
            _ => {
                // Forward to runtime via event channel
                let (response_tx, response_rx) = oneshot::channel();

                let event = InnerEvent::RpcRequest {
                    id,
                    method: request.method,
                    params: request.params,
                    response_tx,
                };

                if self.event_tx.send(event).await.is_err() {
                    return Some(RpcResponse::error(
                        id,
                        RpcError::internal_error("failed to send event to runtime"),
                    ));
                }

                // Wait for response from runtime
                response_rx.await.ok().or_else(|| {
                    Some(RpcResponse::error(
                        id,
                        RpcError::internal_error("runtime did not respond"),
                    ))
                })
            }
        }
    }
}

/// Run the transport reader task
///
/// Reads JSON-RPC requests from transport and sends them to the request channel.
pub async fn run_reader(mut reader: TransportReader, request_tx: mpsc::Sender<RpcRequest>) {
    loop {
        match reader.read_line().await {
            Ok(None) => break, // EOF
            Ok(Some(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                match serde_json::from_str::<RpcRequest>(trimmed) {
                    Ok(request) => {
                        if request_tx.send(request).await.is_err() {
                            break; // Channel closed
                        }
                    }
                    Err(e) => {
                        // Log parse error - can't send response without valid request
                        tracing::warn!("Failed to parse RPC request: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Error reading from transport: {}", e);
                break;
            }
        }
    }
}

/// Run the transport writer task
///
/// Receives responses and notifications and writes them to transport.
pub async fn run_writer(
    mut writer: TransportWriter,
    mut response_rx: mpsc::Receiver<RpcResponse>,
    mut notification_rx: mpsc::Receiver<RpcNotification>,
) {
    loop {
        tokio::select! {
            Some(response) = response_rx.recv() => {
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = writer.write_line(&json).await;
                }
            }
            Some(notification) = notification_rx.recv() => {
                if let Ok(json) = serde_json::to_string(&notification) {
                    let _ = writer.write_line(&json).await;
                }
            }
            else => break,
        }
    }
}

/// Run the main server loop
///
/// Coordinates request handling between stdin reader and runtime.
pub async fn run_server_loop(
    server: Arc<RpcServer>,
    mut request_rx: mpsc::Receiver<RpcRequest>,
    response_tx: mpsc::Sender<RpcResponse>,
) {
    while let Some(request) = request_rx.recv().await {
        let server = Arc::clone(&server);
        let response_tx = response_tx.clone();

        // Handle request (potentially async for key injection)
        tokio::spawn(async move {
            if let Some(response) = server.handle_request(request).await {
                let _ = response_tx.send(response).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_config_default() {
        let config = ServerConfig::default();
        assert!(!config.render_to_terminal);
        assert_eq!(config.width, 80);
        assert_eq!(config.height, 24);
    }

    #[tokio::test]
    async fn test_server_config_headless() {
        let config = ServerConfig::headless(120, 40);
        assert!(!config.render_to_terminal);
        assert_eq!(config.width, 120);
        assert_eq!(config.height, 40);
    }

    #[tokio::test]
    async fn test_server_config_dual() {
        let config = ServerConfig::dual(100, 30);
        assert!(config.render_to_terminal);
    }
}
