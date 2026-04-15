//! Mode transition types — re-exported from subsys-input.
//!
//! The canonical definitions live in `reovim-subsys-input`. This module
//! re-exports them for backward compatibility.

pub use reovim_subsys_input::{PopResult, TransitionContext};

#[cfg(test)]
#[path = "transition_tests.rs"]
mod tests;
