use reovim_kernel::api::v1::{ModeId, ModuleId};

use super::*;

#[test]
fn get_returns_none_when_not_set() {
    let provider = InitialModeProvider::new();
    assert!(provider.get().is_none());
}

#[test]
fn set_and_get_roundtrip() {
    let provider = InitialModeProvider::new();
    let mode = ModeId::new(ModuleId::new("vim"), "normal");
    provider.set(mode.clone());
    let result = provider.get().expect("should be set");
    assert_eq!(result, mode);
}

#[test]
fn multiple_sets_last_writer_wins() {
    let provider = InitialModeProvider::new();
    let vim_mode = ModeId::new(ModuleId::new("vim"), "normal");
    let emacs_mode = ModeId::new(ModuleId::new("emacs"), "default");

    let prev = provider.set(vim_mode.clone());
    assert!(prev.is_none());

    let prev = provider.set(emacs_mode.clone());
    assert_eq!(prev.expect("should return previous"), vim_mode);

    let result = provider.get().expect("should be set");
    assert_eq!(result, emacs_mode);
}

#[test]
fn default_creates_empty_provider() {
    let provider = InitialModeProvider::default();
    assert!(provider.get().is_none());
}
