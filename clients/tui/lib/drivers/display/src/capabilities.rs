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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_returns_valid_capabilities() {
        let caps = DisplayCapabilities::detect();
        // color_mode is always set (it has a detection logic)
        let _ = caps.color_mode;
        // These are booleans, just verify they don't panic
        let _ = caps.supports_underline_color;
        let _ = caps.supports_extended_underlines;
        let _ = caps.supports_mouse;
        let _ = caps.supports_kitty_graphics;
        let _ = caps.supports_sixel;
    }

    #[test]
    fn test_default_matches_detect() {
        let detected = DisplayCapabilities::detect();
        let default = DisplayCapabilities::default();
        // Both should produce the same result
        assert_eq!(detected.color_mode, default.color_mode);
        assert_eq!(detected.supports_mouse, default.supports_mouse);
        assert_eq!(detected.supports_sixel, default.supports_sixel);
    }

    #[test]
    fn test_mouse_always_supported() {
        let caps = DisplayCapabilities::detect();
        assert!(caps.supports_mouse);
    }

    #[test]
    fn test_sixel_not_supported_by_default() {
        let caps = DisplayCapabilities::detect();
        // Sixel requires terminal query, always false in detect()
        assert!(!caps.supports_sixel);
    }

    #[test]
    fn test_capabilities_is_clone() {
        let caps = DisplayCapabilities::detect();
        let cloned = caps.clone();
        assert_eq!(caps.color_mode, cloned.color_mode);
        assert_eq!(caps.supports_mouse, cloned.supports_mouse);
    }

    #[test]
    fn test_capabilities_is_debug() {
        let caps = DisplayCapabilities::detect();
        let debug = format!("{caps:?}");
        assert!(debug.contains("DisplayCapabilities"));
    }
}
