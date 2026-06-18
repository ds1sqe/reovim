//! Minimal BDF (Glyph Bitmap Distribution Format) reader.
//!
//! Terminus ships as BDF source. We only need the printable-ASCII subset and
//! the per-glyph bitmap, so this parser handles just the records we touch:
//! `FONTBOUNDINGBOX`, and per-glyph `ENCODING` / `BBX` / `BITMAP`. A bitmap
//! font is pure coverage, so each set bit becomes `0xFF` and each clear bit
//! `0x00` in the emitted cell.

use crate::Cell;

/// Font bounding box height and x/y offsets relative to the glyph origin (the
/// baseline). Mirrors the BDF `FONTBOUNDINGBOX` record; the box width is
/// unused because per-glyph `BBX` records carry their own widths.
struct FontBox {
    height: i32,
    x_off: i32,
    y_off: i32,
}

/// Rasterizes the printable range `first..=last` of a BDF font into row-major
/// coverage cells sized `cell_w` x `cell_h`. Codepoints absent from the font
/// (or that fall outside the cell) yield a blank cell.
///
/// Placement honors the BDF baseline: a glyph's bounding box is positioned so
/// the font's common baseline lands at the same cell row for every glyph, with
/// descenders extending below it.
pub fn rasterize(src: &str, cell_w: u32, cell_h: u32, first: u8, last: u8) -> Vec<Cell> {
    let font_box = parse_font_box(src).expect("BDF missing FONTBOUNDINGBOX");
    // Baseline measured from the top of the cell: the box top sits
    // `y_off + height` pixels above the baseline, and the cell top is the box
    // top, so the baseline is that many rows down from row 0.
    let baseline_from_top = font_box.y_off + font_box.height;

    let count = (last - first) as usize + 1;
    let mut cells = vec![Cell::blank(cell_w, cell_h); count];

    let mut lines = src.lines().peekable();
    while let Some(line) = lines.next() {
        if !line.starts_with("STARTCHAR") {
            continue;
        }
        let mut encoding: Option<i32> = None;
        let mut bbx: Option<(i32, i32, i32, i32)> = None;
        // Advance to BITMAP, capturing ENCODING and BBX along the way.
        for header in lines.by_ref() {
            if let Some(rest) = header.strip_prefix("ENCODING ") {
                encoding = rest.trim().parse().ok();
            } else if let Some(rest) = header.strip_prefix("BBX ") {
                let v: Vec<i32> = rest.split_whitespace().filter_map(|t| t.parse().ok()).collect();
                if let [w, h, xo, yo] = v[..] {
                    bbx = Some((w, h, xo, yo));
                }
            } else if header.starts_with("BITMAP") {
                break;
            }
        }
        let (Some(cp), Some((bw, bh, bx, by))) = (encoding, bbx) else {
            continue;
        };
        // Collect the bitmap rows until ENDCHAR.
        let mut rows: Vec<u32> = Vec::with_capacity(bh.max(0) as usize);
        for bitmap_line in lines.by_ref() {
            if bitmap_line.starts_with("ENDCHAR") {
                break;
            }
            rows.push(u32::from_str_radix(bitmap_line.trim(), 16).unwrap_or(0));
        }
        if cp < i32::from(first) || cp > i32::from(last) {
            continue;
        }

        let cell = &mut cells[(cp - i32::from(first)) as usize];
        // Top row of this glyph's box, in cell coordinates.
        let top = baseline_from_top - (by + bh);
        let left = bx - font_box.x_off;
        // BDF packs each row MSB-first into ceil(bw/8) bytes; the value we read
        // is left-justified within those bytes, so the leftmost pixel is the
        // high bit of the top byte.
        let row_bytes = (bw as u32).div_ceil(8);
        let msb_shift = row_bytes * 8 - 1;
        for (i, &bits) in rows.iter().enumerate() {
            let cy = top + i as i32;
            for j in 0..bw {
                if bits >> (msb_shift - j as u32) & 1 == 0 {
                    continue;
                }
                cell.set(left + j, cy, 0xFF);
            }
        }
    }
    cells
}

/// Parses the single `FONTBOUNDINGBOX w h x y` record.
fn parse_font_box(src: &str) -> Option<FontBox> {
    let line = src.lines().find(|l| l.starts_with("FONTBOUNDINGBOX "))?;
    let v: Vec<i32> = line
        .trim_start_matches("FONTBOUNDINGBOX ")
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    match v[..] {
        [_width, height, x_off, y_off] => Some(FontBox {
            height,
            x_off,
            y_off,
        }),
        _ => None,
    }
}
