//! Terminal session management.
//!
//! Handles entering and exiting terminal raw mode and alternate screen.
//! Uses RAII pattern - terminal is automatically restored on drop.

use std::io::{self, Stdout, Write};

use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};

/// Terminal session guard.
///
/// Manages the terminal state transition:
/// - Enter: raw mode + alternate screen + hide cursor
/// - Exit: restore original state
///
/// Uses RAII pattern for automatic cleanup on drop.
pub struct Terminal {
    /// Standard output handle.
    stdout: Stdout,
    /// Whether we're in raw mode.
    raw_mode: bool,
    /// Whether we're in alternate screen.
    alternate_screen: bool,
}

impl Terminal {
    /// Enter terminal session.
    ///
    /// Enables raw mode, switches to alternate screen, and hides cursor.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup fails.
    pub fn enter() -> io::Result<Self> {
        let mut stdout = io::stdout();

        // Enable raw mode
        terminal::enable_raw_mode()?;

        // Enter alternate screen and hide cursor
        execute!(
            stdout,
            terminal::EnterAlternateScreen,
            cursor::Hide,
            terminal::Clear(ClearType::All)
        )?;

        Ok(Self {
            stdout,
            raw_mode: true,
            alternate_screen: true,
        })
    }

    /// Get terminal size.
    ///
    /// # Errors
    ///
    /// Returns an error if size cannot be determined.
    pub fn size() -> io::Result<(u16, u16)> {
        terminal::size()
    }

    /// Clear the terminal screen.
    ///
    /// # Errors
    ///
    /// Returns an error if clear fails.
    pub fn clear(&mut self) -> io::Result<()> {
        execute!(self.stdout, terminal::Clear(ClearType::All))
    }

    /// Get mutable reference to stdout for rendering.
    pub const fn stdout(&mut self) -> &mut Stdout {
        &mut self.stdout
    }

    /// Flush stdout.
    ///
    /// # Errors
    ///
    /// Returns an error if flush fails.
    pub fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }

    /// Exit terminal session.
    ///
    /// Restores original terminal state. Called automatically on drop.
    ///
    /// # Errors
    ///
    /// Returns an error if restoration fails.
    pub fn exit(&mut self) -> io::Result<()> {
        if self.alternate_screen {
            execute!(self.stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
            self.alternate_screen = false;
        }

        if self.raw_mode {
            terminal::disable_raw_mode()?;
            self.raw_mode = false;
        }

        Ok(())
    }

    /// Check if in raw mode.
    #[must_use]
    pub const fn is_raw_mode(&self) -> bool {
        self.raw_mode
    }

    /// Check if in alternate screen.
    #[must_use]
    pub const fn is_alternate_screen(&self) -> bool {
        self.alternate_screen
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Best-effort restoration
        let _ = self.exit();
    }
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("raw_mode", &self.raw_mode)
            .field("alternate_screen", &self.alternate_screen)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_size() {
        // Should not fail even in CI (will get default size)
        let result = Terminal::size();
        // We don't assert Ok because CI may not have a terminal
        // Just verify it doesn't panic
        let _ = result;
    }
}
