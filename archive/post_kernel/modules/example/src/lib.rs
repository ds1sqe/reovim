//! Example module demonstrating the kernel-driver architecture.
//!
//! This module shows how to implement a complete vim-like mode system using:
//!
//! - **Kernel layer** (reovim-kernel): Mode identity and behavior, `CommandId`, `ModeStack`
//! - **Display driver** (reovim-driver-display): `ModeDisplay`, `CursorStyle`
//! - **Input driver** (reovim-driver-input): `KeySequence`, `Keybinding`
//! - **Command driver** (reovim-driver-command): Command, `CommandHandler`
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │  EXAMPLE MODULE (this crate)                                POLICY      │
//! │  ExampleMode (Normal, Insert), HelloCommand, QuitCommand, EchoCommand   │
//! │  → Decides HOW things behave                                            │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  COMMAND DRIVER                                             MECHANISM   │
//! │  Command, CommandHandler, CommandContext, ArgSpec, CommandResult        │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  INPUT DRIVER                                               MECHANISM   │
//! │  KeySequence, Keybinding                                                │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  DISPLAY DRIVER                                             MECHANISM   │
//! │  ModeDisplay, CursorStyle                                               │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  KERNEL                                                     MECHANISM   │
//! │  Mode trait (identity + accepts_char_input), ModeId, CommandId, ModeStack│
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use reovim_module_example::{EXAMPLE_MODULE, ExampleMode};
//! use reovim_module_example::command::{HelloCommand, QuitCommand};
//! use reovim_kernel::api::v1::{Mode, ModeStack};
//! use reovim_driver_display::ModeDisplay;
//!
//! // Create initial mode stack
//! let mode = ExampleMode::Normal;
//! let stack = ModeStack::new(mode.id());
//!
//! // Query mode properties from different layers
//! assert_eq!(mode.id().module(), &EXAMPLE_MODULE);        // Kernel: identity
//! assert_eq!(mode.cursor_style(), CursorStyle::Block);   // Display: rendering
//! assert!(!mode.accepts_char_input());                   // Kernel: behavior
//! ```

pub mod command;
pub mod mode;

// Re-exports
pub use {
    command::{EchoCommand, HelloCommand, QuitCommand},
    mode::ExampleMode,
};

use reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version};

/// The module identifier for this example module.
///
/// All modes and commands in this module are namespaced under this ID.
pub const EXAMPLE_MODULE: ModuleId = ModuleId::new("example");

/// Example module instance.
///
/// Demonstrates the kernel-driver architecture with sample modes and commands.
pub struct ExampleModule;

impl ExampleModule {
    /// Create a new example module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ExampleModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ExampleModule {
    fn id(&self) -> ModuleId {
        EXAMPLE_MODULE
    }

    fn name(&self) -> &'static str {
        "Example Module"
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

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ExampleModule);

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use {
        super::*, reovim_driver_command::Command, reovim_driver_display::ModeDisplay,
        reovim_kernel::api::v1::Mode,
    };

    #[test]
    fn test_example_module_id() {
        assert_eq!(EXAMPLE_MODULE.as_str(), "example");
    }

    #[test]
    fn test_mode_implements_all_traits() {
        let normal = ExampleMode::Normal;

        // Kernel: Mode trait (canonical source for identity + behavior)
        let _id = Mode::id(&normal);
        let _style = Mode::cursor_style(&normal);
        let _accepts = Mode::accepts_char_input(&normal);

        // Display driver: ModeDisplay trait (delegates to Mode)
        let _style = <ExampleMode as ModeDisplay>::cursor_style(&normal);
        let _text = <ExampleMode as ModeDisplay>::status_text(&normal);
    }

    #[test]
    fn test_command_implements_all_traits() {
        let cmd = HelloCommand;

        // Command driver: Command trait
        let _id = cmd.id();
        let _desc = cmd.description();
        let _args = cmd.args();
        let _names = cmd.names();
    }

    #[test]
    fn test_all_commands_belong_to_module() {
        assert_eq!(HelloCommand.id().module(), &EXAMPLE_MODULE);
        assert_eq!(QuitCommand.id().module(), &EXAMPLE_MODULE);
        assert_eq!(EchoCommand.id().module(), &EXAMPLE_MODULE);
    }

    #[test]
    fn test_all_modes_belong_to_module() {
        assert_eq!(ExampleMode::Normal.id().module(), &EXAMPLE_MODULE);
        assert_eq!(ExampleMode::Insert.id().module(), &EXAMPLE_MODULE);
    }
}
