//! TUI application main loop.
//!
//! Connects to server, handles input, and renders output.
//! Uses concurrent notification handling with `tokio::select!`.

use std::io;

use {
    crossterm::event::{Event, EventStream, KeyCode, KeyModifiers},
    futures::StreamExt,
    reovim_protocol::v1::{
        RpcNotification, RpcResponse,
        notifications::{
            BUFFER_MODIFIED, BufferModifiedPayload, CURSOR_MOVED, CursorMovedPayload, LOG_ENTRY,
            LogEntryPayload, MODE_CHANGED, ModeChangedPayload, RENDER_COMPLETE,
            RenderCompletePayload,
        },
    },
    serde_json::json,
    tokio::{sync::mpsc, task::JoinHandle},
};

use crate::client::common::{
    ConnectionConfig, ConnectionReader, RpcClient, RpcClientError, RpcWriter, ServerMessage,
};

use super::{
    input::InputHandler,
    log_buffer::{DEFAULT_TUI_LOG_CAPACITY, TuiLogBuffer},
    log_panel::LogPanelState,
    log_render::render_panel,
    render::Renderer,
};

/// Channel buffer size for server messages.
const MESSAGE_CHANNEL_SIZE: usize = 256;

/// Extract HH:MM:SS from an ISO 8601 timestamp.
///
/// Falls back to "??:??:??" if the timestamp cannot be parsed.
fn extract_hms(timestamp: &str) -> String {
    // ISO 8601 format: 2026-01-17T12:34:56Z or 2026-01-17T12:34:56.123Z
    // We want the HH:MM:SS part
    if let Some(t_pos) = timestamp.find('T') {
        let time_part = &timestamp[t_pos + 1..];
        // Take first 8 characters (HH:MM:SS)
        if time_part.len() >= 8 {
            return time_part[..8].to_string();
        }
    }
    "??:??:??".to_string()
}

/// TUI application error.
#[derive(Debug)]
pub enum TuiError {
    /// I/O error.
    Io(io::Error),
    /// RPC error.
    Rpc(RpcClientError),
    /// Server disconnected.
    Disconnected,
}

impl std::fmt::Display for TuiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Rpc(e) => write!(f, "RPC error: {e}"),
            Self::Disconnected => write!(f, "Server disconnected"),
        }
    }
}

impl std::error::Error for TuiError {}

impl From<io::Error> for TuiError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<RpcClientError> for TuiError {
    fn from(e: RpcClientError) -> Self {
        Self::Rpc(e)
    }
}

/// TUI application state from server.
#[derive(Debug, Default)]
struct TuiState {
    /// Current mode display string.
    mode_display: Option<String>,
    /// Cursor line (0-indexed).
    cursor_line: usize,
    /// Cursor column (0-indexed).
    cursor_column: usize,
    /// Whether any buffer is modified.
    buffer_modified: bool,
    /// Whether screen needs redraw.
    needs_redraw: bool,
}

/// TUI application.
pub struct TuiApp {
    /// RPC writer for sending requests.
    rpc_writer: RpcWriter,
    /// Channel receiver for server messages.
    message_rx: mpsc::Receiver<ServerMessage>,
    /// Handle to notification listener task.
    listener_handle: JoinHandle<()>,
    /// Terminal renderer.
    renderer: Renderer,
    /// Whether the app is running.
    running: bool,
    /// Last known terminal size.
    last_size: (u16, u16),
    /// Current state from server.
    state: TuiState,
    /// Log buffer for storing log entries.
    log_buffer: TuiLogBuffer,
    /// Log panel state.
    log_panel: LogPanelState,
    /// Server log subscription ID.
    subscription_id: Option<u64>,
}

impl TuiApp {
    /// Create and connect TUI application.
    ///
    /// Fetches initial state before entering notification-driven mode.
    ///
    /// # Errors
    ///
    /// Returns error if connection or initial state fetch fails.
    pub async fn connect(config: &ConnectionConfig) -> Result<Self, TuiError> {
        // Connect and fetch initial state while we have blocking RpcClient
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

        // Subscribe to server logs (info level and above)
        let subscription_id = match client.call("debug/log_subscribe", json!({})).await {
            Ok(result) => result
                .get("subscription_id")
                .and_then(serde_json::Value::as_u64),
            Err(e) => {
                tracing::debug!("Failed to subscribe to server logs: {e}");
                None
            }
        };

        // Split client for concurrent operation
        let (reader, rpc_writer) = client.into_split();

        // Create message channel
        let (tx, message_rx) = mpsc::channel(MESSAGE_CHANNEL_SIZE);

        // Spawn notification listener
        let listener_handle = tokio::spawn(notification_listener(reader, tx));

        let renderer = Renderer::new();

        Ok(Self {
            rpc_writer,
            message_rx,
            listener_handle,
            renderer,
            running: false,
            last_size: (0, 0),
            state: TuiState {
                mode_display,
                cursor_line,
                cursor_column,
                buffer_modified: false,
                needs_redraw: true,
            },
            log_buffer: TuiLogBuffer::new(DEFAULT_TUI_LOG_CAPACITY),
            log_panel: LogPanelState::new(),
            subscription_id,
        })
    }

    /// Run the TUI application main loop.
    ///
    /// Uses `tokio::select!` to handle both terminal events and server notifications.
    ///
    /// # Errors
    ///
    /// Returns error if fatal error occurs.
    pub async fn run(&mut self) -> Result<(), TuiError> {
        // Install panic hook before entering raw mode to ensure terminal cleanup
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Restore terminal state on panic
            let _ = crossterm::terminal::disable_raw_mode();
            let _ =
                crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
            original_hook(info);
        }));

        // Initialize terminal
        self.renderer.init()?;
        self.running = true;

        // Get initial size and notify server
        let (width, height) = self.renderer.size()?;
        self.last_size = (width, height);
        self.send_resize(width, height).await?;

        // Request initial screen content
        self.refresh_screen().await?;

        // Run the main event loop
        let result = self.run_event_loop().await;

        // Unsubscribe from log notifications
        if let Some(sub_id) = self.subscription_id {
            let _ = self
                .rpc_writer
                .send_request("debug/log_unsubscribe", json!({ "subscription_id": sub_id }))
                .await;
        }

        // Cleanup
        self.renderer.cleanup()?;

        // Abort listener task
        self.listener_handle.abort();

        result
    }

    /// Main event loop using `tokio::select!`.
    async fn run_event_loop(&mut self) -> Result<(), TuiError> {
        let mut terminal_events = EventStream::new();

        while self.running {
            tokio::select! {
                // Handle terminal events
                event = terminal_events.next() => {
                    match event {
                        Some(Ok(Event::Key(key))) => {
                            self.handle_key_event(key).await?;
                        }
                        Some(Ok(Event::Resize(width, height))) => {
                            self.handle_resize(width, height).await?;
                        }
                        Some(Err(e)) => return Err(TuiError::Io(e)),
                        None => break, // Terminal closed
                        _ => {} // Ignore other events (mouse, etc.)
                    }
                }

                // Handle server messages
                msg = self.message_rx.recv() => {
                    match msg {
                        Some(ServerMessage::Notification(notification)) => {
                            self.handle_notification(notification);
                        }
                        Some(ServerMessage::Response(response)) => {
                            self.handle_response(response);
                        }
                        None => {
                            // Channel closed - server disconnected
                            return Err(TuiError::Disconnected);
                        }
                    }
                }
            }

            // Render if needed
            if self.state.needs_redraw {
                self.render().await?;
                self.state.needs_redraw = false;
            }
        }

        Ok(())
    }

    /// Handle a key event.
    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<(), TuiError> {
        // Check for quit (Ctrl+C or Ctrl+Q)
        if matches!(key.code, KeyCode::Char('c' | 'q'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.running = false;
            return Ok(());
        }

        // Check for log panel toggle (Ctrl+L)
        if matches!(key.code, KeyCode::Char('l')) && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.log_panel.toggle();
            self.state.needs_redraw = true;
            return Ok(());
        }

        // Handle log panel keys when visible
        if self.log_panel.visible {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.log_panel.scroll_down(1);
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.log_panel.scroll_up(1);
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Char('G') => {
                    self.log_panel.scroll_to_bottom();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Char('g') => {
                    let total = self.log_buffer.len();
                    self.log_panel.scroll_to_top(total);
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.log_panel.hide();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Char('c') => {
                    self.log_buffer.clear();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Char(c @ '1'..='5') => {
                    self.log_panel.set_level_filter_from_key(c);
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                _ => {} // Fall through to normal key handling
            }
        }

        // Convert key to notation and send to server
        if let Some(keys) = InputHandler::key_to_notation(&key) {
            self.send_keys(&keys).await?;
            // Note: We no longer refresh here - notifications will trigger updates
        }

        Ok(())
    }

    /// Handle a resize event.
    async fn handle_resize(&mut self, width: u16, height: u16) -> Result<(), TuiError> {
        if (width, height) != self.last_size {
            self.last_size = (width, height);
            self.send_resize(width, height).await?;
            // Request refresh after resize
            self.state.needs_redraw = true;
        }
        Ok(())
    }

    /// Handle a server notification.
    fn handle_notification(&mut self, notification: RpcNotification) {
        match notification.method.as_str() {
            MODE_CHANGED => {
                if let Ok(payload) =
                    serde_json::from_value::<ModeChangedPayload>(notification.params)
                {
                    self.state.mode_display = Some(payload.mode.display);
                    self.state.needs_redraw = true;
                    tracing::debug!("Mode changed to: {:?}", self.state.mode_display);
                }
            }
            CURSOR_MOVED => {
                if let Ok(payload) =
                    serde_json::from_value::<CursorMovedPayload>(notification.params)
                {
                    self.state.cursor_line = payload.position.line;
                    self.state.cursor_column = payload.position.column;
                    self.state.needs_redraw = true;
                    tracing::debug!(
                        "Cursor moved to: ({}, {})",
                        self.state.cursor_line,
                        self.state.cursor_column
                    );
                }
            }
            BUFFER_MODIFIED => {
                if let Ok(payload) =
                    serde_json::from_value::<BufferModifiedPayload>(notification.params)
                {
                    self.state.buffer_modified = payload.modified;
                    self.state.needs_redraw = true;
                    tracing::debug!("Buffer modified: {}", self.state.buffer_modified);
                }
            }
            RENDER_COMPLETE => {
                if let Ok(_payload) =
                    serde_json::from_value::<RenderCompletePayload>(notification.params)
                {
                    self.state.needs_redraw = true;
                    tracing::debug!("Render complete notification received");
                }
            }
            LOG_ENTRY => {
                if let Ok(payload) = serde_json::from_value::<LogEntryPayload>(notification.params)
                {
                    // Convert to TuiLogEntry and add to buffer
                    let entry = super::log_buffer::TuiLogEntry::new(
                        extract_hms(&payload.timestamp),
                        payload.level,
                        payload.target,
                        payload.message,
                        payload.source,
                    );
                    self.log_buffer.push(entry);
                    self.log_panel.on_new_entries();
                    // Only redraw if log panel is visible
                    if self.log_panel.visible {
                        self.state.needs_redraw = true;
                    }
                    tracing::trace!("Log entry received: {:?}", payload.level);
                }
            }
            _ => {
                tracing::debug!("Unknown notification: {}", notification.method);
            }
        }
    }

    /// Handle a server response.
    ///
    /// In notification-driven mode, responses are mostly informational.
    #[allow(clippy::unused_self)] // Method signature for future expansion
    fn handle_response(&self, response: RpcResponse) {
        if let Some(error) = response.error {
            tracing::warn!("RPC error for id={}: {}", response.id, error.message);
        }
        // State updates come via notifications, not responses
    }

    /// Render the current screen state.
    async fn render(&mut self) -> Result<(), TuiError> {
        // Request screen content from server
        let _ = self
            .rpc_writer
            .send_request("state/screen_content", json!({ "format": "raw_ansi" }))
            .await;

        // Wait a bit for the response and read it
        // In a fully async model, we'd handle this via the message channel
        // For now, we do a simple refresh
        self.refresh_screen().await?;
        Ok(())
    }

    /// Refresh screen content from server.
    async fn refresh_screen(&mut self) -> Result<(), TuiError> {
        // Send screen content request (fire-and-forget style, but we wait for immediate response)
        // This is a transitional implementation - ideally we'd poll from cached state
        let _ = self
            .rpc_writer
            .send_request("state/screen_content", json!({ "format": "raw_ansi" }))
            .await;

        // Read response from message channel with short timeout
        match tokio::time::timeout(
            std::time::Duration::from_millis(100),
            self.wait_for_screen_content(),
        )
        .await
        {
            Ok(Ok(content)) => {
                self.renderer.render(&content)?;
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                // Timeout - just position cursor
                tracing::debug!("Screen content timeout, skipping render");
            }
        }

        // Render log panel if visible
        if self.log_panel.visible {
            let (width, height) = self.last_size;
            let panel_height = self.log_panel.height.min(height / 2); // Max half screen
            let entries = self.log_buffer.entries();
            let lines = render_panel(&entries, &self.log_panel, width, panel_height);

            // Position cursor at start of panel area
            let panel_start = height.saturating_sub(panel_height);
            self.renderer.set_cursor(0, panel_start)?;

            // Render panel lines
            for line in lines {
                self.renderer.write_line(&line)?;
            }
        }

        // Position cursor based on current state
        #[allow(clippy::cast_possible_truncation)]
        self.renderer
            .set_cursor(self.state.cursor_column as u16, self.state.cursor_line as u16)?;
        self.renderer.show_cursor()?;
        self.renderer.flush()?;

        Ok(())
    }

    /// Wait for screen content response from message channel.
    async fn wait_for_screen_content(&mut self) -> Result<String, TuiError> {
        while let Some(msg) = self.message_rx.recv().await {
            match msg {
                ServerMessage::Response(response) => {
                    if let Some(result) = response.result
                        && let Some(content) = result.get("content").and_then(|v| v.as_str())
                    {
                        return Ok(content.to_string());
                    }
                    if let Some(error) = response.error {
                        tracing::warn!("Screen content error: {}", error.message);
                    }
                    // If it's not the right response, we got our answer
                    return Ok(String::new());
                }
                ServerMessage::Notification(notification) => {
                    // Handle notifications while waiting
                    self.handle_notification(notification);
                }
            }
        }
        Err(TuiError::Disconnected)
    }

    /// Send key sequence to server (fire-and-forget).
    async fn send_keys(&mut self, keys: &str) -> Result<(), TuiError> {
        self.rpc_writer
            .send_request("input/keys", json!({ "keys": keys }))
            .await
            .map_err(TuiError::Rpc)?;
        Ok(())
    }

    /// Send resize notification to server.
    async fn send_resize(&mut self, width: u16, height: u16) -> Result<(), TuiError> {
        self.rpc_writer
            .send_request("editor/resize", json!({ "width": width, "height": height }))
            .await
            .map_err(TuiError::Rpc)?;
        Ok(())
    }

    /// Quit the application.
    pub const fn quit(&mut self) {
        self.running = false;
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        // Abort listener task on drop
        self.listener_handle.abort();
    }
}

/// Notification listener task.
///
/// Reads messages from server and forwards them to the main loop via channel.
async fn notification_listener(mut reader: ConnectionReader, tx: mpsc::Sender<ServerMessage>) {
    loop {
        let line = match reader.read_line().await {
            Ok(l) => l,
            Err(e) => {
                tracing::debug!("Notification listener ended: {e}");
                break;
            }
        };

        // Try to parse as notification first
        if let Ok(notification) = serde_json::from_str::<RpcNotification>(&line) {
            if tx
                .send(ServerMessage::Notification(notification))
                .await
                .is_err()
            {
                break; // Main loop closed channel
            }
            continue;
        }

        // Try to parse as response
        if let Ok(response) = serde_json::from_str::<RpcResponse>(&line) {
            if tx.send(ServerMessage::Response(response)).await.is_err() {
                break; // Main loop closed channel
            }
            continue;
        }

        // Unknown message format - log and continue
        tracing::warn!("Unknown message format: {line}");
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn test_tui_state_default() {
        let state = TuiState::default();
        assert!(state.mode_display.is_none());
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_column, 0);
        assert!(!state.buffer_modified);
        assert!(!state.needs_redraw);
    }

    #[test]
    fn test_mode_changed_payload_parsing() {
        let params = json!({
            "mode": {
                "focus": "Editor",
                "edit_mode": "Insert",
                "sub_mode": "None",
                "display": "INSERT"
            }
        });
        let payload: ModeChangedPayload = serde_json::from_value(params).unwrap();
        assert_eq!(payload.mode.display, "INSERT");
    }

    #[test]
    fn test_cursor_moved_payload_parsing() {
        let params = json!({
            "buffer_id": 1,
            "position": { "line": 10, "column": 5 }
        });
        let payload: CursorMovedPayload = serde_json::from_value(params).unwrap();
        assert_eq!(payload.buffer_id, 1);
        assert_eq!(payload.position.line, 10);
        assert_eq!(payload.position.column, 5);
    }

    #[test]
    fn test_buffer_modified_payload_parsing() {
        let params = json!({
            "buffer_id": 1,
            "modified": true
        });
        let payload: BufferModifiedPayload = serde_json::from_value(params).unwrap();
        assert_eq!(payload.buffer_id, 1);
        assert!(payload.modified);
    }

    #[test]
    fn test_render_complete_payload_parsing() {
        let params = json!({});
        let payload: RenderCompletePayload = serde_json::from_value(params).unwrap();
        // Just verify it parses - payload is currently empty
        let _ = payload;
    }

    #[test]
    fn test_message_channel_size() {
        // Verify channel size is reasonable at compile time
        const _: () = {
            assert!(MESSAGE_CHANNEL_SIZE > 0);
            assert!(MESSAGE_CHANNEL_SIZE <= 1024);
        };
        // Runtime check for test output
        assert_eq!(MESSAGE_CHANNEL_SIZE, 256);
    }
}
