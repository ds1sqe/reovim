//! Shared popup rendering utilities.
//!
//! Generic helpers for rendering bordered floating popups.
//! Used by client-side TUI extensions (which-key, cmdline, etc.).

use crate::{Style, render_backend::RenderBackend};

/// Calculate the popup width from the terminal width.
///
/// 60% of screen width, clamped to \[30, width-4\].
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn popup_width(terminal_width: u16) -> u16 {
    (terminal_width * 3 / 5)
        .max(30)
        .min(terminal_width.saturating_sub(4))
}

/// Calculate the popup X position (centered).
#[must_use]
pub const fn popup_x(terminal_width: u16, pw: u16) -> u16 {
    terminal_width.saturating_sub(pw) / 2
}

/// Render a Unicode box border.
///
/// ```text
/// ╭──────╮
/// │      │
/// ╰──────╯
/// ```
pub fn render_box_border(
    backend: &mut dyn RenderBackend,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    style: &Style,
) {
    if w < 2 || h < 2 {
        return;
    }
    // Top: ╭───╮
    backend.set_cell(x, y, '\u{256D}', style);
    for col in 1..w.saturating_sub(1) {
        backend.set_cell(x + col, y, '\u{2500}', style);
    }
    backend.set_cell(x + w - 1, y, '\u{256E}', style);

    // Sides: │ ... │
    for row in 1..h.saturating_sub(1) {
        backend.set_cell(x, y + row, '\u{2502}', style);
        backend.set_cell(x + w - 1, y + row, '\u{2502}', style);
    }

    // Bottom: ╰───╯
    backend.set_cell(x, y + h - 1, '\u{2570}', style);
    for col in 1..w.saturating_sub(1) {
        backend.set_cell(x + col, y + h - 1, '\u{2500}', style);
    }
    backend.set_cell(x + w - 1, y + h - 1, '\u{256F}', style);
}

#[cfg(test)]
#[path = "popup_utils_tests.rs"]
mod tests;
