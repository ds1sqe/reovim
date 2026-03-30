use super::*;

#[test]
fn binding_count() {
    assert_eq!(keybindings().len(), 2);
}

#[test]
fn all_use_vim_insert_mode() {
    for binding in keybindings() {
        assert_eq!(
            binding.modes,
            &["vim:insert"],
            "binding {:?} should use vim:insert",
            binding.keys
        );
    }
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
