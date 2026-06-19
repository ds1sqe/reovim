//! Pure FDT v17 (DTB) reader over a `&[u8]` slice.
//!
//! Parses the Flattened Device Tree binary format as described in the
//! *Devicetree Specification* release v0.4, §5 (DTB encoding). The reader is:
//!
//! - `no_std`, zero-allocation — all traversal is in-place over the caller's
//!   byte slice.
//! - Panic-free on malformed input — every indexed access uses checked slice
//!   operations; out-of-range positions yield a typed `FdtError`.
//! - Big-endian aware — FDT is a big-endian format even on little-endian hosts.
//! - Consumer-agnostic — the Phase 2 `enumerate.rs` module will call this
//!   interface; nothing here assumes the `BCM2711` or any specific `SoC`.
//!
//! ## Cell-size tracking
//!
//! The FDT spec (§2.3) says a node's `reg` is decoded using the `#address-cells`
//! and `#size-cells` declared on its *parent*. The reader tracks a `CellSizes`
//! pair per level: when `ChildIter` yields a child `Node` it carries the
//! parent's declared values as `parent_cells`; the child's own declared values
//! become `parent_cells` for the grandchildren. Spec defaults when no ancestor
//! declares them: address-cells = 2, size-cells = 1.
//!
//! ## Lifetimes
//!
//! [`Node`] and [`ChildIter`] borrow the DTB *data* (the struct and strings
//! blocks, both `&'a [u8]`), not the [`Fdt`] view. So a `Node` outlives the
//! `&Fdt` borrow it was produced from — only the underlying byte slice must
//! stay alive, exactly as for any other view into `&'a [u8]`.

use core::str;

// ── FDT struct-block token codes (§5.4.2) ────────────────────────────────────

const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_NOP: u32 = 4;
const FDT_END: u32 = 9;

// ── FDT header field offsets (§5.2) ──────────────────────────────────────────

const HDR_MAGIC: usize = 0x00;
const HDR_TOTALSIZE: usize = 0x04;
const HDR_OFF_DT_STRUCT: usize = 0x08;
const HDR_OFF_DT_STRINGS: usize = 0x0c;
#[cfg(feature = "selftest")]
const HDR_VERSION: usize = 0x14;
const HDR_LAST_COMP_VERSION: usize = 0x18;
const HDR_SIZE_DT_STRINGS: usize = 0x20;
const HDR_SIZE_DT_STRUCT: usize = 0x24;

const FDT_MAGIC: u32 = 0xd00d_feed;
/// Maximum `last_comp_version` this reader supports (FDT v17 is compat 16).
const MAX_COMPAT_VERSION: u32 = 16;
/// DTB header is always 40 bytes per §5.2.
const HEADER_LEN: usize = 40;

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by the FDT reader.
///
/// All variants are `Copy` and allocation-free; callers can match exhaustively
/// without pulling in `alloc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdtError {
    /// The first four bytes are not `0xd00dfeed`.
    BadMagic,
    /// `last_comp_version` exceeds what this reader implements (> 16).
    UnsupportedVersion,
    /// The input slice is shorter than the declared `totalsize`, or shorter
    /// than the minimum valid header length (40 bytes).
    Truncated,
    /// A struct-block or strings-block offset/size points outside the input
    /// slice.
    OutOfBounds,
    /// The struct block contains an unexpected token or a node name / property
    /// value that cannot be decoded (e.g. non-UTF-8 node name, unterminated
    /// string, property length overflow).
    Malformed,
}

// ── Big-endian helpers ────────────────────────────────────────────────────────

/// Reads a big-endian `u32` from `buf[offset..]`; returns `None` on underrun.
#[inline]
fn read_be_u32(buf: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let bytes: [u8; 4] = buf.get(offset..end)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

/// Reads N 32-bit big-endian cells from `buf[offset..]`, assembling them into
/// a `u64`. `n_cells` must be 1 or 2; other values return `None`.
///
/// Used for `reg` address and size fields whose width is determined by
/// `#address-cells` / `#size-cells`.
#[inline]
fn read_cells_be(buf: &[u8], offset: usize, n_cells: usize) -> Option<u64> {
    match n_cells {
        1 => Some(u64::from(read_be_u32(buf, offset)?)),
        2 => {
            let hi = u64::from(read_be_u32(buf, offset)?);
            let lo = u64::from(read_be_u32(buf, offset.checked_add(4)?)?);
            Some((hi << 32) | lo)
        }
        // The spec allows up to 4 cells but a 64-bit host can only represent 2.
        // Anything higher is exotic and out of scope for static enumeration.
        _ => None,
    }
}

/// Rounds `n` up to the next 4-byte boundary (or `n` itself if already aligned).
#[inline]
const fn align4(n: usize) -> usize {
    (n + 3) & !3
}

/// Resolves a property-name offset into the strings block, returning the
/// NUL-terminated string as `&str`. Returns `None` on out-of-bounds or invalid
/// UTF-8.
fn string_at(strings: &[u8], nameoff: usize) -> Option<&str> {
    let from = strings.get(nameoff..)?;
    let nul = from.iter().position(|&b| b == 0)?;
    str::from_utf8(&from[..nul]).ok()
}

/// Scans properties in the struct block `sb` starting at `pos`, extracting
/// `#address-cells` and `#size-cells`. Returns `(CellSizes, pos_after_props)`.
///
/// Stops (without consuming) at the first non-NOP, non-PROP token. `strings` is
/// the strings block used to resolve each property name.
fn scan_node_cells(
    sb: &[u8],
    strings: &[u8],
    start: usize,
) -> Result<(CellSizes, usize), FdtError> {
    let mut addr = CellSizes::DEFAULT.address_cells;
    let mut size = CellSizes::DEFAULT.size_cells;
    let mut pos = start;

    loop {
        let tok = read_be_u32(sb, pos).ok_or(FdtError::OutOfBounds)?;
        match tok {
            FDT_NOP => {
                pos += 4;
            }
            FDT_PROP => {
                pos += 4;
                let val_len = read_be_u32(sb, pos).ok_or(FdtError::OutOfBounds)? as usize;
                let nameoff = read_be_u32(sb, pos + 4).ok_or(FdtError::OutOfBounds)? as usize;
                let val_start = pos + 8;
                let val_end = val_start
                    .checked_add(val_len)
                    .ok_or(FdtError::OutOfBounds)?;
                if val_end > sb.len() {
                    return Err(FdtError::OutOfBounds);
                }
                if let Some(name) = string_at(strings, nameoff) {
                    if name == "#address-cells" && val_len == 4 {
                        addr = read_be_u32(sb, val_start).ok_or(FdtError::OutOfBounds)?;
                    } else if name == "#size-cells" && val_len == 4 {
                        size = read_be_u32(sb, val_start).ok_or(FdtError::OutOfBounds)?;
                    }
                }
                pos = align4(val_end);
            }
            // BEGIN_NODE, END_NODE, or FDT_END: stop here without consuming.
            FDT_BEGIN_NODE | FDT_END_NODE | FDT_END => break,
            _ => return Err(FdtError::Malformed),
        }
    }

    Ok((
        CellSizes {
            address_cells: addr,
            size_cells: size,
        },
        pos,
    ))
}

// ── CellSizes ─────────────────────────────────────────────────────────────────

/// The `#address-cells` / `#size-cells` pair in force at a tree level.
///
/// A child node's `reg` is decoded using its *parent's* `CellSizes`. The
/// parent's own declared values (if any) become the `CellSizes` for
/// grandchildren.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSizes {
    /// Number of 32-bit cells per `reg` address component.
    pub address_cells: u32,
    /// Number of 32-bit cells per `reg` size component.
    pub size_cells: u32,
}

impl CellSizes {
    /// Spec defaults (§2.3.5) when no ancestor declares the cell counts.
    const DEFAULT: Self = Self {
        address_cells: 2,
        size_cells: 1,
    };
}

// ── Fdt ───────────────────────────────────────────────────────────────────────

/// A validated view over a raw DTB byte slice.
///
/// Construct with [`Fdt::parse`], which checks the header magic, version, and
/// sub-block bounds. All subsequent operations are bounds-checked and
/// panic-free.
pub struct Fdt<'a> {
    /// Full DTB slice, clipped to `totalsize`.
    raw: &'a [u8],
    /// Byte offset of the struct block within `raw`.
    struct_off: usize,
    /// Length in bytes of the struct block.
    struct_len: usize,
    /// Byte offset of the strings block within `raw`.
    strings_off: usize,
    /// Length in bytes of the strings block.
    strings_len: usize,
}

impl<'a> Fdt<'a> {
    /// Parses the DTB header and returns a validated [`Fdt`].
    ///
    /// Errors:
    /// - `Truncated` — `data` shorter than 40 bytes or shorter than `totalsize`.
    /// - `BadMagic` — wrong magic word.
    /// - `UnsupportedVersion` — `last_comp_version > 16`.
    /// - `OutOfBounds` — struct or strings block extends past `totalsize`.
    pub fn parse(data: &'a [u8]) -> Result<Self, FdtError> {
        if data.len() < HEADER_LEN {
            return Err(FdtError::Truncated);
        }

        let magic = read_be_u32(data, HDR_MAGIC).ok_or(FdtError::Truncated)?;
        if magic != FDT_MAGIC {
            return Err(FdtError::BadMagic);
        }

        let last_compat = read_be_u32(data, HDR_LAST_COMP_VERSION).ok_or(FdtError::Truncated)?;
        if last_compat > MAX_COMPAT_VERSION {
            return Err(FdtError::UnsupportedVersion);
        }

        let totalsize = read_be_u32(data, HDR_TOTALSIZE).ok_or(FdtError::Truncated)? as usize;
        if totalsize > data.len() {
            return Err(FdtError::Truncated);
        }
        let raw = &data[..totalsize];

        let struct_off = read_be_u32(raw, HDR_OFF_DT_STRUCT).ok_or(FdtError::OutOfBounds)? as usize;
        let struct_len =
            read_be_u32(raw, HDR_SIZE_DT_STRUCT).ok_or(FdtError::OutOfBounds)? as usize;
        let strings_off =
            read_be_u32(raw, HDR_OFF_DT_STRINGS).ok_or(FdtError::OutOfBounds)? as usize;
        let strings_len =
            read_be_u32(raw, HDR_SIZE_DT_STRINGS).ok_or(FdtError::OutOfBounds)? as usize;

        struct_off
            .checked_add(struct_len)
            .filter(|&end| end <= raw.len())
            .ok_or(FdtError::OutOfBounds)?;
        strings_off
            .checked_add(strings_len)
            .filter(|&end| end <= raw.len())
            .ok_or(FdtError::OutOfBounds)?;

        Ok(Self {
            raw,
            struct_off,
            struct_len,
            strings_off,
            strings_len,
        })
    }

    /// The DTB `version` field. Test-support introspection (the enumerator does
    /// not branch on the version; `parse` already gates `last_comp_version`).
    #[cfg(feature = "selftest")]
    pub fn version(&self) -> u32 {
        // Header is already validated; the read cannot fail.
        read_be_u32(self.raw, HDR_VERSION).unwrap_or(0)
    }

    /// The struct-block sub-slice (carries the data lifetime `'a`).
    #[inline]
    fn struct_block(&self) -> &'a [u8] {
        &self.raw[self.struct_off..self.struct_off + self.struct_len]
    }

    /// The strings-block sub-slice / property-name pool (carries lifetime `'a`).
    #[inline]
    fn strings_block(&self) -> &'a [u8] {
        &self.raw[self.strings_off..self.strings_off + self.strings_len]
    }

    /// Returns the root `/` node.
    ///
    /// The struct block opens with `FDT_BEGIN_NODE` for the root. `root()`
    /// advances past the opener, scans the root's properties to collect its
    /// declared `#address-cells` / `#size-cells`, and returns a `Node` whose
    /// `children()` iterator yields the root's direct children.
    pub fn root(&self) -> Result<Node<'a>, FdtError> {
        let sb = self.struct_block();
        let strings = self.strings_block();

        // Expect FDT_BEGIN_NODE at position 0.
        let tok = read_be_u32(sb, 0).ok_or(FdtError::Truncated)?;
        if tok != FDT_BEGIN_NODE {
            return Err(FdtError::Malformed);
        }

        // Skip the root node name (NUL-terminated; typically empty string "").
        let name_start = 4usize;
        let name_end = sb
            .get(name_start..)
            .and_then(|s| s.iter().position(|&b| b == 0))
            .ok_or(FdtError::Malformed)?
            + name_start;
        let props_start = align4(name_end + 1); // +1 for the NUL byte

        // Scan root properties to pick up #address-cells / #size-cells.
        let (root_cells, body_start) = scan_node_cells(sb, strings, props_start)?;

        Ok(Node {
            sb,
            strings,
            props_off: props_start,
            body_off: body_start,
            parent_cells: root_cells, // root has no parent; use its own cells
            #[cfg(feature = "selftest")]
            name: "/",
            cells: root_cells,
        })
    }
}

// ── Node ──────────────────────────────────────────────────────────────────────

/// A single FDT node with access to its properties and direct children.
///
/// All offsets are relative to the struct block (`sb`) it borrows; `strings` is
/// the property-name pool. Both slices carry the DTB data lifetime `'a`.
pub struct Node<'a> {
    /// The struct block of the owning DTB.
    sb: &'a [u8],
    /// The strings block (property-name pool) of the owning DTB.
    strings: &'a [u8],
    /// Offset within the struct block where this node's property list begins
    /// (just after the NUL-terminated node name). Stored so `prop_bytes` can
    /// re-walk properties without re-scanning the whole tree.
    props_off: usize,
    /// Offset within the struct block just past this node's own properties
    /// (at the first child `FDT_BEGIN_NODE` or the closing `FDT_END_NODE`).
    body_off: usize,
    /// Cell sizes declared by the *parent* of this node, used to decode this
    /// node's `reg` property.
    parent_cells: CellSizes,
    /// This node's display name (the NUL-terminated name from `FDT_BEGIN_NODE`).
    /// Test-support: the enumerator classifies by `compatible`, not by name, so
    /// the name is only carried for the selftests' navigation/assertions.
    #[cfg(feature = "selftest")]
    name: &'a str,
    /// Cell sizes this node declares (become `parent_cells` for its children).
    cells: CellSizes,
}

impl<'a> Node<'a> {
    /// The node's name as it appears in the struct block (e.g. `"serial@7e201000"`).
    /// Test-support introspection (see the `name` field).
    #[cfg(feature = "selftest")]
    pub fn name(&self) -> &'a str {
        self.name
    }

    /// The first NUL-separated string from the `compatible` property, or `None`
    /// if the node has no `compatible` or its value is not valid UTF-8.
    pub fn first_compatible(&self) -> Option<&'a str> {
        let bytes = self.prop_bytes("compatible")?;
        let nul = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        str::from_utf8(&bytes[..nul]).ok()
    }

    /// Decodes the first `reg` entry using the *parent's* cell sizes.
    ///
    /// Returns `(base_address, length)` as `u64`, or `None` if the node has no
    /// `reg` property or the byte count is insufficient for the declared cell
    /// widths.
    pub fn reg(&self) -> Option<(u64, u64)> {
        let bytes = self.prop_bytes("reg")?;
        let ac = self.parent_cells.address_cells as usize;
        let sc = self.parent_cells.size_cells as usize;
        let entry_bytes = ac.checked_add(sc)?.checked_mul(4)?;
        if bytes.len() < entry_bytes {
            return None;
        }
        let base = read_cells_be(bytes, 0, ac)?;
        let len = read_cells_be(bytes, ac * 4, sc)?;
        Some((base, len))
    }

    /// The raw bytes of the named property, or `None` if not present.
    pub fn prop_bytes(&self, name: &str) -> Option<&'a [u8]> {
        let sb = self.sb;
        let mut pos = self.props_off;
        loop {
            let tok = read_be_u32(sb, pos)?;
            pos += 4;
            match tok {
                FDT_NOP => {}
                FDT_PROP => {
                    let val_len = read_be_u32(sb, pos)? as usize;
                    let nameoff = read_be_u32(sb, pos + 4)? as usize;
                    let val_start = pos + 8;
                    let val_end = val_start.checked_add(val_len)?;
                    let val = sb.get(val_start..val_end)?;
                    if string_at(self.strings, nameoff) == Some(name) {
                        return Some(val);
                    }
                    pos = align4(val_end);
                }
                // Any other token (BEGIN_NODE, END_NODE, END) ends properties.
                _ => return None,
            }
        }
    }

    /// The first 32-bit cell of the `interrupts` property, or `None` if absent
    /// or the value is too short.
    ///
    /// GIC entries are tri-cell (`type`, `number`, `flags`); this returns only
    /// the first cell as a quick IRQ hint. Phase 2 may expose a fuller decode.
    pub fn first_interrupt_cell(&self) -> Option<u32> {
        let bytes = self.prop_bytes("interrupts")?;
        read_be_u32(bytes, 0)
    }

    /// Returns an iterator over this node's direct children.
    ///
    /// Each yielded [`Node`] carries the current node's declared `CellSizes`
    /// as its `parent_cells`, so `node.reg()` decodes correctly at every level.
    pub const fn children(&self) -> ChildIter<'a> {
        ChildIter {
            sb: self.sb,
            strings: self.strings,
            pos: self.body_off,
            parent_cells: self.cells,
            depth: 0,
        }
    }

    /// Finds a direct child by name or `name@addr` prefix (first match).
    ///
    /// `find_child("soc")` matches both `"soc"` and `"soc@7e000000"`.
    /// Test-support navigation (the enumerator walks all children, not by name).
    #[cfg(feature = "selftest")]
    pub fn find_child(&self, name: &str) -> Option<Node<'a>> {
        self.children().find(|n| node_name_matches(n.name(), name))
    }

    /// Finds a direct child by exact full name (e.g. `"serial@7e201000"`).
    /// Test-support navigation (see [`find_child`](Node::find_child)).
    #[cfg(feature = "selftest")]
    pub fn find_child_exact(&self, name: &str) -> Option<Node<'a>> {
        self.children().find(|n| n.name() == name)
    }
}

/// Returns `true` when `node_name` equals `target` or its `name@addr`
/// unit-address prefix equals `target`.
#[cfg(feature = "selftest")]
fn node_name_matches(node_name: &str, target: &str) -> bool {
    node_name == target
        || node_name
            .split_once('@')
            .is_some_and(|(prefix, _)| prefix == target)
}

// ── ChildIter ─────────────────────────────────────────────────────────────────

/// Iterator over the direct children of a node.
///
/// Yields each direct child as a [`Node`] and skips any deeper subtrees (their
/// `FDT_BEGIN_NODE`/`FDT_END_NODE` pairs are consumed but not yielded).
/// Terminates on `FDT_END_NODE` (closing the parent) or `FDT_END`.
pub struct ChildIter<'a> {
    /// The struct block of the owning DTB.
    sb: &'a [u8],
    /// The strings block (property-name pool) of the owning DTB.
    strings: &'a [u8],
    /// Current read position within `sb`.
    pos: usize,
    /// The parent node's declared cell sizes, passed into each yielded child as
    /// `parent_cells` so the child's `reg()` decodes correctly.
    parent_cells: CellSizes,
    /// Tracks nesting depth while skipping subtrees. Zero means we're at the
    /// direct-child level; positive means we're inside a deeper subtree.
    depth: usize,
}

impl<'a> Iterator for ChildIter<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let tok = read_be_u32(self.sb, self.pos)?;
            self.pos += 4;
            match tok {
                FDT_NOP => {}

                FDT_BEGIN_NODE => {
                    // Read the NUL-terminated node name. The position must
                    // advance past it always; the name string itself is only
                    // captured for the selftests' navigation/assertions.
                    let name_start = self.pos;
                    let nul = self.sb.get(name_start..)?.iter().position(|&b| b == 0)?;
                    #[cfg(feature = "selftest")]
                    let name = {
                        let name_bytes = self.sb.get(name_start..name_start + nul)?;
                        // FDT node names are ASCII; accept any valid UTF-8 and
                        // fall back to a sentinel on malformed bytes rather than
                        // aborting the whole walk.
                        str::from_utf8(name_bytes).unwrap_or("<bad-utf8>")
                    };
                    let after_nul = name_start + nul + 1;
                    let props_start = align4(after_nul);
                    self.pos = props_start;

                    if self.depth > 0 {
                        // Inside a deeper subtree — skip this node's props and
                        // continue; its END_NODE will decrement depth.
                        self.depth += 1;
                        self.skip_props();
                        continue;
                    }

                    // Direct child: scan its properties for cell declarations.
                    let (child_cells, body_start) =
                        scan_node_cells(self.sb, self.strings, props_start).ok()?;
                    self.pos = body_start;

                    // Enter this child so its END_NODE is consumed by later
                    // next() calls; depth becomes 1 while inside its body.
                    self.depth += 1;

                    return Some(Node {
                        sb: self.sb,
                        strings: self.strings,
                        props_off: props_start,
                        body_off: body_start,
                        parent_cells: self.parent_cells,
                        #[cfg(feature = "selftest")]
                        name,
                        cells: child_cells,
                    });
                }

                FDT_END_NODE => {
                    if self.depth == 0 {
                        // Closing brace for the node we're iterating; done.
                        return None;
                    }
                    self.depth -= 1;
                }

                FDT_PROP => {
                    // A property at the current level (can appear when depth > 0
                    // while skipping a sub-subtree). Skip it.
                    let val_len = read_be_u32(self.sb, self.pos)? as usize;
                    let val_end = self.pos.checked_add(8)?.checked_add(val_len)?;
                    self.pos = align4(val_end);
                }

                // FDT_END or any unknown token — stop.
                _ => return None,
            }
        }
    }
}

impl ChildIter<'_> {
    /// Skips past all properties at the current position (used when entering a
    /// subtree we're not yielding, so we land at the first child token).
    fn skip_props(&mut self) {
        loop {
            let Some(tok) = read_be_u32(self.sb, self.pos) else {
                return;
            };
            match tok {
                FDT_NOP => {
                    self.pos += 4;
                }
                FDT_PROP => {
                    self.pos += 4;
                    let Some(val_len) = read_be_u32(self.sb, self.pos) else {
                        return;
                    };
                    let Some(val_end) = self
                        .pos
                        .checked_add(8)
                        .and_then(|p| p.checked_add(val_len as usize))
                    else {
                        return;
                    };
                    self.pos = align4(val_end);
                }
                // Non-prop token: leave pos here for the outer loop.
                _ => return,
            }
        }
    }
}

// ── Selftest declaration (L12 layout) ────────────────────────────────────────
// Tests live in the sibling file `reader_tests.rs`, declared as a `#[path]`
// child so `super::` reaches the private reader types and helpers. The file is
// only compiled when this module is (the lib.rs target gate), so the inner
// declaration needs only the `selftest` feature gate.
#[cfg(feature = "selftest")]
#[path = "reader_tests.rs"]
mod tests;
