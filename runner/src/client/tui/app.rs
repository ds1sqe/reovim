//! TUI application main loop.
//!
//! Connects to server, handles input, and renders output.
//! Uses concurrent notification handling with `tokio::select!`.

use std::{
    fmt::Write as _,
    io::{self, Write},
    time::{Duration, Instant},
};

use {
    crossterm::event::{Event, EventStream, KeyCode, KeyModifiers},
    futures::StreamExt,
    reovim_protocol::v1::{
        RpcNotification, RpcResponse,
        notifications::{
            BUFFER_MODIFIED, BufferModifiedPayload, CURSOR_MOVED, CursorMovedPayload, DETACH,
            DetachPayload, LOG_ENTRY, LogEntryPayload, MODE_CHANGED, ModeChangedPayload,
            RENDER_COMPLETE, RenderCompletePayload,
        },
    },
    serde_json::json,
    tokio::{sync::mpsc, task::JoinHandle},
};

/// Interval for frame buffer capture (5 seconds).
const FRAME_CAPTURE_INTERVAL: Duration = Duration::from_secs(5);

use crate::client::common::{
    ConnectionConfig, ConnectionReader, RpcClient, RpcClientError, RpcWriter, ServerMessage,
};

use reovim_driver_display::{ColorMode, FrameRenderer, Style};

use super::{
    cli_executor,
    cli_panel::CliPanelState,
    cli_render::render_panel as render_cli_panel,
    input::InputHandler,
    log_buffer::{DEFAULT_TUI_LOG_CAPACITY, TuiLogBuffer},
    log_panel::LogPanelState,
    log_render::render_panel as render_log_panel,
    render::Renderer,
};

/// Channel buffer size for server messages.
const MESSAGE_CHANNEL_SIZE: usize = 256;

/// Timeout for prefix mode (2 seconds).
const PREFIX_TIMEOUT: Duration = Duration::from_millis(2000);

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
    /// Loaded modules list.
    modules: Vec<String>,
    /// Last key sent to server (for debug).
    last_key: Option<String>,
    /// Last RPC error message (for statusline display).
    last_error: Option<String>,
    /// When the error occurred (for auto-clear after 5 seconds).
    error_timestamp: Option<Instant>,
}

/// TUI application.
pub struct TuiApp {
    /// RPC writer for sending requests.
    rpc_writer: RpcWriter,
    /// Channel receiver for server messages.
    message_rx: mpsc::Receiver<ServerMessage>,
    /// Handle to notification listener task.
    listener_handle: JoinHandle<()>,
    /// Terminal renderer (basic terminal ops).
    renderer: Renderer,
    /// Frame renderer with double-buffering for content.
    frame_renderer: FrameRenderer,
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
    /// CLI panel state.
    cli_panel: CliPanelState,
    /// Server log subscription ID.
    subscription_id: Option<u64>,
    /// Connected server address for statusline.
    server_address: String,
    /// Debug configuration (None = debug disabled).
    debug_config: Option<super::TuiDebugConfig>,
    /// Debug session log file handle.
    debug_log_file: Option<std::fs::File>,
    /// Last frame capture time.
    last_frame_capture: Instant,
    /// Whether we're in prefix mode waiting for next key (e.g., after `<C-b>`).
    prefix_mode: bool,
    /// When prefix mode was entered (for timeout).
    prefix_entered: Option<Instant>,
}

impl TuiApp {
    /// Create and connect TUI application.
    ///
    /// Fetches initial state before entering notification-driven mode.
    ///
    /// # Arguments
    ///
    /// * `config` - Connection configuration for server
    /// * `debug_config` - Optional debug configuration for statusline/frame capture
    ///
    /// # Errors
    ///
    /// Returns error if connection or initial state fetch fails.
    #[allow(clippy::too_many_lines)]
    pub async fn connect(
        config: &ConnectionConfig,
        debug_config: Option<super::TuiDebugConfig>,
    ) -> Result<Self, TuiError> {
        // Get server address for statusline display
        let server_address = match config {
            ConnectionConfig::Tcp { host, port } => format!("{host}:{port}"),
            #[cfg(unix)]
            ConnectionConfig::UnixSocket(path) => path.display().to_string(),
        };

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

        // Get loaded modules
        let modules = match client.call("module/list", json!({})).await {
            Ok(result) => result
                .get("modules")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
            Err(e) => {
                tracing::debug!("Failed to get module list: {e}");
                Vec::new()
            }
        };

        // Register for buffer notifications by setting the active buffer.
        // This is required to receive buffer-scoped notifications like cursor_moved,
        // buffer_modified, and render_complete which trigger TUI updates.
        if let Ok(buffer_list) = client.call("buffer/list", json!({})).await
            && let Some(buffers) = buffer_list.get("buffers").and_then(|v| v.as_array())
            && let Some(first_buffer) = buffers.first()
            && let Some(buffer_id) = first_buffer.get("id").and_then(serde_json::Value::as_u64)
        {
            match client
                .call("editor/set_active_buffer", json!({ "buffer_id": buffer_id }))
                .await
            {
                Ok(_) => tracing::debug!("Set active buffer to {buffer_id}"),
                Err(e) => tracing::debug!("Failed to set active buffer: {e}"),
            }
        }

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

        // Initialize debug features if enabled
        let debug_log_file = debug_config.as_ref().and_then(|cfg| {
            // Create directories
            let frame_dir = cfg.log_dir.join("frame-buffer");
            if let Err(e) = std::fs::create_dir_all(&frame_dir) {
                tracing::warn!("Failed to create frame capture dir: {e}");
            }

            // Open session log file
            let log_path = cfg.session_log_path();
            match std::fs::File::create(&log_path) {
                Ok(file) => {
                    tracing::debug!("Debug session log: {}", log_path.display());
                    Some(file)
                }
                Err(e) => {
                    tracing::warn!("Failed to create debug log file: {e}");
                    None
                }
            }
        });

        Ok(Self {
            rpc_writer,
            message_rx,
            listener_handle,
            renderer,
            frame_renderer: FrameRenderer::default(), // Resized on run()
            running: false,
            last_size: (0, 0),
            state: TuiState {
                mode_display,
                cursor_line,
                cursor_column,
                buffer_modified: false,
                needs_redraw: true,
                modules,
                last_key: None,
                last_error: None,
                error_timestamp: None,
            },
            log_buffer: TuiLogBuffer::new(DEFAULT_TUI_LOG_CAPACITY),
            log_panel: LogPanelState::new(),
            cli_panel: CliPanelState::new(),
            subscription_id,
            server_address,
            debug_config,
            debug_log_file,
            last_frame_capture: Instant::now(),
            prefix_mode: false,
            prefix_entered: None,
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
        // Log session start
        self.debug_log(&format!(
            "TUI session started - server: {}, mode: {}",
            self.server_address,
            self.state.mode_display.as_deref().unwrap_or("?")
        ));

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
        self.frame_renderer.resize(width, height);
        self.send_resize(width, height).await?;

        // Request initial screen content
        self.refresh_screen().await?;

        // Run the main event loop
        let result = self.run_event_loop().await;

        // Log session end
        self.debug_log("TUI session ended");

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

        // Create a 1-second interval for statusline updates (only ticks if debug mode)
        let mut statusline_tick = tokio::time::interval(Duration::from_secs(1));
        statusline_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

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

                // Periodic statusline update (debug mode only)
                // Trigger a full refresh to update statusline and capture
                _ = statusline_tick.tick(), if self.debug_config.is_some() => {
                    self.state.needs_redraw = true;
                }
            }

            // Auto-clear error after 5 seconds
            if let Some(ts) = self.state.error_timestamp
                && ts.elapsed() > Duration::from_secs(5)
            {
                self.state.last_error = None;
                self.state.error_timestamp = None;
                self.state.needs_redraw = true;
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
    #[allow(clippy::too_many_lines)]
    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<(), TuiError> {
        // Check prefix timeout first
        if self.prefix_mode
            && let Some(entered) = self.prefix_entered
            && entered.elapsed() > PREFIX_TIMEOUT
        {
            self.prefix_mode = false;
            self.prefix_entered = None;
            self.state.needs_redraw = true;
            // Timeout - continue to normal handling
        }

        // Handle prefix mode keys
        if self.prefix_mode {
            self.prefix_mode = false;
            self.prefix_entered = None;
            self.state.needs_redraw = true;

            return match key.code {
                KeyCode::Char('d') => {
                    // Detach: quit TUI without killing server
                    tracing::info!("Detaching from server (prefix mode)");
                    self.debug_log("Detach via <C-b>d");
                    self.running = false;
                    Ok(())
                }
                // <C-b>; toggles CLI panel
                KeyCode::Char(';') => {
                    self.cli_panel.toggle();
                    self.state.needs_redraw = true;
                    Ok(())
                }
                // <C-b><C-b> sends literal <C-b>
                KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.send_keys("<C-b>").await?;
                    Ok(())
                }
                KeyCode::Char('\x02') => {
                    self.send_keys("<C-b>").await?;
                    Ok(())
                }
                KeyCode::Esc => {
                    // Cancel prefix mode
                    Ok(())
                }
                _ => {
                    // Unknown prefix command - ignore
                    tracing::debug!("Unknown prefix command: {:?}", key.code);
                    Ok(())
                }
            };
        }

        // Check for prefix mode entry (Ctrl+B)
        // Handle both 'b' with CONTROL modifier and raw control character '\x02'
        let is_ctrl_b = matches!(key.code, KeyCode::Char('b'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
            || matches!(key.code, KeyCode::Char('\x02'));
        if is_ctrl_b {
            self.prefix_mode = true;
            self.prefix_entered = Some(Instant::now());
            self.state.needs_redraw = true;
            return Ok(());
        }

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

        // Handle CLI panel keys when visible
        if self.cli_panel.visible {
            match key.code {
                KeyCode::Esc => {
                    self.cli_panel.hide();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Enter => {
                    if let Some(cmd) = self.cli_panel.submit() {
                        // Check for clear command specially
                        if cmd.trim() == "clear" {
                            self.cli_panel.history.clear();
                            self.state.needs_redraw = true;
                            return Ok(());
                        }
                        let result =
                            cli_executor::execute_command(&mut self.rpc_writer, &cmd).await;
                        self.cli_panel.add_result(cmd, result);
                        self.state.needs_redraw = true;
                    }
                    return Ok(());
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.cli_panel.insert_char(c);
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Backspace => {
                    self.cli_panel.backspace();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Delete => {
                    self.cli_panel.delete();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Left => {
                    self.cli_panel.move_cursor_left();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Right => {
                    self.cli_panel.move_cursor_right();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Up => {
                    self.cli_panel.history_prev();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Down => {
                    self.cli_panel.history_next();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Home => {
                    self.cli_panel.move_cursor_home();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::End => {
                    self.cli_panel.move_cursor_end();
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                _ => {} // Other keys pass through
            }
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
            self.state.last_key = Some(keys.clone());
            self.send_keys(&keys).await?;
        }

        Ok(())
    }

    /// Handle a resize event.
    async fn handle_resize(&mut self, width: u16, height: u16) -> Result<(), TuiError> {
        if (width, height) != self.last_size {
            self.last_size = (width, height);
            self.frame_renderer.resize(width, height);
            self.send_resize(width, height).await?;
            // Request refresh after resize
            self.state.needs_redraw = true;
            self.debug_log(&format!("Terminal resized: {width}x{height}"));
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
                    let new_mode = payload.mode.display.clone();
                    self.state.mode_display = Some(payload.mode.display);
                    self.state.needs_redraw = true;
                    tracing::debug!("Mode changed to: {:?}", self.state.mode_display);
                    self.debug_log(&format!("Mode changed: {new_mode}"));
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
            DETACH => {
                // Server requested client to detach - disconnect gracefully
                if let Ok(payload) =
                    serde_json::from_value::<DetachPayload>(notification.params.clone())
                {
                    if let Some(reason) = payload.reason {
                        tracing::info!("Detaching: {reason}");
                    } else {
                        tracing::info!("Detaching from server");
                    }
                } else {
                    tracing::info!("Detaching from server");
                }
                self.running = false;
            }
            _ => {
                tracing::debug!("Unknown notification: {}", notification.method);
            }
        }
    }

    /// Handle a server response.
    ///
    /// Captures RPC errors for statusline display (auto-clears after 5 seconds).
    fn handle_response(&mut self, response: RpcResponse) {
        if let Some(error) = response.error {
            let msg = format!("RPC {}: {}", response.id, error.message);
            tracing::warn!("{}", msg);
            self.state.last_error = Some(msg);
            self.state.error_timestamp = Some(Instant::now());
            self.state.needs_redraw = true;
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
    ///
    /// Uses double-buffered frame renderer:
    /// 1. Clear back buffer
    /// 2. Write server content to back buffer
    /// 3. Write log panel to back buffer (if visible)
    /// 4. Write statusline to back buffer (if debug mode)
    /// 5. Flush (diff render to stdout)
    async fn refresh_screen(&mut self) -> Result<(), TuiError> {
        let (width, height) = self.last_size;
        if width == 0 || height == 0 {
            return Ok(());
        }

        // Clear back buffer
        self.frame_renderer.buffer_mut().clear();

        // Request screen content from server
        let _ = self
            .rpc_writer
            .send_request("state/screen_content", json!({ "format": "plain_text" }))
            .await;

        // Read response and write to frame buffer
        match tokio::time::timeout(
            std::time::Duration::from_millis(100),
            self.wait_for_screen_content(),
        )
        .await
        {
            Ok(Ok(content)) => {
                // Write server content to frame buffer (line by line)
                let default_style = Style::default();
                for (y, line) in content.lines().enumerate().take(height as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let y_u16 = y as u16;
                    self.frame_renderer
                        .buffer_mut()
                        .write_str(0, y_u16, line, &default_style);
                }
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                tracing::debug!("Screen content timeout, skipping render");
            }
        }

        // Calculate panel heights and positions
        let statusline_height: u16 = u16::from(self.debug_config.is_some());
        let available_height = height.saturating_sub(statusline_height);

        // Write CLI panel to frame buffer (if visible)
        let cli_panel_height = if self.cli_panel.visible {
            let panel_height = self.cli_panel.height.min(available_height / 2);
            let lines = render_cli_panel(&self.cli_panel, width, panel_height);

            let panel_start = available_height.saturating_sub(panel_height);
            let default_style = Style::default();
            for (i, line) in lines.iter().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                let y = panel_start + i as u16;
                if y < available_height {
                    self.frame_renderer
                        .buffer_mut()
                        .write_str(0, y, line, &default_style);
                }
            }
            panel_height
        } else {
            0
        };

        // Write log panel to frame buffer (if visible)
        if self.log_panel.visible {
            let max_log_height = available_height.saturating_sub(cli_panel_height);
            let panel_height = self.log_panel.height.min(max_log_height / 2);
            let entries = self.log_buffer.entries();
            let lines = render_log_panel(&entries, &self.log_panel, width, panel_height);

            let panel_start = available_height.saturating_sub(cli_panel_height + panel_height);
            let default_style = Style::default();
            for (i, line) in lines.iter().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                let y = panel_start + i as u16;
                if y < available_height.saturating_sub(cli_panel_height) {
                    self.frame_renderer
                        .buffer_mut()
                        .write_str(0, y, line, &default_style);
                }
            }
        }

        // Write debug statusline to frame buffer (if enabled)
        self.write_statusline_to_buffer();

        // Capture frame BEFORE flush (back buffer has current content)
        // After flush, buffers swap and back becomes stale
        self.maybe_capture_frame();

        // Flush frame buffer to stdout (diff rendering)
        self.frame_renderer.flush(&mut io::stdout())?;

        // Position cursor and show
        #[allow(clippy::cast_possible_truncation)]
        self.renderer
            .set_cursor(self.state.cursor_column as u16, self.state.cursor_line as u16)?;
        self.renderer.show_cursor()?;
        self.renderer.flush()?;

        Ok(())
    }

    /// Write debug statusline to frame buffer.
    ///
    /// Shows error in red if present, otherwise normal status with inverse video.
    /// Only writes if debug mode is enabled.
    fn write_statusline_to_buffer(&mut self) {
        if self.debug_config.is_none() {
            return;
        }

        let (width, height) = self.last_size;
        if width == 0 || height == 0 {
            return;
        }

        let statusline_y = height.saturating_sub(1);

        // If error exists, show in red; otherwise show normal status
        let (line, style) = if let Some(ref err) = self.state.last_error {
            // Error statusline: red background
            let err_line = format!("ERR: {err}");
            let style = Style::default()
                .bg(reovim_driver_display::Color::Red)
                .fg(reovim_driver_display::Color::White);
            (err_line, style)
        } else {
            // Normal statusline: inverse video
            let now = chrono::Local::now();
            let timestamp = now.format("%y-%m-%d %H:%M:%S %Z").to_string();
            let server = &self.server_address;
            let mode = self.state.mode_display.as_deref().unwrap_or("?");
            let modules_count = self.state.modules.len();
            let last_key = self.state.last_key.as_deref().unwrap_or("-");
            let cursor = format!("{}:{}", self.state.cursor_line, self.state.cursor_column);
            let prefix_indicator = if self.prefix_mode { "^B-" } else { "" };

            let line = format!(
                "{prefix_indicator}{timestamp}|{server}|{mode}|k:{last_key}|c:{cursor}|m:{modules_count}"
            );
            let style = Style::default().reverse();
            (line, style)
        };

        // Truncate or pad to width
        let display_line: String = if line.chars().count() > width as usize {
            line.chars().take(width as usize).collect()
        } else {
            format!("{:width$}", line, width = width as usize)
        };

        // Write to frame buffer
        self.frame_renderer
            .buffer_mut()
            .write_str(0, statusline_y, &display_line, &style);
    }

    /// Capture frame buffer to file if interval has elapsed.
    ///
    /// Captures the complete TUI screen state including:
    /// - Server content (main editor area)
    /// - Log panel (if visible)
    /// - Debug statusline
    ///
    /// Only captures if debug mode is enabled and sufficient time has passed.
    fn maybe_capture_frame(&mut self) {
        let Some(ref config) = self.debug_config else {
            return;
        };

        // Check if capture interval has elapsed
        if self.last_frame_capture.elapsed() < FRAME_CAPTURE_INTERVAL {
            return;
        }

        self.last_frame_capture = Instant::now();

        // Build the complete frame content from current TUI state
        let frame_content = self.build_frame_content();

        // Generate timestamp for filename
        let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
        let path = config.frame_capture_path(&timestamp);

        // Ensure directory exists
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::warn!("Failed to create frame capture dir: {e}");
            return;
        }

        // Write frame content
        if let Err(e) = std::fs::write(&path, &frame_content) {
            tracing::warn!("Failed to write frame capture: {e}");
        } else {
            // Log to session log
            self.debug_log(&format!("Frame captured: {}", path.display()));
        }
    }

    /// Build frame content from the frame buffer in LLM-friendly format.
    ///
    /// Reads directly from `frame_renderer.buffer()` - the same source
    /// that was flushed to stdout. This ensures capture = render.
    ///
    /// Format designed for easy parsing by language models:
    /// - Clear section markers with `===`
    /// - Key-value metadata
    /// - Explicit line numbers in `[N]` format
    /// - ANSI codes preserved for styled content (e.g., inverse statusline)
    fn build_frame_content(&self) -> String {
        let (width, height) = self.last_size;
        if width == 0 || height == 0 {
            return String::new();
        }

        let mut output = String::new();

        // Metadata section (key: value format for easy parsing)
        let now = chrono::Local::now();
        let timestamp = now.format("%Y-%m-%d %H:%M:%S %z").to_string();
        let mode = self.state.mode_display.as_deref().unwrap_or("UNKNOWN");
        let cursor_line = self.state.cursor_line;
        let cursor_col = self.state.cursor_column;
        let modules_count = self.state.modules.len();

        let _ = writeln!(output, "=== FRAME CAPTURE ===");
        let _ = writeln!(output, "timestamp: {timestamp}");
        let _ = writeln!(output, "screen_size: {width}x{height}");
        let _ = writeln!(output, "server: {}", self.server_address);
        let _ = writeln!(output, "mode: {mode}");
        let _ = writeln!(output, "cursor: line={cursor_line}, col={cursor_col}");
        let _ = writeln!(output, "modules: {modules_count}");
        let _ = writeln!(output, "log_panel_visible: {}", self.log_panel.visible);

        // Read screen content directly from frame buffer
        let buffer = self.frame_renderer.buffer();
        let buf_height = buffer.height();

        let _ = writeln!(output, "\n=== SCREEN CONTENT ({width} cols x {buf_height} rows) ===");

        // Read each row from frame buffer
        for y in 0..buf_height {
            let mut line = String::new();
            let mut current_style: Option<Style> = None;

            if let Some(row) = buffer.row(y) {
                for cell in row {
                    if cell.is_continuation {
                        continue;
                    }

                    // Track style changes for ANSI output
                    let cell_style = &cell.style;
                    if current_style.as_ref() != Some(cell_style) {
                        // Close previous style if any
                        if current_style.is_some() {
                            line.push_str("\x1b[0m");
                        }
                        // Open new style if not default
                        let ansi = cell_style.to_ansi_start(ColorMode::TrueColor);
                        if !ansi.is_empty() {
                            line.push_str(&ansi);
                        }
                        current_style = Some(cell_style.clone());
                    }

                    line.push(cell.char);
                }

                // Close any open style
                if current_style.is_some() {
                    line.push_str("\x1b[0m");
                }
            }

            // Use [N] format for easy regex matching
            let _ = writeln!(output, "[{}] {}", y + 1, line);
        }

        let _ = writeln!(output, "=== END FRAME ===");

        output
    }

    /// Write a message to the debug session log.
    ///
    /// Only writes if debug mode is enabled and log file is available.
    fn debug_log(&mut self, msg: &str) {
        if let Some(ref mut file) = self.debug_log_file {
            let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
            if let Err(e) = writeln!(file, "[{timestamp}] {msg}") {
                tracing::warn!("Failed to write debug log: {e}");
            }
            // Flush immediately for debugging
            let _ = file.flush();
        }
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
