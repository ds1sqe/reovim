//! Tests for TUI mouse types — `MouseButton`, `MouseEvent`, `MouseEventKind`.

use crate::{
    Modifiers, MouseButton, MouseEvent, MouseEventKind,
    mouse_types::{bytes_to_mouse_kind, mouse_kind_to_bytes},
};

// ---------------------------------------------------------------------------
// MouseEventKind roundtrip — every variant
// ---------------------------------------------------------------------------

fn all_mouse_event_kinds() -> Vec<MouseEventKind> {
    vec![
        MouseEventKind::Moved,
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Down(MouseButton::Right),
        MouseEventKind::Down(MouseButton::Middle),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Right),
        MouseEventKind::Up(MouseButton::Middle),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Right),
        MouseEventKind::Drag(MouseButton::Middle),
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ]
}

#[test]
fn mouse_event_kind_bytes_roundtrip_all_variants() {
    for kind in all_mouse_event_kinds() {
        let (tag, button) = mouse_kind_to_bytes(&kind);
        let decoded = bytes_to_mouse_kind(tag, button)
            .unwrap_or_else(|| panic!("decode failed for {kind:?}: tag={tag}, button={button}"));
        assert_eq!(decoded, kind, "roundtrip failed for {kind:?}");
    }
}

#[test]
fn unknown_tag_returns_none() {
    assert!(bytes_to_mouse_kind(0xFF, 0).is_none());
    assert!(bytes_to_mouse_kind(8, 0).is_none());
}

#[test]
fn unknown_button_with_tagged_kind_returns_none() {
    // tag=1 (Down) with button=99 is unknown
    assert!(bytes_to_mouse_kind(1, 99).is_none());
}

// ---------------------------------------------------------------------------
// MouseEvent constructors and helpers
// ---------------------------------------------------------------------------

#[test]
fn mouse_event_new_no_modifiers() {
    let ev = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), 10, 5);
    assert_eq!(ev.kind, MouseEventKind::Down(MouseButton::Left));
    assert_eq!(ev.column, 10);
    assert_eq!(ev.row, 5);
    assert_eq!(ev.modifiers, Modifiers::NONE);
    assert!(ev.is_down());
    assert!(!ev.is_up());
    assert!(!ev.is_scroll());
    assert!(!ev.is_drag());
    assert!(!ev.is_moved());
}

#[test]
fn mouse_event_with_modifiers() {
    let ev =
        MouseEvent::with_modifiers(MouseEventKind::Up(MouseButton::Right), 3, 7, Modifiers::CTRL);
    assert_eq!(ev.modifiers, Modifiers::CTRL);
    assert!(ev.is_up());
}

#[test]
fn mouse_event_scroll_helpers() {
    for kind in [
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ] {
        let ev = MouseEvent::new(kind, 0, 0);
        assert!(ev.is_scroll(), "expected is_scroll() for {kind:?}");
    }
}

#[test]
fn mouse_event_drag_helper() {
    let ev = MouseEvent::new(MouseEventKind::Drag(MouseButton::Left), 0, 0);
    assert!(ev.is_drag());
    assert!(!ev.is_moved());
}

#[test]
fn mouse_event_moved_helper() {
    let ev = MouseEvent::new(MouseEventKind::Moved, 0, 0);
    assert!(ev.is_moved());
    assert!(!ev.is_drag());
}

#[test]
fn mouse_event_eq_used_in_collections() {
    use std::collections::HashSet;

    let a = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), 5, 3);
    let b = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), 5, 3);
    let c = MouseEvent::new(MouseEventKind::Down(MouseButton::Right), 5, 3);

    assert_eq!(a, b);
    assert_ne!(a, c);

    let mut set = HashSet::new();
    set.insert(MouseButton::Left);
    set.insert(MouseButton::Right);
    set.insert(MouseButton::Middle);
    assert_eq!(set.len(), 3);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn mouse_event_debug_mentions_type() {
    let dbg = format!("{:?}", MouseEvent::new(MouseEventKind::Moved, 0, 0));
    assert!(dbg.contains("MouseEvent"));
}
