//! Headless TUI client for frame capture without TTY.
//!
//! The `HeadlessClient` connects to the server like an interactive TUI but
//! doesn't require a terminal. It responds to capture requests via RPC,
//! enabling the CLI → Server → TUI → Server → CLI frame capture flow.

use {
    reovim_driver_display::{FrameBuffer, Style},
    reovim_protocol::v1::{
        RpcNotification, RpcResponse, ScreenContentResult,
        notifications::{
            CAPTURE_REQUEST, CAPTURE_RESPONSE, CURSOR_MOVED, CaptureRequestPayload,
            CaptureResponsePayload, CursorMovedPayload, LAYOUT_CHANGED, LayoutChangedPayload,
            MODE_CHANGED, ModeChangedPayload,
        },
    },
    serde_json::json,
    std::collections::VecDeque,
    tokio::sync::mpsc,
};

use crate::client::common::{
    ConnectionConfig, ConnectionReader, RpcClient, RpcClientError, RpcWriter, ServerMessage,
};

use super::render_core::{RenderState, build_frame_content};

/// Default terminal size for headless mode.
const DEFAULT_WIDTH: u16 = 80;
const DEFAULT_HEIGHT: u16 = 24;

/// Maximum terminal size to prevent resource exhaustion.
const MAX_SIZE: u16 = 500;

/// Headless TUI client error.
#[derive(Debug)]
pub enum HeadlessError {
    /// RPC error.
    Rpc(RpcClientError),
    /// Server disconnected.
    Disconnected,
    /// Invalid notification payload.
    InvalidPayload(String),
}

impl std::fmt::Display for HeadlessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rpc(e) => write!(f, "RPC error: {e}"),
            Self::Disconnected => write!(f, "Server disconnected"),
            Self::InvalidPayload(msg) => write!(f, "Invalid payload: {msg}"),
        }
    }
}

impl std::error::Error for HeadlessError {}

impl From<RpcClientError> for HeadlessError {
    fn from(e: RpcClientError) -> Self {
        Self::Rpc(e)
    }
}

/// Headless TUI client for responding to capture requests.
///
/// Unlike `TuiApp`, this client doesn't render to a terminal. It maintains
/// editor state from server notifications and responds to capture requests
/// with frame content built from `RenderState`.
pub struct HeadlessClient {
    /// RPC writer for sending responses.
    rpc_writer: RpcWriter,
    /// Channel receiver for server messages.
    message_rx: mpsc::Receiver<ServerMessage>,
    /// Render state (mode, cursor, size, etc.).
    render_state: RenderState,
    /// Frame buffer for rendering (updated on resize).
    frame_buffer: FrameBuffer,
    /// Whether the client is running.
    running: bool,
    /// Queue of capture requests received during another capture's processing.
    /// This ensures concurrent capture requests are not dropped.
    pending_captures: VecDeque<serde_json::Value>,
}

impl HeadlessClient {
    /// Create and connect headless client.
    ///
    /// Fetches initial state before entering notification-driven mode.
    ///
    /// # Errors
    ///
    /// Returns error if connection or initial state fetch fails.
    pub async fn connect(config: &ConnectionConfig) -> Result<Self, HeadlessError> {
        // Get server address for state
        let server_address = match config {
            ConnectionConfig::Tcp { host, port } => format!("{host}:{port}"),
            #[cfg(unix)]
            ConnectionConfig::UnixSocket(path) => path.display().to_string(),
        };

        // Connect and fetch initial state
        let mut client = RpcClient::connect(config).await?;

        // Get initial mode
        let mode_result = client.call("state/mode", json!({})).await?;
        let mode_display = mode_result
            .get("display")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Get initial cursor position
        let cursor_result = client.call("state/cursor", json!({})).await?;
        #[allow(clippy::cast_possible_truncation)]
        let cursor_line = cursor_result
            .get("line")
            .and_then(serde_json::Value::as_u64)
            .map_or(0, |v| v as usize);
        #[allow(clippy::cast_possible_truncation)]
        let cursor_column = cursor_result
            .get("column")
            .and_then(serde_json::Value::as_u64)
            .map_or(0, |v| v as usize);

        // Get loaded modules
        let modules = client.call("module/list", json!({})).await.map_or_else(
            |_| Vec::new(),
            |result| {
                result
                    .get("modules")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default()
            },
        );

        // Split client into reader/writer
        let (reader, writer) = client.into_split();

        // Create message channel for notifications
        let (tx, rx) = mpsc::channel(256);

        // Spawn notification listener task
        tokio::spawn(Self::notification_listener(reader, tx));

        // Initialize render state with defaults
        let render_state = RenderState {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            mode_display,
            cursor_line,
            cursor_column,
            modules,
            server_address,
            log_panel_visible: false,
        };

        // Create frame buffer with default size
        let frame_buffer = FrameBuffer::new(DEFAULT_WIDTH, DEFAULT_HEIGHT);

        tracing::info!("Headless client connected, size {}x{}", DEFAULT_WIDTH, DEFAULT_HEIGHT);

        Ok(Self {
            rpc_writer: writer,
            message_rx: rx,
            render_state,
            frame_buffer,
            running: true,
            pending_captures: VecDeque::new(),
        })
    }

    /// Run the headless client event loop.
    ///
    /// Processes server notifications and responds to capture requests.
    /// Runs until the server disconnects or an error occurs.
    ///
    /// # Errors
    ///
    /// Returns error if message handling fails.
    pub async fn run(&mut self) -> Result<(), HeadlessError> {
        tracing::info!("Headless client running, waiting for capture requests...");

        while self.running {
            if let Some(msg) = self.message_rx.recv().await {
                self.handle_message(msg).await?;
            } else {
                tracing::info!("Server disconnected");
                self.running = false;
                return Err(HeadlessError::Disconnected);
            }
        }

        Ok(())
    }

    /// Handle a server message.
    async fn handle_message(&mut self, msg: ServerMessage) -> Result<(), HeadlessError> {
        match msg {
            ServerMessage::Notification(notification) => {
                self.handle_notification(&notification).await?;
            }
            ServerMessage::Response(_response) => {
                // Headless client doesn't send requests that expect responses
                // (except during initialization)
            }
        }
        Ok(())
    }

    /// Handle a server notification.
    async fn handle_notification(
        &mut self,
        notification: &RpcNotification,
    ) -> Result<(), HeadlessError> {
        match notification.method.as_str() {
            CAPTURE_REQUEST => {
                self.handle_capture_request(&notification.params).await?;
                // Process any captures that were queued during this capture
                self.process_pending_captures().await?;
            }
            MODE_CHANGED => {
                if let Ok(payload) =
                    serde_json::from_value::<ModeChangedPayload>(notification.params.clone())
                {
                    self.render_state.mode_display = Some(payload.mode.display);
                    tracing::debug!("Mode changed: {:?}", self.render_state.mode_display);
                }
            }
            CURSOR_MOVED => {
                if let Ok(payload) =
                    serde_json::from_value::<CursorMovedPayload>(notification.params.clone())
                {
                    self.render_state.cursor_line = payload.position.line;
                    self.render_state.cursor_column = payload.position.column;
                    tracing::debug!(
                        "Cursor moved: {},{}",
                        payload.position.line,
                        payload.position.column
                    );
                }
            }
            LAYOUT_CHANGED => {
                if let Ok(payload) =
                    serde_json::from_value::<LayoutChangedPayload>(notification.params.clone())
                {
                    // Update size from layout if available
                    let new_width = payload.layout.screen.width;
                    let new_height = payload.layout.screen.height;
                    if new_width > 0
                        && new_height > 0
                        && new_width <= MAX_SIZE
                        && new_height <= MAX_SIZE
                        && (new_width != self.render_state.width
                            || new_height != self.render_state.height)
                    {
                        self.resize(new_width, new_height);
                    }
                }
            }
            _ => {
                // Ignore other notifications
                tracing::trace!("Ignoring notification: {}", notification.method);
            }
        }
        Ok(())
    }

    /// Process any capture requests that were queued during another capture.
    async fn process_pending_captures(&mut self) -> Result<(), HeadlessError> {
        let queue_size = self.pending_captures.len();
        if queue_size > 0 {
            tracing::info!("Processing {} queued capture request(s)", queue_size);
        }
        while let Some(params) = self.pending_captures.pop_front() {
            tracing::info!(
                "Processing queued capture request (remaining: {})",
                self.pending_captures.len()
            );
            self.handle_capture_request(&params).await?;
        }
        Ok(())
    }

    /// Handle a capture request notification.
    ///
    /// Fetches actual screen content from the server before building the capture
    /// response, ensuring the capture reflects the current buffer state.
    async fn handle_capture_request(
        &mut self,
        params: &serde_json::Value,
    ) -> Result<(), HeadlessError> {
        let payload: CaptureRequestPayload = serde_json::from_value(params.clone())
            .map_err(|e| HeadlessError::InvalidPayload(e.to_string()))?;

        tracing::info!(
            "Capture request received: id={}, format={:?}",
            payload.request_id,
            payload.format
        );

        // Fetch actual screen content from server before building capture
        if let Err(e) = self.refresh_frame_buffer().await {
            tracing::warn!("Failed to refresh frame buffer: {e}, using current state");
            // Continue with current buffer state (may be stale/empty)
        }

        // Build frame content (now from populated buffer)
        let content = build_frame_content(&self.render_state, &self.frame_buffer, payload.format);

        // Send response
        let response = CaptureResponsePayload::new(
            payload.request_id,
            ScreenContentResult {
                width: self.render_state.width,
                height: self.render_state.height,
                format: payload.format,
                content,
            },
        );

        let response_value = serde_json::to_value(&response)
            .expect("CaptureResponsePayload serialization cannot fail");

        self.rpc_writer
            .send_notification(CAPTURE_RESPONSE, response_value)
            .await
            .map_err(HeadlessError::Rpc)?;

        tracing::info!("Capture response sent for request {}", payload.request_id);

        Ok(())
    }

    /// Resize the frame buffer.
    fn resize(&mut self, width: u16, height: u16) {
        if width == 0 || height == 0 || width > MAX_SIZE || height > MAX_SIZE {
            tracing::warn!("Invalid resize dimensions: {}x{}", width, height);
            return;
        }

        self.render_state.width = width;
        self.render_state.height = height;
        self.frame_buffer = FrameBuffer::new(width, height);
        tracing::info!("Resized to {}x{}", width, height);
    }

    /// Wait for a response with matching ID, processing notifications meanwhile.
    ///
    /// This allows the headless client to make RPC calls and wait for responses
    /// while continuing to process incoming notifications.
    async fn wait_for_response(
        &mut self,
        request_id: u64,
    ) -> Result<serde_json::Value, HeadlessError> {
        let timeout_duration = std::time::Duration::from_millis(500);

        tokio::time::timeout(timeout_duration, async {
            while let Some(msg) = self.message_rx.recv().await {
                match msg {
                    ServerMessage::Response(response) if response.id == request_id => {
                        if let Some(error) = response.error {
                            return Err(HeadlessError::InvalidPayload(error.message));
                        }
                        return Ok(response.result.unwrap_or(serde_json::Value::Null));
                    }
                    ServerMessage::Response(_) => {
                        // Different response ID, ignore (stale response)
                    }
                    ServerMessage::Notification(notification) => {
                        // Process notification synchronously, continue waiting
                        self.handle_notification_sync(&notification);
                    }
                }
            }
            Err(HeadlessError::Disconnected)
        })
        .await
        .unwrap_or_else(|_| {
            Err(HeadlessError::InvalidPayload("Timeout waiting for screen content".into()))
        })
    }

    /// Fetch screen content from server and populate frame buffer.
    ///
    /// This makes the headless TUI behave like the interactive TUI by fetching
    /// actual buffer content from the server before building capture responses.
    async fn refresh_frame_buffer(&mut self) -> Result<(), HeadlessError> {
        // Clear frame buffer
        self.frame_buffer.clear();

        // Request screen content from server
        let request_id = self
            .rpc_writer
            .send_request("state/screen_content", json!({ "format": "plain_text" }))
            .await
            .map_err(HeadlessError::Rpc)?;

        // Wait for response
        let response = self.wait_for_response(request_id).await?;

        // Extract content
        let content = response
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Write content to frame buffer (line by line, like interactive TUI)
        let default_style = Style::default();
        for (y, line) in content
            .lines()
            .enumerate()
            .take(self.render_state.height as usize)
        {
            #[allow(clippy::cast_possible_truncation)]
            self.frame_buffer
                .write_str(0, y as u16, line, &default_style);
        }

        tracing::debug!("Refreshed frame buffer with {} lines", content.lines().count());
        Ok(())
    }

    /// Handle notification synchronously (for use during response wait).
    ///
    /// This is a non-async version of notification handling used when waiting
    /// for a specific response while still processing incoming notifications.
    /// `CAPTURE_REQUEST`s that arrive during the wait are queued for later processing.
    fn handle_notification_sync(&mut self, notification: &RpcNotification) {
        match notification.method.as_str() {
            CAPTURE_REQUEST => {
                // Queue capture request for processing after current capture completes
                tracing::info!(
                    "Queuing capture request received during wait (queue size: {})",
                    self.pending_captures.len() + 1
                );
                self.pending_captures.push_back(notification.params.clone());
            }
            MODE_CHANGED => {
                if let Ok(payload) =
                    serde_json::from_value::<ModeChangedPayload>(notification.params.clone())
                {
                    self.render_state.mode_display = Some(payload.mode.display);
                }
            }
            CURSOR_MOVED => {
                if let Ok(payload) =
                    serde_json::from_value::<CursorMovedPayload>(notification.params.clone())
                {
                    self.render_state.cursor_line = payload.position.line;
                    self.render_state.cursor_column = payload.position.column;
                }
            }
            LAYOUT_CHANGED => {
                if let Ok(payload) =
                    serde_json::from_value::<LayoutChangedPayload>(notification.params.clone())
                {
                    let new_width = payload.layout.screen.width;
                    let new_height = payload.layout.screen.height;
                    if new_width > 0
                        && new_height > 0
                        && new_width <= MAX_SIZE
                        && new_height <= MAX_SIZE
                    {
                        self.resize(new_width, new_height);
                    }
                }
            }
            _ => {}
        }
    }

    /// Notification listener task.
    ///
    /// Reads messages from the server and sends them to the message channel.
    async fn notification_listener(mut reader: ConnectionReader, tx: mpsc::Sender<ServerMessage>) {
        loop {
            match reader.read_line().await {
                Ok(line) => {
                    // Parse the message
                    let msg = if let Ok(response) = serde_json::from_str::<RpcResponse>(&line) {
                        ServerMessage::Response(response)
                    } else if let Ok(notification) = serde_json::from_str::<RpcNotification>(&line)
                    {
                        ServerMessage::Notification(notification)
                    } else {
                        tracing::warn!("Unknown message format: {line}");
                        continue;
                    };

                    if tx.send(msg).await.is_err() {
                        tracing::debug!("Message channel closed, stopping listener");
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        tracing::debug!("Server connection closed");
                    } else {
                        tracing::error!("Error reading message: {e}");
                    }
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_dimensions() {
        assert_eq!(DEFAULT_WIDTH, 80);
        assert_eq!(DEFAULT_HEIGHT, 24);
    }

    #[test]
    fn test_max_size_limit() {
        assert_eq!(MAX_SIZE, 500);
    }

    #[test]
    fn test_headless_error_display() {
        let err = HeadlessError::Disconnected;
        assert_eq!(format!("{err}"), "Server disconnected");

        let err = HeadlessError::InvalidPayload("bad json".to_string());
        assert_eq!(format!("{err}"), "Invalid payload: bad json");
    }
}
