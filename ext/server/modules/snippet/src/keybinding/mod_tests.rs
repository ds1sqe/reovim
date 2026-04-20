use super::*;

#[test]
fn all_returns_bindings() {
    let bindings = all();
    // With vim-keybindings feature (default on): 3 bindings
    #[cfg(feature = "vim-keybindings")]
    assert_eq!(bindings.len(), 3);
    // Without vim-keybindings feature: 0 bindings
    #[cfg(not(feature = "vim-keybindings"))]
    assert!(bindings.is_empty());
}
