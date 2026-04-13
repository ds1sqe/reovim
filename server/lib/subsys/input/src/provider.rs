//! Default mode provider trait for module-driven mode initialization.
//!
//! This module defines the `DefaultModeProvider` trait that modules can
//! implement to declare themselves as the source of the initial/entry mode.
//!
//! # Design
//!
//! - **Single initialization path**: Entry mode comes from provider registry
//! - **Panic fast**: If no mode provider registered, system panics at boot
//! - **Priority-based**: Multiple providers can coexist, highest priority wins
//!
//! # Example
//!
//! ```ignore
//! use reovim_subsys_input::{DefaultModeProvider, ProviderPriority};
//! use reovim_kernel::api::v1::{ModeId, ModuleId};
//!
//! struct VimDefaultModeProvider;
//!
//! impl DefaultModeProvider for VimDefaultModeProvider {
//!     fn provider_id(&self) -> &ModuleId {
//!         static ID: ModuleId = ModuleId::new("vim");
//!         &ID
//!     }
//!
//!     fn entry_mode(&self) -> &ModeId {
//!         static MODE: ModeId = ModeId::new(ModuleId::new("vim"), "normal");
//!         &MODE
//!     }
//! }
//! ```

use reovim_kernel::api::v1::{ModeId, ModuleId};

/// Priority for provider resolution when multiple providers exist.
///
/// Higher value = more preferred. Used only for ordering when multiple
/// providers register entry modes. NOT for fallback behavior.
///
/// # Panic Fast
///
/// If no mode provider is registered, the system panics at boot.
/// There is no graceful degradation for essential providers.
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
///
/// Modules implement this trait to provide the initial mode during
/// initialization. The runner collects all providers and uses
/// priority-based resolution to determine the entry mode.
///
/// # Panic Fast
///
/// If no default mode provider is registered at boot, the system panics
/// immediately. An entry mode is essential - there is no fallback.
///
/// # Typical Implementors
///
/// - `vim` module: provides `vim:normal` as entry mode
/// - Custom modules: can override with higher priority
pub trait DefaultModeProvider: Send + Sync {
    /// Module ID that provides this default.
    fn provider_id(&self) -> &ModuleId;

    /// Priority for this provider.
    ///
    /// When multiple providers are registered, the one with highest
    /// priority determines the entry mode.
    fn priority(&self) -> ProviderPriority {
        ProviderPriority::Default
    }

    /// The mode ID to use as the initial mode.
    ///
    /// This mode must be registered in the mode registry before
    /// the session starts.
    fn entry_mode(&self) -> &ModeId;
}
