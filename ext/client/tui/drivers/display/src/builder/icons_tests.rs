use super::*;

#[test]
fn test_mode_icons_not_empty() {
    assert!(!mode::NORMAL.is_empty());
    assert!(!mode::INSERT.is_empty());
    assert!(!mode::VISUAL.is_empty());
    assert!(!mode::COMMAND.is_empty());
}

#[test]
fn test_fallback_none_is_space() {
    assert_eq!(fallback::NONE, " ");
}
