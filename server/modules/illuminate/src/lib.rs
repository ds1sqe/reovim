#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Illuminate module for reovim.
//!
//! Highlights all references to the symbol under the cursor throughout
//! the visible buffer, using LSP `textDocument/documentHighlight` when
//! available with word-match fallback.
//!
//! Provides `]]` / `[[` navigation between highlighted references.

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod bridge;
pub mod commands;
pub mod ids;
pub mod state;

pub use state::{HighlightKind, HighlightRange, IlluminateState};

/// Illuminate module.
///
/// Registers the illuminate bridge for cursor-hold highlight detection
/// and navigation commands for jumping between references.
pub struct IlluminateModule;

impl IlluminateModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for IlluminateModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for IlluminateModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Illuminate"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &["illuminate"]
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register bridge for state serialization and tick-based cursor hold
        let bridge_provider = ctx.services.get_or_create::<BridgeProvider>();
        bridge_provider.register(bridge::IlluminateBridge);

        // Register navigation commands
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::all_commands() {
            command_store.add(handler);
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(IlluminateModule);

#[cfg(test)]
mod lib_tests;
