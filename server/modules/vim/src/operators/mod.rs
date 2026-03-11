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
mod tests;
