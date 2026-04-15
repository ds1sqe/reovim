//! Closed return types for domain driver dispatch operations.
//!
//! `DispatchResult` replaces `ChangeSet` — it contains only what the server
//! cannot derive by polling. Like Linux `write()` returning `ssize_t`.

use reovim_kernel::api::v1::BufferId;

/// Closed return type for `dispatch_input` and `dispatch_command`.
///
/// Contains only what the server CANNOT derive by polling.
/// Never grows for new domains — buffer lifecycle and session
/// lifecycle are finite, server-defined operations.
#[derive(Debug, Default, Clone)]
pub struct DispatchResult {
    /// Buffer lifecycle events from this dispatch.
    pub buffers: BufferChanges,
    /// Session-level directive.
    pub directive: Directive,
}

/// Complete buffer lifecycle — closed because buffer operations are
/// finite (create, modify, close). New domains use the same operations.
#[derive(Debug, Default, Clone)]
pub struct BufferChanges {
    /// Buffers whose content was modified.
    pub modified: Vec<BufferId>,
    /// Buffers that were created.
    pub created: Vec<BufferId>,
    /// Buffers that were closed/deleted.
    pub closed: Vec<BufferId>,
}

impl BufferChanges {
    /// No buffer changes.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether any buffer changes occurred.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.modified.is_empty() || !self.created.is_empty() || !self.closed.is_empty()
    }
}

/// Closed enum. Server session lifecycle only.
/// New domains do not add variants — these are server operations.
#[non_exhaustive]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Directive {
    /// Normal return. Server polls for state changes.
    #[default]
    Continue,
    /// Client should disconnect from this session.
    Detach,
    /// Client should exit.
    Quit,
    /// Terminal suspend (Ctrl-Z / :suspend). Server pauses client.
    Suspend,
    /// Domain requests full redraw (display corruption recovery).
    ForceRedraw,
}

/// Result of dispatching a command to a domain driver.
///
/// Distinguishes routing failure (NotHandled) from execution failure (Error).
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// Command recognized and executed.
    Handled(DispatchResult),
    /// Command not recognized by this domain. Server tries next domain.
    NotHandled,
    /// Command recognized but execution failed.
    Error(String),
}
