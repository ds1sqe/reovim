//! Vim mode key resolvers.
//!
//! Implements Vim-style key handling policy for each mode:
//! - Normal: count prefixes, register selection, operator entry
//! - Insert: character insertion, escape handling
//! - `CommandLine`: input capture for `:`, `/`, `?`
//! - Delete/Yank/Change: dedicated operator modes
//!
//! # Architecture (Epic #415, #417)
//!
//! Each operator has its own dedicated mode and resolver:
//! - `VimDeleteResolver` for `vim:delete` mode
//! - `VimYankResolver` for `vim:yank` mode
//! - `VimChangeResolver` for `vim:change` mode
//!
//! Benefits:
//! - Mode carries operator semantics (no runtime lookup)
//! - Each resolver is focused (~300 lines)
//! - Statusline shows "DELETE"/"YANK"/"CHANGE"

mod change;
mod commandline;
mod delete;
mod insert;
mod normal;
pub mod operator_common;
mod window;
mod yank;

// Dedicated operator resolvers (Epic #415)
pub use {change::VimChangeResolver, delete::VimDeleteResolver, yank::VimYankResolver};

// Window mode resolver (Epic #438)
pub use window::VimWindowResolver;

// Other resolvers
pub use {
    commandline::VimCommandLineResolver, insert::VimInsertResolver, normal::VimNormalResolver,
};
