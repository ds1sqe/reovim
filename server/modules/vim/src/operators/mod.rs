//! Vim operators - delete, yank, change, case transformation.
//!
//! This module implements the standard Vim operators:
//! - `d` - Delete (cuts text to register)
//! - `y` - Yank (copies text to register)
//! - `c` - Change (cuts text and enters insert mode)
//! - `gu` - Lowercase
//! - `gU` - Uppercase
//! - `g~` - Toggle case
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

mod case;
mod change;
mod commands;
mod delete;
mod registers;
mod types;
mod yank;
pub mod yank_flash;

// Re-export operator types
pub use types::{Operator, OperatorContext, OperatorError, Range};

// Re-export operator implementations
pub use {
    case::{LowercaseOperator, ToggleCaseOperator, UppercaseOperator},
    change::ChangeOperator,
    delete::DeleteOperator,
    yank::YankOperator,
};

// Re-export command wrappers (Epic #415)
pub use commands::{
    ChangeCommand, DeleteCommand, LowercaseCommand, ToggleCaseCommand, UppercaseCommand,
    YankCommand, operator_commands,
};

/// Returns all operators provided by this module.
#[must_use]
pub fn operators() -> Vec<Box<dyn Operator>> {
    vec![
        Box::new(DeleteOperator),
        Box::new(YankOperator),
        Box::new(ChangeOperator),
        Box::new(LowercaseOperator),
        Box::new(UppercaseOperator),
        Box::new(ToggleCaseOperator),
    ]
}

#[cfg(test)]
mod tests;
