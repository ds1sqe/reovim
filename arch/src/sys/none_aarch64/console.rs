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
//! The surface is finite. When the log reaches the bottom the console scrolls
//! the retained content up one row — repainting from its cell grid, since the
//! framebuffer cannot be read back — rather than wrapping over the top, and a
//! reverse-video block ([`Console::show_cursor`]) marks the write head. CSI
//! cursor-addressing and erase finals stay out of scope: they are parsed and
//! dropped.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

use super::{
    color::Color,
    escape::{Action, Parser},
    fonts::Font,
    framebuffer::Framebuffer,
};

/// Blank pixel rows inserted below each text row so lines are not crammed —
/// it matters most for a full-cell bitmap font (Terminus), where glyphs touch
/// the cell edges; an anti-aliased font already carries some intrinsic leading
/// inside its cell. The blit paints only a cell's glyph rows, so these gap
/// rows keep the background the initial clear set — including after a scroll,
/// which repaints glyphs from the grid but never writes the gaps. Four pixels
/// suits the embedded 8x16 fonts; reviewed and kept at that.
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

/// Screen-buffer grid capacity, in glyph cells. The framebuffer runs a fixed
/// 1280x720 mode (`framebuffer.rs`), and the smallest cell across the embedded
/// fonts is 8 wide by `16 + LINE_LEADING` tall, so these are the worst-case
/// (most-cells) column and row counts. The retained-content grid
/// ([`ScreenGrid`]) is sized to this maximum: scroll repaints displaced lines
/// from the grid because the framebuffer is write-only (no readback), so the
/// grid must be able to back the whole surface.
const MAX_COLS: usize = 1280 / 8;
const MAX_ROWS: usize = 720 / (16 + LINE_LEADING as usize);
/// Total cells in the on-screen console's backing store.
const GRID_CELLS: usize = MAX_COLS * MAX_ROWS;

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

    /// Returns the set with `bits` flipped — used to invert a cell for the
    /// reverse-video block cursor, which stands out against whatever the cell
    /// already shows.
    const fn toggled(self, bits: u8) -> Self {
        Self(self.0 ^ bits)
    }

    /// Whether every bit in `bits` is set.
    const fn contains(self, bits: u8) -> bool {
        self.0 & bits == bits
    }
}

/// One retained screen cell: the glyph byte and the pen + attribute state it
/// was drawn with. The console stores a `Cell` per screen position so it can
/// repaint displaced lines after a scroll — the framebuffer is write-only, so
/// the displaced content is recovered from here, never read back from pixels.
#[derive(Clone, Copy)]
struct Cell {
    ch: u8,
    fg: Color,
    bg: Color,
    attrs: Attrs,
}

// The grid footprint is `GRID_CELLS * size_of::<Cell>()`; pin the cell size so
// a future field change is a visible build error, not a silent `.bss` blow-up.
const _: () = assert!(core::mem::size_of::<Cell>() == 20);

impl Cell {
    /// A blank cell: a space painted with the given pens and no attributes —
    /// the initial grid fill and the fill for a row blanked by a scroll.
    const fn blank(fg: Color, bg: Color) -> Self {
        Self {
            ch: b' ',
            fg,
            bg,
            attrs: Attrs::none(),
        }
    }
}

/// The default fill for the static backing before a console clears it to its
/// own pens. Black-on-black is irrelevant: [`Console::new`] overwrites every
/// in-use cell with `Cell::blank(fg, bg)` matching the cleared surface.
const RESET_CELL: Cell = Cell::blank(Color::Rgb(0), Color::Rgb(0));

/// An opaque backing store for a [`Console`]'s retained screen content.
///
/// The one on-screen console obtains its store from [`screen_grid`]; the cell
/// type it wraps is an implementation detail. Pass it to [`Console::new`].
pub struct ScreenGrid<'g>(&'g mut [Cell]);

/// `.bss`-resident backing for the on-screen console's grid, mirroring the
/// page arena: fixed-size storage reached only through the single `&mut` that
/// [`screen_grid`] hands out at most once.
struct GridStore(UnsafeCell<[Cell; GRID_CELLS]>);

// SAFETY: the storage is reached only through the single `&mut` returned by
// `screen_grid`, whose `AtomicBool` guard fires the hand-out at most once, so
// the reference is unique and no two owners alias.
unsafe impl Sync for GridStore {}

/// The on-screen console's grid storage.
static GRID: GridStore = GridStore(UnsafeCell::new([RESET_CELL; GRID_CELLS]));

/// Set once the backing has been handed out, so a second request yields `None`.
static GRID_TAKEN: AtomicBool = AtomicBool::new(false);

/// Hands out the on-screen console's screen-buffer backing — once.
///
/// The boot console (one per machine) takes it to obtain its retained-content
/// store; a second call returns `None` because the backing is already owned.
/// Selftests do not use this — they build a [`ScreenGrid`] over their own
/// small slice.
pub fn screen_grid() -> Option<ScreenGrid<'static>> {
    if GRID_TAKEN.swap(true, Ordering::Relaxed) {
        return None;
    }
    // SAFETY: the swap transitioned `GRID_TAKEN` from false to true exactly
    // once, so this is the only `&mut` ever produced from `GRID`; no aliasing.
    Some(ScreenGrid(unsafe { &mut *GRID.0.get() }))
}

/// A line-oriented text console over a [`Framebuffer`], rendering through a
/// selectable [`Font`].
///
/// The cursor tracks the next cell in glyph units. [`Console::print`] is the
/// only sink; it renders printable bytes, interprets `\n`/`\r`, and applies
/// the SGR color sequences the embedded [`escape parser`](super::escape)
/// decodes from the same byte stream.
pub struct Console<'g> {
    fb: Framebuffer,
    /// The font this console blits through.
    font: &'static Font,
    /// Cursor column in glyph cells.
    col: u32,
    /// Cursor row in glyph cells.
    row: u32,
    /// A wrap deferred until the next printable byte — the VT "last column"
    /// rule. Set when a glyph fills the last column, so a row filled exactly
    /// does not advance or scroll until there is more to print; cleared by the
    /// next print, a line feed, or a carriage return.
    wrap_pending: bool,
    /// Whether a block cursor is currently drawn at the write head. The cursor
    /// is a display overlay, never part of the grid; this tracks it so the next
    /// write can erase it (repaint the true cell) before drawing.
    cursor_shown: bool,
    /// Columns rendered, clamped so `cols * rows` fits [`grid`](Self::grid).
    cols: u32,
    /// Rows rendered, clamped so `cols * rows` fits [`grid`](Self::grid).
    rows: u32,
    /// Retained screen content, one [`Cell`] per position in row-major
    /// `cols`-stride order. The blit reads from here and scroll repaints from
    /// here — the write-only framebuffer is never read back.
    grid: &'g mut [Cell],
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

impl<'g> Console<'g> {
    /// Builds a console over `fb` rendering through `font` with the given
    /// foreground/background color pens, backed by `grid` for retained screen
    /// content, and clears the whole surface to the background. Both pens also
    /// become the defaults that SGR reset (`0`) and the default-color codes
    /// (`39`/`49`) restore.
    ///
    /// The column/row counts derive from the font's cell metrics (nonzero by
    /// construction — see [`Font`]'s const guards) and are then clamped so the
    /// whole grid (`cols * rows`) fits the supplied backing: a surface larger
    /// than the backing simply renders fewer rows, never out of bounds. The
    /// backing must hold at least one cell (the on-screen store from
    /// [`screen_grid`] holds the full surface; a selftest passes a slice sized
    /// to its stub surface).
    #[must_use]
    pub fn new(
        fb: Framebuffer,
        font: &'static Font,
        fg: Color,
        bg: Color,
        grid: ScreenGrid<'g>,
    ) -> Self {
        let ScreenGrid(grid) = grid;
        // The backing is at most `GRID_CELLS` (5760); a length beyond `u32`
        // is impossible, so saturating the conversion cannot lose a real value.
        let max_cells = u32::try_from(grid.len()).unwrap_or(u32::MAX);
        let cols = (fb.width() / font.cell_w).clamp(1, max_cells);
        let rows = (fb.height() / (font.cell_h + LINE_LEADING)).clamp(1, max_cells / cols);
        // Clear the surface and the in-use cells to the background pen, so the
        // retained grid agrees with the painted surface from the first byte.
        fb.clear(bg.resolve());
        let used = (cols * rows) as usize;
        for cell in &mut grid[..used] {
            *cell = Cell::blank(fg, bg);
        }
        Self {
            fb,
            font,
            col: 0,
            row: 0,
            wrap_pending: false,
            cursor_shown: false,
            cols,
            rows,
            grid,
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
    /// renders in color with no caller-side pen calls. A block cursor shown by
    /// [`show_cursor`](Self::show_cursor) is erased first, so glyphs land on
    /// the true cell and no stale block trails the head.
    pub fn print(&mut self, s: &str) {
        self.hide_cursor();
        for &byte in s.as_bytes() {
            // `advance` returns an owned, `Copy` action that borrows nothing,
            // so the parser borrow ends before `apply` takes `&mut self`.
            if let Some(action) = self.parser.advance(byte) {
                self.apply(action);
            }
        }
    }

    /// Draws a reverse-video block cursor at the write head. The block is the
    /// cell stored there with reverse toggled, so it inverts whatever the cell
    /// shows — a blank head becomes a solid block. It is a display overlay: the
    /// grid is untouched, so the next [`print`](Self::print) erases it (the
    /// erase-on-move) and repaints the true cell. Static — the floor has no
    /// timer to drive a blink.
    pub fn show_cursor(&mut self) {
        let mut cell = self.grid[self.idx(self.col, self.row)];
        cell.attrs = cell.attrs.toggled(Attrs::REVERSE);
        self.blit_cell(self.col, self.row, &cell);
        self.cursor_shown = true;
    }

    /// Erases a shown block cursor by repainting the true cell at the write
    /// head. The head has not moved since [`show_cursor`](Self::show_cursor)
    /// drew the block — only `print` moves it, and `print` calls this first —
    /// so the current `(col, row)` is exactly where the block is.
    fn hide_cursor(&mut self) {
        if self.cursor_shown {
            let cell = self.grid[self.idx(self.col, self.row)];
            self.blit_cell(self.col, self.row, &cell);
            self.cursor_shown = false;
        }
    }

    /// Applies one parser [`Action`]: render a cell, move the cursor, or move
    /// the pen.
    fn apply(&mut self, action: Action) {
        match action {
            Action::Print(byte) => {
                // A wrap deferred from the previous glyph materializes now, on
                // the next printable byte (the VT "last column" rule), so a row
                // filled exactly does not scroll until there is more to print.
                if self.wrap_pending {
                    self.newline();
                    self.wrap_pending = false;
                }
                self.put_cell(byte);
                if self.col + 1 < self.cols {
                    self.col += 1;
                } else {
                    self.wrap_pending = true;
                }
            }
            Action::LineFeed => {
                self.wrap_pending = false;
                self.newline();
            }
            Action::CarriageReturn => {
                self.wrap_pending = false;
                self.col = 0;
            }
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

    /// Moves the cursor to the start of the next row. On the last row the
    /// surface is full, so [`scroll_up`](Self::scroll_up) shifts the retained
    /// content up one row and the cursor stays on the now-cleared last row —
    /// output continues at the bottom instead of overwriting the top.
    fn newline(&mut self) {
        self.col = 0;
        if self.row + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.row += 1;
        }
    }

    /// Scrolls the retained content up one row: the top row is dropped, every
    /// other row shifts up one, the new bottom row is blanked to the current
    /// background pen, and the whole surface is repainted from the grid. The
    /// framebuffer is write-only, so the displaced pixels are recovered by
    /// re-blitting the grid — the source of truth — never read back.
    fn scroll_up(&mut self) {
        let cols = self.cols as usize;
        let used = cols * self.rows as usize;
        shift_rows_up(self.grid, cols, used, Cell::blank(self.fg, self.bg));
        self.repaint();
    }

    /// Re-blits every in-use cell from the grid. The grid is the source of
    /// truth (the framebuffer cannot be read back), so a repaint after a scroll
    /// reconstructs the surface exactly.
    fn repaint(&self) {
        for row in 0..self.rows {
            for col in 0..self.cols {
                let cell = self.grid[self.idx(col, row)];
                self.blit_cell(col, row, &cell);
            }
        }
    }

    /// Row-major index of cell `(col, row)` in the `cols`-stride grid. Every
    /// caller passes `col < cols` and `row < rows`, and [`new`](Self::new)
    /// clamps `cols * rows` to the backing length, so the index is in bounds.
    const fn idx(&self, col: u32, row: u32) -> usize {
        (row * self.cols + col) as usize
    }

    /// Snapshots the current pen + attribute state as the cell for `byte` at
    /// the cursor, stores it in the grid, and blits it. Storing the cell is
    /// what lets a later scroll repaint it without reading the framebuffer.
    fn put_cell(&mut self, byte: u8) {
        let cell = Cell {
            ch: byte,
            fg: self.fg,
            bg: self.bg,
            attrs: self.attrs,
        };
        let i = self.idx(self.col, self.row);
        self.grid[i] = cell;
        self.blit_cell(self.col, self.row, &cell);
    }

    /// Blits `cell` into screen position `(col, row)`, applying its text
    /// attributes. The pens resolve once and fold with the color-domain
    /// attributes (reverse swaps them, dim reduces the foreground toward the
    /// background); each pixel then blends the
    /// [`effective coverage`](effective_coverage) (after the glyph-domain
    /// bold/italic transforms) against the cell background. Every pixel is
    /// written (coverage `0` resolves to the background), so a reused cell is
    /// fully repainted, and underline overlays the finished glyph.
    fn blit_cell(&self, col: u32, row: u32, cell: &Cell) {
        let coverage = self.font.glyph(cell.ch);
        let cell_w = self.font.cell_w;
        let cell_h = self.font.cell_h;
        let x0 = col * cell_w;
        let y0 = row * (cell_h + LINE_LEADING);
        let (mut fg, mut bg) = (cell.fg.resolve(), cell.bg.resolve());
        if cell.attrs.contains(Attrs::REVERSE) {
            core::mem::swap(&mut fg, &mut bg);
        }
        if cell.attrs.contains(Attrs::DIM) {
            fg = blend(fg, bg, DIM_COVERAGE);
        }
        for gy in 0..cell_h {
            for gx in 0..cell_w {
                let cov = effective_coverage(coverage, cell_w, cell_h, cell.attrs, gx, gy);
                self.fb.put_pixel(x0 + gx, y0 + gy, blend(fg, bg, cov));
            }
        }
        // Underline overlays the finished glyph at full foreground.
        if cell.attrs.contains(Attrs::UNDERLINE) {
            let uy = y0 + cell_h.saturating_sub(UNDERLINE_INSET);
            for gx in 0..cell_w {
                self.fb.put_pixel(x0 + gx, uy, fg);
            }
        }
    }
}

/// Coverage byte for destination pixel `(gx, gy)` of a `cell_w` x `cell_h`
/// glyph after the glyph-domain attributes. Italic shears the sampled column
/// per row — the shear grows toward the top of the cell, so the glyph leans
/// right, and a destination column whose source falls outside the cell reads
/// as background (a synthetic italic clips at the cell edge). Bold then dilates
/// the sample horizontally by one pixel, widening stems. Neither moves the
/// resolved colors; they change only which coverage feeds the blend.
fn effective_coverage(
    coverage: &[u8],
    cell_w: u32,
    cell_h: u32,
    attrs: Attrs,
    gx: u32,
    gy: u32,
) -> u8 {
    let shear = if attrs.contains(Attrs::ITALIC) {
        (cell_h - 1 - gy) / ITALIC_RISE
    } else {
        0
    };
    // The destination column maps back to a source shifted left by the row's
    // shear; a column left of the cell has no source and clips to background.
    // The shear only ever shifts the source left, so the remaining source
    // columns stay within `0..cell_w` — the row-major index needs no
    // upper-bound guard.
    let Some(src) = gx.checked_sub(shear) else {
        return 0;
    };
    let at = |sx: u32| coverage[(gy * cell_w + sx) as usize];
    let mut cov = at(src);
    if attrs.contains(Attrs::BOLD) && src > 0 {
        cov = cov.max(at(src - 1));
    }
    cov
}

/// Shifts the in-use region of a cell grid up one row: row `r + 1` becomes row
/// `r`, and the freed bottom row (`used - cols .. used`) is filled with `blank`.
/// `cols <= used <= grid.len()` must hold — the console's [`new`](Console::new)
/// clamp guarantees it — so every index stays in bounds. Pure grid arithmetic
/// (no rendering), so it is unit-testable against a small slice.
fn shift_rows_up(grid: &mut [Cell], cols: usize, used: usize, blank: Cell) {
    // `copy_within` handles the overlapping shift; the source `cols..used` is
    // the rows below the top, copied down to start at row 0.
    grid.copy_within(cols..used, 0);
    for cell in &mut grid[used - cols..used] {
        *cell = blank;
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
