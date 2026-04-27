use super::*;

#[test]
fn binding_count() {
    assert_eq!(keybindings().len(), 1);
}

#[test]
fn all_use_vim_normal_mode() {
    for binding in keybindings() {
        assert_eq!(
            binding.modes,
            &["vim:normal"],
            "binding {:?} should use vim:normal",
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
