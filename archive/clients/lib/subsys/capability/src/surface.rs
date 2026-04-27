//! Surface abstraction for chrome + viewport rendering.
//!
//! `ChromeSurface` is the primitive drawing contract. Platform adapters implement
//! it for their specific backend (cell grid, canvas, native views). It lives in
//! the capability crate so both `chrome` and `module` can depend on it without
//! the cycle that would result from `chrome → module`.

use crate::draw::{Color, Rect, Style};

/// Abstraction over the rendering target for chrome + viewport modules.
///
/// Provides primitive drawing operations. Platform adapters implement this
/// for their specific rendering backend (cell grid, canvas, native views).
pub trait ChromeSurface {
    /// Write styled text at position. Returns the number of columns consumed.
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16;

    /// Apply a style to an existing cell (without changing its content).
    fn apply_style(&mut self, x: u16, y: u16, style: Style);

    /// Override the background color of a cell.
    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color);

    /// Fill a rectangle with a character and style.
    fn fill(&mut self, rect: Rect, ch: char, style: Style);

    /// Clear a rectangle (reset to default).
    fn clear(&mut self, rect: Rect);

    /// Size of the surface in cells (columns, rows).
    fn size(&self) -> (u16, u16);
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
