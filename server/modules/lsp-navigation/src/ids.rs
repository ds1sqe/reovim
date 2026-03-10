//! Command IDs for the LSP navigation module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for lsp-navigation.
pub const MODULE: ModuleId = ModuleId::new("lsp-navigation");

/// Go to definition (gd).
pub const GOTO_DEFINITION: CommandId = CommandId::new(MODULE, "goto-definition");

/// Find references (gr).
pub const REFERENCES: CommandId = CommandId::new(MODULE, "references");

/// Show hover information (K).
pub const HOVER: CommandId = CommandId::new(MODULE, "hover");

/// Show signature help.
pub const SIGNATURE_HELP: CommandId = CommandId::new(MODULE, "signature-help");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id() {
        assert_eq!(MODULE.as_str(), "lsp-navigation");
    }

    #[test]
    fn goto_definition_id() {
        assert_eq!(GOTO_DEFINITION.name(), "goto-definition");
        assert_eq!(*GOTO_DEFINITION.module(), MODULE);
    }

    #[test]
    fn references_id() {
        assert_eq!(REFERENCES.name(), "references");
        assert_eq!(*REFERENCES.module(), MODULE);
    }

    #[test]
    fn hover_id() {
        assert_eq!(HOVER.name(), "hover");
        assert_eq!(*HOVER.module(), MODULE);
    }

    #[test]
    fn signature_help_id() {
        assert_eq!(SIGNATURE_HELP.name(), "signature-help");
        assert_eq!(*SIGNATURE_HELP.module(), MODULE);
    }
}
