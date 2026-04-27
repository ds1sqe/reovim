//! Module entry point for `reovim-content-codec-xxd`.
//!
//! Registers [`XxdCodec`] with the [`DefaultContentCodecRegistry`] on load
//! and unregisters it on unload. Codec lifetime equals module lifetime.

use {
    reovim_content_codec::ContentType,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleId, ProbeResult, Version},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
    std::sync::Arc,
};

use crate::XxdCodec;

/// Content type used to identify the standalone xxd hex-dump codec.
///
/// This content type is distinct from `ContentType::BINARY_RAW` (which the
/// `hex` module owns) so that both modules can coexist in the registry.
pub const XXD_CONTENT_TYPE: &str = "application/xxd-hex-dump";

/// Module that registers [`XxdCodec`] with the [`DefaultContentCodecRegistry`].
#[derive(Default)]
pub struct XxdContentCodecModule {
    registry: Option<Arc<DefaultContentCodecRegistry>>,
    content_type: Option<ContentType>,
}

impl XxdContentCodecModule {
    /// Create a new instance with no registry bound yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            registry: None,
            content_type: None,
        }
    }
}

impl Module for XxdContentCodecModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("reovim-content-codec-xxd")
    }

    fn name(&self) -> &'static str {
        "Xxd Content Codec"
    }

    fn version(&self) -> Version {
        Version::new(0, 15, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
        let content_type = ContentType::new(XXD_CONTENT_TYPE);
        registry.register(content_type.clone(), Arc::new(XxdCodec::new()));
        self.registry = Some(registry);
        self.content_type = Some(content_type);
        ProbeResult::Success
    }

    fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
        // Registration happens in init(); nothing to do here.
    }

    fn exit(&mut self) -> Result<(), reovim_kernel::api::v1::ModuleError> {
        if let (Some(registry), Some(ct)) = (self.registry.take(), self.content_type.take()) {
            registry.unregister(&ct);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "module_tests.rs"]
mod tests;
