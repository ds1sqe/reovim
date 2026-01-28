//! Interactor system for input routing
//!
//! Conceptual model:
//! ```text
//! User <-> Window <- Input Handler -> Buffer
//!          (view)    (focus-based)    (data)
//! ```
//!
//! Input routing is controlled by `ModeState.interactor_id` which identifies
//! the currently focused component. Built-in components (`Editor`, `CommandLine`)
//! are handled directly by the runtime. Plugin components receive input via
//! `PluginTextInput` and `PluginBackspace` events.
//!
//! ## Interactor Configuration
//!
//! Interactors can register their input behavior via [`InteractorConfig`] and
//! [`InteractorRegistry`]. This allows the mode system to determine whether
//! an interactor accepts character input without hardcoding component-specific
//! logic.

mod config;

pub use config::{InteractorConfig, InteractorRegistry};

#[cfg(test)]
mod tests {
    use crate::modd::ComponentId;

    #[test]
    fn test_component_id_equality() {
        assert_eq!(ComponentId::EDITOR, ComponentId("editor"));
        assert_ne!(ComponentId::EDITOR, ComponentId("explorer"));
    }
}
