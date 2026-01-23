//! Root compositor and window layer compositor traits.
//!
//! This module defines the compositor architecture for window layout management.
//! The design follows a Wayland-inspired model where the runner is a pure event
//! loop and compositors handle all window geometry concerns.
//!
//! # Architecture
//!
//! ```text
//! RootCompositor (manages multiple layers)
//!     │
//!     ├── Layer 1 ("main", z_base=100)
//!     │   └── WindowLayerCompositor
//!     │       ├── Tiled Zone
//!     │       ├── Float Zone
//!     │       └── Overlay Zone
//!     │
//!     └── Layer 2 ("float-term", z_base=200)
//!         └── WindowLayerCompositor
//!             ├── Tiled Zone
//!             ├── Float Zone
//!             └── Overlay Zone
//! ```
//!
//! # Responsibilities
//!
//! - **`RootCompositor`**: Layer lifecycle, focus routing between layers
//! - **`WindowLayerCompositor`**: Window management within a single layer

use {
    super::layer::{Layer, LayerConfig, LayerId, OverlayConstraints, WindowPlacement, Zone},
    crate::{NavigateDirection, Rect, SplitDirection, WindowId},
};

/// Result of compositing all layers.
///
/// Contains all information needed to render the complete window layout.
#[derive(Debug, Clone)]
pub struct CompositeResult {
    /// All windows in z-order (lowest first).
    pub placements: Vec<WindowPlacement>,
    /// Currently focused window (if any).
    pub focused: Option<WindowId>,
    /// Active layer (receives keyboard input).
    pub active_layer: Option<LayerId>,
    /// Total screen bounds.
    pub screen: Rect,
}

impl CompositeResult {
    /// Create an empty composite result.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::new() is not const
    pub fn empty(screen: Rect) -> Self {
        Self {
            placements: Vec::new(),
            focused: None,
            active_layer: None,
            screen,
        }
    }

    /// Get placement for a specific window.
    #[must_use]
    pub fn get_placement(&self, window: WindowId) -> Option<&WindowPlacement> {
        self.placements.iter().find(|p| p.window_id == window)
    }
}

/// Root compositor manages multiple layers.
///
/// Each layer is a mini-compositor. The root handles:
/// - Layer creation/destruction
/// - Layer stacking (z-order)
/// - Focus routing between layers
/// - Click-through to lower layers
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow compositors to be
/// shared across async tasks.
pub trait RootCompositor: Send + Sync {
    /// Compute final layout for all layers.
    ///
    /// Returns all window placements sorted by z-order (lowest first).
    fn composite(&self, screen: Rect) -> CompositeResult;

    // ========================================================================
    // Layer Management
    // ========================================================================

    /// Create a new layer.
    ///
    /// Returns the ID of the newly created layer.
    fn create_layer(&mut self, config: LayerConfig) -> LayerId;

    /// Remove a layer (closes all windows in it).
    fn remove_layer(&mut self, layer: LayerId);

    /// Get layer by label (for direct focus shortcuts like `<C-w>term`).
    fn layer_by_label(&self, label: &str) -> Option<LayerId>;

    /// Get all layers in z-order (lowest first).
    fn layers(&self) -> Vec<&Layer>;

    /// Set layer visibility.
    fn set_layer_visible(&mut self, layer: LayerId, visible: bool);

    /// Set layer opacity (future: transparency support).
    fn set_layer_opacity(&mut self, layer: LayerId, opacity: f32);

    /// Move layer in z-order (up/down in stack).
    fn reorder_layer(&mut self, layer: LayerId, new_z: u16);

    // ========================================================================
    // Focus Management
    // ========================================================================

    /// Set active layer (receives keyboard input).
    fn set_active_layer(&mut self, layer: LayerId);

    /// Get active layer.
    fn active_layer(&self) -> Option<LayerId>;

    /// Set focus to window (also activates its layer).
    fn set_focus(&mut self, window: WindowId);

    /// Get focused window in active layer.
    fn focused(&self) -> Option<WindowId>;

    /// Focus window by clicking at position (handles click-through).
    ///
    /// Returns the window that was focused, or None if clicking empty space.
    fn focus_at(&mut self, x: u16, y: u16) -> Option<WindowId>;

    // ========================================================================
    // Per-Layer Operations (delegate to WindowLayerCompositor)
    // ========================================================================

    /// Get the compositor for a specific layer.
    ///
    /// Returns a trait object reference (`&dyn WindowLayerCompositor`) to allow
    /// polymorphic access to layer-specific operations. Callers can invoke
    /// any `WindowLayerCompositor` method on the returned reference.
    ///
    /// # Example
    /// ```ignore
    /// let layer = compositor.layer_compositor(id)?;
    /// let windows = layer.navigate_tiled(from, Direction::Right);
    /// ```
    fn layer_compositor(&self, layer: LayerId) -> Option<&dyn WindowLayerCompositor>;

    /// Get mutable compositor for a layer.
    ///
    /// Required for operations that modify layer state (split, close, resize).
    fn layer_compositor_mut(&mut self, layer: LayerId) -> Option<&mut dyn WindowLayerCompositor>;

    /// Window count across all layers.
    fn window_count(&self) -> usize;

    /// Update screen dimensions.
    ///
    /// Called on terminal resize to update cached screen size.
    fn set_screen(&mut self, screen: Rect);

    /// Find which layer contains a window.
    fn layer_of(&self, window: WindowId) -> Option<LayerId>;

    /// Clone this compositor into a new boxed instance.
    ///
    /// This allows extracting a fresh owned compositor from shared storage
    /// (like `ServiceRegistry`'s `Arc<dyn RootCompositor>`). Implementations
    /// that support `Clone` can simply use `Box::new(self.clone())`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Get compositor from ServiceRegistry (Arc)
    /// let arc_compositor = registry.get(&CompositorKey::Root)?;
    ///
    /// // Convert to owned Box for DriverSession
    /// let box_compositor = arc_compositor.boxed_clone();
    /// driver_session.set_compositor(box_compositor);
    /// ```
    fn boxed_clone(&self) -> Box<dyn RootCompositor>;
}

/// Window layer compositor manages windows within a single layer.
///
/// Each layer has three zones: Tiled, Float, Overlay.
/// This is the mini-compositor for one layer.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow compositors to be
/// shared across async tasks.
pub trait WindowLayerCompositor: Send + Sync {
    /// Get layer ID.
    fn id(&self) -> LayerId;

    /// Arrange all windows in this layer.
    ///
    /// Returns window placements sorted by z-order.
    fn arrange(&self, bounds: Rect) -> Vec<WindowPlacement>;

    // ========================================================================
    // Tiled Zone
    // ========================================================================

    /// Add first tiled window.
    ///
    /// Called when the layer has no tiled windows yet.
    fn add_tiled(&mut self) -> WindowId;

    /// Split a tiled window.
    ///
    /// Creates a new window adjacent to `from` in the specified direction.
    /// Returns the new window's ID, or None if split not possible.
    fn split_tiled(&mut self, from: WindowId, direction: SplitDirection) -> Option<WindowId>;

    /// Navigate within tiled zone.
    ///
    /// Returns the window in the specified direction from `from`,
    /// or None if there is no neighbor in that direction.
    fn navigate_tiled(&self, from: WindowId, direction: NavigateDirection) -> Option<WindowId>;

    /// Resize tiled window.
    ///
    /// Moves the edge of `window` in `direction` by `delta` units.
    /// Positive delta expands, negative contracts.
    fn resize_tiled(&mut self, window: WindowId, direction: NavigateDirection, delta: i16);

    /// Close tiled window.
    ///
    /// Returns the window to focus next, or None if this was the last window.
    fn close_tiled(&mut self, window: WindowId) -> Option<WindowId>;

    /// Equalize tiled windows.
    ///
    /// Distributes space equally among all tiled windows.
    fn equalize_tiled(&mut self);

    /// Cycle through tiled windows.
    ///
    /// Returns the next (forward=true) or previous (forward=false) window
    /// in winnr order.
    fn cycle_tiled(&self, from: WindowId, forward: bool) -> Option<WindowId>;

    // ========================================================================
    // Float Zone
    // ========================================================================

    /// Create floating window.
    ///
    /// Creates a new floating window with the specified bounds.
    fn create_float(&mut self, bounds: Rect) -> WindowId;

    /// Move floating window.
    fn move_float(&mut self, window: WindowId, x: u16, y: u16);

    /// Resize floating window.
    fn resize_float(&mut self, window: WindowId, width: u16, height: u16);

    /// Bring to front within float zone.
    fn raise_float(&mut self, window: WindowId);

    /// Close floating window.
    fn close_float(&mut self, window: WindowId);

    /// Toggle between tiled and floating.
    ///
    /// If window is tiled, removes from tiled zone and creates float.
    /// If window is floating, removes from float zone and adds to tiled.
    fn toggle_float(&mut self, window: WindowId);

    // ========================================================================
    // Overlay Zone
    // ========================================================================

    /// Show overlay with constraints.
    ///
    /// Creates a new overlay positioned according to the constraints.
    fn show_overlay(&mut self, constraints: OverlayConstraints) -> WindowId;

    /// Hide overlay.
    fn hide_overlay(&mut self, window: WindowId);

    /// Update overlay size.
    fn resize_overlay(&mut self, window: WindowId, width: u16, height: u16);

    // ========================================================================
    // Focus
    // ========================================================================

    /// Set focused window within this layer.
    fn set_focus(&mut self, window: WindowId);

    /// Get focused window.
    fn focused(&self) -> Option<WindowId>;

    /// Get all windows in a zone.
    fn windows_in_zone(&self, zone: Zone) -> Vec<WindowId>;

    /// Get window's zone.
    fn zone_of(&self, window: WindowId) -> Option<Zone>;
}

/// Error type for compositor operations.
///
/// Maps to vim-compatible error codes for user feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowError {
    /// E444: Cannot close last window.
    CannotCloseLastWindow,
    /// E36: No neighbor in direction.
    NoNeighbor(NavigateDirection),
    /// E94: Not enough room to split.
    NotEnoughRoom,
    /// E36: Cannot resize at edge.
    CannotResizeAtEdge,
    /// Generic window not found.
    WindowNotFound(WindowId),
    /// Layer not found.
    LayerNotFound(LayerId),
}

impl WindowError {
    /// Get vim-compatible error code.
    #[must_use]
    pub const fn vim_code(&self) -> &'static str {
        match self {
            Self::CannotCloseLastWindow => "E444",
            Self::NotEnoughRoom => "E94",
            Self::NoNeighbor(_)
            | Self::CannotResizeAtEdge
            | Self::WindowNotFound(_)
            | Self::LayerNotFound(_) => "E36",
        }
    }

    /// Get user-facing error message.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::CannotCloseLastWindow => "Cannot close last window".to_string(),
            Self::NoNeighbor(dir) => format!("No window in direction: {dir:?}"),
            Self::NotEnoughRoom => "Not enough room to split".to_string(),
            Self::CannotResizeAtEdge => "Cannot resize at edge".to_string(),
            Self::WindowNotFound(id) => format!("Window not found: {}", id.as_usize()),
            Self::LayerNotFound(id) => format!("Layer not found: {}", id.as_u16()),
        }
    }
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.vim_code(), self.message())
    }
}

impl std::error::Error for WindowError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composite_result_empty() {
        let result = CompositeResult::empty(Rect::new(0, 0, 80, 24));
        assert!(result.placements.is_empty());
        assert!(result.focused.is_none());
        assert!(result.active_layer.is_none());
    }

    #[test]
    fn test_window_error_vim_codes() {
        assert_eq!(WindowError::CannotCloseLastWindow.vim_code(), "E444");
        assert_eq!(WindowError::NoNeighbor(NavigateDirection::Left).vim_code(), "E36");
        assert_eq!(WindowError::NotEnoughRoom.vim_code(), "E94");
        assert_eq!(WindowError::CannotResizeAtEdge.vim_code(), "E36");
    }

    #[test]
    fn test_window_error_display() {
        let error = WindowError::CannotCloseLastWindow;
        assert!(error.to_string().contains("E444"));
        assert!(error.to_string().contains("Cannot close last window"));
    }
}
