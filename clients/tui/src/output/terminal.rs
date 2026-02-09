//! Terminal output for interactive TUI.
//!
//! Wraps `Terminal`, `Screen`, and `Cursor` to provide display-side I/O.
//! Input is handled separately by the TTY reader task (`spawn_tty_reader`).

use std::io;

use {
    reovim_driver_display::FrameBuffer,
    reovim_driver_tui::{Cursor, CursorStyle, Screen, Terminal},
};

use crate::{
    render_backend::RenderBackend,
    tui_output::{CursorStyleHint, TuiOutput},
};

/// Interactive terminal output.
///
/// Owns the terminal session (raw mode, alternate screen), screen buffer
/// (with diff optimization), and cursor state. Restored on drop.
pub struct TerminalOutput {
    /// Terminal session (raw mode, alternate screen).
    terminal: Terminal,
    /// Screen buffer and diff renderer.
    screen: Screen,
    /// Cursor manager for terminal cursor.
    cursor: Cursor,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl TerminalOutput {
    /// Create a new terminal output.
    ///
    /// Enters raw mode and alternate screen. Restored on drop.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal initialization fails.
    pub fn new() -> io::Result<Self> {
        let terminal = Terminal::enter()?;
        let (width, height) = Terminal::size()?;
        let screen = Screen::new(width, height);
        let cursor = Cursor::new();

        Ok(Self {
            terminal,
            screen,
            cursor,
        })
    }

    /// Get the current terminal size.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal size detection fails.
    pub fn terminal_size() -> io::Result<(u16, u16)> {
        Terminal::size()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl TuiOutput for TerminalOutput {
    #[allow(clippy::cast_possible_truncation)]
    fn flush(&mut self, frame: &FrameBuffer) -> io::Result<()> {
        let fw = frame.width();
        let fh = frame.height();

        // Resize screen if dimensions changed
        if fw != self.screen.width() || fh != self.screen.height() {
            self.screen.resize(fw, fh);
        }

        // Copy cells from display::FrameBuffer → Screen via RenderBackend trait.
        // Screen's RenderBackend impl handles display::Style → tui::Style conversion.
        self.screen.clear();
        for y in 0..fh {
            if let Some(row) = frame.row(y) {
                for (x, cell) in row.iter().enumerate() {
                    if cell.is_continuation {
                        continue;
                    }
                    let x = x as u16;
                    if x < fw {
                        RenderBackend::set_cell(&mut self.screen, x, y, cell.char, &cell.style);
                    }
                }
            }
        }

        // Render to terminal with diff optimization
        self.screen.render(&mut self.terminal)
    }

    fn position_cursor(&mut self, x: u16, y: u16) {
        self.cursor.move_to(x, y);
        let _ = self.cursor.apply();
    }

    fn uses_terminal_cursor(&self) -> bool {
        true
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
}

#[cfg(test)]
mod tests {
    // Tests require a real terminal, skip in CI
}
