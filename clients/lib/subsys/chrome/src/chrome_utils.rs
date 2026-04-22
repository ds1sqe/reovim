//! Shared chrome rendering utilities.
//!
//! Generic helpers for rendering bordered floating popups.
//! Used by native `ClientModule` chrome implementations.

use reovim_client_subsys_module::{ChromeSurface, Style};

/// Calculate the popup width from the terminal width.
///
/// 60% of screen width, clamped to \[30, width-4\].
#[must_use]
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

/// Render a Unicode box border on a `ChromeSurface`.
///
/// ```text
/// ╭──────╮
/// │      │
/// ╰──────╯
/// ```
pub fn render_box_border(
    surface: &mut dyn ChromeSurface,
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
    surface.write_styled(x, y, "\u{256D}", style.clone());
    for col in 1..w.saturating_sub(1) {
        surface.write_styled(x + col, y, "\u{2500}", style.clone());
    }
    surface.write_styled(x + w - 1, y, "\u{256E}", style.clone());

    // Sides: │ ... │
    for row in 1..h.saturating_sub(1) {
        surface.write_styled(x, y + row, "\u{2502}", style.clone());
        surface.write_styled(x + w - 1, y + row, "\u{2502}", style.clone());
    }

    // Bottom: ╰───╯
    surface.write_styled(x, y + h - 1, "\u{2570}", style.clone());
    for col in 1..w.saturating_sub(1) {
        surface.write_styled(x + col, y + h - 1, "\u{2500}", style.clone());
    }
    surface.write_styled(x + w - 1, y + h - 1, "\u{256F}", style.clone());
}

#[cfg(test)]
#[path = "chrome_utils_tests.rs"]
mod tests;
