//! Core session types extracted from the session driver.
//!
//! This module provides:
//! - [`ClientId`] — unique client connection identifier
//! - [`Viewport`] — visible area of a buffer in a window
//! - [`CursorSnapshot`] — per-client opaque cursor identity snapshot for bridge tick consumption
//! - [`KeySequence`] — pending key sequence accumulator

use crate::SessionExtension;

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

/// Viewport for a client session.
///
/// Represents the visible area of a buffer in a window.
/// Tracks both vertical and horizontal scroll positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
    /// Vertical scroll offset (first visible line, 0-indexed).
    pub scroll_top: usize,
    /// Horizontal scroll offset (first visible column, 0-indexed).
    pub scroll_left: usize,
}

impl Viewport {
    /// Create a new viewport.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            scroll_top: 0,
            scroll_left: 0,
        }
    }

    /// Create a default viewport (80x24).
    #[must_use]
    pub const fn default_size() -> Self {
        Self::new(80, 24)
    }

    /// Get the last visible line index.
    #[must_use]
    pub const fn last_visible_line(&self) -> usize {
        self.scroll_top + self.height as usize - 1
    }

    /// Get the last visible column index.
    #[must_use]
    pub const fn last_visible_column(&self) -> usize {
        self.scroll_left + self.width as usize - 1
    }

    /// Check if a line is visible.
    #[must_use]
    pub const fn is_line_visible(&self, line: usize) -> bool {
        line >= self.scroll_top && line <= self.last_visible_line()
    }

    /// Check if a column is visible.
    #[must_use]
    pub const fn is_column_visible(&self, column: usize) -> bool {
        column >= self.scroll_left && column <= self.last_visible_column()
    }

    /// Check if a position (line, column) is visible.
    #[must_use]
    pub const fn is_position_visible(&self, line: usize, column: usize) -> bool {
        self.is_line_visible(line) && self.is_column_visible(column)
    }

    /// Adjust `scroll_top` so the cursor line is visible.
    ///
    /// Returns `true` if `scroll_top` was changed.
    pub const fn ensure_cursor_visible(&mut self, cursor_line: usize) -> bool {
        if self.height == 0 {
            return false;
        }
        let old = self.scroll_top;
        let h = self.height as usize;
        if cursor_line < self.scroll_top {
            self.scroll_top = cursor_line;
        } else if cursor_line >= self.scroll_top + h {
            self.scroll_top = cursor_line - h + 1;
        }
        self.scroll_top != old
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::default_size()
    }
}

/// Pending key sequence.
///
/// Accumulates keys that haven't been resolved yet (e.g., `d` waiting for motion).
#[derive(Debug, Clone, Default)]
pub struct KeySequence {
    /// Accumulated key representations.
    keys: Vec<String>,
}

impl KeySequence {
    /// Create an empty sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// Add a key to the sequence.
    pub fn push(&mut self, key: String) {
        self.keys.push(key);
    }

    /// Clear the sequence.
    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// Check if empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Get the keys.
    #[must_use]
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// Get the sequence as a single string.
    #[must_use]
    pub fn as_string(&self) -> String {
        self.keys.join("")
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
