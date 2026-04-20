#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Render codec crate — domain-neutral `CommandBuffer` types for rendering.
//!
//! This crate lives in `shared/` because it defines the encoding format for
//! render commands. Domain drivers produce `CommandBuffer` payloads; platform
//! clients decode and execute them.
//!
//! # Render Surfaces
//!
//! | Surface | Module | Use Case |
//! |---------|--------|----------|
//! | `cell_grid` | Terminal/TUI | Character cells on a 2D grid |
//! | `pixel` | GUI/Web | Pixel-addressed 2D rendering |
//! | `vr` | VR/AR | Stereoscopic 3D rendering |
//! | `volumetric` | Holographic | Volumetric 3D rendering |
//!
//! Each surface defines a `CommandBuffer` that encodes render commands as
//! opaque bytes. Platform clients decode the buffer using the corresponding
//! module's decoder.

pub mod cell_grid;
pub mod pixel;
pub mod volumetric;
pub mod vr;

/// Surface kind identifier for render command routing.
///
/// Used by clients to determine which decoder to use for a `CommandBuffer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SurfaceKind {
    /// Character-cell grid (terminals, TUI).
    CellGrid = 1,
    /// Pixel-addressed 2D surface (GUI, web canvas).
    Pixel = 2,
    /// Stereoscopic VR/AR surface.
    Vr = 3,
    /// Volumetric 3D surface (holographic displays).
    Volumetric = 4,
}

impl SurfaceKind {
    /// Convert from raw byte, returning `None` for unknown values.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::CellGrid),
            2 => Some(Self::Pixel),
            3 => Some(Self::Vr),
            4 => Some(Self::Volumetric),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
