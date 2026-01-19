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
mod delete;
mod types;
mod yank;

// Re-export operator types
pub use types::{Operator, OperatorContext, OperatorError, Range};

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
}
