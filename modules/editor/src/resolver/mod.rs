//! Vim-style mode key resolvers.
//!
//! Implements the `ModeKeyResolver` trait from the input driver for Vim-style
//! editing modes. Each resolver handles mode-specific key interpretation.
//!
//! # Available Resolvers
//!
//! - [`VimNormalResolver`] - Normal mode with counts, registers, operator entry
//! - [`VimInsertResolver`] - Insert mode with character insertion
//! - [`VimOperatorPendingResolver`] - Operator-pending mode for motions
//!
//! # Architecture
//!
//! These resolvers implement POLICY - they decide HOW keys are interpreted
//! in each mode. The runner provides MECHANISM - it dispatches to resolvers
//! and executes their results.
//!
//! ```text
//! +----------------------------------------------------------+
//! | VimNormalResolver                              POLICY    |
//! | - Accumulates count digits (1-9, then 0)                 |
//! | - Handles register prefix (")                            |
//! | - Delegates to keymap for command lookup                 |
//! | - Returns Execute, Pending, or ModeTransition            |
//! +----------------------------------------------------------+
//!                          |
//!                          v uses
//! +----------------------------------------------------------+
//! | ModeKeyResolver trait                        MECHANISM   |
//! | - resolve(key, state) -> ResolveResult                   |
//! | - mode_id() -> &ModeId                                   |
//! +----------------------------------------------------------+
//! ```

mod insert;
mod normal;
mod operator_pending;
mod registry;

pub use {
    insert::VimInsertResolver, normal::VimNormalResolver,
    operator_pending::VimOperatorPendingResolver, registry::ResolverRegistry,
};
