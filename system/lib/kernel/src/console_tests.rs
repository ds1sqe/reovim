//! Tests for the coverage blit and color blend, compiled into the lib under
//! `selftest` and run by the `arch-selftest` no_std runner.
//!
//! The blend math is asserted directly at named `(fg, bg, coverage)` triples.
//! The blit is exercised by rendering into a caller-owned memory region through
//! a callback-backed [`RenderSurface`] the test reads back, so `draw_cell` runs
//! end-to-end for both embedded fonts without an MMIO framebuffer.

use {
    super::{Cell, Console, RESET_CELL, RenderSurface, ScreenGrid, blend, shift_rows_up},
    crate::{
        color::Color,
        fonts::{Font, JETBRAINS_MONO, TERMINUS},
    },
    reovim_testrt::{self as testrt, arch_test},
};

/// Opaque-white foreground over a distinct dark background, so a rendered
/// pixel is unambiguously distinguishable from the cleared cell.
const FG: u32 = 0x00FF_FFFF;
const BG: u32 = 0x0010_2030;

/// One 8x16 cell worth of `u32` pixels — the surface every blit test renders
/// into. Matches both fonts' cell geometry.
const CELL_PIXELS: usize = 8 * 16;

/// Resolved palette pixels the SGR tests assert against: `Indexed(1)`/`SGR 31`
/// normal red, `Indexed(2)`/`SGR 42` normal green, and `Indexed(9)`/`SGR 91`
/// bright red (also cube index 196, via `SGR 38;5;196`).
const PALETTE_RED: u32 = 0x0080_0000;
const PALETTE_GREEN: u32 = 0x0000_8000;
const BRIGHT_RED: u32 = 0x00FF_0000;

struct StubSurface {
    base: *mut u32,
    width: u32,
    height: u32,
    pitch: u32,
}

impl StubSurface {
    const fn new(base: *mut u32, width: u32, height: u32, pitch: u32) -> Self {
        Self {
            base,
            width,
            height,
            pitch,
        }
    }

    fn render_surface(&mut self) -> RenderSurface {
        RenderSurface::new(
            self.width,
            self.height,
            (self as *mut Self).cast::<()>() as usize,
            stub_put_pixel,
            stub_clear,
        )
    }
}

fn stub_put_pixel(ctx: usize, x: u32, y: u32, color: u32) {
    // SAFETY: every test creates the `StubSurface` on the same stack frame as
    // the console that uses it; coordinates are bounds-checked before the write.
    let surface = unsafe { &*(ctx as *const StubSurface) };
    if x >= surface.width || y >= surface.height {
        return;
    }
    let offset = (y as usize) * (surface.pitch as usize / 4) + x as usize;
    // SAFETY: the caller supplied at least `pitch * height` bytes of writable
    // backing storage; the bounds check above keeps the write in that region.
    unsafe {
        surface.base.add(offset).write(color);
    }
}

fn stub_clear(ctx: usize, color: u32) {
    // SAFETY: same context lifetime as `stub_put_pixel`.
    let surface = unsafe { &*(ctx as *const StubSurface) };
    for y in 0..surface.height {
        for x in 0..surface.width {
            stub_put_pixel(ctx, x, y, color);
        }
    }
}

/// Renders `s` into a fresh one-cell stub surface through `font` and returns
/// the backing pixels (row-major, `8 * 16`).
fn render(font: &'static Font, s: &str) -> [u32; CELL_PIXELS] {
    let mut buf = [0u32; CELL_PIXELS];
    let mut surface = StubSurface::new(buf.as_mut_ptr(), 8, 16, 8 * 4);
    // One-cell backing for the one-cell stub surface.
    let mut grid = [RESET_CELL; 1];
    let mut console = Console::new(
        surface.render_surface(),
        font,
        Color::Rgb(FG),
        Color::Rgb(BG),
        ScreenGrid(&mut grid),
    );
    console.print(s);
    buf
}

arch_test!(console_blend_named_triples, {
    // Endpoints: zero coverage is pure background, full coverage is pure
    // foreground (the `+127` rounding still lands exactly on each endpoint).
    testrt::check_eq(blend(0x00FF_FFFF, 0x0000_0000, 0), 0x0000_0000u32);
    testrt::check_eq(blend(0x00FF_FFFF, 0x0000_0000, 255), 0x00FF_FFFFu32);
    // Mid coverage exercises the rounding boundary in both directions:
    // white-over-black at cov=128 rounds up to 0x80; black-over-white at
    // cov=128 rounds to 0x7F.
    testrt::check_eq(blend(0x00FF_FFFF, 0x0000_0000, 128), 0x0080_8080u32);
    testrt::check_eq(blend(0x0000_0000, 0x00FF_FFFF, 128), 0x007F_7F7Fu32);
});

arch_test!(console_blit_renders_blank_and_ink, {
    for font in [&JETBRAINS_MONO, &TERMINUS] {
        // A space glyph is all-background: the whole cell stays `BG`.
        let space = render(font, " ");
        testrt::check(space.iter().all(|&p| p == BG), "space cell is all background");
        // A letter renders ink: at least one pixel differs from the background.
        let letter = render(font, "M");
        testrt::check(letter.iter().any(|&p| p != BG), "letter M paints some ink");
    }
});

arch_test!(console_blit_out_of_range_is_blank, {
    // A byte outside the font range resolves to the blank cell, so the surface
    // stays all background — no panic, no out-of-bounds, no stray ink.
    for font in [&JETBRAINS_MONO, &TERMINUS] {
        let cell = render(font, "\u{01}");
        testrt::check(cell.iter().all(|&p| p == BG), "out-of-range byte renders blank");
    }
});

// The SGR tests below drive the console the way a terminal does — escape
// sequences embedded in the printed stream, not imperative pen calls — and
// blit through Terminus deliberately: its coverage is pure `0x00`/`0xFF`, so a
// full-coverage ink pixel resolves to *exactly* the pen's color and a
// background pixel to *exactly* the bg pen, with no anti-aliased edge values.

arch_test!(console_sgr_indexed_fg_blits_palette, {
    // Both the named form (`31`) and the 256-index form (`38;5;196`) resolve
    // through the palette to the blitted ink.
    let named = render(&TERMINUS, "\x1b[31mM");
    testrt::check(named.iter().any(|&p| p == PALETTE_RED), "SGR 31 paints palette red");
    let indexed = render(&TERMINUS, "\x1b[38;5;196mM");
    testrt::check(indexed.iter().any(|&p| p == BRIGHT_RED), "SGR 38;5;196 paints cube red");
});

arch_test!(console_sgr_bright_fg_blits_palette, {
    // The bright range (`90`–`97`) maps to palette indices 8–15.
    let buf = render(&TERMINUS, "\x1b[91mM");
    testrt::check(buf.iter().any(|&p| p == BRIGHT_RED), "SGR 91 paints bright red (index 9)");
});

arch_test!(console_sgr_truecolor_fg, {
    // `38;2;r;g;b` reaches the surface as the exact 24-bit color.
    let buf = render(&TERMINUS, "\x1b[38;2;171;205;239mM");
    testrt::check(buf.iter().any(|&p| p == 0x00AB_CDEF), "SGR 38;2 paints the exact truecolor");
});

arch_test!(console_sgr_background_fills_cell, {
    // A bg code repaints the cell's background pixels (coverage 0) while the
    // default fg still inks the glyph — exercises the reinstated `set_bg`.
    let named = render(&TERMINUS, "\x1b[42mM");
    testrt::check(named.iter().any(|&p| p == PALETTE_GREEN), "SGR 42 fills the cell background");
    testrt::check(named.iter().any(|&p| p == FG), "the default fg still inks the glyph");
    let truecolor = render(&TERMINUS, "\x1b[48;2;171;205;239mM");
    testrt::check(
        truecolor.iter().any(|&p| p == 0x00AB_CDEF),
        "SGR 48;2 fills a truecolor background",
    );
});

arch_test!(console_sgr_default_and_reset_restore_pens, {
    // `39` restores the default fg; `0` restores both pens (the reinstated
    // `default_bg`).
    let dflt_fg = render(&TERMINUS, "\x1b[31m\x1b[39mM");
    testrt::check(dflt_fg.iter().any(|&p| p == FG), "SGR 39 restores the default fg");
    testrt::check(!dflt_fg.iter().any(|&p| p == PALETTE_RED), "no red remains after SGR 39");
    let reset = render(&TERMINUS, "\x1b[31;42m\x1b[0mM");
    testrt::check(reset.iter().any(|&p| p == FG), "SGR 0 restores the default fg");
    testrt::check(reset.iter().any(|&p| p == BG), "SGR 0 restores the default bg");
    testrt::check(!reset.iter().any(|&p| p == PALETTE_GREEN), "no custom bg remains after SGR 0");
});

arch_test!(console_sgr_malformed_extended_leaves_cell_plain, {
    // A `38` with no mode, a `38;5` with no index, and a short `38;2` triple
    // are all malformed extended-color introducers. Each renders pixel-for-
    // pixel as the plain glyph: the pen does not move, and — critically — the
    // introducer consumes its trailing parameters, so a truncated triple's
    // leftover `1`/`2` are not reinterpreted as bold/dim attribute codes.
    let plain = render(&TERMINUS, "M");
    for seq in ["\x1b[38mM", "\x1b[38;5mM", "\x1b[38;2;1;2mM"] {
        testrt::check(
            render(&TERMINUS, seq) == plain,
            "malformed extended color leaves the cell identical to the plain glyph",
        );
    }
});

arch_test!(console_sgr_pen_change_affects_only_subsequent_cells, {
    // Two cells: cell A printed under a red pen, then SGR switches to green for
    // cell B. Immediate-mode means cell A is never repainted. The 16-wide
    // surface holds two 8-px cells (cols 0 and 1) on one row.
    let mut buf = [0u32; 16 * 16];
    let mut surface = StubSurface::new(buf.as_mut_ptr(), 16, 16, 16 * 4);
    // Two-cell backing for the two 8-px cells this 16-wide stub holds.
    let mut grid = [RESET_CELL; 2];
    let mut console = Console::new(
        surface.render_surface(),
        &TERMINUS,
        Color::Rgb(FG),
        Color::Rgb(BG),
        ScreenGrid(&mut grid),
    );
    console.print("\x1b[38;2;255;0;0mM"); // cell A, col 0 (red)
    console.print("\x1b[38;2;0;255;0mM"); // cell B, col 1 (green)

    let (mut a_red, mut a_green, mut b_green) = (false, false, false);
    for y in 0..16 {
        for x in 0..16 {
            let p = buf[y * 16 + x];
            if x < 8 {
                a_red |= p == 0x00FF_0000;
                a_green |= p == 0x0000_FF00;
            } else {
                b_green |= p == 0x0000_FF00;
            }
        }
    }
    testrt::check(a_red, "cell A keeps the original red pen");
    testrt::check(!a_green, "cell A was not repainted by the later green pen");
    testrt::check(b_green, "cell B uses the new green pen");
});

arch_test!(console_sgr_integration_colored_then_reset, {
    // Integration smoke: a colored glyph then a reset, end to end — parser,
    // SGR dispatch, pen, and blit composing across two cells.
    let mut buf = [0u32; 16 * 16];
    let mut surface = StubSurface::new(buf.as_mut_ptr(), 16, 16, 16 * 4);
    // Two-cell backing for the two 8-px cells this 16-wide stub holds.
    let mut grid = [RESET_CELL; 2];
    let mut console = Console::new(
        surface.render_surface(),
        &TERMINUS,
        Color::Rgb(FG),
        Color::Rgb(BG),
        ScreenGrid(&mut grid),
    );
    console.print("\x1b[31mA\x1b[0mB");

    let (mut a_red, mut b_default) = (false, false);
    for y in 0..16 {
        for x in 0..16 {
            let p = buf[y * 16 + x];
            if x < 8 {
                a_red |= p == PALETTE_RED;
            } else {
                b_default |= p == FG;
            }
        }
    }
    testrt::check(a_red, "the colored cell A blits palette red");
    testrt::check(b_default, "after SGR 0, cell B blits the default fg");
});

// The effect tests below blit through Terminus (exact `0x00`/`0xFF` coverage)
// so a post-effect pixel resolves to an exact color, and drive the effects the
// way a terminal does — SGR attributes embedded in the printed stream.

arch_test!(console_effect_reverse_swaps_pens, {
    // Reverse swaps the pens, so the large cell background is painted in the
    // foreground color: a reversed glyph has strictly more fg pixels than the
    // normal one, whose fg appears only in the ink.
    let normal = render(&TERMINUS, "M");
    let reversed = render(&TERMINUS, "\x1b[7mM");
    let normal_fg = normal.iter().filter(|&&p| p == FG).count();
    let reversed_fg = reversed.iter().filter(|&&p| p == FG).count();
    testrt::check(reversed_fg > normal_fg, "reverse inks the background in the fg pen");
    testrt::check(reversed.iter().any(|&p| p == BG), "reverse paints the glyph in the bg pen");
});

arch_test!(console_effect_reverse_paints_row_leading, {
    // Console rows are 16 glyph pixels plus 4 leading pixels. A visual cell
    // repaint owns both parts; otherwise reverse-video splash bars can leave
    // old pixels behind during refresh/scroll.
    let mut buf = [0u32; 8 * 20];
    let mut surface = StubSurface::new(buf.as_mut_ptr(), 8, 20, 8 * 4);
    let mut grid = [RESET_CELL; 1];
    let mut console = Console::new(
        surface.render_surface(),
        &TERMINUS,
        Color::Rgb(FG),
        Color::Rgb(BG),
        ScreenGrid(&mut grid),
    );
    console.print("\x1b[7m ");

    let leading_is_reverse_bg = (16..20).all(|y| (0..8).all(|x| buf[y * 8 + x] == FG));
    testrt::check(
        leading_is_reverse_bg,
        "reverse-video repaint fills row-leading pixels with the effective background",
    );
});

arch_test!(console_effect_dim_halves_foreground, {
    // Dim composites the foreground over the background at half coverage, so
    // the ink resolves to exactly that blend and no full-intensity fg remains.
    let dim_fg = blend(FG, BG, 128);
    let dim = render(&TERMINUS, "\x1b[2mM");
    testrt::check(dim.iter().any(|&p| p == dim_fg), "dim inks at half intensity toward bg");
    testrt::check(!dim.iter().any(|&p| p == FG), "no full-intensity ink remains under dim");
    testrt::check(dim.iter().any(|&p| p == BG), "dim leaves the cell background unchanged");
});

arch_test!(console_effect_off_codes_restore_normal, {
    // The off-codes return the cell to the plain render: `27` clears reverse,
    // `22` clears dim (and bold).
    let normal = render(&TERMINUS, "M");
    testrt::check(render(&TERMINUS, "\x1b[7m\x1b[27mM") == normal, "SGR 27 clears reverse");
    testrt::check(render(&TERMINUS, "\x1b[2m\x1b[22mM") == normal, "SGR 22 clears dim");
});

arch_test!(console_effect_underline_paints_row, {
    // Underline overlays a full-cell-width fg row near the cell bottom — it
    // shows even on a space, where the plain render has none.
    let row = (16 - 2) as usize; // cell_h - UNDERLINE_INSET
    let underlined = render(&TERMINUS, "\x1b[4m ");
    testrt::check(
        (0..8).all(|x| underlined[row * 8 + x] == FG),
        "underline paints a solid fg row across the cell",
    );
    let plain = render(&TERMINUS, " ");
    testrt::check((0..8).all(|x| plain[row * 8 + x] == BG), "a plain space has no underline row");
});

arch_test!(console_effect_underline_off_code, {
    // `24` removes the underline, restoring the plain space.
    testrt::check(
        render(&TERMINUS, "\x1b[4m\x1b[24m ") == render(&TERMINUS, " "),
        "SGR 24 removes the underline",
    );
});

arch_test!(console_effect_underline_uses_reverse_fg, {
    // Underline + reverse: the cell background fills with the (original) fg,
    // and the underline row draws in the post-reverse fg — the original bg.
    let reversed_underline = render(&TERMINUS, "\x1b[7;4m ");
    let row = (16 - 2) as usize;
    testrt::check(
        (0..8).all(|x| reversed_underline[row * 8 + x] == BG),
        "the underline row uses the post-reverse foreground",
    );
    testrt::check(
        (0..8).all(|x| reversed_underline[x] == FG),
        "the reversed space fills the rest of the cell with the fg pen",
    );
});

arch_test!(console_effect_bold_widens_ink, {
    // Synthetic bold dilates each stem one pixel to the right, so the bold
    // glyph has strictly more inked pixels than the normal one.
    let normal_ink = render(&TERMINUS, "M").iter().filter(|&&p| p != BG).count();
    let bold_ink = render(&TERMINUS, "\x1b[1mM")
        .iter()
        .filter(|&&p| p != BG)
        .count();
    testrt::check(bold_ink > normal_ink, "bold widens the glyph ink");
});

arch_test!(console_effect_bold_leaves_leftmost_column, {
    // The leftmost column has no left neighbor, so bold's rightward dilation
    // does not apply there: column 0 must match the plain glyph exactly. This
    // isolates the `src > 0` false branch of the dilation.
    let normal = render(&TERMINUS, "M");
    let bold = render(&TERMINUS, "\x1b[1mM");
    testrt::check(
        (0..16).all(|y| bold[y * 8] == normal[y * 8]),
        "bold leaves the leftmost column unchanged (no left neighbor to dilate)",
    );
});

arch_test!(console_effect_italic_shears_upper_rows, {
    // Synthetic italic shifts the upper rows right of the lower ones. The shear
    // changes the render of any glyph shape (a sweep over a thin bar and a
    // two-stem letter), and a vertical bar makes the lean direction visible:
    // its topmost inked row sits at or right of its bottommost. The left-edge
    // clip (upper rows whose source falls left of the cell) is exercised here
    // without panic.
    for (italic_seq, plain) in [("\x1b[3m|", "|"), ("\x1b[3mH", "H")] {
        testrt::check(
            render(&TERMINUS, italic_seq) != render(&TERMINUS, plain),
            "italic shears the glyph, changing the render",
        );
    }
    let italic = render(&TERMINUS, "\x1b[3m|");

    let ink_col = |cell: &[u32; CELL_PIXELS], row: usize| (0..8).find(|&x| cell[row * 8 + x] == FG);
    let top = (0..16).find_map(|r| ink_col(&italic, r).map(|c| (r, c)));
    let bottom = (0..16)
        .rev()
        .find_map(|r| ink_col(&italic, r).map(|c| (r, c)));
    match (top, bottom) {
        (Some((tr, tc)), Some((br, bc))) => {
            testrt::check(tr < br, "the bar spans multiple rows");
            testrt::check(tc >= bc, "italic never shifts an upper row left of a lower one");
        }
        _ => testrt::check(false, "the italic bar must render ink"),
    }
});

arch_test!(console_effect_attributes_compose_and_reset, {
    // Bold + italic + a color pen compose on cell A; SGR 0 then clears every
    // attribute and the pen, so cell B is pixel-identical to the plain glyph.
    let mut buf = [0u32; 16 * 16];
    let mut surface = StubSurface::new(buf.as_mut_ptr(), 16, 16, 16 * 4);
    // Two-cell backing for the two 8-px cells this 16-wide stub holds.
    let mut grid = [RESET_CELL; 2];
    let mut console = Console::new(
        surface.render_surface(),
        &TERMINUS,
        Color::Rgb(FG),
        Color::Rgb(BG),
        ScreenGrid(&mut grid),
    );
    console.print("\x1b[1;3;31mM"); // cell A: bold italic red
    console.print("\x1b[0mM"); // cell B: reset → plain default glyph

    let plain = render(&TERMINUS, "M");
    let mut a_red = false;
    let mut b_matches_plain = true;
    for y in 0..16 {
        for x in 0..16 {
            let p = buf[y * 16 + x];
            if x < 8 {
                a_red |= p == PALETTE_RED;
            } else if p != plain[y * 8 + (x - 8)] {
                b_matches_plain = false;
            }
        }
    }
    testrt::check(a_red, "composed cell A inks bold + italic in the color pen");
    testrt::check(
        b_matches_plain,
        "SGR 0 clears every attribute and pen: cell B is the plain glyph",
    );
});

arch_test!(console_scroll_shift_rows_up_moves_content, {
    // A 2-col x 3-row grid. Shifting up drops the top row, moves the lower rows
    // up one, and blanks the freed bottom row — pure grid arithmetic, no blit.
    let mut grid = [
        Cell {
            ch: b'A',
            ..RESET_CELL
        },
        Cell {
            ch: b'a',
            ..RESET_CELL
        },
        Cell {
            ch: b'B',
            ..RESET_CELL
        },
        Cell {
            ch: b'b',
            ..RESET_CELL
        },
        Cell {
            ch: b'C',
            ..RESET_CELL
        },
        Cell {
            ch: b'c',
            ..RESET_CELL
        },
    ];
    shift_rows_up(
        &mut grid,
        2,
        6,
        Cell {
            ch: b' ',
            ..RESET_CELL
        },
    );
    testrt::check_eq(grid[0].ch, b'B');
    testrt::check_eq(grid[1].ch, b'b');
    testrt::check_eq(grid[2].ch, b'C');
    testrt::check_eq(grid[3].ch, b'c');
    testrt::check_eq(grid[4].ch, b' ');
    testrt::check_eq(grid[5].ch, b' ');
});

arch_test!(console_scroll_shift_rows_up_single_row_blanks, {
    // The exact-capacity boundary: a one-row grid has no row to pull down, so
    // the `copy_within` source range is empty and the row is simply blanked.
    let mut grid = [
        Cell {
            ch: b'Z',
            ..RESET_CELL
        },
        Cell {
            ch: b'z',
            ..RESET_CELL
        },
    ];
    shift_rows_up(
        &mut grid,
        2,
        2,
        Cell {
            ch: b' ',
            ..RESET_CELL
        },
    );
    testrt::check_eq(grid[0].ch, b' ');
    testrt::check_eq(grid[1].ch, b' ');
});

arch_test!(console_scroll_drops_top_keeps_newest_at_bottom, {
    // A 1-col x 3-row surface. Writing four lines overflows it: the first line
    // scrolls off, the grid retains the last three (B, C, D) top to bottom, and
    // the surface is repainted from the grid — no framebuffer readback.
    let mut buf = [0u32; 8 * 60]; // three 20px row pitches tall
    let mut surface = StubSurface::new(buf.as_mut_ptr(), 8, 60, 8 * 4);
    let mut grid = [RESET_CELL; 3];
    let mut console = Console::new(
        surface.render_surface(),
        &TERMINUS,
        Color::Rgb(FG),
        Color::Rgb(BG),
        ScreenGrid(&mut grid),
    );
    console.print("A\nB\nC\nD");

    testrt::check_eq(console.grid[0].ch, b'B');
    testrt::check_eq(console.grid[1].ch, b'C');
    testrt::check_eq(console.grid[2].ch, b'D');

    // The bottom row (pixels y=40..56) was repainted from the grid and inks the
    // newest glyph.
    let bottom_inked = (40..56).any(|y| (0..8).any(|x| buf[y * 8 + x] == FG));
    testrt::check(bottom_inked, "the newest line is painted in the bottom row");
});

arch_test!(console_cursor_block_shows_then_restores_on_write, {
    // A 3-col surface. After printing "A" the head is the empty cell to its
    // right; show_cursor paints a reverse-video block there (a blank cell
    // inverted is solid foreground). Printing "B" erases the block (repaints
    // the true cell) and writes B in its place — the cursor never persists in
    // the grid.
    let mut buf = [0u32; 24 * 16]; // three 8px columns, one row
    let mut surface = StubSurface::new(buf.as_mut_ptr(), 24, 16, 24 * 4);
    let mut grid = [RESET_CELL; 3];
    let mut console = Console::new(
        surface.render_surface(),
        &TERMINUS,
        Color::Rgb(FG),
        Color::Rgb(BG),
        ScreenGrid(&mut grid),
    );
    console.print("A"); // A at col 0; head at col 1 (empty)
    console.show_cursor(); // block fills col 1

    // Col 1 (pixels x=8..16) is a blank cell, so its reverse-video block is
    // solid foreground in every pixel.
    let mut block_solid = true;
    for y in 0..16 {
        for x in 8..16 {
            block_solid &= buf[y * 24 + x] == FG;
        }
    }
    testrt::check(block_solid, "the block cursor fills the empty head cell with fg");

    console.print("B"); // erases the block, writes B where the cursor was
    let (mut restored, mut glyph_drawn) = (false, false);
    for y in 0..16 {
        for x in 8..16 {
            let p = buf[y * 24 + x];
            restored |= p == BG;
            glyph_drawn |= p == FG;
        }
    }
    testrt::check(restored, "printing past the cursor erases the block (cell restored)");
    testrt::check(glyph_drawn, "the new glyph is painted where the cursor was");
});

arch_test!(console_backspace_space_backspace_erases_previous_cell, {
    // The line editor echoes erase as BS SP BS. The console must interpret the
    // backspaces as cursor movement so the space lands over the previous glyph
    // and the next printable byte reuses that same cell.
    let mut buf = [0u32; 16 * 16];
    let mut surface = StubSurface::new(buf.as_mut_ptr(), 16, 16, 16 * 4);
    let mut grid = [RESET_CELL; 2];
    let mut console = Console::new(
        surface.render_surface(),
        &TERMINUS,
        Color::Rgb(FG),
        Color::Rgb(BG),
        ScreenGrid(&mut grid),
    );
    console.print("AB\x08 \x08C");

    testrt::check_eq(console.grid[0].ch, b'A');
    testrt::check_eq(console.grid[1].ch, b'C');
});

arch_test!(console_csi_k_erases_stale_carriage_return_text, {
    // A countdown/status redraw often prints a shorter replacement after CR.
    // CSI K must clear the retained cells and repaint the framebuffer so the
    // old suffix cannot remain as ghost text.
    let mut buf = [0u32; 48 * 16];
    let mut surface = StubSurface::new(buf.as_mut_ptr(), 48, 16, 48 * 4);
    let mut grid = [RESET_CELL; 6];
    let mut console = Console::new(
        surface.render_surface(),
        &TERMINUS,
        Color::Rgb(FG),
        Color::Rgb(BG),
        ScreenGrid(&mut grid),
    );
    console.print("abcdef\rxy\x1b[K");

    testrt::check_eq(console.grid[0].ch, b'x');
    testrt::check_eq(console.grid[1].ch, b'y');
    let mut col = 2usize;
    while col < 6 {
        testrt::check_eq(console.grid[col].ch, b' ');
        col += 1;
    }

    let mut erased_cell_is_background = true;
    for y in 0..16 {
        for x in 16..24 {
            erased_cell_is_background &= buf[y * 48 + x] == BG;
        }
    }
    testrt::check(
        erased_cell_is_background,
        "cell after shorter redraw is repainted to background",
    );
});
