//! Module-level tests for `reovim-render-codec-tui`.

use {
    super::*,
    reovim_kernel::api::v1::{Module, ModuleContext, ProbeResult},
    reovim_subsys_render_codec::{
        DefaultRenderCodecRegistry, KIND_CELL_GRID, RenderCodecRegistry,
    },
};

fn test_ctx() -> ModuleContext {
    ModuleContext::default()
}

#[test]
fn module_id() {
    let m = TuiRenderCodecModule::new();
    assert_eq!(m.id().as_str(), "reovim-render-codec-tui");
}

#[test]
fn module_name() {
    let m = TuiRenderCodecModule::new();
    assert_eq!(m.name(), "TUI Render Codec");
}

#[test]
fn default_has_no_registry_handle() {
    let m = TuiRenderCodecModule::default();
    assert!(m.registry.is_none());
}

#[test]
fn init_acquires_registry_and_registers_codec() {
    let ctx = test_ctx();
    let mut m = TuiRenderCodecModule::new();
    assert!(matches!(m.init(&ctx), ProbeResult::Success));
    assert!(m.registry.is_some());

    let registry = ctx.services.get_or_create::<DefaultRenderCodecRegistry>();
    assert!(registry.get(KIND_CELL_GRID).is_some());
}

#[test]
fn exit_releases_registry_and_unregisters_codec() {
    let ctx = test_ctx();
    let mut m = TuiRenderCodecModule::new();
    let _ = m.init(&ctx);

    m.exit().expect("exit should succeed");
    assert!(m.registry.is_none());

    let registry = ctx.services.get_or_create::<DefaultRenderCodecRegistry>();
    assert!(registry.get(KIND_CELL_GRID).is_none());
}

#[test]
fn exit_without_init_is_noop() {
    let mut m = TuiRenderCodecModule::new();
    m.exit().expect("exit should succeed even without init");
    assert!(m.registry.is_none());
}

#[test]
fn double_exit_is_safe() {
    let ctx = test_ctx();
    let mut m = TuiRenderCodecModule::new();
    let _ = m.init(&ctx);
    m.exit().expect("first exit");
    m.exit().expect("second exit (idempotent)");
    assert!(m.registry.is_none());
}

#[test]
fn registered_codec_roundtrips_through_registry() {
    let ctx = test_ctx();
    let mut m = TuiRenderCodecModule::new();
    let _ = m.init(&ctx);

    let registry = ctx.services.get_or_create::<DefaultRenderCodecRegistry>();
    let codec = registry.get(KIND_CELL_GRID).expect("codec must be registered");
    let target = crate::CellGridRender::new(80, 24, 5, 5, vec![0x41, 0x42]);
    let body = codec.encode(&target).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<crate::CellGridRender>().unwrap(), target);
}
