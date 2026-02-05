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

use std::sync::Arc;

use {
    parking_lot::RwLock,
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_display::layout::RootCompositor,
    reovim_driver_input::{ExtensionMap, FallbackContext, ResolverRegistry},
    reovim_driver_session::{ClientId, Session as DriverSession, Window},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{Buffer, BufferId, CommandId, KernelContext, ModeId, ModeStack},
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
    /// - Operator interception (d, y, c enter operator-pending mode)
    /// - Motion handling (w, b, j, k compute ranges)
    /// - Line-operator detection (dd, yy, cc)
    pub resolver_registry: ResolverRegistry,
}

impl SessionState {
    /// Create a new session state.
    ///
    /// # Arguments
    ///
    /// * `kernel` - The kernel context for this session
    /// * `initial_mode` - The mode to start in
    /// * `vfs` - The virtual filesystem driver for file operations
    #[must_use]
    pub fn new(kernel: KernelContext, initial_mode: ModeId, vfs: Arc<dyn VfsDriver>) -> Self {
        // Create driver session (SSOT for session state)
        // ClientId(0) for single-session model
        let driver_session = DriverSession::new(ClientId::new(0), initial_mode);

        Self {
            driver_session,
            app: AppState::new(kernel),
            vfs,
            mode_registry: ModeRegistry::new(),
            command_registry: CommandRegistry::new(),
            keymap_registry: KeymapRegistry::new(),
            resolver_registry: ResolverRegistry::new(),
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
        // Create driver session (SSOT for session state)
        let mut driver_session = DriverSession::new(ClientId::new(0), initial_mode);

        // Set compositor if provided by a module
        if let Some(c) = compositor {
            driver_session.set_compositor(c);
        }

        // Set initial active buffer if kernel has any buffers
        let buffer_ids = kernel.buffers.list();
        if let Some(&first_buffer) = buffer_ids.first() {
            driver_session.set_active_buffer(Some(first_buffer));

            // Phase 8 (#465): Create Window in driver_session.windows for selection tracking.
            // This Window stores the per-window cursor and selection state.
            if driver_session.windows.windows.is_empty() {
                let window = Window::with_buffer(first_buffer);
                driver_session.windows.add(window);
                tracing::debug!("Created initial window in driver_session.windows");
            }

            // Create initial window in compositor for the first buffer.
            if let Some(compositor) = driver_session.compositor_mut()
                && let Some(active_layer) = compositor.active_layer()
                && let Some(layer) = compositor.layer_compositor_mut(active_layer)
                && layer
                    .windows_in_zone(reovim_driver_display::layout::Zone::Tiled)
                    .is_empty()
            {
                let _window_id = layer.add_tiled();
                tracing::debug!("Created initial window in compositor");
            }
        }

        Self {
            driver_session,
            app: AppState::new(kernel),
            vfs,
            mode_registry,
            command_registry,
            keymap_registry,
            resolver_registry,
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
    // Delegation Methods (SSOT in driver_session)
    // ========================================================================

    /// Get a reference to the mode stack (delegates to `driver_session`).
    #[must_use]
    pub const fn mode_stack(&self) -> &ModeStack {
        &self.driver_session.mode_stack
    }

    /// Get a mutable reference to the mode stack (delegates to `driver_session`).
    #[allow(clippy::missing_const_for_fn)]
    pub fn mode_stack_mut(&mut self) -> &mut ModeStack {
        &mut self.driver_session.mode_stack
    }

    /// Get a reference to the extensions map (delegates to `driver_session`).
    #[must_use]
    pub const fn extensions(&self) -> &ExtensionMap {
        &self.driver_session.extensions
    }

    /// Get a mutable reference to the extensions map (delegates to `driver_session`).
    #[allow(clippy::missing_const_for_fn)]
    pub fn extensions_mut(&mut self) -> &mut ExtensionMap {
        &mut self.driver_session.extensions
    }

    /// Get the session-level active buffer ID (delegates to `driver_session`).
    #[must_use]
    pub const fn session_active_buffer(&self) -> Option<BufferId> {
        self.driver_session.active_buffer()
    }

    /// Set the session-level active buffer ID (delegates to `driver_session`).
    pub const fn set_session_active_buffer(&mut self, id: Option<BufferId>) {
        self.driver_session.set_active_buffer(id);
    }

    /// Get the session-level terminal size (delegates to `driver_session`).
    #[must_use]
    pub const fn session_terminal_size(&self) -> (u16, u16) {
        self.driver_session.terminal_size()
    }

    /// Set the session-level terminal size (delegates to `driver_session`).
    pub const fn set_session_terminal_size(&mut self, width: u16, height: u16) {
        self.driver_session.set_terminal_size(width, height);
    }

    // ========================================================================
    // Registry Accessors
    // ========================================================================

    /// Get the current mode ID from SHARED session state.
    ///
    /// # Deprecation Warning (#471)
    ///
    /// This returns the SHARED mode stack, which is DEPRECATED for multi-client.
    /// For per-client mode, use `Session::client_current_mode(client_id)` instead.
    ///
    /// This method should ONLY be used for:
    /// - Initial mode for new clients (in `Session::add_client()`)
    /// - Single-client test scenarios
    /// - Fallback when `client_id` is not available
    ///
    /// # Migration
    ///
    /// ```ignore
    /// // BEFORE (deprecated):
    /// let mode = session.with_state(|s| s.current_mode().clone());
    ///
    /// // AFTER (correct for multi-client):
    /// let mode = session.client_current_mode(client_id)
    ///     .unwrap_or_else(|| session.with_state_sync(|s| s.current_mode().clone()));
    /// ```
    #[must_use]
    #[deprecated(
        since = "0.9.0",
        note = "Use Session::client_current_mode() for per-client mode isolation. \
                This method returns SHARED mode which is incorrect for multi-client scenarios."
    )]
    pub fn current_mode(&self) -> &ModeId {
        self.driver_session.current_mode()
    }

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
        client_mode_stack: &mut reovim_kernel::api::v1::ModeStack,
        client_windows: &mut reovim_driver_session::WindowLayout,
        client_extensions: &mut reovim_driver_session::ExtensionMap,
        id: &CommandId,
        args: &CommandContext,
    ) -> Option<(CommandResult, reovim_driver_session::api::StateChanges)> {
        // Flush pending edits before command execution
        self.app.flush_pending_edits();
        // Use per-client state (#471, #477)
        self.command_registry.execute_for_client(
            id,
            &mut self.driver_session,
            client_mode_stack,
            client_windows,
            client_extensions,
            &self.app,
            &self.vfs,
            args,
        )
    }

    /// Check if the current mode accepts character input.
    #[must_use]
    pub fn mode_accepts_char_input(&self) -> bool {
        // Use driver_session as SSOT for current mode
        self.mode_registry
            .accepts_char_input(self.driver_session.current_mode())
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

    /// Get a buffer by ID.
    #[must_use]
    pub fn buffer(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
        self.app.kernel.buffers.get(id)
    }

    /// Get the active buffer ID (if any).
    ///
    /// Uses `driver_session` as SSOT.
    #[must_use]
    pub const fn active_buffer(&self) -> Option<BufferId> {
        self.driver_session.active_buffer()
    }

    /// Set the active buffer.
    ///
    /// Delegates to `driver_session` which is SSOT.
    #[allow(clippy::missing_const_for_fn)] // driver_session method may not be const
    pub fn set_active_buffer(&mut self, id: Option<BufferId>) {
        self.driver_session.set_active_buffer(id);
    }

    /// Create a new buffer with the given content.
    pub fn create_buffer(&mut self, content: &str) -> BufferId {
        let mut buffer = Buffer::new();
        buffer.set_content(content);
        let id = self.app.kernel.buffers.register(buffer);

        // Set as active if this is the first buffer
        if self.driver_session.active_buffer().is_none() {
            self.driver_session.set_active_buffer(Some(id));
        }

        // Phase 8 (#465): Ensure there's always a Window for selection tracking.
        // If no windows exist, create one for this buffer.
        if self.driver_session.windows.windows.is_empty() {
            let window = Window::with_buffer(id);
            self.driver_session.windows.add(window);
            tracing::debug!(?id, "Created window for buffer in driver_session.windows");
        } else if let Some(window) = self.driver_session.windows.active_mut()
            && window.buffer_id.is_none()
        {
            // Assign to active window if it has no buffer
            window.buffer_id = Some(id);
            tracing::debug!(?id, "Assigned buffer to active window");
        }

        id
    }

    /// Resolve a key event using the resolver registry.
    ///
    /// This is the primary key resolution method that handles:
    /// - Operator interception (d, y, c → operator-pending mode)
    /// - Mode-specific key handling (via registered resolvers)
    /// - Extension access for module state (`VimSessionState`)
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
            reovim_driver_session::{SessionRuntime, api::CommandExecutor},
        };

        // Stub command executor - commands are executed separately
        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn execute(
                &self,
                _cmd: &CommandId,
                _ctx: &CommandContext,
                _kernel: &reovim_kernel::api::v1::KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }

        let mode = self.driver_session.current_mode().clone();
        let mut mode_state = ModeState::new(mode.clone());

        // Create SessionRuntime for resolver access to session state
        // #471 Phase 0: Create temporary per-client state for backward compatibility.
        // This is DEPRECATED - use resolve_key_for_client() with proper per-client state.
        let stub_executor = StubExecutor;
        let home_mode = self.driver_session.mode_stack.current().clone();
        let mut temp_mode_stack = ModeStack::new(home_mode);
        let mut temp_windows = reovim_driver_session::WindowLayout::empty();
        let mut temp_extensions = reovim_driver_session::ExtensionMap::new();

        let mut runtime = SessionRuntime::new(
            &mut self.driver_session,
            &mut temp_mode_stack,
            &mut temp_windows,
            &mut temp_extensions,
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
    /// * `client_mode_stack` - Per-client mode stack (from server-level `EditingState`)
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
    /// let result = session_state.resolve_key_for_client(
    ///     &mut editing_state.mode_stack,
    ///     &mut editing_state.windows,
    ///     &mut editing_state.extensions,
    ///     &key,
    /// );
    /// ```
    pub fn resolve_key_for_client(
        &mut self,
        client_mode_stack: &mut ModeStack,
        client_windows: &mut reovim_driver_session::WindowLayout,
        client_extensions: &mut reovim_driver_session::ExtensionMap,
        key: &reovim_driver_input::KeyEvent,
    ) -> Option<(reovim_driver_input::ResolveResult, reovim_driver_session::api::StateChanges)>
    {
        use {
            reovim_driver_input::ModeState,
            reovim_driver_session::{SessionRuntime, api::CommandExecutor},
        };

        // Stub command executor - commands are executed separately
        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn execute(
                &self,
                _cmd: &CommandId,
                _ctx: &CommandContext,
                _kernel: &reovim_kernel::api::v1::KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }

        // Phase #471, #477: Use per-client state for resolution
        let mode = client_mode_stack.current().clone();
        let mut mode_state = ModeState::new(mode.clone());

        // Create SessionRuntime with per-client state (#471, #477)
        let stub_executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut self.driver_session,
            client_mode_stack,
            client_windows,
            client_extensions,
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
        );

        // Take accumulated changes
        let changes = reovim_driver_session::api::ChangeTracker::take_changes(&mut runtime);

        result.map(|r| (r, changes))
    }

    /// Try to call `on_command_complete` on the current mode's resolver.
    ///
    /// Called after executing a command from `ResolveResult::Execute`.
    /// For operator-pending modes (delete, yank, change), this is where
    /// the resolver reads the post-motion cursor position and builds
    /// the final operator command.
    ///
    /// # Flow (e.g., `dw`)
    ///
    /// 1. `w` key → DELETE resolver returns `Execute(word-forward)`
    /// 2. Runner executes `word-forward` → cursor moves to next word
    /// 3. **This method** → DELETE resolver reads end position, returns
    ///    `ModeTransition::Pop { ExecuteCommand { delete, range } }`
    /// 4. Runner pops DELETE mode and executes the delete command
    pub fn try_on_command_complete(&mut self) -> Option<reovim_driver_input::ModeTransition> {
        use reovim_driver_session::{SessionRuntime, api::CommandExecutor};

        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn execute(
                &self,
                _cmd: &CommandId,
                _ctx: &CommandContext,
                _kernel: &reovim_kernel::api::v1::KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }

        let mode = self.driver_session.current_mode().clone();
        let resolver = self.resolver_registry.get(&mode)?;

        // #471 Phase 0: Create temporary per-client state for backward compatibility.
        // This is DEPRECATED - use try_on_command_complete_for_client() with proper per-client state.
        let stub_executor = StubExecutor;
        let home_mode = self.driver_session.mode_stack.current().clone();
        let mut temp_mode_stack = ModeStack::new(home_mode);
        let mut temp_windows = reovim_driver_session::WindowLayout::empty();
        let mut temp_extensions = reovim_driver_session::ExtensionMap::new();

        let mut runtime = SessionRuntime::new(
            &mut self.driver_session,
            &mut temp_mode_stack,
            &mut temp_windows,
            &mut temp_extensions,
            &self.app.kernel,
            &stub_executor,
        );

        resolver.on_command_complete(&mut runtime, &mut self.app.extensions)
    }

    /// Try to call `on_command_complete` with per-client state (#471, #477).
    ///
    /// Like `try_on_command_complete()`, but uses per-client mode stack, windows,
    /// and extensions instead of the shared session state.
    ///
    /// # Arguments
    ///
    /// * `client_mode_stack` - Per-client mode stack (from server-level `EditingState`)
    /// * `client_windows` - Per-client window layout (from server-level `EditingState`)
    /// * `client_extensions` - Per-client module extensions (#477)
    ///
    /// # Returns
    ///
    /// A `ModeTransition` if the resolver wants to change modes.
    pub fn try_on_command_complete_for_client(
        &mut self,
        client_mode_stack: &mut ModeStack,
        client_windows: &mut reovim_driver_session::WindowLayout,
        client_extensions: &mut reovim_driver_session::ExtensionMap,
    ) -> Option<reovim_driver_input::ModeTransition> {
        use reovim_driver_session::{SessionRuntime, api::CommandExecutor};

        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn execute(
                &self,
                _cmd: &CommandId,
                _ctx: &CommandContext,
                _kernel: &reovim_kernel::api::v1::KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }

        // Phase #471, #477: Use per-client state
        let mode = client_mode_stack.current().clone();
        let resolver = self.resolver_registry.get(&mode)?;
        let stub_executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut self.driver_session,
            client_mode_stack,
            client_windows,
            client_extensions,
            &self.app.kernel,
            &stub_executor,
        );

        resolver.on_command_complete(&mut runtime, &mut self.app.extensions)
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
mod tests {
    use {
        super::*,
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, MarkBank, ModuleId, MotionEngine, OptionRegistry, RegisterBank,
            ServiceRegistry, TextObjectEngine,
        },
    };

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(reovim_driver_vfs::MockVfs::new())
    }

    /// Create a test kernel with a real buffer manager.
    fn test_kernel() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        )
    }

    #[test]
    #[allow(deprecated)] // Testing deprecated current_mode() directly
    fn test_session_state_new() {
        let kernel = KernelContext::default();
        let state = SessionState::new(kernel, test_mode_id(), test_vfs());

        assert!(state.is_running());
        assert!(state.mode_registry.is_empty());
        assert!(state.command_registry.is_empty());
        assert!(state.keymap_registry.is_empty());
        assert_eq!(state.current_mode().name(), "normal");
    }

    #[test]
    fn test_session_state_has_vfs() {
        let kernel = KernelContext::default();
        let state = SessionState::new(kernel, test_mode_id(), test_vfs());

        // VFS should be accessible
        assert!(!state.vfs.exists(std::path::Path::new("/nonexistent")));
    }

    #[test]
    fn test_session_state_quit() {
        let kernel = KernelContext::default();
        let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

        assert!(state.is_running());
        state.request_quit();
        assert!(!state.is_running());
    }

    #[test]
    fn test_session_state_lookup_keys_empty() {
        let kernel = KernelContext::default();
        let state = SessionState::new(kernel, test_mode_id(), test_vfs());

        let keys = reovim_driver_input::KeySequence::parse("j").unwrap();
        let result = state.lookup_keys(&test_mode_id(), &keys);

        assert!(result.is_not_found());
    }

    #[test]
    fn test_session_state_mode_accepts_char_input_default() {
        let kernel = KernelContext::default();
        let state = SessionState::new(kernel, test_mode_id(), test_vfs());

        // Unknown mode defaults to not accepting input
        assert!(!state.mode_accepts_char_input());
    }

    #[test]
    fn test_session_state_create_buffer() {
        // Use test_kernel() which has a real buffer manager
        let kernel = test_kernel();
        let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

        let id = state.create_buffer("hello world");
        assert!(state.buffer(id).is_some());
        assert_eq!(state.active_buffer(), Some(id));
    }

    /// Test `resolve_key_for_client` uses per-client state (#471, #477).
    ///
    /// Verifies that:
    /// 1. Key resolution uses the provided per-client state
    /// 2. The shared session state is NOT affected
    #[test]
    #[allow(deprecated)] // Testing that shared current_mode() is NOT modified by per-client resolution
    fn test_resolve_key_for_client_mode_isolation() {
        let kernel = KernelContext::default();
        let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

        // Create per-client state (#471, #477)
        let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
        let mut client_mode_stack = ModeStack::new(insert_mode);
        let mut client_windows = reovim_driver_session::WindowLayout::empty();
        let mut client_extensions = reovim_driver_session::ExtensionMap::new();

        // Verify initial states
        assert_eq!(state.current_mode().name(), "normal"); // shared
        assert_eq!(client_mode_stack.current().name(), "insert"); // per-client

        // resolve_key_for_client should use per-client state, not shared
        let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));

        // Call resolve_key_for_client - it will return None (no resolver)
        // but the important thing is it uses per-client state
        let _result = state.resolve_key_for_client(
            &mut client_mode_stack,
            &mut client_windows,
            &mut client_extensions,
            &key,
        );

        // Verify shared mode stack is NOT affected
        assert_eq!(state.current_mode().name(), "normal");
        // Client mode stack should still be in insert mode
        assert_eq!(client_mode_stack.current().name(), "insert");
    }
}
