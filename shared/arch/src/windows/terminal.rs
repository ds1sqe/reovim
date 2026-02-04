//! Windows terminal stub implementation.
//!
//! This is a placeholder for future Windows support.

use std::io;

use crate::traits::{ClearType, RawModeGuard, Terminal, TerminalSize};

/// Windows terminal stub.
///
/// This is a placeholder implementation that will panic if used.
/// Full Windows support will be implemented in a future phase.
pub struct WindowsTerminal;

impl WindowsTerminal {
    /// Create a new Windows terminal.
    ///
    /// # Panics
    ///
    /// Panics because Windows support is not yet implemented.
    pub fn new() -> io::Result<Self> {
        todo!("Windows terminal support not yet implemented")
    }
}

impl Default for WindowsTerminal {
    fn default() -> Self {
        todo!("Windows terminal support not yet implemented")
    }
}

impl Terminal for WindowsTerminal {
    fn size(&self) -> io::Result<TerminalSize> {
        todo!("Windows terminal support not yet implemented")
    }

    fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard> {
        todo!("Windows terminal support not yet implemented")
    }

    fn supports_keyboard_enhancement(&self) -> bool {
        todo!("Windows terminal support not yet implemented")
    }

    fn enable_keyboard_enhancement(&mut self) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn disable_keyboard_enhancement(&mut self) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn leave_alternate_screen(&mut self) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn move_cursor(&mut self, _col: u16, _row: u16) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn clear(&mut self, _clear_type: ClearType) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn enable_mouse_capture(&mut self) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn disable_mouse_capture(&mut self) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }

    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        todo!("Windows terminal support not yet implemented")
    }

    fn flush(&mut self) -> io::Result<()> {
        todo!("Windows terminal support not yet implemented")
    }
}
