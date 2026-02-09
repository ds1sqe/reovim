//! Common input bus and programmatic handle for TUI.
//!
//! This module provides the unified input channel that both TTY keyboard
//! and RPC handle feed into, plus the `TuiHandle` for programmatic control.
//!
//! # Architecture
//!
//! ```text
//! TTY keyboard ──┐
//!                ├──► input_tx ──► TuiApp.input_rx
//! TuiHandle ─────┘
//! ```
//!
//! One channel, two protocols:
//! - `TuiInput::Raw(RawInputEvent)` — hardware events from TTY adapter
//! - `TuiInput::Control(ControlRequest)` — commands from `TuiHandle`

use std::time::Duration;

use {
    reovim_driver_tui::{InputEvent, InputReader, KeyEvent, MouseEvent},
    tokio::sync::{mpsc, oneshot},
};

// ============================================================================
// TuiInput — Common Input Enum
// ============================================================================

/// Unified input event from any source. One channel, two protocols.
#[derive(Debug)]
pub enum TuiInput {
    /// Raw hardware events (from TTY adapter, need vim notation conversion).
    Raw(RawInputEvent),
    /// Pre-formatted control commands (from `TuiHandle`, have response channels).
    Control(ControlRequest),
}

/// Raw hardware input events (from TTY keyboard/mouse).
#[derive(Debug, Clone)]
pub enum RawInputEvent {
    /// Keyboard input.
    Key(KeyEvent),
    /// Mouse input.
    Mouse(MouseEvent),
    /// Terminal resize.
    Resize(u16, u16),
}

/// Control commands from `TuiHandle` (both modes).
#[derive(Debug)]
pub enum ControlRequest {
    /// Send keys to the server.
    SendKeys {
        /// Keys in vim notation.
        keys: String,
        /// Channel to send the result.
        response: oneshot::Sender<bool>,
    },
    /// Capture the current frame.
    Capture {
        /// Output format (e.g., `"plain_text"`, `"ansi"`).
        format: String,
        /// Channel to send the result.
        response: oneshot::Sender<String>,
    },
    /// Resize the viewport.
    Resize {
        /// New width.
        width: u16,
        /// New height.
        height: u16,
        /// Channel to send completion.
        response: oneshot::Sender<()>,
    },
    /// Stop the event loop.
    Stop,
}

// ============================================================================
// TuiHandle — Programmatic Control (same for both modes)
// ============================================================================

/// Handle for programmatic control of a TUI instance.
///
/// Sends commands to the common input channel. Works identically
/// for interactive and headless modes.
#[derive(Clone)]
pub struct TuiHandle {
    input_tx: mpsc::Sender<TuiInput>,
}

impl TuiHandle {
    /// Create a new handle from the input sender.
    #[must_use]
    pub const fn new(input_tx: mpsc::Sender<TuiInput>) -> Self {
        Self { input_tx }
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
    pub async fn send_keys(&self, keys: &str) -> Result<bool, TuiHandleError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.input_tx
            .send(TuiInput::Control(ControlRequest::SendKeys {
                keys: keys.to_string(),
                response: response_tx,
            }))
            .await
            .map_err(|_| TuiHandleError::NotRunning)?;
        response_rx.await.map_err(|_| TuiHandleError::NotRunning)
    }

    /// Capture the current frame buffer content.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop is not running.
    pub async fn capture(&self, format: &str) -> Result<String, TuiHandleError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.input_tx
            .send(TuiInput::Control(ControlRequest::Capture {
                format: format.to_string(),
                response: response_tx,
            }))
            .await
            .map_err(|_| TuiHandleError::NotRunning)?;
        response_rx.await.map_err(|_| TuiHandleError::NotRunning)
    }

    /// Resize the viewport.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop is not running.
    pub async fn resize(&self, width: u16, height: u16) -> Result<(), TuiHandleError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.input_tx
            .send(TuiInput::Control(ControlRequest::Resize {
                width,
                height,
                response: response_tx,
            }))
            .await
            .map_err(|_| TuiHandleError::NotRunning)?;
        response_rx.await.map_err(|_| TuiHandleError::NotRunning)
    }

    /// Stop the TUI event loop.
    pub async fn stop(&self) {
        let _ = self
            .input_tx
            .send(TuiInput::Control(ControlRequest::Stop))
            .await;
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
        timeout: Duration,
        predicate: F,
    ) -> Result<String, TuiHandleError>
    where
        F: Fn(&str) -> bool,
    {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(10);

        while start.elapsed() < timeout {
            let frame = self.capture("plain_text").await?;
            if predicate(&frame) {
                return Ok(frame);
            }
            tokio::time::sleep(poll_interval).await;
        }

        Err(TuiHandleError::Timeout)
    }
}

// ============================================================================
// TuiHandleError
// ============================================================================

/// Error from TUI handle operations.
#[derive(Debug)]
pub enum TuiHandleError {
    /// Event loop not running (channel closed).
    NotRunning,
    /// Timeout waiting for condition.
    Timeout,
}

impl std::fmt::Display for TuiHandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunning => write!(f, "TUI event loop not running"),
            Self::Timeout => write!(f, "Timeout waiting for condition"),
        }
    }
}

impl std::error::Error for TuiHandleError {}

// ============================================================================
// TTY Input Adapter (spawned task for interactive mode)
// ============================================================================

/// Spawn a task that reads TTY input and sends to the common input channel.
///
/// This is the producer side of the common input bus for interactive mode.
/// The task reads from `InputReader` (crossterm events) and forwards
/// keyboard, mouse, and resize events as `TuiInput::Raw(...)`.
///
/// Focus and paste events are filtered out (same as the old `InteractiveModel`).
///
/// Returns a join handle for the spawned task.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn spawn_tty_reader(input_tx: mpsc::Sender<TuiInput>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = InputReader::new();
        loop {
            let Some(event) = reader.next_event().await else {
                break;
            };
            let tui_input = match event {
                InputEvent::Key(k) => TuiInput::Raw(RawInputEvent::Key(k)),
                InputEvent::Mouse(m) => TuiInput::Raw(RawInputEvent::Mouse(m)),
                InputEvent::Resize(r) => TuiInput::Raw(RawInputEvent::Resize(r.width, r.height)),
                // Skip focus and paste events
                InputEvent::FocusGained | InputEvent::FocusLost | InputEvent::Paste(_) => continue,
            };
            if input_tx.send(tui_input).await.is_err() {
                // Channel closed, TuiApp dropped
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_handle_error_display() {
        let err = TuiHandleError::NotRunning;
        assert_eq!(err.to_string(), "TUI event loop not running");

        let err = TuiHandleError::Timeout;
        assert_eq!(err.to_string(), "Timeout waiting for condition");
    }

    #[tokio::test]
    async fn test_handle_stop_on_closed_channel() {
        let (tx, rx) = mpsc::channel(1);
        let handle = TuiHandle::new(tx);

        // Drop the receiver
        drop(rx);

        // stop() should not panic
        handle.stop().await;
    }

    #[tokio::test]
    async fn test_handle_send_keys_not_running() {
        let (tx, rx) = mpsc::channel(1);
        let handle = TuiHandle::new(tx);

        drop(rx);

        let result = handle.send_keys("ihello").await;
        assert!(result.is_err());
    }
}
