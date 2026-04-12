//! Tests for text domain events.

use super::*;
use reovim_kernel::api::v1::events::kernel::priority;

// ── TextBufferModified ──────────────────────────────────────────────

#[test]
fn text_buffer_modified_construction() {
    let event = TextBufferModified {
        buffer_id: BufferId::from_raw(1),
        edit: TextEdit::insert(TextPosition::new(0, 0), "hello"),
        start_byte: 0,
        old_end_byte: 0,
        new_end_byte: 5,
    };
    assert_eq!(event.buffer_id, BufferId::from_raw(1));
    assert_eq!(event.start_byte, 0);
    assert_eq!(event.new_end_byte, 5);
}

#[test]
fn text_buffer_modified_priority() {
    let event = TextBufferModified {
        buffer_id: BufferId::from_raw(1),
        edit: TextEdit::insert(TextPosition::origin(), "x"),
        start_byte: 0,
        old_end_byte: 0,
        new_end_byte: 1,
    };
    assert_eq!(event.priority(), priority::NORMAL);
}

#[test]
fn text_buffer_modified_clone() {
    let event = TextBufferModified {
        buffer_id: BufferId::from_raw(1),
        edit: TextEdit::insert(TextPosition::origin(), "x"),
        start_byte: 0,
        old_end_byte: 0,
        new_end_byte: 1,
    };
    let cloned = event.clone();
    assert_eq!(cloned.buffer_id, event.buffer_id);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn text_buffer_modified_debug() {
    let event = TextBufferModified {
        buffer_id: BufferId::from_raw(1),
        edit: TextEdit::insert(TextPosition::origin(), "x"),
        start_byte: 0,
        old_end_byte: 0,
        new_end_byte: 1,
    };
    let debug = format!("{event:?}");
    assert!(debug.contains("TextBufferModified"));
}

// ── CursorMoved ─────────────────────────────────────────────────────

#[test]
fn cursor_moved_construction() {
    let event = CursorMoved {
        window_id: WindowId::from_raw(1),
        buffer_id: BufferId::from_raw(2),
        from: TextPosition::new(0, 0),
        to: TextPosition::new(5, 10),
    };
    assert_eq!(event.window_id, WindowId::from_raw(1));
    assert_eq!(event.to.line, 5);
    assert_eq!(event.to.column, 10);
}

#[test]
fn cursor_moved_priority() {
    let event = CursorMoved {
        window_id: WindowId::from_raw(1),
        buffer_id: BufferId::from_raw(1),
        from: TextPosition::origin(),
        to: TextPosition::new(1, 0),
    };
    assert_eq!(event.priority(), priority::NORMAL);
}

#[test]
fn cursor_moved_is_copy() {
    let event = CursorMoved {
        window_id: WindowId::from_raw(1),
        buffer_id: BufferId::from_raw(1),
        from: TextPosition::origin(),
        to: TextPosition::new(1, 0),
    };
    let copied = event;
    // If CursorMoved isn't Copy, this won't compile.
    assert_eq!(copied.window_id, event.window_id);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn cursor_moved_debug() {
    let event = CursorMoved {
        window_id: WindowId::from_raw(1),
        buffer_id: BufferId::from_raw(1),
        from: TextPosition::origin(),
        to: TextPosition::origin(),
    };
    let debug = format!("{event:?}");
    assert!(debug.contains("CursorMoved"));
}

// ── ViewportScrolled ────────────────────────────────────────────────

#[test]
fn viewport_scrolled_construction() {
    let event = ViewportScrolled {
        window_id: WindowId::from_raw(1),
        buffer_id: BufferId::from_raw(2),
        top_line: 100,
        bottom_line: 150,
    };
    assert_eq!(event.top_line, 100);
    assert_eq!(event.bottom_line, 150);
}

#[test]
fn viewport_scrolled_priority() {
    let event = ViewportScrolled {
        window_id: WindowId::from_raw(1),
        buffer_id: BufferId::from_raw(1),
        top_line: 0,
        bottom_line: 50,
    };
    assert_eq!(event.priority(), priority::NORMAL);
}

#[test]
fn viewport_scrolled_is_copy() {
    let event = ViewportScrolled {
        window_id: WindowId::from_raw(1),
        buffer_id: BufferId::from_raw(1),
        top_line: 0,
        bottom_line: 50,
    };
    let copied = event;
    assert_eq!(copied.top_line, event.top_line);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn viewport_scrolled_debug() {
    let event = ViewportScrolled {
        window_id: WindowId::from_raw(1),
        buffer_id: BufferId::from_raw(1),
        top_line: 0,
        bottom_line: 50,
    };
    let debug = format!("{event:?}");
    assert!(debug.contains("ViewportScrolled"));
}
