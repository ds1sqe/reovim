use {
    crate::{KIND_KEY, KIND_MOUSE, KIND_SCROLL, TuiInputCodecModule},
    reovim_kernel::api::v1::{Module, ModuleContext},
    reovim_subsys_input::{DefaultInputCodecRegistry, InputCodecRegistry},
    std::sync::Arc,
};

fn shared_registry(ctx: &ModuleContext) -> Arc<DefaultInputCodecRegistry> {
    ctx.services.get_or_create::<DefaultInputCodecRegistry>()
}

#[test]
fn init_registers_three_tui_codecs() {
    let ctx = ModuleContext::default();
    let registry = shared_registry(&ctx);
    assert!(registry.get(KIND_KEY).is_none());
    assert!(registry.get(KIND_MOUSE).is_none());
    assert!(registry.get(KIND_SCROLL).is_none());

    let mut module = TuiInputCodecModule::new();
    let _ = module.init(&ctx);

    assert!(registry.get(KIND_KEY).is_some());
    assert!(registry.get(KIND_MOUSE).is_some());
    assert!(registry.get(KIND_SCROLL).is_some());
}

#[test]
fn exit_unregisters_three_tui_codecs() {
    let ctx = ModuleContext::default();
    let registry = shared_registry(&ctx);
    let mut module = TuiInputCodecModule::new();
    let _ = module.init(&ctx);

    assert!(registry.get(KIND_KEY).is_some());
    assert!(registry.get(KIND_MOUSE).is_some());
    assert!(registry.get(KIND_SCROLL).is_some());

    module.exit().expect("exit succeeds");

    assert!(registry.get(KIND_KEY).is_none());
    assert!(registry.get(KIND_MOUSE).is_none());
    assert!(registry.get(KIND_SCROLL).is_none());
}

#[test]
fn exit_without_init_is_noop() {
    let mut module = TuiInputCodecModule::new();
    module.exit().expect("exit without init succeeds");
}

#[test]
fn double_exit_is_safe() {
    let ctx = ModuleContext::default();
    let mut module = TuiInputCodecModule::new();
    let _ = module.init(&ctx);
    module.exit().expect("first exit succeeds");
    module.exit().expect("second exit is safe");
}
