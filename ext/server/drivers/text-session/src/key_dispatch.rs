//! Key dispatch provider trait — injection point for resolver access.
//!
//! Breaks the circular dependency between driver-text-session and
//! driver-text-input. The trait is defined here (driver-text-session);
//! implemented in driver-text-input (which already depends on us).
//!
//! # Architecture
//!
//! ```text
//! driver-text-session  ──defines──▶  TextKeyDispatchProvider (trait)
//!        ▲                                     │
//!        │ depends on                          implemented by
//!        │                                     │
//! driver-text-input   ──implements──▶  ResolverDispatchProvider (struct)
//! ```
//!
//! The implementation handles the FULL key dispatch pipeline:
//! 1. Resolve key via the mode's resolver (`ResolverRegistry`)
//! 2. Handle `ResolveResult` (execute command, mode transition, inject keys)
//! 3. Handle `on_command_complete` for pending operators
//! 4. Manage `PendingBindings` for whichkey hints
//! 5. Return accumulated `StateChanges`
//!
//! The server never sees `ResolveResult`, `ModeTransition`, or resolver
//! internals — those are encapsulated inside the implementation.

use {reovim_input_codec::KeyEvent, reovim_subsys_session::ExtensionMap};

use crate::{
    SessionRuntime,
    api::{CommandExecutor, StateChanges},
};

/// Provider for full key dispatch (resolve + handle result + pending bindings).
///
/// Implemented by `ResolverDispatchProvider` in driver-text-input, which wraps
/// `ResolverRegistry` + `KeymapQuery`.
///
/// # Returns
///
/// `(handled, changes)` where:
/// - `handled` — `true` if the key was processed (not `NotHandled`)
/// - `changes` — accumulated state changes including command execution,
///   mode transitions, buffer modifications
pub trait TextKeyDispatchProvider: Send + Sync {
    /// Resolve a key and handle the result completely.
    ///
    /// The implementation:
    /// 1. Finds the resolver for `runtime`'s current mode
    /// 2. Resolves the key (may modify buffers, cursors via `SessionApi`)
    /// 3. Handles the result:
    ///    - `Execute` → looks up command via `executor`, calls `handle.execute(runtime, ctx)`
    ///    - `ModeTransition` → push/pop/set on runtime's mode stack
    ///    - `InjectKeys` → recursively resolves injected keys
    ///    - `Completed`/`Pending` → no further action
    /// 4. Calls `on_command_complete` on the resolver after command execution
    /// 5. Populates `PendingBindings` in client extensions for whichkey hints
    ///
    /// # Arguments
    ///
    /// * `runtime` — borrows session + per-client state (mode, windows, etc.)
    /// * `key` — the key event to dispatch
    /// * `shared_ext` — session-scoped extensions (shared across all clients)
    /// * `client_ext` — per-client extensions (`PendingBindings`, etc.)
    /// * `executor` — command lookup (server's `CommandRegistry` behind this trait)
    fn dispatch_key(
        &self,
        runtime: &mut SessionRuntime<'_>,
        key: &KeyEvent,
        shared_ext: &mut ExtensionMap,
        client_ext: &mut ExtensionMap,
        executor: &dyn CommandExecutor,
    ) -> (bool, StateChanges);
}
