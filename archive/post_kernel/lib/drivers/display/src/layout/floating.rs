//! Floating layer trait for free-positioned windows.
//!
//! The floating layer manages windows that can be freely positioned
//! and sized on screen, independent of the tiled split tree.
//!
//! # Future Work
//!
//! This trait will be implemented in Phase 2 (#398).

use {
    super::layer::{WindowPlacement, ZOrder},
    crate::{Rect, WindowId},
};

/// Floating window state.
#[derive(Debug, Clone)]
pub struct FloatingWindow {
    /// Window identifier.
    pub id: WindowId,
    /// Window bounds (position and size).
    pub bounds: Rect,
    /// Z-order within float zone.
    pub z_order: ZOrder,
}

/// Floating layer manages freely-positioned windows.
///
/// Floating windows exist above the tiled zone but below overlays.
/// They can be moved and resized freely within the layer bounds.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow the floating layer
/// to be shared across async tasks.
pub trait FloatingLayer: Send + Sync {
    /// Get all floating windows in z-order.
    ///
    /// Returns window placements sorted by z-order (lowest first).
    fn arrange(&self) -> Vec<WindowPlacement>;

    /// Create a new floating window.
    ///
    /// # Arguments
    ///
    /// * `id` - Window identifier
    /// * `bounds` - Initial position and size
    ///
    /// # Returns
    ///
    /// The window ID (same as input for consistency).
    fn create(&mut self, id: WindowId, bounds: Rect) -> WindowId;

    /// Close a floating window.
    ///
    /// # Returns
    ///
    /// `true` if the window existed and was closed.
    fn close(&mut self, window: WindowId) -> bool;

    /// Move window to position.
    fn move_to(&mut self, window: WindowId, x: u16, y: u16);

    /// Resize window.
    fn resize(&mut self, window: WindowId, width: u16, height: u16);

    /// Bring to front (highest z-order).
    fn raise(&mut self, window: WindowId);

    /// Send to back (lowest z-order in floating layer).
    fn lower(&mut self, window: WindowId);

    /// Get all floating window IDs.
    fn windows(&self) -> Vec<WindowId>;

    /// Check if window is in floating layer.
    fn contains(&self, window: WindowId) -> bool;
}
