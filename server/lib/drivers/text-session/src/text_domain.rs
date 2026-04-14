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
//! | `dispatch_key` | Write on clients + session | Mutates mode, cursor |
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
//! `dispatch_key` acquires write locks on `session` and `clients`, then creates
//! `SessionRuntime` borrowing from the lock guards. The key invariant:
//!
//! **The borrow must not cross any async yield.** All dispatch is synchronous
//! within the lock scope. The lock guards live on the stack, `SessionRuntime`
//! borrows from them, and both are dropped before the function returns.
//!
//! This is safe because:
//! 1. `DomainDriver::dispatch_key` is not async
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
    reovim_domain_text::{HistoryRing, RegisterBank},
    reovim_kernel::api::v1::{BufferId, KernelContext, ModeId, ModeStack, WindowId},
    reovim_provider_text::TextBufferRegistry,
    reovim_subsys_coordination::Cursor,
    reovim_subsys_input::KeyEvent,
    reovim_subsys_session::{
        BufferContentProvider, ChangeSet, ClientId, DomainDriver, DomainStateQuery, ExtensionMap,
        RegisterInfo,
    },
};

use crate::{
    Jumplist, MarkBank, Session, WindowLayout,
    api::{ChangeTracker, CommandExecutor, StateChanges},
    change_bridge::state_changes_to_change_set,
    tab::TabPageSet,
    text_content::TextContentProvider,
    text_cursor::TextCursor,
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
}

impl TextDomainDriver {
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
        }
    }

    /// Borrow the home mode from the session.
    fn home_mode(&self) -> ModeId {
        self.session.read().shared.home_mode().clone()
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

    fn dispatch_key(&self, client_id: ClientId, key: &KeyEvent) -> ChangeSet {
        // Lock state, create SessionRuntime, dispatch key.
        //
        // Currently: without a resolver chain wired (driver-session cannot
        // depend on driver-input due to circular dep), dispatch_key creates
        // the runtime infrastructure but cannot resolve keys.
        //
        // Sub-plan 05 resolves this by either:
        // - Moving TextDomainDriver to its own crate that CAN depend on driver-input
        // - Injecting the resolver via a trait at construction time
        //
        // For now, the infrastructure is proven by:
        // - with_client_runtime creates SessionRuntime from locked state
        // - StateChanges → ChangeSet bridge works (Phase 4 tests)
        // - dispatch_command demonstrates the full path via CommandExecutor
        let _ = key;
        self.with_client_runtime(client_id, |_runtime| {
            // Key resolution deferred: see comment above.
            // When wired, this would call:
            //   resolver.resolve_with_keymap(key, mode_state, runtime)
        })
        .map(|((), changes)| state_changes_to_change_set(&changes))
        .unwrap_or_default()
    }

    fn dispatch_command(
        &self,
        client_id: ClientId,
        _command: &str,
        _args: &[String],
    ) -> Option<ChangeSet> {
        // Command dispatch requires mapping command name strings to CommandId
        // (module + name). This mapping lives in the server layer's command
        // registry, not in the driver.
        //
        // Sub-plan 05 wires the full command dispatch path. For now, verify
        // the infrastructure: lock state, create SessionRuntime, take changes.
        self.with_client_runtime(client_id, |_runtime| {
            // Command lookup deferred to sub-plan 05.
        })
        .map(|((), changes)| state_changes_to_change_set(&changes))
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

impl DomainStateQuery for TextDomainDriver {
    fn mode_name(&self, client_id: ClientId) -> Option<String> {
        let clients = self.clients.read();
        let state = clients.get(&client_id)?;
        let name = state.mode_stack.current().name().to_owned();
        drop(clients);
        Some(name)
    }

    fn registers(&self, client_id: ClientId) -> Vec<RegisterInfo> {
        let clients = self.clients.read();
        let Some(state) = clients.get(&client_id) else {
            return Vec::new();
        };

        let infos: Vec<RegisterInfo> = state
            .registers
            .iter_non_empty()
            .map(|(name, content)| {
                use reovim_domain_text::YankType;
                let type_label = match content.yank_type {
                    YankType::Characterwise => "characterwise",
                    YankType::Linewise => "linewise",
                };
                RegisterInfo {
                    name,
                    display: content.text.clone(),
                    content_type: format!("text/{type_label}"),
                }
            })
            .collect();
        drop(clients);
        infos
    }

    fn status_info(&self, _client_id: ClientId) -> Option<String> {
        None
    }
}

#[cfg(test)]
#[path = "text_domain_tests.rs"]
mod tests;
