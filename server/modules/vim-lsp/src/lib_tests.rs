use std::{path::PathBuf, sync::Arc};

use super::*;

#[test]
fn module_id() {
    let module = VimLspModule::new();
    assert_eq!(module.id().as_str(), "vim-lsp");
}

#[test]
fn module_name() {
    let module = VimLspModule::new();
    assert_eq!(module.name(), "Vim LSP Keybindings");
}

#[test]
fn module_version() {
    let module = VimLspModule::new();
    let v = module.version();
    assert_eq!((v.major, v.minor, v.patch), (0, 1, 0));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = VimLspModule::default();
    assert_eq!(module.id().as_str(), "vim-lsp");
}

#[test]
fn module_exit() {
    let mut module = VimLspModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn init_registers_keybindings() {
    use reovim_kernel::api::v1::ServiceRegistry;

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = VimLspModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let store = services.get::<KeybindingStore>();
    assert!(store.is_some());
}

#[test]
fn keybindings_count() {
    let module = VimLspModule::new();
    assert_eq!(module.keybindings().len(), 4);
}

#[test]
fn keybindings_keys() {
    let module = VimLspModule::new();
    let keys: Vec<_> = module.keybindings().iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"gd"));
    assert!(keys.contains(&"gr"));
    assert!(keys.contains(&"K"));
    assert!(keys.contains(&"<C-k>"));
}

#[test]
fn keybindings_normal_mode() {
    let module = VimLspModule::new();
    let normal_keys: Vec<_> = module
        .keybindings()
        .into_iter()
        .filter(|kb| kb.modes.contains(&"vim:normal"))
        .map(|kb| kb.keys)
        .collect();
    assert!(normal_keys.contains(&"gd"));
    assert!(normal_keys.contains(&"gr"));
    assert!(normal_keys.contains(&"K"));
}

#[test]
fn keybindings_insert_mode() {
    let module = VimLspModule::new();
    assert!(
        module
            .keybindings()
            .into_iter()
            .filter(|kb| kb.modes.contains(&"vim:insert"))
            .map(|kb| kb.keys)
            .any(|k| k == "<C-k>")
    );
}

#[test]
fn keybindings_have_lsp_category() {
    let module = VimLspModule::new();
    for kb in module.keybindings() {
        assert_eq!(kb.category, Some("lsp"));
    }
}

#[test]
fn keybindings_have_descriptions() {
    let module = VimLspModule::new();
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
