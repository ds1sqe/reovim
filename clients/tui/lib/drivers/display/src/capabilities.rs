//! Terminal capability detection.

use crate::highlight::ColorMode;

/// Terminal display capabilities.
///
/// Contains detected terminal features for adaptive rendering.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct DisplayCapabilities {
    /// Color rendering mode (`Ansi16`, `Color256`, `TrueColor`).
    pub color_mode: ColorMode,
    /// Supports ANSI code 58 (underline color).
    pub supports_underline_color: bool,
    /// Supports Kitty/VTE extended underlines (curly, dotted, dashed).
    pub supports_extended_underlines: bool,
    /// Supports mouse input.
    pub supports_mouse: bool,
    /// Supports Kitty graphics protocol.
    pub supports_kitty_graphics: bool,
    /// Supports Sixel graphics.
    pub supports_sixel: bool,
}

impl DisplayCapabilities {
    /// Detect terminal capabilities from environment.
    #[must_use]
    pub fn detect() -> Self {
        Self {
            color_mode: ColorMode::detect(),
            supports_underline_color: Self::detect_underline_color(),
            supports_extended_underlines: Self::detect_extended_underlines(),
            supports_mouse: true, // Most terminals support this
            supports_kitty_graphics: Self::detect_kitty(),
            supports_sixel: false, // Requires terminal query
        }
    }

    fn detect_underline_color() -> bool {
        // Check for modern terminals (Kitty, iTerm2, VTE, Windows Terminal)
        std::env::var("TERM").is_ok_and(|t| {
            t.contains("kitty") || t.contains("xterm-256color") || t.contains("alacritty")
        }) || std::env::var("TERM_PROGRAM").is_ok_and(|p| p == "iTerm.app")
            || std::env::var("WT_SESSION").is_ok()
    }

    fn detect_extended_underlines() -> bool {
        // Kitty and VTE-based terminals
        std::env::var("TERM").is_ok_and(|t| t.contains("kitty"))
            || std::env::var("VTE_VERSION").is_ok()
    }

    fn detect_kitty() -> bool {
        std::env::var("TERM").is_ok_and(|t| t.contains("kitty"))
    }
}

impl Default for DisplayCapabilities {
    fn default() -> Self {
        Self::detect()
    }
}
