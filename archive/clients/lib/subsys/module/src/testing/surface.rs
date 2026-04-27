//! Minimal `ChromeSurface` fixture for subsys-module crate internal tests.
//!
//! Production consumers use `reovim_ext_client_tui_cap_cell::CellCapability`
//! as the canonical `ChromeSurface` impl. This fixture exists only because
//! the subsys crate cannot dev-depend on `cap-cell` (that would create a
//! cyclic dev-dependency).
//!
//! Scope: subsys-module-crate-internal tests only. Not re-exported from
//! `testing::*`. External crates (chrome modules, tests in other repos)
//! must use `CellCapability` instead.

use crate::{
    traits::ChromeSurface,
    types::{Color, Rect, Style},
};

/// A single displayable cell.
#[derive(Debug, Clone)]
struct TestCell {
    ch: char,
    style: Style,
}

impl Default for TestCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: Style::new(),
        }
    }
}

/// Minimal bounded 2-D grid implementing `ChromeSurface`.
///
/// Records the final composited state of each cell (no draw-log).
pub struct TestChromeGrid {
    width: u16,
    height: u16,
    cells: Vec<Vec<TestCell>>,
}

impl TestChromeGrid {
    /// Create a `width × height` grid filled with default cells.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let cells = (0..height)
            .map(|_| (0..width).map(|_| TestCell::default()).collect())
            .collect();
        Self {
            width,
            height,
            cells,
        }
    }

    /// True if any cell holds a non-space character.
    #[must_use]
    pub fn has_content(&self) -> bool {
        self.cells.iter().any(|row| row.iter().any(|c| c.ch != ' '))
    }
}

impl ChromeSurface for TestChromeGrid {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        if y >= self.height {
            return 0;
        }
        let mut cx = x;
        for ch in text.chars() {
            if cx >= self.width {
                break;
            }
            self.cells[y as usize][cx as usize] = TestCell {
                ch,
                style: style.clone(),
            };
            cx = cx.saturating_add(1);
        }
        cx.saturating_sub(x)
    }

    fn apply_style(&mut self, x: u16, y: u16, style: Style) {
        if x < self.width && y < self.height {
            self.cells[y as usize][x as usize].style = style;
        }
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        if x < self.width && y < self.height {
            self.cells[y as usize][x as usize].style.bg = Some(bg);
        }
    }

    fn fill(&mut self, rect: Rect, ch: char, style: Style) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let y_end = rect.y.saturating_add(rect.height).min(self.height);
        let x_end = rect.x.saturating_add(rect.width).min(self.width);
        for y in rect.y..y_end {
            for x in rect.x..x_end {
                self.cells[y as usize][x as usize] = TestCell {
                    ch,
                    style: style.clone(),
                };
            }
        }
    }

    fn clear(&mut self, rect: Rect) {
        self.fill(rect, ' ', Style::new());
    }

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}
