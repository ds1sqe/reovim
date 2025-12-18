//! Trait-based overlay system for unified overlay window handling
//!
//! This module provides shared traits for overlay windows (completion, telescope,
//! settings menu, which-key) while allowing each feature to own its positioning
//! and sizing heuristics.

mod compositor;
mod geometry;
mod render;
mod selectable;

pub use {
    compositor::OverlayCompositor,
    geometry::{OverlayBounds, OverlayGeometry},
    render::OverlayRender,
    selectable::{Scrollable, Selectable},
};

/// Combined trait for full overlay functionality
///
/// Overlays implementing this trait can be managed by the `OverlayCompositor`
/// for z-order rendering and input handling.
pub trait Overlay: OverlayGeometry + OverlayRender {
    /// Unique identifier for this overlay type
    fn overlay_id(&self) -> &'static str;

    /// Z-order for rendering (higher = on top)
    ///
    /// Default z-orders:
    /// - Completion: 100
    /// - `WhichKey`: 200
    /// - Telescope: 300
    /// - `SettingsMenu`: 400
    fn z_order(&self) -> u16;

    /// Whether this overlay captures input when visible
    ///
    /// When true, the overlay consumes input events.
    /// Default: true
    fn captures_input(&self) -> bool {
        true
    }

    /// Whether this overlay is currently visible
    fn is_visible(&self) -> bool;
}
