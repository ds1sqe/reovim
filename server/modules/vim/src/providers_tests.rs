use std::sync::Arc;

use reovim_driver_input::{DefaultModeProvider, ProviderPriority};

use crate::providers::*;

#[test]
fn test_vim_default_mode_provider_id() {
    let provider = VimDefaultModeProvider::new();
    assert_eq!(provider.provider_id().as_str(), "vim");
}

#[test]
fn test_vim_default_mode_provider_priority() {
    let provider = VimDefaultModeProvider::new();
    assert_eq!(provider.priority(), ProviderPriority::Default);
}

#[test]
fn test_vim_default_mode_provider_entry_mode() {
    let provider = VimDefaultModeProvider::new();
    assert_eq!(provider.entry_mode().module().as_str(), "vim");
    assert_eq!(provider.entry_mode().name(), "normal");
}

#[test]
fn test_vim_default_mode_provider_default() {
    let provider = VimDefaultModeProvider::default();
    assert_eq!(provider.provider_id().as_str(), "vim");
}

#[test]
fn test_vim_module_provider_ext_returns_some() {
    let provider = VimModuleProviderExt::default_mode_provider();
    assert!(provider.is_some());
}

#[test]
fn test_vim_module_provider_ext_provider_id() {
    let provider = VimModuleProviderExt::default_mode_provider().unwrap();
    assert_eq!(provider.provider_id().as_str(), "vim");
}

#[test]
fn test_vim_module_provider_ext_priority() {
    let provider = VimModuleProviderExt::default_mode_provider().unwrap();
    assert_eq!(provider.priority(), ProviderPriority::Default);
}

#[test]
fn test_vim_module_provider_ext_entry_mode() {
    let provider = VimModuleProviderExt::default_mode_provider().unwrap();
    assert_eq!(provider.entry_mode().module().as_str(), "vim");
    assert_eq!(provider.entry_mode().name(), "normal");
}

#[test]
fn test_vim_default_mode_provider_const_new() {
    // Verify const fn construction works
    const PROVIDER: VimDefaultModeProvider = VimDefaultModeProvider::new();
    assert_eq!(PROVIDER.provider_id().as_str(), "vim");
}

#[test]
fn test_vim_module_provider_ext_arc_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    let provider = VimModuleProviderExt::default_mode_provider().unwrap();
    assert_send_sync::<Arc<dyn DefaultModeProvider>>();
    // Prove the actual provider is Send+Sync by moving it
    let _ = provider;
}

#[test]
fn test_vim_default_mode_provider_entry_mode_discriminant() {
    let provider = VimDefaultModeProvider::new();
    assert_eq!(provider.entry_mode().discriminant(), 0);
}

#[test]
fn test_vim_module_provider_ext_provider_entry_mode_discriminant() {
    let provider = VimModuleProviderExt::default_mode_provider().unwrap();
    assert_eq!(provider.entry_mode().discriminant(), 0);
}

#[test]
fn test_vim_default_mode_provider_provider_id_stable() {
    let provider = VimDefaultModeProvider::new();
    // Multiple calls should return the same reference
    let id1 = provider.provider_id();
    let id2 = provider.provider_id();
    assert_eq!(id1, id2);
}

#[test]
fn test_vim_default_mode_provider_entry_mode_stable() {
    let provider = VimDefaultModeProvider::new();
    let mode1 = provider.entry_mode();
    let mode2 = provider.entry_mode();
    assert_eq!(mode1, mode2);
}
