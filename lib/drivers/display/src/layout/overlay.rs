//! Overlay layer trait for pop-ups, menus, tooltips.
//!
//! The overlay layer manages temporary UI elements that appear above
//! all other windows, such as autocomplete popups, hover documentation,
//! and command palettes.
//!
//! # Future Work
//!
//! This trait will be implemented in Phase 3 (#399).

use {
    super::layer::{OverlayConstraints, WindowPlacement, ZOrder},
    crate::{Rect, WindowId},
};

/// Overlay window state.
#[derive(Debug, Clone)]
pub struct OverlayWindow {
    /// Window identifier.
    pub id: WindowId,
    /// Positioning constraints.
    pub constraints: OverlayConstraints,
    /// Computed screen bounds.
    pub computed_bounds: Rect,
    /// Z-order within overlay zone.
    pub z_order: ZOrder,
}

/// Overlay layer manages pop-ups and temporary UI elements.
///
/// Overlays are positioned using anchor constraints and appear above
/// all other windows within their layer.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow the overlay layer
/// to be shared across async tasks.
pub trait OverlayLayer: Send + Sync {
    /// Get all overlays in z-order.
    ///
    /// # Arguments
    ///
    /// * `screen` - Screen bounds for computing overlay positions
    ///
    /// # Returns
    ///
    /// Window placements sorted by z-order (lowest first).
    fn arrange(&self, screen: Rect) -> Vec<WindowPlacement>;

    /// Show an overlay with constraints.
    ///
    /// # Arguments
    ///
    /// * `id` - Window identifier
    /// * `constraints` - Positioning constraints
    fn show(&mut self, id: WindowId, constraints: OverlayConstraints);

    /// Hide (remove) an overlay.
    ///
    /// # Returns
    ///
    /// `true` if the overlay existed and was hidden.
    fn hide(&mut self, id: WindowId) -> bool;

    /// Update overlay size (content changed).
    fn update_size(&mut self, id: WindowId, width: u16, height: u16);

    /// Check if overlay is visible.
    fn is_visible(&self, id: WindowId) -> bool;

    /// Get all visible overlay IDs.
    fn visible_overlays(&self) -> Vec<WindowId>;

    /// Hide all overlays.
    fn hide_all(&mut self);
}
