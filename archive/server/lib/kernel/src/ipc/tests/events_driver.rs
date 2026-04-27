use super::*;

use crate::ipc::events::driver::{KeyCode, Modifiers, MouseButton, MouseEvent, MouseInput};

#[test]
fn test_display_resized() {
    let event = events::DisplayResized {
        width: 120,
        height: 40,
    };
    assert_eq!(event.width, 120);
    assert_eq!(event.height, 40);
}

#[test]
fn test_key_input() {
    let event = events::KeyInput {
        key: KeyCode::Char('a'),
        modifiers: Modifiers::CTRL,
    };
    assert_eq!(event.key, KeyCode::Char('a'));
    assert!(event.modifiers.ctrl);
}

#[test]
fn test_keycode_char() {
    let key = KeyCode::Char('x');
    assert!(key.is_char());
    assert_eq!(key.as_char(), Some('x'));

    let key = KeyCode::Enter;
    assert!(!key.is_char());
    assert_eq!(key.as_char(), None);
}

#[test]
fn test_modifiers_empty() {
    assert!(Modifiers::NONE.is_empty());
    assert!(!Modifiers::NONE.any());
}

#[test]
fn test_modifiers_any() {
    assert!(!Modifiers::CTRL.is_empty());
    assert!(Modifiers::CTRL.any());
}

#[test]
fn test_modifiers_default() {
    let mods = Modifiers::default();
    assert!(mods.is_empty());
}

#[test]
fn test_mouse_input() {
    let event = MouseInput {
        event: MouseEvent::Down(MouseButton::Left),
        column: 10,
        row: 5,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(event.column, 10);
    assert_eq!(event.row, 5);
}
