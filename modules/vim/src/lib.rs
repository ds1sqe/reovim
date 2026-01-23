//! Vim policy module.
//!
//! This module provides Vim-style behavior for reovim:
//! - **Keybindings**: Standard Vim keys (hjkl, operators, modes)
//! - **Operators**: Vim operators (d, y, c) - delete, yank, change
//! - **Resolvers**: Mode-specific key interpretation (counts, registers)
//! - **Visual mode**: Entry, exit, manipulation, operators
//! - **Policy**: Vim lookup behavior (wait for longer sequences)
//!
//! # Architecture
//!
//! This is a **POLICY** module - it defines HOW the editor behaves.
//! The kernel and drivers provide the mechanisms (WHAT can be done).
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  VIM POLICY MODULE (this module)           POLICY       │
//! │  → Vim keybindings (hjkl, dd, etc.)                     │
//! │  → Vim operators (d, y, c with motions)                 │
//! │  → Vim resolvers (count/register handling)              │
//! │  → Visual mode commands (selection operations)          │
//! │  → Vim behavior (wait for longer sequences)             │
//! ├─────────────────────────────────────────────────────────┤
//! │  MECHANISM MODULES                         CAPABILITIES │
//! │  editor/, keymap/, motions/                             │
//! │  → "What operations are possible"                       │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_vim::VimModule;
//!
//! let module = VimModule::new();
//! let bindings = module.keybindings();
//! // Returns ~180 Vim keybindings
//! ```

use {
    reovim_driver_command::{CommandHandler, CommandProvider},
    reovim_kernel::api::v1::*,
};

pub mod bindings;
pub mod commands;
pub mod fallback;
pub mod ids;
pub mod modes;
pub mod operators;
pub mod resolvers;
pub mod session_state;
pub mod visual;

#[cfg(test)]
mod registry_integration;

// Re-export mode types (Epic #372 - Mode Ownership)
pub use modes::{VIM_MODULE, VimMode};

// Re-export session state (Epic #385 - Server Simplification)
// PendingOperator is deprecated but still exported for backward compatibility
#[allow(deprecated)]
pub use session_state::{LastFind, PendingCharOp, PendingOperator, VimSessionState};

// Re-export OperatorId (Epic #385 - operators are vim policy, not kernel mechanism)
pub use ids::{CHANGE, DELETE, OperatorId, YANK};

// Re-export operators (Epic #385 - operators are vim-specific, merged from operators module)
pub use operators::{
    ChangeCommand, ChangeOperator, DeleteCommand, DeleteOperator, Operator, OperatorContext,
    OperatorError, Range, YankCommand, YankOperator, operator_commands,
};

// Re-export fallback handler (Epic #372 - Mode Ownership)
pub use fallback::VimFallbackHandler;

// Re-export mode commands (Epic #372 - Mode Ownership)
pub use commands::{
    ChangeLine, ChangeToEndOfLine, EnterCommandLineMode, EnterInsertEndOfLine,
    EnterInsertFirstNonBlank, EnterInsertMode, EnterInsertModeAppend, EnterWindowMode,
    ExecuteFindChar, ExitCommandLineMode, ExitOperatorPending, ExitToNormal, OpenLineAbove,
    OpenLineBelow,
};

// Re-export resolvers
#[allow(deprecated)]
pub use resolvers::VimOperatorPendingResolver;
pub use resolvers::{
    VimChangeResolver, VimDeleteResolver, VimInsertResolver, VimNormalResolver, VimYankResolver,
};

// Re-export visual mode commands
pub use visual::{
    // Operators
    ChangeSelection,
    DedentSelection,
    DeleteSelection,
    // Entry
    EnterVisualBlockMode,
    EnterVisualLineMode,
    EnterVisualMode,
    // Exit
    ExitVisualMode,
    IndentSelection,
    // Manipulation
    ReselectLast,
    SwapAnchor,
    ToggleVisualBlock,
    ToggleVisualChar,
    ToggleVisualLine,
    YankSelection,
    // Helper functions
    visual_commands,
    visual_entry_commands,
    visual_exit_commands,
    visual_operator_commands,
    visual_selection_commands,
};

/// Vim policy module.
///
/// Provides standard Vim keybindings and behavior.
/// This module is stateless - all state is managed by the kernel.
pub struct VimModule;

impl VimModule {
    /// Create a new Vim module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("vim")
    }

    fn name(&self) -> &'static str {
        "Vim"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        pr_info!("Vim module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Vim module exiting");
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        bindings::all()
    }
}

impl CommandProvider for VimModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        let mut handlers = Vec::new();
        handlers.extend(commands::mode_commands());
        handlers.extend(visual::visual_commands());
        handlers.extend(operators::operator_commands()); // Epic #415
        handlers
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vim_module_id() {
        let module = VimModule::new();
        assert_eq!(module.id().as_str(), "vim");
    }

    #[test]
    fn test_vim_module_name() {
        let module = VimModule::new();
        assert_eq!(module.name(), "Vim");
    }

    #[test]
    fn test_vim_module_version() {
        let module = VimModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
    }

    #[test]
    fn test_vim_keybindings_not_empty() {
        let module = VimModule::new();
        let bindings = module.keybindings();
        assert!(!bindings.is_empty(), "Vim module should provide keybindings");
        // Should have at least 100 bindings across all modes
        assert!(bindings.len() > 100, "Vim module should have many keybindings");
    }

    #[test]
    fn test_normal_mode_bindings() {
        let bindings = bindings::normal::bindings();
        assert!(!bindings.is_empty(), "Normal mode should have bindings");
        // Verify basic navigation keys exist
        assert!(bindings.iter().any(|b| b.keys == "h"), "Normal mode should have 'h' binding");
        assert!(bindings.iter().any(|b| b.keys == "j"), "Normal mode should have 'j' binding");
        assert!(bindings.iter().any(|b| b.keys == "k"), "Normal mode should have 'k' binding");
        assert!(bindings.iter().any(|b| b.keys == "l"), "Normal mode should have 'l' binding");
    }

    #[test]
    fn test_insert_mode_bindings() {
        let bindings = bindings::insert::bindings();
        assert!(!bindings.is_empty(), "Insert mode should have bindings");
        // Verify escape key exists
        assert!(
            bindings.iter().any(|b| b.keys == "<Esc>"),
            "Insert mode should have Escape binding"
        );
    }

    #[test]
    fn test_visual_mode_bindings() {
        let bindings = bindings::visual::bindings();
        assert!(!bindings.is_empty(), "Visual mode should have bindings");
    }

    #[test]
    fn test_operator_pending_mode_bindings() {
        let bindings = bindings::operator_pending::bindings();
        assert!(!bindings.is_empty(), "Operator-pending mode should have bindings");
    }

    #[test]
    fn test_commandline_mode_bindings() {
        let bindings = bindings::commandline::bindings();
        assert!(!bindings.is_empty(), "Commandline mode should have bindings");
    }

    #[test]
    fn test_all_bindings_aggregation() {
        let all = bindings::all();
        let normal = bindings::normal::bindings();
        let insert = bindings::insert::bindings();
        let visual = bindings::visual::bindings();
        let op_pending = bindings::operator_pending::bindings();
        let operator_modes = bindings::operator_modes::all_operator_bindings();
        let cmdline = bindings::commandline::bindings();

        let expected_total = normal.len()
            + insert.len()
            + visual.len()
            + op_pending.len()
            + operator_modes.len()
            + cmdline.len();
        assert_eq!(all.len(), expected_total, "all() should aggregate all mode bindings");
    }
}
