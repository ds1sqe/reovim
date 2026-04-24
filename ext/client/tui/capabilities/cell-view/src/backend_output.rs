//! [`BackendRasterOutput`] — production [`RasterOutput`] adapter over a
//! [`RenderBackend`].
//!
//! The rasterizer writes terminal-ready [`RasterCell`]s into a
//! [`RasterOutput`]. In tests that sink is a [`RecordingRasterOutput`];
//! in production it is the TUI backend itself, wrapped here.
//!
//! `size()` reports the **terminal** dimensions, not any doubled logical
//! height. The rasterizer clamps its own coordinate space against this;
//! if `size()` returned the doubled grid the clamp would be a no-op and
//! the adapter would emit past the bottom of the terminal.

use {
    reovim_driver_display::{Color, Style, render_backend::RenderBackend},
    reovim_ext_client_tui_cap_cell::{CellAttrs, CellColor, CellStyle},
};

use crate::raster::{RasterCell, RasterOutput};

/// Production [`RasterOutput`] that writes each [`RasterCell`] to a
/// [`RenderBackend`] via `set_cell`.
pub struct BackendRasterOutput<'b, B: RenderBackend + ?Sized> {
    backend: &'b mut B,
}

impl<'b, B: RenderBackend + ?Sized> BackendRasterOutput<'b, B> {
    /// Wrap a mutable backend reference.
    pub const fn new(backend: &'b mut B) -> Self {
        Self { backend }
    }
}

impl<B: RenderBackend + ?Sized> RasterOutput for BackendRasterOutput<'_, B> {
    fn set_cell(&mut self, cell: RasterCell) {
        let style = convert_cell_style_to_display_style(&cell.style);
        self.backend.set_cell(cell.x, cell.y, cell.ch, &style);
    }

    fn size(&self) -> (u16, u16) {
        self.backend.size()
    }
}

/// Convert a [`CellStyle`] (capability-layer styling) into a
/// [`Style`] (display-driver styling).
///
/// Color mapping:
/// - `CellColor::Rgb(r, g, b)` → `Color::Rgb { r, g, b }`
/// - `CellColor::Ansi256(n)`   → `Color::AnsiValue(n)`
/// - `CellColor::Named(n)`     → `Color::AnsiValue(n)` (0..=15 palette
///   entries live in the same ANSI-256 index space)
/// - `CellColor::Default`      → `Color::Reset`
///
/// `fg: None` / `bg: None` carry through as `None` (inherit).
#[must_use]
pub fn convert_cell_style_to_display_style(src: &CellStyle) -> Style {
    let mut out = Style::new();
    if let Some(fg) = src.fg {
        out.fg = Some(convert_color(fg));
    }
    if let Some(bg) = src.bg {
        out.bg = Some(convert_color(bg));
    }
    if src.attrs.contains(CellAttrs::BOLD) {
        out = out.bold();
    }
    if src.attrs.contains(CellAttrs::ITALIC) {
        out = out.italic();
    }
    if src.attrs.contains(CellAttrs::UNDERLINE) {
        out = out.underline();
    }
    if src.attrs.contains(CellAttrs::REVERSE) {
        out = out.reverse();
    }
    if src.attrs.contains(CellAttrs::DIM) {
        out = out.dim();
    }
    out
}

const fn convert_color(c: CellColor) -> Color {
    match c {
        CellColor::Rgb(r, g, b) => Color::Rgb { r, g, b },
        CellColor::Ansi256(n) | CellColor::Named(n) => Color::AnsiValue(n),
        CellColor::Default => Color::Reset,
    }
}
