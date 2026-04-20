use crate::{Modifiers, MouseButton, MouseEvent, MouseEventKind};

#[test]
fn test_mouse_event_creation() {
    let event = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), 10, 20);
    assert_eq!(event.column, 10);
    assert_eq!(event.row, 20);
    assert!(event.is_down());
    assert!(!event.is_up());
    assert!(!event.is_scroll());
    assert!(!event.is_drag());
    assert!(!event.is_moved());
}

#[test]
fn test_mouse_event_with_modifiers() {
    let event = MouseEvent::with_modifiers(
        MouseEventKind::Down(MouseButton::Left),
        5,
        10,
        Modifiers::CTRL | Modifiers::SHIFT,
    );
    assert!(event.modifiers.contains(Modifiers::CTRL));
    assert!(event.modifiers.contains(Modifiers::SHIFT));
}

#[test]
fn test_mouse_scroll() {
    let scroll_up = MouseEvent::new(MouseEventKind::ScrollUp, 0, 0);
    assert!(scroll_up.is_scroll());
    assert!(!scroll_up.is_down());

    let scroll_down = MouseEvent::new(MouseEventKind::ScrollDown, 0, 0);
    assert!(scroll_down.is_scroll());

    let scroll_left = MouseEvent::new(MouseEventKind::ScrollLeft, 0, 0);
    assert!(scroll_left.is_scroll());

    let scroll_right = MouseEvent::new(MouseEventKind::ScrollRight, 0, 0);
    assert!(scroll_right.is_scroll());
}

#[test]
fn test_mouse_up() {
    let event = MouseEvent::new(MouseEventKind::Up(MouseButton::Right), 0, 0);
    assert!(event.is_up());
    assert!(!event.is_down());
}

#[test]
fn test_mouse_drag() {
    let event = MouseEvent::new(MouseEventKind::Drag(MouseButton::Left), 15, 25);
    assert!(event.is_drag());
    assert!(!event.is_down());
    assert!(!event.is_up());
}

#[test]
fn test_mouse_moved() {
    let event = MouseEvent::new(MouseEventKind::Moved, 30, 40);
    assert!(event.is_moved());
    assert!(!event.is_down());
    assert!(!event.is_drag());
}

#[test]
fn test_mouse_button_variants() {
    let left = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), 0, 0);
    let right = MouseEvent::new(MouseEventKind::Down(MouseButton::Right), 0, 0);
    let middle = MouseEvent::new(MouseEventKind::Down(MouseButton::Middle), 0, 0);

    assert!(left.is_down());
    assert!(right.is_down());
    assert!(middle.is_down());
}
