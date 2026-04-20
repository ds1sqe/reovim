//! Module entry point for `reovim-surface-codec-pixel`.
//!
//! Registers the [`PixelCodec`] with the surface-codec registry on
//! `init()` and releases it on `exit()`. Codec lifetime equals module
//! lifetime per the Plan 14 design rule.

use {
    super::codec::PixelCodec,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_surface_codec::{
        DefaultSurfaceCodecRegistry, KIND_PIXEL_BUFFER, SurfaceCodecRegistry,
    },
    std::sync::Arc,
};

/// Module owning the pixel surface codec.
#[derive(Default)]
pub struct PixelSurfaceCodecModule {
    registry: Option<Arc<DefaultSurfaceCodecRegistry>>,
}

impl PixelSurfaceCodecModule {
    /// Create a new instance with no registry bound yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { registry: None }
    }
}

impl Module for PixelSurfaceCodecModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("reovim-surface-codec-pixel")
    }

    fn name(&self) -> &'static str {
        "Pixel Surface Codec"
    }

    fn version(&self) -> Version {
        Version::new(0, 15, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<DefaultSurfaceCodecRegistry>();
        registry.register(Arc::new(PixelCodec::new()));
        self.registry = Some(registry);
        tracing::info!(
            "PixelSurfaceCodecModule: registered PixelCodec (kind={KIND_PIXEL_BUFFER:#06x})"
        );
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        if let Some(registry) = self.registry.take() {
            registry.unregister(KIND_PIXEL_BUFFER);
        }
        tracing::info!("PixelSurfaceCodecModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "module_tests.rs"]
mod tests;
