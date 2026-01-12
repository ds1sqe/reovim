//! Compositor types and trait for z-ordered rendering.

// TODO(Phase 5-6): Move ZGroup, ZOrder, Composable, ComposableId from lib/core/src/compositor/
// to this file. After move, remove these re-exports and define types locally.
// CRITICAL: ZGroup values MUST remain: Base=0, Sidebar=100, Editor=200, Floating=300,
//           Overlay=400, Popup=500, Panel=600, Modal=700, Alert=800
pub use reovim_core::compositor::{Composable, ComposableId, ZGroup, ZOrder};

// TODO(Phase 5-6): These will be imported from local modules after type migration
pub use reovim_core::{frame::FrameBuffer, highlight::Style};

/// Compositor trait for managing z-ordered elements.
///
/// Handles registration, ordering, and rendering of composable elements.
pub trait Compositor: Send + Sync {
    /// Register a composable element.
    fn register(&mut self, composable: Box<dyn Composable>);

    /// Unregister by ID.
    fn unregister(&mut self, id: ComposableId);

    /// Get composable by ID.
    fn get(&self, id: ComposableId) -> Option<&dyn Composable>;

    /// Get mutable composable by ID.
    fn get_mut(&mut self, id: ComposableId) -> Option<&mut dyn Composable>;

    /// Set z-order for a composable.
    fn set_z_order(&mut self, id: ComposableId, z: ZOrder);

    /// Bring composable to front within its z-group.
    fn bring_to_front(&mut self, id: ComposableId);

    /// Send composable to back within its z-group.
    fn send_to_back(&mut self, id: ComposableId);

    /// Get render order (sorted by z-order).
    fn render_order(&self) -> Vec<ComposableId>;

    /// Render all visible composables, returns cursor position if any.
    fn render(&mut self, buffer: &mut FrameBuffer, default_style: &Style) -> Option<(u16, u16)>;

    /// Hit test for mouse clicks (returns topmost at position).
    fn hit_test(
        &self,
        x: u16,
        y: u16,
        screen_width: u16,
        screen_height: u16,
    ) -> Option<ComposableId>;

    /// Find keyboard target (topmost that captures keyboard).
    fn keyboard_target(&self) -> Option<ComposableId>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify `ZGroup` values match the issue #176 specification.
    /// These values are critical for correct z-order rendering.
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
