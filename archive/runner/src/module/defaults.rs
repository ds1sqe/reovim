//! Default module registration.
//!
//! Registers the bundled policy modules (keymap, operators, commands, buffer-ops, etc.).
//!
//! # Architecture
//!
//! These modules implement the "policy" layer of the mechanism-vs-policy split:
//! - **keymap**: Vim keybindings (which keys trigger which actions)
//! - **operators**: Vim operators (d, y, c behavior)
//! - **commands**: Ex commands (:w, :q, :wq)
//! - **buffer-ops**: Buffer lifecycle event handlers
//! - **window-ops**: Window lifecycle event handlers
//! - **mode-manager**: Mode state transition handlers
//!
//! The kernel provides the mechanisms (motion calculation, buffer operations),
//! these modules decide HOW those mechanisms are used.

use {reovim_kernel::api::v1::ModuleError, reovim_module_defaults::DefaultsModule};

// Event handler modules (Phase 2 of runtime consolidation)
use {
    reovim_module_buffer_ops::BufferOps, reovim_module_mode_manager::ModeManager,
    reovim_module_window_ops::WindowOps,
};

use super::ModuleRegistry;

/// Register all default modules with the registry.
///
/// This registers the bundled policy modules that provide vim-like behavior:
/// - `keymap`: Standard vim keybindings
/// - `operators`: Vim operators (delete, yank, change)
/// - `commands`: Ex commands (write, quit)
/// - `defaults`: Bundle aggregator
/// - `buffer-ops`: Buffer lifecycle event handlers
/// - `window-ops`: Window lifecycle event handlers
/// - `mode-manager`: Mode state transition handlers
///
/// # Errors
///
/// Returns error if any module fails to register.
pub fn register_defaults(registry: &ModuleRegistry) -> Result<(), ModuleError> {
    // Register sub-modules first (dependencies)
    let modules = DefaultsModule::create_modules();
    for module in modules {
        // Use the module's ID for logging
        let id = module.id();
        registry.register_boxed(module)?;
        tracing::debug!(module = %id, "registered default module");
    }

    // Register the bundle module
    registry.register(DefaultsModule)?;

    // Register event handler modules (Phase 2 of runtime consolidation)
    // These modules subscribe to kernel EventBus events and coordinate
    // editor behavior across buffer, window, and mode state changes.
    registry.register(BufferOps::new())?;
    tracing::debug!("registered buffer-ops module");

    registry.register(WindowOps::new())?;
    tracing::debug!("registered window-ops module");

    registry.register(ModeManager::new())?;
    tracing::debug!("registered mode-manager module");

    tracing::info!("registered {} default modules", 7);

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{ModuleContext, ModuleId},
    };

    #[test]
    fn test_register_defaults() {
        let registry = ModuleRegistry::new();
        register_defaults(&registry).unwrap();

        // Verify all modules are registered
        let ids = registry.registered_ids();
        assert!(ids.iter().any(|id| id.as_str() == "keymap"));
        assert!(ids.iter().any(|id| id.as_str() == "operators"));
        assert!(ids.iter().any(|id| id.as_str() == "commands"));
        assert!(ids.iter().any(|id| id.as_str() == "defaults"));
        // Event handler modules (Phase 2)
        assert!(ids.iter().any(|id| id.as_str() == "buffer-ops"));
        assert!(ids.iter().any(|id| id.as_str() == "window-ops"));
        assert!(ids.iter().any(|id| id.as_str() == "mode-manager"));
    }

    #[test]
    fn test_init_defaults() {
        let registry = ModuleRegistry::new();
        register_defaults(&registry).unwrap();

        let ctx = ModuleContext::default();
        registry.init_all(&ctx).unwrap();

        // Verify modules are running
        assert!(registry.state(&ModuleId::new("keymap")).is_some());
        assert!(registry.state(&ModuleId::new("operators")).is_some());
        assert!(registry.state(&ModuleId::new("commands")).is_some());
    }
}
