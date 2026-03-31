use super::*;

#[test]
fn binding_count() {
    assert_eq!(keybindings().len(), 1);
}

#[test]
fn uses_vim_normal_mode() {
    let binding = &keybindings()[0];
    assert_eq!(binding.modes, &["vim:normal"]);
}

#[test]
fn has_description() {
    let binding = &keybindings()[0];
    assert!(!binding.description.is_empty());
}
