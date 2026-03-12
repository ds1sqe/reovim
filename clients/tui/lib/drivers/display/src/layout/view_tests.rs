use super::*;

#[test]
fn test_line_index() {
    let line = LineIndex::new(10);
    assert_eq!(line.as_usize(), 10);

    // Arithmetic
    assert_eq!((line + 5).as_usize(), 15);
    assert_eq!((line - 3).as_usize(), 7);

    // Saturating subtraction
    let zero = LineIndex::ZERO;
    assert_eq!((zero - 5).as_usize(), 0);

    // Display (1-indexed)
    assert_eq!(format!("{line}"), "11");
}

#[test]
fn test_col_index() {
    let col = ColIndex::new(5);
    assert_eq!(col.as_usize(), 5);

    // Arithmetic
    assert_eq!((col + 3).as_usize(), 8);
    assert_eq!((col - 2).as_usize(), 3);

    // Display (1-indexed)
    assert_eq!(format!("{col}"), "6");
}

#[test]
fn test_position_new() {
    let pos = Position::new(LineIndex::new(10), ColIndex::new(5));
    assert_eq!(pos.line.as_usize(), 10);
    assert_eq!(pos.col.as_usize(), 5);
}

#[test]
fn test_position_from_raw() {
    let pos = Position::from_raw(10, 5);
    assert_eq!(pos.line.as_usize(), 10);
    assert_eq!(pos.col.as_usize(), 5);
}

#[test]
fn test_position_origin() {
    let pos = Position::origin();
    assert_eq!(pos.line.as_usize(), 0);
    assert_eq!(pos.col.as_usize(), 0);
}

#[test]
fn test_view_new() {
    let buffer_id = BufferId::from_raw(1);
    let view = View::new(buffer_id);
    assert_eq!(view.buffer_id, buffer_id);
    assert_eq!(view.cursor, Position::origin());
    assert_eq!(view.scroll_top.as_usize(), 0);
    assert_eq!(view.scroll_left.as_usize(), 0);
}

#[test]
fn test_view_builder() {
    let buffer_id = BufferId::from_raw(1);
    let view = View::new(buffer_id)
        .with_cursor(Position::from_raw(10, 5))
        .with_scroll(LineIndex::new(100), ColIndex::new(20));

    assert_eq!(view.cursor.line.as_usize(), 10);
    assert_eq!(view.cursor.col.as_usize(), 5);
    assert_eq!(view.scroll_top.as_usize(), 100);
    assert_eq!(view.scroll_left.as_usize(), 20);
}

#[test]
fn test_view_builder_raw() {
    let buffer_id = BufferId::from_raw(1);
    let view = View::new(buffer_id)
        .with_cursor(Position::from_raw(10, 5))
        .with_scroll_raw(100, 20);

    assert_eq!(view.scroll_top.as_usize(), 100);
    assert_eq!(view.scroll_left.as_usize(), 20);
}

// Mock ViewManager for testing default trait methods
struct MockViewManager {
    views: std::collections::HashMap<WindowId, View>,
}

impl MockViewManager {
    fn new() -> Self {
        Self {
            views: std::collections::HashMap::new(),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ViewManager for MockViewManager {
    fn create(&mut self, window: WindowId, buffer: BufferId) -> &View {
        self.views.insert(window, View::new(buffer));
        self.views.get(&window).unwrap()
    }

    fn get(&self, window: WindowId) -> Option<&View> {
        self.views.get(&window)
    }

    fn get_mut(&mut self, window: WindowId) -> Option<&mut View> {
        self.views.get_mut(&window)
    }

    fn remove(&mut self, window: WindowId) {
        self.views.remove(&window);
    }

    fn on_focus_changed(&mut self, _from: WindowId, _to: WindowId) {}

    fn all_views(&self) -> Vec<(WindowId, &View)> {
        self.views.iter().map(|(&k, v)| (k, v)).collect()
    }
}

#[test]
fn test_view_manager_buffer_of() {
    let mut vm = MockViewManager::new();
    let win = WindowId::from_raw(1);
    let buf = BufferId::from_raw(10);

    vm.create(win, buf);
    assert_eq!(vm.buffer_of(win), Some(buf));
    assert_eq!(vm.buffer_of(WindowId::from_raw(99)), None);
}

#[test]
fn test_view_manager_contains() {
    let mut vm = MockViewManager::new();
    let win = WindowId::from_raw(1);
    let buf = BufferId::from_raw(10);

    assert!(!vm.contains(win));
    vm.create(win, buf);
    assert!(vm.contains(win));
}

#[test]
fn test_view_manager_len_and_is_empty() {
    let mut vm = MockViewManager::new();

    assert!(vm.is_empty());
    assert_eq!(vm.len(), 0);

    vm.create(WindowId::from_raw(1), BufferId::from_raw(10));
    assert!(!vm.is_empty());
    assert_eq!(vm.len(), 1);

    vm.create(WindowId::from_raw(2), BufferId::from_raw(20));
    assert_eq!(vm.len(), 2);
}
