//! Render commands for terminal output.

/// Commands emitted during rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderCommand {
    /// Move cursor to position.
    MoveTo { x: u16, y: u16 },
    /// Print a string at current position.
    Print(String),
    /// Set style (ANSI escape sequence).
    SetStyle(String),
    /// Reset style to default.
    ResetStyle,
    /// Clear to end of line.
    ClearToEndOfLine,
    /// Clear entire line.
    ClearLine,
    /// Clear rectangular region.
    ClearRect {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    },
    /// Show cursor.
    ShowCursor,
    /// Hide cursor.
    HideCursor,
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
