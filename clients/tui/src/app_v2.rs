//! gRPC v2 TUI application.
//!
//! A clean TUI implementation using only gRPC v2 protocol.
//! Follows mechanism/policy separation - server provides raw data,
//! client renders locally.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  TuiAppV2 (this module)                          POLICY     │
//! │    - Client-side rendering decisions                        │
//! │    - Layout and styling                                     │
//! ├─────────────────────────────────────────────────────────────┤
//! │  TUI Driver (lib/drivers/tui/)                   MECHANISM  │
//! │    - Terminal session (raw mode, alternate screen)          │
//! │    - Input event stream                                     │
//! │    - Screen rendering primitives                            │
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

use std::{collections::HashMap, io, time::Duration};

use {
    crossterm::event::{KeyCode, KeyModifiers},
    reovim_arch::Color,
    reovim_driver_tui::{Cursor, CursorStyle, InputEvent, InputReader, Screen, Style, Terminal},
    reovim_protocol::v2::{
        GetLayoutResponse, Notification, WindowInfo, WindowNode, notification::Payload,
    },
    tokio::{select, time::interval},
    tonic::Streaming,
};

use crate::{
    TuiDebugConfig,
    grpc_client::{TuiGrpcClient, TuiGrpcError},
};

/// Line number display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineNumberMode {
    /// No line numbers.
    #[default]
    None,
    /// Absolute line numbers (1, 2, 3...).
    Absolute,
    /// Relative line numbers (distance from cursor).
    Relative,
    /// Hybrid: absolute for cursor line, relative for others.
    Hybrid,
}

/// TUI application error.
#[derive(Debug)]
pub enum TuiAppV2Error {
    /// I/O error.
    Io(io::Error),
    /// gRPC error.
    Grpc(TuiGrpcError),
    /// Server disconnected.
    Disconnected,
    /// Stream ended.
    StreamEnded,
}

impl std::fmt::Display for TuiAppV2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Grpc(e) => write!(f, "gRPC error: {e}"),
            Self::Disconnected => write!(f, "Server disconnected"),
            Self::StreamEnded => write!(f, "Notification stream ended"),
        }
    }
}

impl std::error::Error for TuiAppV2Error {}

impl From<io::Error> for TuiAppV2Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<TuiGrpcError> for TuiAppV2Error {
    fn from(e: TuiGrpcError) -> Self {
        Self::Grpc(e)
    }
}

/// TUI state tracked locally from server notifications.
#[derive(Debug, Default)]
struct TuiState {
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
    /// Whether screen needs redraw.
    needs_redraw: bool,
    /// Last error message for statusline.
    last_error: Option<String>,
    /// Buffer content cache (`buffer_id` -> lines).
    buffer_cache: HashMap<u64, Vec<String>>,
    /// Line number display mode.
    line_number_mode: LineNumberMode,
    /// Per-window viewport scroll (`window_id` -> `top_line`).
    /// TODO: Implement viewport scrolling per window
    #[allow(dead_code)]
    viewport_scroll: HashMap<u64, usize>,
}

/// gRPC v2 TUI application.
///
/// Uses streaming notifications for real-time updates and
/// client-side rendering for full policy control.
pub struct TuiAppV2 {
    /// gRPC client for server communication.
    client: TuiGrpcClient,
    /// Notification stream from server.
    notification_stream: Streaming<Notification>,
    /// Terminal session.
    terminal: Terminal,
    /// Screen buffer and renderer.
    screen: Screen,
    /// Cursor manager.
    cursor: Cursor,
    /// Input event reader.
    input: InputReader,
    /// Current TUI state.
    state: TuiState,
    /// Whether the app is running.
    running: bool,
    /// Server address for display.
    server_address: String,
    /// Debug configuration.
    #[allow(dead_code)]
    debug_config: Option<TuiDebugConfig>,
}

impl TuiAppV2 {
    /// Connect to a gRPC server and create a new TUI app.
    ///
    /// # Arguments
    ///
    /// * `addr` - Server address in `host:port` format.
    /// * `debug_config` - Optional debug configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if connection fails.
    pub async fn connect(
        addr: &str,
        debug_config: Option<TuiDebugConfig>,
    ) -> Result<Self, TuiAppV2Error> {
        // Connect gRPC client
        let mut client = TuiGrpcClient::connect(addr).await?;

        // Subscribe to all notifications
        let notification_stream = client.subscribe_all().await?;

        // Enter terminal session
        let terminal = Terminal::enter()?;

        // Get terminal size
        let (width, height) = Terminal::size()?;

        // Notify server of viewport size
        client.resize(u64::from(width), u64::from(height)).await?;

        // Create screen and cursor
        let screen = Screen::new(width, height);
        let cursor = Cursor::new();
        let input = InputReader::new();

        Ok(Self {
            client,
            notification_stream,
            terminal,
            screen,
            cursor,
            input,
            state: TuiState::default(),
            running: true,
            server_address: addr.to_string(),
            debug_config,
        })
    }

    /// Run the TUI event loop.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop fails.
    pub async fn run(&mut self) -> Result<(), TuiAppV2Error> {
        // Initial state fetch
        self.fetch_initial_state().await?;

        // Initial render
        self.state.needs_redraw = true;
        self.render()?;

        // Event loop
        self.event_loop().await
    }

    /// Fetch initial state from server.
    async fn fetch_initial_state(&mut self) -> Result<(), TuiAppV2Error> {
        // Get mode
        let mode_resp = self.client.get_mode().await?;
        self.state.mode_name = mode_resp.name;
        self.state.mode_display = mode_resp.display;
        self.state.is_insert_mode = mode_resp.is_insert;

        // Get cursor
        let cursor_resp = self.client.get_cursor(None).await?;
        if let Some(pos) = cursor_resp.position {
            self.state.cursor_line = pos.line;
            self.state.cursor_col = pos.column;
        }

        // Get layout
        let layout_resp = self.client.get_layout().await?;
        self.apply_layout(&layout_resp);

        // Get buffer content for each window
        self.fetch_buffer_contents().await?;

        // Fetch display options (line numbers)
        self.fetch_display_options().await;

        Ok(())
    }

    /// Fetch display-related options from server.
    async fn fetch_display_options(&mut self) {
        // Try to fetch number/relativenumber options
        // Server may return unimplemented if options not supported yet
        match self
            .client
            .get_options(vec!["number".to_string(), "relativenumber".to_string()])
            .await
        {
            Ok(opts) => {
                self.apply_display_options(&opts.options);
            }
            Err(e) => {
                // Log but don't fail - options are optional
                tracing::debug!("Could not fetch display options: {e}");
            }
        }
    }

    /// Apply display options to state.
    fn apply_display_options(
        &mut self,
        options: &std::collections::HashMap<String, reovim_protocol::v2::OptionValue>,
    ) {
        use reovim_protocol::v2::option_value::Value;

        let number = options
            .get("number")
            .and_then(|v| {
                v.value.as_ref().and_then(|val| match val {
                    Value::BoolValue(b) => Some(*b),
                    _ => None,
                })
            })
            .unwrap_or(false);

        let relativenumber = options
            .get("relativenumber")
            .and_then(|v| {
                v.value.as_ref().and_then(|val| match val {
                    Value::BoolValue(b) => Some(*b),
                    _ => None,
                })
            })
            .unwrap_or(false);

        self.state.line_number_mode = match (number, relativenumber) {
            (true, true) => LineNumberMode::Hybrid,
            (true, false) => LineNumberMode::Absolute,
            (false, true) => LineNumberMode::Relative,
            (false, false) => LineNumberMode::None,
        };
    }

    /// Apply layout response to state.
    fn apply_layout(&mut self, layout: &GetLayoutResponse) {
        self.state.focused_window_id = layout.focused_window_id;

        // Flatten window tree to list
        self.state.windows.clear();
        if let Some(root) = &layout.root {
            self.collect_windows(root);
        }
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
    async fn fetch_buffer_contents(&mut self) -> Result<(), TuiAppV2Error> {
        let buffer_ids: Vec<u64> = self.state.windows.iter().map(|w| w.buffer_id).collect();

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

    /// Main event loop.
    async fn event_loop(&mut self) -> Result<(), TuiAppV2Error> {
        let mut redraw_timer = interval(Duration::from_millis(16)); // 60 FPS max

        while self.running {
            select! {
                // Terminal input events
                event = self.input.next_event() => {
                    match event {
                        Some(event) => self.handle_input_event(event).await?,
                        None => return Err(TuiAppV2Error::Disconnected),
                    }
                }

                // Server notifications
                notification = self.notification_stream.message() => {
                    match notification {
                        Ok(Some(notif)) => self.handle_notification(notif).await?,
                        Ok(None) => return Err(TuiAppV2Error::StreamEnded),
                        Err(e) => return Err(TuiAppV2Error::Grpc(e.into())),
                    }
                }

                // Redraw timer (coalesces multiple updates)
                _ = redraw_timer.tick() => {
                    if self.state.needs_redraw {
                        self.render()?;
                        self.state.needs_redraw = false;
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle terminal input event.
    async fn handle_input_event(&mut self, event: InputEvent) -> Result<(), TuiAppV2Error> {
        match event {
            InputEvent::Key(key_event) => {
                // Ctrl+Q to quit
                if key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && key_event.code == KeyCode::Char('q')
                {
                    self.running = false;
                    return Ok(());
                }

                // Send vim notation to server
                if !key_event.vim_notation.is_empty()
                    && let Err(e) = self.client.send_keys(&key_event.vim_notation).await
                {
                    self.state.last_error = Some(format!("Send keys failed: {e}"));
                }
            }
            InputEvent::Resize(resize) => {
                self.screen.resize(resize.width, resize.height);

                // Notify server
                if let Err(e) = self
                    .client
                    .resize(u64::from(resize.width), u64::from(resize.height))
                    .await
                {
                    self.state.last_error = Some(format!("Resize failed: {e}"));
                }

                self.state.needs_redraw = true;
            }
            InputEvent::Mouse(_) | InputEvent::FocusGained | InputEvent::FocusLost => {
                // Mouse support and focus tracking can be added later
            }
            InputEvent::Paste(text) => {
                // Handle paste by sending as keys
                if let Err(e) = self.client.send_keys(&text).await {
                    self.state.last_error = Some(format!("Paste failed: {e}"));
                }
            }
        }
        Ok(())
    }

    /// Handle server notification.
    async fn handle_notification(&mut self, notif: Notification) -> Result<(), TuiAppV2Error> {
        if let Some(payload) = notif.payload {
            match payload {
                Payload::ModeChanged(mode) => {
                    self.state.mode_name = mode.name;
                    self.state.mode_display = mode.display;
                    self.state.is_insert_mode = mode.is_insert;
                    self.state.needs_redraw = true;
                }
                Payload::CursorMoved(cursor) => {
                    if let Some(pos) = cursor.position {
                        self.state.cursor_line = pos.line;
                        self.state.cursor_col = pos.column;
                    }
                    self.state.needs_redraw = true;
                }
                Payload::BufferModified(buf) => {
                    // Invalidate cache and refetch
                    self.state.buffer_cache.remove(&buf.buffer_id);
                    // Refetch content
                    if let Ok(content) = self
                        .client
                        .get_buffer_content(Some(buf.buffer_id), None, None)
                        .await
                    {
                        self.state.buffer_cache.insert(buf.buffer_id, content.lines);
                    }
                    self.state.needs_redraw = true;
                }
                Payload::LayoutChanged(layout) => {
                    self.state.focused_window_id = layout.focused_window_id;
                    self.state.windows = layout.windows;
                    self.state.needs_redraw = true;
                }
                Payload::RenderComplete(_) => {
                    // Server signals a frame is ready - refresh content
                    self.state.needs_redraw = true;
                }
                Payload::Detach(detach) => {
                    tracing::info!("Server requested detach: {}", detach.reason);
                    self.running = false;
                }
                Payload::OptionChanged(opt) => {
                    // Refetch options if line number settings changed
                    match opt.name.as_str() {
                        "number" | "relativenumber" => {
                            // Refetch display options asynchronously
                            self.fetch_display_options().await;
                            self.state.needs_redraw = true;
                        }
                        _ => {
                            self.state.needs_redraw = true;
                        }
                    }
                }
                _ => {
                    // Other notifications (selection_changed, etc.)
                    self.state.needs_redraw = true;
                }
            }
        }
        Ok(())
    }

    /// Render the current state to terminal.
    #[allow(clippy::result_large_err)]
    fn render(&mut self) -> Result<(), TuiAppV2Error> {
        let (width, height) = (self.screen.width(), self.screen.height());
        if width == 0 || height == 0 {
            return Ok(());
        }

        // Clear screen buffer
        self.screen.clear();

        // Render each window
        self.render_windows();

        // Render statusline
        self.render_statusline();

        // Apply screen to terminal
        self.screen.render(&mut self.terminal)?;

        // Position cursor
        self.position_cursor()?;

        Ok(())
    }

    /// Calculate gutter width for line numbers.
    fn calculate_gutter_width(&self, total_lines: usize) -> u16 {
        if self.state.line_number_mode == LineNumberMode::None {
            return 0;
        }

        // Calculate width needed: digits + 1 space padding
        let digits = if total_lines == 0 {
            1
        } else {
            // Safe: using f64 for log10 calculation, result is small
            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let d = ((total_lines as f64).log10().floor() as u16) + 1;
            d
        };
        digits + 2 // digits + space before + space after
    }

    /// Render a line number at the given position.
    #[allow(clippy::too_many_arguments)]
    fn render_line_number(&mut self, x: u16, y: u16, width: u16, line: usize, cursor_line: usize) {
        #[allow(clippy::cast_possible_truncation)]
        let fmt_width = (width as usize).saturating_sub(2);

        let (number_str, is_cursor_line) = match self.state.line_number_mode {
            LineNumberMode::None => return,
            LineNumberMode::Absolute => {
                let num = line + 1;
                let s = format!("{num:>fmt_width$} ");
                (s, line == cursor_line)
            }
            LineNumberMode::Relative => {
                let rel = line.abs_diff(cursor_line);
                let s = format!("{rel:>fmt_width$} ");
                (s, line == cursor_line)
            }
            LineNumberMode::Hybrid => {
                let num = if line == cursor_line {
                    line + 1
                } else {
                    line.abs_diff(cursor_line)
                };
                let s = format!("{num:>fmt_width$} ");
                (s, line == cursor_line)
            }
        };

        // Style: cursor line number is brighter
        let style = if is_cursor_line {
            Style::default().with_fg(Color::White)
        } else {
            Style::default().with_fg(Color::DarkGrey)
        };

        self.screen.write_str(x, y, &number_str, &style);
    }

    /// Render all windows to screen buffer.
    fn render_windows(&mut self) {
        let default_style = Style::default();
        let tilde_style = Style::default().with_fg(Color::DarkBlue);

        // Clone windows and buffer cache to avoid borrow issues
        let windows: Vec<_> = self.state.windows.clone();
        let buffer_cache = self.state.buffer_cache.clone();

        // Get cursor line (for line number rendering)
        #[allow(clippy::cast_possible_truncation)]
        let cursor_line = self.state.cursor_line as usize;

        for window in &windows {
            let Some(rect) = &window.rect else { continue };

            #[allow(clippy::cast_possible_truncation)]
            let x = rect.x as u16;
            #[allow(clippy::cast_possible_truncation)]
            let y = rect.y as u16;
            #[allow(clippy::cast_possible_truncation)]
            let w = rect.width as u16;
            #[allow(clippy::cast_possible_truncation)]
            let h = rect.height as u16;

            // Get buffer content
            let lines = buffer_cache.get(&window.buffer_id);
            let total_lines = lines.map_or(0, Vec::len);

            // Calculate gutter width for line numbers
            let gutter_width = self.calculate_gutter_width(total_lines);
            let content_x = x + gutter_width;
            let content_width = w.saturating_sub(gutter_width);

            // Render content
            if let Some(lines) = lines {
                for (row, line) in lines.iter().enumerate().take(h as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let screen_y = y + row as u16;

                    // Render line number if enabled
                    if gutter_width > 0 {
                        self.render_line_number(x, screen_y, gutter_width, row, cursor_line);
                    }

                    // Truncate line to content width
                    let display_line: String = line.chars().take(content_width as usize).collect();
                    self.screen
                        .write_str(content_x, screen_y, &display_line, &default_style);
                }

                // Render empty lines past EOF with tilde (~)
                for row in lines.len()..(h as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let screen_y = y + row as u16;

                    // Render tilde at content position (or gutter if no line numbers)
                    let tilde_x = if gutter_width > 0 {
                        x + gutter_width.saturating_sub(2)
                    } else {
                        x
                    };
                    self.screen.put_char(tilde_x, screen_y, '~', &tilde_style);
                }
            } else {
                // No buffer - fill with tildes
                for row in 0..(h as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let screen_y = y + row as u16;
                    self.screen.put_char(x, screen_y, '~', &tilde_style);
                }
            }

            // Draw window border if multiple windows
            if windows.len() > 1 && window.focused {
                let border_style = Style::default();
                self.screen.put_char(x, y, '▪', &border_style);
            }
        }
    }

    /// Get mode-specific style for the statusline mode section.
    fn mode_statusline_style(&self) -> Style {
        let mode = self.state.mode_name.to_lowercase();
        let (fg, bg) = match mode.as_str() {
            "insert" | "i" => (Color::Black, Color::Green),
            "visual" | "v" | "visual-line" | "visual-block" => (Color::Black, Color::Magenta),
            "replace" | "r" => (Color::Black, Color::Red),
            "command" | ":" | "cmdline" => (Color::Black, Color::Yellow),
            "delete" | "d" => (Color::Black, Color::DarkRed),
            "yank" | "y" => (Color::Black, Color::DarkYellow),
            "change" | "c" => (Color::Black, Color::DarkCyan),
            _ => (Color::Black, Color::Blue), // Normal mode and others
        };
        Style::default().with_fg(fg).with_bg(bg).bold()
    }

    /// Render statusline at bottom of screen.
    fn render_statusline(&mut self) {
        let height = self.screen.height();
        let width = self.screen.width();
        if height == 0 {
            return;
        }

        let status_y = height - 1;
        let default_style = Style::default().reverse();

        // Copy values to avoid borrow issues
        let mode_display = self.state.mode_display.clone();
        let cursor_line = self.state.cursor_line;
        let cursor_col = self.state.cursor_col;
        let server = self.server_address.clone();

        // Get mode-specific style
        let mode_style = self.mode_statusline_style();

        // Build statusline
        let left = format!(" {mode_display} ");
        let cursor_str = format!("{}:{}", cursor_line + 1, cursor_col + 1);
        let right = format!(" {cursor_str} | {server} ");

        // Fill statusline with default style
        self.screen
            .fill_horizontal(0, status_y, width, ' ', &default_style);

        // Write mode section with mode-specific colors
        self.screen.write_str(0, status_y, &left, &mode_style);

        // Write right part with default style
        #[allow(clippy::cast_possible_truncation)]
        let right_x = width.saturating_sub(right.len() as u16);
        self.screen
            .write_str(right_x, status_y, &right, &default_style);
    }

    /// Get cursor style based on current mode.
    fn cursor_style_for_mode(&self) -> CursorStyle {
        let mode = self.state.mode_name.to_lowercase();
        match mode.as_str() {
            // Insert mode and command line: blinking bar
            "insert" | "i" | "command" | ":" | "cmdline" => CursorStyle::BlinkingBar,
            // Replace mode: underline
            "replace" | "r" => CursorStyle::SteadyUnderline,
            // Normal mode, visual modes, and others: steady block
            _ => CursorStyle::SteadyBlock,
        }
    }

    /// Position terminal cursor at editor cursor location.
    #[allow(clippy::result_large_err)]
    fn position_cursor(&mut self) -> Result<(), TuiAppV2Error> {
        // Find focused window and copy necessary info to avoid borrow issues
        let focused_info = self.state.windows.iter().find(|w| w.focused).and_then(|w| {
            w.rect.as_ref().map(|rect| {
                let total_lines = self
                    .state
                    .buffer_cache
                    .get(&w.buffer_id)
                    .map_or(0, Vec::len);
                (*rect, total_lines)
            })
        });

        if let Some((rect, total_lines)) = focused_info {
            // Calculate gutter width for proper cursor offset
            let gutter_width = self.calculate_gutter_width(total_lines);

            #[allow(clippy::cast_possible_truncation)]
            let cursor_x = rect.x as u16 + gutter_width + self.state.cursor_col as u16;
            #[allow(clippy::cast_possible_truncation)]
            let cursor_y = rect.y as u16 + self.state.cursor_line as u16;

            // Set cursor style based on mode
            let cursor_style = self.cursor_style_for_mode();
            self.cursor.set_style(cursor_style);
            self.cursor.move_to(cursor_x, cursor_y);
            self.cursor.show();
            self.cursor.apply()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_state_default() {
        let state = TuiState::default();
        assert!(state.mode_name.is_empty());
        assert!(state.mode_display.is_empty());
        assert!(!state.is_insert_mode);
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 0);
        assert_eq!(state.line_number_mode, LineNumberMode::None);
    }

    #[test]
    fn test_error_display() {
        let err = TuiAppV2Error::Disconnected;
        assert_eq!(err.to_string(), "Server disconnected");

        let err = TuiAppV2Error::StreamEnded;
        assert_eq!(err.to_string(), "Notification stream ended");
    }

    #[test]
    fn test_line_number_mode_default() {
        assert_eq!(LineNumberMode::default(), LineNumberMode::None);
    }

    #[test]
    fn test_line_number_mode_variants() {
        // Test all variants exist
        let _ = LineNumberMode::None;
        let _ = LineNumberMode::Absolute;
        let _ = LineNumberMode::Relative;
        let _ = LineNumberMode::Hybrid;
    }
}
