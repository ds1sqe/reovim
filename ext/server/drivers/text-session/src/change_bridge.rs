//! `StateChanges` → dispatch-result bridge.
//!
//! Converts text-domain [`StateChanges`] directly to domain-neutral
//! [`DispatchResult`] / [`CommandResult`].
//!
//! # What is NOT mapped
//!
//! Text-specific fields stay inside the text domain for internal consumers
//! (syntax highlighting, codec index):
//!
//! | Field | Classification | Disposition |
//! |-------|---------------|-------------|
//! | `text_buffer_edits` | Text-domain | Syntax/codec consumers |
//! | `byte_edits` | Codec-domain | Codec index |
//!
//! These are stored in `SessionState::pending_text_edits` / `pending_byte_edits`
//! by the text driver and consumed by the server's syntax/codec paths.

use {
    crate::api::StateChanges,
    reovim_subsys_session::{
        CommandResult, Directive, DispatchResult, dispatch_result::BufferChanges,
    },
};

/// Convert text-domain `StateChanges` to a `DispatchResult`.
///
/// Extracts only the fields that `DispatchResult` cares about (buffer lifecycle
/// and session directives). All signal flags (`cursor_moved`, `mode_changed`, etc.)
/// remain driver/server-internal and are not exposed through the contract.
#[must_use]
pub fn state_changes_to_dispatch_result(changes: &StateChanges) -> DispatchResult {
    let directive = if changes.should_quit {
        Directive::Quit
    } else {
        Directive::Continue
    };

    DispatchResult {
        buffers: BufferChanges {
            modified: changes.modified_buffers.clone(),
            created: changes.buffers_created.clone(),
            closed: changes.buffers_deleted.clone(),
        },
        directive,
    }
}

/// Convert text-domain `StateChanges` to a `CommandResult`.
#[must_use]
pub fn state_changes_to_command_result(changes: &StateChanges) -> CommandResult {
    CommandResult::Handled(state_changes_to_dispatch_result(changes))
}

#[cfg(test)]
#[path = "change_bridge_tests.rs"]
mod tests;
