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
//! composited as `fg·cov + bg·(255−cov)` against the console's current solid
//! background, because the framebuffer is write-only (no readback). A bitmap
//! font (Terminus) is just the `0x00`/`0xFF` special case of the same path, so
//! one blit serves both the anti-aliased and the crisp font — selecting a font
//! is passing a different [`Font`] descriptor.
//!
//! Color is driven the way a terminal is: [`Console::print`] feeds the byte
//! stream through a sans-IO [`escape parser`](super::escape), and the SGR
//! sequences it decodes move the color pen ([`Console::apply_sgr`]). The
//! console is the *policy* half of that split — it maps SGR codes to
//! [`Color`] pens — while the parser is the IO-free *mechanism*. Only color
//! SGR is acted on; non-color attributes (bold, underline, …) are recognized
//! by the parser and ignored here until effect rendering is added.
//!
//! Scope is the splash floor: no scrollback (the boot log fits the surface)
//! and no cursor addressing — CSI cursor/erase finals are parsed and dropped.

use super::{
    color::Color,
    escape::{Action, Parser},
    fonts::Font,
    framebuffer::Framebuffer,
};

/// Blank pixel rows inserted below each text row so lines are not crammed —
/// it matters most for a full-cell bitmap font (Terminus), where glyphs touch
/// the cell edges; an anti-aliased font already carries some intrinsic leading
/// inside its cell. Scrollback will repaint these gaps; the splash
/// log fits the surface without wrapping, so the initial clear suffices here.
const LINE_LEADING: u32 = 4;

/// A line-oriented text console over a [`Framebuffer`], rendering through a
/// selectable [`Font`].
///
/// The cursor tracks the next cell in glyph units. [`Console::print`] is the
/// only sink; it renders printable bytes, interprets `\n`/`\r`, and applies
/// the SGR color sequences the embedded [`escape parser`](super::escape)
/// decodes from the same byte stream.
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
    /// Current foreground (glyph) pen, resolved per cell at draw time.
    fg: Color,
    /// Current background (cell) pen, resolved per cell at draw time.
    bg: Color,
    /// Foreground restored by SGR reset / default (`SGR 0` / `39`).
    default_fg: Color,
    /// Background restored by SGR reset / default (`SGR 0` / `49`).
    default_bg: Color,
    /// Decodes escape sequences out of the byte stream fed to [`print`].
    parser: Parser,
}

impl Console {
    /// Builds a console over `fb` rendering through `font` with the given
    /// foreground/background color pens, and clears the whole surface to the
    /// background. Both pens also become the defaults that SGR reset (`0`) and
    /// the default-color codes (`39`/`49`) restore. The cell grid derives from
    /// the font's cell metrics, which are nonzero by construction (see
    /// [`Font`]'s const guards), so the column/row division cannot divide by
    /// zero.
    #[must_use]
    pub fn new(fb: Framebuffer, font: &'static Font, fg: Color, bg: Color) -> Self {
        let cols = fb.width() / font.cell_w;
        let rows = fb.height() / (font.cell_h + LINE_LEADING);
        fb.clear(bg.resolve());
        Self {
            fb,
            font,
            col: 0,
            row: 0,
            cols,
            rows,
            fg,
            bg,
            default_fg: fg,
            default_bg: bg,
            parser: Parser::new(),
        }
    }

    /// Renders `s` at the cursor, feeding every byte through the escape parser.
    /// Printable bytes advance one cell, `\n`/`\r` move the cursor, and a
    /// complete `ESC [ … m` moves the color pen — so an ANSI-colored stream
    /// renders in color with no caller-side pen calls.
    pub fn print(&mut self, s: &str) {
        for &byte in s.as_bytes() {
            // `advance` returns an owned, `Copy` action that borrows nothing,
            // so the parser borrow ends before `apply` takes `&mut self`.
            if let Some(action) = self.parser.advance(byte) {
                self.apply(action);
            }
        }
    }

    /// Applies one parser [`Action`]: render a cell, move the cursor, or move
    /// the pen.
    fn apply(&mut self, action: Action) {
        match action {
            Action::Print(byte) => {
                self.draw_cell(byte);
                self.col += 1;
                if self.col >= self.cols {
                    self.newline();
                }
            }
            Action::LineFeed => self.newline(),
            Action::CarriageReturn => self.col = 0,
            Action::Sgr(params) => self.apply_sgr(params.as_slice()),
        }
    }

    /// Moves the color pen per one SGR sequence's parameters. Handles the
    /// color codes — the 16 named colors (`30`–`37`/`90`–`97` fg,
    /// `40`–`47`/`100`–`107` bg), the extended forms (`38`/`48` with `5;n`
    /// indexed or `2;r;g;b` truecolor), the defaults (`39`/`49`), and reset
    /// (`0`, or no parameters). Any other code is a non-color attribute the
    /// console does not render yet and is ignored. Malformed extended
    /// sequences leave the pen unchanged.
    // The `as u8` casts below are SGR named-color offsets (`code - 30`, etc.),
    // each provably in `0..=15`, so the narrowing cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    fn apply_sgr(&mut self, params: &[u16]) {
        if params.is_empty() {
            self.reset();
            return;
        }
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.reset(),
                30..=37 => self.set_fg(Color::Indexed((params[i] - 30) as u8)),
                90..=97 => self.set_fg(Color::Indexed((params[i] - 90 + 8) as u8)),
                40..=47 => self.set_bg(Color::Indexed((params[i] - 40) as u8)),
                100..=107 => self.set_bg(Color::Indexed((params[i] - 100 + 8) as u8)),
                38 => {
                    if let Some(color) = extended_color(params, &mut i) {
                        self.set_fg(color);
                    }
                }
                48 => {
                    if let Some(color) = extended_color(params, &mut i) {
                        self.set_bg(color);
                    }
                }
                39 => self.fg = self.default_fg,
                49 => self.bg = self.default_bg,
                _ => {}
            }
            i += 1;
        }
    }

    /// Sets the foreground pen for subsequent cells. Private: the pen is moved
    /// by SGR sequences through [`apply_sgr`](Console::apply_sgr), not by
    /// callers.
    const fn set_fg(&mut self, fg: Color) {
        self.fg = fg;
    }

    /// Sets the background pen for subsequent cells. Private, as [`set_fg`].
    const fn set_bg(&mut self, bg: Color) {
        self.bg = bg;
    }

    /// Restores both pens to the console's defaults — the terminal `reset` /
    /// `SGR 0` behavior.
    const fn reset(&mut self) {
        self.fg = self.default_fg;
        self.bg = self.default_bg;
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
        // Resolve the pen to concrete pixels once per cell, not per pixel.
        let fg = self.fg.resolve();
        let bg = self.bg.resolve();
        for gy in 0..cell_h {
            for gx in 0..cell_w {
                let cov = coverage[(gy * cell_w + gx) as usize];
                let color = blend(fg, bg, cov);
                self.fb.put_pixel(x0 + gx, y0 + gy, color);
            }
        }
    }
}

/// Decodes an extended-color SGR introducer's trailing parameters, where
/// `params[*i]` is the `38`/`48` introducer. Advances `*i` past the parameters
/// it consumes and returns the color, or `None` when they are missing or
/// malformed (an out-of-range indexed value, or a too-short truecolor triple)
/// — in which case the caller leaves the pen unchanged. The `*i` advance still
/// happens for a recognized `5`/`2` mode, so the parameter cursor stays in
/// step even when the value itself is rejected.
fn extended_color(params: &[u16], i: &mut usize) -> Option<Color> {
    match *params.get(*i + 1)? {
        5 => {
            // `38;5;n` — indexed. `n` outside a byte is not a palette entry.
            let n = *params.get(*i + 2)?;
            *i += 2;
            u8::try_from(n).ok().map(Color::Indexed)
        }
        2 => {
            // `38;2;r;g;b` — truecolor. Each channel is taken modulo 256 (its
            // low byte), the conventional reading of an over-range channel.
            let r = u32::from(*params.get(*i + 2)?) & 0xFF;
            let g = u32::from(*params.get(*i + 3)?) & 0xFF;
            let b = u32::from(*params.get(*i + 4)?) & 0xFF;
            *i += 4;
            Some(Color::Rgb((r << 16) | (g << 8) | b))
        }
        _ => None,
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
