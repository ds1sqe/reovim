use super::*;

#[test]
fn test_all_bindings_non_empty() {
    let bindings = all();
    assert!(!bindings.is_empty());
}

#[test]
fn test_bindings_use_emacs_mode() {
    for binding in all() {
        assert_eq!(binding.modes, EMACS_MODE, "binding {} should use emacs mode", binding.keys);
    }
}

#[test]
fn test_cursor_movement_bindings() {
    let bindings = all();
    let keys: Vec<&str> = bindings.iter().map(|b| b.keys).collect();
    assert!(keys.contains(&"<C-f>"));
    assert!(keys.contains(&"<C-b>"));
    assert!(keys.contains(&"<C-p>"));
    assert!(keys.contains(&"<C-n>"));
    assert!(keys.contains(&"<C-a>"));
    assert!(keys.contains(&"<C-e>"));
}
