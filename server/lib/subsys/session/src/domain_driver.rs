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
    reovim_input_codec::KeyEvent,
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

    // --- Key dispatch ---

    /// Dispatch a key event for a client editing a buffer of this domain.
    ///
    /// The domain driver internally:
    /// 1. Looks up the active mode's resolver (from its resolver registry)
    /// 2. Resolves the key (module policy)
    /// 3. Executes the resulting action (command, insert, mode transition)
    /// 4. Updates internal state (cursors, undo, syntax)
    /// 5. Returns a [`DispatchResult`] describing what changed
    ///
    /// The server never sees `ResolveResult` or mode transitions.
    fn dispatch_key(&self, client_id: ClientId, key: &KeyEvent) -> DispatchResult;

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
                // KIND_KEY — delegate to dispatch_key_with_extensions
                let key_event = self.decode_key_fallback(payload);
                if let Some(key) = key_event {
                    return self
                        .dispatch_key_with_extensions(client_id, &key, client_ext, shared_ext);
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
        // Delegate to dispatch_command (same contract, v2 is now the canonical path)
        self.dispatch_command(client_id, command, args)
    }

    // --- Extension-aware dispatch ---

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
    ) -> DispatchResult {
        self.dispatch_key(client_id, key)
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

    /// Viewport info for a client's window (scroll position, dimensions).
    fn viewport_info(&self, _client_id: ClientId, _window_id: WindowId) -> Option<ViewportInfo> {
        None
    }

    /// Selection state for a client.
    fn selection_info(&self, _client_id: ClientId) -> Option<SelectionInfo> {
        None
    }

    /// Tab page info for a client.
    fn tab_info(&self, _client_id: ClientId) -> Vec<TabInfo> {
        Vec::new()
    }

    /// Register entries for a client (name, opaque content, display string).
    fn register_entries(&self, _client_id: ClientId) -> Vec<RegisterEntry> {
        Vec::new()
    }

    /// Single register entry by name.
    fn register_entry(&self, _client_id: ClientId, _name: &str) -> Option<RegisterEntry> {
        None
    }

    /// Clipboard history entry at index.
    fn clipboard_history_entry(&self, _client_id: ClientId, _index: usize) -> Option<Vec<u8>> {
        None
    }
}

// ==========================================================================
// Domain-neutral query result types
// ==========================================================================

/// Viewport information for a client's window (domain-neutral).
#[derive(Debug, Clone)]
pub struct ViewportInfo {
    /// Scroll offset (top line visible).
    pub scroll_top: u64,
    /// Viewport width in cells.
    pub width: u16,
    /// Viewport height in cells.
    pub height: u16,
}

/// Selection state for a client (domain-neutral).
///
/// Selection coordinates are opaque bytes — the server transports them
/// without interpreting line/column values.
#[derive(Debug, Clone)]
pub struct SelectionInfo {
    /// Selection mode as string (e.g., "char", "line", "block").
    pub mode: String,
    /// Display representation of selection bounds.
    pub display: String,
}

/// Tab page info (domain-neutral).
#[derive(Debug, Clone)]
pub struct TabInfo {
    /// Tab page ID.
    pub id: u64,
    /// Display label.
    pub label: String,
    /// Whether this tab is active.
    pub is_active: bool,
}

/// Register entry (domain-neutral).
#[derive(Debug, Clone)]
pub struct RegisterEntry {
    /// Register name (e.g., `"\"\"`, `"a"`, `"+"`).
    pub name: String,
    /// Opaque content bytes.
    pub content: Vec<u8>,
    /// Human-readable display string.
    pub display: Option<String>,
}
