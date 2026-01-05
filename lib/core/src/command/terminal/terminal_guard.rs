use {
    reovim_sys::{
        cursor, execute,
        terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    },
    std::io,
};

/// RAII guard for terminal state that ensures cleanup even on panic.
///
/// This guard manages the terminal lifecycle:
/// - Enters alternate screen mode to isolate editor UI from shell
/// - Enables raw mode for key capture
/// - Restores terminal state on Drop (guaranteed even on panic)
/// - Prevents visual artifacts (afterimage, "%" marker) on exit
pub struct TerminalGuard {
    enabled: bool, // Skip cleanup in server/test modes
}

impl TerminalGuard {
    /// Create a new terminal guard for interactive mode.
    ///
    /// This will:
    /// - Enter alternate screen (isolates editor from shell)
    /// - Hide cursor (editor manages cursor rendering)
    /// - Enable raw mode (capture keys, disable line buffering)
    ///
    /// # Errors
    ///
    /// Returns error if terminal setup fails.
    pub fn new_interactive() -> io::Result<Self> {
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
        terminal::enable_raw_mode()?;
        Ok(Self { enabled: true })
    }

    /// Create a disabled guard that skips all terminal operations.
    ///
    /// Used for server/stdio modes where terminal is not managed by reovim.
    #[must_use]
    pub const fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Clean up terminal state.
    ///
    /// This function is called by Drop and performs cleanup in the correct order:
    /// 1. Disable raw mode first (allows terminal to process commands)
    /// 2. Show cursor (make it visible again)
    /// 3. Leave alternate screen (restore shell content)
    /// 4. Print final newline (prevents "%" marker in zsh)
    fn cleanup() -> io::Result<()> {
        terminal::disable_raw_mode()?;
        execute!(io::stdout(), cursor::Show, LeaveAlternateScreen)?;
        println!(); // Final newline prevents "%" marker
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.enabled {
            // Ignore errors in Drop - can't propagate them
            // This is safe: terminal will be cleaned up by OS anyway
            let _ = Self::cleanup();
        }
    }
}
