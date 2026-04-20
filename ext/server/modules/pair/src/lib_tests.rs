use reovim_kernel::api::v1::{Module, ModuleContext, ModuleId, Version};

use super::*;

#[test]
fn module_id() {
    let module = PairModule::new();
    assert_eq!(module.id(), ModuleId::new("pair"));
}

#[test]
fn module_name() {
    let module = PairModule::new();
    assert_eq!(module.name(), "Pair");
}

#[test]
fn module_version() {
    let module = PairModule::new();
    assert_eq!(module.version(), Version::new(0, 1, 0));
}

#[test]
fn module_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let module: PairModule = create_default();
    assert_eq!(module.id(), ModuleId::new("pair"));
}

#[test]
fn module_extension_kinds() {
    let module = PairModule::new();
    assert_eq!(module.extension_kinds(), &["pair"]);
}

#[test]
fn module_optional_dependencies() {
    let module = PairModule::new();
    let deps = module.optional_dependencies();
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&ModuleId::new("treesitter-rust")));
    assert!(deps.contains(&ModuleId::new("treesitter-markdown")));
}

#[test]
fn module_init_and_exit() {
    let mut module = PairModule::new();
    let ctx = ModuleContext::default();
    let result = module.init(&ctx);
    assert_eq!(result, reovim_kernel::api::v1::ProbeResult::Success);

    // Verify bridge was registered
    let provider = ctx.services.get::<BridgeProvider>();
    assert!(provider.is_some());

    assert!(module.exit().is_ok());
}
