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
//! The `AppState` within this struct provides runner-specific state (kernel,
//! `undo_registry`, windows, cmdline) that doesn't belong in the driver layer.

use std::sync::Arc;

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_display::layout::RootCompositor,
    reovim_driver_input::{ExtensionMap, FallbackContext},
    reovim_driver_session::{ClientId, Session as DriverSession},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{BufferId, CommandId, KernelContext, ModeId, ModeStack},
};

use crate::{
    AppState,
    module::ModuleManager,
    registry::{CommandRegistry, KeyLookupResult, KeymapRegistry, ModeRegistry},
};

// Epic #417 Part 2: UndoPersistence removed. Persistence is now internal to
// the undo module's UndoProvider implementation.

use reovim_driver_input::ResolverRegistry;

/// Session state combining application state with registries.
///
/// This is the complete state for a single editing session. Each session
/// (like tmux sessions) has its own `SessionState` with independent:
/// - Driver-layer session (SSOT for `mode_stack`, `pending_keys`, `extensions`, etc.)
/// - Kernel context (buffers, events, options)
/// - Mode/command/keymap/module registries
///
/// # SSOT Architecture
///
/// `driver_session` is the Single Source of Truth for per-session state.
/// `AppState` provides runner-specific state that doesn't belong in the driver.
///
/// # Thread Safety
///
/// `SessionState` is NOT `Sync` by itself. The `Session` wrapper provides
/// thread-safe access via `RwLock<SessionState>`.
///
/// # Example
///
/// ```ignore
/// use runner::session::SessionState;
/// use reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId};
///
/// let kernel = KernelContext::default();
/// let initial_mode = ModeId::new(ModuleId::new("editor"), "normal");
/// let state = SessionState::new(kernel, initial_mode);
///
/// // Process keys through the session state
/// let result = state.lookup_keys(&mode_id, &key_sequence);
/// ```
pub struct SessionState {
    /// Driver-layer session state (SSOT for `mode_stack`, `pending_keys`, `extensions`,
    /// `active_buffer`, `terminal_size`).
    ///
    /// This is the canonical source for per-session editing state.
    pub driver_session: DriverSession,

    /// Application state (kernel + runner-specific state).
    ///
    /// Contains: kernel context, running flag, `undo_registry`, windows, cmdline.
    /// NOTE: `mode_stack`, `pending_keys`, `extensions`, `active_buffer`, and
    /// `terminal_size` in `AppState` are DEPRECATED - use `driver_session` instead.
    pub app: AppState,

    /// Virtual filesystem driver for file operations.
    ///
    /// Commands access files through this VFS abstraction rather than
    /// using `std::fs` directly. This enables:
    /// - Test isolation via `MockVfs`
    /// - Future remote filesystem support
    /// - Per-session working directory
    pub vfs: Arc<dyn VfsDriver>,

    /// Registry of mode metadata and behavior.
    pub mode_registry: ModeRegistry,

    /// Registry of command handlers.
    pub command_registry: CommandRegistry,

    /// Registry of keybindings.
    pub keymap_registry: KeymapRegistry,

    /// Registry of loaded modules (static and dynamic).
    ///
    /// Each session owns its own module set, enabling per-session
    /// module loading and isolation (similar to Linux process contexts).
    pub module_registry: ModuleManager,

    /// Registry of mode key resolvers.
    ///
    /// Resolvers implement mode-specific key handling policy:
    /// - Operator interception (d, y, c enter operator-pending mode)
    /// - Motion handling (w, b, j, k compute ranges)
    /// - Line-operator detection (dd, yy, cc)
    pub resolver_registry: ResolverRegistry,
    // Epic #417 Part 2: undo_persistence removed.
    // Persistence is now internal to the undo module's UndoProvider implementation.
    // Use app.get_undo_provider() to access undo functionality.
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
        // Epic #417 Part 2: undo persistence removed from SessionState.
        // Persistence is now internal to UndoProvider (in undo module).

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
            module_registry: ModuleManager::new(),
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
        module_registry: ModuleManager,
        resolver_registry: ResolverRegistry,
        compositor: Option<Box<dyn RootCompositor>>,
    ) -> Self {
        // Epic #417 Part 2: undo persistence removed from SessionState.
        // Persistence is now internal to UndoProvider (in undo module).

        // Create driver session (SSOT for session state)
        // ClientId(0) for single-session model
        let mut driver_session = DriverSession::new(ClientId::new(0), initial_mode);

        // Set compositor if provided by a module
        if let Some(c) = compositor {
            driver_session.set_compositor(c);
        }

        // Set initial active buffer if kernel has any buffers
        // (e.g., scratch buffer created by empty session handler)
        let buffer_ids = kernel.buffers.list();
        if let Some(&first_buffer) = buffer_ids.first() {
            driver_session.set_active_buffer(Some(first_buffer));
        }

        Self {
            driver_session,
            app: AppState::new(kernel),
            vfs,
            mode_registry,
            command_registry,
            keymap_registry,
            module_registry,
            resolver_registry,
        }
    }

    /// Get a reference to the driver session (SSOT for session state).
    #[must_use]
    pub const fn driver_session(&self) -> &DriverSession {
        &self.driver_session
    }

    /// Get a mutable reference to the driver session.
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
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
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
    pub fn mode_stack_mut(&mut self) -> &mut ModeStack {
        &mut self.driver_session.mode_stack
    }

    /// Get a reference to the extensions map (delegates to `driver_session`).
    #[must_use]
    pub const fn extensions(&self) -> &ExtensionMap {
        &self.driver_session.extensions
    }

    /// Get a mutable reference to the extensions map (delegates to `driver_session`).
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
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

    /// Get a reference to the module registry.
    #[must_use]
    pub const fn module_registry(&self) -> &ModuleManager {
        &self.module_registry
    }

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
    ///
    /// # Context Population (Epic #415)
    ///
    /// VFS is passed from `SessionState` to the command registry,
    /// where all context enrichment happens in a single clone.
    #[must_use]
    pub fn execute_command(
        &mut self,
        id: &CommandId,
        args: &CommandContext,
    ) -> Option<CommandResult> {
        // Flush pending edits before command execution
        // (any command breaks insert mode batching)
        self.app.flush_pending_edits();
        // Use driver_session as SSOT for mode_stack and active_buffer
        // Pass VFS to command registry for context enrichment (Epic #415)
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
    ///
    /// # Note
    ///
    /// This method requires the `RuntimeAdapter` from `event_loop` module.
    /// Call `handle_resolve_result()` to process the result.
    pub fn resolve_key(
        &mut self,
        key: &reovim_driver_input::KeyEvent,
    ) -> Option<(reovim_driver_input::ResolveResult, reovim_driver_session::api::StateChanges)>
    {
        use {
            crate::server::event_loop::RuntimeAdapter,
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

        // Create runtime for resolver
        let stub_executor = StubExecutor;
        let session_runtime =
            SessionRuntime::new(&mut self.driver_session, &self.app.kernel, &stub_executor);
        let mut runtime =
            RuntimeAdapter::new(session_runtime, &mut self.app.windows, &self.app.kernel);

        // Call resolver
        // NOTE: Uses app.extensions due to borrow checker - driver_session is already borrowed by runtime
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

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_vfs::MockVfs, reovim_kernel::api::v1::ModuleId};

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(MockVfs::new())
    }

    #[test]
    fn test_session_state_new() {
        let kernel = KernelContext::default();
        let state = SessionState::new(kernel, test_mode_id(), test_vfs());

        assert!(state.is_running());
        assert!(state.mode_registry.is_empty());
        assert!(state.command_registry.is_empty());
        assert!(state.keymap_registry.is_empty());
        assert!(state.module_registry.is_empty());
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
    fn test_session_state_module_registry_accessor() {
        let kernel = KernelContext::default();
        let state = SessionState::new(kernel, test_mode_id(), test_vfs());

        // Verify we can access the module registry
        assert!(state.module_registry().is_empty());
        assert_eq!(state.module_registry().len(), 0);
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
}
