use super::*;

#[test]
fn all_returns_bindings() {
    let bindings = all();
    // With vim-keybindings feature (default on): 1 binding
    #[cfg(feature = "vim-keybindings")]
    assert_eq!(bindings.len(), 1);
    // Without vim-keybindings feature: 0 bindings
    #[cfg(not(feature = "vim-keybindings"))]
    assert!(bindings.is_empty());
}
