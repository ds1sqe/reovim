use {
    reovim_kernel::api::v1::{BufferId, WindowId},
    reovim_subsys_coordination::{Cursor, CursorHeader},
};

use {super::Viewport, crate::window::Window};

// --- Test cursor ---

#[derive(Clone)]
struct TestCursor {
    header_val: CursorHeader,
    data: Vec<u8>,
}

impl TestCursor {
    fn new(domain_id: u32, inner_id: u16) -> Self {
        Self {
            header_val: CursorHeader::new(domain_id, inner_id, 0),
            data: Vec::new(),
        }
    }
}

impl Cursor for TestCursor {
    fn header(&self) -> &CursorHeader {
        &self.header_val
    }

    fn content(&self) -> &[u8] {
        &self.data
    }

    fn encode(&self) -> Vec<u8> {
        Vec::from(self.header().as_bytes())
    }

    fn display(&self) -> String {
        format!("cursor({}:{})", self.header_val.domain_id(), self.header_val.inner_id())
    }

    fn clone_box(&self) -> Box<dyn Cursor> {
        Box::new(self.clone())
    }
}

fn test_viewport() -> Viewport {
    Viewport {
        width: 80,
        height: 24,
        scroll_top: 0,
        scroll_left: 0,
    }
}

// --- Tests ---

#[test]
fn new_window_has_initial_cursor() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let win =
        Window::new(WindowId::from_raw(1), BufferId::from_raw(10), test_viewport(), 1, cursor);
    assert_eq!(win.cursors().len(), 1);
}

#[test]
fn window_accessors() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let win = Window::new(
        WindowId::from_raw(5),
        BufferId::from_raw(20),
        Viewport {
            width: 120,
            height: 30,
            scroll_top: 10,
            scroll_left: 0,
        },
        1,
        cursor,
    );
    assert_eq!(win.id(), WindowId::from_raw(5));
    assert_eq!(win.buffer_id(), BufferId::from_raw(20));
    assert_eq!(win.domain_id(), 1);
    assert_eq!(win.viewport().scroll_top, 10);
    assert_eq!(win.viewport().height, 30);
    assert_eq!(win.viewport().width, 120);
}

#[test]
fn primary_cursor() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let win =
        Window::new(WindowId::from_raw(1), BufferId::from_raw(10), test_viewport(), 1, cursor);
    let primary = win.primary_cursor().unwrap();
    assert_eq!(primary.header().domain_id(), 1);
    assert_eq!(primary.display(), "cursor(1:0)");
}

#[test]
fn set_cursors_replaces_all() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let mut win =
        Window::new(WindowId::from_raw(1), BufferId::from_raw(10), test_viewport(), 1, cursor);
    assert_eq!(win.cursors().len(), 1);

    // Multi-cursor update
    let new_cursors: Vec<Box<dyn Cursor>> = vec![
        Box::new(TestCursor::new(1, 0)),
        Box::new(TestCursor::new(1, 1)),
        Box::new(TestCursor::new(1, 2)),
    ];
    win.set_cursors(new_cursors);
    assert_eq!(win.cursors().len(), 3);
    assert_eq!(win.cursors()[1].header().inner_id(), 1);
}

#[test]
fn set_buffer_changes_buffer_and_domain() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let mut win =
        Window::new(WindowId::from_raw(1), BufferId::from_raw(10), test_viewport(), 1, cursor);
    assert_eq!(win.buffer_id(), BufferId::from_raw(10));
    assert_eq!(win.domain_id(), 1);

    // Switch to a mesh buffer (different domain)
    win.set_buffer(BufferId::from_raw(50), 2);
    assert_eq!(win.buffer_id(), BufferId::from_raw(50));
    assert_eq!(win.domain_id(), 2);
}

#[test]
fn set_buffer_same_domain() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let mut win =
        Window::new(WindowId::from_raw(1), BufferId::from_raw(10), test_viewport(), 1, cursor);

    // Switch to another text buffer (same domain)
    win.set_buffer(BufferId::from_raw(20), 1);
    assert_eq!(win.buffer_id(), BufferId::from_raw(20));
    assert_eq!(win.domain_id(), 1);
}

#[test]
fn viewport_mut() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let mut win =
        Window::new(WindowId::from_raw(1), BufferId::from_raw(10), test_viewport(), 1, cursor);
    win.viewport_mut().scroll_top = 42;
    assert_eq!(win.viewport().scroll_top, 42);
}

#[test]
fn primary_cursor_empty() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let mut win =
        Window::new(WindowId::from_raw(1), BufferId::from_raw(10), test_viewport(), 1, cursor);
    win.set_cursors(Vec::new());
    assert!(win.primary_cursor().is_none());
}

#[test]
fn window_drop_releases_cursors() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let win =
        Window::new(WindowId::from_raw(1), BufferId::from_raw(10), test_viewport(), 1, cursor);
    // Dropping the window drops all cursor handles — no on_window_closed needed
    drop(win);
}

#[test]
fn debug_format() {
    let cursor = Box::new(TestCursor::new(1, 0));
    let win =
        Window::new(WindowId::from_raw(1), BufferId::from_raw(10), test_viewport(), 1, cursor);
    let debug = format!("{win:?}");
    assert!(debug.contains("Window"));
}
