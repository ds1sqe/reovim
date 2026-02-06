//! Headless TUI model with in-memory frame buffer.
//!
//! This model provides a frame buffer for rendering without terminal I/O.
//! Used for testing, scripting, and CI environments.
//!
//! Supports external API via channel-based requests:
//! - `send_keys()` - send keys programmatically
//! - `capture()` - capture frame content
//! - `resize()` - resize viewport
//! - `stop()` - stop event loop

use std::io;

use {reovim_driver_display::FrameBuffer, reovim_protocol::v1::ScreenFormat, tokio::sync::mpsc};

use crate::tui_model::{CursorStyleHint, ExternalRequest, TuiEvent, TuiInputEvent, TuiModel};

/// Headless TUI model with in-memory frame buffer.
///
/// No terminal I/O - renders to an in-memory buffer that can be captured.
/// Supports programmatic control via external requests.
pub struct HeadlessModel {
    /// In-memory frame buffer.
    frame_buffer: FrameBuffer,
    /// Channel to receive external requests.
    request_rx: mpsc::Receiver<ExternalRequest>,
}

/// Handle for sending requests to a headless TUI.
///
/// This is the programmatic API for controlling a headless TUI.
/// Created by [`HeadlessModel::new_with_handle`].
#[derive(Clone)]
pub struct HeadlessHandle {
    request_tx: mpsc::Sender<ExternalRequest>,
}

impl HeadlessHandle {
    /// Capture the current frame buffer content.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop is not running.
    pub async fn capture(&self, format: &str) -> Result<String, HeadlessCaptureError> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.request_tx
            .send(ExternalRequest::Capture {
                format: format.to_string(),
                response: response_tx,
            })
            .await
            .map_err(|_| HeadlessCaptureError::NotRunning)?;
        response_rx
            .await
            .map_err(|_| HeadlessCaptureError::NotRunning)
    }

    /// Resize the viewport.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop is not running.
    pub async fn resize(&self, width: u16, height: u16) -> Result<(), HeadlessCaptureError> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.request_tx
            .send(ExternalRequest::Resize {
                width,
                height,
                response: response_tx,
            })
            .await
            .map_err(|_| HeadlessCaptureError::NotRunning)?;
        response_rx
            .await
            .map_err(|_| HeadlessCaptureError::NotRunning)
    }

    /// Send keys to the server.
    ///
    /// # Returns
    ///
    /// Returns `true` if keys were processed by the server.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop is not running.
    pub async fn send_keys(&self, keys: &str) -> Result<bool, HeadlessCaptureError> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.request_tx
            .send(ExternalRequest::SendKeys {
                keys: keys.to_string(),
                response: response_tx,
            })
            .await
            .map_err(|_| HeadlessCaptureError::NotRunning)?;
        response_rx
            .await
            .map_err(|_| HeadlessCaptureError::NotRunning)
    }

    /// Stop the headless TUI event loop.
    pub async fn stop(&self) {
        let _ = self.request_tx.send(ExternalRequest::Stop).await;
    }

    /// Wait for a condition to be met in the frame.
    ///
    /// Polls every 10ms up to the timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if timeout expires or capture fails.
    pub async fn wait_for<F>(
        &self,
        timeout: std::time::Duration,
        predicate: F,
    ) -> Result<String, HeadlessCaptureError>
    where
        F: Fn(&str) -> bool,
    {
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(10);

        while start.elapsed() < timeout {
            let frame = self.capture("plain_text").await?;
            if predicate(&frame) {
                return Ok(frame);
            }
            tokio::time::sleep(poll_interval).await;
        }

        Err(HeadlessCaptureError::Timeout)
    }
}

/// Error from headless capture operations.
#[derive(Debug)]
pub enum HeadlessCaptureError {
    /// Event loop not running.
    NotRunning,
    /// Timeout waiting for condition.
    Timeout,
}

impl std::fmt::Display for HeadlessCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunning => write!(f, "Headless TUI event loop not running"),
            Self::Timeout => write!(f, "Timeout waiting for condition"),
        }
    }
}

impl std::error::Error for HeadlessCaptureError {}

impl HeadlessModel {
    /// Create a new headless model with the given dimensions.
    ///
    /// Returns both the model (for the event loop) and a handle (for API control).
    #[must_use]
    pub fn new_with_handle(width: u16, height: u16) -> (Self, HeadlessHandle) {
        let (request_tx, request_rx) = mpsc::channel(32);
        let model = Self {
            frame_buffer: FrameBuffer::new(width, height),
            request_rx,
        };
        let handle = HeadlessHandle { request_tx };
        (model, handle)
    }

    /// Create a new headless model without a handle.
    ///
    /// Used for testing where no external API is needed.
    /// The request channel will never receive anything.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let (_request_tx, request_rx) = mpsc::channel(1);
        Self {
            frame_buffer: FrameBuffer::new(width, height),
            request_rx,
        }
    }

    /// Get reference to the frame buffer.
    #[must_use]
    pub const fn frame_buffer(&self) -> &FrameBuffer {
        &self.frame_buffer
    }

    /// Convert frame buffer to string in the specified format.
    fn format_frame(&self, format: ScreenFormat) -> String {
        match format {
            ScreenFormat::RawAnsi => self.to_ansi(),
            ScreenFormat::PlainText | ScreenFormat::CellGrid => self.to_plain_text(),
        }
    }

    /// Convert frame buffer to ANSI-colored string.
    fn to_ansi(&self) -> String {
        use reovim_driver_display::ColorMode;

        let mut output = String::new();
        let _width = self.frame_buffer.width();
        let height = self.frame_buffer.height();

        for y in 0..height {
            if let Some(row) = self.frame_buffer.row(y) {
                for cell in row {
                    // Skip continuation cells (part of wide characters)
                    if cell.is_continuation {
                        continue;
                    }

                    // Apply style
                    let ansi_start = cell.style.to_ansi_start(ColorMode::TrueColor);
                    if !ansi_start.is_empty() {
                        output.push_str(&ansi_start);
                    }

                    output.push(cell.char);

                    // Reset if style was applied
                    if !ansi_start.is_empty() {
                        output.push_str("\x1b[0m");
                    }
                }
            }
            if y < height - 1 {
                output.push('\n');
            }
        }

        output
    }

    /// Convert frame buffer to plain text (no ANSI codes).
    fn to_plain_text(&self) -> String {
        let mut output = String::new();
        let _width = self.frame_buffer.width();
        let height = self.frame_buffer.height();

        for y in 0..height {
            if let Some(row) = self.frame_buffer.row(y) {
                for cell in row {
                    // Skip continuation cells
                    if cell.is_continuation {
                        continue;
                    }
                    output.push(cell.char);
                }
            }
            if y < height - 1 {
                output.push('\n');
            }
        }

        output
    }
}

impl TuiModel for HeadlessModel {
    type Backend = FrameBuffer;

    fn backend_mut(&mut self) -> &mut FrameBuffer {
        &mut self.frame_buffer
    }

    fn size(&self) -> (u16, u16) {
        (self.frame_buffer.width(), self.frame_buffer.height())
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.frame_buffer.resize(width, height);
    }

    fn flush(&mut self) -> io::Result<()> {
        // Headless: no-op (frame buffer is always "ready")
        Ok(())
    }

    fn position_cursor(&mut self, _x: u16, _y: u16) {
        // Headless: no terminal cursor to position
        // Cursor is rendered in the frame buffer via render_self_cursor
    }

    async fn poll_input(&mut self) -> Option<TuiInputEvent> {
        // Headless: no keyboard input
        // This future never resolves (pending forever)
        std::future::pending().await
    }

    fn render_self_cursor(&self) -> bool {
        // Headless: cursor must be rendered in the frame buffer
        // so it's visible in captures
        true
    }

    fn capture(&self, format: &str) -> Option<String> {
        let screen_format = match format.to_lowercase().as_str() {
            "ansi" | "raw_ansi" | "rawansi" => ScreenFormat::RawAnsi,
            _ => ScreenFormat::PlainText,
        };
        Some(self.format_frame(screen_format))
    }

    fn set_cursor_style(&mut self, _style: CursorStyleHint) {
        // Headless: no terminal cursor style to set
    }

    fn set_cursor_visible(&mut self, _visible: bool) {
        // Headless: no terminal cursor visibility to set
    }

    fn invalidate(&mut self) {
        // Headless: no cached state to invalidate
        // Could clear the frame buffer, but typically not needed
    }

    async fn poll_request(&mut self) -> Option<ExternalRequest> {
        // Receive external requests from the headless handle
        self.request_rx.recv().await
    }

    async fn poll_event(&mut self) -> Option<TuiEvent> {
        // Headless mode: poll for external requests from the handle
        let request = self.poll_request().await?;
        Some(TuiEvent::Request(request))
    }
}

impl Default for HeadlessModel {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headless_model_new() {
        let model = HeadlessModel::new(80, 24);
        assert_eq!(model.size(), (80, 24));
    }

    #[test]
    fn test_headless_model_resize() {
        let mut model = HeadlessModel::new(80, 24);
        model.resize(120, 40);
        assert_eq!(model.size(), (120, 40));
    }

    #[test]
    fn test_headless_model_render_self_cursor() {
        let model = HeadlessModel::new(80, 24);
        assert!(model.render_self_cursor());
    }

    #[test]
    fn test_headless_model_capture() {
        let model = HeadlessModel::new(5, 1);
        let capture = model.capture("plain").unwrap();
        // Empty frame buffer should have spaces
        assert_eq!(capture, "     ");
    }

    #[test]
    fn test_headless_model_flush() {
        let mut model = HeadlessModel::new(80, 24);
        // Flush should succeed (no-op)
        assert!(model.flush().is_ok());
    }
}
