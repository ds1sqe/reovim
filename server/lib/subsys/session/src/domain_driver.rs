//! Domain driver trait — the contract each domain implements.
//!
//! The server holds `Arc<dyn DomainDriver>` per registered domain and routes
//! operations based on the active buffer's domain. This is the VFS analogy:
//! `SessionRuntime` is VFS (mechanism), each `DomainDriver` is a filesystem
//! implementation (ext4/btrfs).
//!
//! # Interior Mutability
//!
//! All methods take `&self`. Implementations use interior mutability
//! (`RwLock`/`Mutex`) to manage per-client and per-buffer state.
//!
//! # Dispatch vs Resolution
//!
//! `dispatch_key` and `dispatch_command` are routing entry points from the
//! server. The domain driver internally delegates to its resolver registry
//! and command registry (populated by modules at startup). The server never
//! sees `ResolveResult`, mode transitions, or pending bindings — those are
//! domain-internal orchestration.

use std::sync::Arc;

use {
    reovim_kernel::api::v1::{BufferId, ModeId, WindowId},
    reovim_subsys_coordination::Cursor,
    reovim_subsys_input::{InputEvent, KeyEvent},
};

use super::{
    BufferContentProvider, ChangeSet, ClientId, CommandResult, Directive, DispatchResult,
    ExtensionMap,
};
use super::dispatch_result::BufferChanges;

/// What each domain implements.
///
/// The server holds `Arc<dyn DomainDriver>` per registered domain and routes
/// operations based on the active buffer's domain.
///
/// # Minimal Surface
///
/// This trait is intentionally small — the domain driver is the entry point
/// for the server to interact with a domain, not the surface area for all
/// domain capabilities. Optional queries go on [`DomainStateQuery`].
pub trait DomainDriver: Send + Sync {
    /// Domain name (e.g., "text", "mesh", "image").
    ///
    /// Must match the name used at `CoordinationRegistry` enlistment.
    fn domain_name(&self) -> &'static str;

    /// Domain ID assigned by `CoordinationRegistry` at enlistment.
    fn domain_id(&self) -> u32;

    // --- Buffer lifecycle ---

    /// Create a new buffer with initial content bytes.
    ///
    /// The domain interprets the bytes (text: UTF-8, mesh: OBJ/glTF, etc.).
    fn create_buffer(&self, content: &[u8]) -> BufferId;

    /// Close a buffer. Domain cleans up internal state (undo tree, syntax, etc.).
    fn close_buffer(&self, buffer_id: BufferId);

    /// Buffer content provider for this domain.
    ///
    /// `Arc`'d for async safety — server holds across `.await` boundaries.
    fn content_provider(&self) -> Arc<dyn BufferContentProvider>;

    // --- Key dispatch ---

    /// Dispatch a key event for a client editing a buffer of this domain.
    ///
    /// The domain driver internally:
    /// 1. Looks up the active mode's resolver (from its resolver registry)
    /// 2. Resolves the key (module policy)
    /// 3. Executes the resulting action (command, insert, mode transition)
    /// 4. Updates internal state (cursors, undo, syntax)
    /// 5. Returns a [`ChangeSet`] describing what changed
    ///
    /// The server never sees `ResolveResult` or mode transitions.
    fn dispatch_key(&self, client_id: ClientId, key: &KeyEvent) -> ChangeSet;

    /// Dispatch a command for a client.
    ///
    /// Returns `Some(ChangeSet)` if the domain handles this command.
    /// Returns `None` if the command is unknown to this domain — the server
    /// falls back to session-global commands (`:q`, `:split`, etc.).
    fn dispatch_command(
        &self,
        client_id: ClientId,
        command: &str,
        args: &[String],
    ) -> Option<ChangeSet>;

    // --- Per-client lifecycle ---

    /// A client joined the session.
    ///
    /// Domain initializes per-client state (mode stack, registers, marks, etc.).
    /// Called on ALL registered domain drivers, not just the domain of the
    /// client's initial buffer.
    fn on_client_added(&self, client_id: ClientId);

    /// A client left the session.
    ///
    /// Domain cleans up per-client state. Called on ALL registered domain
    /// drivers unconditionally.
    fn on_client_removed(&self, client_id: ClientId);

    // --- Focus notifications ---

    /// Client's active window changed to a buffer of this domain.
    ///
    /// Domain activates per-client mode state for this domain.
    fn on_focus_gained(&self, client_id: ClientId, window_id: WindowId, buffer_id: BufferId);

    /// Client's active window moved away from this domain's buffer.
    ///
    /// Domain suspends per-client mode state (preserves for later resume).
    fn on_focus_lost(&self, client_id: ClientId, window_id: WindowId, buffer_id: BufferId);

    // --- Cursor access ---

    /// Get the current cursors for a client in a specific window.
    ///
    /// The server calls this when `ChangeSet` reports `cursor_moved`, or
    /// when a client first connects, or when broadcasting cursor presence.
    fn cursors(&self, client_id: ClientId, window_id: WindowId) -> Vec<Box<dyn Cursor>>;

    /// Create the initial cursor when a client opens a buffer of this domain.
    fn initial_cursor(&self, client_id: ClientId, buffer_id: BufferId) -> Box<dyn Cursor>;

    // --- Opaque input dispatch (Phase A, domain-neutral) ---

    /// Dispatch an opaque input event with access to extension maps.
    ///
    /// This is the new domain-neutral dispatch path. The domain driver decodes
    /// the InputEvent payload via codec crates and dispatches accordingly.
    /// Returns `DispatchResult` — the server polls for all other state changes.
    ///
    /// Default: decodes as KeyEvent via input-codec fallback to dispatch_key.
    /// Domains should override this to handle all input modalities.
    fn dispatch_input(
        &self,
        client_id: ClientId,
        event: &InputEvent,
        client_ext: &mut ExtensionMap,
        shared_ext: &mut ExtensionMap,
    ) -> DispatchResult {
        // Fallback: try to decode as key event for backward compatibility
        let payload = event.payload();
        if payload.len() >= 12 {
            let kind = u16::from_le_bytes([payload[0], payload[1]]);
            if kind == 0x0001 {
                // KIND_KEY — delegate to legacy dispatch_key path
                let key_event = self.decode_key_fallback(payload);
                if let Some(key) = key_event {
                    let change_set = self.dispatch_key_with_extensions(
                        client_id, &key, client_ext, shared_ext,
                    );
                    return changeset_to_dispatch_result(&change_set);
                }
            }
        }
        DispatchResult::default()
    }

    /// Decode a key event from InputEvent payload (fallback helper).
    /// Override not needed — this is only for the default dispatch_input impl.
    fn decode_key_fallback(&self, _payload: &[u8]) -> Option<KeyEvent> {
        None
    }

    /// Dispatch a command for a client (domain-neutral).
    ///
    /// Returns `CommandResult`:
    /// - `Handled(DispatchResult)` — command recognized and executed
    /// - `NotHandled` — command not recognized by this domain
    /// - `Error(String)` — command recognized but execution failed
    fn dispatch_command_v2(
        &self,
        client_id: ClientId,
        command: &str,
        args: &[String],
    ) -> CommandResult {
        // Fallback to legacy dispatch_command
        match self.dispatch_command(client_id, command, args) {
            Some(cs) => CommandResult::Handled(changeset_to_dispatch_result(&cs)),
            None => CommandResult::NotHandled,
        }
    }

    // --- Legacy dispatch (kept for gradual migration) ---

    /// Dispatch a key with access to the server's extension maps.
    ///
    /// The server owns `client_ext` (per-client module state: `PendingBindings`,
    /// `VimSessionState`, etc.) and `shared_ext` (session-scoped state). The
    /// domain driver borrows them for the duration of dispatch so that modules
    /// and bridges can read/write extension state during key resolution.
    ///
    /// # Lock ordering
    ///
    /// The caller (`Session::dispatch_key_for_client`) acquires locks in order:
    /// `clients` (write) → `state` (write). The domain driver's internal locks
    /// (`TextDomainDriver::session`, `TextDomainDriver::clients`) are disjoint
    /// from these. No deadlock risk.
    ///
    /// Default implementation delegates to [`dispatch_key`](Self::dispatch_key),
    /// ignoring the extension maps.
    fn dispatch_key_with_extensions(
        &self,
        client_id: ClientId,
        key: &KeyEvent,
        _client_ext: &mut ExtensionMap,
        _shared_ext: &mut ExtensionMap,
    ) -> ChangeSet {
        self.dispatch_key(client_id, key)
    }

    // --- State queries (Phase 4A, consumed by 4B/4C) ---

    /// Current mode for a client. Returns `None` if client not found.
    ///
    /// The server calls this for mode change detection and notifications.
    fn current_mode(&self, _client_id: ClientId) -> Option<ModeId> {
        None
    }

    /// Active (focused) window for a client.
    fn active_window(&self, _client_id: ClientId) -> Option<WindowId> {
        None
    }

    /// Buffer assigned to a window for a client.
    fn window_buffer(&self, _client_id: ClientId, _window_id: WindowId) -> Option<BufferId> {
        None
    }

    /// All window IDs for a client.
    fn windows(&self, _client_id: ClientId) -> Vec<WindowId> {
        Vec::new()
    }

    /// Number of windows for a client.
    fn window_count(&self, _client_id: ClientId) -> usize {
        0
    }
}

/// Optional state queries for domains that support them.
///
/// The server uses this for gRPC debug endpoints and status line rendering.
/// Not all domains need to implement all methods — defaults return `None`
/// or empty.
///
/// - Text domain: implements all methods
/// - Image domain: might only implement `mode_name`
/// - Binary/hex domain: might implement none
pub trait DomainStateQuery: Send + Sync {
    /// Active mode name for display (status line, gRPC).
    ///
    /// Returns `None` if this domain has no mode concept.
    fn mode_name(&self, _client_id: ClientId) -> Option<String> {
        None
    }

    /// Register contents as display representations.
    ///
    /// Returns empty if this domain has no registers.
    fn registers(&self, _client_id: ClientId) -> Vec<RegisterInfo> {
        Vec::new()
    }

    /// Domain-specific status information for the status line.
    fn status_info(&self, _client_id: ClientId) -> Option<String> {
        None
    }

    /// Selection info for a client's window.
    ///
    /// Returns domain-neutral selection coordinates for gRPC notifications.
    /// `mode` is a domain-specific label (text: "char"/"line"/"block").
    fn selection_info(&self, _client_id: ClientId, _window_id: WindowId) -> Option<SelectionInfo> {
        None
    }
}

/// Domain-neutral selection information for gRPC notifications.
///
/// The server never interprets selection semantics — it passes these
/// display coordinates to clients. The `mode` label is domain-specific
/// (text domain uses "char"/"line"/"block").
#[derive(Debug, Clone)]
pub struct SelectionInfo {
    /// Start line (0-indexed).
    pub start_line: u64,
    /// Start column (0-indexed).
    pub start_column: u64,
    /// End line (0-indexed).
    pub end_line: u64,
    /// End column (0-indexed).
    pub end_column: u64,
    /// Domain-specific selection mode label.
    pub mode: String,
}

/// Display representation of a register entry.
///
/// Used by [`DomainStateQuery::registers`] for gRPC debug views.
/// The server never interprets register content — it just passes
/// these display strings to clients.
#[derive(Debug, Clone)]
pub struct RegisterInfo {
    /// Register name (e.g., `'a'`, `'"'`, `'+'`).
    pub name: char,
    /// Human-readable display of the register content.
    pub display: String,
    /// Content type label (e.g., "text/characterwise", "mesh/vertices").
    pub content_type: String,
}

/// Bridge: convert a legacy `ChangeSet` to a `DispatchResult`.
///
/// Extracts only the fields that `DispatchResult` cares about (buffer lifecycle
/// and session directives). All signal flags (cursor_moved, mode_changed, etc.)
/// are dropped — the server polls for those.
pub fn changeset_to_dispatch_result(cs: &ChangeSet) -> DispatchResult {
    let directive = if cs.should_quit {
        Directive::Quit
    } else if cs.should_detach {
        Directive::Detach
    } else {
        Directive::Continue
    };

    let created = cs.created_buffers.clone();
    let closed = {
        let mut c = cs.deleted_buffers.clone();
        c.extend_from_slice(&cs.closed_buffers);
        c
    };

    DispatchResult {
        buffers: BufferChanges {
            modified: cs.modified_buffers.clone(),
            created,
            closed,
        },
        directive,
    }
}
