//! TUI application main loop.
//!
//! Connects to server, handles input, and renders output.

use std::time::Duration;

use {crossterm::event::Event, serde_json::json};

use crate::client::common::{ConnectionConfig, RpcClient, RpcClientError};

use super::{input::InputHandler, render::Renderer};

/// TUI application error.
#[derive(Debug)]
pub enum TuiError {
    /// I/O error.
    Io(std::io::Error),
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

impl From<std::io::Error> for TuiError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<RpcClientError> for TuiError {
    fn from(e: RpcClientError) -> Self {
        Self::Rpc(e)
    }
}

/// TUI application.
pub struct TuiApp {
    client: RpcClient,
    renderer: Renderer,
    running: bool,
    last_size: (u16, u16),
}

impl TuiApp {
    /// Create and connect TUI application.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect(config: &ConnectionConfig) -> Result<Self, TuiError> {
        let client = RpcClient::connect(config).await?;
        let renderer = Renderer::new();

        Ok(Self {
            client,
            renderer,
            running: false,
            last_size: (0, 0),
        })
    }

    /// Run the TUI application main loop.
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

        // Main loop
        while self.running {
            // Poll for terminal events (non-blocking with short timeout)
            if let Some(event) = InputHandler::poll_event(Duration::from_millis(10))? {
                self.handle_terminal_event(event).await?;
            }

            // TODO: Poll for server messages (notifications)
            // This requires async read which conflicts with sync terminal input
            // For now, we refresh after each key
        }

        // Cleanup
        self.renderer.cleanup()?;
        Ok(())
    }

    /// Handle a terminal event.
    async fn handle_terminal_event(&mut self, event: Event) -> Result<(), TuiError> {
        match event {
            Event::Key(key) => {
                // Check for quit (Ctrl+C or Ctrl+Q)
                if let crossterm::event::KeyCode::Char('c' | 'q') = key.code
                    && key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL)
                {
                    self.running = false;
                    return Ok(());
                }

                // Convert key to notation and send to server
                if let Some(keys) = InputHandler::key_to_notation(&key) {
                    self.send_keys(&keys).await?;
                    // Refresh screen after key input
                    self.refresh_screen().await?;
                }
            }

            Event::Resize(width, height) => {
                if (width, height) != self.last_size {
                    self.last_size = (width, height);
                    self.send_resize(width, height).await?;
                    self.refresh_screen().await?;
                }
            }

            _ => {}
        }

        Ok(())
    }

    /// Send key sequence to server.
    async fn send_keys(&mut self, keys: &str) -> Result<(), TuiError> {
        self.client
            .call("input/keys", json!({ "keys": keys }))
            .await?;
        Ok(())
    }

    /// Send resize notification to server.
    async fn send_resize(&mut self, width: u16, height: u16) -> Result<(), TuiError> {
        self.client
            .call("editor/resize", json!({ "width": width, "height": height }))
            .await?;
        Ok(())
    }

    /// Refresh screen content from server.
    async fn refresh_screen(&mut self) -> Result<(), TuiError> {
        let result = self
            .client
            .call("state/screen_content", json!({ "format": "raw_ansi" }))
            .await?;

        if let Some(content) = result.get("content").and_then(|v| v.as_str()) {
            self.renderer.render(content)?;
        }

        // Position cursor based on server state
        let cursor = self.client.call("state/cursor", json!({})).await?;
        if let (Some(x), Some(y)) = (
            cursor.get("column").and_then(serde_json::Value::as_u64),
            cursor.get("line").and_then(serde_json::Value::as_u64),
        ) {
            // Cursor position from server is 0-indexed
            // Terminal cursor is also 0-indexed
            #[allow(clippy::cast_possible_truncation)]
            self.renderer.set_cursor(x as u16, y as u16)?;
            self.renderer.show_cursor()?;
        }

        self.renderer.flush()?;
        Ok(())
    }

    /// Quit the application.
    pub const fn quit(&mut self) {
        self.running = false;
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        // Cleanup is handled by Renderer's Drop
    }
}
