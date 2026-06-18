//! TrueType rasterization for the anti-aliased font (JetBrains Mono).
//!
//! `fontdue` produces a grayscale coverage bitmap per glyph; we place each one
//! into a fixed `cell_w` x `cell_h` cell on a shared baseline so the result is
//! a monospace grid the console can blit cell-by-cell. The pixel size is
//! chosen so the glyph advance fits the cell width and the ascent+descent fit
//! the cell height — JetBrains Mono is wider-aspect than an 8x16 cell, so the
//! height is the binding constraint.

use crate::Cell;
use fontdue::{Font, FontSettings};

/// Pixel size JetBrains Mono is rasterized at. Tuned so `ascent + |descent|`
/// (~1.32 em) fits the 16px cell height; the 0.6 em advance then fits the 8px
/// width with room to spare. Adjust here if the on-screen cell geometry changes.
const PX: f32 = 12.0;

/// Rasterizes `first..=last` of a TrueType font into row-major coverage cells.
pub fn rasterize(ttf: &[u8], cell_w: u32, cell_h: u32, first: u8, last: u8) -> Vec<Cell> {
    let font = Font::from_bytes(ttf, FontSettings::default()).expect("invalid TTF");
    let metrics = font
        .horizontal_line_metrics(PX)
        .expect("font has no horizontal line metrics");
    // Baseline row from the top of the cell. The block (ascent above, descent
    // below) is centered vertically within the cell so the leading is even.
    let block = metrics.ascent - metrics.descent; // descent is negative
    let top_pad = ((cell_h as f32 - block) / 2.0).round().max(0.0);
    let baseline_from_top = (top_pad + metrics.ascent).round() as i32;

    (first..=last)
        .map(|byte| {
            let (m, coverage) = font.rasterize(byte as char, PX);
            let mut cell = Cell::blank(cell_w, cell_h);
            if m.width == 0 || m.height == 0 {
                return cell; // space and other inkless glyphs
            }
            // Center the advance within the cell, then offset by the glyph's
            // own left bearing (`xmin`).
            let left_pad = ((cell_w as f32 - m.advance_width) / 2.0).round() as i32;
            let left = left_pad + m.xmin;
            // `ymin` is the baseline-to-bottom offset (negative below); the
            // bitmap's top row sits `ymin + height` pixels above the baseline.
            let top = baseline_from_top - (m.ymin + m.height as i32);
            for (i, &cov) in coverage.iter().enumerate() {
                if cov == 0 {
                    continue;
                }
                let x = left + (i % m.width) as i32;
                let y = top + (i / m.width) as i32;
                cell.set(x, y, cov);
            }
            cell
        })
        .collect()
}
