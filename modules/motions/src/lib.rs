//! Vim motions module - POLICY.
//!
//! This module implements motion commands that move the cursor:
//! - Word motions: `w`, `b`, `e`, `W`, `B`, `E`, `ge`, `gE`
//! - Line motions: `0`, `$`, `^`, `gg`, `G`
//! - Find-char motions: `f`, `F`, `t`, `T`, `;`, `,`
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

pub mod find_char;
pub mod line;
pub mod word;

use reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version};

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
        assert_eq!(cmds.len(), 5); // 0, $, ^, gg, G
    }

    #[test]
    fn test_find_char_commands_count() {
        let cmds = find_char::all_commands();
        assert_eq!(cmds.len(), 6); // f, F, t, T, ;, ,
    }
}
