//! Interactive TUI model with terminal I/O.
//!
//! This model wraps `Terminal`, `Screen`, `InputReader`, and `Cursor`
//! to provide full terminal-based TUI functionality.

use std::io;

use reovim_driver_tui::{Cursor, CursorStyle, InputEvent, InputReader, Terminal};

use crate::{
    render_backend::ScreenBackend,
    tui_model::{CursorStyleHint, TuiEvent, TuiInputEvent, TuiModel},
};

/// Interactive TUI model with terminal I/O.
///
/// Provides keyboard input, terminal cursor, and screen rendering.
pub struct InteractiveModel {
    /// Terminal session (raw mode, alternate screen).
    terminal: Terminal,
    /// Screen buffer and renderer (wrapped for `RenderBackend` trait).
    screen: ScreenBackend,
    /// Cursor manager for terminal cursor.
    cursor: Cursor,
    /// Async input event reader.
    input: InputReader,
}

impl InteractiveModel {
    /// Create a new interactive model.
    ///
    /// Enters raw mode and alternate screen. Restored on drop.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal initialization fails.
    pub fn new() -> io::Result<Self> {
        let terminal = Terminal::enter()?;
        let (width, height) = Terminal::size()?;
        let screen = ScreenBackend::new(width, height);
        let cursor = Cursor::new();
        let input = InputReader::new();

        Ok(Self {
            terminal,
            screen,
            cursor,
            input,
        })
    }

    /// Create with specific size (for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if terminal initialization fails.
    pub fn with_size(width: u16, height: u16) -> io::Result<Self> {
        let terminal = Terminal::enter()?;
        let screen = ScreenBackend::new(width, height);
        let cursor = Cursor::new();
        let input = InputReader::new();

        Ok(Self {
            terminal,
            screen,
            cursor,
            input,
        })
    }

    /// Get reference to terminal.
    #[must_use]
    pub const fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// Get mutable reference to terminal.
    #[allow(clippy::missing_const_for_fn)] // &mut self cannot be const
    pub fn terminal_mut(&mut self) -> &mut Terminal {
        &mut self.terminal
    }
}

impl TuiModel for InteractiveModel {
    type Backend = ScreenBackend;

    fn backend_mut(&mut self) -> &mut ScreenBackend {
        &mut self.screen
    }

    fn size(&self) -> (u16, u16) {
        (self.screen.width(), self.screen.height())
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.screen.resize(width, height);
    }

    fn flush(&mut self) -> io::Result<()> {
        self.screen.render(&mut self.terminal)
    }

    fn position_cursor(&mut self, x: u16, y: u16) {
        self.cursor.move_to(x, y);
        // apply() writes to stdout, no need to pass terminal
        let _ = self.cursor.apply();
    }

    async fn poll_input(&mut self) -> Option<TuiInputEvent> {
        loop {
            let event = self.input.next_event().await?;
            match event {
                InputEvent::Key(key) => return Some(TuiInputEvent::Key(key)),
                InputEvent::Mouse(mouse) => return Some(TuiInputEvent::Mouse(mouse)),
                InputEvent::Resize(resize) => {
                    return Some(TuiInputEvent::Resize(resize.width, resize.height));
                }
                // Skip focus and paste events, continue polling
                InputEvent::FocusGained | InputEvent::FocusLost | InputEvent::Paste(_) => {}
            }
        }
    }

    fn render_self_cursor(&self) -> bool {
        // Interactive mode uses terminal cursor, not buffer-rendered cursor
        // This cannot be const because it's a trait method implementation
        false
    }

    fn capture(&self, _format: &str) -> Option<String> {
        // Interactive doesn't support capture (use terminal directly)
        None
    }

    fn set_cursor_style(&mut self, style: CursorStyleHint) {
        let cursor_style = match style {
            CursorStyleHint::Block => CursorStyle::SteadyBlock,
            CursorStyleHint::Bar => CursorStyle::SteadyBar,
            CursorStyleHint::Underline => CursorStyle::SteadyUnderline,
        };
        self.cursor.set_style(cursor_style);
        let _ = self.cursor.apply();
    }

    fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor.set_visible(visible);
        let _ = self.cursor.apply();
    }

    fn invalidate(&mut self) {
        self.screen.invalidate();
    }

    async fn poll_event(&mut self) -> Option<TuiEvent> {
        // Interactive mode: poll for keyboard/mouse input
        let input = self.poll_input().await?;
        Some(TuiEvent::Input(input))
    }
}

#[cfg(test)]
#[path = "interactive_tests.rs"]
mod tests;
