//! Headless output for testing and scripting.
//!
//! All methods are no-ops — the `FrameBuffer` owned by `TuiApp` is the
//! final output. Capture reads directly from it.

use std::io;

use reovim_driver_display::FrameBuffer;

use crate::tui_output::{CursorStyleHint, TuiOutput};

/// Headless output — all no-ops.
///
/// In headless mode, `TuiApp` owns the `FrameBuffer` and that IS the output.
/// No terminal session, no screen buffer, no cursor management.
pub struct HeadlessOutput;

impl TuiOutput for HeadlessOutput {
    fn flush(&mut self, _frame: &FrameBuffer) -> io::Result<()> {
        // No-op: FrameBuffer is the final output
        Ok(())
    }

    fn position_cursor(&mut self, _x: u16, _y: u16) {
        // No-op: no terminal cursor
    }

    fn uses_terminal_cursor(&self) -> bool {
        false
    }

    fn set_cursor_style(&mut self, _style: CursorStyleHint) {
        // No-op: no terminal cursor
    }

    fn set_cursor_visible(&mut self, _visible: bool) {
        // No-op: no terminal cursor
    }

    fn invalidate(&mut self) {
        // No-op: no cached screen state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headless_output_flush() {
        let mut output = HeadlessOutput;
        let fb = FrameBuffer::new(80, 24);
        assert!(output.flush(&fb).is_ok());
    }

    #[test]
    fn test_headless_output_no_terminal_cursor() {
        let output = HeadlessOutput;
        assert!(!output.uses_terminal_cursor());
    }
}
