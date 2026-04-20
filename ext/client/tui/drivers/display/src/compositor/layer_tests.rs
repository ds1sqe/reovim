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

#[test]
fn test_set_z_order_nonexistent() {
    // Line 122 else branch: set_z_order with nonexistent ID
    let mut compositor = LayerCompositor::new();
    compositor.set_z_order(ComposableId::Window(999), ZOrder::editor(0));
    // Should be a no-op without panic
    assert!(compositor.get(ComposableId::Window(999)).is_none());
}

#[test]
fn test_set_z_group_nonexistent() {
    // Line 133 else branch: set_z_group with nonexistent ID
    let mut compositor = LayerCompositor::new();
    compositor.set_z_group(ComposableId::Window(999), ZGroup::Editor);
    assert!(compositor.get(ComposableId::Window(999)).is_none());
}

#[test]
fn test_bring_to_front_nonexistent() {
    // Line 146 else branch: bring_to_front with nonexistent ID
    let mut compositor = LayerCompositor::new();
    compositor.bring_to_front(ComposableId::Window(999));
    assert!(compositor.get(ComposableId::Window(999)).is_none());
}

#[test]
fn test_send_to_back_nonexistent() {
    // Line 158 else branch: send_to_back with nonexistent ID
    let mut compositor = LayerCompositor::new();
    compositor.send_to_back(ComposableId::Window(999));
    assert!(compositor.get(ComposableId::Window(999)).is_none());
}

#[test]
fn test_keyboard_target_visible_not_capturing() {
    // Line 189/191: visible=true but captures_keyboard=false
    let mut compositor = LayerCompositor::new();
    let mock = MockComposable {
        id: ComposableId::Window(0),
        z_order: ZOrder::editor(0),
        visible: true,
        bounds: Bounds::new(0, 0, 10, 10),
        captures_keyboard: false,
        cursor_pos: None,
    };
    compositor.register(Box::new(mock));

    // No element captures keyboard, should return None
    assert!(compositor.keyboard_target().is_none());
}

#[test]
fn test_hit_test_invisible_element() {
    // Line 215/216: entry exists but is not visible in hit_test
    let mut compositor = LayerCompositor::new();
    let mock = MockComposable {
        id: ComposableId::Window(0),
        z_order: ZOrder::editor(0),
        visible: false,
        bounds: Bounds::new(0, 0, 80, 24),
        captures_keyboard: false,
        cursor_pos: None,
    };
    compositor.register(Box::new(mock));

    let buffer = FrameBuffer::new(80, 24);
    // Point is within bounds, but element is invisible
    assert!(compositor.hit_test(5, 5, &buffer).is_none());
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
