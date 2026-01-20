//! Application state for the runner.
//!
//! Separates runtime concerns from kernel context. The kernel provides
//! core services (buffers, events, options); the runner tracks runtime
//! state like active buffer and mode stack.

mod cmdline;

pub use cmdline::CommandLineState;

use {
    crate::{UndoRegistry, server::window::WindowRegistry},
    reovim_arch::sync::RwLock,
    reovim_driver_display::WindowId,
    reovim_driver_input::{ExtensionMap, FallbackContext, KeySequence},
    reovim_kernel::api::v1::{Buffer, BufferId, Edit, KernelContext, ModeId, Position},
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
/// # SSOT Note
///
/// `mode_stack`, `active_buffer`, `terminal_size`, and `pending_keys` are now
/// stored in `driver::Session` (SSOT). See `SessionState::driver_session`.
/// This struct only keeps kernel-related and window-related state.
///
/// # Example
///
/// ```ignore
/// use runner::AppState;
/// use reovim_kernel::api::v1::KernelContext;
///
/// let kernel = KernelContext::default();
/// let app = AppState::new(kernel);
/// ```
#[derive(Debug)]
pub struct AppState {
    /// Kernel context providing access to all kernel services.
    pub kernel: KernelContext,

    /// Pending key sequence for multi-key bindings.
    ///
    /// For sequences like `gg` or `<C-w>h`, keys accumulate here until
    /// they either match a binding, are a prefix, or don't match.
    ///
    /// NOTE: This is a legacy field. The SSOT is `driver::Session::pending_keys`.
    /// Kept for backward compatibility with code that hasn't migrated yet.
    pub pending_keys: KeySequence,

    /// Whether the application is running.
    pub running: bool,

    /// Terminal width in columns.
    ///
    /// NOTE: This is a legacy field. The SSOT is `driver::Session::terminal_size`.
    /// Kept for backward compatibility.
    pub terminal_width: u16,

    /// Terminal height in rows.
    ///
    /// NOTE: This is a legacy field. The SSOT is `driver::Session::terminal_size`.
    /// Kept for backward compatibility.
    pub terminal_height: u16,

    /// Per-buffer undo registry.
    ///
    /// Maintains separate undo trees for each buffer, enabling per-buffer
    /// undo/redo operations. Each buffer has its own isolated undo history.
    pub undo_registry: UndoRegistry,

    /// Window registry for multi-window support.
    ///
    /// Tracks window state, layout, and focus. Each window has its own
    /// cursor position, allowing multiple views of the same buffer.
    pub windows: WindowRegistry,

    /// Command-line mode state for : commands.
    ///
    /// Tracks input buffer and whether command-line mode is active.
    /// Used for Ex-style commands like `:w`, `:q`, `:set`, etc.
    pub cmdline: CommandLineState,

    /// Per-session module extensions (Epic #385).
    ///
    /// Modules store per-session policy state via `SessionExtension` trait.
    /// For example, `VimSessionState` stores pending operator, find-char state,
    /// and other vim-specific policy that was previously in `AppState`.
    ///
    /// # Architecture
    ///
    /// - **Mechanism (runner)**: Provides storage via `ExtensionMap`
    /// - **Policy (modules)**: Store/access their own state types
    ///
    /// This enables the runner to be policy-agnostic while still allowing
    /// modules to maintain per-session state.
    pub extensions: ExtensionMap,
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Arguments
    ///
    /// * `kernel` - The kernel context providing core services
    ///
    /// # Note
    ///
    /// `mode_stack` and `active_buffer` are now stored in `driver::Session` (SSOT).
    /// Use `SessionState::driver_session` for these values.
    #[must_use]
    pub fn new(kernel: KernelContext) -> Self {
        Self {
            kernel,
            pending_keys: KeySequence::new(),
            running: true,
            terminal_width: 80,
            terminal_height: 24,
            undo_registry: UndoRegistry::new(),
            windows: WindowRegistry::new(),
            cmdline: CommandLineState::new(),
            extensions: ExtensionMap::new(),
        }
    }

    /// Request the application to quit.
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
    pub fn request_quit(&mut self) {
        self.running = false;
    }

    /// Request clients to detach (server continues running).
    ///
    /// Unlike `request_quit`, the server remains active and accepts new connections.
    /// Connected TUI clients receive a DETACH notification and disconnect gracefully.
    ///
    /// Note: This sets a flag; the actual notification is sent by the broadcaster.
    #[allow(clippy::missing_const_for_fn)]
    pub fn request_detach(&mut self) {
        // TODO: Set a detach_requested flag and have broadcaster send notifications.
        // For now, this is a placeholder that logs the intent.
        tracing::info!("Detach requested - clients will be notified");
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

    // ========================================================================
    // Window Management Methods
    // ========================================================================

    /// Get the currently active window ID.
    #[must_use]
    pub const fn active_window(&self) -> Option<WindowId> {
        self.windows.active_window()
    }

    /// Find the first window displaying a given buffer.
    #[must_use]
    pub fn window_for_buffer(&self, buffer_id: BufferId) -> Option<WindowId> {
        self.windows.windows().find(|&win_id| {
            self.windows
                .get(win_id)
                .is_some_and(|state| state.buffer_id == Some(buffer_id))
        })
    }

    /// Get the buffer displayed in a given window.
    #[must_use]
    pub fn buffer_for_window(&self, window_id: WindowId) -> Option<BufferId> {
        self.windows
            .get(window_id)
            .and_then(|state| state.buffer_id)
    }

    /// Ensure a window exists when setting the active buffer.
    ///
    /// For backward compatibility with single-window usage, this creates
    /// a window if none exist when a buffer is set as active.
    ///
    /// NOTE: This only updates the window registry. The SSOT for `active_buffer`
    /// is now `driver::Session::active_buffer`. Use `SessionState::set_session_active_buffer()`
    /// to set the actual active buffer.
    pub fn set_active_buffer_with_window(&mut self, buffer_id: BufferId) {
        // If no windows exist, create one
        if self.windows.is_empty() {
            let window_id = self.windows.create_window(Some(buffer_id));
            self.windows.set_active_window(window_id);
        } else if let Some(active) = self.windows.active_window() {
            // Update the active window's buffer
            if let Some(state) = self.windows.get_mut(active) {
                state.buffer_id = Some(buffer_id);
            }
        }
    }
}

/// `FallbackContext` implementation for `AppState`.
///
/// # SSOT Migration Note
///
/// `current_mode()` and `active_buffer()` are implemented to satisfy the trait,
/// but the SSOT for these values is now `driver::Session`. These methods
/// panic because they should not be called - use `SessionState` methods instead.
impl FallbackContext for AppState {
    fn current_mode(&self) -> &ModeId {
        // This should not be called - SSOT is now driver::Session
        // The VimFallbackHandler is not currently connected to the event loop
        panic!(
            "AppState::current_mode() should not be called - use SessionState::current_mode() instead"
        );
    }

    fn active_buffer(&self) -> Option<BufferId> {
        // This should not be called - SSOT is now driver::Session
        // The VimFallbackHandler is not currently connected to the event loop
        panic!(
            "AppState::active_buffer() should not be called - use SessionState::session_active_buffer() instead"
        );
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

    fn accumulate_edit(
        &mut self,
        buffer_id: BufferId,
        edit: Edit,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        // For now, just record directly
        // TODO: Implement batching via extensions when SessionContext is available
        self.undo_registry
            .record(buffer_id, vec![edit], cursor_before, cursor_after);
    }

    fn flush_pending_edits(&mut self) {
        // No-op - batching removed, edits recorded immediately
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_new() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel);

        assert!(app.is_running());
        // Note: active_buffer and mode_stack are now in driver::Session (SSOT)
        assert!(app.pending_keys.is_empty());
    }

    #[test]
    fn test_app_state_quit() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel);

        assert!(app.is_running());
        app.request_quit();
        assert!(!app.is_running());
    }

    #[test]
    fn test_app_state_clear_pending() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel);

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
        let app = AppState::new(kernel);

        // Default terminal size is 80x24 (VT100 standard)
        assert_eq!(app.terminal_width, 80);
        assert_eq!(app.terminal_height, 24);
    }

    #[test]
    fn test_app_state_terminal_size_access() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel);

        // Verify we can read and write terminal dimensions
        app.terminal_width = 120;
        app.terminal_height = 40;

        assert_eq!(app.terminal_width, 120);
        assert_eq!(app.terminal_height, 40);
    }

    #[test]
    fn test_app_state_has_undo_registry() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel);

        // AppState should initialize with empty UndoRegistry
        assert_eq!(app.undo_registry.buffer_count(), 0);
    }

    #[test]
    fn test_app_state_undo_registry_accessible() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel);

        // Can call undo_registry.record()
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "hello");
        app.undo_registry
            .record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        assert!(app.undo_registry.has_history(buffer_id));
    }

    #[test]
    fn test_app_state_undo_registry_per_buffer() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel);

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

    // ========================================================================
    // Window Management Tests
    // ========================================================================

    #[test]
    fn test_app_state_has_window_registry() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel);

        // Initially no windows
        assert!(app.windows.is_empty());
        assert!(app.active_window().is_none());
    }

    #[test]
    fn test_app_state_active_window_initially_none() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel);

        assert!(app.active_window().is_none());
        assert_eq!(app.windows.window_count(), 0);
    }

    #[test]
    fn test_app_state_create_window_on_buffer_set() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel);

        let buffer_id = BufferId::new();

        // Set active buffer with window creation
        app.set_active_buffer_with_window(buffer_id);

        // Should have created a window
        assert_eq!(app.windows.window_count(), 1);
        assert!(app.active_window().is_some());

        // The window should contain the buffer
        let win_id = app.active_window().unwrap();
        assert_eq!(app.buffer_for_window(win_id), Some(buffer_id));
    }

    #[test]
    fn test_app_state_backward_compat_single_buffer() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel);

        // Old single-buffer workflow:
        // 1. Create buffer
        // 2. Set as active with window
        // 3. Edit buffer

        let buffer_id = BufferId::new();
        app.set_active_buffer_with_window(buffer_id);

        // Verify window was created
        assert!(app.active_window().is_some());

        // Window should have the buffer
        let win = app.active_window().unwrap();
        assert_eq!(app.buffer_for_window(win), Some(buffer_id));

        // Set another buffer - should reuse existing window
        let buffer_id2 = BufferId::new();
        app.set_active_buffer_with_window(buffer_id2);

        // Still one window, but with new buffer
        assert_eq!(app.windows.window_count(), 1);
        assert_eq!(app.buffer_for_window(win), Some(buffer_id2));
    }

    #[test]
    fn test_app_state_window_for_buffer() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel);

        let buffer_id = BufferId::new();
        app.set_active_buffer_with_window(buffer_id);

        // Should find the window for this buffer
        let found = app.window_for_buffer(buffer_id);
        assert!(found.is_some());
        assert_eq!(found, app.active_window());

        // Should not find window for non-existent buffer
        let other_buffer = BufferId::new();
        assert!(app.window_for_buffer(other_buffer).is_none());
    }
}
