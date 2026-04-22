//! Bounded rendering surface that clips and offsets coordinates.
//!
//! `ScopedSurface` wraps any `&mut dyn ChromeSurface` with a `Rect` bounds.
//! All drawing operations are automatically offset to the bounds origin and
//! clipped to the bounds dimensions. Modules use this to render into their
//! allocated chrome region without manual coordinate arithmetic.
//!
//! # Example
//!
//! ```ignore
//! let mut scoped = ScopedSurface::new(&mut surface, bounds);
//! // (0, 0) in scoped == (bounds.x, bounds.y) in the inner surface
//! scoped.write_styled(0, 0, "hello", style);
//! ```

use crate::{ChromeSurface, Rect, Style, types::Color};

/// A rendering surface that clips and offsets all operations to a bounded region.
///
/// Created via `ScopedSurface::new()` (standalone constructor, not a trait method,
/// to preserve `ChromeSurface` object safety).
pub struct ScopedSurface<'a> {
    inner: &'a mut dyn ChromeSurface,
    bounds: Rect,
}

impl<'a> ScopedSurface<'a> {
    /// Create a scoped surface that renders within the given bounds.
    ///
    /// All coordinates passed to `ChromeSurface` methods are relative to the
    /// bounds origin (`bounds.x`, `bounds.y`). Operations outside the bounds
    /// are silently clipped.
    #[must_use]
    pub fn new(inner: &'a mut dyn ChromeSurface, bounds: Rect) -> Self {
        Self { inner, bounds }
    }
}

impl ChromeSurface for ScopedSurface<'_> {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        if y >= self.bounds.height || x >= self.bounds.width {
            return 0;
        }
        let abs_x = self.bounds.x.saturating_add(x);
        let abs_y = self.bounds.y.saturating_add(y);
        let max_cols = self.bounds.width.saturating_sub(x);
        // Truncate text to fit within bounds
        let truncated = truncate_to_cols(text, max_cols);
        if truncated.is_empty() {
            return 0;
        }
        self.inner.write_styled(abs_x, abs_y, truncated, style)
    }

    fn apply_style(&mut self, x: u16, y: u16, style: Style) {
        if y >= self.bounds.height || x >= self.bounds.width {
            return;
        }
        let abs_x = self.bounds.x.saturating_add(x);
        let abs_y = self.bounds.y.saturating_add(y);
        self.inner.apply_style(abs_x, abs_y, style);
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        if y >= self.bounds.height || x >= self.bounds.width {
            return;
        }
        let abs_x = self.bounds.x.saturating_add(x);
        let abs_y = self.bounds.y.saturating_add(y);
        self.inner.overlay_bg(abs_x, abs_y, bg);
    }

    fn fill(&mut self, rect: Rect, ch: char, style: Style) {
        if let Some(clipped) =
            self.bounds
                .intersect(&offset_rect(rect, self.bounds.x, self.bounds.y))
        {
            self.inner.fill(clipped, ch, style);
        }
    }

    fn clear(&mut self, rect: Rect) {
        if let Some(clipped) =
            self.bounds
                .intersect(&offset_rect(rect, self.bounds.x, self.bounds.y))
        {
            self.inner.clear(clipped);
        }
    }

    fn size(&self) -> (u16, u16) {
        (self.bounds.width, self.bounds.height)
    }
}

/// Offset a rect's origin by (dx, dy) using saturating addition.
const fn offset_rect(rect: Rect, dx: u16, dy: u16) -> Rect {
    Rect::new(rect.x.saturating_add(dx), rect.y.saturating_add(dy), rect.width, rect.height)
}

/// Truncate a string to fit within `max_cols` columns.
fn truncate_to_cols(text: &str, max_cols: u16) -> &str {
    use unicode_width::UnicodeWidthChar;

    let max = max_cols as usize;
    let mut cols = 0;
    let mut byte_end = 0;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if cols + w > max {
            break;
        }
        cols += w;
        byte_end += ch.len_utf8();
    }
    &text[..byte_end]
}

#[cfg(test)]
#[path = "scoped_surface_tests.rs"]
mod tests;
