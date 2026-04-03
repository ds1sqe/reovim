//! Session state containing application state and registries.
//!
//! `SessionState` bundles the runtime application state with the registries
//! needed for key processing. Each session has its own isolated state.
//!
//! # SSOT Architecture
//!
//! `driver_session` is the Single Source of Truth (SSOT) for per-session state:
//! - `mode_stack` - current editing mode
//! - `pending_keys` - accumulated key sequence
//! - `extensions` - module-provided policy state
//! - `active_buffer` - currently active buffer ID
//! - `terminal_size` - session-level terminal dimensions
//!
//! The `AppState` within this struct provides server-specific state (kernel,
//! windows, cmdline) that doesn't belong in the driver layer.

use std::{collections::HashMap, sync::Arc};

use {
    parking_lot::RwLock,
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_input::{FallbackContext, PendingBindings, ResolverRegistry},
    reovim_driver_layout::RootCompositor,
    reovim_driver_session::{ClientId, Session as DriverSession},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{
        BufferId, BufferOps, CommandId, Jumplist, KernelContext, ModeId, ModeStack,
    },
    reovim_provider_text::Buffer,
    reovim_types_text::RegisterContent,
};

use crate::{
    app::AppState,
    registry::{CommandRegistry, KeyLookupResult, KeymapRegistry, ModeRegistry},
};

/// Session state combining application state with registries.
///
/// This is the complete state for a single editing session. Each session
/// (like tmux sessions) has its own `SessionState` with independent:
/// - Driver-layer session (SSOT for `mode_stack`, `pending_keys`, `extensions`, etc.)
/// - Kernel context (buffers, events, options)
/// - Mode/command/keymap registries
///
/// # SSOT Architecture
///
/// `driver_session` is the Single Source of Truth for per-session state.
/// `AppState` provides server-specific state that doesn't belong in the driver.
///
/// # Thread Safety
///
/// `SessionState` is NOT `Sync` by itself. The `Session` wrapper provides
/// thread-safe access via `RwLock<SessionState>`.
pub struct SessionState {
    /// Driver-layer session state (SSOT for buffers and shared resources).
    ///
    /// # Multi-Client Warning (#471)
    ///
    /// This contains SHARED state used by ALL clients. In multi-client scenarios:
    ///
    /// | Field | Status | Use Instead |
    /// |-------|--------|-------------|
    /// | `mode_stack` | **DEPRECATED** | `Client::Owner.state.mode_stack` |
    /// | `pending_keys` | **DEPRECATED** | `Client::Owner.state.pending_keys` |
    /// | `windows` | Shared (layout) | Per-client cursor in `EditingState.cursor` |
    /// | `extensions` | Shared | Module state is inherently shared |
    /// | `active_buffer` | Shared | All clients see same buffers |
    ///
    /// **DO NOT** access `driver_session.mode_stack` directly for key resolution.
    /// Use `Session::resolve_key_for_client()` which routes through per-client state.
    pub driver_session: DriverSession,

    /// Application state (kernel + server-specific state).
    ///
    /// Contains: kernel context, running flag, windows, cmdline.
    /// NOTE: `mode_stack`, `pending_keys`, `extensions`, `active_buffer`, and
    /// `terminal_size` in `AppState` are DEPRECATED - use `driver_session` instead.
    pub app: AppState,

    /// Virtual filesystem driver for file operations.
    ///
    /// Commands access files through this VFS abstraction rather than
    /// using `std::fs` directly.
    pub vfs: Arc<dyn VfsDriver>,

    /// Registry of mode metadata and behavior.
    pub mode_registry: ModeRegistry,

    /// Registry of command handlers.
    pub command_registry: CommandRegistry,

    /// Registry of keybindings.
    pub keymap_registry: KeymapRegistry,

    /// Registry of mode key resolvers.
    ///
    /// Resolvers implement mode-specific key handling policy:
    /// - Operator interception (keys that enter operator-pending mode)
    /// - Motion handling (keys that compute cursor ranges)
    /// - Line-operator detection (repeated operator keys)
    pub resolver_registry: ResolverRegistry,

    /// Session-scoped shared registers (A-Z) (#515 Phase 5).
    ///
    /// All clients in the session read/write from this shared storage.
    /// Accessed via `Register::Session('A')` through `Register::Session('Z')`.
    /// Provides cross-client register sharing within a single session.
    pub session_registers: HashMap<char, RegisterContent>,
}

impl SessionState {
    /// Create a new session state.
    ///
    /// # Arguments
    ///
    /// * `kernel` - The kernel context for this session
    /// * `initial_mode` - The home mode for new clients joining this session
    /// * `vfs` - The virtual filesystem driver for file operations
    #[must_use]
    pub fn new(kernel: KernelContext, initial_mode: ModeId, vfs: Arc<dyn VfsDriver>) -> Self {
        // Create driver session with home_mode in SessionShared (#491)
        // ClientId(0) is a placeholder - real clients get IDs from server layer
        let driver_session = DriverSession::new(ClientId::new(0), initial_mode);

        Self {
            driver_session,
            app: AppState::new(kernel),
            vfs,
            mode_registry: ModeRegistry::new(),
            command_registry: CommandRegistry::new(),
            keymap_registry: KeymapRegistry::new(),
            resolver_registry: ResolverRegistry::new(),
            session_registers: HashMap::new(),
        }
    }

    /// Create session state with existing registries.
    ///
    /// Used when modules need to populate registries before creating
    /// the session state.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn with_registries(
        kernel: KernelContext,
        initial_mode: ModeId,
        vfs: Arc<dyn VfsDriver>,
        mode_registry: ModeRegistry,
        command_registry: CommandRegistry,
        keymap_registry: KeymapRegistry,
        resolver_registry: ResolverRegistry,
        compositor: Option<Box<dyn RootCompositor>>,
    ) -> Self {
        // Create driver session with home_mode in SessionShared (#491)
        let mut driver_session = DriverSession::new(ClientId::new(0), initial_mode);

        // Set compositor if provided by a module
        if let Some(c) = compositor {
            driver_session.set_compositor(c);
        }

        // Create initial window in compositor if kernel has any buffers.
        // Per-client active_buffer is set in EditingState when clients connect.
        let buffer_ids = kernel.buffers.list();
        if !buffer_ids.is_empty()
            && let Some(compositor) = driver_session.compositor_mut()
            && let Some(active_layer) = compositor.active_layer()
            && let Some(layer) = compositor.layer_compositor_mut(active_layer)
            && layer
                .windows_in_zone(reovim_driver_layout::Zone::Tiled)
                .is_empty()
        {
            let _window_id = layer.add_tiled();
            tracing::debug!("Created initial window in compositor");
        }

        Self {
            driver_session,
            app: AppState::new(kernel),
            vfs,
            mode_registry,
            command_registry,
            keymap_registry,
            resolver_registry,
            session_registers: HashMap::new(),
        }
    }

    /// Ensure the compositor has at least one tiled window.
    ///
    /// Called after scratch buffer creation to handle the case where
    /// `with_registries()` ran before any buffers existed.
    pub fn ensure_initial_compositor_window(&mut self) {
        if self.app.kernel.buffers.list().is_empty() {
            return;
        }
        let Some(compositor) = self.driver_session.compositor_mut() else {
            return;
        };
        let Some(active_layer) = compositor.active_layer() else {
            return;
        };
        let Some(layer) = compositor.layer_compositor_mut(active_layer) else {
            return;
        };
        if layer
            .windows_in_zone(reovim_driver_layout::Zone::Tiled)
            .is_empty()
        {
            layer.add_tiled();
        }
    }

    /// Get a reference to the driver session (SSOT for session state).
    #[must_use]
    pub const fn driver_session(&self) -> &DriverSession {
        &self.driver_session
    }

    /// Get a mutable reference to the driver session.
    #[allow(clippy::missing_const_for_fn)]
    pub fn driver_session_mut(&mut self) -> &mut DriverSession {
        &mut self.driver_session
    }

    // ========================================================================
    // Delegation Methods (SSOT in driver_session.shared)
    // ========================================================================
    //
    // NOTE (#491/#471): mode_stack, extensions, active_buffer, terminal_size
    // are all per-client state now. Access via Session::client_state().
    // Only compositor and home_mode remain in SessionShared.

    /// Get the home mode for initializing new clients (#491).
    ///
    /// When a new client connects, their mode stack is initialized with
    /// this mode at the bottom. This is stored in `SessionShared`.
    #[must_use]
    pub const fn home_mode(&self) -> &ModeId {
        self.driver_session.shared.home_mode()
    }

    // ========================================================================
    // Registry Accessors
    // ========================================================================

    // NOTE (#491): current_mode() removed. Per-client mode lives in EditingState.
    // Use Session::client_current_mode(client_id) or home_mode() instead.

    /// Look up a key sequence in the current mode's keymap.
    #[must_use]
    pub fn lookup_keys(
        &self,
        mode: &ModeId,
        keys: &reovim_driver_input::KeySequence,
    ) -> KeyLookupResult {
        self.keymap_registry.lookup(mode, keys)
    }

    /// Execute a command with per-client state (#471, #477).
    ///
    /// This enables multi-client isolation by operating on per-client
    /// mode, cursor, and extension state instead of shared session state.
    ///
    /// # Arguments
    ///
    /// * `client_mode_stack` - Per-client mode stack (source of truth for mode)
    /// * `client_windows` - Per-client window layout (source of truth for cursor)
    /// * `client_extensions` - Per-client module extensions (#477)
    /// * `id` - The command ID to execute
    /// * `args` - Command arguments (count, register, etc.)
    #[must_use]
    pub fn execute_command_for_client(
        &mut self,
        client_id: usize,
        client: reovim_driver_session::ClientContext<'_>,
        id: &CommandId,
        args: &CommandContext,
    ) -> Option<(
        CommandResult,
        reovim_driver_session::api::StateChanges,
        Vec<reovim_driver_command_types::RuntimeSignal>,
    )> {
        // Flush pending edits before command execution
        self.app.flush_pending_edits();
        // Use per-client state (#471, #477, #515)
        // Pass client_id for per-client undo support
        // Pass shared extensions for session-wide state (#543)
        let kernel = &self.app.kernel;
        let shared_ext = &mut self.app.extensions;
        self.command_registry.execute_for_client(
            client_id,
            id,
            &mut self.driver_session,
            client,
            kernel,
            &self.vfs,
            args,
            Some(shared_ext),
        )
    }

    /// Check if the home mode accepts character input.
    ///
    /// NOTE (#491): This uses `home_mode()` since per-client mode lives in `EditingState`.
    /// For per-client mode checking, use the per-client state directly.
    #[must_use]
    pub fn mode_accepts_char_input(&self) -> bool {
        self.mode_registry.accepts_char_input(self.home_mode())
    }

    /// Check if the session should continue running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.app.is_running()
    }

    /// Request the session to quit.
    pub fn request_quit(&mut self) {
        self.app.request_quit();
    }

    /// Request clients to detach (server continues running).
    pub fn request_detach(&mut self) {
        self.app.request_detach();
    }

    /// Get the resolver registry.
    #[must_use]
    pub const fn resolver_registry(&self) -> &ResolverRegistry {
        &self.resolver_registry
    }

    // ========================================================================
    // Buffer Methods (delegated to kernel)
    // ========================================================================

    /// Get a buffer by ID from the unified buffer manager.
    #[must_use]
    pub fn buffer(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>> {
        self.app.kernel.buffers.get(id)
    }

    /// Create a new buffer with the given content.
    ///
    /// Returns the buffer ID. Per-client `active_buffer` is managed by
    /// `EditingState` — callers must set it there if needed.
    ///
    /// Uses `Buffer::from_string` so buffers start with `modified = false`.
    pub fn create_buffer(&mut self, content: &str) -> BufferId {
        let buffer = Buffer::from_string(content);
        self.app
            .kernel
            .buffers
            .register(Arc::new(RwLock::new(buffer)))
    }

    /// Resolve a key event using the resolver registry.
    ///
    /// This is the primary key resolution method that handles:
    /// - Operator interception (entering operator-pending mode)
    /// - Mode-specific key handling (via registered resolvers)
    /// - Extension access for module state
    ///
    /// # Returns
    ///
    /// - `Some(ResolveResult)` - if a resolver handled the key
    /// - `None` - if no resolver is registered for the current mode
    pub fn resolve_key(
        &mut self,
        key: &reovim_driver_input::KeyEvent,
    ) -> Option<(reovim_driver_input::ResolveResult, reovim_driver_session::api::StateChanges)>
    {
        use {
            reovim_driver_input::ModeState,
            reovim_driver_session::{
                SessionRuntime,
                api::{CommandExecutor, CommandHandle},
            },
        };

        // Stub command executor - commands are executed separately
        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn get_handle(&self, _id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
                None
            }
        }

        // Phase #491: Use home_mode since current_mode() removed.
        // This method is DEPRECATED - use resolve_key_for_client() with per-client state.
        let home_mode = self.driver_session.shared.home_mode().clone();
        let mode = home_mode.clone();
        let mut mode_state = ModeState::new(mode.clone());

        // Create SessionRuntime for resolver access to session state
        // #471 Phase 0: Create temporary per-client state for backward compatibility.
        // This is DEPRECATED - use resolve_key_for_client() with proper per-client state.
        let stub_executor = StubExecutor;
        let mut temp_mode_stack = ModeStack::new(home_mode);
        let mut temp_windows = reovim_driver_session::WindowLayout::empty();
        let mut runtime_ext = reovim_driver_session::ExtensionMap::new();
        let mut temp_client_extensions = reovim_driver_session::ExtensionMap::new();
        let mut temp_compositor = None;
        let mut temp_tabs = reovim_driver_session::TabPageSet::new();
        let mut temp_registers = reovim_types_text::RegisterBank::new();
        let mut temp_clipboard_history = reovim_types_text::HistoryRing::new();
        let mut temp_local_marks = reovim_kernel::api::v1::MarkBank::new();
        let mut temp_jumplist = Jumplist::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut self.driver_session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut temp_mode_stack,
                windows: &mut temp_windows,
                extensions: &mut runtime_ext,
                compositor: &mut temp_compositor,
                tabs: &mut temp_tabs,
                registers: &mut temp_registers,
                clipboard_history: &mut temp_clipboard_history,
                local_marks: &mut temp_local_marks,
                jumplist: &mut temp_jumplist,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &self.app.kernel,
            &stub_executor,
        );

        // Call resolver
        let result = self.resolver_registry.resolve_with_session(
            &mode,
            key,
            &mut mode_state,
            &self.keymap_registry,
            &mut runtime,
            &mut self.app.extensions,
            &mut temp_client_extensions,
        );

        // Take accumulated changes
        let changes = reovim_driver_session::api::ChangeTracker::take_changes(&mut runtime);

        result.map(|r| (r, changes))
    }

    /// Resolve a key event with per-client mode stack (#471).
    ///
    /// Like `resolve_key()`, but uses a provided per-client mode stack instead
    /// of the shared session mode stack. This enables multi-client mode isolation
    /// where each client has independent mode state.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client ID for undo origin tracking (#471 Phase 5)
    /// * `client_mode_stack` - Per-client mode stack (from server-level `EditingState`)
    /// * `client_windows` - Per-client window layout
    /// * `client_extensions` - Per-client extensions
    /// * `key` - The key event to resolve
    ///
    /// # Returns
    ///
    /// - `Some((ResolveResult, StateChanges))` - if a resolver handled the key
    /// - `None` - if no resolver is registered for the current mode
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Get per-client EditingState
    /// let editing_state = session.client_state_mut(client_id)?;
    ///
    /// // Resolve key with per-client state
    /// let client = editing_state.client_context();
    /// let result = session_state.resolve_key_for_client(client_id, client, &key);
    /// ```
    #[allow(clippy::too_many_lines)] // PendingBindings population requires match arms
    pub fn resolve_key_for_client(
        &mut self,
        client_id: usize,
        client: reovim_driver_session::ClientContext<'_>,
        key: &reovim_driver_input::KeyEvent,
    ) -> Option<(reovim_driver_input::ResolveResult, reovim_driver_session::api::StateChanges)>
    {
        use {
            reovim_driver_input::ModeState,
            reovim_driver_session::{
                ClientId as DriverClientId, SessionRuntime,
                api::{CommandExecutor, CommandHandle},
            },
        };

        // Stub command executor - commands are executed separately
        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn get_handle(&self, _id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
                None
            }
        }

        let reovim_driver_session::ClientContext {
            mode_stack: client_mode_stack,
            windows: client_windows,
            extensions: client_extensions,
            compositor: client_compositor,
            tabs: client_tabs,
            registers: client_registers,
            clipboard_history: client_clipboard_history,
            local_marks: client_local_marks,
            jumplist: client_jumplist,
            active_buffer: client_active_buffer,
            terminal_size: client_terminal_size,
        } = client;

        // Phase #471, #477: Use per-client state for resolution
        let mode = client_mode_stack.current().clone();
        let mut mode_state = ModeState::new(mode.clone());

        // Create SessionRuntime with per-client state and owner (#471 Phase 5)
        // The owner enables undo_mine()/redo_mine() for per-client undo
        //
        // Use placeholder extensions in runtime - resolvers access session only
        // via SessionApiDyn (excludes ExtensionApi), so placeholder is safe.
        let stub_executor = StubExecutor;
        let driver_client_id = DriverClientId::new(client_id);
        let mut runtime_ext = reovim_driver_session::ExtensionMap::new();
        let mut runtime = SessionRuntime::with_owner(
            driver_client_id,
            &mut self.driver_session,
            reovim_driver_session::ClientContext {
                mode_stack: client_mode_stack,
                windows: client_windows,
                extensions: &mut runtime_ext,
                compositor: client_compositor,
                tabs: client_tabs,
                registers: client_registers,
                clipboard_history: client_clipboard_history,
                local_marks: client_local_marks,
                jumplist: client_jumplist,
                active_buffer: client_active_buffer,
                terminal_size: client_terminal_size,
            },
            &self.app.kernel,
            &stub_executor,
        );

        // Call resolver - mode operations will use client_mode_stack
        let result = self.resolver_registry.resolve_with_session(
            &mode,
            key,
            &mut mode_state,
            &self.keymap_registry,
            &mut runtime,
            &mut self.app.extensions,
            client_extensions,
        );

        // Generic PendingBindings population for bridge consumers (#468).
        // After resolution, populate PendingBindings so bridges (e.g., WhichKeyBridge)
        // can produce UI hints without knowing about specific resolvers.
        match &result {
            Some(reovim_driver_input::ResolveResult::Pending) => {
                let pending = self.resolver_registry.pending_keys_for(&mode);
                if !pending.is_empty() {
                    let mut continuations =
                        self.keymap_registry.bindings_with_prefix(&mode, &pending);
                    // Include parent mode bindings (consistent with Push arm)
                    if let Some(resolver) = self.resolver_registry.get(&mode)
                        && let Some(parent) = resolver.inherits_from()
                    {
                        let parent_bindings =
                            self.keymap_registry.bindings_with_prefix(parent, &pending);
                        continuations.extend(parent_bindings);
                    }
                    let pb = client_extensions.get_or_insert::<PendingBindings>();
                    // Preserve mode_prefix from Push (e.g., trigger key for operator mode)
                    pb.pending_keys = pending;
                    pb.mode = mode;
                    pb.continuations = continuations;
                }
            }
            Some(reovim_driver_input::ResolveResult::ModeTransition(
                reovim_driver_input::ModeTransition::Push {
                    mode: target_mode, ..
                },
            )) => {
                // On mode push (e.g., entering an operator-pending mode),
                // populate PendingBindings with the new mode's available bindings
                // so which-key can show hints immediately on mode entry.
                let trigger_key = reovim_driver_input::KeySequence::from_keys(&[*key]);
                let empty = reovim_driver_input::KeySequence::new();
                let mut continuations = self
                    .keymap_registry
                    .bindings_with_prefix(target_mode, &empty);
                // Include parent mode bindings (operator modes inherit motions)
                if let Some(resolver) = self.resolver_registry.get(target_mode)
                    && let Some(parent) = resolver.inherits_from()
                {
                    let parent_bindings = self.keymap_registry.bindings_with_prefix(parent, &empty);
                    continuations.extend(parent_bindings);
                }
                if !continuations.is_empty() {
                    let pb = client_extensions.get_or_insert::<PendingBindings>();
                    pb.mode_prefix = trigger_key;
                    pb.pending_keys = reovim_driver_input::KeySequence::new();
                    pb.mode = target_mode.clone();
                    pb.continuations = continuations;
                }
            }
            Some(_) => {
                // Non-pending, non-push: clear any previous pending bindings
                if let Some(pb) = client_extensions.get_mut::<PendingBindings>() {
                    pb.clear();
                }
            }
            None => {}
        }

        // Take accumulated changes
        let changes = reovim_driver_session::api::ChangeTracker::take_changes(&mut runtime);

        result.map(|r| (r, changes))
    }

    /// Try to call `on_command_complete` on the current mode's resolver.
    ///
    /// Called after executing a command from `ResolveResult::Execute`.
    /// For operator-pending modes, this is where the resolver reads
    /// the post-motion cursor position and builds the final command.
    ///
    /// # Flow
    ///
    /// 1. Key press → resolver returns `Execute(motion-command)`
    /// 2. Runner executes the motion → cursor moves
    /// 3. **This method** → resolver reads end position, returns
    ///    `ModeTransition::Pop { ExecuteCommand { operator, range } }`
    /// 4. Runner pops the operator mode and executes the operator command
    pub fn try_on_command_complete(&mut self) -> Option<reovim_driver_input::ModeTransition> {
        use reovim_driver_session::{
            SessionRuntime,
            api::{CommandExecutor, CommandHandle},
        };

        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn get_handle(&self, _id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
                None
            }
        }

        // Phase #491: Use home_mode since current_mode() removed.
        // This method is DEPRECATED - use try_on_command_complete_for_client() with per-client state.
        let home_mode = self.driver_session.shared.home_mode().clone();
        let mode = home_mode.clone();
        let resolver = self.resolver_registry.get(&mode)?;

        // #471 Phase 0: Create temporary per-client state for backward compatibility.
        let stub_executor = StubExecutor;
        let mut temp_mode_stack = ModeStack::new(home_mode);
        let mut temp_windows = reovim_driver_session::WindowLayout::empty();
        let mut runtime_ext = reovim_driver_session::ExtensionMap::new();
        let mut temp_client_extensions = reovim_driver_session::ExtensionMap::new();
        let mut temp_compositor = None;
        let mut temp_tabs = reovim_driver_session::TabPageSet::new();
        let mut temp_registers = reovim_types_text::RegisterBank::new();
        let mut temp_clipboard_history = reovim_types_text::HistoryRing::new();
        let mut temp_local_marks = reovim_kernel::api::v1::MarkBank::new();
        let mut temp_jumplist = Jumplist::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut self.driver_session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut temp_mode_stack,
                windows: &mut temp_windows,
                extensions: &mut runtime_ext,
                compositor: &mut temp_compositor,
                tabs: &mut temp_tabs,
                registers: &mut temp_registers,
                clipboard_history: &mut temp_clipboard_history,
                local_marks: &mut temp_local_marks,
                jumplist: &mut temp_jumplist,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &self.app.kernel,
            &stub_executor,
        );

        resolver.on_command_complete(
            &mut runtime,
            &mut self.app.extensions,
            &mut temp_client_extensions,
        )
    }

    /// Try to call `on_command_complete` with per-client state (#471, #477).
    ///
    /// Like `try_on_command_complete()`, but uses per-client mode stack, windows,
    /// and extensions instead of the shared session state.
    ///
    /// # Arguments
    ///
    /// * `client` - Per-client state bundle (from server-level `EditingState`)
    ///
    /// # Returns
    ///
    /// A `ModeTransition` if the resolver wants to change modes.
    pub fn try_on_command_complete_for_client(
        &mut self,
        client_id: usize,
        client: reovim_driver_session::ClientContext<'_>,
    ) -> Option<reovim_driver_input::ModeTransition> {
        use reovim_driver_session::{
            ClientId as DriverClientId, SessionRuntime,
            api::{CommandExecutor, CommandHandle},
        };

        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn get_handle(&self, _id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
                None
            }
        }

        let reovim_driver_session::ClientContext {
            mode_stack: client_mode_stack,
            windows: client_windows,
            extensions: client_extensions,
            compositor: client_compositor,
            tabs: client_tabs,
            registers: client_registers,
            clipboard_history: client_clipboard_history,
            local_marks: client_local_marks,
            jumplist: client_jumplist,
            active_buffer: client_active_buffer,
            terminal_size: client_terminal_size,
        } = client;

        // Phase #471, #477, #515: Use per-client state with owner for per-client undo
        //
        // Use placeholder extensions in runtime - resolvers access session only
        // via SessionApiDyn (excludes ExtensionApi), so placeholder is safe.
        let mode = client_mode_stack.current().clone();
        let resolver = self.resolver_registry.get(&mode)?;
        let stub_executor = StubExecutor;
        let driver_client_id = DriverClientId::new(client_id);
        let mut runtime_ext = reovim_driver_session::ExtensionMap::new();
        let mut runtime = SessionRuntime::with_owner(
            driver_client_id,
            &mut self.driver_session,
            reovim_driver_session::ClientContext {
                mode_stack: client_mode_stack,
                windows: client_windows,
                extensions: &mut runtime_ext,
                compositor: client_compositor,
                tabs: client_tabs,
                registers: client_registers,
                clipboard_history: client_clipboard_history,
                local_marks: client_local_marks,
                jumplist: client_jumplist,
                active_buffer: client_active_buffer,
                terminal_size: client_terminal_size,
            },
            &self.app.kernel,
            &stub_executor,
        );

        resolver.on_command_complete(&mut runtime, &mut self.app.extensions, client_extensions)
    }
}

impl Default for SessionState {
    fn default() -> Self {
        // Create with default mode (will be overwritten by modules)
        let mode = ModeId::new(reovim_kernel::api::v1::ModuleId::new("default"), "normal");
        let vfs: Arc<dyn VfsDriver> = Arc::new(reovim_driver_vfs::MockVfs::new());
        Self::new(KernelContext::default(), mode, vfs)
    }
}

impl SessionState {
    /// Create a session state with a custom kernel context.
    ///
    /// Useful for testing with non-default buffer managers.
    #[must_use]
    pub fn with_kernel(kernel: KernelContext) -> Self {
        let mode = ModeId::new(reovim_kernel::api::v1::ModuleId::new("default"), "normal");
        let vfs: Arc<dyn VfsDriver> = Arc::new(reovim_driver_vfs::MockVfs::new());
        Self::new(kernel, mode, vfs)
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
