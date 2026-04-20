//! Opaque render descriptor for domain-neutral rendering.
//!
//! Each domain encodes its render target as a `kind` byte pair plus an
//! opaque `body`. The server routes descriptors to clients without
//! interpretation. Per-platform interpretation lives in
//! `ext/render-codec/<variant>/` codec modules.

/// Opaque render descriptor.
///
/// Kind values 0x0001–0x00FF are reserved for in-repo well-known codecs
/// (see module constants below). 0x0100–0xFFFF is available to external
/// codec crates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderDescriptor {
    /// Render kind identifier.
    pub kind: u16,
    /// Opaque body bytes — kind-specific encoding.
    pub body: Vec<u8>,
}

impl RenderDescriptor {
    /// Create a new render descriptor.
    #[must_use]
    pub const fn new(kind: u16, body: Vec<u8>) -> Self {
        Self { kind, body }
    }

    /// Return the render kind identifier.
    #[must_use]
    pub const fn kind(&self) -> u16 {
        self.kind
    }

    /// Return the opaque body bytes.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

// ── Well-known kinds (in-repo allocation) ───────────────────────────────────
//
// Constants are defined only for codecs that ship as modules in this repo.
// In-repo codecs use kind values in 0x0001–0x00FF; external codec crates
// use 0x0100–0xFFFF. When a new in-repo codec ships, its crate adds its
// own `KIND_*` constant here.

/// `CellGrid` — terminal/TUI render target (styled cells + cursor overlay).
pub const KIND_CELL_GRID: u16 = 0x0001;

// ── Header accessors ────────────────────────────────────────────────────────

/// Return the kind of a descriptor (alias for `descriptor.kind`).
#[must_use]
pub const fn render_kind(descriptor: &RenderDescriptor) -> u16 {
    descriptor.kind
}

/// Return a reference to the body bytes (alias for `descriptor.body()`).
#[must_use]
pub fn render_body(descriptor: &RenderDescriptor) -> &[u8] {
    descriptor.body()
}
