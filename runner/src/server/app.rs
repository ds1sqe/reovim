//! Application state for the runner.
//!
//! Separates runtime concerns from kernel context. The kernel provides
//! core services (buffers, events, options); the runner tracks runtime
//! state like active buffer and mode stack.

use {
    crate::UndoRegistry,
    reovim_arch::sync::RwLock,
    reovim_driver_input::{FallbackContext, KeySequence},
    reovim_kernel::api::v1::{Buffer, BufferId, Edit, KernelContext, ModeId, ModeStack, Position},
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
/// the event loop and commands need to operate.
///
/// # Example
///
/// ```ignore
/// use runner::AppState;
/// use reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId};
///
/// let kernel = KernelContext::default();
/// let initial_mode = ModeId::new(ModuleId::new("editor"), "normal");
/// let app = AppState::new(kernel, initial_mode);
/// ```
#[derive(Debug)]
pub struct AppState {
    /// Kernel context providing access to all kernel services.
    pub kernel: KernelContext,

    /// Current mode stack for vim-style mode handling.
    ///
    /// Supports mode stacking (e.g., Normal → Operator-Pending → back to Normal).
    pub mode_stack: ModeStack,

    /// Currently active buffer ID.
    ///
    /// This is a runtime concern, not kernel - the kernel just stores buffers,
    /// the runner decides which one is "active".
    pub active_buffer: Option<BufferId>,

    /// Pending key sequence for multi-key bindings.
    ///
    /// For sequences like `gg` or `<C-w>h`, keys accumulate here until
    /// they either match a binding, are a prefix, or don't match.
    pub pending_keys: KeySequence,

    /// Whether the application is running.
    pub running: bool,

    /// Terminal width in columns.
    ///
    /// Tracked at runtime for headless server mode. TUI clients send
    /// `editor/resize` to update this when their terminal is resized.
    /// Default: 80 (standard VT100 width).
    pub terminal_width: u16,

    /// Terminal height in rows.
    ///
    /// Tracked at runtime for headless server mode. TUI clients send
    /// `editor/resize` to update this when their terminal is resized.
    /// Default: 24 (standard VT100 height).
    pub terminal_height: u16,

    /// Per-buffer undo registry.
    ///
    /// Maintains separate undo trees for each buffer, enabling per-buffer
    /// undo/redo operations. Each buffer has its own isolated undo history.
    pub undo_registry: UndoRegistry,
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Arguments
    ///
    /// * `kernel` - The kernel context providing core services
    /// * `initial_mode` - The mode to start in (typically Normal mode)
    #[must_use]
    pub fn new(kernel: KernelContext, initial_mode: ModeId) -> Self {
        Self {
            kernel,
            mode_stack: ModeStack::new(initial_mode),
            active_buffer: None,
            pending_keys: KeySequence::new(),
            running: true,
            terminal_width: 80,
            terminal_height: 24,
            undo_registry: UndoRegistry::new(),
        }
    }

    /// Get the current mode ID.
    #[must_use]
    pub fn current_mode(&self) -> &ModeId {
        self.mode_stack.current()
    }

    /// Request the application to quit.
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
    pub fn request_quit(&mut self) {
        self.running = false;
    }

    /// Check if the application should continue running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Clear the pending key sequence.
    pub fn clear_pending_keys(&mut self) {
        self.pending_keys.clear();
    }
}

impl FallbackContext for AppState {
    fn current_mode(&self) -> &ModeId {
        self.mode_stack.current()
    }

    fn active_buffer(&self) -> Option<BufferId> {
        self.active_buffer
    }

    fn get_buffer(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
        self.kernel.buffers.get(id)
    }

    fn record_edit(
        &mut self,
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        self.undo_registry
            .record(buffer_id, edits, cursor_before, cursor_after);
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    #[test]
    fn test_app_state_new() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        assert!(app.is_running());
        assert!(app.active_buffer.is_none());
        assert!(app.pending_keys.is_empty());
        assert_eq!(app.current_mode().name(), "normal");
    }

    #[test]
    fn test_app_state_quit() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        assert!(app.is_running());
        app.request_quit();
        assert!(!app.is_running());
    }

    #[test]
    fn test_app_state_clear_pending() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Add some keys
        app.pending_keys
            .push(reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('g')));
        assert!(!app.pending_keys.is_empty());

        app.clear_pending_keys();
        assert!(app.pending_keys.is_empty());
    }

    #[test]
    fn test_app_state_default_terminal_size() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        // Default terminal size is 80x24 (VT100 standard)
        assert_eq!(app.terminal_width, 80);
        assert_eq!(app.terminal_height, 24);
    }

    #[test]
    fn test_app_state_terminal_size_access() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Verify we can read and write terminal dimensions
        app.terminal_width = 120;
        app.terminal_height = 40;

        assert_eq!(app.terminal_width, 120);
        assert_eq!(app.terminal_height, 40);
    }

    #[test]
    fn test_app_state_has_undo_registry() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        // AppState should initialize with empty UndoRegistry
        assert_eq!(app.undo_registry.buffer_count(), 0);
    }

    #[test]
    fn test_app_state_undo_registry_accessible() {
        use reovim_kernel::api::v1::{Edit, Position};

        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Can call undo_registry.record()
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "hello");
        app.undo_registry
            .record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        assert!(app.undo_registry.has_history(buffer_id));
    }

    #[test]
    fn test_app_state_undo_registry_per_buffer() {
        use reovim_kernel::api::v1::{Edit, Position};

        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let buffer1 = BufferId::from_raw(1);
        let buffer2 = BufferId::from_raw(2);

        // Record edit to buffer1
        let edit = Edit::insert(Position::new(0, 0), "hello");
        app.undo_registry
            .record(buffer1, vec![edit], Position::new(0, 0), Position::new(0, 5));

        // buffer1 has history, buffer2 does not (isolated)
        assert!(app.undo_registry.has_history(buffer1));
        assert!(!app.undo_registry.has_history(buffer2));
    }
}
