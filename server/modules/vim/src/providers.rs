//! Default mode provider for the Vim module.
//!
//! This module implements `DefaultModeProvider` to declare `vim:normal`
//! as the entry mode for reovim.

use std::sync::Arc;

use {
    reovim_driver_text_input::{DefaultModeProvider, ProviderPriority},
    reovim_kernel::api::v1::{ModeId, ModuleId},
};

/// Vim module's default mode provider.
///
/// Declares `vim:normal` as the entry mode with default priority.
/// This can be overridden by user configuration with higher priority.
pub struct VimDefaultModeProvider {
    /// Cached reference to `VIM_MODULE` for lifetime safety.
    _marker: (),
}

impl VimDefaultModeProvider {
    /// Create a new provider.
    #[must_use]
    pub const fn new() -> Self {
        Self { _marker: () }
    }
}

impl Default for VimDefaultModeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultModeProvider for VimDefaultModeProvider {
    fn provider_id(&self) -> &ModuleId {
        // Use static for stable address (const creates temporaries)
        static ID: ModuleId = ModuleId::new("vim");
        &ID
    }

    fn priority(&self) -> ProviderPriority {
        ProviderPriority::Default
    }

    fn entry_mode(&self) -> &ModeId {
        // Use static for stable address (const creates temporaries)
        static MODE: ModeId = ModeId::with_discriminant(ModuleId::new("vim"), "normal", 0);
        &MODE
    }
}

/// Marker struct for implementing `DefaultModeProviderModule` on `VimModule`.
///
/// This is used internally to provide the companion trait implementation.
pub struct VimModuleProviderExt;

impl VimModuleProviderExt {
    /// Get the default mode provider for the Vim module.
    #[must_use]
    pub fn default_mode_provider() -> Option<Arc<dyn DefaultModeProvider>> {
        Some(Arc::new(VimDefaultModeProvider::new()))
    }
}
