//! Module-level tests for `reovim-surface-codec-tui`.

use {
    super::*,
    reovim_kernel::api::v1::{Module, ModuleContext, ProbeResult},
    reovim_subsys_surface_codec::{
        DefaultSurfaceCodecRegistry, KIND_CELL_GRID, SurfaceCodecRegistry,
    },
};

fn test_ctx() -> ModuleContext {
    ModuleContext::default()
}

#[test]
fn module_id() {
    let m = TuiSurfaceCodecModule::new();
    assert_eq!(m.id().as_str(), "reovim-surface-codec-tui");
}

#[test]
fn module_name() {
    let m = TuiSurfaceCodecModule::new();
    assert_eq!(m.name(), "TUI Surface Codec");
}

#[test]
fn default_has_no_registry_handle() {
    let m = TuiSurfaceCodecModule::default();
    assert!(m.registry.is_none());
}

#[test]
fn init_acquires_registry_and_registers_codec() {
    let ctx = test_ctx();
    let mut m = TuiSurfaceCodecModule::new();
    assert!(matches!(m.init(&ctx), ProbeResult::Success));
    assert!(m.registry.is_some());

    let registry = ctx.services.get_or_create::<DefaultSurfaceCodecRegistry>();
    assert!(registry.get(KIND_CELL_GRID).is_some());
}

#[test]
fn exit_releases_registry_and_unregisters_codec() {
    let ctx = test_ctx();
    let mut m = TuiSurfaceCodecModule::new();
    let _ = m.init(&ctx);

    m.exit().expect("exit should succeed");
    assert!(m.registry.is_none());

    let registry = ctx.services.get_or_create::<DefaultSurfaceCodecRegistry>();
    assert!(registry.get(KIND_CELL_GRID).is_none());
}

#[test]
fn exit_without_init_is_noop() {
    let mut m = TuiSurfaceCodecModule::new();
    m.exit().expect("exit should succeed even without init");
    assert!(m.registry.is_none());
}

#[test]
fn double_exit_is_safe() {
    let ctx = test_ctx();
    let mut m = TuiSurfaceCodecModule::new();
    let _ = m.init(&ctx);
    m.exit().expect("first exit");
    m.exit().expect("second exit (idempotent)");
    assert!(m.registry.is_none());
}

#[test]
fn registered_codec_roundtrips_through_registry() {
    let ctx = test_ctx();
    let mut m = TuiSurfaceCodecModule::new();
    let _ = m.init(&ctx);

    let registry = ctx.services.get_or_create::<DefaultSurfaceCodecRegistry>();
    let codec = registry.get(KIND_CELL_GRID).expect("codec must be registered");
    let surface = crate::CellGridSurface::new(80, 24);
    let body = codec.encode(&surface).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<crate::CellGridSurface>().unwrap(), surface);
}
