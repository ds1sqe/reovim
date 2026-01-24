//! Test utilities for session-based command testing.
//!
//! This module provides [`TestSessionRuntime`], which simplifies testing commands
//! that use the new `SessionRuntime` signature.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::testing::TestSessionRuntime;
//! use reovim_driver_command::{CommandHandler, CommandContext, CommandResult};
//!
//! #[test]
//! fn test_enter_insert_mode() {
//!     let mut test = TestSessionRuntime::new();
//!     let cmd = EnterInsertMode;
//!     let args = CommandContext::new();
//!
//!     // Use with_runtime to automatically capture changes
//!     let result = test.with_runtime(|runtime| {
//!         cmd.execute(runtime, &args)
//!     });
//!
//!     assert_eq!(result, CommandResult::Success);
//!     test.assert_mode_name("insert");
//!     assert!(test.changes().mode_changed);
//! }
//! ```

use {
    crate::{
        ClientId, Session,
        api::{CommandExecutor, StateChanges},
        runtime::SessionRuntime,
    },
    reovim_arch::sync::RwLock,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_kernel::api::v1::{
        Buffer, BufferError, BufferId, BufferManager, CommandId, KernelContext, ModeId, ModuleId,
        Position,
    },
    std::{collections::HashMap, sync::Arc},
};

/// Test helper for commands using `SessionRuntime`.
///
/// Owns all the components needed to create a `SessionRuntime` and provides
/// convenient assertion methods for testing.
pub struct TestSessionRuntime {
    session: Session,
    kernel: KernelContext,
    executor: StubExecutor,
    /// Accumulated changes from operations.
    changes: StateChanges,
}

impl Default for TestSessionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl TestSessionRuntime {
    /// Create a `KernelContext` that uses a real buffer manager for testing.
    fn make_test_kernel() -> KernelContext {
        use reovim_kernel::api::v1::{
            EventBus, MarkBank, MotionEngine, OptionRegistry, RegisterBank, ServiceRegistry,
            TextObjectEngine,
        };

        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        )
    }

    /// Create a new test runtime with default normal mode.
    #[must_use]
    pub fn new() -> Self {
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        Self {
            session: Session::new(ClientId::new(1), home_mode),
            kernel: Self::make_test_kernel(),
            executor: StubExecutor,
            changes: StateChanges::new(),
        }
    }

    /// Create a test runtime with a specific home mode.
    #[must_use]
    pub fn with_home_mode(mode: ModeId) -> Self {
        Self {
            session: Session::new(ClientId::new(1), mode),
            kernel: Self::make_test_kernel(),
            executor: StubExecutor,
            changes: StateChanges::new(),
        }
    }

    /// Create a test runtime with a buffer containing the given content.
    #[must_use]
    pub fn with_buffer(content: &str) -> Self {
        let mut test = Self::new();

        // Create buffer with content
        let buffer = Buffer::from_string(content);
        let buffer_id = test.kernel.buffers.register(buffer);

        // Create a window displaying this buffer
        let mut window = crate::Window::new();
        window.buffer_id = Some(buffer_id);
        test.session.windows.add(window);

        // Set the active buffer (SSOT for session state)
        test.session.set_active_buffer(Some(buffer_id));

        test
    }

    /// Execute operations with a runtime, capturing changes automatically.
    ///
    /// This is the preferred way to use the test runtime. Changes are
    /// automatically captured when the callback returns.
    ///
    /// # Example
    ///
    /// ```ignore
    /// test.with_runtime(|runtime| {
    ///     runtime.push_mode(insert_mode, TransitionContext::new());
    /// });
    /// assert!(test.changes().mode_changed);
    /// ```
    pub fn with_runtime<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut SessionRuntime<'_>) -> R,
    {
        use crate::api::ChangeTracker;

        let mut runtime = SessionRuntime::new(&mut self.session, &self.kernel, &self.executor);
        let result = f(&mut runtime);
        let changes = ChangeTracker::take_changes(&mut runtime);
        self.changes.merge(changes);
        result
    }

    /// Get a mutable reference to the `SessionRuntime`.
    ///
    /// **Note**: Changes are NOT automatically captured when using this method.
    /// Prefer `with_runtime` for tests that need to verify changes.
    /// Use this for simple operations where change tracking isn't needed.
    pub fn runtime(&mut self) -> SessionRuntime<'_> {
        SessionRuntime::new(&mut self.session, &self.kernel, &self.executor)
    }

    /// Take accumulated changes and reset the tracker.
    ///
    /// Returns all changes that have been recorded since the last call.
    pub fn take_changes(&mut self) -> StateChanges {
        std::mem::take(&mut self.changes)
    }

    /// Get reference to all accumulated changes (doesn't reset).
    #[must_use]
    pub const fn changes(&self) -> &StateChanges {
        &self.changes
    }

    // === Assertions ===

    /// Assert the current mode matches the expected mode ID.
    ///
    /// # Panics
    ///
    /// Panics if the current mode doesn't match.
    pub fn assert_mode(&self, expected: &ModeId) {
        let current = self.session.mode_stack.current();
        assert_eq!(current, expected, "Expected mode {expected:?}, got {current:?}");
    }

    /// Assert the current mode name matches (ignores module).
    ///
    /// # Panics
    ///
    /// Panics if the mode name doesn't match.
    pub fn assert_mode_name(&self, expected_name: &str) {
        let current = self.session.mode_stack.current();
        assert_eq!(
            current.name(),
            expected_name,
            "Expected mode name '{}', got '{}'",
            expected_name,
            current.name()
        );
    }

    /// Assert the mode stack depth.
    ///
    /// # Panics
    ///
    /// Panics if the depth doesn't match.
    pub fn assert_mode_depth(&self, expected: usize) {
        let depth = self.session.mode_stack.depth();
        assert_eq!(depth, expected, "Expected mode depth {expected}, got {depth}");
    }

    /// Assert the cursor position for the active buffer.
    ///
    /// # Panics
    ///
    /// Panics if no active window or cursor position doesn't match.
    pub fn assert_cursor(&self, line: usize, column: usize) {
        let window = self
            .session
            .windows
            .active()
            .expect("No active window for cursor assertion");
        assert_eq!(
            (window.cursor.line, window.cursor.column),
            (line, column),
            "Expected cursor at ({}, {}), got ({}, {})",
            line,
            column,
            window.cursor.line,
            window.cursor.column
        );
    }

    /// Assert the buffer content for the active buffer.
    ///
    /// # Panics
    ///
    /// Panics if no active buffer or content doesn't match.
    pub fn assert_buffer_content(&self, expected: &str) {
        let buffer_id = self
            .session
            .active_buffer()
            .expect("No active buffer for content assertion");
        let buffer = self
            .kernel
            .buffers
            .get(buffer_id)
            .expect("Buffer not found");
        let content = buffer.read().content();
        assert_eq!(
            content, expected,
            "Buffer content mismatch.\nExpected:\n{expected}\nGot:\n{content}"
        );
    }

    /// Assert the buffer line count for the active buffer.
    ///
    /// # Panics
    ///
    /// Panics if no active buffer or line count doesn't match.
    pub fn assert_line_count(&self, expected: usize) {
        let buffer_id = self
            .session
            .active_buffer()
            .expect("No active buffer for line count assertion");
        let buffer = self
            .kernel
            .buffers
            .get(buffer_id)
            .expect("Buffer not found");
        let count = buffer.read().line_count();
        assert_eq!(count, expected, "Expected {expected} lines, got {count}");
    }

    /// Assert window count.
    ///
    /// # Panics
    ///
    /// Panics if window count doesn't match.
    pub fn assert_window_count(&self, expected: usize) {
        let count = self.session.windows.len();
        assert_eq!(count, expected, "Expected {expected} windows, got {count}");
    }

    // === Getters for advanced assertions ===

    /// Get the current mode ID.
    #[must_use]
    pub fn current_mode(&self) -> &ModeId {
        self.session.mode_stack.current()
    }

    /// Get the active buffer ID, if any.
    #[must_use]
    pub const fn active_buffer(&self) -> Option<BufferId> {
        self.session.active_buffer()
    }

    /// Get the cursor position for the active buffer.
    #[must_use]
    pub fn cursor_position(&self) -> Option<Position> {
        self.session
            .windows
            .active()
            .map(|w| Position::new(w.cursor.line, w.cursor.column))
    }

    /// Get buffer content for the active buffer.
    #[must_use]
    pub fn buffer_content(&self) -> Option<String> {
        self.session
            .active_buffer()
            .and_then(|id| self.kernel.buffers.get(id).map(|b| b.read().content()))
    }

    /// Get direct access to the kernel context.
    #[must_use]
    pub const fn kernel(&self) -> &KernelContext {
        &self.kernel
    }

    /// Get direct access to the session.
    #[must_use]
    pub const fn session(&self) -> &Session {
        &self.session
    }
}

/// Test buffer manager that actually stores buffers.
///
/// Unlike the kernel's `StubBufferManager`, this implementation properly stores
/// and retrieves buffers for testing.
struct TestBufferManager {
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
}

impl TestBufferManager {
    fn new() -> Self {
        Self {
            buffers: RwLock::new(HashMap::new()),
        }
    }
}

impl BufferManager for TestBufferManager {
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
        self.buffers.read().get(&id).cloned()
    }

    fn create(&self) -> BufferId {
        let id = BufferId::new();
        let buffer = Arc::new(RwLock::new(Buffer::new()));
        self.buffers.write().insert(id, buffer);
        id
    }

    fn register(&self, buffer: Buffer) -> BufferId {
        let id = BufferId::new();
        let buffer = Arc::new(RwLock::new(buffer));
        self.buffers.write().insert(id, buffer);
        id
    }

    fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
        self.buffers
            .write()
            .remove(&id)
            .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                Arc::try_unwrap(arc_buffer)
                    .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
            })
    }

    fn list(&self) -> Vec<BufferId> {
        self.buffers.read().keys().copied().collect()
    }

    fn count(&self) -> usize {
        self.buffers.read().len()
    }
}

/// Stub command executor for testing.
///
/// Always returns `Success` for any command execution.
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

#[cfg(test)]
mod tests {
    use {super::*, crate::api::ModeApi};

    #[test]
    fn test_new_creates_valid_runtime() {
        let mut test = TestSessionRuntime::new();
        let _runtime = test.runtime();
        test.assert_mode_name("normal");
        test.assert_mode_depth(1);
    }

    #[test]
    fn test_with_buffer_creates_buffer_and_window() {
        let test = TestSessionRuntime::with_buffer("hello world");
        test.assert_buffer_content("hello world");
        test.assert_window_count(1);
    }

    #[test]
    fn test_mode_operations() {
        let mut test = TestSessionRuntime::new();
        let insert_mode = ModeId::new(ModuleId::new("test"), "insert");

        test.with_runtime(|runtime| {
            runtime.push_mode(insert_mode.clone(), crate::TransitionContext::new());
        });

        test.assert_mode(&insert_mode);
        test.assert_mode_name("insert");
        test.assert_mode_depth(2);

        let changes = test.take_changes();
        assert!(changes.mode_changed);
    }

    #[test]
    fn test_buffer_operations() {
        use crate::api::BufferApi;

        let mut test = TestSessionRuntime::with_buffer("hello");

        test.with_runtime(|runtime| {
            if let Some(buffer_id) = runtime.active_buffer() {
                runtime.insert_text(buffer_id, Position::new(0, 5), " world");
            }
        });

        test.assert_buffer_content("hello world");

        let changes = test.take_changes();
        assert!(changes.buffer_modified);
    }

    #[test]
    fn test_cursor_operations() {
        use crate::api::BufferApi;

        let mut test = TestSessionRuntime::with_buffer("hello\nworld");

        test.with_runtime(|runtime| {
            if let Some(buffer_id) = runtime.active_buffer() {
                runtime.move_cursor(buffer_id, Position::new(1, 3));
            }
        });

        test.assert_cursor(1, 3);

        let changes = test.take_changes();
        assert!(changes.cursor_moved);
    }

    #[test]
    fn test_with_home_mode() {
        let custom_mode = ModeId::new(ModuleId::new("custom"), "special");
        let test = TestSessionRuntime::with_home_mode(custom_mode.clone());
        test.assert_mode(&custom_mode);
    }

    #[test]
    fn test_buffer_position_api() {
        use crate::api::BufferApi;

        let mut test = TestSessionRuntime::with_buffer("hello\nworld");
        let buffer_id = test.active_buffer().expect("should have active buffer");

        // Initial position should be (0, 0)
        test.with_runtime(|runtime| {
            let pos = runtime.buffer_position(buffer_id);
            assert_eq!(pos, Some(Position::new(0, 0)));

            // Set new position
            runtime.set_buffer_position(buffer_id, Position::new(1, 3));

            // Verify new position
            let pos = runtime.buffer_position(buffer_id);
            assert_eq!(pos, Some(Position::new(1, 3)));

            // Non-existent buffer returns None
            let fake_id = BufferId::new();
            assert!(runtime.buffer_position(fake_id).is_none());
        });
    }

    #[test]
    fn test_buffer_line_len_api() {
        use crate::api::BufferApi;

        let mut test = TestSessionRuntime::with_buffer("hello\nworld!");
        let buffer_id = test.active_buffer().expect("should have active buffer");

        test.with_runtime(|runtime| {
            // Check line lengths
            assert_eq!(runtime.buffer_line_len(buffer_id, 0), Some(5)); // "hello"
            assert_eq!(runtime.buffer_line_len(buffer_id, 1), Some(6)); // "world!"

            // Out of bounds line
            assert!(runtime.buffer_line_len(buffer_id, 99).is_none());

            // Non-existent buffer
            let fake_id = BufferId::new();
            assert!(runtime.buffer_line_len(fake_id, 0).is_none());
        });
    }

    #[test]
    fn test_buffer_position_vs_cursor_position() {
        use crate::api::BufferApi;

        let mut test = TestSessionRuntime::with_buffer("hello\nworld");
        let buffer_id = test.active_buffer().expect("should have active buffer");

        test.with_runtime(|runtime| {
            // Set buffer position (kernel) and window cursor separately
            runtime.set_buffer_position(buffer_id, Position::new(1, 2));
            runtime.move_cursor(buffer_id, Position::new(0, 4));

            // Verify they are independent
            assert_eq!(runtime.buffer_position(buffer_id), Some(Position::new(1, 2)));
            assert_eq!(runtime.cursor_position(buffer_id), Some(Position::new(0, 4)));
        });
    }

    #[test]
    fn test_register_api() {
        use crate::api::{RegisterApi, RegisterContent};

        let mut test = TestSessionRuntime::new();

        test.with_runtime(|runtime| {
            // Named registers should be initially empty
            assert!(runtime.get_register(Some('z')).is_none());

            // Set unnamed register
            runtime.set_register(None, RegisterContent::characterwise("hello"));
            let content = runtime.get_register(None).expect("should have content");
            assert_eq!(content.text, "hello");
            assert!(content.is_characterwise());

            // Set named register
            runtime.set_register(Some('a'), RegisterContent::linewise("world\n"));
            let content = runtime
                .get_register(Some('a'))
                .expect("should have content");
            assert_eq!(content.text, "world\n");
            assert!(content.is_linewise());

            // Overwrite unnamed register
            runtime.set_register(None, RegisterContent::linewise("replaced\n"));
            let content = runtime.get_register(None).expect("should have content");
            assert_eq!(content.text, "replaced\n");
            assert!(content.is_linewise());

            // Named register unchanged
            assert_eq!(runtime.get_register(Some('a')).unwrap().text, "world\n");
        });
    }
}
