//! Trait-based overlay system for unified overlay window handling
//!
//! This module provides shared traits for overlay windows (completion, telescope,
//! settings menu, which-key) while allowing each feature to own its positioning
//! and sizing heuristics.
//!
//! ## Plugin-Based Overlays
//!
//! For plugin-based overlays, use the [`OverlayRenderer`] trait and [`OverlayRegistry`]:
//! - [`OverlayRenderer`]: Renders directly to `FrameBuffer` for diff-based rendering
//! - [`OverlayRegistry`]: Stores registered overlays, sorted by z-order
//!
//! Plugins register overlays via `PluginContext::register_overlay()`.

mod compositor;
mod geometry;
mod registry;
mod render;
mod selectable;

pub use {
    compositor::OverlayCompositor,
    geometry::{OverlayBounds, OverlayGeometry},
    registry::{OverlayRegistry, OverlayRenderer},
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
    /// - Settings: 400
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
