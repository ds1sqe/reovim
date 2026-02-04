//! Compositor module for z-ordered layer management.
//!
//! This module provides the compositor system for managing z-ordered
//! UI elements. The compositor handles layer stacking, rendering order,
//! hit testing, and keyboard focus tracking.
//!
//! # Architecture
//!
//! ```text
//! Compositor Trait ────────────────────────────────────────────────────
//!       │                                                               │
//!       ├── register/unregister  (manage composables)                   │
//!       ├── bring_to_front/send_to_back  (z-order manipulation)         │
//!       ├── hit_test/keyboard_target  (interaction)                     │
//!       └── render  (output)                                            │
//!                                                                       │
//! LayerCompositor (concrete implementation) ──────────────────────────│
//! ```
//!
//! # Types
//!
//! - [`Bounds`] - Rectangular region with position and dimensions
//! - [`ZGroup`] - Major z-order categories (Base, Editor, Modal, etc.)
//! - [`ZOrder`] - Fine-grained z-ordering (group + `sub_order` + sequence)
//! - [`ComposableId`] - Unique identifier for composable elements
//! - [`Composable`] - Trait for renderable elements
//! - [`CompositorEntry`] - Entry in the compositor registry
//! - [`LayerCompositor`] - Concrete compositor implementation
//! - [`Compositor`] - Trait for compositor implementations

mod bounds;
mod composable;
mod layer;
mod z_order;

// Re-export all public types
pub use {
    bounds::Bounds,
    composable::{Composable, ComposableId, FrameBuffer, Style},
    layer::{CompositorEntry, LayerCompositor},
    z_order::{ZGroup, ZOrder},
};

/// Compositor trait for managing z-ordered elements.
///
/// This trait defines the interface for compositor implementations.
/// The primary implementation is [`LayerCompositor`].
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow compositors to be
/// shared across threads (e.g., for async rendering).
pub trait Compositor: Send + Sync {
    // ========================================================================
    // Registration
    // ========================================================================

    /// Register a composable element.
    ///
    /// If an element with the same ID already exists, it is replaced.
    fn register(&mut self, composable: Box<dyn Composable>);

    /// Unregister a composable by ID.
    ///
    /// If the removed element was focused, focus should be cleared.
    fn unregister(&mut self, id: ComposableId);

    // ========================================================================
    // Access
    // ========================================================================

    /// Get a composable by ID.
    fn get(&self, id: ComposableId) -> Option<&dyn Composable>;

    /// Get a mutable composable by ID.
    fn get_mut(&mut self, id: ComposableId) -> Option<&mut (dyn Composable + '_)>;

    // ========================================================================
    // Z-Order Management
    // ========================================================================

    /// Set the z-order for a composable.
    fn set_z_order(&mut self, id: ComposableId, z: ZOrder);

    /// Set the z-group for a composable, preserving `sub_order`.
    fn set_z_group(&mut self, id: ComposableId, group: ZGroup);

    /// Bring a composable to the front within its z-group.
    fn bring_to_front(&mut self, id: ComposableId);

    /// Send a composable to the back within its z-group.
    fn send_to_back(&mut self, id: ComposableId);

    // ========================================================================
    // Focus Management
    // ========================================================================

    /// Set the focused element.
    fn set_focus(&mut self, id: Option<ComposableId>);

    /// Get the currently focused element ID.
    fn focused(&self) -> Option<ComposableId>;

    // ========================================================================
    // Render Order
    // ========================================================================

    /// Get the render order (sorted by z-order).
    fn render_order(&mut self) -> Vec<ComposableId>;

    // ========================================================================
    // Rendering
    // ========================================================================

    /// Render all visible composables in z-order.
    ///
    /// Returns the cursor position from the topmost composable that provides one.
    fn render(&mut self, buffer: &mut FrameBuffer, default_style: &Style) -> Option<(u16, u16)>;

    // ========================================================================
    // Interaction
    // ========================================================================

    /// Hit test for the topmost element at a position.
    ///
    /// Uses buffer dimensions for bounds calculation.
    fn hit_test(&mut self, x: u16, y: u16, buffer: &FrameBuffer) -> Option<ComposableId>;

    /// Find the keyboard target (topmost that captures keyboard).
    fn keyboard_target(&mut self) -> Option<ComposableId>;
}

/// Implement Compositor trait for `LayerCompositor`.
impl Compositor for LayerCompositor {
    fn register(&mut self, composable: Box<dyn Composable>) {
        Self::register(self, composable);
    }

    fn unregister(&mut self, id: ComposableId) {
        Self::unregister(self, id);
    }

    fn get(&self, id: ComposableId) -> Option<&dyn Composable> {
        Self::get(self, id)
    }

    fn get_mut(&mut self, id: ComposableId) -> Option<&mut (dyn Composable + '_)> {
        Self::get_mut(self, id)
    }

    fn set_z_order(&mut self, id: ComposableId, z: ZOrder) {
        Self::set_z_order(self, id, z);
    }

    fn set_z_group(&mut self, id: ComposableId, group: ZGroup) {
        Self::set_z_group(self, id, group);
    }

    fn bring_to_front(&mut self, id: ComposableId) {
        Self::bring_to_front(self, id);
    }

    fn send_to_back(&mut self, id: ComposableId) {
        Self::send_to_back(self, id);
    }

    fn set_focus(&mut self, id: Option<ComposableId>) {
        Self::set_focus(self, id);
    }

    fn focused(&self) -> Option<ComposableId> {
        Self::focused(self)
    }

    fn render_order(&mut self) -> Vec<ComposableId> {
        Self::render_order(self)
    }

    fn render(&mut self, buffer: &mut FrameBuffer, default_style: &Style) -> Option<(u16, u16)> {
        Self::render(self, buffer, default_style)
    }

    fn hit_test(&mut self, x: u16, y: u16, buffer: &FrameBuffer) -> Option<ComposableId> {
        Self::hit_test(self, x, y, buffer)
    }

    fn keyboard_target(&mut self) -> Option<ComposableId> {
        Self::keyboard_target(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `LayerCompositor` implements Compositor trait.
    #[test]
    fn test_layer_compositor_implements_trait() {
        let _compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
    }

    /// Verify `ZGroup` values match the specification.
    #[test]
    fn test_zgroup_values_match_spec() {
        assert_eq!(ZGroup::Base as u16, 0);
        assert_eq!(ZGroup::Sidebar as u16, 100);
        assert_eq!(ZGroup::Editor as u16, 200);
        assert_eq!(ZGroup::Floating as u16, 300);
        assert_eq!(ZGroup::Overlay as u16, 400);
        assert_eq!(ZGroup::Popup as u16, 500);
        assert_eq!(ZGroup::Panel as u16, 600);
        assert_eq!(ZGroup::Modal as u16, 700);
        assert_eq!(ZGroup::Alert as u16, 800);
    }
}
