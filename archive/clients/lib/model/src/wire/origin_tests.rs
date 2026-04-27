use super::*;

#[test]
fn buffer_position_constructor() {
    let origin = SemanticOrigin::buffer_position(1, 5, 12);
    assert_eq!(
        origin,
        SemanticOrigin::BufferPosition {
            buffer_id: 1,
            line: 5,
            col: 12,
        }
    );
}

#[test]
fn buffer_range_constructor() {
    let origin = SemanticOrigin::buffer_range(2, 5, 0, 7, 15);
    assert_eq!(
        origin,
        SemanticOrigin::BufferRange {
            buffer_id: 2,
            start_line: 5,
            start_col: 0,
            end_line: 7,
            end_col: 15,
        }
    );
}

#[test]
fn buffer_constructor() {
    let origin = SemanticOrigin::buffer(3);
    assert_eq!(origin, SemanticOrigin::Buffer { buffer_id: 3 });
}

#[test]
fn session_constructor() {
    let origin = SemanticOrigin::session();
    assert_eq!(origin, SemanticOrigin::Session);
}

#[test]
fn buffer_id_returns_id_for_position() {
    assert_eq!(SemanticOrigin::buffer_position(1, 0, 0).buffer_id(), Some(1));
}

#[test]
fn buffer_id_returns_id_for_range() {
    assert_eq!(SemanticOrigin::buffer_range(2, 0, 0, 1, 0).buffer_id(), Some(2));
}

#[test]
fn buffer_id_returns_id_for_buffer() {
    assert_eq!(SemanticOrigin::buffer(3).buffer_id(), Some(3));
}

#[test]
fn buffer_id_returns_none_for_session() {
    assert_eq!(SemanticOrigin::session().buffer_id(), None);
}

#[test]
fn is_session_true() {
    assert!(SemanticOrigin::session().is_session());
}

#[test]
fn is_session_false_for_buffer_position() {
    assert!(!SemanticOrigin::buffer_position(1, 0, 0).is_session());
}

#[test]
fn is_session_false_for_buffer_range() {
    assert!(!SemanticOrigin::buffer_range(1, 0, 0, 1, 0).is_session());
}

#[test]
fn is_session_false_for_buffer() {
    assert!(!SemanticOrigin::buffer(1).is_session());
}

#[test]
fn clone_and_debug() {
    let origin = SemanticOrigin::buffer_position(1, 5, 12);
    let cloned = origin.clone();
    assert_eq!(origin, cloned);
    let debug = format!("{origin:?}");
    assert!(debug.contains("BufferPosition"));
}

#[test]
fn serde_roundtrip_buffer_position() {
    let origin = SemanticOrigin::buffer_position(1, 5, 12);
    let json = serde_json::to_string(&origin).unwrap();
    let deserialized: SemanticOrigin = serde_json::from_str(&json).unwrap();
    assert_eq!(origin, deserialized);
}

#[test]
fn serde_roundtrip_buffer_range() {
    let origin = SemanticOrigin::buffer_range(2, 5, 0, 7, 15);
    let json = serde_json::to_string(&origin).unwrap();
    let deserialized: SemanticOrigin = serde_json::from_str(&json).unwrap();
    assert_eq!(origin, deserialized);
}

#[test]
fn serde_roundtrip_buffer() {
    let origin = SemanticOrigin::buffer(3);
    let json = serde_json::to_string(&origin).unwrap();
    let deserialized: SemanticOrigin = serde_json::from_str(&json).unwrap();
    assert_eq!(origin, deserialized);
}

#[test]
fn serde_roundtrip_session() {
    let origin = SemanticOrigin::session();
    let json = serde_json::to_string(&origin).unwrap();
    let deserialized: SemanticOrigin = serde_json::from_str(&json).unwrap();
    assert_eq!(origin, deserialized);
}
