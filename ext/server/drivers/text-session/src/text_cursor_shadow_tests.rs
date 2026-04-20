use super::*;

#[test]
fn invalid_shadow_is_default() {
    let shadow = TextCursorShadow::invalid();
    assert_eq!(shadow.buffer_id, BufferId::from_raw(0));
    assert_eq!(shadow.line, 0);
    assert_eq!(shadow.col, 0);
    assert!(!shadow.valid);
}

#[test]
fn update_sets_cursor_fields() {
    let mut shadow = TextCursorShadow::invalid();
    shadow.update(BufferId::from_raw(7), 11, 13);
    assert_eq!(shadow.buffer_id, BufferId::from_raw(7));
    assert_eq!(shadow.line, 11);
    assert_eq!(shadow.col, 13);
    assert!(shadow.valid);
}

#[test]
fn clear_restores_invalid_sentinel() {
    let mut shadow = TextCursorShadow::invalid();
    shadow.update(BufferId::from_raw(3), 4, 5);
    shadow.clear();
    assert_eq!(shadow, TextCursorShadow::invalid());
}

#[test]
fn valid_shadow_produces_non_sentinel_snapshot() {
    let mut shadow = TextCursorShadow::invalid();
    shadow.update(BufferId::from_raw(3), 4, 5);
    assert_ne!(shadow.cursor_snapshot(), CursorSnapshot::SENTINEL);
}

#[test]
fn snapshot_changes_with_position() {
    let mut shadow = TextCursorShadow::invalid();
    shadow.update(BufferId::from_raw(3), 4, 5);
    let a = shadow.cursor_snapshot();
    shadow.update(BufferId::from_raw(3), 4, 6);
    let b = shadow.cursor_snapshot();
    assert_ne!(a, b);
}
