//! Coverage-blended text console over the `VideoCore` framebuffer.
//!
//! [`super::framebuffer::Framebuffer`] exposes only raw pixel stores. This
//! module turns that surface into a line-oriented text console so the
//! bare-metal boot log is legible on the HDMI output, not only on the UART.
//! It owns a cursor (column/row in glyph cells), renders each printable byte
//! from a selectable [`Font`], and advances/wraps as bytes arrive.
//!
//! Rendering is grayscale-coverage software alpha-blending. Each glyph cell is
//! a coverage map (`0x00` background .. `0xFF` foreground); every pixel is
//! composited as `fg·cov + bg·(255−cov)` against the console's known solid
//! background, because the framebuffer is write-only (no readback). A bitmap
//! font (Terminus) is just the `0x00`/`0xFF` special case of the same path, so
//! one blit serves both the anti-aliased and the crisp font — selecting a font
//! is passing a different [`Font`] descriptor.
//!
//! Scope is the splash floor: one fixed foreground/background pair, no
//! scrollback (the boot log fits the surface), and no escape-sequence parsing
//! — only `\n`/`\r` are interpreted.

use super::{fonts::Font, framebuffer::Framebuffer};

/// Blank pixel rows inserted below each text row so lines are not crammed —
/// it matters most for a full-cell bitmap font (Terminus), where glyphs touch
/// the cell edges; an anti-aliased font already carries some intrinsic leading
/// inside its cell. Scrollback (flight 05) will repaint these gaps; the splash
/// log fits the surface without wrapping, so the initial clear suffices here.
const LINE_LEADING: u32 = 4;

/// A line-oriented text console over a [`Framebuffer`], rendering through a
/// selectable [`Font`].
///
/// The cursor tracks the next cell in glyph units. [`Console::print`] is the
/// only sink; it renders printable bytes and interprets `\n`/`\r`.
pub struct Console {
    fb: Framebuffer,
    /// The font this console blits through.
    font: &'static Font,
    /// Cursor column in glyph cells.
    col: u32,
    /// Cursor row in glyph cells.
    row: u32,
    /// Columns that fit the surface (`fb.width() / font.cell_w`).
    cols: u32,
    /// Rows that fit the surface (`fb.height() / (font.cell_h + LINE_LEADING)`).
    rows: u32,
    /// Foreground (glyph) pixel, `0x00RRGGBB`.
    fg: u32,
    /// Background (cell) pixel, `0x00RRGGBB`.
    bg: u32,
}

impl Console {
    /// Builds a console over `fb` rendering through `font` with the given
    /// foreground/background colors, and clears the whole surface to the
    /// background. The cell grid derives from the font's cell metrics, which
    /// are nonzero by construction (see [`Font`]'s const guards), so the
    /// column/row division cannot divide by zero.
    #[must_use]
    pub fn new(fb: Framebuffer, font: &'static Font, fg: u32, bg: u32) -> Self {
        let cols = fb.width() / font.cell_w;
        let rows = fb.height() / (font.cell_h + LINE_LEADING);
        fb.clear(bg);
        Self {
            fb,
            font,
            col: 0,
            row: 0,
            cols,
            rows,
            fg,
            bg,
        }
    }

    /// Renders `s` at the cursor, advancing one cell per byte. `\n` moves to
    /// the start of the next row, `\r` returns to column 0, and any other byte
    /// renders a cell (printable bytes as their glyph, everything else blank).
    pub fn print(&mut self, s: &str) {
        for &byte in s.as_bytes() {
            self.put_byte(byte);
        }
    }

    /// Renders one byte and advances the cursor, wrapping at the row edge.
    fn put_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => self.col = 0,
            _ => {
                self.draw_cell(byte);
                self.col += 1;
                if self.col >= self.cols {
                    self.newline();
                }
            }
        }
    }

    /// Moves the cursor to the start of the next row, wrapping to the top when
    /// the surface is full (the splash log fits, so no scrollback is needed).
    const fn newline(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= self.rows {
            self.row = 0;
        }
    }

    /// Blits the glyph for `byte` into the current cell, alpha-blending each
    /// coverage value against the cell background. Every pixel is written
    /// (coverage `0` resolves to the background), so a reused cell is fully
    /// repainted.
    fn draw_cell(&self, byte: u8) {
        let coverage = self.font.glyph(byte);
        let cell_w = self.font.cell_w;
        let cell_h = self.font.cell_h;
        let x0 = self.col * cell_w;
        let y0 = self.row * (cell_h + LINE_LEADING);
        for gy in 0..cell_h {
            for gx in 0..cell_w {
                let cov = coverage[(gy * cell_w + gx) as usize];
                let color = blend(self.fg, self.bg, cov);
                self.fb.put_pixel(x0 + gx, y0 + gy, color);
            }
        }
    }
}

/// Alpha-composites one `0x00RRGGBB` channel:
/// `(fg·cov + bg·(255−cov) + 127) / 255` — the coverage blend, rounded to
/// nearest. `cov = 0` yields `bg`, `cov = 255` yields `fg`.
const fn blend_channel(fg: u32, bg: u32, cov: u32) -> u32 {
    (fg * cov + bg * (255 - cov) + 127) / 255
}

/// Blends foreground over background by `coverage` (`0` = none, `255` = full)
/// per channel. No framebuffer readback: `bg` is the console's known solid
/// cell background.
const fn blend(fg: u32, bg: u32, coverage: u8) -> u32 {
    let cov = coverage as u32;
    let r = blend_channel((fg >> 16) & 0xFF, (bg >> 16) & 0xFF, cov);
    let g = blend_channel((fg >> 8) & 0xFF, (bg >> 8) & 0xFF, cov);
    let b = blend_channel(fg & 0xFF, bg & 0xFF, cov);
    (r << 16) | (g << 8) | b
}

#[cfg(feature = "selftest")]
#[path = "console_tests.rs"]
mod console_tests;
