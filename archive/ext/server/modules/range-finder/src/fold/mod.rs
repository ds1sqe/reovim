//! Code folding sub-module.
//!
//! Provides fold commands (`za`/`zo`/`zc`/`zR`/`zM`) using fold ranges
//! from `SyntaxDriver::folds()`. Fold state is per-buffer within a
//! shared session extension.

pub mod bridge;
pub mod command;
pub mod ids;
pub mod state;
