//! Mode transition types for text-input.
//!
//! The canonical definitions live in `reovim-driver-text-session` (to avoid a
//! circular dependency: text-input depends on text-session). This module
//! re-exports them for internal use.

pub use reovim_driver_text_session::{ModeTransition, PopResult, TransitionContext};
