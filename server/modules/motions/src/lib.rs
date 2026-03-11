#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim motions module - POLICY.
//!
//! This module implements motion commands that move the cursor:
//! - Word motions: `w`, `b`, `e`, `W`, `B`, `E`, `ge`, `gE`
//! - Line motions: `0`, `$`, `^`, `gg`, `G`
//! - Find-char motions: `f`, `F`, `t`, `T`, `;`, `,`
//! - Search motions: `/`, `?`, `n`, `N`, `*`, `#`, `:noh`
//!
//! # Mechanism vs Policy
//!
//! - **Mechanism (Kernel)**: `MotionEngine::calculate()` computes target positions
//! - **Policy (This Module)**: Commands wire keys to specific motions
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_motions::word;
//!
//! // Get all word motion commands
//! let cmds = word::all_commands();
//! for cmd in &cmds {
//!     println!("{}: {}", cmd.id(), cmd.description());
//! }
//! ```

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod find_char;
pub mod ids;
pub mod line;
pub mod search;
pub mod search_state;
pub mod word;

pub use search_state::SearchState;

/// Module identifier for motions module.
pub const MOTIONS_MODULE: ModuleId = ModuleId::new("motions");

/// Motions module instance.
pub struct MotionsModule;

impl MotionsModule {
    /// Create a new motions module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for MotionsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for MotionsModule {
    fn id(&self) -> ModuleId {
        MOTIONS_MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Motions"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Epic #417 Part 3: Self-register commands
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in self.command_handlers() {
            command_store.add(handler);
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

impl CommandProvider for MotionsModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        let mut handlers = Vec::new();
        handlers.extend(word::all_commands());
        handlers.extend(line::all_commands());
        handlers.extend(find_char::all_commands());
        handlers.extend(search::all_commands());
        handlers
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(MotionsModule);

#[cfg(test)]
mod find_char_tests;
#[cfg(test)]
mod lib_tests;
#[cfg(test)]
mod line_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod word_tests;
