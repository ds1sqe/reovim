//! Renders coverage cells into a checked-in Rust source file.
//!
//! The emitted file is pure data — `CELL_W`/`CELL_H`/`FIRST`/`LAST` constants
//! and a `static GLYPHS` coverage array — with no reference to the `Font`
//! descriptor type. That keeps the generated tables decoupled from where the
//! descriptor lives, so the `fonts/` module can move without regenerating.

use crate::{Cell, CELL_H, CELL_W, FIRST, LAST};
use std::{fmt::Write as _, fs, path::Path};

/// Provenance for the generated file's header comment.
pub struct Meta {
    /// Human-facing font name (also the descriptor `name` in `fonts/mod.rs`).
    pub display: &'static str,
    /// "anti-aliased TrueType" or "bitmap" — describes the coverage origin.
    pub kind: &'static str,
    /// The vendored source file the table was derived from.
    pub source: &'static str,
    /// The OFL license file under `assets/`.
    pub ofl_file: &'static str,
}

/// Writes the coverage table for `cells` to `path` as formatted Rust source.
pub fn write(path: &Path, cells: &[Cell], meta: Meta) {
    let bytes_per_glyph = (CELL_W * CELL_H) as usize;
    let total = cells.len() * bytes_per_glyph;
    let mut s = String::new();

    let _ = write!(
        s,
        "//! GENERATED FILE — do not edit by hand.\n\
         //!\n\
         //! Coverage table for `{display}` ({kind}) over \
         U+{first:04X}..=U+{last:04X},\n\
         //! {w}x{h} cell, row-major, one byte per pixel (0x00 = background .. \
         0xFF =\n\
         //! foreground). Regenerate with `cargo run` in \
         `arch/tests/glyph-rasterizer/`.\n\
         //!\n\
         //! Source: `{source}`. Licensed under the SIL Open Font License 1.1; \
         license\n\
         //! text at `arch/tests/glyph-rasterizer/assets/{ofl}`. The table \
         below is our\n\
         //! own coverage data derived from that font, embedded because the \
         bare-metal\n\
         //! console has no filesystem to load fonts from (mirrors the \
         `font8x8` CC0\n\
         //! precedent in `console.rs`).\n\n\
         /// Glyph cell width in pixels.\n\
         pub const CELL_W: u32 = {w};\n\
         /// Glyph cell height in pixels.\n\
         pub const CELL_H: u32 = {h};\n\
         /// First glyph in [`GLYPHS`] (ASCII space).\n\
         pub const FIRST: u8 = 0x{first:02X};\n\
         /// Last glyph in [`GLYPHS`].\n\
         pub const LAST: u8 = 0x{last:02X};\n\n\
         /// Row-major coverage, `CELL_W * CELL_H` bytes per glyph, \
         `LAST - FIRST + 1` glyphs.\n\
         #[rustfmt::skip]\n\
         pub static GLYPHS: [u8; {total}] = [\n",
        display = meta.display,
        kind = meta.kind,
        source = meta.source,
        ofl = meta.ofl_file,
        first = FIRST,
        last = LAST,
        w = CELL_W,
        h = CELL_H,
        total = total,
    );

    for (idx, cell) in cells.iter().enumerate() {
        let cp = FIRST + idx as u8;
        let _ = writeln!(s, "    // {}", label(cp));
        for row in cell.bytes().chunks(CELL_W as usize) {
            s.push_str("    ");
            for &b in row {
                let _ = write!(s, "0x{b:02X}, ");
            }
            s.push('\n');
        }
    }
    s.push_str("];\n");

    fs::write(path, s).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// A readable label for the per-glyph comment.
fn label(cp: u8) -> String {
    match cp {
        0x20 => "U+0020 space".to_string(),
        0x7F => "U+007F DEL".to_string(),
        _ => format!("U+{:04X} '{}'", cp, cp as char),
    }
}
