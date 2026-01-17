//! Vim mode key resolvers.
//!
//! Implements Vim-style key handling policy for each mode:
//! - Normal: count prefixes, register selection, operator entry
//! - Insert: character insertion, escape handling
//! - Operator-pending: motion/text-object completion

mod insert;
mod normal;
mod operator_pending;

pub use {
    insert::VimInsertResolver, normal::VimNormalResolver,
    operator_pending::VimOperatorPendingResolver,
};
