use std::{path::PathBuf, sync::Arc};

use super::*;

#[test]
fn module_id() {
    let module = VimExplorerModule::new();
    assert_eq!(module.id().as_str(), "vim-explorer");
}

#[test]
fn module_name() {
    let module = VimExplorerModule::new();
    assert_eq!(module.name(), "Vim Explorer Keybindings");
}

#[test]
fn module_version() {
    let module = VimExplorerModule::new();
    let v = module.version();
    assert_eq!((v.major, v.minor, v.patch), (0, 1, 0));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = VimExplorerModule::default();
    assert_eq!(module.id().as_str(), "vim-explorer");
}

#[test]
fn module_exit() {
    let mut module = VimExplorerModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn init_registers_keybindings() {
    use reovim_kernel::api::v1::ServiceRegistry;

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = VimExplorerModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let store = services.get::<KeybindingStore>();
    assert!(store.is_some());
}

#[test]
fn keybindings_count() {
    let module = VimExplorerModule::new();
    assert_eq!(module.keybindings().len(), 1);
}

#[test]
fn keybindings_keys() {
    let module = VimExplorerModule::new();
    assert!(module.keybindings().iter().any(|kb| kb.keys == "<Space>e"));
}

#[test]
fn keybindings_target_vim_normal() {
    let module = VimExplorerModule::new();
    for kb in module.keybindings() {
        assert!(kb.modes.contains(&"vim:normal"));
    }
}

#[test]
fn keybindings_have_explorer_category() {
    let module = VimExplorerModule::new();
    for kb in module.keybindings() {
        assert_eq!(kb.category, Some("explorer"));
    }
}

#[test]
fn keybindings_have_descriptions() {
    let module = VimExplorerModule::new();
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
