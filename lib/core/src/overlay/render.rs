//! Overlay rendering trait

use crate::highlight::{ColorMode, Theme};

/// Trait for overlays that can render themselves
///
/// Returns positioned lines ready for output. Each tuple contains:
/// - The rendered line content (including ANSI styles)
/// - X position (column)
/// - Y position (row)
pub trait OverlayRender {
    /// Render the overlay to positioned lines
    ///
    /// # Arguments
    /// - `theme`: Current color theme
    /// - `color_mode`: Current color mode (`TrueColor`, `Ansi256`, etc.)
    ///
    /// # Returns
    /// A vector of `(line_content, x, y)` tuples ready for terminal output.
    fn render(&self, theme: &Theme, color_mode: ColorMode) -> Vec<(String, u16, u16)>;
}
