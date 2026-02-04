//! Headless TUI application for testing and scripting.
//!
//! This module provides a headless TUI that processes notifications
//! identically to the interactive TUI but without terminal I/O.
//! Used for E2E testing and CI environments.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  TuiAppV2Headless (this module)                             │
//! │    - No terminal I/O                                        │
//! │    - Internal frame buffer                                  │
//! │    - Programmatic capture/resize/keys                       │
//! ├─────────────────────────────────────────────────────────────┤
//! │  TuiGrpcClient (grpc_client.rs)                  TRANSPORT  │
//! │    - gRPC v2 service calls                                  │
//! │    - Streaming notifications                                │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Server (lib/server/)                            MECHANISM  │
//! │    - Raw buffer content                                     │
//! │    - Layout, mode, cursor state                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let mut tui = TuiAppV2Headless::connect("127.0.0.1:50051").await?;
//!
//! // Start notification processing
//! tui.start().await?;
//!
//! // Send keys programmatically
//! tui.send_keys("ihello<Esc>").await?;
//!
//! // Resize viewport
//! tui.resize(120, 40).await?;
//!
//! // Capture current frame
//! let frame = tui.capture(ScreenFormat::PlainText).await?;
//! println!("{}", frame);
//!
//! // Stop the event loop
//! tui.stop();
//! ```

use std::collections::HashMap;

use {
    reovim_driver_display::FrameBuffer,
    reovim_protocol::{
        v1::ScreenFormat,
        v2::{
            GetLayoutResponse, Notification, WindowInfo, WindowNode, WindowRect,
            notification::Payload,
        },
    },
    tokio::{
        select,
        sync::{mpsc, oneshot},
        time::{Duration, interval},
    },
    tonic::Streaming,
};

use crate::{
    grpc_client::{TuiGrpcClient, TuiGrpcError},
    render_core::{RenderState, build_frame_content},
};

/// Headless TUI application error.
#[derive(Debug)]
pub enum HeadlessError {
    /// gRPC error.
    Grpc(TuiGrpcError),
    /// Server disconnected.
    Disconnected,
    /// Notification stream ended.
    StreamEnded,
    /// Event loop not running.
    NotRunning,
    /// Capture request failed.
    CaptureError(String),
}

impl std::fmt::Display for HeadlessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Grpc(e) => write!(f, "gRPC error: {e}"),
            Self::Disconnected => write!(f, "Server disconnected"),
            Self::StreamEnded => write!(f, "Notification stream ended"),
            Self::NotRunning => write!(f, "Headless TUI event loop not running"),
            Self::CaptureError(msg) => write!(f, "Capture failed: {msg}"),
        }
    }
}

impl std::error::Error for HeadlessError {}

impl From<TuiGrpcError> for HeadlessError {
    fn from(e: TuiGrpcError) -> Self {
        Self::Grpc(e)
    }
}

/// Request sent to the headless event loop.
#[derive(Debug)]
enum HeadlessRequest {
    /// Capture the current frame.
    Capture {
        format: ScreenFormat,
        response: oneshot::Sender<Result<String, HeadlessError>>,
    },
    /// Resize the viewport.
    Resize {
        width: u16,
        height: u16,
        response: oneshot::Sender<Result<(), HeadlessError>>,
    },
    /// Send keys to the server.
    SendKeys {
        keys: String,
        response: oneshot::Sender<Result<bool, HeadlessError>>,
    },
    /// Stop the event loop.
    Stop,
}

/// Metadata about a captured frame.
#[derive(Debug, Clone, Default)]
pub struct FrameMetadata {
    /// Frame capture timestamp (ISO 8601).
    pub timestamp: String,
    /// Current mode name.
    pub mode_name: String,
    /// Mode display string.
    pub mode_display: String,
    /// Cursor line (0-indexed).
    pub cursor_line: u64,
    /// Cursor column (0-indexed).
    pub cursor_col: u64,
    /// Viewport width.
    pub width: u16,
    /// Viewport height.
    pub height: u16,
    /// Number of windows.
    pub window_count: usize,
}

/// Headless TUI state (mirrors interactive TUI state).
#[derive(Debug, Default)]
struct HeadlessState {
    /// This client's unique ID (#471).
    ///
    /// Used for per-client state isolation. All `send_keys` requests must
    /// include this ID for proper mode/cursor isolation.
    my_client_id: u64,
    /// Current mode name (internal).
    mode_name: String,
    /// Current mode display string.
    mode_display: String,
    /// Whether mode accepts text input.
    is_insert_mode: bool,
    /// Cursor line (0-indexed).
    cursor_line: u64,
    /// Cursor column (0-indexed).
    cursor_col: u64,
    /// Focused window ID.
    focused_window_id: u64,
    /// Window layout info.
    windows: Vec<WindowInfo>,
    /// Buffer content cache (`buffer_id` -> lines).
    buffer_cache: HashMap<u64, Vec<String>>,
    /// Viewport width.
    width: u16,
    /// Viewport height.
    height: u16,
    /// Whether client needs to create a default window (empty server layout).
    needs_default_window: bool,
}

/// Headless TUI application handle.
///
/// This struct provides the user-facing API for controlling the headless TUI.
/// The actual event loop runs in a background task.
pub struct TuiAppV2Headless {
    /// Channel to send requests to the event loop.
    request_tx: mpsc::Sender<HeadlessRequest>,
    /// Server address for metadata.
    server_address: String,
}

impl TuiAppV2Headless {
    /// Connect to a gRPC server and create a headless TUI.
    ///
    /// The TUI is not started automatically. Call [`start`](Self::start) to
    /// begin processing notifications.
    ///
    /// Uses default viewport size of 160x48 (realistic terminal dimensions).
    ///
    /// # Arguments
    ///
    /// * `addr` - Server address in `host:port` format.
    ///
    /// # Errors
    ///
    /// Returns an error if connection fails.
    pub async fn connect(addr: &str) -> Result<Self, HeadlessError> {
        Self::connect_with_size(addr, 160, 48).await
    }

    /// Connect with a specific initial viewport size.
    ///
    /// # Arguments
    ///
    /// * `addr` - Server address in `host:port` format.
    /// * `width` - Initial viewport width.
    /// * `height` - Initial viewport height.
    ///
    /// # Errors
    ///
    /// Returns an error if connection fails.
    #[allow(clippy::significant_drop_tightening)] // event_loop is moved into spawn, not dropped
    pub async fn connect_with_size(
        addr: &str,
        width: u16,
        height: u16,
    ) -> Result<Self, HeadlessError> {
        // Connect gRPC client
        let mut client = TuiGrpcClient::connect(addr).await?;

        // Per-client state (#471): Join presence to get unique client_id for per-client state isolation.
        // The client_id is required for proper mode/cursor isolation in multi-client scenarios.
        let join_response = client
            .presence_join("headless", "Headless TUI")
            .await
            .map_err(|e| HeadlessError::CaptureError(format!("Failed to join presence: {e}")))?;
        let my_client_id = join_response.client_id;
        tracing::debug!(client_id = my_client_id, "Headless TUI joined presence");

        // Subscribe to all notifications
        let notification_stream = client.subscribe_all().await?;

        // TUI knows its own size - no need to tell the server anything.
        // Server has no screen. If CLI wants to resize this TUI, it sends
        // a resize command through the server which relays it here.

        // Create request channel
        let (request_tx, request_rx) = mpsc::channel(32);

        // Create initial state with client_id
        let state = HeadlessState {
            my_client_id,
            width,
            height,
            ..Default::default()
        };

        // Create frame buffer
        let frame_buffer = FrameBuffer::new(width, height);

        // Spawn event loop task
        let server_address = addr.to_string();
        let event_loop = HeadlessEventLoop {
            client,
            notification_stream,
            request_rx,
            state,
            frame_buffer,
            server_address: server_address.clone(),
            running: true,
        };

        tokio::spawn(event_loop.run());

        Ok(Self {
            request_tx,
            server_address,
        })
    }

    /// Capture the current frame buffer content.
    ///
    /// # Arguments
    ///
    /// * `format` - Output format (plain text or ANSI colored).
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop is not running or capture fails.
    pub async fn capture(&self, format: ScreenFormat) -> Result<String, HeadlessError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(HeadlessRequest::Capture {
                format,
                response: response_tx,
            })
            .await
            .map_err(|_| HeadlessError::NotRunning)?;

        response_rx.await.map_err(|_| HeadlessError::NotRunning)?
    }

    /// Resize the viewport.
    ///
    /// # Arguments
    ///
    /// * `width` - New viewport width.
    /// * `height` - New viewport height.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop is not running.
    pub async fn resize(&self, width: u16, height: u16) -> Result<(), HeadlessError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(HeadlessRequest::Resize {
                width,
                height,
                response: response_tx,
            })
            .await
            .map_err(|_| HeadlessError::NotRunning)?;

        response_rx.await.map_err(|_| HeadlessError::NotRunning)?
    }

    /// Send keys to the server.
    ///
    /// # Arguments
    ///
    /// * `keys` - Keys in vim notation (e.g., "iHello<Esc>").
    ///
    /// # Returns
    ///
    /// Returns `true` if keys were processed by the server.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop is not running or send fails.
    pub async fn send_keys(&self, keys: &str) -> Result<bool, HeadlessError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(HeadlessRequest::SendKeys {
                keys: keys.to_string(),
                response: response_tx,
            })
            .await
            .map_err(|_| HeadlessError::NotRunning)?;

        response_rx.await.map_err(|_| HeadlessError::NotRunning)?
    }

    /// Stop the headless TUI event loop.
    ///
    /// After calling this, the TUI will no longer process notifications.
    pub async fn stop(&self) {
        let _ = self.request_tx.send(HeadlessRequest::Stop).await;
    }

    /// Get the server address this TUI is connected to.
    #[must_use]
    pub fn server_address(&self) -> &str {
        &self.server_address
    }

    /// Wait for a notification that matches the predicate.
    ///
    /// This is useful for testing to wait for specific state changes.
    /// Polls every 10ms up to the timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait.
    /// * `predicate` - Function to check if the captured state matches.
    ///
    /// # Returns
    ///
    /// Returns the captured frame if predicate matches within timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if timeout expires or capture fails.
    pub async fn wait_for<F>(
        &self,
        timeout: Duration,
        predicate: F,
    ) -> Result<String, HeadlessError>
    where
        F: Fn(&str) -> bool,
    {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(10);

        while start.elapsed() < timeout {
            let frame = self.capture(ScreenFormat::PlainText).await?;
            if predicate(&frame) {
                return Ok(frame);
            }
            tokio::time::sleep(poll_interval).await;
        }

        Err(HeadlessError::CaptureError("Timeout waiting for predicate".to_string()))
    }
}

/// Internal event loop for the headless TUI.
struct HeadlessEventLoop {
    /// gRPC client.
    client: TuiGrpcClient,
    /// Notification stream.
    notification_stream: Streaming<Notification>,
    /// Channel to receive requests.
    request_rx: mpsc::Receiver<HeadlessRequest>,
    /// Current state.
    state: HeadlessState,
    /// Frame buffer for rendering.
    frame_buffer: FrameBuffer,
    /// Server address.
    server_address: String,
    /// Whether the loop is running.
    running: bool,
}

impl HeadlessEventLoop {
    /// Run the event loop until stopped.
    async fn run(mut self) {
        // Fetch initial state
        if let Err(e) = self.fetch_initial_state().await {
            tracing::error!("Failed to fetch initial state: {e}");
            return;
        }

        // Render initial frame to buffer
        self.render_to_buffer();

        // Event loop
        let mut tick = interval(Duration::from_millis(50));

        while self.running {
            select! {
                // Handle requests from the API
                Some(request) = self.request_rx.recv() => {
                    self.handle_request(request).await;
                }

                // Handle server notifications
                notification = self.notification_stream.message() => {
                    match notification {
                        Ok(Some(notif)) => {
                            self.handle_notification(notif).await;
                        }
                        Ok(None) => {
                            tracing::info!("Notification stream ended");
                            self.running = false;
                        }
                        Err(e) => {
                            tracing::error!("Notification error: {e}");
                            self.running = false;
                        }
                    }
                }

                // Periodic tick for any maintenance
                _ = tick.tick() => {
                    // Render to buffer periodically to ensure it's up to date
                    self.render_to_buffer();
                }
            }
        }

        tracing::debug!("Headless TUI event loop stopped");
    }

    /// Fetch initial state from server.
    ///
    /// Uses fail-open strategy: mode is required (panic if fails), but layout
    /// and buffer content are optional. This ensures the event loop starts
    /// even when some RPCs (like `GetLayout`) are not yet implemented.
    ///
    /// # Phase #479: Fail-Loud Policy
    ///
    /// Mode is required for statusline and uses `get_mode_or_panic()`.
    /// After `presence_join()` succeeds (which assigns `my_client_id`),
    /// mode lookup should always work - failure indicates a bug.
    async fn fetch_initial_state(&mut self) -> Result<(), HeadlessError> {
        // Phase #479: Use my_client_id (assigned by presence_join in connect())
        let client_id = self.state.my_client_id;

        // Mode is required for statusline - panic if fails (should never happen after join)
        let mode_resp = self.client.get_mode_or_panic(client_id).await;
        self.state.mode_name = mode_resp.name;
        self.state.mode_display = mode_resp.display;
        self.state.is_insert_mode = mode_resp.is_insert;

        // Cursor is optional (may fail if no buffer exists)
        match self.client.get_cursor_for_client(None, client_id).await {
            Ok(cursor_resp) => {
                if let Some(pos) = cursor_resp.position {
                    self.state.cursor_line = pos.line;
                    self.state.cursor_col = pos.column;
                }
            }
            Err(e) => tracing::warn!("Cursor not available during init: {e}"),
        }

        // Layout is optional (not yet implemented in gRPC server)
        match self.client.get_layout_for_client(client_id).await {
            Ok(layout_resp) => self.apply_layout(&layout_resp),
            Err(e) => tracing::warn!("Layout not available during init: {e}"),
        }

        // Handle empty layout - create default local window
        if self.state.needs_default_window
            && let Ok(active_buffer_resp) = self.client.get_active_buffer().await
            && let Some(buffer_id) = active_buffer_resp.buffer_id
        {
            self.create_default_window(buffer_id);
            tracing::info!(buffer_id, "Created default window for empty server layout");
        }

        // Buffer content depends on having windows from layout
        if !self.state.windows.is_empty()
            && let Err(e) = self.fetch_buffer_contents().await
        {
            tracing::warn!("Buffer content not available during init: {e}");
        }

        Ok(())
    }

    /// Apply layout response to state.
    fn apply_layout(&mut self, layout: &GetLayoutResponse) {
        // Phase #479: focused_window_id is now Option<u64> to eliminate ID ambiguity
        self.state.focused_window_id = layout.focused_window_id.unwrap_or(0);

        // Flatten window tree to list
        self.state.windows.clear();
        if let Some(root) = &layout.root {
            self.collect_windows(root);
        }

        // Mark if we need to create a default window
        self.state.needs_default_window = self.state.windows.is_empty();
    }

    /// Create a default window view for empty layout.
    fn create_default_window(&mut self, buffer_id: u64) {
        let content_height = self.state.height.saturating_sub(1);

        let window = WindowInfo {
            window_id: 1,
            // Phase #479: buffer_id is now Option<u64>
            buffer_id: Some(buffer_id),
            rect: Some(WindowRect {
                x: 0,
                y: 0,
                width: u64::from(self.state.width),
                height: u64::from(content_height),
            }),
            focused: true,
        };

        self.state.windows.push(window);
        self.state.focused_window_id = 1;
        self.state.needs_default_window = false;

        tracing::debug!(buffer_id, "Created default window for empty layout (headless)");
    }

    /// Recursively collect windows from layout tree.
    fn collect_windows(&mut self, node: &WindowNode) {
        if let Some(n) = &node.node {
            match n {
                reovim_protocol::v2::window_node::Node::Leaf(leaf) => {
                    self.state.windows.push(WindowInfo {
                        window_id: leaf.window_id,
                        buffer_id: leaf.buffer_id,
                        rect: leaf.rect,
                        focused: leaf.window_id == self.state.focused_window_id,
                    });
                }
                reovim_protocol::v2::window_node::Node::Split(split) => {
                    for child in &split.children {
                        self.collect_windows(child);
                    }
                }
            }
        }
    }

    /// Fetch buffer content for all visible windows.
    async fn fetch_buffer_contents(&mut self) -> Result<(), HeadlessError> {
        // Phase #479: buffer_id is now Option<u64>, filter out None values
        let buffer_ids: Vec<u64> = self
            .state
            .windows
            .iter()
            .filter_map(|w| w.buffer_id)
            .collect();

        for buffer_id in buffer_ids {
            if !self.state.buffer_cache.contains_key(&buffer_id) {
                let content = self
                    .client
                    .get_buffer_content(Some(buffer_id), None, None)
                    .await?;
                self.state.buffer_cache.insert(buffer_id, content.lines);
            }
        }

        Ok(())
    }

    /// Handle a request from the API.
    async fn handle_request(&mut self, request: HeadlessRequest) {
        match request {
            HeadlessRequest::Capture { format, response } => {
                let result = self.capture_frame(format);
                let _ = response.send(Ok(result));
            }
            HeadlessRequest::Resize {
                width,
                height,
                response,
            } => {
                let result = self.do_resize(width, height).await;
                let _ = response.send(result);
            }
            HeadlessRequest::SendKeys { keys, response } => {
                let result = self.do_send_keys(&keys).await;
                let _ = response.send(result);
            }
            HeadlessRequest::Stop => {
                self.running = false;
            }
        }
    }

    /// Capture the current frame buffer.
    fn capture_frame(&self, format: ScreenFormat) -> String {
        let render_state = self.build_render_state();
        build_frame_content(&render_state, &self.frame_buffer, format)
    }

    /// Build render state from current headless state.
    #[allow(clippy::cast_possible_truncation)]
    fn build_render_state(&self) -> RenderState {
        use reovim_protocol::v1::notifications::WireCmdlinePrompt;

        RenderState {
            width: self.state.width,
            height: self.state.height,
            mode_display: Some(self.state.mode_display.clone()),
            cursor_line: self.state.cursor_line as usize,
            cursor_column: self.state.cursor_col as usize,
            modules: vec![],
            server_address: self.server_address.clone(),
            log_panel_visible: false,
            cmdline_visible: false,
            cmdline_prompt: WireCmdlinePrompt::default(),
            cmdline_input: String::new(),
            cmdline_cursor: 0,
        }
    }

    /// Resize the viewport.
    async fn do_resize(&mut self, width: u16, height: u16) -> Result<(), HeadlessError> {
        self.state.width = width;
        self.state.height = height;
        self.frame_buffer = FrameBuffer::new(width, height);

        // Notify server
        self.client
            .resize(u64::from(width), u64::from(height))
            .await?;

        // Re-render
        self.render_to_buffer();

        Ok(())
    }

    /// Send keys to the server.
    async fn do_send_keys(&mut self, keys: &str) -> Result<bool, HeadlessError> {
        // Per-client state (#471): Use client_id for per-client state isolation.
        // Without this, all headless TUIs would share mode/cursor state.
        let response = self
            .client
            .send_keys_with_client(keys, self.state.my_client_id)
            .await?;
        Ok(response.ok)
    }

    /// Handle server notification.
    async fn handle_notification(&mut self, notif: Notification) {
        if let Some(payload) = notif.payload {
            match payload {
                Payload::ModeChanged(mode) => {
                    self.state.mode_name = mode.name;
                    self.state.mode_display = mode.display;
                    self.state.is_insert_mode = mode.is_insert;
                }
                Payload::CursorMoved(cursor) => {
                    if let Some(pos) = cursor.position {
                        self.state.cursor_line = pos.line;
                        self.state.cursor_col = pos.column;
                    }
                }
                Payload::BufferModified(buf) => {
                    // Invalidate cache and refetch
                    self.state.buffer_cache.remove(&buf.buffer_id);
                    if let Ok(content) = self
                        .client
                        .get_buffer_content(Some(buf.buffer_id), None, None)
                        .await
                    {
                        self.state.buffer_cache.insert(buf.buffer_id, content.lines);
                    }
                }
                Payload::LayoutChanged(layout) => {
                    // Phase #479: focused_window_id is now Option<u64>
                    self.state.focused_window_id = layout.focused_window_id.unwrap_or(0);
                    self.state.windows = layout.windows;
                }
                Payload::Detach(detach) => {
                    tracing::info!("Server requested detach: {}", detach.reason);
                    self.running = false;
                }
                Payload::ResizeRequest(resize_req) => {
                    // Handle CLI -> Server -> TUI resize relay
                    #[allow(clippy::cast_possible_truncation)]
                    let width = resize_req.width as u16;
                    #[allow(clippy::cast_possible_truncation)]
                    let height = resize_req.height as u16;

                    if width > 0 && height > 0 {
                        tracing::debug!(width, height, "Resize request from CLI");
                        self.state.width = width;
                        self.state.height = height;
                        self.frame_buffer = FrameBuffer::new(width, height);
                    }
                }
                Payload::CaptureRequest(capture_req) => {
                    // Handle CLI→Server→TUI capture request relay
                    tracing::debug!(
                        request_id = capture_req.request_id,
                        format = %capture_req.format,
                        "Received capture request"
                    );

                    // Convert format string to ScreenFormat
                    let screen_format = match capture_req.format.as_str() {
                        "plain_text" => ScreenFormat::PlainText,
                        "cell_grid" => ScreenFormat::CellGrid,
                        _ => ScreenFormat::RawAnsi, // Default to raw_ansi
                    };

                    // Capture the frame
                    let content = self.capture_frame(screen_format);

                    // Submit the response back to server
                    let result = self
                        .client
                        .submit_capture_response(
                            capture_req.request_id,
                            u64::from(self.state.width),
                            u64::from(self.state.height),
                            &capture_req.format,
                            content,
                        )
                        .await;

                    match result {
                        Ok(reply) => {
                            tracing::debug!(
                                request_id = capture_req.request_id,
                                ok = reply.ok,
                                "Submitted capture response"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                request_id = capture_req.request_id,
                                error = %e,
                                "Failed to submit capture response"
                            );
                        }
                    }
                }
                _ => {
                    // Other notifications - just trigger a re-render
                }
            }
        }

        // Re-render after any notification
        self.render_to_buffer();
    }

    /// Render current state to the frame buffer.
    fn render_to_buffer(&mut self) {
        use reovim_driver_display::Style;

        let (width, height) = (self.state.width, self.state.height);
        if width == 0 || height == 0 {
            return;
        }

        // Clear buffer
        self.frame_buffer = FrameBuffer::new(width, height);

        let default_style = Style::default();

        // Render each window
        for window in &self.state.windows {
            let Some(rect) = &window.rect else { continue };

            #[allow(clippy::cast_possible_truncation)]
            let x = rect.x as u16;
            #[allow(clippy::cast_possible_truncation)]
            let y = rect.y as u16;
            #[allow(clippy::cast_possible_truncation)]
            let w = rect.width as u16;
            #[allow(clippy::cast_possible_truncation)]
            let h = rect.height as u16;

            // Get buffer content (Phase #479: buffer_id is Option<u64>)
            let lines = window
                .buffer_id
                .and_then(|bid| self.state.buffer_cache.get(&bid));
            if let Some(lines) = lines {
                for (row, line) in lines.iter().enumerate().take(h as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let screen_y = y + row as u16;

                    // Truncate line to width
                    let display_line: String = line.chars().take(w as usize).collect();
                    self.frame_buffer
                        .write_str(x, screen_y, &display_line, &default_style);
                }

                // Render empty lines past EOF with tilde
                for row in lines.len()..(h as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let screen_y = y + row as u16;
                    self.frame_buffer.put_char(x, screen_y, '~', &default_style);
                }
            } else {
                // No buffer content - fill with tildes
                for row in 0..(h as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let screen_y = y + row as u16;
                    self.frame_buffer.put_char(x, screen_y, '~', &default_style);
                }
            }
        }

        // Render statusline at bottom
        self.render_statusline();
    }

    /// Render statusline to frame buffer.
    fn render_statusline(&mut self) {
        use reovim_driver_display::Style;

        let height = self.state.height;
        let width = self.state.width;
        if height == 0 {
            return;
        }

        let status_y = height - 1;
        let style = Style::default().reverse();

        // Build statusline content
        let mode = &self.state.mode_display;
        let cursor = format!("{}:{}", self.state.cursor_line + 1, self.state.cursor_col + 1);
        let left = format!(" {mode} ");
        let right = format!(" {cursor} | {} ", self.server_address);

        // Fill with spaces
        let fill_width = width as usize;
        let fill = " ".repeat(fill_width);
        self.frame_buffer.write_str(0, status_y, &fill, &style);

        // Write left and right parts
        self.frame_buffer.write_str(0, status_y, &left, &style);

        #[allow(clippy::cast_possible_truncation)]
        let right_x = width.saturating_sub(right.len() as u16);
        self.frame_buffer
            .write_str(right_x, status_y, &right, &style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headless_error_display() {
        let err = HeadlessError::Disconnected;
        assert_eq!(err.to_string(), "Server disconnected");

        let err = HeadlessError::StreamEnded;
        assert_eq!(err.to_string(), "Notification stream ended");

        let err = HeadlessError::NotRunning;
        assert_eq!(err.to_string(), "Headless TUI event loop not running");
    }

    #[test]
    fn test_headless_state_default() {
        let state = HeadlessState::default();
        assert!(state.mode_name.is_empty());
        assert!(state.mode_display.is_empty());
        assert!(!state.is_insert_mode);
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 0);
        assert_eq!(state.width, 0);
        assert_eq!(state.height, 0);
    }

    #[test]
    fn test_frame_metadata_default() {
        let meta = FrameMetadata::default();
        assert!(meta.timestamp.is_empty());
        assert!(meta.mode_name.is_empty());
        assert_eq!(meta.cursor_line, 0);
        assert_eq!(meta.width, 0);
    }
}
