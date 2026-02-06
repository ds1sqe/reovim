//! Terminal environment types for TUI mode selection.
//!
//! This module provides:
//! - `TermEnv`: Terminal environment parameters (size, color mode)
//! - `TuiEnv`: Mode selector (Real TTY vs Headless)
//!
//! # Design Principle
//!
//! The only difference between interactive and headless modes is the
//! terminal environment source:
//! - Real TTY: auto-detect from terminal
//! - Headless: explicit parameters provided
//!
//! Everything else (theme, debug, behavior) is identical.

use std::io;

use reovim_driver_display::ColorMode;

/// Terminal environment parameters.
///
/// Used by `TuiApp` internally regardless of mode.
/// For `TuiEnv::Real`, detected from the terminal.
/// For `TuiEnv::Headless`, provided explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermEnv {
    /// Terminal size (width, height) in cells.
    pub size: (u16, u16),
    /// Color mode supported by the terminal.
    pub color_mode: ColorMode,
}

impl TermEnv {
    /// Create a new terminal environment with explicit parameters.
    #[must_use]
    pub const fn new(size: (u16, u16), color_mode: ColorMode) -> Self {
        Self { size, color_mode }
    }

    /// Detect terminal environment from real TTY.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal size detection fails.
    pub fn from_terminal() -> io::Result<Self> {
        let size = reovim_driver_tui::Terminal::size()?;
        let color_mode = detect_color_mode();
        Ok(Self { size, color_mode })
    }

    /// Create a headless environment with default settings.
    ///
    /// Uses 80x24 size and `TrueColor` mode.
    #[must_use]
    pub const fn headless_default() -> Self {
        Self {
            size: (80, 24),
            color_mode: ColorMode::TrueColor,
        }
    }
}

impl Default for TermEnv {
    fn default() -> Self {
        Self::headless_default()
    }
}

/// TUI environment selector.
///
/// Determines how the terminal environment is obtained:
/// - `Real`: Auto-detect from TTY (interactive mode)
/// - `Headless`: Use explicit parameters (testing/scripting)
#[derive(Debug, Clone)]
pub enum TuiEnv {
    /// Real TTY mode - auto-detect terminal environment.
    ///
    /// Uses the actual terminal's size and color capabilities.
    /// Input comes from keyboard, output goes to terminal.
    Real,

    /// Headless mode - explicit terminal environment.
    ///
    /// Uses the provided `TermEnv` parameters.
    /// Input comes from programmatic API, output to frame buffer.
    Headless(TermEnv),
}

impl TuiEnv {
    /// Create a headless environment with the given size.
    ///
    /// Uses `TrueColor` mode by default.
    #[must_use]
    pub const fn headless(width: u16, height: u16) -> Self {
        Self::Headless(TermEnv::new((width, height), ColorMode::TrueColor))
    }

    /// Create a headless environment with full parameters.
    #[must_use]
    pub const fn headless_with_env(env: TermEnv) -> Self {
        Self::Headless(env)
    }
}

/// Detect the color mode supported by the terminal.
///
/// Checks environment variables and terminal capabilities.
/// Defaults to `TrueColor` for modern terminals.
fn detect_color_mode() -> ColorMode {
    // Check COLORTERM for true color support
    if let Ok(colorterm) = std::env::var("COLORTERM")
        && (colorterm == "truecolor" || colorterm == "24bit")
    {
        return ColorMode::TrueColor;
    }

    // Check TERM for 256 color support
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("256color") {
            return ColorMode::Color256;
        }
        if term.contains("color") || term.contains("xterm") || term.contains("screen") {
            return ColorMode::Ansi16;
        }
    }

    // Default to TrueColor for modern terminals
    ColorMode::TrueColor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_term_env_new() {
        let env = TermEnv::new((120, 40), ColorMode::Color256);
        assert_eq!(env.size, (120, 40));
        assert_eq!(env.color_mode, ColorMode::Color256);
    }

    #[test]
    fn test_term_env_default() {
        let env = TermEnv::default();
        assert_eq!(env.size, (80, 24));
        assert_eq!(env.color_mode, ColorMode::TrueColor);
    }

    #[test]
    fn test_tui_env_headless() {
        let env = TuiEnv::headless(100, 30);
        match env {
            TuiEnv::Headless(term_env) => {
                assert_eq!(term_env.size, (100, 30));
                assert_eq!(term_env.color_mode, ColorMode::TrueColor);
            }
            TuiEnv::Real => panic!("Expected Headless"),
        }
    }
}
