use super::*;

#[test]
fn test_cursor_style_hint_default() {
    assert_eq!(CursorStyleHint::default(), CursorStyleHint::Block);
}
