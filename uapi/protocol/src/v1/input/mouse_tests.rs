use super::*;

#[test]
fn test_mouse_button_serialization() {
    assert_eq!(serde_json::to_string(&MouseButton::Left).unwrap(), "\"left\"");
    assert_eq!(serde_json::to_string(&MouseButton::Right).unwrap(), "\"right\"");
    assert_eq!(serde_json::to_string(&MouseButton::Middle).unwrap(), "\"middle\"");
}

#[test]
fn test_click_kind_serialization() {
    assert_eq!(serde_json::to_string(&ClickKind::Down).unwrap(), "\"down\"");
    assert_eq!(serde_json::to_string(&ClickKind::Up).unwrap(), "\"up\"");
    assert_eq!(serde_json::to_string(&ClickKind::Drag).unwrap(), "\"drag\"");
    assert_eq!(serde_json::to_string(&ClickKind::Moved).unwrap(), "\"moved\"");
}

#[test]
fn test_click_event_minimal() {
    let click = ClickEvent::new(MouseButton::Left, ClickKind::Down, 10, 20);
    let json = serde_json::to_string(&click).unwrap();
    assert!(json.contains("\"button\":\"left\""));
    assert!(json.contains("\"kind\":\"down\""));
    assert!(json.contains("\"column\":10"));
    assert!(json.contains("\"row\":20"));
    // No modifiers should be serialized (skipped)
    assert!(!json.contains("\"modifiers\""));
}

#[test]
fn test_click_event_moved() {
    let moved = ClickEvent::moved(5, 15);
    let json = serde_json::to_string(&moved).unwrap();
    assert!(json.contains("\"kind\":\"moved\""));
    // button should be skipped (None)
    assert!(!json.contains("\"button\""));
}

#[test]
fn test_click_event_helpers() {
    let down = ClickEvent::new(MouseButton::Left, ClickKind::Down, 0, 0);
    assert!(down.is_down());
    assert!(!down.is_up());

    let up = ClickEvent::new(MouseButton::Left, ClickKind::Up, 0, 0);
    assert!(up.is_up());

    let drag = ClickEvent::drag(MouseButton::Left, 10, 20);
    assert!(drag.is_drag());
    assert_eq!(drag.button, Some(MouseButton::Left));
    assert_eq!(drag.column, 10);
    assert_eq!(drag.row, 20);

    let moved = ClickEvent::moved(0, 0);
    assert!(moved.is_moved());
}

#[test]
fn test_scroll_direction_serialization() {
    assert_eq!(serde_json::to_string(&ScrollDirection::Up).unwrap(), "\"up\"");
    assert_eq!(serde_json::to_string(&ScrollDirection::Down).unwrap(), "\"down\"");
    assert_eq!(serde_json::to_string(&ScrollDirection::Left).unwrap(), "\"left\"");
    assert_eq!(serde_json::to_string(&ScrollDirection::Right).unwrap(), "\"right\"");
}

#[test]
fn test_scroll_event_serialization() {
    let scroll = ScrollEvent::new(ScrollDirection::Down, 15, 25);
    let json = serde_json::to_string(&scroll).unwrap();
    assert!(json.contains("\"direction\":\"down\""));
    assert!(json.contains("\"column\":15"));
    assert!(json.contains("\"row\":25"));
    assert!(!json.contains("\"modifiers\""));
}

#[test]
fn test_click_event_roundtrip() {
    let click = ClickEvent::new(MouseButton::Middle, ClickKind::Down, 42, 13);
    let json = serde_json::to_string(&click).unwrap();
    let decoded: ClickEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.button, Some(MouseButton::Middle));
    assert_eq!(decoded.kind, ClickKind::Down);
    assert_eq!(decoded.column, 42);
    assert_eq!(decoded.row, 13);
}

#[test]
fn test_click_event_with_modifiers() {
    let click = ClickEvent {
        button: Some(MouseButton::Left),
        kind: ClickKind::Down,
        column: 0,
        row: 0,
        modifiers: Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    };
    let json = serde_json::to_string(&click).unwrap();
    assert!(json.contains("\"modifiers\""));
    assert!(json.contains("\"ctrl\":true"));
}

#[test]
fn test_scroll_event_roundtrip() {
    let scroll = ScrollEvent::new(ScrollDirection::Left, 10, 20);
    let json = serde_json::to_string(&scroll).unwrap();
    let decoded: ScrollEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.direction, ScrollDirection::Left);
    assert_eq!(decoded.column, 10);
    assert_eq!(decoded.row, 20);
}

#[test]
fn test_scroll_event_with_modifiers() {
    let scroll = ScrollEvent {
        direction: ScrollDirection::Right,
        column: 0,
        row: 0,
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::NONE
        },
    };
    let json = serde_json::to_string(&scroll).unwrap();
    assert!(json.contains("\"shift\":true"));
}

#[test]
fn test_scroll_direction_deserialization() {
    let dir: ScrollDirection = serde_json::from_str("\"left\"").unwrap();
    assert_eq!(dir, ScrollDirection::Left);
    let dir: ScrollDirection = serde_json::from_str("\"right\"").unwrap();
    assert_eq!(dir, ScrollDirection::Right);
}

#[test]
fn test_mouse_button_deserialization() {
    let btn: MouseButton = serde_json::from_str("\"middle\"").unwrap();
    assert_eq!(btn, MouseButton::Middle);
}

#[test]
fn test_click_kind_deserialization() {
    let kind: ClickKind = serde_json::from_str("\"drag\"").unwrap();
    assert_eq!(kind, ClickKind::Drag);
    let kind: ClickKind = serde_json::from_str("\"moved\"").unwrap();
    assert_eq!(kind, ClickKind::Moved);
}

#[test]
fn test_click_event_drag_constructor() {
    let drag = ClickEvent::drag(MouseButton::Right, 30, 40);
    assert!(drag.is_drag());
    assert_eq!(drag.button, Some(MouseButton::Right));
    assert_eq!(drag.column, 30);
    assert_eq!(drag.row, 40);
    assert!(drag.modifiers.is_empty());
}
