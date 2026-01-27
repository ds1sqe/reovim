//! Overlay layer trait for pop-ups, menus, tooltips.
//!
//! The overlay layer manages temporary UI elements that appear above
//! all other windows, such as autocomplete popups, hover documentation,
//! and command palettes.
//!
//! # Anchor Resolution
//!
//! Overlays use anchor-based positioning. The `arrange()` method receives
//! `window_placements` from the compositor so that anchors like `Cursor`
//! and `Below` can resolve positions relative to other windows.
//!
//! # Implementation
//!
//! See `modules/layout/src/overlayzone.rs` for the policy implementation.

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
    /// * `window_placements` - Existing window placements from tiled/float zones,
    ///   used for anchor resolution (e.g., `Anchor::Cursor` needs to know where
    ///   the referenced window is positioned)
    ///
    /// # Returns
    ///
    /// Window placements sorted by z-order (lowest first).
    fn arrange(&self, screen: Rect, window_placements: &[WindowPlacement]) -> Vec<WindowPlacement>;

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
