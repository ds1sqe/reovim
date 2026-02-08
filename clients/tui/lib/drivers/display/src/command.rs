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
mod tests {
    use super::*;

    #[test]
    fn test_move_to() {
        let cmd = RenderCommand::MoveTo { x: 10, y: 20 };
        assert_eq!(cmd, RenderCommand::MoveTo { x: 10, y: 20 });
    }

    #[test]
    fn test_print() {
        let cmd = RenderCommand::Print("hello".to_string());
        assert_eq!(cmd, RenderCommand::Print("hello".to_string()));
    }

    #[test]
    fn test_set_style() {
        let cmd = RenderCommand::SetStyle("\x1b[1m".to_string());
        assert_eq!(cmd, RenderCommand::SetStyle("\x1b[1m".to_string()));
    }

    #[test]
    fn test_reset_style() {
        let cmd = RenderCommand::ResetStyle;
        assert_eq!(cmd, RenderCommand::ResetStyle);
    }

    #[test]
    fn test_clear_commands() {
        assert_eq!(RenderCommand::ClearToEndOfLine, RenderCommand::ClearToEndOfLine);
        assert_eq!(RenderCommand::ClearLine, RenderCommand::ClearLine);
    }

    #[test]
    fn test_clear_rect() {
        let cmd = RenderCommand::ClearRect {
            x: 5,
            y: 10,
            width: 20,
            height: 15,
        };
        assert_eq!(
            cmd,
            RenderCommand::ClearRect {
                x: 5,
                y: 10,
                width: 20,
                height: 15,
            }
        );
    }

    #[test]
    fn test_cursor_commands() {
        assert_eq!(RenderCommand::ShowCursor, RenderCommand::ShowCursor);
        assert_eq!(RenderCommand::HideCursor, RenderCommand::HideCursor);
        assert_ne!(RenderCommand::ShowCursor, RenderCommand::HideCursor);
    }

    #[test]
    fn test_clone() {
        let cmd = RenderCommand::Print("test".to_string());
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_debug() {
        let cmd = RenderCommand::MoveTo { x: 1, y: 2 };
        let debug = format!("{cmd:?}");
        assert!(debug.contains("MoveTo"));
    }

    #[test]
    fn test_inequality() {
        let cmd1 = RenderCommand::MoveTo { x: 1, y: 2 };
        let cmd2 = RenderCommand::MoveTo { x: 3, y: 4 };
        assert_ne!(cmd1, cmd2);
    }
}
