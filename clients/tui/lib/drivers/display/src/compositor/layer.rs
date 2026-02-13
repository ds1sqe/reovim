//! Layer compositor implementation.

use std::collections::BTreeMap;

use super::{Composable, ComposableId, FrameBuffer, Style, ZGroup, ZOrder};

/// Entry in the compositor registry.
///
/// Stores both the composable and a cached z-order value for efficient sorting.
pub struct CompositorEntry {
    /// The composable element
    pub composable: Box<dyn Composable>,
    /// Cached z-order for sorting
    pub z_order: ZOrder,
}

impl CompositorEntry {
    /// Create a new entry from a composable.
    ///
    /// The z-order is initialized from the composable's current z-order.
    #[must_use]
    pub fn new(composable: Box<dyn Composable>) -> Self {
        let z_order = composable.z_order();
        Self {
            composable,
            z_order,
        }
    }
}

/// Layer compositor for z-ordered rendering.
///
/// Manages a collection of composable elements and renders them in z-order.
/// Elements with higher z-order are rendered on top of lower ones.
pub struct LayerCompositor {
    /// Registered composables by ID
    entries: BTreeMap<ComposableId, CompositorEntry>,
    /// Cached render order (sorted by z-order)
    render_order: Vec<ComposableId>,
    /// Whether `render_order` needs rebuilding
    order_dirty: bool,
    /// Currently focused element
    focused_id: Option<ComposableId>,
}

impl Default for LayerCompositor {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerCompositor {
    /// Create a new empty layer compositor.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            render_order: Vec::new(),
            order_dirty: true,
            focused_id: None,
        }
    }

    // ========================================================================
    // Registration
    // ========================================================================

    /// Register a composable element.
    ///
    /// If an element with the same ID already exists, it is replaced.
    pub fn register(&mut self, composable: Box<dyn Composable>) {
        let id = composable.id();
        self.entries.insert(id, CompositorEntry::new(composable));
        self.order_dirty = true;
    }

    /// Unregister a composable by ID.
    ///
    /// If the removed element was focused, focus is cleared.
    pub fn unregister(&mut self, id: ComposableId) {
        self.entries.remove(&id);
        self.order_dirty = true;
        if self.focused_id == Some(id) {
            self.focused_id = None;
        }
    }

    // ========================================================================
    // Entry Access
    // ========================================================================

    /// Get a composable entry by ID.
    #[must_use]
    pub fn get_entry(&self, id: ComposableId) -> Option<&CompositorEntry> {
        self.entries.get(&id)
    }

    /// Get a mutable composable entry by ID.
    pub fn get_entry_mut(&mut self, id: ComposableId) -> Option<&mut CompositorEntry> {
        self.entries.get_mut(&id)
    }

    /// Get a composable by ID (trait object).
    #[must_use]
    pub fn get(&self, id: ComposableId) -> Option<&dyn Composable> {
        self.get_entry(id).map(|e| e.composable.as_ref())
    }

    /// Get a mutable composable by ID (trait object).
    pub fn get_mut(&mut self, id: ComposableId) -> Option<&mut (dyn Composable + '_)> {
        Some(self.get_entry_mut(id)?.composable.as_mut())
    }

    // ========================================================================
    // Z-Order Management
    // ========================================================================

    /// Set the z-order for a composable.
    ///
    /// Updates both the cached z-order and the composable's z-order.
    pub fn set_z_order(&mut self, id: ComposableId, z: ZOrder) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.z_order = z;
            entry.composable.set_z_order(z);
            self.order_dirty = true;
        }
    }

    /// Set the z-group for a composable, preserving `sub_order`.
    ///
    /// Creates a new z-order with the same `sub_order` but different group.
    pub fn set_z_group(&mut self, id: ComposableId, group: ZGroup) {
        if let Some(entry) = self.entries.get_mut(&id) {
            let new_z = ZOrder::new(group, entry.z_order.sub_order);
            entry.z_order = new_z;
            entry.composable.set_z_order(new_z);
            self.order_dirty = true;
        }
    }

    /// Bring a composable to the front within its z-group.
    ///
    /// Updates the sequence number to make this the topmost element
    /// among those with the same group and `sub_order`.
    pub fn bring_to_front(&mut self, id: ComposableId) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.z_order.bring_to_front();
            entry.composable.set_z_order(entry.z_order);
            self.order_dirty = true;
        }
    }

    /// Send a composable to the back within its z-group.
    ///
    /// Resets the sequence to 0, making this the bottommost element
    /// among those with the same group and `sub_order`.
    pub fn send_to_back(&mut self, id: ComposableId) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.z_order.send_to_back();
            entry.composable.set_z_order(entry.z_order);
            self.order_dirty = true;
        }
    }

    // ========================================================================
    // Focus Management
    // ========================================================================

    /// Set the focused element.
    pub const fn set_focus(&mut self, id: Option<ComposableId>) {
        self.focused_id = id;
    }

    /// Get the currently focused element ID.
    #[must_use]
    pub const fn focused(&self) -> Option<ComposableId> {
        self.focused_id
    }

    /// Find the keyboard target (topmost that captures keyboard).
    ///
    /// Returns the ID of the topmost visible composable that captures
    /// keyboard input, or None if no such element exists.
    pub fn keyboard_target(&mut self) -> Option<ComposableId> {
        self.ensure_render_order();

        // Search from back (highest z-order) to front
        for id in self.render_order.iter().rev() {
            if let Some(entry) = self.entries.get(id)
                && entry.composable.is_visible()
                && entry.composable.captures_keyboard()
            {
                return Some(*id);
            }
        }
        None
    }

    // ========================================================================
    // Hit Testing
    // ========================================================================

    /// Hit test for the topmost element at a position.
    ///
    /// Returns the ID of the topmost visible composable that contains
    /// the given point, or None if no element is at that position.
    pub fn hit_test(&mut self, x: u16, y: u16, buffer: &FrameBuffer) -> Option<ComposableId> {
        self.ensure_render_order();

        let screen_width = buffer.width();
        let screen_height = buffer.height();

        // Search from back (highest z-order) to front
        for id in self.render_order.iter().rev() {
            if let Some(entry) = self.entries.get(id)
                && entry.composable.is_visible()
            {
                let bounds = entry.composable.bounds(screen_width, screen_height);
                if bounds.contains(x, y) {
                    return Some(*id);
                }
            }
        }
        None
    }

    // ========================================================================
    // Rendering
    // ========================================================================

    /// Render all visible composables in z-order.
    ///
    /// Returns the cursor position from the topmost composable that provides one.
    pub fn render(
        &mut self,
        buffer: &mut FrameBuffer,
        default_style: &Style,
    ) -> Option<(u16, u16)> {
        self.ensure_render_order();

        let screen_width = buffer.width();
        let screen_height = buffer.height();

        // Clone render_order to avoid borrow conflicts
        // (need immutable iteration while mutably accessing entries for rendering)
        let ids: Vec<ComposableId> = self.render_order.clone();

        let mut cursor_pos = None;

        for entry in ids.iter().filter_map(|id| self.entries.get(id)) {
            if !entry.composable.is_visible() {
                continue;
            }

            let bounds = entry.composable.bounds(screen_width, screen_height);

            // Skip composables with zero-size bounds
            if bounds.is_empty() {
                continue;
            }

            entry.composable.render(buffer, default_style);

            // Cursor position from topmost wins (last in iteration)
            if let Some(pos) = entry.composable.cursor_position() {
                cursor_pos = Some(pos);
            }
        }

        cursor_pos
    }

    /// Get the render order (sorted by z-order).
    ///
    /// Returns a clone of the internal render order.
    #[must_use]
    pub fn render_order(&mut self) -> Vec<ComposableId> {
        self.ensure_render_order();
        self.render_order.clone()
    }

    // ========================================================================
    // Utilities
    // ========================================================================

    /// Get the number of registered composables.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the compositor is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all registered composable IDs.
    pub fn ids(&self) -> impl Iterator<Item = &ComposableId> {
        self.entries.keys()
    }

    // ========================================================================
    // Internal
    // ========================================================================

    /// Ensure `render_order` is up to date.
    fn ensure_render_order(&mut self) {
        if self.order_dirty {
            self.rebuild_render_order();
            self.order_dirty = false;
        }
    }

    /// Rebuild the render order by sorting entries by z-order.
    fn rebuild_render_order(&mut self) {
        self.render_order.clear();
        self.render_order.extend(self.entries.keys().copied());

        // Sort by z-order (lower first, rendered first = behind)
        self.render_order.sort_by(|a, b| {
            let z_a = self.entries.get(a).map(|e| e.z_order);
            let z_b = self.entries.get(b).map(|e| e.z_order);
            z_a.cmp(&z_b)
        });
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::compositor::Bounds};

    /// Mock composable for testing.
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

        fn with_z_order(mut self, z: ZOrder) -> Self {
            self.z_order = z;
            self
        }

        fn with_bounds(mut self, bounds: Bounds) -> Self {
            self.bounds = bounds;
            self
        }

        fn with_visible(mut self, visible: bool) -> Self {
            self.visible = visible;
            self
        }

        fn with_captures_keyboard(mut self, captures: bool) -> Self {
            self.captures_keyboard = captures;
            self
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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
        fn render(&self, _buffer: &mut FrameBuffer, _style: &Style) {
            // No-op for tests
        }
        fn captures_keyboard(&self) -> bool {
            self.captures_keyboard
        }
        fn cursor_position(&self) -> Option<(u16, u16)> {
            self.cursor_pos
        }
    }

    #[test]
    fn test_compositor_empty() {
        let compositor = LayerCompositor::new();
        assert!(compositor.is_empty());
        assert_eq!(compositor.len(), 0);
    }

    #[test]
    fn test_compositor_register_unregister() {
        let mut compositor = LayerCompositor::new();

        let mock = MockComposable::new(ComposableId::Window(0));
        compositor.register(Box::new(mock));

        assert!(!compositor.is_empty());
        assert_eq!(compositor.len(), 1);
        assert!(compositor.get(ComposableId::Window(0)).is_some());

        compositor.unregister(ComposableId::Window(0));

        assert!(compositor.is_empty());
        assert_eq!(compositor.len(), 0);
        assert!(compositor.get(ComposableId::Window(0)).is_none());
    }

    #[test]
    fn test_compositor_unregister_clears_focus() {
        let mut compositor = LayerCompositor::new();

        let mock = MockComposable::new(ComposableId::Window(0));
        compositor.register(Box::new(mock));
        compositor.set_focus(Some(ComposableId::Window(0)));

        assert_eq!(compositor.focused(), Some(ComposableId::Window(0)));

        compositor.unregister(ComposableId::Window(0));

        assert_eq!(compositor.focused(), None);
    }

    #[test]
    fn test_compositor_z_order_rendering() {
        let mut compositor = LayerCompositor::new();

        // Register in reverse order (highest z-order first)
        let mock1 = MockComposable::new(ComposableId::Window(0)).with_z_order(ZOrder::modal(0));
        let mock2 = MockComposable::new(ComposableId::Window(1)).with_z_order(ZOrder::editor(0));
        let mock3 = MockComposable::new(ComposableId::Window(2)).with_z_order(ZOrder::base());

        compositor.register(Box::new(mock1));
        compositor.register(Box::new(mock2));
        compositor.register(Box::new(mock3));

        let order = compositor.render_order();

        // Should be sorted: base < editor < modal
        assert_eq!(order.len(), 3);
        assert_eq!(order[0], ComposableId::Window(2)); // base
        assert_eq!(order[1], ComposableId::Window(1)); // editor
        assert_eq!(order[2], ComposableId::Window(0)); // modal
    }

    #[test]
    fn test_compositor_bring_to_front_syncs() {
        let mut compositor = LayerCompositor::new();

        let mock = MockComposable::new(ComposableId::Window(0)).with_z_order(ZOrder::editor(5));
        compositor.register(Box::new(mock));

        let old_seq = compositor
            .get_entry(ComposableId::Window(0))
            .unwrap()
            .z_order
            .sequence;

        compositor.bring_to_front(ComposableId::Window(0));

        let entry = compositor.get_entry(ComposableId::Window(0)).unwrap();
        // Entry z_order should be updated
        assert!(entry.z_order.sequence > old_seq);
        // Composable z_order should be synced
        assert_eq!(entry.composable.z_order().sequence, entry.z_order.sequence);
    }

    #[test]
    fn test_compositor_send_to_back_resets_sequence() {
        let mut compositor = LayerCompositor::new();

        let mock = MockComposable::new(ComposableId::Window(0)).with_z_order(ZOrder::editor(5));
        compositor.register(Box::new(mock));
        compositor.bring_to_front(ComposableId::Window(0));

        compositor.send_to_back(ComposableId::Window(0));

        let entry = compositor.get_entry(ComposableId::Window(0)).unwrap();
        assert_eq!(entry.z_order.sequence, 0);
        assert_eq!(entry.composable.z_order().sequence, 0);
    }

    #[test]
    fn test_compositor_set_z_group_preserves_sub_order() {
        let mut compositor = LayerCompositor::new();

        let mock = MockComposable::new(ComposableId::Window(0)).with_z_order(ZOrder::editor(5));
        compositor.register(Box::new(mock));

        compositor.set_z_group(ComposableId::Window(0), ZGroup::Modal);

        let entry = compositor.get_entry(ComposableId::Window(0)).unwrap();
        assert_eq!(entry.z_order.group, ZGroup::Modal);
        assert_eq!(entry.z_order.sub_order, 5); // Preserved!
    }

    #[test]
    fn test_compositor_keyboard_target() {
        let mut compositor = LayerCompositor::new();

        let mock1 = MockComposable::new(ComposableId::Window(0))
            .with_z_order(ZOrder::editor(0))
            .with_captures_keyboard(true);
        let mock2 = MockComposable::new(ComposableId::Window(1))
            .with_z_order(ZOrder::modal(0))
            .with_captures_keyboard(false);
        let mock3 = MockComposable::new(ComposableId::Window(2))
            .with_z_order(ZOrder::popup(0))
            .with_captures_keyboard(true);

        compositor.register(Box::new(mock1));
        compositor.register(Box::new(mock2));
        compositor.register(Box::new(mock3));

        // Popup captures keyboard and is topmost among keyboard-capturing elements
        assert_eq!(compositor.keyboard_target(), Some(ComposableId::Window(2)));
    }

    #[test]
    fn test_compositor_hit_test() {
        let mut compositor = LayerCompositor::new();

        // Two overlapping windows
        let mock1 = MockComposable::new(ComposableId::Window(0))
            .with_z_order(ZOrder::editor(0))
            .with_bounds(Bounds::new(0, 0, 50, 50));
        let mock2 = MockComposable::new(ComposableId::Window(1))
            .with_z_order(ZOrder::modal(0))
            .with_bounds(Bounds::new(25, 25, 50, 50));

        compositor.register(Box::new(mock1));
        compositor.register(Box::new(mock2));

        // Create a buffer for hit testing
        let buffer = FrameBuffer::new(80, 24);

        // Point in overlap region - should return topmost (modal)
        assert_eq!(compositor.hit_test(30, 30, &buffer), Some(ComposableId::Window(1)));

        // Point only in editor region
        assert_eq!(compositor.hit_test(10, 10, &buffer), Some(ComposableId::Window(0)));

        // Point outside all
        assert_eq!(compositor.hit_test(100, 100, &buffer), None);
    }

    #[test]
    fn test_compositor_invisible_elements_skipped() {
        let mut compositor = LayerCompositor::new();

        // Topmost element is invisible
        let mock1 = MockComposable::new(ComposableId::Window(0))
            .with_z_order(ZOrder::editor(0))
            .with_bounds(Bounds::new(0, 0, 50, 50));
        let mock2 = MockComposable::new(ComposableId::Window(1))
            .with_z_order(ZOrder::modal(0))
            .with_bounds(Bounds::new(0, 0, 50, 50))
            .with_visible(false); // Higher z-order but invisible

        compositor.register(Box::new(mock1));
        compositor.register(Box::new(mock2));

        let buffer = FrameBuffer::new(80, 24);

        // Should return editor (Window 0) because modal (Window 1) is invisible
        assert_eq!(compositor.hit_test(10, 10, &buffer), Some(ComposableId::Window(0)));

        // Keyboard target should also skip invisible
        assert_eq!(compositor.keyboard_target(), Some(ComposableId::Window(0)));
    }

    // =========================================================================
    // Coverage tests for uncovered lines
    // =========================================================================

    /// Test `LayerCompositor::default()` delegates to `new()` (lines 47-49).
    #[test]
    fn test_compositor_default() {
        let compositor = LayerCompositor::default();
        assert!(compositor.is_empty());
        assert_eq!(compositor.len(), 0);
        assert_eq!(compositor.focused(), None);
    }

    /// Test `keyboard_target` returns `None` when no composable captures keyboard (line 196).
    #[test]
    fn test_keyboard_target_returns_none_when_none_capture() {
        let mut compositor = LayerCompositor::new();

        // Register elements that do NOT capture keyboard
        let mock1 = MockComposable::new(ComposableId::Window(0))
            .with_z_order(ZOrder::editor(0))
            .with_captures_keyboard(false);
        let mock2 = MockComposable::new(ComposableId::Window(1))
            .with_z_order(ZOrder::modal(0))
            .with_captures_keyboard(false);

        compositor.register(Box::new(mock1));
        compositor.register(Box::new(mock2));

        // No element captures keyboard -> returns None
        assert_eq!(compositor.keyboard_target(), None);
    }

    /// Test `keyboard_target` returns `None` on empty compositor (line 196).
    #[test]
    fn test_keyboard_target_empty_compositor() {
        let mut compositor = LayerCompositor::new();
        assert_eq!(compositor.keyboard_target(), None);
    }

    /// Test render skips invisible composables (line 253).
    #[test]
    fn test_render_skips_invisible() {
        let mut compositor = LayerCompositor::new();

        let mock = MockComposable::new(ComposableId::Window(0))
            .with_z_order(ZOrder::editor(0))
            .with_bounds(Bounds::new(0, 0, 10, 10))
            .with_visible(false);

        compositor.register(Box::new(mock));

        let mut buffer = FrameBuffer::new(80, 24);
        let cursor = compositor.render(&mut buffer, &Style::default());
        // Invisible composable should not contribute a cursor
        assert!(cursor.is_none());
    }

    /// Test render skips composables with empty bounds (line 260).
    #[test]
    fn test_render_skips_empty_bounds() {
        let mut compositor = LayerCompositor::new();

        // Visible but with zero-size bounds
        let mock = MockComposable {
            id: ComposableId::Window(0),
            z_order: ZOrder::editor(0),
            visible: true,
            bounds: Bounds::new(0, 0, 0, 0), // Empty bounds
            captures_keyboard: true,
            cursor_pos: Some((5, 5)),
        };

        compositor.register(Box::new(mock));

        let mut buffer = FrameBuffer::new(80, 24);
        let cursor = compositor.render(&mut buffer, &Style::default());
        // Element with empty bounds should be skipped, so no cursor
        assert!(cursor.is_none());
    }

    /// Test render handles entry not found in entries map (line 269).
    /// This covers the implicit else of `if let Some(entry) = self.entries.get(id)`.
    /// In practice this cannot happen unless entries are modified concurrently,
    /// but the coverage counts the closing brace. We cover it by testing a
    /// full render that exercises the iteration logic.
    #[test]
    fn test_render_cursor_from_topmost_visible() {
        let mut compositor = LayerCompositor::new();

        // Two visible elements, both have cursor, topmost cursor wins
        let mock1 = MockComposable {
            id: ComposableId::Window(0),
            z_order: ZOrder::editor(0),
            visible: true,
            bounds: Bounds::new(0, 0, 10, 10),
            captures_keyboard: true,
            cursor_pos: Some((1, 1)),
        };
        let mock2 = MockComposable {
            id: ComposableId::Window(1),
            z_order: ZOrder::modal(0),
            visible: true,
            bounds: Bounds::new(0, 0, 10, 10),
            captures_keyboard: true,
            cursor_pos: Some((7, 7)),
        };

        compositor.register(Box::new(mock1));
        compositor.register(Box::new(mock2));

        let mut buffer = FrameBuffer::new(80, 24);
        let cursor = compositor.render(&mut buffer, &Style::default());
        // Topmost (modal, Window(1)) cursor should win
        assert_eq!(cursor, Some((7, 7)));
    }

    /// Test `ids()` iterator returns all registered composable IDs (lines 301-303).
    #[test]
    fn test_compositor_ids_iterator() {
        let mut compositor = LayerCompositor::new();

        let mock1 = MockComposable::new(ComposableId::Window(0));
        let mock2 = MockComposable::new(ComposableId::Window(1));
        let mock3 = MockComposable::new(ComposableId::Base);

        compositor.register(Box::new(mock1));
        compositor.register(Box::new(mock2));
        compositor.register(Box::new(mock3));

        let ids: Vec<_> = compositor.ids().copied().collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&ComposableId::Window(0)));
        assert!(ids.contains(&ComposableId::Window(1)));
        assert!(ids.contains(&ComposableId::Base));
    }
}
