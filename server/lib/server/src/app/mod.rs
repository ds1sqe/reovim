//! Application state for the server.
//!
//! Separates runtime concerns from kernel context. The kernel provides
//! core services (buffers, events, options); the server tracks runtime
//! state like active buffer and mode stack.

use {
    reovim_arch::sync::RwLock,
    reovim_driver_input::{ExtensionMap, FallbackContext},
    reovim_driver_undo::{UndoKey, UndoProvider, UndoProviderRegistry},
    reovim_kernel::api::v1::{
        BufferId, BufferOps, Edit, KernelContext, ModeId, Position, ServiceRegistry,
    },
    std::sync::Arc,
};

/// Application state combining kernel context with runtime state.
///
/// # Design Philosophy
///
/// `KernelContext` provides kernel services (buffers, event bus, etc.)
/// but intentionally does NOT track runtime state like "which buffer is active"
/// because that's a runtime/window manager concern.
///
/// `AppState` wraps `KernelContext` and adds runtime-specific state that
/// the server and commands need to operate.
///
#[derive(Debug)]
pub struct AppState {
    /// Kernel context providing access to all kernel services.
    pub kernel: KernelContext,

    /// Service registry for cross-module service discovery.
    pub services: Arc<ServiceRegistry>,

    /// Whether the application is running.
    pub running: bool,

    /// Per-session module extensions (Epic #385).
    pub extensions: ExtensionMap,
}

impl AppState {
    /// Create a new application state.
    #[must_use]
    pub fn new(kernel: KernelContext) -> Self {
        let services = Arc::clone(&kernel.services);
        Self {
            kernel,
            services,
            running: true,
            extensions: ExtensionMap::new(),
        }
    }

    /// Get the undo provider from the service registry.
    #[must_use]
    pub fn get_undo_provider(&self) -> Option<Arc<dyn UndoProvider>> {
        self.services
            .get::<UndoProviderRegistry>()
            .and_then(|registry| registry.get(&UndoKey::Buffer))
    }

    /// Request the application to quit.
    #[allow(clippy::missing_const_for_fn)]
    pub fn request_quit(&mut self) {
        self.running = false;
    }

    /// Request clients to detach (server continues running).
    #[allow(clippy::missing_const_for_fn)]
    pub fn request_detach(&mut self) {
        tracing::info!("Detach requested - clients will be notified");
    }

    /// Check if the application should continue running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }
}

/// `FallbackContext` implementation for `AppState`.
impl FallbackContext for AppState {
    fn current_mode(&self) -> &ModeId {
        panic!(
            "AppState::current_mode() should not be called - use SessionState::current_mode() instead"
        );
    }

    fn active_buffer(&self) -> Option<BufferId> {
        panic!(
            "AppState::active_buffer() should not be called - use SessionState::session_active_buffer() instead"
        );
    }

    fn cursor_position(&self) -> Option<Position> {
        panic!(
            "AppState::cursor_position() should not be called - use client-specific window state instead"
        );
    }

    fn set_cursor_position(&mut self, _pos: Position) {
        panic!(
            "AppState::set_cursor_position() should not be called - use client-specific window state instead"
        );
    }

    fn get_buffer(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>> {
        self.kernel.buffers.get(id)
    }

    fn record_edit(
        &mut self,
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        if let Some(undo_provider) = self.get_undo_provider() {
            undo_provider.record(buffer_id, edits, cursor_before, cursor_after);
        }
    }

    fn accumulate_edit(
        &mut self,
        buffer_id: BufferId,
        edit: Edit,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        if let Some(undo_provider) = self.get_undo_provider() {
            undo_provider.record(buffer_id, vec![edit], cursor_before, cursor_after);
        }
    }

    fn flush_pending_edits(&mut self) {
        // No-op - edits recorded immediately
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
