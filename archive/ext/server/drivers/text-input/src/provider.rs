//! Default mode provider trait for module-driven mode initialization.

use reovim_kernel::api::v1::{ModeId, ModuleId};

/// Priority for provider resolution when multiple providers exist.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ProviderPriority {
    /// Standard module provider (default).
    #[default]
    Default = 50,
    /// User configuration override (highest priority).
    Override = 100,
}

/// Trait for modules that declare entry modes.
pub trait DefaultModeProvider: Send + Sync {
    /// Module ID that provides this default.
    fn provider_id(&self) -> &ModuleId;

    /// Priority for this provider.
    fn priority(&self) -> ProviderPriority {
        ProviderPriority::Default
    }

    /// The mode ID to use as the initial mode.
    fn entry_mode(&self) -> &ModeId;
}
