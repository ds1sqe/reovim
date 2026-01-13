//! Vim operators module - POLICY.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)
//!
//! This module implements the standard Vim operators:
//! - `d` - Delete (cuts text to register)
//! - `y` - Yank (copies text to register)
//! - `c` - Change (cuts text and enters insert mode)
//!
//! # Mechanism vs Policy
//!
//! - **Mechanism (Kernel)**: `Operator` trait, `Range`, `OperatorContext`
//! - **Policy (This Module)**: Which operators exist, what they do
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_operators::operators;
//!
//! // Get all operators registered by this module
//! let ops = operators();
//! for op in &ops {
//!     println!("{}: {}", op.id(), if op.is_text_modifying() { "modifies" } else { "read-only" });
//! }
//! ```

mod change;
mod delete;
mod yank;

use reovim_kernel::api::v1::{
    Module, ModuleContext, ModuleError, ModuleId, Operator, ProbeResult, Version,
};

pub use {change::ChangeOperator, delete::DeleteOperator, yank::YankOperator};

/// Returns all operators provided by this module.
#[must_use]
pub fn operators() -> Vec<Box<dyn Operator>> {
    vec![
        Box::new(DeleteOperator),
        Box::new(YankOperator),
        Box::new(ChangeOperator),
    ]
}

// ============================================================================
// Module trait implementation
// ============================================================================

/// Operators module instance.
pub struct OperatorsModule;

impl Module for OperatorsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("operators")
    }

    fn name(&self) -> &'static str {
        "Vim Operators"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operators_list() {
        let ops = operators();
        assert_eq!(ops.len(), 3);

        // Check operators are present
        let ids: Vec<_> = ops.iter().map(|o| o.id()).collect();
        assert!(ids.contains(&"delete"));
        assert!(ids.contains(&"yank"));
        assert!(ids.contains(&"change"));
    }

    #[test]
    fn test_delete_is_text_modifying() {
        let delete = DeleteOperator;
        assert!(delete.is_text_modifying());
        assert!(!delete.is_linewise());
    }

    #[test]
    fn test_yank_is_not_text_modifying() {
        let yank = YankOperator;
        assert!(!yank.is_text_modifying());
    }

    #[test]
    fn test_change_is_text_modifying() {
        let change = ChangeOperator;
        assert!(change.is_text_modifying());
    }

    #[test]
    fn test_module_trait() {
        let module = OperatorsModule;
        assert_eq!(module.id().as_str(), "operators");
        assert_eq!(module.name(), "Vim Operators");
    }
}
