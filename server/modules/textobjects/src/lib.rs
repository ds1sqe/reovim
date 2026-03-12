#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim text objects module - POLICY.
//!
//! This module implements text object commands for operator-pending mode:
//! - Word objects: `iw`, `aw`, `iW`, `aW`
//! - Quote objects: `i"`, `a"`, `i'`, `a'`, `` i` ``, `` a` ``
//! - Bracket objects: `i(`, `a(`, `i[`, `a[`, `i{`, `a{`, `i<`, `a<`
//! - Paragraph objects: `ip`, `ap`
//!
//! # Mechanism vs Policy
//!
//! - **Mechanism (Kernel)**: `TextObjectEngine::range()` computes text ranges
//! - **Policy (This Module)**: Commands wire keys to specific text objects
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_textobjects::word;
//!
//! // Get all word text object commands
//! let cmds = word::all_commands();
//! for cmd in &cmds {
//!     println!("{}: {}", cmd.id(), cmd.description());
//! }
//! ```

pub mod bracket;
pub mod ids;
pub mod paragraph;
pub mod quote;
pub mod word;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Module identifier for text objects module.
pub const TEXTOBJECTS_MODULE: ModuleId = ModuleId::new("textobjects");

/// Text objects module instance.
pub struct TextObjectsModule;

impl TextObjectsModule {
    /// Create a new text objects module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TextObjectsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TextObjectsModule {
    fn id(&self) -> ModuleId {
        TEXTOBJECTS_MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Text Objects"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Self-register commands (same pattern as EditorModule)
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

impl CommandProvider for TextObjectsModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        all_commands()
    }
}

/// Get all text object commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    let mut cmds = Vec::new();
    cmds.extend(word::all_commands());
    cmds.extend(quote::all_commands());
    cmds.extend(bracket::all_commands());
    cmds.extend(paragraph::all_commands());
    cmds
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(TextObjectsModule);

#[cfg(test)]
mod bracket_tests;
#[cfg(test)]
mod lib_tests;
#[cfg(test)]
mod paragraph_tests;
#[cfg(test)]
mod quote_tests;
#[cfg(test)]
mod word_tests;
