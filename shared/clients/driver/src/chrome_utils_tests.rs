use super::*;
use crate::types::Color;

// =============================================================================
// MockSurface for testing chrome_utils
// =============================================================================

struct MockSurface {
    writes: std::cell::RefCell<Vec<(u16, u16, String, Style)>>,
    width: u16,
    height: u16,
}

impl MockSurface {
    fn new(width: u16, height: u16) -> Self {
        Self {
            writes: std::cell::RefCell::new(Vec::new()),
            width,
            height,
        }
    }

    fn write_count(&self) -> usize {
        self.writes.borrow().len()
    }

    fn char_at(&self, x: u16, y: u16) -> Option<String> {
        self.writes
            .borrow()
            .iter()
            .rev()
            .find(|(wx, wy, _, _)| *wx == x && *wy == y)
            .map(|(_, _, s, _)| s.clone())
    }
}

impl RenderSurface for MockSurface {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        #[allow(clippy::cast_possible_truncation)]
        let len = text.len() as u16;
        self.writes
            .borrow_mut()
            .push((x, y, text.to_string(), style));
        len
    }

    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}

    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: Color) {}

    fn fill(&mut self, _rect: crate::Rect, _ch: char, _style: Style) {}

    fn clear(&mut self, _rect: crate::Rect) {}

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

// =============================================================================
// popup_width tests
// =============================================================================

#[test]
fn popup_width_standard_terminal() {
    // 80 * 3 / 5 = 48
    assert_eq!(popup_width(80), 48);
}

#[test]
fn popup_width_wide_terminal() {
    // 200 * 3 / 5 = 120, clamped to min(120, 196) = 120
    assert_eq!(popup_width(200), 120);
}

#[test]
fn popup_width_narrow_terminal_clamps_to_min() {
    // 40 * 3 / 5 = 24, clamped to max(24, 30) = 30, then min(30, 36) = 30
    assert_eq!(popup_width(40), 30);
}

#[test]
fn popup_width_very_narrow() {
    // 20 * 3 / 5 = 12, clamped to max(12, 30) = 30, then min(30, 16) = 16
    assert_eq!(popup_width(20), 16);
}

// =============================================================================
// popup_x tests
// =============================================================================

#[test]
fn popup_x_centers_horizontally() {
    // (80 - 48) / 2 = 16
    assert_eq!(popup_x(80, 48), 16);
}

#[test]
fn popup_x_wide_popup() {
    // (80 - 80) / 2 = 0
    assert_eq!(popup_x(80, 80), 0);
}

#[test]
fn popup_x_narrow_popup() {
    // (80 - 10) / 2 = 35
    assert_eq!(popup_x(80, 10), 35);
}

// =============================================================================
// render_box_border tests
// =============================================================================

#[test]
fn render_box_border_minimum_size() {
    let mut surface = MockSurface::new(80, 24);
    let style = Style::new();
    render_box_border(&mut surface, 0, 0, 2, 2, &style);
    // 2x2 border: 4 corners only
    assert_eq!(surface.char_at(0, 0), Some("\u{256D}".to_string())); // ╭
    assert_eq!(surface.char_at(1, 0), Some("\u{256E}".to_string())); // ╮
    assert_eq!(surface.char_at(0, 1), Some("\u{2570}".to_string())); // ╰
    assert_eq!(surface.char_at(1, 1), Some("\u{256F}".to_string())); // ╯
}

#[test]
fn render_box_border_3x3() {
    let mut surface = MockSurface::new(80, 24);
    let style = Style::new();
    render_box_border(&mut surface, 5, 5, 3, 3, &style);
    // Top row
    assert_eq!(surface.char_at(5, 5), Some("\u{256D}".to_string()));
    assert_eq!(surface.char_at(6, 5), Some("\u{2500}".to_string())); // ─
    assert_eq!(surface.char_at(7, 5), Some("\u{256E}".to_string()));
    // Middle row
    assert_eq!(surface.char_at(5, 6), Some("\u{2502}".to_string())); // │
    assert_eq!(surface.char_at(7, 6), Some("\u{2502}".to_string()));
    // Bottom row
    assert_eq!(surface.char_at(5, 7), Some("\u{2570}".to_string()));
    assert_eq!(surface.char_at(6, 7), Some("\u{2500}".to_string()));
    assert_eq!(surface.char_at(7, 7), Some("\u{256F}".to_string()));
}

#[test]
fn render_box_border_too_small_width() {
    let mut surface = MockSurface::new(80, 24);
    let style = Style::new();
    render_box_border(&mut surface, 0, 0, 1, 5, &style);
    assert_eq!(surface.write_count(), 0); // Nothing rendered
}

#[test]
fn render_box_border_too_small_height() {
    let mut surface = MockSurface::new(80, 24);
    let style = Style::new();
    render_box_border(&mut surface, 0, 0, 5, 1, &style);
    assert_eq!(surface.write_count(), 0); // Nothing rendered
}

#[test]
fn render_box_border_with_offset() {
    let mut surface = MockSurface::new(80, 24);
    let style = Style::new();
    render_box_border(&mut surface, 10, 5, 4, 3, &style);
    // Verify corners are at correct offset
    assert_eq!(surface.char_at(10, 5), Some("\u{256D}".to_string()));
    assert_eq!(surface.char_at(13, 5), Some("\u{256E}".to_string()));
    assert_eq!(surface.char_at(10, 7), Some("\u{2570}".to_string()));
    assert_eq!(surface.char_at(13, 7), Some("\u{256F}".to_string()));
}

#[test]
fn render_box_border_uses_provided_style() {
    let mut surface = MockSurface::new(80, 24);
    let style = Style::new().fg(Color::Cyan);
    render_box_border(&mut surface, 0, 0, 2, 2, &style);
    // Verify style is passed through
    let writes = surface.writes.borrow();
    assert!(writes.iter().all(|(_, _, _, s)| s.fg == Some(Color::Cyan)));
}
