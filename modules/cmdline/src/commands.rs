//! Command line editing commands.
//!
//! This module defines command IDs for cmdline editing operations.
//!
//! # Phase 1 Architecture
//!
//! For Phase 1, cmdline editing keys (Backspace, Left, Right, etc.) are
//! handled directly by the input handler when cmdline is active, similar
//! to how `InsertChar` is handled. This avoids the complexity of routing
//! through the command system.
//!
//! The command IDs defined here are reserved for future use when:
//! - Cmdline editing needs to be customizable via keybindings
//! - Commands need to be invokable programmatically
//! - History navigation and completion are added
//!
//! # Future Architecture
//!
//! In later phases, these commands will be registered and callable:
//! ```ignore
//! // Keybinding registration
//! KeybindingRegistration::new("<BS>", cmdline::CMDLINE_BACKSPACE)
//!     .with_modes(&["vim:command"])
//!
//! // Command execution
//! runtime.execute_command(&cmdline::CMDLINE_BACKSPACE, &ctx)
//! ```

// Command IDs are defined in ids.rs
// Commands will be implemented in Phase 2 when command execution is needed

#[cfg(test)]
mod tests {
    use crate::ids;

    #[test]
    fn test_command_ids_defined() {
        // Verify command IDs are properly defined
        assert_eq!(ids::CMDLINE_BACKSPACE.name(), "backspace");
        assert_eq!(ids::CMDLINE_CURSOR_LEFT.name(), "cursor-left");
        assert_eq!(ids::CMDLINE_CURSOR_RIGHT.name(), "cursor-right");
    }
}
