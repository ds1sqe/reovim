//! Mode transition types — re-exported from shared input contracts.
//!
//! The canonical definitions live in `reovim-subsys-input-contracts`. This module
//! re-exports them for backward compatibility.

pub use reovim_subsys_input_contracts::{PopResult, TransitionContext};

#[cfg(test)]
#[path = "transition_tests.rs"]
mod tests;
