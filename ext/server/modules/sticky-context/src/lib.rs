#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Sticky context viewport headers module for reovim.
//!
//! Pins 1-3 enclosing scope headers at the top of the viewport as the user
//! scrolls, similar to VS Code's sticky scroll. Consumes scope data from the
//! context module. Language-aware by inheritance.
//!
//! # Architecture
//!
//! ```text
//! ContextSessionState (from context module)
//!         |
//! StickyContextState (filters top N outermost scopes)
//!         |
//! StickyContextBridge --> JSON header rows --> TUI/web
//! ```

mod bridge;
mod state;

pub use {
    bridge::StickyContextBridge,
    state::{HeaderRow, StickyContextOptions, StickyContextState},
};

use {
    reovim_driver_text_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Sticky context viewport headers module.
///
/// Registers a `StickyContextBridge` for TUI/web clients.
pub struct StickyContextModule;

impl StickyContextModule {
    /// Create a new sticky context module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for StickyContextModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for StickyContextModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("sticky-context")
    }

    fn name(&self) -> &'static str {
        "Sticky Context"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let bridge_provider = ctx.services.get_or_create::<BridgeProvider>();
        bridge_provider.register(StickyContextBridge);

        tracing::info!("StickyContextModule: registered sticky context bridge");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("StickyContextModule: exiting");
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(StickyContextModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
