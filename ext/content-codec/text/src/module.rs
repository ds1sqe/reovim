//! Module entry point for `reovim-content-codec-text`.
//!
//! Registers the text-domain codec runtime with the kernel. This module
//! currently registers no `ContentCodec` — the text format codecs
//! (utf8, cjk, legacy, csv, hex, xxd, pdf, rlib, tar-gz, binary-struct)
//! live as sibling modules under `ext/content-codec/`. The text
//! carve-out provides the session extension (`CodecSessionState`) and
//! stale-check adapter that those format modules share.
//!
//! The module still holds a registry handle on init so a future text-
//! specific bundled codec (e.g. a plain-text default) can register via
//! the same lifecycle used by the sibling format modules.

use {
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleId, ProbeResult, Version},
    reovim_subsys_content_codec::DefaultContentCodecRegistry,
    std::sync::Arc,
};

/// Module owning the text-domain content codec runtime glue.
///
/// Holds an `Arc` to the content codec registry obtained during `init()`
/// so `exit()` can release it symmetrically — codec lifetime equals module
/// lifetime per the Plan 14 design rule.
#[derive(Default)]
pub struct TextContentCodecModule {
    registry: Option<Arc<DefaultContentCodecRegistry>>,
}

impl TextContentCodecModule {
    /// Create a new instance with no registry bound yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { registry: None }
    }
}

impl Module for TextContentCodecModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("reovim-content-codec-text")
    }

    fn name(&self) -> &'static str {
        "Text Content Codec"
    }

    fn version(&self) -> Version {
        Version::new(0, 15, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
        // No codec registered today — format modules register their own.
        self.registry = Some(registry);
        ProbeResult::Success
    }

    fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
        // Registration happens in init(); nothing to do here.
    }

    fn exit(&mut self) -> Result<(), reovim_kernel::api::v1::ModuleError> {
        // Drop the registry handle; no codec to unregister.
        self.registry.take();
        Ok(())
    }
}

#[cfg(test)]
#[path = "module_tests.rs"]
mod tests;
