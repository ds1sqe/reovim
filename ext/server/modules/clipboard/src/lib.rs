#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Clipboard module for reovim.
//!
//! Provides system clipboard access via `arboard`.
//!
//! # Architecture (#515 Phase 4)
//!
//! Following the mechanism/policy separation:
//! - **Mechanism**: `ClipboardProvider` trait (in `reovim-subsys-clipboard`)
//! - **Policy**: `ClipboardService` (this module) provides OS clipboard implementation
//!
//! History (numbered registers 0-9) has been moved to per-client `HistoryRing`
//! in `EditingState`, managed by `SessionRuntime::push_to_clipboard_history`.
//!
//! # Register Mapping
//!
//! | Register | Source | Description |
//! |----------|--------|-------------|
//! | `""` | Per-client `RegisterBank` | Unnamed (default) |
//! | `a-z` | Per-client `RegisterBank` | Named registers |
//! | `+` | This module (OS clipboard) | System clipboard |
//! | `*` | This module (OS selection) | Selection (X11 primary) |
//! | `0-9` | Per-client `HistoryRing` | Yank history |

mod service;

pub use service::ClipboardService;

use std::sync::Arc;

use {
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version, pr_info,
    },
    reovim_subsys_clipboard::{ClipboardKey, ClipboardProviderRegistry},
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

    fn provides(&self) -> &[&'static str] {
        &[reovim_subsys_clipboard::capabilities::CLIPBOARD_PROVIDER]
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
#[path = "lib_tests.rs"]
mod tests;
