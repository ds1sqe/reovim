//! Binding metadata for which-key and help systems.
//!
//! [`BindingInfo`] carries the command ID alongside human-readable metadata
//! (category, description, binding layer) so consumers like `WhichKeyBridge`
//! can filter and display rich information.
//!
//! # Architecture (#459)
//!
//! ```text
//! KeybindingRegistration  ← module declares category/description
//!   → KeymapRegistry      ← stores BindingInfo per entry
//!   → bindings_with_prefix ← returns Vec<(KeySequence, BindingInfo)>
//!   → PendingBindings      ← carries metadata to bridges
//!   → WhichKeyBridge       ← filters + renders with metadata
//! ```

use {crate::BindingLayer, reovim_kernel::api::v1::CommandId};

/// Metadata about a keybinding returned by `bindings_with_prefix()`.
///
/// Populated during bootstrap from [`KeybindingRegistration`] fields
/// and carried through the pipeline to consumers like `WhichKeyBridge`.
///
/// [`KeybindingRegistration`]: reovim_kernel::api::module::KeybindingRegistration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingInfo {
    /// The command this binding triggers.
    pub command: CommandId,
    /// Human-readable description (e.g., "Focus window to the left").
    pub description: &'static str,
    /// Category for grouping/filtering (e.g., "motion", "operator", "window").
    pub category: Option<&'static str>,
    /// The layer this binding was registered at.
    pub layer: BindingLayer,
}

impl BindingInfo {
    /// Create a new `BindingInfo` with all fields.
    #[must_use]
    pub const fn new(
        command: CommandId,
        description: &'static str,
        category: Option<&'static str>,
        layer: BindingLayer,
    ) -> Self {
        Self {
            command,
            description,
            category,
            layer,
        }
    }

    /// Create a `BindingInfo` with only a command, using defaults for metadata.
    ///
    /// Convenience for tests and programmatic bindings that don't carry
    /// human-readable metadata.
    #[must_use]
    pub const fn from_command(command: CommandId, layer: BindingLayer) -> Self {
        Self {
            command,
            description: "",
            category: None,
            layer,
        }
    }
}
