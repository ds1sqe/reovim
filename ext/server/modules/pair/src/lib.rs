#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Bracket pair highlighting module for reovim.
//!
//! Provides rainbow bracket coloring, matched-pair highlighting, and
//! auto-pair insertion. Language-agnostic: reads per-language bracket
//! configs from [`BracketConfigStore`](reovim_driver_text_syntax::BracketConfigStore).
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-syntax      (BracketConfig, BracketConfigStore, SyntaxContext)
//!         ^
//!         |
//! reovim-module-pair         (THIS CRATE - Module, state, bridge)
//! ```
//!
//! # Features
//!
//! - **Rainbow brackets**: Stack-based depth coloring (`depth % N` cycling)
//! - **Matched-pair**: Innermost pair around cursor highlighted
//! - **Auto-pair**: Context-aware insertion (skips strings/comments)
//!
//! All three can be individually toggled via options:
//! `rainbow`, `autopair`, `matchpair`.

pub mod autopair;
mod bridge;
pub mod matched;
pub mod rainbow;
pub mod state;

use {
    reovim_driver_text_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

use crate::bridge::PairBridge;

/// Extension kind string.
const KIND: &str = "pair";
/// Module ID constant.
const MODULE_ID: &str = "pair";

/// Bracket pair highlighting module.
///
/// Registers [`PairBridge`] during `init()` so that bracket state
/// is streamed to TUI/web clients via the extension bridge system.
pub struct PairModule;

impl PairModule {
    /// Create a new pair module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PairModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PairModule {
    fn id(&self) -> ModuleId {
        ModuleId::new(MODULE_ID)
    }

    fn name(&self) -> &'static str {
        "Pair"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn optional_dependencies(&self) -> Vec<ModuleId> {
        // Treesitter modules are optional - pair works without them
        // (falls back to default bracket config)
        vec![
            ModuleId::new("treesitter-rust"),
            ModuleId::new("treesitter-markdown"),
        ]
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register bridge for client communication
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(PairBridge);

        tracing::info!("PairModule: registered pair extension bridge");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("PairModule: exiting");
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PairModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
