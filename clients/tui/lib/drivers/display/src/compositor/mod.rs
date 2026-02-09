//! Compositor module for z-ordered layer management.
//!
//! This module provides the compositor system for managing z-ordered
//! UI elements. The compositor handles layer stacking, rendering order,
//! hit testing, and keyboard focus tracking.
//!
//! # Architecture
//!
//! ```text
//! ┌─ Composable (trait) ─┐
//! │  id, z_order, bounds │
//! │  render, hit test    │
//! └──────────┬───────────┘
//!            │ registered into
//!            ▼
//! ┌─ Compositor (trait) ─────────────────────────────────┐
//! │  register/unregister    (manage composables)         │
//! │  bring_to_front/send_to_back  (z-order manipulation) │
//! │  hit_test/keyboard_target     (interaction)          │
//! │  render                       (output)               │
//! └──────────────────────┬───────────────────────────────┘
//!                        │ implemented by
//!                        ▼
//! ┌─ LayerCompositor ────────────────────────────────────┐
//! │  BTreeMap<ComposableId, CompositorEntry>             │
//! │  z-order sorted render order, focus tracking         │
//! └──────────────────────────────────────────────────────┘
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

    // =========================================================================
    // Compositor trait impl delegation tests (cover all delegating methods)
    // =========================================================================

    /// Mock composable for testing the Compositor trait impl.
    #[derive(Debug)]
    struct MockComposable {
        id: ComposableId,
        z_order: ZOrder,
        visible: bool,
        bounds: Bounds,
        captures_keyboard: bool,
        cursor_pos: Option<(u16, u16)>,
    }

    impl MockComposable {
        fn new(id: ComposableId) -> Self {
            Self {
                id,
                z_order: ZOrder::new(id.default_group(), 0),
                visible: true,
                bounds: Bounds::new(0, 0, 10, 10),
                captures_keyboard: true,
                cursor_pos: None,
            }
        }

        fn with_cursor(mut self, pos: (u16, u16)) -> Self {
            self.cursor_pos = Some(pos);
            self
        }

        fn with_bounds(mut self, b: Bounds) -> Self {
            self.bounds = b;
            self
        }
    }

    impl Composable for MockComposable {
        fn id(&self) -> ComposableId {
            self.id
        }
        fn z_order(&self) -> ZOrder {
            self.z_order
        }
        fn set_z_order(&mut self, z: ZOrder) {
            self.z_order = z;
        }
        fn is_visible(&self) -> bool {
            self.visible
        }
        fn bounds(&self, _w: u16, _h: u16) -> Bounds {
            self.bounds
        }
        fn render(&self, _buffer: &mut FrameBuffer, _style: &Style) {}
        fn captures_keyboard(&self) -> bool {
            self.captures_keyboard
        }
        fn cursor_position(&self) -> Option<(u16, u16)> {
            self.cursor_pos
        }
    }

    /// Test register/unregister via trait object.
    #[test]
    fn test_compositor_trait_register_unregister() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        compositor.register(Box::new(MockComposable::new(ComposableId::Window(0))));
        assert!(compositor.get(ComposableId::Window(0)).is_some());

        compositor.unregister(ComposableId::Window(0));
        assert!(compositor.get(ComposableId::Window(0)).is_none());
    }

    /// Test `get`/`get_mut` via trait object.
    #[test]
    fn test_compositor_trait_get_get_mut() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        compositor.register(Box::new(MockComposable::new(ComposableId::Window(0))));

        // get (immutable)
        let composable = compositor.get(ComposableId::Window(0));
        assert!(composable.is_some());
        assert_eq!(composable.unwrap().id(), ComposableId::Window(0));

        // get_mut (mutable)
        let composable_mut = compositor.get_mut(ComposableId::Window(0));
        assert!(composable_mut.is_some());

        // get non-existent
        assert!(compositor.get(ComposableId::Window(99)).is_none());
        assert!(compositor.get_mut(ComposableId::Window(99)).is_none());
    }

    /// Test `set_z_order` via trait object.
    #[test]
    fn test_compositor_trait_set_z_order() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        compositor.register(Box::new(MockComposable::new(ComposableId::Window(0))));

        let new_z = ZOrder::modal(5);
        compositor.set_z_order(ComposableId::Window(0), new_z);

        let composable = compositor.get(ComposableId::Window(0)).unwrap();
        assert_eq!(composable.z_order().group, ZGroup::Modal);
        assert_eq!(composable.z_order().sub_order, 5);
    }

    /// Test `set_z_group` via trait object.
    #[test]
    fn test_compositor_trait_set_z_group() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        compositor.register(Box::new(MockComposable::new(ComposableId::Window(0))));

        compositor.set_z_group(ComposableId::Window(0), ZGroup::Alert);

        let composable = compositor.get(ComposableId::Window(0)).unwrap();
        assert_eq!(composable.z_order().group, ZGroup::Alert);
    }

    /// Test `bring_to_front`/`send_to_back` via trait object.
    #[test]
    fn test_compositor_trait_bring_to_front_send_to_back() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        compositor.register(Box::new(MockComposable::new(ComposableId::Window(0))));

        let old_seq = compositor
            .get(ComposableId::Window(0))
            .unwrap()
            .z_order()
            .sequence;
        compositor.bring_to_front(ComposableId::Window(0));
        let new_seq = compositor
            .get(ComposableId::Window(0))
            .unwrap()
            .z_order()
            .sequence;
        assert!(new_seq > old_seq);

        compositor.send_to_back(ComposableId::Window(0));
        assert_eq!(
            compositor
                .get(ComposableId::Window(0))
                .unwrap()
                .z_order()
                .sequence,
            0
        );
    }

    /// Test `set_focus`/`focused` via trait object.
    #[test]
    fn test_compositor_trait_focus() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        compositor.register(Box::new(MockComposable::new(ComposableId::Window(0))));

        assert!(compositor.focused().is_none());

        compositor.set_focus(Some(ComposableId::Window(0)));
        assert_eq!(compositor.focused(), Some(ComposableId::Window(0)));

        compositor.set_focus(None);
        assert!(compositor.focused().is_none());
    }

    /// Test `render_order` via trait object.
    #[test]
    fn test_compositor_trait_render_order() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        let mock1 = MockComposable {
            id: ComposableId::Window(0),
            z_order: ZOrder::modal(0),
            visible: true,
            bounds: Bounds::new(0, 0, 10, 10),
            captures_keyboard: true,
            cursor_pos: None,
        };
        let mock2 = MockComposable {
            id: ComposableId::Window(1),
            z_order: ZOrder::base(),
            visible: true,
            bounds: Bounds::new(0, 0, 10, 10),
            captures_keyboard: true,
            cursor_pos: None,
        };

        compositor.register(Box::new(mock1));
        compositor.register(Box::new(mock2));

        let order = compositor.render_order();
        assert_eq!(order.len(), 2);
        // Base should come before modal
        assert_eq!(order[0], ComposableId::Window(1));
        assert_eq!(order[1], ComposableId::Window(0));
    }

    /// Test render via trait object returns cursor position.
    #[test]
    fn test_compositor_trait_render() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        let mock = MockComposable::new(ComposableId::Window(0))
            .with_cursor((5, 3))
            .with_bounds(Bounds::new(0, 0, 80, 24));

        compositor.register(Box::new(mock));

        let mut buffer = FrameBuffer::new(80, 24);
        let cursor_pos = compositor.render(&mut buffer, &Style::default());
        assert_eq!(cursor_pos, Some((5, 3)));
    }

    /// Test render via trait object with no cursor.
    #[test]
    fn test_compositor_trait_render_no_cursor() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        let mock =
            MockComposable::new(ComposableId::Window(0)).with_bounds(Bounds::new(0, 0, 80, 24));

        compositor.register(Box::new(mock));

        let mut buffer = FrameBuffer::new(80, 24);
        let cursor_pos = compositor.render(&mut buffer, &Style::default());
        assert!(cursor_pos.is_none());
    }

    /// Test `hit_test` via trait object.
    #[test]
    fn test_compositor_trait_hit_test() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        let mock =
            MockComposable::new(ComposableId::Window(0)).with_bounds(Bounds::new(0, 0, 50, 50));

        compositor.register(Box::new(mock));

        let buffer = FrameBuffer::new(80, 24);
        assert_eq!(compositor.hit_test(10, 10, &buffer), Some(ComposableId::Window(0)));
        assert!(compositor.hit_test(60, 60, &buffer).is_none());
    }

    /// Test `keyboard_target` via trait object.
    #[test]
    fn test_compositor_trait_keyboard_target() {
        let mut compositor: Box<dyn Compositor> = Box::new(LayerCompositor::new());
        let mock = MockComposable::new(ComposableId::Window(0));
        compositor.register(Box::new(mock));

        assert_eq!(compositor.keyboard_target(), Some(ComposableId::Window(0)));
    }
}
