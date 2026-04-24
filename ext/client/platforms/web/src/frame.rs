//! Self-contained cell-grid primitives for the web SSR spike.
//!
//! `WebFrame` is intentionally **not** `reovim_ext_client_tui_cap_cell::
//! CellCapability`: CLM v7 forbids a direct
//! `ext/client/platforms/* → ext/client/capabilities/*` edge. The two
//! types are structurally similar (bounded 2D grid of styled cells) and
//! will be unified when Phase E.1 of #753 absorbs client-side
//! rendering into platform-owned pipelines. Until then, the ≤60 LOC
//! duplication in this module is the price of respecting the locked
//! layer model.

/// Colour vocabulary for a single cell.
///
/// `Named` carries a static CSS-legal colour name (e.g. `"red"`,
/// `"slategray"`) which is emitted verbatim in the `fill=` attribute.
/// `Default` means "no explicit fill" — the SVG renderer omits the
/// attribute entirely, letting the browser pick the default foreground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebColor {
    /// 24-bit RGB triple, emitted as `#rrggbb`.
    Rgb(u8, u8, u8),
    /// CSS colour keyword, emitted verbatim.
    Named(&'static str),
    /// Terminal / browser default — no `fill` attribute is emitted.
    Default,
}

/// One styled cell in a [`WebFrame`].
///
/// A Flight-76 cell carries only a glyph and a foreground colour.
/// Background rectangles are out of scope; a future flight that
/// renders them can add a `bg` field at the same time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebCell {
    /// Displayed character.
    pub ch: char,
    /// Foreground colour.
    pub fg: Option<WebColor>,
}

impl Default for WebCell {
    fn default() -> Self {
        Self { ch: ' ', fg: None }
    }
}

/// Bounded 2D grid of [`WebCell`]s, row-major.
///
/// Constructed either via [`WebFrame::new`] + explicit cell writes, or
/// via the [`canonical_frame`] fixture. The crate is deliberately
/// narrow — the spike needs exactly one fixture and one renderer.
#[derive(Debug, Clone)]
pub struct WebFrame {
    width: u16,
    height: u16,
    cells: Vec<WebCell>,
}

impl WebFrame {
    /// Create a `width × height` grid of default cells.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let len = usize::from(width) * usize::from(height);
        Self {
            width,
            height,
            cells: vec![WebCell::default(); len],
        }
    }

    /// Grid width (columns).
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Grid height (rows).
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Read a cell at `(x, y)`, or `None` if out of bounds.
    #[must_use]
    pub fn cell(&self, x: u16, y: u16) -> Option<&WebCell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.cells.get(idx)
    }

    /// Overwrite the cell at `(x, y)`. Out-of-bounds writes are
    /// silently ignored — the fixture builder writes within bounds by
    /// construction, and there is no public caller otherwise.
    pub(crate) fn set_cell(&mut self, x: u16, y: u16, cell: WebCell) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
        if let Some(slot) = self.cells.get_mut(idx) {
            *slot = cell;
        }
    }
}

/// Flight 76's locked canonical frame: a 20×3 ASCII banner.
///
/// ```text
///   HELLO RENDER-CODEC
///   ──────────────────
///   FROM apps/web #753
/// ```
///
/// Each row is exactly 20 cells. Two leading spaces on every row give
/// the banner a small left margin; the SVG renderer skips space cells
/// so they do not appear in the output.
///
/// - Row 0 is the message; all non-space cells carry
///   `fg = Some(WebColor::Named("slategray"))` for a subtle tint that
///   exercises the coloured-fill branch.
/// - Row 1 is a `─` (U+2500) underline, all cells coloured
///   `WebColor::Named("dimgray")`.
/// - Row 2 is plain text in the default colour, exercising the
///   no-`fill` branch of the SVG renderer.
///
/// # Panics
///
/// Would panic if the loop index for any row overflowed `u16` — only
/// reachable if the frame's locked 20-column width is changed to a
/// value > 65 535, which the `frame_tests::canonical_frame_has_expected_dimensions`
/// test forbids.
#[must_use]
pub fn canonical_frame() -> WebFrame {
    let mut f = WebFrame::new(20, 3);
    let msg = b"  HELLO RENDER-CODEC";
    // Row 1: two leading spaces + 18 U+2500 = 20 chars.
    let underline = "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\
                     \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}";
    let footer = b"  FROM apps/web #753";

    for (x, &b) in msg.iter().enumerate() {
        let ch = b as char;
        let fg = if ch == ' ' {
            None
        } else {
            Some(WebColor::Named("slategray"))
        };
        let col = u16::try_from(x).expect("canonical frame is 20 cells wide");
        f.set_cell(col, 0, WebCell { ch, fg });
    }

    for (x, ch) in underline.chars().enumerate() {
        let fg = if ch == ' ' {
            None
        } else {
            Some(WebColor::Named("dimgray"))
        };
        let col = u16::try_from(x).expect("canonical frame is 20 cells wide");
        f.set_cell(col, 1, WebCell { ch, fg });
    }

    for (x, &b) in footer.iter().enumerate() {
        let col = u16::try_from(x).expect("canonical frame is 20 cells wide");
        f.set_cell(
            col,
            2,
            WebCell {
                ch: b as char,
                fg: None,
            },
        );
    }

    f
}
