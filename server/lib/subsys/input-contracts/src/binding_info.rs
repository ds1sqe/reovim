use {crate::BindingLayer, reovim_kernel::api::v1::CommandId};

/// Metadata about a keybinding returned by `bindings_with_prefix()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingInfo {
    /// The command this binding triggers.
    pub command: CommandId,
    /// Human-readable description.
    pub description: &'static str,
    /// Optional category for grouping/filtering.
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

    /// Create a `BindingInfo` with only a command and layer.
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
    fn binding_info_constructors_preserve_fields() {
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

        let bare = BindingInfo::from_command(cmd("delete"), BindingLayer::User);
        assert_eq!(bare.command, cmd("delete"));
        assert_eq!(bare.description, "");
        assert_eq!(bare.category, None);
        assert_eq!(bare.layer, BindingLayer::User);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn binding_info_debug_mentions_type_name() {
        let debug = format!(
            "{:?}",
            BindingInfo::new(cmd("delete"), "Delete", Some("operator"), BindingLayer::Base)
        );
        assert!(debug.contains("BindingInfo"));
    }
}
