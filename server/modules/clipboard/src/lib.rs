//! Clipboard module for reovim.
//!
//! Provides system clipboard access and yank history.
//!
//! # Architecture
//!
//! Following the mechanism/policy separation:
//! - **Mechanism**: `ClipboardProvider` trait (in `reovim-driver-clipboard`)
//! - **Policy**: `ClipboardService` (this module) provides implementation
//!
//! # Register Mapping
//!
//! | Register | Source | Description |
//! |----------|--------|-------------|
//! | `""` | Kernel `RegisterBank` | Unnamed (default) |
//! | `a-z` | Kernel `RegisterBank` | Named registers |
//! | `+` | This module | System clipboard |
//! | `*` | This module | Selection (X11 primary) |
//! | `0-9` | This module | Yank history |
//!
//! # Usage in Operators
//!
//! ```ignore
//! // After yanking/deleting:
//! let clipboard = ctx.services.get::<ClipboardProviderRegistry>();
//! if let Some(provider) = clipboard.and_then(|r| r.get(&ClipboardKey::Default)) {
//!     // Always push to history for 0-9 access
//!     provider.push_history(content.clone());
//!
//!     // Handle + and * registers
//!     match register {
//!         Some('+') => { provider.copy_to_clipboard(&content.text)?; }
//!         Some('*') => { provider.copy_to_selection(&content.text)?; }
//!         _ => {}
//!     }
//! }
//!
//! // For pasting from + or *:
//! match register {
//!     Some('+') => provider.paste_from_clipboard()?,
//!     Some('*') => provider.paste_from_selection()?,
//!     Some(n @ '0'..='9') => provider.get_numbered(n),
//!     _ => kernel.registers.read().get_by_name(register),
//! }
//! ```

mod history;
mod service;

pub use {history::HistoryRing, service::ClipboardService};

use std::sync::Arc;

use {
    reovim_driver_clipboard::{ClipboardKey, ClipboardProviderRegistry},
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version, pr_info,
    },
};

/// Clipboard module instance.
///
/// Provides system clipboard access and yank history.
pub struct ClipboardModule;

impl ClipboardModule {
    /// Create a new clipboard module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ClipboardModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ClipboardModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("clipboard")
    }

    fn name(&self) -> &'static str {
        "Clipboard"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register clipboard provider with typed key
        let clipboard_registry = ctx.services.get_or_create::<ClipboardProviderRegistry>();
        clipboard_registry.register(ClipboardKey::Default, Arc::new(ClipboardService::new()));

        pr_info!("Clipboard module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Clipboard module exiting");
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ClipboardModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = ClipboardModule::new();
        assert_eq!(module.id().as_str(), "clipboard");
    }

    #[test]
    fn test_module_name() {
        let module = ClipboardModule::new();
        assert_eq!(module.name(), "Clipboard");
    }

    #[test]
    fn test_module_version() {
        let module = ClipboardModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }
}
