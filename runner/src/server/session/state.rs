//! Session state containing application state and registries.
//!
//! `SessionState` bundles the runtime application state with the registries
//! needed for key processing. Each session has its own isolated state.

use std::sync::Arc;

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{CommandId, KernelContext, ModeId},
};

use crate::{
    AppState,
    module::ModuleRegistry,
    registry::{CommandRegistry, KeyLookupResult, KeymapRegistry, ModeRegistry},
};

/// Session state combining application state with registries.
///
/// This is the complete state for a single editing session. Each session
/// (like tmux sessions) has its own `SessionState` with independent:
/// - Kernel context (buffers, events, options)
/// - Mode/command/keymap/module registries
/// - Runtime state (active buffer, pending keys)
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
    /// Application state (kernel + runtime state).
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
    pub module_registry: ModuleRegistry,
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
        Self {
            app: AppState::new(kernel, initial_mode),
            vfs,
            mode_registry: ModeRegistry::new(),
            command_registry: CommandRegistry::new(),
            keymap_registry: KeymapRegistry::new(),
            module_registry: ModuleRegistry::new(),
        }
    }

    /// Create session state with existing registries.
    ///
    /// Used when modules need to populate registries before creating
    /// the session state.
    #[must_use]
    pub fn with_registries(
        kernel: KernelContext,
        initial_mode: ModeId,
        vfs: Arc<dyn VfsDriver>,
        mode_registry: ModeRegistry,
        command_registry: CommandRegistry,
        keymap_registry: KeymapRegistry,
        module_registry: ModuleRegistry,
    ) -> Self {
        Self {
            app: AppState::new(kernel, initial_mode),
            vfs,
            mode_registry,
            command_registry,
            keymap_registry,
            module_registry,
        }
    }

    /// Get a reference to the module registry.
    #[must_use]
    pub const fn module_registry(&self) -> &ModuleRegistry {
        &self.module_registry
    }

    /// Get the current mode ID.
    #[must_use]
    pub fn current_mode(&self) -> &ModeId {
        self.app.current_mode()
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
    #[must_use]
    pub fn execute_command(
        &mut self,
        id: &CommandId,
        args: &CommandContext,
    ) -> Option<CommandResult> {
        self.command_registry.execute(id, &mut self.app, args)
    }

    /// Check if the current mode accepts character input.
    #[must_use]
    pub fn mode_accepts_char_input(&self) -> bool {
        self.mode_registry
            .accepts_char_input(self.app.current_mode())
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
