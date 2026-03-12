use std::{path::PathBuf, sync::Arc};

use super::*;

#[test]
fn module_id() {
    let module = VimRangeFinderModule::new();
    assert_eq!(module.id().as_str(), "vim-range-finder");
}

#[test]
fn module_name() {
    let module = VimRangeFinderModule::new();
    assert_eq!(module.name(), "Vim Range Finder Keybindings");
}

#[test]
fn module_version() {
    let module = VimRangeFinderModule::new();
    let v = module.version();
    assert_eq!((v.major, v.minor, v.patch), (0, 1, 0));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = VimRangeFinderModule::default();
    assert_eq!(module.id().as_str(), "vim-range-finder");
}

#[test]
fn module_exit() {
    let mut module = VimRangeFinderModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn init_registers_keybindings_and_parent_mode() {
    use reovim_kernel::api::v1::{CursorStyle, ModeId, ServiceRegistry};

    let services = Arc::new(ServiceRegistry::new());

    // vim:normal must exist (vim initializes before vim-range-finder)
    let modes = services.get_or_create::<ModeInfoStore>();
    modes.add(reovim_driver_input::ModeInfo {
        id: ModeId::new(ModuleId::new("vim"), "normal"),
        display_name: "NORMAL",
        cursor_style: CursorStyle::Block,
        accepts_char_input: false,
        has_selection: false,
        inherits_from: None,
        is_entry: true,
    });

    let ctx = test_module_context(services.clone());

    let mut module = VimRangeFinderModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let store = services.get::<KeybindingStore>();
    assert!(store.is_some());

    // Verify JumpParentMode was registered
    let parent = services.get::<JumpParentMode>();
    assert!(parent.is_some());
}

#[test]
fn keybindings_count() {
    let module = VimRangeFinderModule::new();
    assert_eq!(module.keybindings().len(), 6);
}

#[test]
fn keybindings_keys() {
    let module = VimRangeFinderModule::new();
    let keys: Vec<_> = module.keybindings().iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"s"));
    assert!(keys.contains(&"za"));
    assert!(keys.contains(&"zo"));
    assert!(keys.contains(&"zc"));
    assert!(keys.contains(&"zR"));
    assert!(keys.contains(&"zM"));
}

#[test]
fn keybindings_target_vim_normal() {
    let module = VimRangeFinderModule::new();
    for kb in module.keybindings() {
        assert!(kb.modes.contains(&"vim:normal"));
    }
}

#[test]
fn keybindings_have_categories() {
    let module = VimRangeFinderModule::new();
    let bindings = module.keybindings();
    assert_eq!(bindings[0].category, Some("jump"));
    for kb in &bindings[1..] {
        assert_eq!(kb.category, Some("folding"));
    }
}

#[test]
fn keybindings_have_descriptions() {
    let module = VimRangeFinderModule::new();
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
