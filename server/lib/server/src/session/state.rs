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
    reovim_driver_session::{ClientId, Session as DriverSession},
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
    /// Driver-layer session state (SSOT for `mode_stack`, `pending_keys`, `extensions`,
    /// `active_buffer`, `terminal_size`).
    ///
    /// This is the canonical source for per-session editing state.
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

    /// Get the current mode ID (from `driver_session` SSOT).
    #[must_use]
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

    /// Execute a command by ID.
    ///
    /// Returns `None` if the command isn't registered.
    ///
    /// Flushes any pending edits before execution to ensure undo batching
    /// works correctly (commands break insert mode batches).
    #[must_use]
    pub fn execute_command(
        &mut self,
        id: &CommandId,
        args: &CommandContext,
    ) -> Option<CommandResult> {
        // Flush pending edits before command execution
        self.app.flush_pending_edits();
        // Use driver_session as SSOT for mode_stack and active_buffer
        self.command_registry
            .execute(id, &mut self.driver_session, &mut self.app, &self.vfs, args)
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
                _kernel: &mut reovim_kernel::api::v1::KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }

        let mode = self.driver_session.current_mode().clone();
        let mut mode_state = ModeState::new(mode.clone());

        // Create SessionRuntime for resolver access to session state
        let stub_executor = StubExecutor;
        let mut runtime =
            SessionRuntime::new(&mut self.driver_session, &self.app.kernel, &stub_executor);

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
}
