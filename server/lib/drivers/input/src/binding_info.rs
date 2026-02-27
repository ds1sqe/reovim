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

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    const TEST_MODULE: ModuleId = ModuleId::new("test");

    fn cmd(name: &'static str) -> CommandId {
        CommandId::new(TEST_MODULE, name)
    }

    #[test]
    fn test_binding_info_new() {
        let info = BindingInfo::new(
            cmd("cursor-down"),
            "Move cursor down",
            Some("motion"),
            BindingLayer::Policy,
        );
        assert_eq!(info.command, cmd("cursor-down"));
        assert_eq!(info.description, "Move cursor down");
        assert_eq!(info.category, Some("motion"));
        assert_eq!(info.layer, BindingLayer::Policy);
    }

    #[test]
    fn test_binding_info_from_command() {
        let info = BindingInfo::from_command(cmd("delete"), BindingLayer::User);
        assert_eq!(info.command, cmd("delete"));
        assert_eq!(info.description, "");
        assert!(info.category.is_none());
        assert_eq!(info.layer, BindingLayer::User);
    }

    #[test]
    fn test_binding_info_no_category() {
        let info = BindingInfo::new(cmd("noop"), "No-op", None, BindingLayer::Base);
        assert!(info.category.is_none());
        assert_eq!(info.layer, BindingLayer::Base);
    }

    #[test]
    fn test_binding_info_clone() {
        let info =
            BindingInfo::new(cmd("yank"), "Yank text", Some("operator"), BindingLayer::Policy);
        let cloned = info.clone();
        assert_eq!(info, cloned);
    }

    #[test]
    fn test_binding_info_eq() {
        let a = BindingInfo::new(cmd("delete"), "Delete", Some("operator"), BindingLayer::Policy);
        let b = BindingInfo::new(cmd("delete"), "Delete", Some("operator"), BindingLayer::Policy);
        assert_eq!(a, b);
    }

    #[test]
    fn test_binding_info_ne_command() {
        let a = BindingInfo::new(cmd("delete"), "Delete", Some("operator"), BindingLayer::Policy);
        let b = BindingInfo::new(cmd("yank"), "Delete", Some("operator"), BindingLayer::Policy);
        assert_ne!(a, b);
    }

    #[test]
    fn test_binding_info_ne_description() {
        let a = BindingInfo::new(cmd("delete"), "Delete text", None, BindingLayer::Policy);
        let b = BindingInfo::new(cmd("delete"), "Remove text", None, BindingLayer::Policy);
        assert_ne!(a, b);
    }

    #[test]
    fn test_binding_info_ne_category() {
        let a = BindingInfo::new(cmd("delete"), "Delete", Some("operator"), BindingLayer::Policy);
        let b = BindingInfo::new(cmd("delete"), "Delete", Some("edit"), BindingLayer::Policy);
        assert_ne!(a, b);
    }

    #[test]
    fn test_binding_info_ne_layer() {
        let a = BindingInfo::new(cmd("delete"), "Delete", None, BindingLayer::Policy);
        let b = BindingInfo::new(cmd("delete"), "Delete", None, BindingLayer::User);
        assert_ne!(a, b);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_binding_info_debug() {
        let info = BindingInfo::new(cmd("delete"), "Delete", Some("op"), BindingLayer::Policy);
        let debug_str = format!("{info:?}");
        assert!(debug_str.contains("BindingInfo"));
    }
}
