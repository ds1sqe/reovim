use super::*;

#[test]
fn all_returns_bindings() {
    let bindings = all();
    #[cfg(feature = "vim-keybindings")]
    assert_eq!(bindings.len(), 1);
    #[cfg(not(feature = "vim-keybindings"))]
    assert!(bindings.is_empty());
}
