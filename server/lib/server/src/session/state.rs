//! Session state containing application state and registries.
//!
//! `SessionState` bundles the runtime application state with the registries
//! needed for key processing. Each session has its own isolated state.
//!
//! # Architecture (#753 E6)
//!
//! The server is a domain-neutral topological router. It does NOT import any
//! driver crate. Per-session shared state that formerly lived inside
//! `DriverSession` (compositor, home_mode) is now directly owned here.
//! Per-client state (mode_stack, cursor, extensions) lives in `EditingState`.
//! Buffer access, command execution, and key dispatch all route through
//! `DomainDriver` — the server never touches driver types directly.

use std::{collections::HashMap, sync::Arc};

use {
    reovim_kernel::api::v1::{KernelContext, ModeId},
    reovim_subsys_input_contracts::KeySequence,
    reovim_subsys_layout::RootCompositor,
    reovim_subsys_vfs::VfsDriver,
};

use crate::{
    app::AppState,
    registry::{CommandRegistry, KeyLookupResult, KeymapRegistry, ModeRegistry},
};

/// Session state combining application state with registries.
///
/// This is the complete state for a single editing session. Each session
/// (like tmux sessions) has its own `SessionState` with independent:
/// - Compositor (shared layout, cloned per-client on join)
/// - Home mode (initial mode for new clients)
/// - Kernel context (buffers, events, options)
/// - Mode/command/keymap registries
///
/// # Domain-Neutral (#753 E6)
///
/// Zero driver imports. Buffer access, command execution, and key dispatch
/// all route through `DomainDriver` (via `Session`).
///
/// # Thread Safety
///
/// `SessionState` is NOT `Sync` by itself. The `Session` wrapper provides
/// thread-safe access via `RwLock<SessionState>`.
pub struct SessionState {
    /// Shared compositor for layout (dissolved from DriverSession #753 E6).
    ///
    /// Cloned per-client on join for independent layout notifications.
    pub compositor: Option<Box<dyn RootCompositor>>,

    /// Home mode for initializing new clients (#491).
    ///
    /// When a new client connects, their mode stack starts with this mode.
    pub home_mode: ModeId,

    /// Application state (kernel + server-specific state).
    ///
    /// Contains: kernel context, running flag, extensions.
    pub app: AppState,

    /// Virtual filesystem driver for file operations.
    pub vfs: Arc<dyn VfsDriver>,

    /// Registry of mode metadata and behavior.
    pub mode_registry: ModeRegistry,

    /// Registry of command handlers (metadata-only in server, execution via DomainDriver).
    pub command_registry: CommandRegistry,

    /// Registry of keybindings.
    pub keymap_registry: KeymapRegistry,

    /// Session-scoped shared registers (A-Z) (#515 Phase 5).
    ///
    /// All clients in the session read/write from this shared storage.
    /// Content is stored as opaque bytes — the domain driver interprets the format.
    pub session_registers: HashMap<char, Vec<u8>>,
}

impl SessionState {
    /// Create a new session state.
    #[must_use]
    pub fn new(kernel: KernelContext, initial_mode: ModeId, vfs: Arc<dyn VfsDriver>) -> Self {
        Self {
            compositor: None,
            home_mode: initial_mode,
            app: AppState::new(kernel),
            vfs,
            mode_registry: ModeRegistry::new(),
            command_registry: CommandRegistry::new(),
            keymap_registry: KeymapRegistry::new(),
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
        compositor: Option<Box<dyn RootCompositor>>,
    ) -> Self {
        let mut state = Self {
            compositor,
            home_mode: initial_mode,
            app: AppState::new(kernel),
            vfs,
            mode_registry,
            command_registry,
            keymap_registry,
            session_registers: HashMap::new(),
        };

        // Create initial window in compositor if kernel has any buffers.
        state.ensure_initial_compositor_window();

        state
    }

    /// Ensure the compositor has at least one tiled window.
    ///
    /// Called after scratch buffer creation to handle the case where
    /// `with_registries()` ran before any buffers existed.
    pub fn ensure_initial_compositor_window(&mut self) {
        if self.app.kernel.buffers.list().is_empty() {
            return;
        }
        let Some(compositor) = self.compositor.as_mut() else {
            return;
        };
        if let Some(active) = compositor.active_layer()
            && let Some(layer) = compositor.layer_compositor_mut(active)
            && layer
                .windows_in_zone(reovim_subsys_layout::Zone::Tiled)
                .is_empty()
        {
            layer.add_tiled();
        }
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get the home mode for initializing new clients (#491).
    #[must_use]
    pub const fn home_mode(&self) -> &ModeId {
        &self.home_mode
    }

    /// Look up a key sequence in the current mode's keymap.
    #[must_use]
    pub fn lookup_keys(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupResult {
        self.keymap_registry.lookup(mode, keys)
    }

    /// Check if the home mode accepts character input.
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
}

impl Default for SessionState {
    fn default() -> Self {
        let mode = ModeId::new(reovim_kernel::api::v1::ModuleId::new("default"), "normal");
        let vfs: Arc<dyn VfsDriver> = Arc::new(reovim_subsys_vfs::MockVfs::new());
        Self::new(KernelContext::default(), mode, vfs)
    }
}

impl SessionState {
    /// Create a session state with a custom kernel context.
    #[must_use]
    pub fn with_kernel(kernel: KernelContext) -> Self {
        let mode = ModeId::new(reovim_kernel::api::v1::ModuleId::new("default"), "normal");
        let vfs: Arc<dyn VfsDriver> = Arc::new(reovim_subsys_vfs::MockVfs::new());
        Self::new(kernel, mode, vfs)
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
