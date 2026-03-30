use super::*;

#[test]
fn binding_count() {
    assert_eq!(keybindings().len(), 3);
}

#[test]
fn insert_mode_bindings() {
    let count = keybindings()
        .into_iter()
        .filter(|b| b.modes == ["vim:insert"])
        .count();
    assert_eq!(count, 1);
}

#[test]
fn normal_mode_bindings() {
    let count = keybindings()
        .into_iter()
        .filter(|b| b.modes == ["vim:normal"])
        .count();
    assert_eq!(count, 2);
}

#[test]
fn all_have_descriptions() {
    for binding in keybindings() {
        assert!(
            !binding.description.is_empty(),
            "binding {:?} missing description",
            binding.keys
        );
    }
}
