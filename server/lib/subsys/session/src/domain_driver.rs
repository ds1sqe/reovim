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
//! `dispatch_input` and `dispatch_command` are routing entry points from the
//! server. The domain driver internally delegates to its resolver registry
//! and command registry (populated by modules at startup). The server never
//! sees `ResolveResult`, mode transitions, or pending bindings — those are
//! domain-internal orchestration.

use std::sync::Arc;

use {
    reovim_kernel::api::v1::{BufferId, ModeId, WindowId},
    reovim_subsys_coordination::{Cursor, Projection},
    reovim_subsys_input::InputEvent,
};

use super::{BufferContentProvider, ClientId, CommandResult, DispatchResult, ExtensionMap};

/// What each domain implements.
///
/// The server holds `Arc<dyn DomainDriver>` per registered domain and routes
/// operations based on the active buffer's domain.
///
/// # Minimal Surface
///
/// This trait is intentionally small — the domain driver is the entry point
/// for the server to interact with a domain, not the surface area for all
/// domain capabilities.
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

    /// Dispatch a command for a client.
    ///
    /// Returns [`CommandResult::Handled`] if the domain handles this command.
    /// Returns [`CommandResult::NotHandled`] if the command is unknown to this
    /// domain — the server falls back to session-global commands (`:q`, `:split`, etc.).
    fn dispatch_command(
        &self,
        client_id: ClientId,
        command: &str,
        args: &[String],
    ) -> CommandResult;

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
    /// The server calls this after dispatch (polling for cursor state), or
    /// when a client first connects, or when broadcasting cursor presence.
    fn cursors(&self, client_id: ClientId, window_id: WindowId) -> Vec<Box<dyn Cursor>>;

    /// Create the initial cursor when a client opens a buffer of this domain.
    fn initial_cursor(&self, client_id: ClientId, buffer_id: BufferId) -> Box<dyn Cursor>;

    // --- Opaque input dispatch (Phase A, domain-neutral) ---

    /// Dispatch an opaque input event with access to extension maps.
    ///
    /// This is the sole domain-neutral dispatch path. The domain driver decodes
    /// the InputEvent payload via codec crates and dispatches accordingly.
    /// Returns `DispatchResult` — the server polls for all other state changes.
    fn dispatch_input(
        &self,
        client_id: ClientId,
        event: &InputEvent,
        client_ext: &mut ExtensionMap,
        shared_ext: &mut ExtensionMap,
    ) -> DispatchResult;

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
        // Delegate to dispatch_command (same contract, v2 is now the canonical path)
        self.dispatch_command(client_id, command, args)
    }

    // --- Projections (domain-neutral state transport) ---

    /// Collect projections for changed state after dispatch.
    ///
    /// REQUIRED: no default. A forgotten implementation is a compile error,
    /// not a silent blank client. Domain drivers that genuinely emit no
    /// projections return empty Vec explicitly.
    ///
    /// Called after EVERY dispatch_input. Must be O(changed-tags), not O(all-state).
    /// The domain driver tracks dirty state internally.
    fn collect_projections(&self, client_id: ClientId) -> Vec<Projection>;

    /// Seed projections for a new client (initial state).
    ///
    /// REQUIRED: no default. Must return only Persistent projections.
    /// Transient projections would deliver phantom events to late-joining clients.
    fn initial_projections(&self, client_id: ClientId) -> Vec<Projection>;

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

    /// Active buffer for a client (the buffer in the focused window).
    ///
    /// Returns `None` if client not found or no buffer is active.
    fn active_buffer(&self, _client_id: ClientId) -> Option<BufferId> {
        None
    }
}
