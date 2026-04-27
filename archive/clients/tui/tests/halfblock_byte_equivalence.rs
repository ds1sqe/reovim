//! `HalfBlock` byte-equivalence integration test.
//!
//! Projects a canonical 4×6 logical `CellCapability` through
//! `HalfBlockRasterizer` into a 4×3 `RecordingRasterOutput`, serializes
//! the result, and compares to the fixture
//! `halfblock_byte_equivalence_fixture.txt`. A second-rasterization
//! idempotency check guards against hidden non-determinism in the
//! rasterizer (e.g. hash-iteration leaks in future refactors).
//!
//! Expected encoding-policy coverage (all 5 rows of the table in
//! `half_block.rs` module docs):
//!
//! | Terminal (x, y) | Logical top          | Logical bot          | Result            |
//! |-----------------|----------------------|----------------------|-------------------|
//! | (0, 0)          | default              | default              | ` ` plain         |
//! | (1, 0)          | space + bg=Ansi(1)   | space + bg=Ansi(1)   | ` ` bg=Ansi(1)    |
//! | (2, 0)          | space + bg=Ansi(5)   | space + bg=Ansi(4)   | `▀` fg=5 bg=4     |
//! | (3, 0)          | space + bg=Ansi(7)   | space + bg=Ansi(0)   | `▀` fg=7 bg=0     |
//! | (0, 1)          | default              | default              | ` ` plain         |
//! | (1, 1)          | space + bg=Ansi(2)   | default              | `▀` fg=2          |
//! | (2, 1)          | default              | default              | ` ` plain         |
//! | (3, 1)          | space + bg=Ansi(4)   | space + bg=Ansi(4)   | ` ` bg=Ansi(4)    |
//! | (0, 2)          | default              | default              | ` ` plain         |
//! | (1, 2)          | default              | space + bg=Ansi(3)   | `▄` fg=3          |
//! | (2, 2)          | space + bg=Ansi(6)   | space + bg=Ansi(6)   | ` ` bg=Ansi(6)    |
//! | (3, 2)          | default              | default              | ` ` plain         |

use {
    reovim_ext_client_tui_cap_cell::{Cell, CellCapability, CellColor, CellStyle},
    reovim_ext_client_tui_cap_cell_view::{
        HalfBlockRasterizer, RasterCell, ViewRasterizer, raster::RecordingRasterOutput,
    },
};

const FIXTURE: &str = include_str!("halfblock_byte_equivalence_fixture.txt");

/// Build the canonical 4×6 grid described in the module doc-comment.
fn canonical_grid() -> CellCapability {
    let mut g = CellCapability::new(4, 6);
    let put = |g: &mut CellCapability, x: u16, y: u16, bg: CellColor| {
        g.write_cell(x, y, Cell::new(' ', CellStyle::plain().with_bg(bg)))
            .unwrap();
    };

    // Column 1 — shared / top-only / bottom-only
    put(&mut g, 1, 0, CellColor::Ansi256(1));
    put(&mut g, 1, 1, CellColor::Ansi256(1));
    put(&mut g, 1, 2, CellColor::Ansi256(2)); // top of pair 1
    put(&mut g, 1, 5, CellColor::Ansi256(3)); // bot of pair 2

    // Column 2 — mixed / blank / shared
    put(&mut g, 2, 0, CellColor::Ansi256(5));
    put(&mut g, 2, 1, CellColor::Ansi256(4));
    put(&mut g, 2, 4, CellColor::Ansi256(6));
    put(&mut g, 2, 5, CellColor::Ansi256(6));

    // Column 3 — mixed / shared / blank
    put(&mut g, 3, 0, CellColor::Ansi256(7));
    put(&mut g, 3, 1, CellColor::Ansi256(0));
    put(&mut g, 3, 2, CellColor::Ansi256(4));
    put(&mut g, 3, 3, CellColor::Ansi256(4));

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
fn halfblock_rasterize_matches_fixture_byte_for_byte() {
    let grid = canonical_grid();
    let mut out = RecordingRasterOutput::new(4, 3);
    HalfBlockRasterizer::new().rasterize(&grid, &mut out);

    let actual: Vec<String> = out.cells().iter().map(encode_cell).collect();
    let expected = parse_fixture(FIXTURE);

    assert_eq!(
        actual, expected,
        "halfblock-rasterized cells diverge from fixture\n\
         actual:   {actual:#?}\n\
         expected: {expected:#?}"
    );
}

#[test]
fn halfblock_rasterize_is_idempotent() {
    // Rasterizing the same grid twice must produce byte-identical
    // output. Guards against hash-iteration leaks or other hidden
    // non-determinism creeping into future rasterizer refactors.
    let grid = canonical_grid();
    let mut first = RecordingRasterOutput::new(4, 3);
    let mut second = RecordingRasterOutput::new(4, 3);
    HalfBlockRasterizer::new().rasterize(&grid, &mut first);
    HalfBlockRasterizer::new().rasterize(&grid, &mut second);
    assert_eq!(first.cells(), second.cells());
}
