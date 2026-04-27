use super::*;

#[test]
fn test_resize_event_serialization() {
    let resize = ResizeEvent::new(80, 24);
    let json = serde_json::to_string(&resize).unwrap();
    assert_eq!(json, r#"{"width":80,"height":24}"#);

    let decoded: ResizeEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.width, 80);
    assert_eq!(decoded.height, 24);
}

#[test]
fn test_focus_event_serialization() {
    let gained = FocusEvent::gained();
    let json = serde_json::to_string(&gained).unwrap();
    assert_eq!(json, r#"{"kind":"gained"}"#);

    let lost = FocusEvent::lost();
    let json = serde_json::to_string(&lost).unwrap();
    assert_eq!(json, r#"{"kind":"lost"}"#);
}

#[test]
fn test_paste_event_serialization() {
    let paste = PasteEvent::new("Hello, World!");
    let json = serde_json::to_string(&paste).unwrap();
    assert!(json.contains("\"content\":\"Hello, World!\""));

    let decoded: PasteEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.content, "Hello, World!");
}

#[test]
fn test_resize_event_roundtrip() {
    let resize = ResizeEvent::new(200, 50);
    let json = serde_json::to_string(&resize).unwrap();
    let decoded: ResizeEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, resize);
}

#[test]
fn test_focus_event_roundtrip() {
    let gained = FocusEvent::gained();
    let json = serde_json::to_string(&gained).unwrap();
    let decoded: FocusEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, gained);

    let lost = FocusEvent::lost();
    let json = serde_json::to_string(&lost).unwrap();
    let decoded: FocusEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, lost);
}

#[test]
fn test_paste_event_empty() {
    let paste = PasteEvent::new("");
    assert!(paste.content.is_empty());
}

#[test]
fn test_paste_event_multiline() {
    let paste = PasteEvent::new("line1\nline2\nline3");
    assert_eq!(paste.content, "line1\nline2\nline3");
}

#[test]
fn test_focus_kind_serialization() {
    assert_eq!(serde_json::to_string(&FocusKind::Gained).unwrap(), "\"gained\"");
    assert_eq!(serde_json::to_string(&FocusKind::Lost).unwrap(), "\"lost\"");
}

#[test]
fn test_focus_kind_deserialization() {
    let kind: FocusKind = serde_json::from_str("\"gained\"").unwrap();
    assert_eq!(kind, FocusKind::Gained);
    let kind: FocusKind = serde_json::from_str("\"lost\"").unwrap();
    assert_eq!(kind, FocusKind::Lost);
}

#[test]
fn test_resize_event_zero_size() {
    let resize = ResizeEvent::new(0, 0);
    assert_eq!(resize.width, 0);
    assert_eq!(resize.height, 0);
}
