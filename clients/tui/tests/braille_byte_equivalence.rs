//! `Braille` byte-equivalence integration test.
//!
//! Projects a canonical 4×8 logical `CellCapability` through
//! `BrailleRasterizer` into a 2×2 `RecordingRasterOutput`, serializes
//! the result, and compares to the fixture
//! `braille_byte_equivalence_fixture.txt`. A second-rasterization
//! idempotency check guards against hidden non-determinism in the
//! rasterizer.
//!
//! Expected encoding-policy coverage (all 4 cases in the fixture):
//!
//! | Terminal (x, y) | Sub-grid (logical anchor + dots)               | Result            |
//! |-----------------|------------------------------------------------|-------------------|
//! | (0, 0)          | anchor (0, 0); every cell default               | ` `  plain        |
//! | (1, 0)          | anchor (2, 0); all 8 cells bg=Ansi(1)           | `⣿`  fg=Ansi(1)   |
//! | (0, 1)          | anchor (0, 4); only (0, 4) bg=Ansi(2)           | `⠁`  fg=Ansi(2)   |
//! | (1, 1)          | anchor (2, 4); (2, 4) bg=Ansi(1), (3, 5) Ansi(4), rest default | `⠑`  fg=Ansi(1) (first-in-scan) |

use {
    reovim_ext_client_tui_cap_cell::{Cell, CellCapability, CellColor, CellStyle},
    reovim_ext_client_tui_cap_cell_view::{
        BrailleRasterizer, RasterCell, ViewRasterizer, raster::RecordingRasterOutput,
    },
};

const FIXTURE: &str = include_str!("braille_byte_equivalence_fixture.txt");

/// Build the canonical 4×8 grid described in the module doc-comment.
fn canonical_grid() -> CellCapability {
    let mut g = CellCapability::new(4, 8);
    let put = |g: &mut CellCapability, x: u16, y: u16, bg: CellColor| {
        g.write_cell(x, y, Cell::new(' ', CellStyle::plain().with_bg(bg)))
            .unwrap();
    };

    // Terminal (1, 0): anchor (2, 0), all 8 cells bg=Ansi(1).
    for dx in 0..2u16 {
        for dy in 0..4u16 {
            put(&mut g, 2 + dx, dy, CellColor::Ansi256(1));
        }
    }

    // Terminal (0, 1): anchor (0, 4), only (0, 4) bg=Ansi(2).
    put(&mut g, 0, 4, CellColor::Ansi256(2));

    // Terminal (1, 1): anchor (2, 4), mixed.
    put(&mut g, 2, 4, CellColor::Ansi256(1)); // dot (0, 0) of sub-grid
    put(&mut g, 3, 5, CellColor::Ansi256(4)); // dot (1, 1) of sub-grid

    g
}

/// Serialize a recorded cell into the `<x>,<y>,<ch>,<fg>,<bg>` form.
fn encode_cell(c: &RasterCell) -> String {
    format!(
        "{},{},{},{},{}",
        c.x,
        c.y,
        c.ch,
        encode_color(c.style.fg),
        encode_color(c.style.bg)
    )
}

fn encode_color(c: Option<CellColor>) -> String {
    match c {
        None => "-".into(),
        Some(CellColor::Default) => "default".into(),
        Some(CellColor::Ansi256(n)) => format!("ansi:{n}"),
        Some(CellColor::Named(n)) => format!("named:{n}"),
        Some(CellColor::Rgb(r, g, b)) => format!("rgb:{r},{g},{b}"),
    }
}

/// Strip the fixture's comment/blank lines, leaving only the data rows.
fn parse_fixture(src: &str) -> Vec<String> {
    src.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[test]
fn braille_rasterize_matches_fixture_byte_for_byte() {
    let grid = canonical_grid();
    let mut out = RecordingRasterOutput::new(2, 2);
    BrailleRasterizer::new().rasterize(&grid, &mut out);

    let actual: Vec<String> = out.cells().iter().map(encode_cell).collect();
    let expected = parse_fixture(FIXTURE);

    assert_eq!(
        actual, expected,
        "braille-rasterized cells diverge from fixture\n\
         actual:   {actual:#?}\n\
         expected: {expected:#?}"
    );
}

#[test]
fn braille_rasterize_is_idempotent() {
    // Two rasterize calls on the same grid must produce byte-identical
    // output — no hidden non-determinism.
    let grid = canonical_grid();
    let mut first = RecordingRasterOutput::new(2, 2);
    let mut second = RecordingRasterOutput::new(2, 2);
    BrailleRasterizer::new().rasterize(&grid, &mut first);
    BrailleRasterizer::new().rasterize(&grid, &mut second);
    assert_eq!(first.cells(), second.cells());
}
