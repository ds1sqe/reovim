//! Pixel-addressed render surface for GUI/web clients.
//!
//! Encodes render commands as operations on a 2D pixel canvas.

/// Opaque command buffer for pixel-addressed rendering.
///
/// Domain drivers produce this buffer; GUI/web clients decode and execute it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBuffer {
    /// Encoded render commands.
    data: Vec<u8>,
    /// Canvas width in pixels.
    width: u32,
    /// Canvas height in pixels.
    height: u32,
}

impl CommandBuffer {
    /// Create a new empty command buffer for the given canvas dimensions.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self {
            data: Vec::new(),
            width,
            height,
        }
    }

    /// Create a command buffer from raw encoded data.
    #[must_use]
    pub const fn from_raw(data: Vec<u8>, width: u32, height: u32) -> Self {
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

    /// Canvas width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Canvas height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Whether the buffer contains any commands.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Number of bytes in the command data.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.data.len()
    }
}
