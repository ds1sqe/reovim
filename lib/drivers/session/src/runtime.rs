//! `SessionRuntime` - concrete implementation of all session API traits.
//!
//! This module provides [`SessionRuntime`], which bundles `Session`, `KernelContext`,
//! and `CommandExecutor` to implement all the focused API traits.
//!
//! # Design
//!
//! ```text
//! SessionRuntime
//! ├── Session (mode stack, windows, extensions)
//! ├── KernelContext (buffers, registers, marks)
//! ├── CommandExecutor (command lookup/execution)
//! └── StateChanges (accumulated changes)
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use reovim_driver_session::SessionRuntime;
//!
//! // In runner's event loop:
//! let mut runtime = SessionRuntime::new(
//!     &mut session,
//!     &mut kernel,
//!     &command_executor,
//! );
//!
//! // Resolver uses runtime via trait bounds
//! resolver.resolve_with_session(&key, &mut state, &mut runtime);
//!
//! // Take accumulated changes
//! let changes = runtime.take_changes();
//!
//! // Broadcast notifications
//! if changes.mode_changed {
//!     notify_mode_change().await;
//! }
//! ```

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_kernel::api::v1::{BufferId, CommandId, KernelContext, ModeId, Position, WindowId},
};

use crate::{
    Session, SessionExtension, Window,
    api::{
        BufferApi, BufferError, ChangeTracker, CommandApi, CommandExecutor, ExtensionApi, ModeApi,
        ModeError, Selection, StateChanges, WindowApi, WindowError,
    },
    transition::{PopResult, TransitionContext},
};

/// Runtime that implements all session API traits.
///
/// Bundles `Session` + `KernelContext` + `CommandExecutor`.
/// Changes accumulate internally; runner takes at end via [`take_changes`].
///
/// [`take_changes`]: ChangeTracker::take_changes
pub struct SessionRuntime<'a> {
    /// Per-session state (mode stack, windows, extensions).
    session: &'a mut Session,
    /// Kernel context (buffers, registers, marks).
    kernel: &'a KernelContext,
    /// Command executor for looking up and running commands.
    executor: &'a dyn CommandExecutor,
    /// Accumulated changes - runner takes at end.
    changes: StateChanges,
}

impl<'a> SessionRuntime<'a> {
    /// Create a new runtime.
    pub fn new(
        session: &'a mut Session,
        kernel: &'a KernelContext,
        executor: &'a dyn CommandExecutor,
    ) -> Self {
        Self {
            session,
            kernel,
            executor,
            changes: StateChanges::new(),
        }
    }

    /// Get direct access to the session.
    #[must_use]
    pub const fn session(&self) -> &Session {
        self.session
    }

    /// Get mutable access to the session.
    pub const fn session_mut(&mut self) -> &mut Session {
        self.session
    }

    /// Get direct access to the kernel context.
    #[must_use]
    pub const fn kernel(&self) -> &KernelContext {
        self.kernel
    }
}

// === ModeApi ===

impl ModeApi for SessionRuntime<'_> {
    fn current_mode(&self) -> &ModeId {
        self.session.mode_stack.current()
    }

    fn home_mode(&self) -> &ModeId {
        self.session.mode_stack.home()
    }

    fn mode_depth(&self) -> usize {
        self.session.mode_stack.depth()
    }

    fn is_mode_active(&self, mode: &ModeId) -> bool {
        self.session.mode_stack.contains(mode)
    }

    fn mode_stack(&self) -> Vec<ModeId> {
        self.session.mode_stack.as_slice().to_vec()
    }

    fn push_mode(&mut self, mode: ModeId, _ctx: TransitionContext) {
        self.session.mode_stack.push(mode);
        self.changes.record_mode_change();
    }

    fn pop_mode(&mut self, _result: Option<PopResult>) -> Result<(), ModeError> {
        if self.session.mode_stack.depth() <= 1 {
            return Err(ModeError::CannotPopHomeMode);
        }
        self.session.mode_stack.pop();
        self.changes.record_mode_change();
        Ok(())
    }

    fn set_mode(&mut self, mode: ModeId, _ctx: TransitionContext) {
        self.session.mode_stack.set(mode);
        self.changes.record_mode_change();
    }
}

// === BufferApi ===

impl BufferApi for SessionRuntime<'_> {
    fn active_buffer(&self) -> Option<BufferId> {
        self.session.active_buffer()
    }

    fn buffer_line(&self, buffer: BufferId, line: usize) -> Option<String> {
        self.kernel
            .buffers
            .get(buffer)
            .and_then(|buf| buf.read().line(line).map(String::from))
    }

    fn buffer_line_count(&self, buffer: BufferId) -> Option<usize> {
        self.kernel
            .buffers
            .get(buffer)
            .map(|buf| buf.read().line_count())
    }

    fn cursor_position(&self, buffer: BufferId) -> Option<Position> {
        // Get from window displaying this buffer
        self.session
            .windows
            .windows
            .iter()
            .find(|w| w.buffer_id == Some(buffer))
            .map(|w| Position::new(w.cursor.line, w.cursor.column))
    }

    fn selection(&self, _buffer: BufferId) -> Option<Selection> {
        // Selection is stored per-buffer in the kernel
        // TODO: Implement when selection storage is added to session
        None
    }

    fn insert_text(&mut self, buffer: BufferId, pos: Position, text: &str) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            buf.write().insert_at(pos, text);
            self.changes.record_buffer_modified(buffer);
        }
    }

    fn delete_range(&mut self, buffer: BufferId, start: Position, end: Position) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            buf.write().delete_range(start, end);
            self.changes.record_buffer_modified(buffer);
        }
    }

    fn move_cursor(&mut self, buffer: BufferId, pos: Position) {
        // Update cursor in window displaying this buffer
        if let Some(window) = self
            .session
            .windows
            .windows
            .iter_mut()
            .find(|w| w.buffer_id == Some(buffer))
        {
            window.cursor.line = pos.line;
            window.cursor.column = pos.column;
            self.changes.record_cursor_move(buffer);
        }
    }

    fn set_selection(&mut self, buffer: BufferId, _sel: Option<Selection>) {
        // TODO: Implement selection storage
        self.changes.record_selection_change(buffer);
    }

    fn create_buffer(&mut self, name: Option<&str>, content: &str) -> BufferId {
        use reovim_kernel::api::v1::Buffer;

        let mut buffer = Buffer::from_string(content);
        if let Some(name) = name {
            buffer.set_file_path(Some(name.to_string()));
        }
        let id = self.kernel.buffers.register(buffer);
        self.changes.record_buffer_created(id);
        id
    }

    fn delete_buffer(&mut self, buffer: BufferId) -> Result<(), BufferError> {
        if self.kernel.buffers.count() <= 1 {
            return Err(BufferError::CannotDeleteLastBuffer);
        }
        if self.kernel.buffers.unregister(buffer).is_err() {
            return Err(BufferError::NotFound(buffer));
        }
        self.changes.record_buffer_deleted(buffer);
        Ok(())
    }

    fn rename_buffer(&mut self, buffer: BufferId, new_name: &str) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            buf.write().set_file_path(Some(new_name.to_string()));
            self.changes
                .record_buffer_renamed(buffer, new_name.to_string());
        }
    }
}

// === WindowApi ===

impl WindowApi for SessionRuntime<'_> {
    fn active_window(&self) -> Option<WindowId> {
        self.session.windows.active_id()
    }

    fn window_count(&self) -> usize {
        self.session.windows.len()
    }

    fn window_buffer(&self, window: WindowId) -> Option<BufferId> {
        self.session.windows.get(window).and_then(|w| w.buffer_id)
    }

    fn create_window(&mut self, buffer: Option<BufferId>) -> WindowId {
        let mut window = Window::new();
        window.buffer_id = buffer;
        let id = window.id;
        self.session.windows.add(window);
        self.changes.record_window_created(id);
        id
    }

    fn close_window(&mut self, window: WindowId) -> Result<(), WindowError> {
        if self.session.windows.len() <= 1 {
            return Err(WindowError::CannotCloseLastWindow);
        }
        // Find and remove the window
        let idx = self
            .session
            .windows
            .windows
            .iter()
            .position(|w| w.id == window);
        if let Some(idx) = idx {
            self.session.windows.windows.remove(idx);
            self.changes.record_window_closed(window);
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }

    fn focus_window(&mut self, window: WindowId) -> Result<(), WindowError> {
        if !self.session.windows.set_active(window) {
            return Err(WindowError::NotFound(window));
        }
        self.changes.record_focus_change();
        Ok(())
    }

    fn set_window_buffer(&mut self, window: WindowId, buffer: BufferId) -> Result<(), WindowError> {
        if self.kernel.buffers.get(buffer).is_none() {
            return Err(WindowError::BufferNotFound(buffer));
        }
        if let Some(w) = self.session.windows.get_mut(window) {
            w.buffer_id = Some(buffer);
            self.changes.window_changed = true;
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }
}

// === CommandApi ===

impl CommandApi for SessionRuntime<'_> {
    fn execute_command(&mut self, cmd: CommandId, ctx: CommandContext) -> CommandResult {
        // NOTE: Full command execution requires mutable KernelContext.
        // CommandExecutor expects `&mut KernelContext`, but we only have `&KernelContext`.
        //
        // For now, we validate that the command exists but defer actual execution
        // to the runner which has mutable access to KernelContext.
        //
        // Future work: Either change CommandExecutor to not need mutable context,
        // or have commands receive SessionRuntime directly.
        //
        // This reference ensures the executor field is used (for dead code check).
        let _ = &self.executor;
        let _ = (cmd, ctx);
        CommandResult::Error("command execution via SessionRuntime not yet implemented".to_string())
    }
}

// === ExtensionApi ===

impl ExtensionApi for SessionRuntime<'_> {
    fn ext<T: SessionExtension>(&self) -> Option<&T> {
        self.session.extensions.get::<T>()
    }

    fn ext_mut<T: SessionExtension>(&mut self) -> &mut T {
        self.session.extensions.get_or_insert::<T>()
    }
}

// === ChangeTracker ===

impl ChangeTracker for SessionRuntime<'_> {
    fn take_changes(&mut self) -> StateChanges {
        std::mem::take(&mut self.changes)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::SessionId, reovim_kernel::api::v1::ModuleId};

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_mode_2() -> ModeId {
        ModeId::with_discriminant(ModuleId::new("test"), "insert", 1)
    }

    struct StubExecutor;

    impl CommandExecutor for StubExecutor {
        fn execute(
            &self,
            _cmd: &CommandId,
            _ctx: &CommandContext,
            _kernel: &mut KernelContext,
        ) -> Option<CommandResult> {
            Some(CommandResult::Success)
        }
    }

    #[test]
    fn test_mode_api() {
        let mut session = Session::new(SessionId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        let mut runtime = SessionRuntime::new(&mut session, &kernel, &executor);

        // Check initial state
        assert_eq!(runtime.current_mode(), &test_mode());
        assert_eq!(runtime.mode_depth(), 1);
        assert!(runtime.is_mode_active(&test_mode()));

        // Push mode
        runtime.push_mode(test_mode_2(), TransitionContext::new());
        assert_eq!(runtime.current_mode(), &test_mode_2());
        assert_eq!(runtime.mode_depth(), 2);

        // Pop mode
        let result = runtime.pop_mode(None);
        assert!(result.is_ok());
        assert_eq!(runtime.current_mode(), &test_mode());

        // Cannot pop home mode
        let result = runtime.pop_mode(None);
        assert!(matches!(result, Err(ModeError::CannotPopHomeMode)));

        // Check changes were recorded
        let changes = runtime.take_changes();
        assert!(changes.mode_changed);
    }

    #[test]
    fn test_window_api() {
        let mut session = Session::new(SessionId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        let mut runtime = SessionRuntime::new(&mut session, &kernel, &executor);

        // Create window
        let window_id = runtime.create_window(None);
        assert_eq!(runtime.window_count(), 1);
        assert_eq!(runtime.active_window(), Some(window_id));

        // Cannot close last window
        let result = runtime.close_window(window_id);
        assert!(matches!(result, Err(WindowError::CannotCloseLastWindow)));

        // Create second window
        let window_id2 = runtime.create_window(None);
        assert_eq!(runtime.window_count(), 2);

        // Focus second window
        let result = runtime.focus_window(window_id2);
        assert!(result.is_ok());

        // Close first window
        let result = runtime.close_window(window_id);
        assert!(result.is_ok());
        assert_eq!(runtime.window_count(), 1);

        // Check changes
        let changes = runtime.take_changes();
        assert!(changes.window_changed);
        assert!(!changes.windows_created.is_empty());
    }

    #[test]
    fn test_extension_api() {
        #[derive(Debug, Default)]
        struct TestExtension {
            value: i32,
        }

        impl SessionExtension for TestExtension {
            fn create() -> Self {
                Self { value: 42 }
            }
        }

        let mut session = Session::new(SessionId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        let mut runtime = SessionRuntime::new(&mut session, &kernel, &executor);

        // Extension doesn't exist initially
        assert!(runtime.ext::<TestExtension>().is_none());

        // ext_mut creates it
        let ext = runtime.ext_mut::<TestExtension>();
        assert_eq!(ext.value, 42);
        ext.value = 100;

        // ext now returns it
        assert_eq!(runtime.ext::<TestExtension>().unwrap().value, 100);
    }

    #[test]
    fn test_change_tracking() {
        let mut session = Session::new(SessionId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        let mut runtime = SessionRuntime::new(&mut session, &kernel, &executor);

        // No changes initially
        assert!(!runtime.changes.has_changes());

        // Push mode records change
        runtime.push_mode(test_mode_2(), TransitionContext::new());
        assert!(runtime.changes.mode_changed);

        // Take changes resets
        let changes = runtime.take_changes();
        assert!(changes.mode_changed);
        assert!(!runtime.changes.has_changes());
    }
}
