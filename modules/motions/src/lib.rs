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
    reovim_driver_command::{CommandHandler, CommandProvider},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod find_char;
pub mod ids;
pub mod line;
pub mod search;
pub mod word;

/// Module identifier for motions module.
pub const MOTIONS_MODULE: ModuleId = ModuleId::new("motions");

/// Motions module instance.
pub struct MotionsModule;

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

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_trait() {
        let module = MotionsModule;
        assert_eq!(module.id().as_str(), "motions");
        assert_eq!(module.name(), "Vim Motions");
    }

    #[test]
    fn test_word_commands_count() {
        let cmds = word::all_commands();
        assert_eq!(cmds.len(), 8); // w, b, e, W, B, E, ge, gE
    }

    #[test]
    fn test_line_commands_count() {
        let cmds = line::all_commands();
        assert_eq!(cmds.len(), 6); // 0, $, ^, gg, G, whole-line
    }

    #[test]
    fn test_find_char_commands_count() {
        let cmds = find_char::all_commands();
        assert_eq!(cmds.len(), 6); // f, F, t, T, ;, ,
    }

    #[test]
    fn test_search_commands_count() {
        let cmds = search::all_commands();
        assert_eq!(cmds.len(), 7); // /, ?, n, N, *, #, :noh
    }
}
