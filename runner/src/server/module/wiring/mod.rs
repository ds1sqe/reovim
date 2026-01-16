//! Registry integration subsystem for module handler wiring.
//!
//! This module provides infrastructure to wire module handlers (commands,
//! keybindings) to the session registries when modules are loaded.
//!
//! # Architecture
//!
//! ```text
//! wiring/
//! └── keybindings.rs   # Keybinding wiring function
//! ```
//!
//! # Example
//!
//! ```ignore
//! use runner::module::wiring::wire_module_keybindings;
//!
//! let module_id = ModuleId::new("my-module");
//! let keybindings = module.keybindings();
//!
//! let result = wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry);
//! assert!(result.is_ok());
//! ```

mod keybindings;

// Re-exports
pub use keybindings::{WiringError, WiringResult, WiringStats, wire_module_keybindings};
