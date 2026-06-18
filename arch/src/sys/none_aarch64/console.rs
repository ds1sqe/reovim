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
//! Color and text effects are driven the way a terminal is:
//! [`Console::print`] feeds the byte stream through a sans-IO
//! [`escape parser`](super::escape), and the SGR sequences it decodes move the
//! color pen and the text attributes ([`Console::apply_sgr`]). The console is
//! the *policy* half of that split — it maps SGR codes to [`Color`] pens and
//! to the rendered attributes (bold, dim, italic, underline, reverse) — while
//! the parser is the IO-free *mechanism*. Reverse and dim recolor the cell,
//! underline overlays a row, and bold/italic are synthetic coverage transforms
//! (no separate bold/italic font tables).
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

/// Foreground coverage for the dim attribute (SGR 2): the foreground is
/// composited over the background at half strength — the conventional
/// "reduced-intensity" reading of dim. With bold also set, the widened glyph is
/// still drawn at this reduced intensity (bold changes shape, dim changes
/// intensity; they compose).
const DIM_COVERAGE: u8 = 128;

/// Pixels above the cell's bottom edge where the underline row is drawn (SGR
/// 4). Two keeps the rule inside the glyph rows, clear of the descenders and of
/// the [`LINE_LEADING`] gap below.
const UNDERLINE_INSET: u32 = 2;

/// Rows of cell height per one pixel of synthetic-italic shear (SGR 3). The
/// slant shifts the top of the glyph right by `(cell_h - 1) / ITALIC_RISE`
/// pixels, tapering to zero at the baseline — a gentle oblique that stays
/// within the cell for the embedded 8x16 fonts (max two pixels). A nonzero
/// constant divisor keeps the shear total with no divide-by-zero.
const ITALIC_RISE: u32 = 6;

/// The set of SGR text attributes in effect, as a bitset. A `u8` rather than
/// five `bool` fields keeps the value `Copy` and `const`-constructible and is
/// the natural shape for an attribute *set* — it also stays clear of
/// `clippy::struct_excessive_bools`.
#[derive(Clone, Copy)]
struct Attrs(u8);

impl Attrs {
    const BOLD: u8 = 1 << 0;
    const DIM: u8 = 1 << 1;
    const ITALIC: u8 = 1 << 2;
    const UNDERLINE: u8 = 1 << 3;
    const REVERSE: u8 = 1 << 4;

    /// No attributes — a freshly reset cell.
    const fn none() -> Self {
        Self(0)
    }

    /// Returns the set with `bits` added.
    const fn with(self, bits: u8) -> Self {
        Self(self.0 | bits)
    }

    /// Returns the set with `bits` removed.
    const fn without(self, bits: u8) -> Self {
        Self(self.0 & !bits)
    }

    /// Whether every bit in `bits` is set.
    const fn contains(self, bits: u8) -> bool {
        self.0 & bits == bits
    }
}

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
    /// Text attributes (bold/dim/italic/underline/reverse) in effect, applied
    /// per cell at draw time. Cleared by SGR reset (`0`).
    attrs: Attrs,
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
            attrs: Attrs::none(),
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

    /// Applies one SGR sequence's parameters to the pen and attributes. Color
    /// codes move the pen — the 16 named colors (`30`–`37`/`90`–`97` fg,
    /// `40`–`47`/`100`–`107` bg), the extended forms (`38`/`48` with `5;n`
    /// indexed or `2;r;g;b` truecolor), and the defaults (`39`/`49`). Attribute
    /// codes set or clear the text effects: bold (`1`/`22`), dim (`2`/`22`),
    /// italic (`3`/`23`), underline (`4`/`24`), reverse (`7`/`27`); `22` clears
    /// bold and dim together (normal intensity). Reset (`0`, or no parameters)
    /// restores both pens and clears every attribute. Codes outside this set
    /// are not in the console's scope and are ignored; malformed extended
    /// color sequences leave the pen unchanged.
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
                1 => self.attrs = self.attrs.with(Attrs::BOLD),
                2 => self.attrs = self.attrs.with(Attrs::DIM),
                3 => self.attrs = self.attrs.with(Attrs::ITALIC),
                4 => self.attrs = self.attrs.with(Attrs::UNDERLINE),
                7 => self.attrs = self.attrs.with(Attrs::REVERSE),
                22 => self.attrs = self.attrs.without(Attrs::BOLD | Attrs::DIM),
                23 => self.attrs = self.attrs.without(Attrs::ITALIC),
                24 => self.attrs = self.attrs.without(Attrs::UNDERLINE),
                27 => self.attrs = self.attrs.without(Attrs::REVERSE),
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

    /// Restores both pens to the console's defaults and clears every text
    /// attribute — the terminal `reset` / `SGR 0` behavior.
    const fn reset(&mut self) {
        self.fg = self.default_fg;
        self.bg = self.default_bg;
        self.attrs = Attrs::none();
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

    /// Blits the glyph for `byte` into the current cell, applying the active
    /// text attributes. The pens are resolved once and folded with the
    /// color-domain attributes (reverse swaps them, dim reduces the foreground
    /// toward the background); each pixel then blends the
    /// [`effective coverage`](Console::effective_coverage) (after the
    /// glyph-domain bold/italic transforms) against the cell background. Every
    /// pixel is written (coverage `0` resolves to the background), so a reused
    /// cell is fully repainted, and underline overlays the finished glyph.
    fn draw_cell(&self, byte: u8) {
        let coverage = self.font.glyph(byte);
        let cell_w = self.font.cell_w;
        let cell_h = self.font.cell_h;
        let x0 = self.col * cell_w;
        let y0 = self.row * (cell_h + LINE_LEADING);
        // Resolve the pens once, then fold in the color-domain attributes:
        // reverse swaps them; dim reduces the post-reverse foreground.
        let (mut fg, mut bg) = (self.fg.resolve(), self.bg.resolve());
        if self.attrs.contains(Attrs::REVERSE) {
            core::mem::swap(&mut fg, &mut bg);
        }
        if self.attrs.contains(Attrs::DIM) {
            fg = blend(fg, bg, DIM_COVERAGE);
        }
        for gy in 0..cell_h {
            for gx in 0..cell_w {
                let cov = self.effective_coverage(coverage, gx, gy);
                self.fb.put_pixel(x0 + gx, y0 + gy, blend(fg, bg, cov));
            }
        }
        // Underline overlays the finished glyph at full foreground.
        if self.attrs.contains(Attrs::UNDERLINE) {
            let uy = y0 + cell_h.saturating_sub(UNDERLINE_INSET);
            for gx in 0..cell_w {
                self.fb.put_pixel(x0 + gx, uy, fg);
            }
        }
    }

    /// Coverage byte for destination pixel `(gx, gy)` after the glyph-domain
    /// attributes. Italic shears the sampled column per row — the shear grows
    /// toward the top of the cell, so the glyph leans right, and a destination
    /// column whose source falls outside the cell reads as background (a
    /// synthetic italic clips at the cell edge). Bold then dilates the sample
    /// horizontally by one pixel, widening stems. Neither moves the resolved
    /// colors; they change only which coverage feeds the blend.
    fn effective_coverage(&self, coverage: &[u8], gx: u32, gy: u32) -> u8 {
        let cell_w = self.font.cell_w;
        let cell_h = self.font.cell_h;
        let shear = if self.attrs.contains(Attrs::ITALIC) {
            (cell_h - 1 - gy) / ITALIC_RISE
        } else {
            0
        };
        // The destination column maps back to a source shifted left by the
        // row's shear; a column left of the cell has no source and clips to
        // background. The shear only ever shifts the source left, so the
        // remaining source columns stay within `0..cell_w` — the row-major
        // index below needs no upper-bound guard.
        let Some(src) = gx.checked_sub(shear) else {
            return 0;
        };
        let at = |sx: u32| coverage[(gy * cell_w + sx) as usize];
        let mut cov = at(src);
        if self.attrs.contains(Attrs::BOLD) && src > 0 {
            cov = cov.max(at(src - 1));
        }
        cov
    }
}

/// Decodes an extended-color SGR introducer's trailing parameters, where
/// `params[*i]` is the `38`/`48` introducer. Returns the color, or `None` when
/// the mode is unknown or its values are missing or out of range — in which
/// case the caller leaves the pen unchanged.
///
/// Crucially, `*i` advances past every parameter a recognized `5`/`2` mode
/// claims, *even when the color is malformed*. Otherwise a truncated triple
/// like `38;2;1;2` would leave its `1`/`2` to be reinterpreted by the SGR loop
/// as independent attribute codes (bold, dim) rather than the discarded color
/// channels they are.
fn extended_color(params: &[u16], i: &mut usize) -> Option<Color> {
    // Consume the next parameter, advancing the cursor past it. Returning `None`
    // (no further parameter) leaves the cursor put, so nothing is over-consumed.
    let mut take = || {
        let v = *params.get(*i + 1)?;
        *i += 1;
        Some(v)
    };
    match take()? {
        5 => {
            // `38;5;n` — indexed. Consume `n`; a value outside a byte is not a
            // palette entry.
            u8::try_from(take()?).ok().map(Color::Indexed)
        }
        2 => {
            // `38;2;r;g;b` — truecolor. Consume the three channels; each is
            // taken modulo 256 (its low byte), the conventional reading of an
            // over-range channel.
            let r = u32::from(take()?) & 0xFF;
            let g = u32::from(take()?) & 0xFF;
            let b = u32::from(take()?) & 0xFF;
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
