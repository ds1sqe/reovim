use {
    super::*,
    crate::ipc::events::{
        driver::{KeyCode, Modifiers},
        key::{ClientId, KeyPressEvent, SessionId},
    },
};

#[test]
fn test_session_id_creation() {
    let id = SessionId::new(42);
    assert_eq!(id.as_usize(), 42);
}

#[test]
fn test_session_id_display() {
    let id = SessionId::new(42);
    assert_eq!(id.to_string(), "session-42");
}

#[test]
fn test_session_id_equality() {
    let id1 = SessionId::new(1);
    let id2 = SessionId::new(1);
    let id3 = SessionId::new(2);
    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn test_client_id_creation() {
    let id = ClientId::new(99);
    assert_eq!(id.as_usize(), 99);
}

#[test]
fn test_client_id_display() {
    let id = ClientId::new(99);
    assert_eq!(id.to_string(), "client-99");
}

#[test]
fn test_client_id_equality() {
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(1);
    let id3 = ClientId::new(2);
    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn test_key_press_event_creation() {
    let key = events::KeyInput {
        key: KeyCode::Char('j'),
        modifiers: Modifiers::NONE,
    };
    let event = KeyPressEvent::new(key.clone(), SessionId::new(0), ClientId::new(1));

    assert_eq!(event.key, key);
    assert_eq!(event.session_id.as_usize(), 0);
    assert_eq!(event.client_id.as_usize(), 1);
}

#[test]
fn test_key_press_event_type() {
    let key = events::KeyInput {
        key: KeyCode::Char('k'),
        modifiers: Modifiers::CTRL,
    };
    let event = KeyPressEvent::new(key, SessionId::new(0), ClientId::new(0));
    assert_eq!(event.event_type(), "KeyPressEvent");
}

#[test]
fn test_key_press_event_with_modifiers() {
    let key = events::KeyInput {
        key: KeyCode::Char('s'),
        modifiers: Modifiers::CTRL,
    };
    let event = KeyPressEvent::new(key, SessionId::new(1), ClientId::new(2));

    assert_eq!(event.key.key, KeyCode::Char('s'));
    assert!(event.key.modifiers.ctrl);
    assert!(!event.key.modifiers.alt);
}

#[test]
fn test_key_press_event_special_keys() {
    let key = events::KeyInput {
        key: KeyCode::Esc,
        modifiers: Modifiers::NONE,
    };
    let event = KeyPressEvent::new(key, SessionId::new(0), ClientId::new(0));
    assert_eq!(event.key.key, KeyCode::Esc);

    let key = events::KeyInput {
        key: KeyCode::Enter,
        modifiers: Modifiers::SHIFT,
    };
    let event = KeyPressEvent::new(key, SessionId::new(0), ClientId::new(0));
    assert_eq!(event.key.key, KeyCode::Enter);
    assert!(event.key.modifiers.shift);
}
