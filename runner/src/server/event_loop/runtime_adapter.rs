//! `RuntimeAdapter` - combines `SessionRuntime` with runner's `WindowRegistry`.
//!
//! This adapter allows the event loop to use `SessionRuntime` for most session
//! API operations while delegating window and cursor operations to the runner's
//! `WindowRegistry`.
//!
//! # Architecture
//!
//! ```text
//! RuntimeAdapter
//! ├── SessionRuntime → ModeApi, BufferApi (except cursor), RegisterApi,
//! │                    ExtensionApi, CommandApi
//! └── WindowRegistry → WindowApi, cursor operations in BufferApi
//! ```
//!
//! # SSOT
//!
//! - `SessionRuntime.session` is SSOT for mode stack, active buffer, extensions
//! - `WindowRegistry` is SSOT for window layout and per-window cursor positions
//!
//! # Changes
//!
//! Changes are tracked in two places:
//! 1. `SessionRuntime` tracks mode/buffer changes
//! 2. `RuntimeAdapter` tracks window/cursor changes
//!
//! On `take_changes()`, both are merged.

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_session::{
        PopResult, SessionExtension, SessionRuntime, TransitionContext,
        api::{
            BufferApi, BufferError, ChangeTracker, CommandApi, ExtensionApi, ModeApi, ModeError,
            RegisterApi, RegisterContent, Selection, SelectionMode, StateChanges, UndoApi,
            WindowApi, WindowError,
        },
    },
    reovim_kernel::api::v1::{
        BufferId, CommandId, Edit, KernelContext, ModeId, Position, UndoResult, WindowId,
    },
};

use crate::server::window::WindowRegistry;

/// Runtime adapter combining `SessionRuntime` with `WindowRegistry`.
///
/// Delegates most operations to `SessionRuntime`, but uses `WindowRegistry`
/// for window layout and cursor operations.
pub struct RuntimeAdapter<'a> {
    /// Session runtime for mode/buffer/register/extension/command operations.
    session_runtime: SessionRuntime<'a>,
    /// Window registry for window layout and cursor (SSOT).
    windows: &'a mut WindowRegistry,
    /// Kernel context for buffer validation.
    kernel: &'a KernelContext,
    /// Changes tracked for window/cursor operations.
    window_changes: StateChanges,
}

impl<'a> RuntimeAdapter<'a> {
    /// Create a new runtime adapter.
    ///
    /// # Arguments
    ///
    /// * `session_runtime` - The session runtime for most API operations
    /// * `windows` - Window registry (SSOT for window layout and cursor)
    /// * `kernel` - Kernel context for buffer validation
    pub fn new(
        session_runtime: SessionRuntime<'a>,
        windows: &'a mut WindowRegistry,
        kernel: &'a KernelContext,
    ) -> Self {
        Self {
            session_runtime,
            windows,
            kernel,
            window_changes: StateChanges::new(),
        }
    }

    /// Find the first window displaying a given buffer.
    fn find_window_for_buffer(&self, buffer_id: BufferId) -> Option<WindowId> {
        // WindowId is now unified - no type conversion needed
        self.windows.windows().find(|&win_id| {
            self.windows
                .get(win_id)
                .is_some_and(|state| state.buffer_id == Some(buffer_id))
        })
    }
}

// === ModeApi ===
// Delegate entirely to SessionRuntime

impl ModeApi for RuntimeAdapter<'_> {
    fn current_mode(&self) -> &ModeId {
        self.session_runtime.current_mode()
    }

    fn home_mode(&self) -> &ModeId {
        self.session_runtime.home_mode()
    }

    fn mode_depth(&self) -> usize {
        self.session_runtime.mode_depth()
    }

    fn is_mode_active(&self, mode: &ModeId) -> bool {
        self.session_runtime.is_mode_active(mode)
    }

    fn mode_stack(&self) -> Vec<ModeId> {
        self.session_runtime.mode_stack()
    }

    fn push_mode(&mut self, mode: ModeId, ctx: TransitionContext) {
        self.session_runtime.push_mode(mode, ctx);
    }

    fn pop_mode(&mut self, result: Option<PopResult>) -> Result<(), ModeError> {
        self.session_runtime.pop_mode(result)
    }

    fn set_mode(&mut self, mode: ModeId, ctx: TransitionContext) {
        self.session_runtime.set_mode(mode, ctx);
    }
}

// === BufferApi ===
// Delegate most to SessionRuntime, but cursor operations use WindowRegistry

impl BufferApi for RuntimeAdapter<'_> {
    fn active_buffer(&self) -> Option<BufferId> {
        self.session_runtime.active_buffer()
    }

    fn buffer_line(&self, buffer: BufferId, line: usize) -> Option<String> {
        self.session_runtime.buffer_line(buffer, line)
    }

    fn buffer_line_count(&self, buffer: BufferId) -> Option<usize> {
        self.session_runtime.buffer_line_count(buffer)
    }

    // OVERRIDE: Use WindowRegistry for cursor position
    fn cursor_position(&self, buffer: BufferId) -> Option<Position> {
        let win_id = self.find_window_for_buffer(buffer)?;
        self.windows
            .get(win_id)
            .map(|state| Position::new(state.cursor.line, state.cursor.column))
    }

    fn buffer_position(&self, buffer: BufferId) -> Option<Position> {
        self.session_runtime.buffer_position(buffer)
    }

    fn set_buffer_position(&mut self, buffer: BufferId, pos: Position) {
        self.session_runtime.set_buffer_position(buffer, pos);
    }

    fn buffer_line_len(&self, buffer: BufferId, line: usize) -> Option<usize> {
        self.session_runtime.buffer_line_len(buffer, line)
    }

    fn selection(&self, buffer: BufferId) -> Option<Selection> {
        self.session_runtime.selection(buffer)
    }

    fn buffer_text_range(
        &self,
        buffer: BufferId,
        start: Position,
        end: Position,
    ) -> Option<String> {
        self.session_runtime.buffer_text_range(buffer, start, end)
    }

    fn buffer_content(&self, buffer: BufferId) -> Option<String> {
        self.session_runtime.buffer_content(buffer)
    }

    fn buffer_file_path(&self, buffer: BufferId) -> Option<String> {
        self.session_runtime.buffer_file_path(buffer)
    }

    fn is_buffer_modified(&self, buffer: BufferId) -> Option<bool> {
        self.session_runtime.is_buffer_modified(buffer)
    }

    fn set_buffer_modified(&mut self, buffer: BufferId, modified: bool) {
        self.session_runtime.set_buffer_modified(buffer, modified);
    }

    fn insert_text(&mut self, buffer: BufferId, pos: Position, text: &str) {
        self.session_runtime.insert_text(buffer, pos, text);
    }

    fn delete_range(&mut self, buffer: BufferId, start: Position, end: Position) {
        self.session_runtime.delete_range(buffer, start, end);
    }

    // OVERRIDE: Use WindowRegistry for cursor movement
    fn move_cursor(&mut self, buffer: BufferId, pos: Position) {
        if let Some(win_id) = self.find_window_for_buffer(buffer)
            && let Some(state) = self.windows.get_mut(win_id)
        {
            state.cursor.line = pos.line;
            state.cursor.column = pos.column;
            self.window_changes.record_cursor_move(buffer);
        }
    }

    fn set_selection(&mut self, buffer: BufferId, sel: Option<Selection>) {
        self.session_runtime.set_selection(buffer, sel);
    }

    fn swap_selection_ends(&mut self, buffer: BufferId) {
        self.session_runtime.swap_selection_ends(buffer);
    }

    fn set_selection_mode(&mut self, buffer: BufferId, mode: SelectionMode) {
        self.session_runtime.set_selection_mode(buffer, mode);
    }

    fn create_buffer(&mut self, name: Option<&str>, content: &str) -> BufferId {
        self.session_runtime.create_buffer(name, content)
    }

    fn delete_buffer(&mut self, buffer: BufferId) -> Result<(), BufferError> {
        self.session_runtime.delete_buffer(buffer)
    }

    fn rename_buffer(&mut self, buffer: BufferId, new_name: &str) {
        self.session_runtime.rename_buffer(buffer, new_name);
    }
}

// === WindowApi ===
// Use WindowBridge (backed by WindowRegistry)

impl WindowApi for RuntimeAdapter<'_> {
    fn active_window(&self) -> Option<WindowId> {
        // Display and kernel WindowId are now the same type (unified)
        self.windows.active_window()
    }

    fn window_count(&self) -> usize {
        self.windows.window_count()
    }

    fn window_buffer(&self, window: WindowId) -> Option<BufferId> {
        // No conversion needed - types are unified
        self.windows.get(window).and_then(|w| w.buffer_id)
    }

    fn create_window(&mut self, buffer: Option<BufferId>) -> WindowId {
        // No conversion needed - types are unified
        let window_id = self.windows.create_window(buffer);
        self.window_changes.record_window_created(window_id);
        window_id
    }

    fn close_window(&mut self, window: WindowId) -> Result<(), WindowError> {
        if self.windows.window_count() <= 1 {
            return Err(WindowError::CannotCloseLastWindow);
        }
        // No conversion needed - types are unified
        if self.windows.close_window(window) {
            self.window_changes.record_window_closed(window);
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }

    fn focus_window(&mut self, window: WindowId) -> Result<(), WindowError> {
        // No conversion needed - types are unified
        if self.windows.get(window).is_some() {
            self.windows.set_active_window(window);
            self.window_changes.record_focus_change();
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }

    fn set_window_buffer(&mut self, window: WindowId, buffer: BufferId) -> Result<(), WindowError> {
        if self.kernel.buffers.get(buffer).is_none() {
            return Err(WindowError::BufferNotFound(buffer));
        }
        // No conversion needed - types are unified
        if let Some(state) = self.windows.get_mut(window) {
            state.buffer_id = Some(buffer);
            self.window_changes.window_changed = true;
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }
}

// === CommandApi ===
// Delegate to SessionRuntime

impl CommandApi for RuntimeAdapter<'_> {
    fn execute_command(&mut self, cmd: CommandId, ctx: CommandContext) -> CommandResult {
        self.session_runtime.execute_command(cmd, ctx)
    }
}

// === ExtensionApi ===
// Delegate to SessionRuntime

impl ExtensionApi for RuntimeAdapter<'_> {
    fn ext<T: SessionExtension>(&self) -> Option<&T> {
        self.session_runtime.ext::<T>()
    }

    fn ext_mut<T: SessionExtension>(&mut self) -> &mut T {
        self.session_runtime.ext_mut::<T>()
    }
}

// === RegisterApi ===
// Delegate to SessionRuntime

impl RegisterApi for RuntimeAdapter<'_> {
    fn get_register(&self, name: Option<char>) -> Option<RegisterContent> {
        self.session_runtime.get_register(name)
    }

    fn set_register(&mut self, name: Option<char>, content: RegisterContent) {
        self.session_runtime.set_register(name, content);
    }
}

// === UndoApi ===
// Delegate to SessionRuntime

impl UndoApi for RuntimeAdapter<'_> {
    fn undo(&mut self, buffer: BufferId) -> Option<UndoResult> {
        self.session_runtime.undo(buffer)
    }

    fn redo(&mut self, buffer: BufferId) -> Option<UndoResult> {
        self.session_runtime.redo(buffer)
    }

    fn record_edit(
        &mut self,
        buffer: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        self.session_runtime
            .record_edit(buffer, edits, cursor_before, cursor_after);
    }

    fn can_undo(&self, buffer: BufferId) -> bool {
        self.session_runtime.can_undo(buffer)
    }

    fn can_redo(&self, buffer: BufferId) -> bool {
        self.session_runtime.can_redo(buffer)
    }
}

// === ChangeTracker ===
// Merge changes from SessionRuntime and window_changes

impl ChangeTracker for RuntimeAdapter<'_> {
    fn take_changes(&mut self) -> StateChanges {
        // Take changes from SessionRuntime
        let mut changes = self.session_runtime.take_changes();
        // Merge window/cursor changes
        changes.merge(std::mem::take(&mut self.window_changes));
        changes
    }

    fn record_cursor_move(&mut self, buffer: BufferId) {
        self.window_changes.record_cursor_move(buffer);
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_session::{ClientId, Session as DriverSession, api::CommandExecutor},
    };

    fn test_mode() -> ModeId {
        ModeId::new(reovim_kernel::api::v1::ModuleId::new("test"), "normal")
    }

    fn test_mode_2() -> ModeId {
        ModeId::with_discriminant(reovim_kernel::api::v1::ModuleId::new("test"), "insert", 1)
    }

    struct StubExecutor;
    impl CommandExecutor for StubExecutor {
        fn execute(
            &self,
            _cmd: &CommandId,
            _ctx: &CommandContext,
            _kernel: &mut KernelContext,
        ) -> Option<CommandResult> {
            None
        }
    }

    #[test]
    fn test_runtime_adapter_mode_api() {
        let kernel = KernelContext::default();
        let mut session = DriverSession::new(ClientId::new(0), test_mode());
        let mut windows = WindowRegistry::new();
        let executor = StubExecutor;

        let session_runtime = SessionRuntime::new(&mut session, &kernel, &executor);
        let mut adapter = RuntimeAdapter::new(session_runtime, &mut windows, &kernel);

        // Check initial state
        assert_eq!(adapter.current_mode(), &test_mode());
        assert_eq!(adapter.mode_depth(), 1);

        // Push mode
        adapter.push_mode(test_mode_2(), TransitionContext::new());
        assert_eq!(adapter.current_mode(), &test_mode_2());
        assert_eq!(adapter.mode_depth(), 2);

        // Pop mode
        let result = adapter.pop_mode(None);
        assert!(result.is_ok());
        assert_eq!(adapter.current_mode(), &test_mode());
    }

    #[test]
    fn test_runtime_adapter_cursor_uses_window_registry() {
        let kernel = KernelContext::default();
        let buffer_id = kernel
            .buffers
            .register(reovim_kernel::api::v1::Buffer::from_string("hello"));

        let mut session = DriverSession::new(ClientId::new(0), test_mode());
        session.set_active_buffer(Some(buffer_id));

        let mut windows = WindowRegistry::new();
        let win_id = windows.create_window(Some(buffer_id));

        // Set cursor position in WindowRegistry
        if let Some(state) = windows.get_mut(win_id) {
            state.cursor.line = 5;
            state.cursor.column = 10;
        }

        let executor = StubExecutor;
        let session_runtime = SessionRuntime::new(&mut session, &kernel, &executor);
        let adapter = RuntimeAdapter::new(session_runtime, &mut windows, &kernel);

        // cursor_position should read from WindowRegistry
        let pos = adapter.cursor_position(buffer_id);
        assert_eq!(pos, Some(Position::new(5, 10)));
    }

    #[test]
    fn test_runtime_adapter_move_cursor_updates_window_registry() {
        let kernel = KernelContext::default();
        let buffer_id = kernel
            .buffers
            .register(reovim_kernel::api::v1::Buffer::from_string("hello"));

        let mut session = DriverSession::new(ClientId::new(0), test_mode());
        session.set_active_buffer(Some(buffer_id));

        let mut windows = WindowRegistry::new();
        windows.create_window(Some(buffer_id));

        let executor = StubExecutor;
        let session_runtime = SessionRuntime::new(&mut session, &kernel, &executor);
        let mut adapter = RuntimeAdapter::new(session_runtime, &mut windows, &kernel);

        // Move cursor
        adapter.move_cursor(buffer_id, Position::new(3, 7));

        // Verify cursor was updated
        let pos = adapter.cursor_position(buffer_id);
        assert_eq!(pos, Some(Position::new(3, 7)));
    }

    #[test]
    fn test_runtime_adapter_change_tracking() {
        let kernel = KernelContext::default();
        let mut session = DriverSession::new(ClientId::new(0), test_mode());
        let mut windows = WindowRegistry::new();
        let executor = StubExecutor;

        let session_runtime = SessionRuntime::new(&mut session, &kernel, &executor);
        let mut adapter = RuntimeAdapter::new(session_runtime, &mut windows, &kernel);

        // Create a window (tracked in window_changes)
        let window_id = adapter.create_window(None);

        // Push mode (tracked in session_runtime)
        adapter.push_mode(test_mode_2(), TransitionContext::new());

        // Take changes - should merge both
        let changes = adapter.take_changes();
        assert!(changes.mode_changed);
        assert!(changes.window_changed);
        assert!(changes.windows_created.contains(&window_id));
    }

    #[test]
    fn test_cursor_position_buffer_not_displayed() {
        // Test that cursor_position returns None when buffer has no displaying window
        let kernel = KernelContext::default();
        let buffer_id = kernel
            .buffers
            .register(reovim_kernel::api::v1::Buffer::from_string("orphan buffer"));

        let mut session = DriverSession::new(ClientId::new(0), test_mode());
        session.set_active_buffer(Some(buffer_id));

        let mut windows = WindowRegistry::new();
        // Create a window but DON'T associate it with the buffer
        windows.create_window(None);

        let executor = StubExecutor;
        let session_runtime = SessionRuntime::new(&mut session, &kernel, &executor);
        let adapter = RuntimeAdapter::new(session_runtime, &mut windows, &kernel);

        // cursor_position should return None since no window displays this buffer
        let pos = adapter.cursor_position(buffer_id);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_move_cursor_silently_noops_when_window_missing() {
        // Test that move_cursor silently no-ops when buffer has no displaying window
        // (defensive behavior - no panic, no corruption)
        let kernel = KernelContext::default();
        let buffer_id = kernel
            .buffers
            .register(reovim_kernel::api::v1::Buffer::from_string("orphan buffer"));

        let mut session = DriverSession::new(ClientId::new(0), test_mode());
        session.set_active_buffer(Some(buffer_id));

        let mut windows = WindowRegistry::new();
        // Create a window with a DIFFERENT buffer
        let other_buffer = kernel
            .buffers
            .register(reovim_kernel::api::v1::Buffer::from_string("other buffer"));
        windows.create_window(Some(other_buffer));

        let executor = StubExecutor;
        let session_runtime = SessionRuntime::new(&mut session, &kernel, &executor);
        let mut adapter = RuntimeAdapter::new(session_runtime, &mut windows, &kernel);

        // This should silently no-op, not panic or corrupt state
        adapter.move_cursor(buffer_id, Position::new(100, 200));

        // Verify the orphan buffer still has no cursor position
        assert_eq!(adapter.cursor_position(buffer_id), None);

        // Verify the other buffer's cursor wasn't affected
        let other_pos = adapter.cursor_position(other_buffer);
        assert_eq!(other_pos, Some(Position::new(0, 0))); // Default cursor position
    }

    #[test]
    fn test_take_changes_is_idempotent() {
        // Test that second take_changes() returns empty state
        let kernel = KernelContext::default();
        let mut session = DriverSession::new(ClientId::new(0), test_mode());
        let mut windows = WindowRegistry::new();
        let executor = StubExecutor;

        let session_runtime = SessionRuntime::new(&mut session, &kernel, &executor);
        let mut adapter = RuntimeAdapter::new(session_runtime, &mut windows, &kernel);

        // Create some changes
        adapter.create_window(None);
        adapter.push_mode(test_mode_2(), TransitionContext::new());

        // First take_changes() should have changes
        let changes1 = adapter.take_changes();
        assert!(changes1.mode_changed);
        assert!(changes1.window_changed);

        // Second take_changes() should be empty (changes were consumed)
        let changes2 = adapter.take_changes();
        assert!(!changes2.mode_changed);
        assert!(!changes2.window_changed);
        assert!(changes2.windows_created.is_empty());
        assert!(changes2.windows_closed.is_empty());
    }
}
