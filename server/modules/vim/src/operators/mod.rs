//! Vim operators - delete, yank, change.
//!
//! This module implements the standard Vim operators:
//! - `d` - Delete (cuts text to register)
//! - `y` - Yank (copies text to register)
//! - `c` - Change (cuts text and enters insert mode)
//!
//! # Mechanism vs Policy
//!
//! - **Mechanism (Kernel)**: Buffer management, position types, register storage
//! - **Policy (This Module)**: `Operator` trait, `Range`, what operators do
//!
//! # Note
//!
//! Operators are a vim-specific concept (operator + motion = action on range).
//! This is why they live in the vim module, not as a separate module.

mod change;
mod commands;
mod delete;
mod registers;
mod types;
mod yank;

// Re-export operator types
pub use types::{Operator, OperatorContext, OperatorError, Range};

// Re-export operator implementations
pub use {change::ChangeOperator, delete::DeleteOperator, yank::YankOperator};

// Re-export command wrappers (Epic #415)
pub use commands::{ChangeCommand, DeleteCommand, YankCommand, operator_commands};

/// Returns all operators provided by this module.
#[must_use]
pub fn operators() -> Vec<Box<dyn Operator>> {
    vec![
        Box::new(DeleteOperator),
        Box::new(YankOperator),
        Box::new(ChangeOperator),
    ]
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
    fn test_operators_ids_unique() {
        let ops = operators();
        let ids: Vec<_> = ops.iter().map(|o| o.id()).collect();
        // Check all IDs are unique
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(seen.insert(id), "Duplicate operator id: {id}");
        }
    }

    #[test]
    fn test_operator_commands_not_empty() {
        let cmds = operator_commands();
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_operator_commands_count() {
        let cmds = operator_commands();
        assert_eq!(cmds.len(), 3); // delete, yank, change
    }

    #[test]
    fn test_delete_operator_debug() {
        let delete = DeleteOperator;
        assert!(format!("{delete:?}").contains("DeleteOperator"));
    }

    #[test]
    fn test_yank_operator_debug() {
        let yank = YankOperator;
        assert!(format!("{yank:?}").contains("YankOperator"));
    }

    #[test]
    fn test_change_operator_debug() {
        let change = ChangeOperator;
        assert!(format!("{change:?}").contains("ChangeOperator"));
    }

    #[test]
    fn test_operators_all_not_linewise() {
        let ops = operators();
        for op in &ops {
            assert!(!op.is_linewise(), "operator '{}' should not be linewise by default", op.id());
        }
    }

    #[test]
    fn test_operators_text_modifying_flags() {
        let ops = operators();
        let text_modifying: Vec<_> = ops
            .iter()
            .filter(|o| o.is_text_modifying())
            .map(|o| o.id())
            .collect();
        assert!(text_modifying.contains(&"delete"));
        assert!(text_modifying.contains(&"change"));
        assert!(!text_modifying.contains(&"yank"));
    }

    #[test]
    fn test_delete_operator_clone_copy() {
        let del = DeleteOperator;
        let copied: DeleteOperator = del;
        assert_eq!(copied.id(), "delete");
    }

    #[test]
    fn test_yank_operator_clone_copy() {
        let yank = YankOperator;
        let copied: YankOperator = yank;
        assert_eq!(copied.id(), "yank");
    }

    #[test]
    fn test_operator_commands_has_all_ids() {
        let cmds = operator_commands();
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        assert!(ids.iter().any(|id| id.name() == "delete"));
        assert!(ids.iter().any(|id| id.name() == "yank"));
        assert!(ids.iter().any(|id| id.name() == "change"));
    }

    #[test]
    fn test_operator_commands_all_vim_module() {
        let cmds = operator_commands();
        for cmd in &cmds {
            assert_eq!(
                cmd.id().module().as_str(),
                "vim",
                "command '{}' should be in vim module",
                cmd.id().name()
            );
        }
    }
}
