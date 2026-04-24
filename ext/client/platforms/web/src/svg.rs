//! `WebFrame → SVG / HTML string` rendering for the Flight-76 spike.
//!
//! ## Encoding policy (locked in Plan 03)
//!
//! - One `<text>` element per non-blank cell, absolutely positioned at
//!   `x = col * VIEW_COL_PX`, `y = (row + 1) * VIEW_ROW_PX - 3`
//!   (baseline offset so the glyph sits inside its row rectangle).
//! - `fill` attribute:
//!   - `WebColor::Rgb(r, g, b)` → `fill="#rrggbb"`
//!   - `WebColor::Named(n)`      → `fill="{n}"`
//!   - `WebColor::Default` or `None` → omit the attribute.
//! - Space glyphs (`' '`) are skipped — they would be invisible and
//!   only inflate the snapshot.
//! - Row-major deterministic order over `(y, x)`.
//! - The root `<svg>` carries `viewBox="0 0 {W * VIEW_COL_PX} {H *
//!   VIEW_ROW_PX}"`, `font-family="monospace"`, `font-size="14"`.
//! - No inline `<style>`, no stylesheet reference.
//!
//! ## Whitespace contract (locked)
//!
//! [`render_page`] emits a single-line HTML5 document with zero
//! inter-tag newlines, leading, trailing, or indentation whitespace.
//! The snapshot test is byte-for-byte, so any future cosmetic reformat
//! breaks the test on purpose.

use std::fmt::Write;

use crate::frame::{WebColor, WebFrame};

pub(crate) const VIEW_COL_PX: u16 = 8;
pub(crate) const VIEW_ROW_PX: u16 = 16;
pub(crate) const FONT_SIZE: u16 = 14;

/// Render a [`WebFrame`] as a standalone SVG string. The output is the
/// exact SVG body referenced by [`render_page`] — no wrapping HTML.
#[must_use]
pub fn render_frame_svg(frame: &WebFrame) -> String {
    let w_px = u32::from(frame.width()) * u32::from(VIEW_COL_PX);
    let h_px = u32::from(frame.height()) * u32::from(VIEW_ROW_PX);
    let mut out =
        String::with_capacity(64 + usize::from(frame.width()) * usize::from(frame.height()) * 48);
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w_px} {h_px}\" \
         font-family=\"monospace\" font-size=\"{FONT_SIZE}\">"
    );
    for y in 0..frame.height() {
        for x in 0..frame.width() {
            let Some(cell) = frame.cell(x, y) else {
                continue;
            };
            if cell.ch == ' ' {
                continue;
            }
            let px = u32::from(x) * u32::from(VIEW_COL_PX);
            let py = u32::from(y + 1) * u32::from(VIEW_ROW_PX) - 3;
            match cell.fg {
                Some(WebColor::Rgb(r, g, b)) => {
                    let _ =
                        write!(out, "<text x=\"{px}\" y=\"{py}\" fill=\"#{r:02x}{g:02x}{b:02x}\">");
                }
                Some(WebColor::Named(n)) => {
                    let _ = write!(out, "<text x=\"{px}\" y=\"{py}\" fill=\"{n}\">");
                }
                Some(WebColor::Default) | None => {
                    let _ = write!(out, "<text x=\"{px}\" y=\"{py}\">");
                }
            }
            append_xml_char(&mut out, cell.ch);
            out.push_str("</text>");
        }
    }
    out.push_str("</svg>");
    out
}

/// Render a [`WebFrame`] as a minimal HTML5 document wrapping the SVG
/// produced by [`render_frame_svg`]. See the module whitespace
/// contract.
#[must_use]
pub fn render_page(frame: &WebFrame) -> String {
    let svg = render_frame_svg(frame);
    let mut out = String::with_capacity(svg.len() + 128);
    out.push_str(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\">\
         <title>reovim-web — canonical frame</title></head><body>",
    );
    out.push_str(&svg);
    out.push_str("</body></html>");
    out
}

/// Escape the five XML-significant characters when inlining a glyph
/// into an SVG `<text>` element. Most canonical-frame glyphs are ASCII
/// letters or `─` (U+2500), so the fast path is a no-op push.
fn append_xml_char(out: &mut String, ch: char) {
    match ch {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        '\'' => out.push_str("&apos;"),
        other => out.push(other),
    }
}
