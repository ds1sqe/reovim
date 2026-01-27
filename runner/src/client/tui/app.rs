//! TUI application main loop.
//!
//! Connects to server, handles input, and renders output.
//! Uses concurrent notification handling with `tokio::select!`.

use std::{
    collections::HashMap,
    io::{self, Write},
    time::{Duration, Instant},
};

use {
    crossterm::event::{Event, EventStream, KeyCode, KeyModifiers},
    futures::StreamExt,
    reovim_protocol::v1::{
        RpcNotification, RpcResponse, ScreenContentResult, ScreenFormat, WireLayoutInfo,
        notifications::{
            BUFFER_MODIFIED, BufferModifiedPayload, CAPTURE_REQUEST, CAPTURE_RESPONSE,
            CURSOR_MOVED, CaptureRequestPayload, CaptureResponsePayload, CursorMovedPayload,
            DETACH, DetachPayload, LAYOUT_CHANGED, LOG_ENTRY, LayoutChangedPayload,
            LogEntryPayload, MODE_CHANGED, ModeChangedPayload, OPTION_CHANGED,
            OptionChangedPayload, RENDER_COMPLETE, RenderCompletePayload,
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

use reovim_driver_display::{FrameRenderer, Style};

use super::{
    cli_executor,
    cli_panel::CliPanelState,
    cli_render::render_panel as render_cli_panel,
    input::InputHandler,
    log_buffer::{DEFAULT_TUI_LOG_CAPACITY, TuiLogBuffer},
    log_panel::LogPanelState,
    log_render::render_panel as render_log_panel,
    render::{CursorStyleKind, Renderer},
    render_core::{self, RenderState},
};

/// Channel buffer size for server messages.
const MESSAGE_CHANNEL_SIZE: usize = 256;

/// Timeout for prefix mode (2 seconds).
const PREFIX_TIMEOUT: Duration = Duration::from_secs(2);

/// Maximum concurrent pending CLI query requests.
const MAX_PENDING_REQUESTS: usize = 16;

/// Query timeout duration (10 seconds).
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Tracks a pending RPC query for CLI panel correlation.
struct PendingRequest {
    /// The command (format key) that was executed.
    command: String,
    /// When the request was sent.
    sent_at: Instant,
    /// Index in `cli_panel.history` for updating result.
    history_index: usize,
}

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
    /// Current layout state from server (#444).
    layout: Option<WireLayoutInfo>,
    /// Cached option values from server (#445).
    /// Used for line number rendering and other client-side display decisions.
    options: HashMap<String, serde_json::Value>,
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
    /// Pending CLI query requests awaiting responses.
    pending_requests: HashMap<u64, PendingRequest>,
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

        // Get initial layout state (#444)
        let initial_layout = match client.call("state/layout", json!({})).await {
            Ok(result) => serde_json::from_value::<WireLayoutInfo>(result).ok(),
            Err(e) => {
                tracing::debug!("Failed to get initial layout: {e}");
                None
            }
        };

        // Get initial options (#445) - for line number display etc.
        let initial_options = match client.call("state/options", json!({})).await {
            Ok(result) => result
                .get("options")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<HashMap<String, serde_json::Value>>()
                })
                .unwrap_or_default(),
            Err(e) => {
                tracing::debug!("Failed to get initial options: {e}");
                HashMap::new()
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
                layout: initial_layout,   // Initial layout from server (#444)
                options: initial_options, // Initial options from server (#445)
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
            pending_requests: HashMap::new(),
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
                            self.handle_notification(notification).await;
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

            // Check for timed-out pending CLI requests
            {
                use super::cli_panel::CliResult;

                let now = Instant::now();
                let timed_out: Vec<u64> = self
                    .pending_requests
                    .iter()
                    .filter(|(_, req)| now.duration_since(req.sent_at) > QUERY_TIMEOUT)
                    .map(|(id, _)| *id)
                    .collect();

                for id in timed_out {
                    if let Some(pending) = self.pending_requests.remove(&id) {
                        tracing::warn!("Query timeout for command: {}", pending.command);
                        let _ = self.cli_panel.update_result(
                            pending.history_index,
                            CliResult::Err("Query timed out".to_string()),
                        );
                        self.state.needs_redraw = true;
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
                    use super::cli_panel::CliResult;

                    if let Some(cmd) = self.cli_panel.submit() {
                        // Check for clear command specially
                        if cmd.trim() == "clear" {
                            self.cli_panel.history.clear();
                            self.pending_requests.clear(); // Clear stale pending refs
                            self.state.needs_redraw = true;
                            return Ok(());
                        }

                        match cli_executor::classify_command(&cmd) {
                            cli_executor::CommandType::Local(result) => {
                                self.cli_panel.add_result(cmd, result);
                            }
                            cli_executor::CommandType::FireAndForget => {
                                let result = cli_executor::execute_fire_and_forget(
                                    &mut self.rpc_writer,
                                    &cmd,
                                )
                                .await;
                                self.cli_panel.add_result(cmd, result);
                            }
                            cli_executor::CommandType::Query {
                                method,
                                params,
                                format_key,
                            } => {
                                // Check pending request limit
                                if self.pending_requests.len() >= MAX_PENDING_REQUESTS {
                                    self.cli_panel.add_result(
                                        cmd,
                                        CliResult::Err("Too many pending requests".to_string()),
                                    );
                                } else {
                                    // Send request
                                    match self.rpc_writer.send_request(method, params).await {
                                        Ok(id) => {
                                            // Add pending entry to history
                                            let history_index = self.cli_panel.history.len();
                                            self.cli_panel.add_result(
                                                cmd.clone(),
                                                CliResult::Pending("Querying...".to_string()),
                                            );

                                            // Track for correlation
                                            self.pending_requests.insert(
                                                id,
                                                PendingRequest {
                                                    command: format_key.to_string(),
                                                    sent_at: Instant::now(),
                                                    history_index,
                                                },
                                            );
                                        }
                                        Err(e) => {
                                            self.cli_panel.add_result(
                                                cmd,
                                                CliResult::Err(format!("Send error: {e}")),
                                            );
                                        }
                                    }
                                }
                            }
                        }
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
                KeyCode::PageUp => {
                    // Scroll up by roughly half the visible panel height
                    let visible_height = (self.last_size.1 / 4) as usize;
                    self.cli_panel.scroll_up(visible_height.max(5));
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::PageDown => {
                    // Scroll down by roughly half the visible panel height
                    let visible_height = (self.last_size.1 / 4) as usize;
                    self.cli_panel.scroll_down(visible_height.max(5));
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.cli_panel.scroll_to_bottom();
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
                KeyCode::PageUp => {
                    let visible_height = (self.last_size.1 / 4) as usize;
                    self.log_panel.scroll_up(visible_height.max(5));
                    self.state.needs_redraw = true;
                    return Ok(());
                }
                KeyCode::PageDown => {
                    let visible_height = (self.last_size.1 / 4) as usize;
                    self.log_panel.scroll_down(visible_height.max(5));
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

    /// Handle a capture request notification (#447).
    ///
    /// Responds to `tui/capture-request` from server with frame content.
    async fn handle_capture_request(&mut self, params: serde_json::Value) {
        let Ok(payload) = serde_json::from_value::<CaptureRequestPayload>(params) else {
            tracing::warn!("Failed to parse capture request payload");
            return;
        };

        tracing::debug!(
            "Received capture request {} (format: {:?})",
            payload.request_id,
            payload.format
        );

        // Build frame content using the shared render core
        let render_state = self.to_render_state();
        let content = render_core::build_frame_content(
            &render_state,
            self.frame_renderer.buffer(),
            payload.format,
        );

        // Send capture response back to server
        let response = CaptureResponsePayload::new(
            payload.request_id,
            ScreenContentResult {
                width: self.last_size.0,
                height: self.last_size.1,
                format: payload.format,
                content,
            },
        );

        if let Err(e) = self
            .rpc_writer
            .send_notification(
                CAPTURE_RESPONSE,
                serde_json::to_value(&response)
                    .expect("CaptureResponsePayload serialization cannot fail"),
            )
            .await
        {
            tracing::warn!("Failed to send capture response: {e}");
        } else {
            tracing::debug!("Sent capture response {}", payload.request_id);
        }
    }

    /// Handle a server notification.
    ///
    /// Handles both state update notifications (mode, cursor, etc.) and
    /// capture requests (#447) for frame capture via RPC relay.
    #[expect(
        clippy::too_many_lines,
        reason = "notification handling requires separate logic for each notification type"
    )]
    async fn handle_notification(&mut self, notification: RpcNotification) {
        match notification.method.as_str() {
            CAPTURE_REQUEST => self.handle_capture_request(notification.params).await,
            MODE_CHANGED => {
                if let Ok(payload) =
                    serde_json::from_value::<ModeChangedPayload>(notification.params)
                {
                    let new_mode = payload.mode.display.clone();
                    self.state.mode_display = Some(payload.mode.display);
                    self.state.needs_redraw = true;

                    // Update cursor style based on edit mode (#440)
                    let cursor_style = match payload.mode.edit_mode.as_str() {
                        "Insert" => CursorStyleKind::Bar,
                        "Replace" => CursorStyleKind::Underline,
                        _ => CursorStyleKind::Block, // Normal, Visual, etc.
                    };
                    if let Err(e) = self.renderer.set_cursor_style(cursor_style) {
                        tracing::warn!("Failed to set cursor style: {e}");
                    }

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
            LAYOUT_CHANGED => {
                if let Ok(payload) =
                    serde_json::from_value::<LayoutChangedPayload>(notification.params)
                {
                    tracing::debug!(
                        "Layout changed: {:?}, windows: {}",
                        payload.kind,
                        payload.layout.window_count
                    );
                    self.state.layout = Some(payload.layout);
                    self.state.needs_redraw = true;
                    self.debug_log(&format!(
                        "Layout changed: {} windows",
                        self.state.layout.as_ref().map_or(0, |l| l.window_count)
                    ));
                }
            }
            OPTION_CHANGED => {
                // #445: Update cached options when server notifies of change
                if let Ok(payload) =
                    serde_json::from_value::<OptionChangedPayload>(notification.params)
                {
                    tracing::debug!(
                        "Option changed: {} = {:?} (window: {:?})",
                        payload.name,
                        payload.value,
                        payload.window_id
                    );
                    // Update the cached option value
                    self.state
                        .options
                        .insert(payload.name.clone(), payload.value.clone());
                    self.state.needs_redraw = true;
                    self.debug_log(&format!(
                        "Option changed: {} = {}",
                        payload.name, payload.value
                    ));
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
    /// Routes responses to pending CLI query requests for correlation.
    fn handle_response(&mut self, response: RpcResponse) {
        use super::{cli_executor::format_query_result, cli_panel::CliResult};

        // Check if this is a tracked CLI query
        if let Some(pending) = self.pending_requests.remove(&response.id) {
            let result = if let Some(ref error) = response.error {
                CliResult::Err(error.message.clone())
            } else if let Some(ref result) = response.result {
                // Format result based on command
                let formatted = format_query_result(&pending.command, result);
                CliResult::Ok(formatted)
            } else {
                CliResult::Err("No result returned".to_string())
            };

            // Update CLI history entry
            // Note: Index may be invalid if history was cleared
            let was_at_bottom = self.cli_panel.scroll_offset == 0;
            if self.cli_panel.update_result(pending.history_index, result) {
                // Only scroll to bottom if user was already at bottom
                if was_at_bottom {
                    self.cli_panel.scroll_offset = 0;
                }
                self.state.needs_redraw = true;
            }
            return;
        }

        // Existing error handling for non-query responses
        if let Some(error) = response.error {
            let msg = format!("RPC {}: {}", response.id, error.message);
            tracing::warn!("{}", msg);
            self.state.last_error = Some(msg);
            self.state.error_timestamp = Some(Instant::now());
            self.state.needs_redraw = true;
        }
    }

    /// Render the current screen state.
    async fn render(&mut self) -> Result<(), TuiError> {
        // Refresh screen using cell_grid format which includes decoration colors (#440)
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

        // Request screen content from server with cell_grid format for decoration colors (#440)
        let _ = self
            .rpc_writer
            .send_request("state/screen_content", json!({ "format": "cell_grid" }))
            .await;

        // Read response and write to frame buffer
        match tokio::time::timeout(
            std::time::Duration::from_millis(100),
            self.wait_for_screen_content(),
        )
        .await
        {
            Ok(Ok(content)) => {
                // Parse cell_grid JSON and write with decoration styles (#440)
                self.write_cell_grid_to_buffer(&content, width, height);
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                tracing::debug!("Screen content timeout, skipping render");
                // Return early - don't flush the cleared buffer which would blank the screen
                return Ok(());
            }
        }

        // Draw window borders for multi-window layouts (#444)
        self.draw_window_borders();

        // Calculate panel heights and positions
        // Always reserve 1 row for statusline (TUI renders statusline locally)
        let statusline_height: u16 = 1;
        let available_height = height.saturating_sub(statusline_height);

        // Write CLI panel to frame buffer (if visible)
        let cli_panel_height = if self.cli_panel.visible {
            let panel_height = available_height / 2;
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

    /// Draw window borders based on layout state (#444).
    ///
    /// Renders window separators for multi-window layouts. Each window's
    /// borders are drawn based on its bounds from `WireLayoutInfo`.
    ///
    /// Uses box drawing characters:
    /// - `│` for vertical separators
    /// - `─` for horizontal separators
    /// - `┼` for intersections
    fn draw_window_borders(&mut self) {
        use reovim_protocol::v1::WireZone;

        let Some(layout) = &self.state.layout else {
            return;
        };

        // Only draw borders if more than one window
        if layout.window_count <= 1 {
            return;
        }

        let (width, height) = self.last_size;
        if width == 0 || height == 0 {
            return;
        }

        // Reserve space for statusline (always present)
        let available_height = height.saturating_sub(1);

        // Style for window borders
        let border_style = Style::default().fg(reovim_driver_display::Color::DarkGrey);

        // Draw separator for each window that has a right or bottom neighbor
        for placement in &layout.windows {
            // Only draw borders for tiled windows
            if placement.zone != WireZone::Tiled || !placement.visible {
                continue;
            }

            let bounds = &placement.bounds;

            // Draw right border (vertical separator) if not at screen edge
            let right_edge = bounds.x + bounds.width;
            if right_edge < width {
                for y in bounds.y..bounds.y.saturating_add(bounds.height).min(available_height) {
                    self.frame_renderer
                        .buffer_mut()
                        .write_str(right_edge, y, "│", &border_style);
                }
            }

            // Draw bottom border (horizontal separator) if not at screen edge
            let bottom_edge = bounds.y + bounds.height;
            if bottom_edge < available_height {
                for x in bounds.x..bounds.x.saturating_add(bounds.width).min(width) {
                    self.frame_renderer
                        .buffer_mut()
                        .write_str(x, bottom_edge, "─", &border_style);
                }
            }

            // Draw intersection if both borders exist
            if right_edge < width && bottom_edge < available_height {
                self.frame_renderer.buffer_mut().write_str(
                    right_edge,
                    bottom_edge,
                    "┼",
                    &border_style,
                );
            }
        }

        // Highlight focused window border if exists
        if let Some(focused_id) = &layout.focused_window
            && let Some(focused) = layout.windows.iter().find(|w| &w.window_id == focused_id)
        {
            let focus_style = Style::default().fg(reovim_driver_display::Color::Blue);
            let bounds = &focused.bounds;

            // Draw a subtle focus indicator at the top-left of focused window
            if bounds.x < width && bounds.y < available_height {
                self.frame_renderer
                    .buffer_mut()
                    .write_str(bounds.x, bounds.y, "▪", &focus_style);
            }
        }
    }

    /// Write debug statusline to frame buffer.
    ///
    /// Write statusline to frame buffer (always rendered).
    ///
    /// Shows error in red if present, otherwise:
    /// - Debug mode: detailed status with timestamp, server, modules count
    /// - Normal mode: simple vim-like status with mode, filename, position
    fn write_statusline_to_buffer(&mut self) {
        let (width, height) = self.last_size;
        if width == 0 || height == 0 {
            return;
        }

        let statusline_y = height.saturating_sub(1);

        // If error exists, show in red
        let (line, style) = if let Some(ref err) = self.state.last_error {
            // Error statusline: red background
            let err_line = format!("ERR: {err}");
            let style = Style::default()
                .bg(reovim_driver_display::Color::Red)
                .fg(reovim_driver_display::Color::White);
            (err_line, style)
        } else if self.debug_config.is_some() {
            // Debug mode: detailed statusline
            let now = chrono::Local::now();
            let timestamp = now.format("%y-%m-%d %H:%M:%S %Z").to_string();
            let server = &self.server_address;
            let mode = self.state.mode_display.as_deref().unwrap_or("?");
            let modules_count = self.state.modules.len();
            let last_key = self.state.last_key.as_deref().unwrap_or("-");
            let cursor = format!("{}:{}", self.state.cursor_line + 1, self.state.cursor_column + 1);
            let prefix_indicator = if self.prefix_mode { "^B-" } else { "" };

            let line = format!(
                "{prefix_indicator}{timestamp}|{server}|{mode}|k:{last_key}|c:{cursor}|m:{modules_count}"
            );
            let style = Style::default().reverse();
            (line, style)
        } else {
            // Normal mode: simple vim-like statusline
            // Format: " MODE  [No Name]              1:1 "
            let mode = self.state.mode_display.as_deref().unwrap_or("NORMAL");
            let modified = if self.state.buffer_modified {
                " [+]"
            } else {
                ""
            };
            let filename = "[No Name]"; // TODO: Add filename to TuiState when available
            let line_num = self.state.cursor_line + 1; // 1-indexed display
            let col_num = self.state.cursor_column + 1; // 1-indexed display

            let left = format!(" {mode}  {filename}{modified}");
            let right = format!("{line_num}:{col_num} ");

            // Build full line with padding
            let padding_width = (width as usize).saturating_sub(left.len() + right.len());
            let line = format!("{left}{:padding_width$}{right}", "");

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
    /// Delegates to `render_core::build_frame_content` which can be shared
    /// with `HeadlessClient`.
    fn build_frame_content(&self) -> String {
        let render_state = self.to_render_state();
        render_core::build_frame_content(
            &render_state,
            self.frame_renderer.buffer(),
            ScreenFormat::RawAnsi,
        )
    }

    /// Convert current TUI state to a `RenderState` for frame capture.
    fn to_render_state(&self) -> RenderState {
        RenderState {
            width: self.last_size.0,
            height: self.last_size.1,
            mode_display: self.state.mode_display.clone(),
            cursor_line: self.state.cursor_line,
            cursor_column: self.state.cursor_column,
            modules: self.state.modules.clone(),
            server_address: self.server_address.clone(),
            log_panel_visible: self.log_panel.visible,
        }
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
    ///
    /// Routes non-screen responses to `handle_response()` for CLI query correlation.
    async fn wait_for_screen_content(&mut self) -> Result<String, TuiError> {
        while let Some(msg) = self.message_rx.recv().await {
            match msg {
                ServerMessage::Response(response) => {
                    // Check if this is the screen content response
                    if let Some(ref result) = response.result
                        && let Some(content) = result.get("content").and_then(|v| v.as_str())
                    {
                        return Ok(content.to_string());
                    }
                    // Not screen content - route to handle_response for CLI query correlation
                    // and continue waiting for the actual screen content response
                    self.handle_response(response);
                    // Continue the loop to wait for the screen content response
                }
                ServerMessage::Notification(notification) => {
                    // Handle notifications while waiting
                    self.handle_notification(notification).await;
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

    /// Write `cell_grid` content to frame buffer with decoration colors (#440).
    ///
    /// Parses the JSON cell grid format and applies per-cell styles for
    /// rainbow brackets and other decorations.
    fn write_cell_grid_to_buffer(&mut self, content: &str, width: u16, height: u16) {
        // Try to parse as JSON cell grid
        let cells: Result<Vec<Vec<serde_json::Value>>, _> = serde_json::from_str(content);

        if let Ok(rows) = cells {
            // Write each cell with its decoration style
            for (y, row) in rows.iter().enumerate().take(height as usize) {
                #[allow(clippy::cast_possible_truncation)]
                let y_u16 = y as u16;

                for (x, cell) in row.iter().enumerate().take(width as usize) {
                    let ch = cell
                        .get("char")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.chars().next())
                        .unwrap_or(' ');

                    let style = render_core::parse_cell_style(cell);

                    #[allow(clippy::cast_possible_truncation)]
                    let x_u16 = x as u16;
                    self.frame_renderer
                        .buffer_mut()
                        .put_char(x_u16, y_u16, ch, &style);
                }
            }
        } else {
            // Fallback to plain text if JSON parsing fails
            let default_style = Style::default();
            for (y, line) in content.lines().enumerate().take(height as usize) {
                #[allow(clippy::cast_possible_truncation)]
                let y_u16 = y as u16;
                self.frame_renderer
                    .buffer_mut()
                    .write_str(0, y_u16, line, &default_style);
            }
        }
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
        use reovim_protocol::v1::BufferId as ProtocolBufferId;
        let params = json!({
            "buffer_id": 1,
            "position": { "line": 10, "column": 5 }
        });
        let payload: CursorMovedPayload = serde_json::from_value(params).unwrap();
        assert_eq!(payload.buffer_id, ProtocolBufferId::from(1));
        assert_eq!(payload.position.line, 10);
        assert_eq!(payload.position.column, 5);
    }

    #[test]
    fn test_buffer_modified_payload_parsing() {
        use reovim_protocol::v1::BufferId as ProtocolBufferId;
        let params = json!({
            "buffer_id": 1,
            "modified": true
        });
        let payload: BufferModifiedPayload = serde_json::from_value(params).unwrap();
        assert_eq!(payload.buffer_id, ProtocolBufferId::from(1));
        assert!(payload.modified);
    }

    #[test]
    fn test_render_complete_payload_parsing() {
        let params = json!({});
        let payload: RenderCompletePayload = serde_json::from_value(params).unwrap();
        // Just verify it parses - payload is currently empty
        let _ = payload;
    }

    // === Option notification tests (#445) ===

    #[test]
    fn test_option_changed_payload_parsing_global() {
        let params = json!({
            "name": "number",
            "value": true
        });
        let payload: OptionChangedPayload = serde_json::from_value(params).unwrap();
        assert_eq!(payload.name, "number");
        assert_eq!(payload.value, json!(true));
        assert!(payload.window_id.is_none());
    }

    #[test]
    fn test_option_changed_payload_parsing_window() {
        let params = json!({
            "name": "relativenumber",
            "value": false,
            "window_id": 42
        });
        let payload: OptionChangedPayload = serde_json::from_value(params).unwrap();
        assert_eq!(payload.name, "relativenumber");
        assert_eq!(payload.value, json!(false));
        assert_eq!(payload.window_id, Some(42));
    }

    #[test]
    fn test_tui_state_options_default_empty() {
        let state = TuiState::default();
        assert!(state.options.is_empty());
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
