//! Module entry point for `reovim-surface-codec-tui`.
//!
//! Registers the [`CellGridCodec`] with the surface-codec registry on
//! `init()` and releases it on `exit()`. Codec lifetime equals module
//! lifetime per the Plan 14 design rule.

use {
    super::codec::CellGridCodec,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_surface_codec::{
        DefaultSurfaceCodecRegistry, KIND_CELL_GRID, SurfaceCodecRegistry,
    },
    std::sync::Arc,
};

/// Module owning the cell-grid surface codec.
#[derive(Default)]
pub struct TuiSurfaceCodecModule {
    registry: Option<Arc<DefaultSurfaceCodecRegistry>>,
}

impl TuiSurfaceCodecModule {
    /// Create a new instance with no registry bound yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { registry: None }
    }
}

impl Module for TuiSurfaceCodecModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("reovim-surface-codec-tui")
    }

    fn name(&self) -> &'static str {
        "TUI Surface Codec"
    }

    fn version(&self) -> Version {
        Version::new(0, 15, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<DefaultSurfaceCodecRegistry>();
        registry.register(Arc::new(CellGridCodec::new()));
        self.registry = Some(registry);
        tracing::info!("TuiSurfaceCodecModule: registered CellGridCodec (kind={KIND_CELL_GRID:#06x})");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        if let Some(registry) = self.registry.take() {
            registry.unregister(KIND_CELL_GRID);
        }
        tracing::info!("TuiSurfaceCodecModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "module_tests.rs"]
mod tests;
