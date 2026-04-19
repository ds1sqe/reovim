//! Core session types extracted from the session driver.
//!
//! This module provides:
//! - [`ClientId`] — unique client connection identifier
//! - [`CursorSnapshot`] — per-client opaque cursor identity snapshot for bridge tick consumption
//! - [`KeySequence`] — pending key sequence accumulator

use crate::SessionExtension;

pub use reovim_subsys_input_contracts::KeySequence;

/// Unique client connection identifier.
///
/// Each terminal/TUI that connects to the server gets a unique `ClientId`.
/// IDs are monotonically increasing and not reused after disconnect.
///
/// # Semantics
///
/// - **Client**: Individual connection to the server (like tmux clients)
/// - **Session**: Named editing context (defined in runner layer)
/// - Multiple clients can attach to the same session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub usize);

impl ClientId {
    /// Create a new client ID.
    #[must_use]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client-{}", self.0)
    }
}

/// Per-client cursor identity snapshot for bridge tick consumption.
///
/// Updated by the runner after each key event. Bridges read this in
/// `tick()` to detect cursor movement without direct access to the
/// window layout.
///
/// This is a domain-neutral mechanism-level type: any bridge can read it,
/// the runner writes it. The 8-byte opaque body uses the same wire format
/// as [`reovim_subsys_coordination::CursorHeader`] — domain_id (4 bytes,
/// LE) + inner_id (2 bytes, LE) + flags (2 bytes, LE). A sentinel value
/// of all-zeros indicates "no cursor seen yet".
///
/// No text-domain fields (line, col, buffer_id) — the runner encodes its
/// text cursor into the 8-byte body via the coordination codec.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CursorSnapshot(pub [u8; 8]);

impl CursorSnapshot {
    /// The sentinel value meaning "no cursor position recorded yet".
    pub const SENTINEL: Self = Self([0u8; 8]);

    /// Create a snapshot from raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// Return the raw 8-byte representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
}

impl std::fmt::Debug for CursorSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CursorSnapshot").field(&self.0).finish()
    }
}

impl SessionExtension for CursorSnapshot {
    fn create() -> Self {
        Self::SENTINEL
    }
}

/// Opaque surface descriptor for domain-neutral rendering.
///
/// Each domain encodes its rendering surface as a kind + body pair.
/// The server routes descriptors to clients without interpretation.
///
/// # Kinds (by convention)
/// - `KIND_CELL_GRID` (0x0001): CellGrid — text terminal rows × cols of styled cells
/// - `KIND_PIXEL_BUFFER` (0x0002): PixelBuffer — 2D raster
/// - `KIND_VR_SCENE` (0x0003): VR scene
/// - `KIND_VOLUMETRIC` (0x0004): Volumetric
/// - `KIND_NEURAL` (0x0005): Neural
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceDescriptor {
    /// Surface kind identifier.
    pub kind: u16,
    /// Opaque body bytes — kind-specific encoding.
    pub body: Vec<u8>,
}

impl SurfaceDescriptor {
    /// CellGrid — text terminal (rows × cols of styled cells).
    pub const KIND_CELL_GRID: u16 = 0x0001;
    /// PixelBuffer — 2D raster.
    pub const KIND_PIXEL_BUFFER: u16 = 0x0002;
    /// VR scene.
    pub const KIND_VR_SCENE: u16 = 0x0003;
    /// Volumetric.
    pub const KIND_VOLUMETRIC: u16 = 0x0004;
    /// Neural.
    pub const KIND_NEURAL: u16 = 0x0005;

    /// Create a new surface descriptor.
    #[must_use]
    pub fn new(kind: u16, body: Vec<u8>) -> Self {
        Self { kind, body }
    }

    /// Return the surface kind identifier.
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

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
