//! Module entry point for `reovim-codec-tui-input`.
//!
//! Registers `TuiKeyCodec`, `TuiMouseCodec`, and `TuiScrollCodec` with the
//! `InputCodecRegistry` on load and unregisters them on unload.
//!
//! The registry is looked up from `ctx.services` as a
//! `DefaultInputCodecRegistry` — the concrete type registered by the server
//! runtime during bootstrap.

use {
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleId, ProbeResult, Version},
    reovim_module_macros::declare_module,
    reovim_subsys_input::{DefaultInputCodecRegistry, InputCodecRegistry},
    std::sync::Arc,
};

use crate::codecs::{KIND_KEY, KIND_MOUSE, KIND_SCROLL, tui_key_codec, tui_mouse_codec, tui_scroll_codec};

/// Module that registers TUI input codecs with the `InputCodecRegistry`.
///
/// The module holds an `Arc` to the registry obtained during `init()` so
/// that `exit()` can unregister its codecs symmetrically — codec lifetime
/// equals module lifetime.
#[derive(Default)]
pub struct TuiInputCodecModule {
    registry: Option<Arc<DefaultInputCodecRegistry>>,
}

impl TuiInputCodecModule {
    /// Create a new instance with no registry bound yet.
    #[must_use]
    pub fn new() -> Self {
        Self { registry: None }
    }
}

impl Module for TuiInputCodecModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("reovim-codec-tui-input")
    }

    fn name(&self) -> &'static str {
        "TUI Input Codec"
    }

    fn version(&self) -> Version {
        Version::new(0, 15, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx
            .services
            .get_or_create::<DefaultInputCodecRegistry>();
        register_codecs(registry.as_ref());
        self.registry = Some(registry);
        ProbeResult::Success
    }

    fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
        // Registration already done in init(); nothing to do here.
    }

    fn exit(&mut self) -> Result<(), reovim_kernel::api::v1::ModuleError> {
        if let Some(registry) = self.registry.take() {
            unregister_codecs(registry.as_ref());
        }
        Ok(())
    }
}

fn register_codecs(registry: &dyn InputCodecRegistry) {
    let key_arc: Arc<_> = tui_key_codec();
    let mouse_arc: Arc<_> = tui_mouse_codec();
    let scroll_arc: Arc<_> = tui_scroll_codec();
    registry.register(key_arc);
    registry.register(mouse_arc);
    registry.register(scroll_arc);
}

/// Unregister all three TUI codecs from the given registry.
///
/// This function is available for integration tests and server shutdown paths
/// that hold a direct reference to the registry.
pub fn unregister_codecs(registry: &dyn InputCodecRegistry) {
    registry.unregister(KIND_KEY);
    registry.unregister(KIND_MOUSE);
    registry.unregister(KIND_SCROLL);
}

declare_module!(TuiInputCodecModule);
