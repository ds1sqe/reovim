//! Terminal rendering for TUI client.
//!
//! Handles terminal setup, cleanup, and screen rendering.

use std::io::{self, Stdout, Write};

use crossterm::{
    cursor::{self, SetCursorStyle},
    event::DisableMouseCapture,
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

/// Cursor style kind for terminal rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyleKind {
    /// Block cursor (default for normal mode).
    #[default]
    Block,
    /// Vertical bar cursor (for insert mode).
    Bar,
    /// Underline cursor (for replace mode).
    Underline,
}

/// Terminal renderer.
pub struct Renderer {
    stdout: Stdout,
    initialized: bool,
}

impl Renderer {
    /// Create a new renderer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stdout: io::stdout(),
            initialized: false,
        }
    }

    /// Initialize terminal for TUI mode.
    ///
    /// Enables raw mode, alternate screen, and hides cursor.
    ///
    /// # Errors
    ///
    /// Returns error if terminal setup fails.
    pub fn init(&mut self) -> io::Result<()> {
        if self.initialized {
            return Ok(());
        }

        terminal::enable_raw_mode()?;
        execute!(self.stdout, EnterAlternateScreen)?;

        self.initialized = true;
        Ok(())
    }

    /// Cleanup terminal and restore previous state.
    ///
    /// # Errors
    ///
    /// Returns error if cleanup fails.
    pub fn cleanup(&mut self) -> io::Result<()> {
        if !self.initialized {
            return Ok(());
        }

        execute!(self.stdout, LeaveAlternateScreen, DisableMouseCapture, cursor::Show)?;
        terminal::disable_raw_mode()?;

        self.initialized = false;
        Ok(())
    }

    /// Get current terminal size.
    ///
    /// # Errors
    ///
    /// Returns error if size query fails.
    pub fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    /// Render content to terminal.
    ///
    /// The content should be pre-rendered ANSI escape sequences from the server.
    ///
    /// # Errors
    ///
    /// Returns error if write fails.
    pub fn render(&mut self, content: &str) -> io::Result<()> {
        // Move to top-left
        execute!(self.stdout, cursor::MoveTo(0, 0))?;

        // Write content (already contains ANSI codes)
        write!(self.stdout, "{content}")?;

        self.stdout.flush()?;
        Ok(())
    }

    /// Clear screen.
    ///
    /// # Errors
    ///
    /// Returns error if clear fails.
    pub fn clear(&mut self) -> io::Result<()> {
        execute!(self.stdout, terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0))
    }

    /// Set cursor position.
    ///
    /// # Errors
    ///
    /// Returns error if cursor move fails.
    pub fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        execute!(self.stdout, cursor::MoveTo(x, y))
    }

    /// Show cursor.
    ///
    /// # Errors
    ///
    /// Returns error if show fails.
    pub fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.stdout, cursor::Show)
    }

    /// Hide cursor.
    ///
    /// # Errors
    ///
    /// Returns error if hide fails.
    pub fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.stdout, cursor::Hide)
    }

    /// Set cursor style (block, bar, underline).
    ///
    /// # Errors
    ///
    /// Returns error if style change fails.
    pub fn set_cursor_style(&mut self, style: CursorStyleKind) -> io::Result<()> {
        let crossterm_style = match style {
            CursorStyleKind::Block => SetCursorStyle::SteadyBlock,
            CursorStyleKind::Bar => SetCursorStyle::SteadyBar,
            CursorStyleKind::Underline => SetCursorStyle::SteadyUnderScore,
        };
        execute!(self.stdout, crossterm_style)
    }

    /// Flush output buffer.
    ///
    /// # Errors
    ///
    /// Returns error if flush fails.
    pub fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }

    /// Write a line of content (with newline).
    ///
    /// # Errors
    ///
    /// Returns error if write fails.
    pub fn write_line(&mut self, content: &str) -> io::Result<()> {
        writeln!(self.stdout, "{content}")
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // Best effort cleanup on drop
        let _ = self.cleanup();
    }
}
