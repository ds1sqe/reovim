use super::*;

#[test]
fn test_overlay_state_default() {
    let state = OverlayState::new();
    assert_eq!(state.selected_index, None);
    assert!(state.filter.is_empty());
    assert_eq!(state.scroll_offset, 0);
    assert!(!state.loading);
}

#[test]
fn test_overlay_state_with_selection() {
    let state = OverlayState::with_selection(5);
    assert_eq!(state.selected_index, Some(5));
}

#[test]
fn test_logical_overlay_new() {
    let overlay = LogicalOverlay::new("comp-1", "completion");
    assert_eq!(overlay.id, "comp-1");
    assert_eq!(overlay.kind, "completion");
    assert_eq!(overlay.origin, None);
    assert_eq!(overlay.priority, 0);
}

#[test]
fn test_logical_overlay_with_origin() {
    let origin = SemanticOrigin::buffer_position(1, 5, 12);
    let overlay = LogicalOverlay::new("hover-1", "hover").with_origin(origin.clone());
    assert_eq!(overlay.origin, Some(origin));
}

#[test]
fn test_logical_overlay_builder() {
    let overlay = LogicalOverlay::new("hover-1", "hover")
        .with_origin(SemanticOrigin::session())
        .with_data(serde_json::json!({"text": "Hello"}))
        .with_priority(10);

    assert_eq!(overlay.id, "hover-1");
    assert_eq!(overlay.priority, 10);
    assert_eq!(overlay.data["text"], "Hello");
    assert_eq!(overlay.origin, Some(SemanticOrigin::Session));
}

#[test]
fn test_overlay_serialization_roundtrip() {
    let data = serde_json::json!({
        "items": ["foo", "bar", "baz"],
        "count": 3
    });
    let overlay = LogicalOverlay::new("test", "completion")
        .with_origin(SemanticOrigin::buffer_position(1, 10, 3))
        .with_data(data.clone());

    // Verify data is preserved
    assert_eq!(overlay.data, data);
    assert_eq!(overlay.data["items"][0], "foo");
    assert_eq!(overlay.data["count"], 3);

    // Verify serde roundtrip
    let json = serde_json::to_string(&overlay).unwrap();
    let deserialized: LogicalOverlay = serde_json::from_str(&json).unwrap();
    assert_eq!(overlay, deserialized);
}

#[test]
fn test_overlay_serde_no_origin() {
    let overlay = LogicalOverlay::new("test", "notification");
    let json = serde_json::to_string(&overlay).unwrap();
    let deserialized: LogicalOverlay = serde_json::from_str(&json).unwrap();
    assert_eq!(overlay, deserialized);
    assert_eq!(deserialized.origin, None);
}
