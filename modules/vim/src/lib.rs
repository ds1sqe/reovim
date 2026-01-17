//! Vim policy module.
//!
//! This module provides Vim-style behavior for reovim:
//! - **Keybindings**: Standard Vim keys (hjkl, operators, modes)
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
//! │  → Vim behavior (wait for longer sequences)             │
//! ├─────────────────────────────────────────────────────────┤
//! │  MECHANISM MODULES                         CAPABILITIES │
//! │  editor/, keymap/, motions/, operators/                 │
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

use {reovim_kernel::api::v1::*, reovim_module_macros::declare_module};

pub mod bindings;

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

// Generate FFI entry points for dynamic loading
declare_module!(VimModule);

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
        let cmdline = bindings::commandline::bindings();

        let expected_total =
            normal.len() + insert.len() + visual.len() + op_pending.len() + cmdline.len();
        assert_eq!(all.len(), expected_total, "all() should aggregate all mode bindings");
    }
}
