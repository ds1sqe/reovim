//! Rendered overlay types.
//!
//! These types represent overlays after the client has resolved their
//! positions and computed rendering properties.

use serde::{Deserialize, Serialize};

use crate::{Rect, ScreenPosition, Size, wire::LogicalOverlay};

/// A rendered overlay with computed screen position.
///
/// Created by the client after resolving the overlay's anchor
/// to a screen position based on viewport state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RenderedOverlay {
    /// The original logical overlay from the server.
    pub logical: LogicalOverlay,
    /// Computed top-left position on screen.
    pub position: ScreenPosition,
    /// Computed size (from renderer measurement).
    pub size: Size,
    /// Z-index for stacking order (higher = on top).
    pub z_index: u32,
    /// Whether this overlay captures all input (modal).
    pub is_modal: bool,
}

impl RenderedOverlay {
    /// Create a new rendered overlay.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Can't be const: accesses logical.priority
    pub fn new(logical: LogicalOverlay, position: ScreenPosition, size: Size) -> Self {
        let z_index = logical.priority;
        Self {
            logical,
            position,
            size,
            z_index,
            is_modal: false,
        }
    }

    /// Mark this overlay as modal.
    #[must_use]
    pub const fn as_modal(mut self) -> Self {
        self.is_modal = true;
        self
    }

    /// Set the z-index.
    #[must_use]
    pub const fn with_z_index(mut self, z_index: u32) -> Self {
        self.z_index = z_index;
        self
    }

    /// Get the bounding rectangle.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        Rect::new(self.position.x, self.position.y, self.size.width, self.size.height)
    }

    /// Get the overlay ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.logical.id
    }

    /// Get the overlay kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.logical.kind
    }
}

/// Stack of rendered overlays with z-ordering.
///
/// Manages overlay visibility, stacking order, and modal state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct OverlayStack {
    overlays: Vec<RenderedOverlay>,
    next_z_index: u32,
}

impl OverlayStack {
    /// Create an empty overlay stack.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            overlays: Vec::new(),
            next_z_index: 0,
        }
    }

    /// Push an overlay onto the stack.
    ///
    /// Automatically assigns the next z-index unless the overlay
    /// already has a higher z-index.
    pub fn push(&mut self, mut overlay: RenderedOverlay) {
        if overlay.z_index < self.next_z_index {
            overlay.z_index = self.next_z_index;
        }
        self.next_z_index = overlay.z_index + 1;
        self.overlays.push(overlay);
    }

    /// Remove an overlay by ID.
    ///
    /// Returns `true` if the overlay was found and removed.
    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(pos) = self.overlays.iter().position(|o| o.id() == id) {
            self.overlays.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get an overlay by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&RenderedOverlay> {
        self.overlays.iter().find(|o| o.id() == id)
    }

    /// Get a mutable reference to an overlay by ID.
    #[must_use]
    pub fn get_mut(&mut self, id: &str) -> Option<&mut RenderedOverlay> {
        self.overlays.iter_mut().find(|o| o.id() == id)
    }

    /// Check if any overlay is modal.
    #[must_use]
    pub fn has_modal(&self) -> bool {
        self.overlays.iter().any(|o| o.is_modal)
    }

    /// Get the topmost modal overlay, if any.
    #[must_use]
    pub fn topmost_modal(&self) -> Option<&RenderedOverlay> {
        self.overlays
            .iter()
            .filter(|o| o.is_modal)
            .max_by_key(|o| o.z_index)
    }

    /// Get all overlays in z-order (bottom to top).
    #[must_use]
    pub fn in_z_order(&self) -> Vec<&RenderedOverlay> {
        let mut sorted: Vec<_> = self.overlays.iter().collect();
        sorted.sort_by_key(|o| o.z_index);
        sorted
    }

    /// Get the number of overlays.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.overlays.len()
    }

    /// Check if the stack is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.overlays.is_empty()
    }

    /// Clear all overlays.
    pub fn clear(&mut self) {
        self.overlays.clear();
        self.next_z_index = 0;
    }

    /// Find the overlay at a given position.
    ///
    /// Returns the topmost (highest z-index) overlay containing the point.
    #[must_use]
    pub fn at_position(&self, pos: ScreenPosition) -> Option<&RenderedOverlay> {
        self.overlays
            .iter()
            .filter(|o| o.bounds().contains(pos))
            .max_by_key(|o| o.z_index)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::wire::Anchor};

    fn test_overlay(id: &str, kind: &str) -> LogicalOverlay {
        LogicalOverlay::new(id, kind, Anchor::Cursor)
    }

    fn rendered(id: &str, x: u16, y: u16, w: u16, h: u16) -> RenderedOverlay {
        RenderedOverlay::new(test_overlay(id, "test"), ScreenPosition::new(x, y), Size::new(w, h))
    }

    #[test]
    fn test_rendered_overlay_new() {
        let overlay = rendered("test-1", 10, 20, 100, 50);
        assert_eq!(overlay.id(), "test-1");
        assert_eq!(overlay.position, ScreenPosition::new(10, 20));
        assert_eq!(overlay.size, Size::new(100, 50));
        assert!(!overlay.is_modal);
    }

    #[test]
    fn test_rendered_overlay_bounds() {
        let overlay = rendered("test", 10, 20, 100, 50);
        let bounds = overlay.bounds();
        assert_eq!(bounds.x, 10);
        assert_eq!(bounds.y, 20);
        assert_eq!(bounds.width, 100);
        assert_eq!(bounds.height, 50);
    }

    #[test]
    fn test_rendered_overlay_modal() {
        let overlay = rendered("modal", 0, 0, 80, 24).as_modal();
        assert!(overlay.is_modal);
    }

    #[test]
    fn test_overlay_stack_push_remove() {
        let mut stack = OverlayStack::new();
        assert!(stack.is_empty());

        stack.push(rendered("a", 0, 0, 10, 10));
        stack.push(rendered("b", 0, 0, 10, 10));
        assert_eq!(stack.len(), 2);

        assert!(stack.remove("a"));
        assert_eq!(stack.len(), 1);
        assert!(!stack.remove("a")); // Already removed
    }

    #[test]
    fn test_overlay_stack_z_order() {
        let mut stack = OverlayStack::new();
        stack.push(rendered("a", 0, 0, 10, 10));
        stack.push(rendered("b", 0, 0, 10, 10));
        stack.push(rendered("c", 0, 0, 10, 10));

        let ordered = stack.in_z_order();
        assert_eq!(ordered[0].id(), "a");
        assert_eq!(ordered[1].id(), "b");
        assert_eq!(ordered[2].id(), "c");

        // Verify z-indices are ascending
        assert!(ordered[0].z_index < ordered[1].z_index);
        assert!(ordered[1].z_index < ordered[2].z_index);
    }

    #[test]
    fn test_overlay_stack_has_modal() {
        let mut stack = OverlayStack::new();
        stack.push(rendered("a", 0, 0, 10, 10));
        assert!(!stack.has_modal());

        stack.push(rendered("modal", 0, 0, 80, 24).as_modal());
        assert!(stack.has_modal());
    }

    #[test]
    fn test_overlay_stack_topmost_modal() {
        let mut stack = OverlayStack::new();
        stack.push(rendered("modal-1", 0, 0, 80, 24).as_modal());
        stack.push(rendered("normal", 0, 0, 10, 10));
        stack.push(rendered("modal-2", 0, 0, 80, 24).as_modal());

        let topmost = stack.topmost_modal().unwrap();
        assert_eq!(topmost.id(), "modal-2");
    }

    #[test]
    fn test_overlay_stack_at_position() {
        let mut stack = OverlayStack::new();
        stack.push(rendered("a", 0, 0, 20, 20));
        stack.push(rendered("b", 10, 10, 20, 20));

        // Position only in "a"
        let hit = stack.at_position(ScreenPosition::new(5, 5));
        assert_eq!(hit.unwrap().id(), "a");

        // Position in both - should get "b" (higher z-index)
        let hit = stack.at_position(ScreenPosition::new(15, 15));
        assert_eq!(hit.unwrap().id(), "b");

        // Position outside both
        let hit = stack.at_position(ScreenPosition::new(50, 50));
        assert!(hit.is_none());
    }

    #[test]
    fn test_overlay_stack_get() {
        let mut stack = OverlayStack::new();
        stack.push(rendered("test", 0, 0, 10, 10));

        assert!(stack.get("test").is_some());
        assert!(stack.get("nonexistent").is_none());
    }

    #[test]
    fn test_overlay_stack_clear() {
        let mut stack = OverlayStack::new();
        stack.push(rendered("a", 0, 0, 10, 10));
        stack.push(rendered("b", 0, 0, 10, 10));
        assert_eq!(stack.len(), 2);

        stack.clear();
        assert!(stack.is_empty());
    }
}
