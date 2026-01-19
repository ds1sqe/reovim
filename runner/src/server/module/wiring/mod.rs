//! Registry integration subsystem for module handler wiring.
//!
//! This module provides infrastructure to wire module handlers (commands,
//! keybindings) to the session registries when modules are loaded.
//!
//! # Architecture
//!
//! ```text
//! wiring/
//! ├── commands.rs      # Command wiring for CommandProvider
//! └── keybindings.rs   # Keybinding wiring function
//! ```
//!
//! # Example
//!
//! ```ignore
//! use runner::module::wiring::{wire_module_keybindings, wire_module_commands};
//!
//! let module_id = ModuleId::new("my-module");
//!
//! // Wire keybindings
//! let keybindings = module.keybindings();
//! let result = wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry);
//! assert!(result.is_ok());
//!
//! // Wire commands if module implements CommandProvider
//! let result = wire_module_commands(&module_id, &module, &mut command_registry);
//! assert!(result.is_ok());
//! ```

mod commands;
mod keybindings;

// Re-exports - keybinding wiring
pub use keybindings::{WiringError, WiringResult, WiringStats, wire_module_keybindings};

// Re-exports - command wiring
pub use commands::{
    CommandWiringError, CommandWiringResult, CommandWiringStats, wire_module_commands,
};
