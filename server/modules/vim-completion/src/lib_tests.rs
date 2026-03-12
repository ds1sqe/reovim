use std::{path::PathBuf, sync::Arc};

use super::*;

#[test]
fn module_id() {
    let module = VimCompletionModule::new();
    assert_eq!(module.id().as_str(), "vim-completion");
}

#[test]
fn module_name() {
    let module = VimCompletionModule::new();
    assert_eq!(module.name(), "Vim Completion Keybindings");
}

#[test]
fn module_version() {
    let module = VimCompletionModule::new();
    let v = module.version();
    assert_eq!((v.major, v.minor, v.patch), (0, 1, 0));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = VimCompletionModule::default();
    assert_eq!(module.id().as_str(), "vim-completion");
}

#[test]
fn module_exit() {
    let mut module = VimCompletionModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn init_registers_keybindings() {
    use reovim_kernel::api::v1::ServiceRegistry;

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = VimCompletionModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let store = services.get::<KeybindingStore>();
    assert!(store.is_some());
}

#[test]
fn keybindings_count() {
    let module = VimCompletionModule::new();
    assert_eq!(module.keybindings().len(), 2);
}

#[test]
fn keybindings_keys() {
    let module = VimCompletionModule::new();
    let keys: Vec<_> = module.keybindings().iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"<C-y>"));
    assert!(keys.contains(&"<C-e>"));
}

#[test]
fn keybindings_target_vim_insert() {
    let module = VimCompletionModule::new();
    for kb in module.keybindings() {
        assert!(kb.modes.contains(&"vim:insert"));
    }
}

#[test]
fn keybindings_have_completion_category() {
    let module = VimCompletionModule::new();
    for kb in module.keybindings() {
        assert_eq!(kb.category, Some("completion"));
    }
}

#[test]
fn keybindings_have_descriptions() {
    let module = VimCompletionModule::new();
    for kb in module.keybindings() {
        assert!(!kb.description.is_empty());
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_context(
    services: Arc<reovim_kernel::api::v1::ServiceRegistry>,
) -> ModuleContext {
    ModuleContext::new(
        reovim_kernel::api::v1::KernelContext::default(),
        services,
        PathBuf::from("/tmp"),
        PathBuf::from("/tmp"),
    )
}
