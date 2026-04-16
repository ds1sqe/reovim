//! Stereoscopic VR/AR render surface.
//!
//! Encodes render commands for stereoscopic 3D rendering (VR headsets, AR overlays).

/// Opaque command buffer for VR/AR rendering.
///
/// Domain drivers produce this buffer; VR/AR clients decode and execute it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBuffer {
    /// Encoded render commands.
    data: Vec<u8>,
}

impl CommandBuffer {
    /// Create a new empty command buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Create a command buffer from raw encoded data.
    #[must_use]
    pub const fn from_raw(data: Vec<u8>) -> Self {
        Self { data }
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

impl Default for CommandBuffer {
    fn default() -> Self {
        Self::new()
    }
}
