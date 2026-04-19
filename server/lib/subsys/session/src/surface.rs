//! Opaque surface descriptor for domain-neutral rendering.
//!
//! Each domain encodes its rendering surface as a kind + body pair. The server
//! routes descriptors to clients without interpretation.

/// Opaque surface descriptor for domain-neutral rendering.
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
#[path = "surface_tests.rs"]
mod tests;
