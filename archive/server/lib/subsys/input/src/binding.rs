//! `BindingLayer` and `BindingInfo` — keybinding mechanism metadata.
//!
//! These are mechanism-layer types used by the server's `KeymapRegistry`.
//! They carry no domain-specific content: `BindingLayer` is a pure
//! priority enum and `BindingInfo` holds only kernel `CommandId` metadata.
//!
//! Moved here from `reovim-subsys-input-contracts` so the server can use
//! them without depending on domain-driver crates.

use reovim_kernel::api::v1::CommandId;

/// Binding layer for keybinding composition.
///
/// Higher layers override lower layers for the same key sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum BindingLayer {
    /// Base (mechanism defaults, rarely used).
    Base = 0,
    /// Policy (module-provided defaults).
    Policy = 1,
    /// User (user configuration overrides, highest priority).
    User = 2,
}

impl BindingLayer {
    /// Return all layers in ascending priority order.
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Base, Self::Policy, Self::User]
    }

    /// Return the display name of this layer.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Policy => "policy",
            Self::User => "user",
        }
    }
}

/// Metadata about a keybinding entry.
///
/// Returned by `bindings_with_prefix()` in the keymap registry to give
/// callers (e.g., which-key) access to description and category metadata
/// alongside the command and its layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingInfo {
    /// The command this binding triggers.
    pub command: CommandId,
    /// Human-readable description (empty string if not provided).
    pub description: &'static str,
    /// Optional category for grouping or filtering.
    pub category: Option<&'static str>,
    /// The layer at which this binding was registered.
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

    /// Create a `BindingInfo` with only a command and layer.
    ///
    /// Sets `description` to `""` and `category` to `None`.
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
