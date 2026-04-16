//! Cell-grid render surface for terminal/TUI clients.
//!
//! Encodes render commands as operations on a 2D character-cell grid.
//! Each cell holds a character, foreground/background color, and style attributes.

/// Opaque command buffer for cell-grid rendering.
///
/// Domain drivers produce this buffer; TUI clients decode and execute it.
/// The buffer contains a sequence of encoded render commands (set cell,
/// clear region, set cursor, etc.) that operate on a character-cell grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBuffer {
    /// Encoded render commands.
    data: Vec<u8>,
    /// Grid width in cells.
    width: u16,
    /// Grid height in cells.
    height: u16,
}

impl CommandBuffer {
    /// Create a new empty command buffer for the given grid dimensions.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            data: Vec::new(),
            width,
            height,
        }
    }

    /// Create a command buffer from raw encoded data.
    #[must_use]
    pub const fn from_raw(data: Vec<u8>, width: u16, height: u16) -> Self {
        Self {
            data,
            width,
            height,
        }
    }

    /// Get the encoded command data.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Consume the buffer and return the raw data.
    #[must_use]
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    /// Grid width in cells.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Grid height in cells.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Whether the buffer contains any commands.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Number of bytes in the command data.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }
}
