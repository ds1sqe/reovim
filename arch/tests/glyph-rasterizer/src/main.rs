//! Offline glyph rasterizer for the bare-metal console.
//!
//! Reads the two vendored OFL fonts under `assets/` and emits checked-in
//! `static` grayscale-coverage tables into the shipped `arch` crate's
//! `fonts/` module:
//!
//! - JetBrains Mono (TrueType) -> anti-aliased coverage (`fontdue`).
//! - Terminus (BDF bitmap) -> 0x00/0xFF coverage.
//!
//! Both target the same `CELL_W` x `CELL_H` monospace cell over the printable
//! range `FIRST..=LAST`, so the console blits either through one path. Output
//! is deterministic: rerunning reproduces byte-identical tables.
//!
//! This crate is dev-only and depgraph-shielded (see `Cargo.toml`); the shipped
//! `arch` crate embeds only the emitted data, never this tool or its deps.

mod bdf;
mod emit;
mod raster;

use std::{fs, path::Path};

/// Console cell width in pixels (matches Terminus `ter-u16n`, the classic
/// 8x16 text cell).
const CELL_W: u32 = 8;
/// Console cell height in pixels.
const CELL_H: u32 = 16;
/// First glyph emitted (ASCII space).
const FIRST: u8 = 0x20;
/// Last glyph emitted (the U+007F slot, blank).
const LAST: u8 = 0x7F;

/// One glyph's row-major coverage cell: `width * height` bytes, `0x00` = no
/// ink (background), `0xFF` = full ink (foreground), values between are
/// anti-aliased edge coverage.
#[derive(Clone)]
pub struct Cell {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Cell {
    /// A fully-transparent cell of the given size.
    fn blank(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height) as usize],
        }
    }

    /// Sets coverage at `(x, y)`, ignoring out-of-cell coordinates so callers
    /// can place glyphs without bounds bookkeeping.
    fn set(&mut self, x: i32, y: i32, coverage: u8) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        self.pixels[(y as u32 * self.width + x as u32) as usize] = coverage;
    }

    /// Row-major coverage bytes.
    fn bytes(&self) -> &[u8] {
        &self.pixels
    }
}

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let assets = Path::new(manifest).join("assets");
    let out = Path::new(manifest).join("../../src/sys/none_aarch64/fonts");
    fs::create_dir_all(&out).expect("create fonts dir");

    // JetBrains Mono — anti-aliased.
    let ttf = fs::read(assets.join("JetBrainsMono-Regular.ttf")).expect("read JBM TTF");
    let jbm = raster::rasterize(&ttf, CELL_W, CELL_H, FIRST, LAST);
    emit::write(
        &out.join("jetbrains_mono.rs"),
        &jbm,
        emit::Meta {
            display: "JetBrains Mono",
            kind: "anti-aliased",
            source: "JetBrainsMono-Regular.ttf (JetBrains Mono v2 release)",
            ofl_file: "JetBrainsMono-OFL.txt",
        },
    );

    // Terminus — crisp bitmap.
    let bdf_src = fs::read_to_string(assets.join("ter-u16n.bdf")).expect("read Terminus BDF");
    let ter = bdf::rasterize(&bdf_src, CELL_W, CELL_H, FIRST, LAST);
    emit::write(
        &out.join("terminus.rs"),
        &ter,
        emit::Meta {
            display: "Terminus",
            kind: "bitmap",
            source: "ter-u16n.bdf (terminus-font 4.49)",
            ofl_file: "Terminus-OFL.txt",
        },
    );

    println!(
        "emitted {} glyphs/font ({}x{} cell) for U+{:04X}..=U+{:04X}",
        (LAST - FIRST) as usize + 1,
        CELL_W,
        CELL_H,
        FIRST,
        LAST
    );
}
