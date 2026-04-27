//! TUI model trait for abstracting I/O backends.
//!
//! This module defines the `TuiModel` trait that abstracts the differences
//! between interactive (terminal) and headless (frame buffer) TUI modes.
//! Both modes share the same event loop and rendering logic via `TuiApp<M>`.
//!
//! # Design Principle
//!
//! "They must be same, only different for the input and output is tty or not."
//!
//! The `TuiModel` trait captures exactly these differences:
//! - **Input**: Interactive reads keyboard, headless has no input
//! - **Output**: Interactive uses terminal, headless uses frame buffer
//! - **Cursor**: Interactive positions terminal cursor, headless renders in buffer
//! - **Requests**: Interactive has none, headless has external API requests
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  TuiApp<M: TuiModel>                                        │
//! │    - Event loop (shared)                                    │
//! │    - Notification handling (shared via NotificationContext) │
//! │    - Rendering (shared via RenderBackend)                   │
//! ├──────────────────────────┬──────────────────────────────────┤
//! │  InteractiveModel        │  HeadlessModel                   │
//! │    - Terminal + Screen   │    - FrameBuffer only            │
//! │    - Keyboard input      │    - No keyboard (pending)       │
//! │    - Terminal cursor     │    - Cursor in buffer            │
//! │    - No external API     │    - Channel-based requests      │
//! │    - No capture          │    - Frame capture               │
//! └──────────────────────────┴──────────────────────────────────┘
//! ```

use std::io;

use crate::render_backend::RenderBackend;

/// External request from programmatic API (headless TUI only).
///
/// These requests come from test code or CLI commands, not from the server.
/// Interactive TUI never receives these.
#[derive(Debug)]
pub enum ExternalRequest {
    /// Capture the current frame.
    Capture {
        /// Output format (e.g., `"plain_text"`, `"ansi"`).
        format: String,
        /// Channel to send the result.
        response: tokio::sync::oneshot::Sender<String>,
    },
    /// Resize the viewport.
    Resize {
        /// New width.
        width: u16,
        /// New height.
        height: u16,
        /// Channel to send completion.
        response: tokio::sync::oneshot::Sender<()>,
    },
    /// Send keys to the server.
    SendKeys {
        /// Keys in vim notation.
        keys: String,
        /// Channel to send the result.
        response: tokio::sync::oneshot::Sender<bool>,
    },
    /// Stop the event loop.
    Stop,
}

/// Input event from the TUI model.
///
/// Interactive models produce keyboard/mouse events.
/// Headless models produce no events (None from poll).
#[derive(Debug, Clone)]
pub enum TuiInputEvent {
    /// Keyboard input event.
    Key(reovim_driver_tui::KeyEvent),
    /// Mouse input event.
    Mouse(reovim_driver_tui::MouseEvent),
    /// Terminal resize event.
    Resize(u16, u16),
}

/// Combined event from the TUI model.
///
/// This unifies input events (keyboard/mouse) and external requests
/// (`send_keys`/`capture`/`resize` from API) into a single poll.
#[derive(Debug)]
pub enum TuiEvent {
    /// Input event (interactive mode).
    Input(TuiInputEvent),
    /// External request (headless mode).
    Request(ExternalRequest),
}

/// TUI model trait for abstracting I/O backends.
///
/// Implementations provide the specific I/O mechanism:
/// - `InteractiveModel`: Terminal I/O with keyboard input
/// - `HeadlessModel`: In-memory frame buffer without keyboard
///
/// The generic `TuiApp<M: TuiModel>` uses this trait to handle both modes
/// with shared logic for event loop, notifications, and rendering.
pub trait TuiModel {
    /// The render backend type (`Screen` or `FrameBuffer`).
    type Backend: RenderBackend;

    /// Get mutable reference to the render backend.
    fn backend_mut(&mut self) -> &mut Self::Backend;

    /// Get the current size (width, height).
    fn size(&self) -> (u16, u16);

    /// Resize the render target.
    ///
    /// Interactive: Updates terminal and screen size.
    /// Headless: Resizes the frame buffer.
    fn resize(&mut self, width: u16, height: u16);

    /// Flush rendered content to output.
    ///
    /// Interactive: Renders to terminal with diff optimization.
    /// Headless: No-op (frame buffer is always "ready").
    ///
    /// # Errors
    ///
    /// Returns an error if terminal rendering fails.
    fn flush(&mut self) -> io::Result<()>;

    /// Position the cursor at (x, y).
    ///
    /// Interactive: Moves terminal cursor.
    /// Headless: No-op (cursor rendered in buffer).
    fn position_cursor(&mut self, x: u16, y: u16);

    /// Poll for input events.
    ///
    /// Interactive: Waits for keyboard/mouse events.
    /// Headless: Always returns None (no keyboard input).
    ///
    /// This method is cancel-safe for use in `tokio::select!`.
    fn poll_input(&mut self) -> impl std::future::Future<Output = Option<TuiInputEvent>> + '_;

    /// Whether to render self cursor in the render backend.
    ///
    /// Interactive: false (uses terminal cursor).
    /// Headless: true (cursor must be visible in captures).
    fn render_self_cursor(&self) -> bool;

    /// Capture the current frame content.
    ///
    /// Interactive: Returns None (use terminal directly).
    /// Headless: Returns the frame buffer content in the requested format.
    fn capture(&self, format: &str) -> Option<String>;

    /// Set the cursor style.
    ///
    /// Interactive: Changes terminal cursor appearance.
    /// Headless: No-op.
    fn set_cursor_style(&mut self, style: CursorStyleHint);

    /// Show or hide the cursor.
    ///
    /// Interactive: Shows/hides terminal cursor.
    /// Headless: No-op.
    fn set_cursor_visible(&mut self, visible: bool);

    /// Force a full redraw on next render.
    ///
    /// Used after resize or when screen state may be corrupted.
    fn invalidate(&mut self);

    /// Poll for external requests (headless API only).
    ///
    /// Interactive: Returns pending forever (no external API).
    /// Headless: Returns requests from `send_keys()`, `capture()`, etc.
    ///
    /// This method is cancel-safe for use in `tokio::select!`.
    fn poll_request(&mut self) -> impl std::future::Future<Output = Option<ExternalRequest>> + '_ {
        // Default: pending forever (no external requests)
        async { std::future::pending().await }
    }

    /// Poll for any event (input or external request).
    ///
    /// This is a combined poll that avoids the double-borrow issue
    /// in `tokio::select!` when polling both input and requests.
    ///
    /// Interactive: Returns input events.
    /// Headless: Returns external requests.
    fn poll_event(&mut self) -> impl std::future::Future<Output = Option<TuiEvent>> + '_;
}

/// Hint for cursor style (mode-dependent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyleHint {
    /// Block cursor (normal mode).
    #[default]
    Block,
    /// Vertical bar cursor (insert mode).
    Bar,
    /// Underline cursor (replace mode).
    Underline,
}

#[cfg(test)]
#[path = "tui_model_tests.rs"]
mod tests;
