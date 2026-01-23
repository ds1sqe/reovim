//! Vim mode key resolvers.
//!
//! Implements Vim-style key handling policy for each mode:
//! - Normal: count prefixes, register selection, operator entry
//! - Insert: character insertion, escape handling
//! - Delete/Yank/Change: dedicated operator modes (replaces generic operator-pending)
//!
//! # Architecture (Epic #415)
//!
//! Previously, a single generic `VimOperatorPendingResolver` handled all operators.
//! Now, each operator has its own dedicated mode and resolver:
//! - `VimDeleteResolver` for `vim:delete` mode
//! - `VimYankResolver` for `vim:yank` mode
//! - `VimChangeResolver` for `vim:change` mode
//!
//! Benefits:
//! - Mode carries operator semantics (no runtime lookup)
//! - Each resolver is focused (~300 lines vs 1000+)
//! - Statusline shows "DELETE"/"YANK"/"CHANGE" instead of "OP-PENDING"

mod change;
mod delete;
mod insert;
mod normal;
pub mod operator_common;
#[allow(deprecated)]
mod operator_pending;
mod yank;

// Dedicated operator resolvers (Epic #415)
pub use {change::VimChangeResolver, delete::VimDeleteResolver, yank::VimYankResolver};

// Other resolvers
pub use {insert::VimInsertResolver, normal::VimNormalResolver};

// Deprecated operator-pending resolver (kept for compatibility)
#[allow(deprecated)]
pub use operator_pending::VimOperatorPendingResolver;
