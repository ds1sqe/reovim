use super::*;

#[test]
fn test_input_key_serialization() {
    let input = Input::key(KeyEvent::new(KeyCode::Char('a')));
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"key\""));
    assert!(input.is_key());
    assert!(!input.is_click());
}

#[test]
fn test_input_click_serialization() {
    let input = Input::click(ClickEvent::new(MouseButton::Left, ClickKind::Down, 10, 20));
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"click\""));
    assert!(input.is_click());
}

#[test]
fn test_input_scroll_serialization() {
    let input = Input::scroll(ScrollEvent::new(ScrollDirection::Up, 5, 10));
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"scroll\""));
    assert!(input.is_scroll());
}

#[test]
fn test_input_resize_serialization() {
    let input = Input::resize(ResizeEvent::new(120, 40));
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"resize\""));
    assert!(json.contains("\"width\":120"));
    assert!(input.is_resize());
}

#[test]
fn test_input_focus_serialization() {
    let input = Input::focus(FocusEvent::gained());
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"focus\""));
    assert!(json.contains("\"kind\":\"gained\""));
    assert!(input.is_focus());
}

#[test]
fn test_input_paste_serialization() {
    let input = Input::paste(PasteEvent::new("pasted text"));
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"paste\""));
    assert!(json.contains("\"content\":\"pasted text\""));
    assert!(input.is_paste());
}

#[test]
fn test_input_roundtrip() {
    // Key event
    let key_input = Input::key(KeyEvent::with_modifiers(
        KeyCode::Char('s'),
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    ));
    let json = serde_json::to_string(&key_input).unwrap();
    let decoded: Input = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_key());

    // Click event
    let click_input = Input::click(ClickEvent::new(MouseButton::Right, ClickKind::Up, 5, 15));
    let json = serde_json::to_string(&click_input).unwrap();
    let decoded: Input = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_click());

    // Resize event
    let resize_input = Input::resize(ResizeEvent::new(100, 50));
    let json = serde_json::to_string(&resize_input).unwrap();
    let decoded: Input = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_resize());
}

#[test]
fn test_input_attach_serialization() {
    let input = Input::attach(AttachEvent::with_session("my-session"));
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"attach\""));
    assert!(json.contains("\"session_id\":\"my-session\""));
    assert!(input.is_attach());
    assert!(!input.is_detach());
}

#[test]
fn test_input_detach_serialization() {
    let input = Input::detach(DetachEvent::normal());
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"detach\""));
    assert!(json.contains("\"reason\":\"normal\""));
    assert!(input.is_detach());
    assert!(!input.is_attach());
}

#[test]
fn test_session_events_roundtrip() {
    // Attach event
    let attach_input = Input::attach(AttachEvent::with_size(120, 40));
    let json = serde_json::to_string(&attach_input).unwrap();
    let decoded: Input = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_attach());

    // Detach event
    let detach_input = Input::detach(DetachEvent::with_reason(DetachReason::Disconnected));
    let json = serde_json::to_string(&detach_input).unwrap();
    let decoded: Input = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_detach());
}

#[test]
fn test_input_ping_serialization() {
    let input = Input::ping(PingEvent::with_seq(1));
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"ping\""));
    assert!(json.contains("\"seq\":1"));
    assert!(input.is_ping());
    assert!(!input.is_pong());
}

#[test]
fn test_input_pong_serialization() {
    let input = Input::pong(PongEvent::with_seq(1));
    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"type\":\"pong\""));
    assert!(json.contains("\"seq\":1"));
    assert!(input.is_pong());
    assert!(!input.is_ping());
}

#[test]
fn test_ping_pong_roundtrip() {
    // Ping event
    let health_check = Input::ping(PingEvent::with_seq(999));
    let json = serde_json::to_string(&health_check).unwrap();
    let decoded: Input = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_ping());

    // Pong event
    let health_response = Input::pong(PongEvent::with_seq(999));
    let json2 = serde_json::to_string(&health_response).unwrap();
    let decoded2: Input = serde_json::from_str(&json2).unwrap();
    assert!(decoded2.is_pong());
}

#[test]
fn test_input_all_is_methods_exclusive() {
    let key = Input::key(KeyEvent::new(KeyCode::Char('a')));
    assert!(key.is_key());
    assert!(!key.is_click());
    assert!(!key.is_scroll());
    assert!(!key.is_resize());
    assert!(!key.is_focus());
    assert!(!key.is_paste());
    assert!(!key.is_attach());
    assert!(!key.is_detach());
    assert!(!key.is_ping());
    assert!(!key.is_pong());
}

#[test]
fn test_input_scroll_variant_is_methods() {
    let scroll = Input::scroll(ScrollEvent::new(ScrollDirection::Down, 0, 0));
    assert!(scroll.is_scroll());
    assert!(!scroll.is_key());
}

#[test]
fn test_input_focus_variant_is_methods() {
    let focus = Input::focus(FocusEvent::lost());
    assert!(focus.is_focus());
    assert!(!scroll_input_is_not_focus(&focus));
}

fn scroll_input_is_not_focus(input: &Input) -> bool {
    input.is_scroll()
}

#[test]
fn test_input_paste_variant() {
    let paste = Input::paste(PasteEvent::new("text"));
    assert!(paste.is_paste());
    assert!(!paste.is_key());
}

#[test]
fn test_input_equality() {
    let a = Input::key(KeyEvent::new(KeyCode::Char('a')));
    let b = Input::key(KeyEvent::new(KeyCode::Char('a')));
    assert_eq!(a, b);

    let c = Input::key(KeyEvent::new(KeyCode::Char('b')));
    assert_ne!(a, c);
}

#[test]
fn test_input_all_constructors() {
    // Verify all constructors compile and produce correct variants
    let _ = Input::key(KeyEvent::new(KeyCode::Escape));
    let _ = Input::click(ClickEvent::moved(0, 0));
    let _ = Input::scroll(ScrollEvent::new(ScrollDirection::Up, 0, 0));
    let _ = Input::resize(ResizeEvent::new(80, 24));
    let _ = Input::focus(FocusEvent::gained());
    let _ = Input::paste(PasteEvent::new(""));
    let _ = Input::attach(AttachEvent::new());
    let _ = Input::detach(DetachEvent::normal());
    let _ = Input::ping(PingEvent::new());
    let _ = Input::pong(PongEvent::new());
}
