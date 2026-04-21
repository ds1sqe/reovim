//! Module entry point for `reovim-render-codec-tui`.
//!
//! Registers the [`CellGridRenderCodec`] with the render-codec registry
//! on `init()` and releases it on `exit()`. Codec lifetime equals module
//! lifetime per the Plan 14 design rule.

use {
    super::codec::CellGridRenderCodec,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_render_codec::{DefaultRenderCodecRegistry, KIND_CELL_GRID, RenderCodecRegistry},
    std::sync::Arc,
};

/// Module owning the cell-grid render codec.
#[derive(Default)]
pub struct TuiRenderCodecModule {
    registry: Option<Arc<DefaultRenderCodecRegistry>>,
}

impl TuiRenderCodecModule {
    /// Create a new instance with no registry bound yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { registry: None }
    }
}

impl Module for TuiRenderCodecModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("reovim-render-codec-tui")
    }

    fn name(&self) -> &'static str {
        "TUI Render Codec"
    }

    fn version(&self) -> Version {
        Version::new(0, 15, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<DefaultRenderCodecRegistry>();
        registry.register(Arc::new(CellGridRenderCodec::new()));
        self.registry = Some(registry);
        tracing::info!(
            "TuiRenderCodecModule: registered CellGridRenderCodec (kind={KIND_CELL_GRID:#06x})"
        );
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        if let Some(registry) = self.registry.take() {
            registry.unregister(KIND_CELL_GRID);
        }
        tracing::info!("TuiRenderCodecModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "module_tests.rs"]
mod tests;
