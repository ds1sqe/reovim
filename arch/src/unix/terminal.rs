//! Unix terminal implementation using crossterm.

use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

use {
    super::convert::convert_clear_type,
    crate::traits::{ClearType, RawModeGuard, Terminal, TerminalSize},
};

/// Unix terminal implementation wrapping crossterm.
pub struct UnixTerminal {
    stdout: Stdout,
    keyboard_enhancement_enabled: bool,
}

impl UnixTerminal {
    /// Create a new Unix terminal.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stdout: io::stdout(),
            keyboard_enhancement_enabled: false,
        }
    }
}

impl Default for UnixTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal for UnixTerminal {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn size(&self) -> io::Result<TerminalSize> {
        let (cols, rows) = terminal::size()?;
        Ok(TerminalSize::new(cols, rows))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard> {
        terminal::enable_raw_mode()?;
        Ok(RawModeGuard::new(terminal::disable_raw_mode))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn supports_keyboard_enhancement(&self) -> bool {
        terminal::supports_keyboard_enhancement().unwrap_or(false)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn enable_keyboard_enhancement(&mut self) -> io::Result<()> {
        if self.supports_keyboard_enhancement() && !self.keyboard_enhancement_enabled {
            execute!(
                self.stdout,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
            self.keyboard_enhancement_enabled = true;
        }
        Ok(())
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn disable_keyboard_enhancement(&mut self) -> io::Result<()> {
        if self.keyboard_enhancement_enabled {
            execute!(self.stdout, PopKeyboardEnhancementFlags)?;
            self.keyboard_enhancement_enabled = false;
        }
        Ok(())
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        execute!(self.stdout, EnterAlternateScreen)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn leave_alternate_screen(&mut self) -> io::Result<()> {
        execute!(self.stdout, LeaveAlternateScreen)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.stdout, cursor::Hide)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.stdout, cursor::Show)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn move_cursor(&mut self, col: u16, row: u16) -> io::Result<()> {
        execute!(self.stdout, cursor::MoveTo(col, row))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clear(&mut self, clear_type: ClearType) -> io::Result<()> {
        let ct_clear = convert_clear_type(clear_type);
        execute!(self.stdout, terminal::Clear(ct_clear))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn enable_mouse_capture(&mut self) -> io::Result<()> {
        execute!(self.stdout, EnableMouseCapture)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn disable_mouse_capture(&mut self) -> io::Result<()> {
        execute!(self.stdout, DisableMouseCapture)
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdout.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
