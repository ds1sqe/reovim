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
mod tests {
    use crate::FrameBuffer;

    use super::*;

    #[test]
    fn test_popup_width_normal() {
        // 40 wide: pw = max(24, 30).min(36) = 30
        assert_eq!(popup_width(40), 30);
    }

    #[test]
    fn test_popup_width_wide() {
        // 100 wide: pw = max(60, 30).min(96) = 60
        assert_eq!(popup_width(100), 60);
    }

    #[test]
    fn test_popup_width_narrow() {
        // 32 wide: 32*3/5=19, max(19,30)=30, min(30,28)=28
        assert_eq!(popup_width(32), 28);
    }

    #[test]
    fn test_popup_x_centered() {
        assert_eq!(popup_x(40, 30), 5);
        assert_eq!(popup_x(100, 60), 20);
    }

    #[test]
    fn test_render_box_border() {
        let mut fb = FrameBuffer::new(20, 10);
        let style = Style::default();
        render_box_border(&mut fb, 2, 3, 10, 4, &style);

        // Top corners
        assert_eq!(fb.get(2, 3).unwrap().char, '\u{256D}'); // ╭
        assert_eq!(fb.get(11, 3).unwrap().char, '\u{256E}'); // ╮
        // Top edge
        assert_eq!(fb.get(3, 3).unwrap().char, '\u{2500}'); // ─
        // Side
        assert_eq!(fb.get(2, 4).unwrap().char, '\u{2502}'); // │
        assert_eq!(fb.get(11, 4).unwrap().char, '\u{2502}'); // │
        // Bottom corners
        assert_eq!(fb.get(2, 6).unwrap().char, '\u{2570}'); // ╰
        assert_eq!(fb.get(11, 6).unwrap().char, '\u{256F}'); // ╯
    }

    #[test]
    fn test_render_box_border_too_small() {
        let mut fb = FrameBuffer::new(10, 10);
        let style = Style::default();
        // Width=1 is too small — should be a no-op
        render_box_border(&mut fb, 0, 0, 1, 3, &style);
        assert_eq!(fb.get(0, 0).unwrap().char, ' '); // unchanged
    }
}
