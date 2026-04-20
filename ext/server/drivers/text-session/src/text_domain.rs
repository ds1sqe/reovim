//! Text domain driver — implements [`DomainDriver`] for the text domain.
//!
//! This is the main deliverable of sub-plan 03. The `TextDomainDriver` wraps the
//! existing `SessionRuntime` and provides the `DomainDriver` interface that the
//! server will use in sub-plan 05.
//!
//! # Interior Mutability
//!
//! `DomainDriver` methods take `&self`. `TextDomainDriver` uses `RwLock` for
//! mutable state:
//!
//! | Method | Lock type | Rationale |
//! |--------|-----------|-----------|
//! | `dispatch_input` | Write on clients + session | Mutates mode, cursor |
//! | `dispatch_command` | Write on clients + session | Same |
//! | `on_client_added` | Write on clients | Adds entry |
//! | `on_client_removed` | Write on clients | Removes entry |
//! | `on_focus_gained/lost` | Write on clients | Mutates mode state |
//! | `cursors` | Read on clients | Only reads cursor positions |
//! | `content_provider` | No lock | Returns Arc clone |
//! | `domain_name/id` | No lock | Immutable |
//!
//! # `RwLock` → `SessionRuntime` Lifetime Safety
//!
//! `dispatch_input` acquires write locks on `session` and `clients`, then creates
//! `SessionRuntime` borrowing from the lock guards. The key invariant:
//!
//! **The borrow must not cross any async yield.** All dispatch is synchronous
//! within the lock scope. The lock guards live on the stack, `SessionRuntime`
//! borrows from them, and both are dropped before the function returns.
//!
//! This is safe because:
//! 1. `DomainDriver::dispatch_input` is not async
//! 2. `SessionRuntime` does not escape the function
//! 3. All state mutations happen within the lock scope
//!
//! # Dual Ownership (Decision 14)
//!
//! Currently the server orchestrates `SessionRuntime` directly. `TextDomainDriver`
//! creates a parallel path. Sub-plan 05 resolves this by switching the server to
//! use `DomainDriver` exclusively.

use std::{collections::HashMap, sync::Arc};

use {
    reovim_arch::sync::RwLock,
    reovim_codec_tui_input::{KIND_KEY, KeyEvent, decode_key_event},
    reovim_domain_text::{HistoryRing, RegisterBank},
    reovim_kernel::api::v1::{BufferId, KernelContext, ModeId, ModeStack, WindowId},
    reovim_provider_text::TextBufferRegistry,
    reovim_subsys_coordination::Cursor,
    reovim_subsys_input::InputEvent,
    reovim_subsys_session::{
        BufferContentProvider, ClientId, CommandResult, CursorSnapshot, DispatchResult,
        DomainDriver, DomainRouting, ExtensionMap,
    },
};

use crate::{
    Jumplist, MarkBank, Session, WindowLayout,
    api::{ChangeTracker, CommandExecutor, StateChanges},
    change_bridge::{state_changes_to_command_result, state_changes_to_dispatch_result},
    tab::TabPageSet,
    text_content::TextContentProvider,
    text_cursor::TextCursor,
    text_cursor_shadow::TextCursorShadow,
};

// ============================================================================
// Per-client state (internal to the driver)
// ============================================================================

/// Per-client state bundle owned by `TextDomainDriver`.
///
/// Mirrors the fields in `server::EditingState` that `SessionRuntime` borrows.
/// This is an internal type — not exported from the crate.
struct PerClientState {
    mode_stack: ModeStack,
    windows: WindowLayout,
    extensions: ExtensionMap,
    compositor: Option<Box<dyn reovim_subsys_layout::RootCompositor>>,
    tabs: TabPageSet,
    registers: RegisterBank,
    clipboard_history: HistoryRing,
    local_marks: MarkBank,
    jumplist: Jumplist,
    active_buffer: Option<BufferId>,
    terminal_size: (u16, u16),
}

impl PerClientState {
    fn new(home_mode: ModeId) -> Self {
        Self {
            mode_stack: ModeStack::new(home_mode),
            windows: WindowLayout::empty(),
            extensions: ExtensionMap::new(),
            compositor: None,
            tabs: TabPageSet::new(),
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
        }
    }

    /// Create a [`ClientContext`] borrowing from this state.
    fn client_context(&mut self) -> crate::ClientContext<'_> {
        crate::ClientContext {
            mode_stack: &mut self.mode_stack,
            windows: &mut self.windows,
            extensions: &mut self.extensions,
            compositor: &mut self.compositor,
            tabs: &mut self.tabs,
            registers: &mut self.registers,
            clipboard_history: &mut self.clipboard_history,
            local_marks: &mut self.local_marks,
            jumplist: &mut self.jumplist,
            active_buffer: &mut self.active_buffer,
            terminal_size: &mut self.terminal_size,
        }
    }
}

// ============================================================================
// TextDomainDriver
// ============================================================================

/// Text-domain driver implementing `DomainDriver`.
///
/// Wraps the existing `SessionRuntime` infrastructure. The server does NOT
/// switch to using this yet — that happens in sub-plan 05.
pub struct TextDomainDriver {
    domain_id: u32,
    domain_name: &'static str,
    content_provider: Arc<TextContentProvider>,
    /// Shared session state (compositor template, global marks, home mode).
    session: RwLock<Session>,
    /// Per-client state (interior mutability for `&self` methods).
    clients: RwLock<HashMap<ClientId, PerClientState>>,
    /// Kernel context for buffer operations.
    kernel: Arc<KernelContext>,
    /// Command executor for command dispatch.
    executor: Arc<dyn CommandExecutor>,
    /// Text buffer registry for buffer content access.
    text_buffers: Arc<TextBufferRegistry>,
    /// Key dispatch provider (sub-plan 05 Phase 1).
    ///
    /// Wraps `ResolverRegistry` + `KeymapQuery` from driver-text-input.
    /// Injected at construction so `TextDomainDriver` can dispatch keys
    /// without depending on driver-text-input directly.
    dispatch_provider: Option<Arc<dyn crate::TextKeyDispatchProvider>>,
}

impl TextDomainDriver {
    fn sync_text_cursor_shadow(client_state: &PerClientState, client_ext: &mut ExtensionMap) {
        let shadow = client_ext.get_or_insert::<TextCursorShadow>();
        let Some(window) = client_state.windows.active() else {
            shadow.clear();
            *client_ext.get_or_insert::<CursorSnapshot>() = CursorSnapshot::SENTINEL;
            return;
        };
        let Some(buffer_id) = window.buffer_id else {
            shadow.clear();
            *client_ext.get_or_insert::<CursorSnapshot>() = CursorSnapshot::SENTINEL;
            return;
        };
        shadow.update(buffer_id, window.cursor.line, window.cursor.column);
        *client_ext.get_or_insert::<CursorSnapshot>() = shadow.cursor_snapshot();
    }

    /// Create a new text domain driver.
    ///
    /// # Arguments
    ///
    /// * `domain_id` — assigned by `CoordinationRegistry` at enlistment
    /// * `home_mode` — the mode used to initialize new clients
    /// * `kernel` — kernel context for buffer operations
    /// * `executor` — command executor
    /// * `text_buffers` — text buffer registry
    #[must_use]
    pub fn new(
        domain_id: u32,
        home_mode: ModeId,
        kernel: Arc<KernelContext>,
        executor: Arc<dyn CommandExecutor>,
        text_buffers: Arc<TextBufferRegistry>,
    ) -> Self {
        let content_provider = Arc::new(TextContentProvider::new(Arc::clone(&text_buffers)));
        let session = Session::new(ClientId::new(0), home_mode);

        Self {
            domain_id,
            domain_name: "text",
            content_provider,
            session: RwLock::new(session),
            clients: RwLock::new(HashMap::new()),
            kernel,
            executor,
            text_buffers,
            dispatch_provider: None,
        }
    }

    /// Set the key dispatch provider (sub-plan 05 Phase 1).
    ///
    /// Must be called after construction, before `dispatch_input` is used.
    /// The provider wraps `ResolverRegistry` + `KeymapQuery` from driver-text-input.
    pub fn set_dispatch_provider(&mut self, provider: Arc<dyn crate::TextKeyDispatchProvider>) {
        self.dispatch_provider = Some(provider);
    }

    /// Borrow the home mode from the session.
    fn home_mode(&self) -> ModeId {
        self.session.read().shared.home_mode().clone()
    }

    /// Dispatch a decoded key event through the injected dispatch provider.
    fn dispatch_decoded_key(
        &self,
        client_id: ClientId,
        key: &KeyEvent,
        client_ext: &mut ExtensionMap,
        shared_ext: &mut ExtensionMap,
    ) -> DispatchResult {
        let Some(ref provider) = self.dispatch_provider else {
            return self
                .with_client_runtime(client_id, |_runtime| {})
                .map(|((), changes)| state_changes_to_dispatch_result(&changes))
                .unwrap_or_default();
        };

        let mut session_guard = self.session.write();
        let mut clients_guard = self.clients.write();

        let Some(client_state) = clients_guard.get_mut(&client_id) else {
            return DispatchResult::default();
        };

        // Destructure to get extensions separately (placeholder pattern).
        // The runtime gets a placeholder; real client_ext goes to the provider.
        let mut placeholder_ext = ExtensionMap::new();
        let client_ctx = crate::ClientContext {
            mode_stack: &mut client_state.mode_stack,
            windows: &mut client_state.windows,
            extensions: &mut placeholder_ext,
            compositor: &mut client_state.compositor,
            tabs: &mut client_state.tabs,
            registers: &mut client_state.registers,
            clipboard_history: &mut client_state.clipboard_history,
            local_marks: &mut client_state.local_marks,
            jumplist: &mut client_state.jumplist,
            active_buffer: &mut client_state.active_buffer,
            terminal_size: &mut client_state.terminal_size,
        };

        let mut runtime = crate::SessionRuntime::with_owner(
            client_id,
            &mut session_guard,
            client_ctx,
            &self.kernel,
            &*self.executor,
        );

        let (_handled, changes) =
            provider.dispatch_key(&mut runtime, key, shared_ext, client_ext, &*self.executor);

        drop(runtime);

        Self::sync_text_cursor_shadow(client_state, client_ext);

        drop(session_guard);
        drop(clients_guard);

        state_changes_to_dispatch_result(&changes)
    }

    /// Internal dispatch: lock state, create `SessionRuntime`, take changes.
    ///
    /// # `RwLock` Lifetime Safety
    ///
    /// The write locks (`session_guard`, `clients_guard`) live on the stack.
    /// `SessionRuntime` borrows from these guards. The closure `f` executes
    /// synchronously — no async yield. Both guards drop when this function
    /// returns.
    fn with_client_runtime<F, R>(&self, client_id: ClientId, f: F) -> Option<(R, StateChanges)>
    where
        F: FnOnce(&mut crate::SessionRuntime<'_>) -> R,
    {
        let mut session_guard = self.session.write();
        let mut clients_guard = self.clients.write();

        let client_state = clients_guard.get_mut(&client_id)?;
        let client_ctx = client_state.client_context();

        let mut runtime = crate::SessionRuntime::with_owner(
            client_id,
            &mut session_guard,
            client_ctx,
            &self.kernel,
            &*self.executor,
        );

        let result = f(&mut runtime);
        let changes = ChangeTracker::take_changes(&mut runtime);

        drop(runtime);
        drop(session_guard);
        drop(clients_guard);

        Some((result, changes))
    }
}

impl DomainDriver for TextDomainDriver {
    fn domain_name(&self) -> &'static str {
        self.domain_name
    }

    fn domain_id(&self) -> u32 {
        self.domain_id
    }

    fn create_buffer(&self, content: &[u8]) -> BufferId {
        use reovim_provider_text::Buffer;

        let text = String::from_utf8_lossy(content);
        let buffer = Buffer::from_string(&text);
        let buffer_id = buffer.id();
        let arc = Arc::new(reovim_arch::sync::RwLock::new(buffer));
        self.text_buffers.register(arc.clone());
        self.kernel.buffers.register(arc);
        buffer_id
    }

    fn close_buffer(&self, buffer_id: BufferId) {
        self.text_buffers.unregister(buffer_id);
        self.kernel.buffers.unregister(buffer_id);
    }

    fn content_provider(&self) -> Arc<dyn BufferContentProvider> {
        Arc::clone(&self.content_provider) as Arc<dyn BufferContentProvider>
    }

    fn dispatch_input(
        &self,
        client_id: ClientId,
        event: &InputEvent,
        client_ext: &mut ExtensionMap,
        shared_ext: &mut ExtensionMap,
    ) -> DispatchResult {
        // Decode InputEvent payload via input-codec
        let payload = event.payload();
        let kind = reovim_subsys_input::input_kind(payload);

        match kind {
            KIND_KEY => {
                if let Ok(key) = decode_key_event(payload) {
                    self.dispatch_decoded_key(client_id, &key, client_ext, shared_ext)
                } else {
                    DispatchResult::default()
                }
            }
            // Pointer and scroll events not handled by text domain yet
            _ => DispatchResult::default(),
        }
    }

    fn dispatch_command(
        &self,
        client_id: ClientId,
        _command: &str,
        _args: &[String],
    ) -> CommandResult {
        // Command dispatch requires mapping command name strings to CommandId
        // (module + name). This mapping lives in the server layer's command
        // registry, not in the driver.
        //
        // Sub-plan 05 wires the full command dispatch path. For now, verify
        // the infrastructure: lock state, create SessionRuntime, take changes.
        match self.with_client_runtime(client_id, |_runtime| {
            // Command lookup deferred to sub-plan 05.
        }) {
            Some(((), changes)) => state_changes_to_command_result(&changes),
            None => CommandResult::NotHandled,
        }
    }

    fn collect_projections(
        &self,
        client_id: ClientId,
    ) -> Vec<reovim_subsys_coordination::Projection> {
        use reovim_subsys_coordination::{DomainId, Projection, ProjectionDelivery, ProjectionTag};

        let clients = self.clients.read();
        let Some(state) = clients.get(&client_id) else {
            return Vec::new();
        };

        let domain_id = DomainId(self.domain_id);
        let mut projections = Vec::new();

        // text.mode — current mode name
        let mode_name = state.mode_stack.current().name().to_owned();
        projections.push(Projection {
            tag: ProjectionTag::from("text.mode"),
            domain_id,
            window_id: None,
            payload: mode_name.as_bytes().to_vec(),
            display: Some(mode_name),
            delivery: ProjectionDelivery::Persistent,
        });

        projections
    }

    fn initial_projections(
        &self,
        client_id: ClientId,
    ) -> Vec<reovim_subsys_coordination::Projection> {
        // Initial state = same as collect (all persistent, no transient)
        self.collect_projections(client_id)
    }

    fn on_client_added(&self, client_id: ClientId) {
        let home_mode = self.home_mode();
        let mut clients = self.clients.write();
        clients.insert(client_id, PerClientState::new(home_mode));
    }

    fn on_client_removed(&self, client_id: ClientId) {
        let mut clients = self.clients.write();
        clients.remove(&client_id);
    }

    fn on_focus_gained(&self, client_id: ClientId, _window_id: WindowId, buffer_id: BufferId) {
        let mut clients = self.clients.write();
        if let Some(state) = clients.get_mut(&client_id) {
            state.active_buffer = Some(buffer_id);
        }
    }

    fn on_focus_lost(&self, _client_id: ClientId, _window_id: WindowId, _buffer_id: BufferId) {
        // No cleanup: per-client state is preserved for `on_focus_gained` resume.
    }

    fn cursors(&self, client_id: ClientId, window_id: WindowId) -> Vec<Box<dyn Cursor>> {
        let clients = self.clients.read();
        let Some(state) = clients.get(&client_id) else {
            return Vec::new();
        };

        let Some(window) = state.windows.get(window_id) else {
            return Vec::new();
        };

        #[allow(clippy::cast_possible_truncation)] // usize→u64 widening on 64-bit; never truncates
        let (line, col) = (window.cursor.line as u64, window.cursor.column as u64);
        drop(clients);

        let cursor = TextCursor::new(self.domain_id, line, col);
        vec![Box::new(cursor)]
    }

    fn initial_cursor(&self, _client_id: ClientId, _buffer_id: BufferId) -> Box<dyn Cursor> {
        Box::new(TextCursor::new(self.domain_id, 0, 0))
    }
}

impl DomainRouting for TextDomainDriver {
    fn current_mode(&self, client_id: ClientId) -> Option<ModeId> {
        let clients = self.clients.read();
        let mode = clients.get(&client_id)?.mode_stack.current().clone();
        drop(clients);
        Some(mode)
    }

    fn active_window(&self, client_id: ClientId) -> Option<WindowId> {
        let clients = self.clients.read();
        let id = clients.get(&client_id)?.windows.active_id();
        drop(clients);
        id
    }

    fn window_buffer(&self, client_id: ClientId, window_id: WindowId) -> Option<BufferId> {
        let clients = self.clients.read();
        let bid = clients.get(&client_id)?.windows.get(window_id)?.buffer_id;
        drop(clients);
        bid
    }

    fn windows(&self, client_id: ClientId) -> Vec<WindowId> {
        let clients = self.clients.read();
        let ids = clients
            .get(&client_id)
            .map_or_else(Vec::new, |s| s.windows.windows.iter().map(|w| w.id).collect());
        drop(clients);
        ids
    }

    fn window_count(&self, client_id: ClientId) -> usize {
        let clients = self.clients.read();
        let count = clients
            .get(&client_id)
            .map_or(0, |state| state.windows.windows.len());
        drop(clients);
        count
    }

    fn active_buffer(&self, client_id: ClientId) -> Option<BufferId> {
        let clients = self.clients.read();
        let buf = clients.get(&client_id)?.active_buffer;
        drop(clients);
        buf
    }

    fn cursor_generation(&self, _client_id: ClientId) -> u64 {
        0
    }
}

#[cfg(test)]
#[path = "text_domain_tests.rs"]
mod tests;
