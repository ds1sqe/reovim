use {
    super::*,
    reovim_driver_layout::{
        CompositeResult, Layer, LayerConfig, LayerId, RootCompositor, WindowLayerCompositor,
    },
    reovim_kernel::testing::test_mode,
};

#[test]
fn test_client_id() {
    let id = ClientId::new(42);
    assert_eq!(id.as_usize(), 42);
    assert_eq!(id.to_string(), "client-42");
}

#[test]
fn test_viewport() {
    let vp = Viewport::new(100, 50);
    assert_eq!(vp.width, 100);
    assert_eq!(vp.height, 50);
    assert_eq!(vp.scroll_top, 0);
    assert_eq!(vp.scroll_left, 0);
    assert_eq!(vp.last_visible_line(), 49);
    assert_eq!(vp.last_visible_column(), 99);
    assert!(vp.is_line_visible(0));
    assert!(vp.is_line_visible(49));
    assert!(!vp.is_line_visible(50));
    assert!(vp.is_column_visible(0));
    assert!(vp.is_column_visible(99));
    assert!(!vp.is_column_visible(100));
    assert!(vp.is_position_visible(25, 50));
    assert!(!vp.is_position_visible(50, 50));
}

#[test]
fn test_viewport_default() {
    let vp = Viewport::default();
    assert_eq!(vp.width, 80);
    assert_eq!(vp.height, 24);
}

#[test]
fn test_cursor_position() {
    let cursor = CursorPosition::new(5, 10);
    assert_eq!(cursor.line, 5);
    assert_eq!(cursor.column, 10);

    let origin = CursorPosition::origin();
    assert_eq!(origin.line, 0);
    assert_eq!(origin.column, 0);
}

#[test]
fn test_cursor_position_conversion() {
    let pos = Position::new(10, 20);
    let cursor: CursorPosition = pos.into();
    assert_eq!(cursor.line, 10);
    assert_eq!(cursor.column, 20);

    let back: Position = cursor.into();
    assert_eq!(back, pos);
}

#[test]
fn test_window() {
    let window = Window::new();
    assert!(window.buffer_id.is_none());

    let buf_id = BufferId::new();
    let window = Window::with_buffer(buf_id);
    assert_eq!(window.buffer_id, Some(buf_id));
}

#[test]
fn test_window_layout_empty() {
    let layout = WindowLayout::empty();
    assert!(layout.is_empty());
    assert_eq!(layout.len(), 0);
    assert!(layout.active().is_none());
}

#[test]
fn test_window_layout_single() {
    let window = Window::new();
    let id = window.id;
    let layout = WindowLayout::single(window);

    assert!(!layout.is_empty());
    assert_eq!(layout.len(), 1);
    assert_eq!(layout.active_id(), Some(id));
}

#[test]
fn test_window_layout_add() {
    let mut layout = WindowLayout::empty();
    let w1 = Window::new();
    let id1 = w1.id;
    layout.add(w1);

    assert_eq!(layout.active_id(), Some(id1)); // First becomes active

    let w2 = Window::new();
    let id2 = w2.id;
    layout.add(w2);

    assert_eq!(layout.len(), 2);
    assert_eq!(layout.active_id(), Some(id1)); // Still first

    assert!(layout.set_active(id2));
    assert_eq!(layout.active_id(), Some(id2));
}

#[test]
fn test_key_sequence() {
    let mut seq = KeySequence::new();
    assert!(seq.is_empty());

    seq.push("d".to_string());
    seq.push("w".to_string());
    assert!(!seq.is_empty());
    assert_eq!(seq.as_string(), "dw");

    seq.clear();
    assert!(seq.is_empty());
}

#[test]
fn test_session_new() {
    let mode = test_mode();
    let session = Session::new(ClientId::new(1), mode.clone());

    assert_eq!(session.id.as_usize(), 1);
    // home_mode is stored in shared (#491)
    assert_eq!(session.shared.home_mode(), &mode);
    // active_buffer and terminal_size are per-client (#471), not in Session
}

#[test]
fn test_session_bootstrap() {
    let mode = test_mode();
    let (session, bootstrap) = Session::bootstrap(ClientId::new(1), mode.clone());

    // Session has only shared infrastructure
    assert_eq!(session.id.as_usize(), 1);
    // active_buffer and terminal_size are per-client (#471)

    // BootstrapState has per-client initial state
    assert_eq!(bootstrap.mode_stack.current(), &mode);
    assert!(bootstrap.windows.is_empty());
    assert!(bootstrap.pending_keys.is_empty());
    assert!(bootstrap.extensions.is_empty());
}

// NOTE (#471): active_buffer and terminal_size tests removed.
// These are now per-client (in EditingState/ClientContext), tested in testing.rs.

// =========================================================================
// TextObjRange tests
// =========================================================================

#[test]
fn test_window_layout_active_fallback() {
    // Regression test for cursor visibility bug (#465):
    // active() should return first window when active_index is None
    let mut layout = WindowLayout::empty();

    // Empty layout returns None
    assert!(layout.active().is_none());
    assert!(layout.active_id().is_none());

    // Add window but don't set active_index explicitly
    let w1 = Window::new();
    let id1 = w1.id;
    layout.windows.push(w1); // Direct push bypasses active_index setting

    // Fallback should return first window
    assert!(layout.active().is_some());
    assert_eq!(layout.active_id(), Some(id1));

    // active_mut should also work
    assert!(layout.active_mut().is_some());
}

#[test]
fn test_window_layout_active_stale_index() {
    // Test fallback when active_index points to invalid index
    let mut layout = WindowLayout::empty();
    let w1 = Window::new();
    let id1 = w1.id;
    layout.add(w1);

    // Manually set invalid index
    layout.active_index = Some(999);

    // Should fallback to first window
    assert!(layout.active().is_some());
    assert_eq!(layout.active_id(), Some(id1));
}

#[test]
fn test_textobj_range_linewise() {
    let range = TextObjRange::linewise(Position::new(1, 0), Position::new(3, 0));
    assert_eq!(range.start, Position::new(1, 0));
    assert_eq!(range.end, Position::new(3, 0));
    assert!(range.is_linewise);
}

#[test]
fn test_textobj_range_is_empty() {
    let empty = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 0));
    assert!(empty.is_empty());

    let not_empty = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 1));
    assert!(!not_empty.is_empty());

    let multiline_empty = TextObjRange::linewise(Position::new(1, 0), Position::new(2, 0));
    assert!(!multiline_empty.is_empty()); // Different lines
}

// =========================================================================
// SessionShared tests (#471, #491)
// =========================================================================

#[test]
fn test_session_shared_new() {
    let mode = test_mode();
    let shared = SessionShared::new(mode.clone());

    assert!(shared.compositor.is_none());
    // active_buffer and terminal_size are per-client (#471)
    assert_eq!(shared.home_mode(), &mode);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_session_shared_debug() {
    let mode = test_mode();
    let shared = SessionShared::new(mode);
    let debug_str = format!("{shared:?}");

    assert!(debug_str.contains("SessionShared"));
    assert!(debug_str.contains("compositor"));
    assert!(debug_str.contains("home_mode"));
}

#[test]
fn test_session_shared_home_mode() {
    let mode = test_mode();
    let shared = SessionShared::new(mode.clone());

    assert_eq!(shared.home_mode(), &mode);
}

// =========================================================================
// Additional WindowLayout tests
// =========================================================================

#[test]
fn test_window_layout_clear() {
    let mut layout = WindowLayout::empty();
    layout.add(Window::new());
    layout.add(Window::new());
    assert_eq!(layout.len(), 2);

    layout.clear();
    assert!(layout.is_empty());
    assert_eq!(layout.len(), 0);
    assert!(layout.active().is_none());
    assert!(layout.active_mut().is_none());
}

#[test]
fn test_window_layout_get() {
    let mut layout = WindowLayout::empty();
    let w = Window::new();
    let id = w.id;
    layout.add(w);

    // Get existing window
    let found = layout.get(id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, id);

    // Get non-existing window
    let fake_id = WindowId::new();
    assert!(layout.get(fake_id).is_none());
}

#[test]
fn test_window_layout_get_mut() {
    let mut layout = WindowLayout::empty();
    let w = Window::new();
    let id = w.id;
    layout.add(w);

    // Get mut existing window
    let found = layout.get_mut(id);
    assert!(found.is_some());

    // Modify window
    found.unwrap().cursor = CursorPosition::new(5, 10);
    assert_eq!(layout.get(id).unwrap().cursor.line, 5);

    // Get mut non-existing window
    let fake_id = WindowId::new();
    assert!(layout.get_mut(fake_id).is_none());
}

#[test]
fn test_window_layout_set_active_nonexistent() {
    let mut layout = WindowLayout::empty();
    layout.add(Window::new());

    let fake_id = WindowId::new();
    assert!(!layout.set_active(fake_id));
}

#[test]
fn test_window_layout_set_active_returns_true() {
    let mut layout = WindowLayout::empty();
    let w1 = Window::new();
    let w2 = Window::new();
    let id2 = w2.id;
    layout.add(w1);
    layout.add(w2);

    assert!(layout.set_active(id2));
    assert_eq!(layout.active_id(), Some(id2));
}

#[test]
fn test_window_layout_default() {
    let layout = WindowLayout::default();
    assert!(layout.is_empty());
}

#[test]
fn test_window_layout_clone() {
    let mut layout = WindowLayout::empty();
    let w = Window::new();
    let id = w.id;
    layout.add(w);

    let cloned = layout.clone();
    assert_eq!(cloned.len(), 1);
    assert_eq!(cloned.active_id(), Some(id));
}

// =========================================================================
// Additional KeySequence tests
// =========================================================================

#[test]
fn test_key_sequence_keys() {
    let mut seq = KeySequence::new();
    seq.push("d".to_string());
    seq.push("w".to_string());

    let keys = seq.keys();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], "d");
    assert_eq!(keys[1], "w");
}

#[test]
fn test_key_sequence_default() {
    let seq = KeySequence::default();
    assert!(seq.is_empty());
    assert!(seq.keys().is_empty());
    assert!(seq.as_string().is_empty());
}

#[test]
fn test_key_sequence_as_string_single() {
    let mut seq = KeySequence::new();
    seq.push("a".to_string());
    assert_eq!(seq.as_string(), "a");
}

#[test]
fn test_key_sequence_special_keys() {
    let mut seq = KeySequence::new();
    seq.push("<C-w>".to_string());
    seq.push("h".to_string());
    assert_eq!(seq.as_string(), "<C-w>h");
    assert_eq!(seq.keys().len(), 2);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_debug() {
    let seq = KeySequence::new();
    let debug = format!("{seq:?}");
    assert!(debug.contains("KeySequence"));
}

// =========================================================================
// Additional Viewport tests
// =========================================================================

#[test]
fn test_viewport_with_scroll() {
    let mut vp = Viewport::new(80, 24);
    vp.scroll_top = 10;
    vp.scroll_left = 5;

    assert_eq!(vp.last_visible_line(), 33); // 10 + 24 - 1
    assert_eq!(vp.last_visible_column(), 84); // 5 + 80 - 1

    // Lines before scroll_top should not be visible
    assert!(!vp.is_line_visible(9));
    assert!(vp.is_line_visible(10));
    assert!(vp.is_line_visible(33));
    assert!(!vp.is_line_visible(34));

    // Columns before scroll_left should not be visible
    assert!(!vp.is_column_visible(4));
    assert!(vp.is_column_visible(5));
    assert!(vp.is_column_visible(84));
    assert!(!vp.is_column_visible(85));
}

#[test]
fn test_viewport_is_position_visible_with_scroll() {
    let mut vp = Viewport::new(40, 20);
    vp.scroll_top = 5;
    vp.scroll_left = 10;

    // Position within viewport
    assert!(vp.is_position_visible(10, 20));

    // Position outside - line too early
    assert!(!vp.is_position_visible(4, 20));

    // Position outside - column too early
    assert!(!vp.is_position_visible(10, 9));

    // Position outside - both out
    assert!(!vp.is_position_visible(100, 200));
}

#[test]
fn test_viewport_default_size() {
    let vp = Viewport::default_size();
    assert_eq!(vp.width, 80);
    assert_eq!(vp.height, 24);
    assert_eq!(vp.scroll_top, 0);
    assert_eq!(vp.scroll_left, 0);
}

// =========================================================================
// Viewport::ensure_cursor_visible tests
// =========================================================================

#[test]
fn test_ensure_cursor_visible_no_change_when_visible() {
    let mut vp = Viewport::new(80, 24);
    assert!(!vp.ensure_cursor_visible(0));
    assert_eq!(vp.scroll_top, 0);
    assert!(!vp.ensure_cursor_visible(23));
    assert_eq!(vp.scroll_top, 0);
}

#[test]
fn test_ensure_cursor_visible_scroll_down() {
    let mut vp = Viewport::new(80, 24);
    assert!(vp.ensure_cursor_visible(30));
    assert_eq!(vp.scroll_top, 7); // 30 - 24 + 1
}

#[test]
fn test_ensure_cursor_visible_scroll_up() {
    let mut vp = Viewport::new(80, 24);
    vp.scroll_top = 20;
    assert!(vp.ensure_cursor_visible(10));
    assert_eq!(vp.scroll_top, 10);
}

#[test]
fn test_ensure_cursor_visible_zero_height() {
    let mut vp = Viewport::new(80, 0);
    assert!(!vp.ensure_cursor_visible(5));
    assert_eq!(vp.scroll_top, 0);
}

#[test]
fn test_ensure_cursor_visible_exact_boundary() {
    let mut vp = Viewport::new(80, 10);
    // Cursor at line 9 (last visible line when scroll_top=0, height=10)
    assert!(!vp.ensure_cursor_visible(9));
    assert_eq!(vp.scroll_top, 0);
    // Cursor at line 10 (just past the boundary)
    assert!(vp.ensure_cursor_visible(10));
    assert_eq!(vp.scroll_top, 1);
}

#[test]
fn test_ensure_cursor_visible_returns_false_when_already_at_cursor() {
    let mut vp = Viewport::new(80, 10);
    vp.scroll_top = 5;
    // Cursor within visible range [5, 14]
    assert!(!vp.ensure_cursor_visible(5));
    assert!(!vp.ensure_cursor_visible(14));
    assert_eq!(vp.scroll_top, 5);
}

// =========================================================================
// Additional CursorPosition tests
// =========================================================================

#[test]
fn test_cursor_position_default() {
    let cursor = CursorPosition::default();
    assert_eq!(cursor.line, 0);
    assert_eq!(cursor.column, 0);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_position_debug() {
    let cursor = CursorPosition::new(5, 10);
    let debug = format!("{cursor:?}");
    assert!(debug.contains('5'));
    assert!(debug.contains("10"));
}

#[test]
fn test_cursor_position_eq() {
    let a = CursorPosition::new(5, 10);
    let b = CursorPosition::new(5, 10);
    let c = CursorPosition::new(5, 11);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// =========================================================================
// Additional Window tests
// =========================================================================

#[test]
fn test_window_default() {
    let w = Window::default();
    assert!(w.buffer_id.is_none());
    assert_eq!(w.cursor, CursorPosition::origin());
    assert!(w.selection.is_none());
}

#[test]
fn test_window_with_buffer_has_default_cursor() {
    let buf_id = BufferId::new();
    let w = Window::with_buffer(buf_id);
    assert_eq!(w.buffer_id, Some(buf_id));
    assert_eq!(w.cursor, CursorPosition::origin());
    assert_eq!(w.viewport.width, 80);
    assert_eq!(w.viewport.height, 24);
    assert!(w.selection.is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_window_debug() {
    let w = Window::new();
    let debug = format!("{w:?}");
    assert!(debug.contains("Window"));
}

// =========================================================================
// Additional TextObjRange tests
// =========================================================================

#[test]
fn test_textobj_range_clone() {
    let range = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
    let cloned = range;
    assert_eq!(range, cloned);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_textobj_range_debug() {
    let range = TextObjRange::linewise(Position::new(1, 0), Position::new(3, 0));
    let debug = format!("{range:?}");
    assert!(debug.contains("TextObjRange"));
    assert!(debug.contains("is_linewise: true"));
}

#[test]
fn test_textobj_range_eq() {
    let a = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
    let b = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(a, b);
}

#[test]
fn test_textobj_range_ne() {
    let a = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
    let b = TextObjRange::linewise(Position::new(0, 0), Position::new(0, 5));
    assert_ne!(a, b);
}

// =========================================================================
// Additional BootstrapState tests
// =========================================================================

#[test]
fn test_bootstrap_state_new() {
    let mode = test_mode();
    let state = BootstrapState::new(mode.clone());

    assert_eq!(state.mode_stack.current(), &mode);
    assert!(state.windows.is_empty());
    assert!(state.pending_keys.is_empty());
    assert!(state.extensions.is_empty());
}

#[test]
fn test_bootstrap_state_with_buffer() {
    let mode = test_mode();
    let buf_id = BufferId::new();
    let state = BootstrapState::with_buffer(mode.clone(), buf_id);

    assert_eq!(state.mode_stack.current(), &mode);
    assert_eq!(state.windows.len(), 1);
    assert_eq!(state.windows.active().unwrap().buffer_id, Some(buf_id));
    assert!(state.pending_keys.is_empty());
    assert!(state.extensions.is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_bootstrap_state_debug() {
    let mode = test_mode();
    let state = BootstrapState::new(mode);
    let debug = format!("{state:?}");
    assert!(debug.contains("BootstrapState"));
}

// =========================================================================
// Additional Session tests
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_session_debug() {
    let mode = test_mode();
    let session = Session::new(ClientId::new(1), mode);
    let debug = format!("{session:?}");
    assert!(debug.contains("Session"));
    assert!(debug.contains("ClientId"));
}

#[test]
fn test_session_compositor_none_by_default() {
    let mode = test_mode();
    let session = Session::new(ClientId::new(1), mode);
    assert!(session.compositor().is_none());
}

#[test]
fn test_session_shared_compositor_none_by_default() {
    let mode = test_mode();
    let mut shared = SessionShared::new(mode);
    assert!(shared.compositor().is_none());
    assert!(shared.compositor_mut().is_none());
}

// =========================================================================
// ClientId tests
// =========================================================================

#[test]
fn test_client_id_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(ClientId::new(1));
    set.insert(ClientId::new(2));
    set.insert(ClientId::new(1)); // duplicate

    assert_eq!(set.len(), 2);
}

#[test]
fn test_client_id_clone_copy() {
    let id = ClientId::new(42);
    let cloned = id;
    assert_eq!(id, cloned);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_client_id_debug() {
    let id = ClientId::new(42);
    let debug = format!("{id:?}");
    assert!(debug.contains("42"));
}

#[test]
fn test_client_id_display_format() {
    assert_eq!(ClientId::new(0).to_string(), "client-0");
    assert_eq!(ClientId::new(100).to_string(), "client-100");
}

// =========================================================================
// MockCompositor for compositor tests
// =========================================================================

struct MockCompositor;

impl MockCompositor {
    fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl RootCompositor for MockCompositor {
    fn composite(&self, screen: reovim_driver_layout::Rect) -> CompositeResult {
        CompositeResult::empty(screen)
    }
    fn create_layer(&mut self, _config: LayerConfig) -> LayerId {
        LayerId::new(0)
    }
    fn remove_layer(&mut self, _layer: LayerId) {}
    fn layer_by_label(&self, _label: &str) -> Option<LayerId> {
        None
    }
    fn layers(&self) -> Vec<&Layer> {
        Vec::new()
    }
    fn set_layer_visible(&mut self, _layer: LayerId, _visible: bool) {}
    fn set_layer_opacity(&mut self, _layer: LayerId, _opacity: f32) {}
    fn reorder_layer(&mut self, _layer: LayerId, _new_z: u16) {}
    fn set_active_layer(&mut self, _layer: LayerId) {}
    fn active_layer(&self) -> Option<LayerId> {
        None
    }
    fn set_focus(&mut self, _window: WindowId) {}
    fn focused(&self) -> Option<WindowId> {
        None
    }
    fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
        None
    }
    fn layer_compositor(&self, _layer: LayerId) -> Option<&dyn WindowLayerCompositor> {
        None
    }
    fn layer_compositor_mut(&mut self, _layer: LayerId) -> Option<&mut dyn WindowLayerCompositor> {
        None
    }
    fn window_count(&self) -> usize {
        0
    }
    fn set_screen(&mut self, _screen: reovim_driver_layout::Rect) {}
    fn layer_of(&self, _window: WindowId) -> Option<LayerId> {
        None
    }
    fn boxed_clone(&self) -> Box<dyn RootCompositor> {
        Box::new(Self)
    }
}

// =========================================================================
// Compositor tests
// =========================================================================

#[test]
fn test_session_shared_compositor_set_and_get() {
    let mode = test_mode();
    let mut shared = SessionShared::new(mode);

    shared.set_compositor(Box::new(MockCompositor::new()));
    assert!(shared.compositor().is_some());
    assert!(shared.compositor_mut().is_some());
}

#[test]
fn test_session_compositor_set_and_get() {
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode);

    session.set_compositor(Box::new(MockCompositor::new()));
    assert!(session.compositor().is_some());
    assert!(session.compositor_mut().is_some());
}

// =========================================================================
// Window::with_id_and_buffer test
// =========================================================================

#[test]
fn test_window_with_id_and_buffer() {
    let wid = WindowId::new();
    let bid = BufferId::new();
    let w = Window::with_id_and_buffer(wid, bid);

    assert_eq!(w.id, wid);
    assert_eq!(w.buffer_id, Some(bid));
    assert_eq!(w.cursor, CursorPosition::origin());
    assert!(w.selection.is_none());
}

// =========================================================================
// WindowLayout::remove() tests
// =========================================================================

#[test]
fn test_window_layout_remove_active_window() {
    let mut layout = WindowLayout::empty();
    let w1 = Window::new();
    let w2 = Window::new();
    let id1 = w1.id;
    let id2 = w2.id;
    layout.add(w1);
    layout.add(w2);

    // Remove active window (w1 at index 0) → resets to Some(0)
    assert!(layout.remove(id1));
    assert_eq!(layout.len(), 1);
    assert_eq!(layout.active_id(), Some(id2));
}

#[test]
fn test_window_layout_remove_before_active() {
    let mut layout = WindowLayout::empty();
    let w1 = Window::new();
    let w2 = Window::new();
    let id1 = w1.id;
    let id2 = w2.id;
    layout.add(w1);
    layout.add(w2);
    layout.set_active(id2); // active_index = 1

    // Remove w1 (before active) → active shifts from 1 to 0
    assert!(layout.remove(id1));
    assert_eq!(layout.len(), 1);
    assert_eq!(layout.active_id(), Some(id2));
}

#[test]
fn test_window_layout_remove_after_active() {
    let mut layout = WindowLayout::empty();
    let w1 = Window::new();
    let w2 = Window::new();
    let id1 = w1.id;
    let id2 = w2.id;
    layout.add(w1);
    layout.add(w2);
    // active is w1 at index 0

    // Remove w2 (after active) → active_index unchanged
    assert!(layout.remove(id2));
    assert_eq!(layout.len(), 1);
    assert_eq!(layout.active_id(), Some(id1));
}

#[test]
fn test_window_layout_remove_nonexistent() {
    let mut layout = WindowLayout::empty();
    layout.add(Window::new());
    let fake_id = WindowId::new();
    assert!(!layout.remove(fake_id));
    assert_eq!(layout.len(), 1);
}

#[test]
fn test_window_layout_remove_last_window() {
    let mut layout = WindowLayout::empty();
    let w = Window::new();
    let id = w.id;
    layout.add(w);

    assert!(layout.remove(id));
    assert!(layout.is_empty());
    assert!(layout.active_id().is_none());
}
