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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_floating_window_struct() {
        let fw = FloatingWindow {
            id: WindowId::from_raw(1),
            bounds: Rect::new(10, 20, 30, 40),
            z_order: ZOrder::new(5),
        };
        assert_eq!(fw.id, WindowId::from_raw(1));
        assert_eq!(fw.bounds.x, 10);
        assert_eq!(fw.bounds.y, 20);
        assert_eq!(fw.bounds.width, 30);
        assert_eq!(fw.bounds.height, 40);
        assert_eq!(fw.z_order, ZOrder::new(5));
    }

    #[test]
    fn test_floating_window_clone() {
        let fw = FloatingWindow {
            id: WindowId::from_raw(2),
            bounds: Rect::new(0, 0, 80, 24),
            z_order: ZOrder::new(0),
        };
        let cloned = fw.clone();
        assert_eq!(fw.id, cloned.id);
        assert_eq!(fw.bounds, cloned.bounds);
        assert_eq!(fw.z_order, cloned.z_order);
    }

    #[test]
    fn test_floating_window_debug() {
        let fw = FloatingWindow {
            id: WindowId::from_raw(3),
            bounds: Rect::new(0, 0, 10, 10),
            z_order: ZOrder::new(1),
        };
        let debug = format!("{fw:?}");
        assert!(debug.contains("FloatingWindow"));
    }

    #[test]
    fn test_floating_layer_is_object_safe() {
        fn _accepts_ref(_: &dyn FloatingLayer) {}
        fn _accepts_box(_: Box<dyn FloatingLayer>) {}
    }
}
