//! `impl RenderSurface for CellCapability` + Style/Color/Attrs
//! translation layer (Plan 21 / 17-β.2b-impl-a).
//!
//! This module is the architectural unlock for 17-β.2b-impl-b: any
//! `&mut dyn RenderSurface` call-site (of which there are ~30 today
//! across chrome modules and driver helpers) accepts a
//! `CellCapability` without changing the call-site signature. Test
//! fixtures that today use `MockSurface` implementations can switch
//! to `CellCapability::new(w, h)` directly.
//!
//! # Out of scope
//!
//! - **Unicode width**: each `char` in the input string consumes
//!   exactly one cell. Wide characters (CJK, emoji) will render into
//!   a single cell. Higher layers (or a future `RichCellCapability`)
//!   handle width-aware rendering.
//! - **Fallible writes**: `CellCapability::write_cell` returns
//!   `Result<(), WriteCellError>` (Plan 17-α decision) while
//!   `RenderSurface::write_styled` is documented as silent-noop on
//!   OOB. The impl ignores `write_cell` errors (`let _ = …`) to
//!   preserve the `RenderSurface` contract.

use {
    reovim_client_driver::{
        Rect, Style,
        traits::RenderSurface,
        types::{Attributes, Color},
    },
    crate::{
        capability::{Cell, CellCapability},
        style::{CellAttrs, CellColor, CellStyle},
    },
};

/// Convert a driver-side [`Color`] into a cell-capability [`CellColor`].
///
/// 19-arm exhaustive match per Plan 21 §2.1 locked decision. Driver's
/// `Reset` maps to `CellColor::Default` (terminal-default rendering).
/// The 16 named ANSI colors map to `CellColor::Named(0..=15)` by their
/// ANSI index. `AnsiValue(n)` maps to `CellColor::Ansi256(n)`.
/// `Rgb { r, g, b }` maps to `CellColor::Rgb(r, g, b)`.
///
/// No `_ =>` fallthrough — adding a new `reovim_arch::Color` variant
/// must force a compile error on this match (good), not silent data
/// loss.
#[must_use]
const fn convert_color(c: Color) -> CellColor {
    match c {
        Color::Reset => CellColor::Default,
        Color::Black => CellColor::Named(0),
        Color::DarkRed => CellColor::Named(1),
        Color::DarkGreen => CellColor::Named(2),
        Color::DarkYellow => CellColor::Named(3),
        Color::DarkBlue => CellColor::Named(4),
        Color::DarkMagenta => CellColor::Named(5),
        Color::DarkCyan => CellColor::Named(6),
        Color::Grey => CellColor::Named(7),
        Color::DarkGrey => CellColor::Named(8),
        Color::Red => CellColor::Named(9),
        Color::Green => CellColor::Named(10),
        Color::Yellow => CellColor::Named(11),
        Color::Blue => CellColor::Named(12),
        Color::Magenta => CellColor::Named(13),
        Color::Cyan => CellColor::Named(14),
        Color::White => CellColor::Named(15),
        Color::AnsiValue(n) => CellColor::Ansi256(n),
        Color::Rgb { r, g, b } => CellColor::Rgb(r, g, b),
    }
}

/// Convert driver-side [`Attributes`] into cell-capability [`CellAttrs`].
///
/// Per-flag translation per Plan 21 §2.2 locked decision (raw-copy is
/// unsafe: `Attributes::STRIKETHROUGH` = `0b0000_1000` collides with
/// `CellAttrs::REVERSE` = `0b0000_1000`).
///
/// NOTE: `Attributes::STRIKETHROUGH` has no `CellAttrs` counterpart
/// and is silently dropped. A follow-on flight may add
/// `CellAttrs::STRIKETHROUGH` if the feature is needed.
#[must_use]
fn convert_attrs(a: Attributes) -> CellAttrs {
    let mut out = CellAttrs::empty();
    if a.contains(Attributes::BOLD) {
        out |= CellAttrs::BOLD;
    }
    if a.contains(Attributes::ITALIC) {
        out |= CellAttrs::ITALIC;
    }
    if a.contains(Attributes::UNDERLINE) {
        out |= CellAttrs::UNDERLINE;
    }
    if a.contains(Attributes::REVERSE) {
        out |= CellAttrs::REVERSE;
    }
    if a.contains(Attributes::DIM) {
        out |= CellAttrs::DIM;
    }
    // Attributes::STRIKETHROUGH has no CellAttrs counterpart — dropped.
    out
}

/// Convert a driver-side [`Style`] into a cell-capability [`CellStyle`].
///
/// Composes [`convert_color`] + [`convert_attrs`]. `Option<Color>` →
/// `Option<CellColor>` preserves the "inherit from prior cell" nuance
/// for unspecified fg/bg.
#[must_use]
fn convert_style(s: &Style) -> CellStyle {
    CellStyle {
        fg: s.fg.map(convert_color),
        bg: s.bg.map(convert_color),
        attrs: convert_attrs(s.attributes),
    }
}

impl RenderSurface for CellCapability {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        // §2.4 locked: one char = one cell, no Unicode width awareness.
        let cell_style = convert_style(&style);
        let mut written: u16 = 0;
        for (i, ch) in text.chars().enumerate() {
            let col = x.saturating_add(u16::try_from(i).unwrap_or(u16::MAX));
            if col >= self.width() {
                break;
            }
            // §2.3 locked: OOB writes silently noop.
            let _ = self.write_cell(col, y, Cell::new(ch, cell_style));
            written = written.saturating_add(1);
        }
        written
    }

    fn apply_style(&mut self, x: u16, y: u16, style: Style) {
        // Read the existing cell, preserve `ch`, swap the style. OOB
        // → noop per §2.3.
        let Some(existing) = self.get_cell(x, y).cloned() else {
            return;
        };
        let new_cell = Cell::new(existing.ch, convert_style(&style));
        let _ = self.write_cell(x, y, new_cell);
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        // Preserve ch + fg + attrs; swap only the bg.
        let Some(existing) = self.get_cell(x, y).cloned() else {
            return;
        };
        let mut new_style = existing.style;
        new_style.bg = Some(convert_color(bg));
        let _ = self.write_cell(x, y, Cell::new(existing.ch, new_style));
    }

    fn fill(&mut self, rect: Rect, ch: char, style: Style) {
        let cell_style = convert_style(&style);
        let filler = Cell::new(ch, cell_style);
        // Iterate in bounds; OOB cells beyond self.size() silently
        // noop via write_cell's bounds check.
        for dy in 0..rect.height {
            for dx in 0..rect.width {
                let px = rect.x.saturating_add(dx);
                let py = rect.y.saturating_add(dy);
                let _ = self.write_cell(px, py, filler.clone());
            }
        }
    }

    fn clear(&mut self, rect: Rect) {
        // Delegate to the RenderSurface::fill method defined on this
        // same impl block (not CellCapability::fill which fills the
        // entire grid with no rect parameter).
        <Self as RenderSurface>::fill(self, rect, ' ', Style::default());
    }

    fn size(&self) -> (u16, u16) {
        (self.width(), self.height())
    }
}

#[cfg(test)]
#[path = "render_surface_impl_tests.rs"]
mod tests;
