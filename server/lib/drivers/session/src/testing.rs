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
        ClientId, Session, WindowLayout,
        api::{CommandExecutor, StateChanges},
        extension::ExtensionMap,
        runtime::SessionRuntime,
    },
    reovim_arch::sync::RwLock,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_kernel::api::v1::{
        Buffer, BufferError, BufferId, BufferManager, CommandId, KernelContext, ModeId, ModeStack,
        ModuleId, Position,
    },
    std::{collections::HashMap, sync::Arc},
};

/// Test helper for commands using `SessionRuntime`.
///
/// Owns all the components needed to create a `SessionRuntime` and provides
/// convenient assertion methods for testing.
///
/// # Architecture (#471 Phase 0)
///
/// Per-client state (`mode_stack`, `windows`, `extensions`) is held as **separate
/// fields** instead of inside `session`. This mirrors the production architecture
/// where `EditingState` (per-client) is separate from `Session` (shared).
///
/// This separation is REQUIRED by the borrow checker: `SessionRuntime::new()` takes
/// `&mut Session` AND `&mut ModeStack` etc. If `mode_stack` were inside `session`,
/// we'd have a double mutable borrow conflict.
///
/// ```text
/// TestSessionRuntime
/// ├── session: Session              // Shared infra (terminal_size, compositor)
/// ├── mode_stack: ModeStack         // Per-client (SEPARATE field)
/// ├── windows: WindowLayout         // Per-client (SEPARATE field)
/// ├── extensions: ExtensionMap      // Per-client (SEPARATE field)
/// ├── kernel: KernelContext
/// └── changes: StateChanges
/// ```
pub struct TestSessionRuntime {
    /// Shared session infrastructure (compositor, `terminal_size`).
    ///
    /// Does NOT contain per-client state - that's in separate fields below.
    session: Session,
    /// Per-client mode stack (SEPARATE from session for borrow-checker).
    ///
    /// Commands use this via `runtime.current_mode()`, `runtime.push_mode()`, etc.
    /// Public for direct test access (e.g., `test.mode_stack.current()`).
    pub mode_stack: ModeStack,
    /// Per-client window layout with cursors (SEPARATE from session).
    ///
    /// Commands use this via `runtime.windows()`.
    /// Public for direct test access (e.g., `test.windows.active_mut()`).
    pub windows: WindowLayout,
    /// Per-client extensions (SEPARATE from session).
    ///
    /// Commands use this via `runtime.ext::<T>()`, `runtime.ext_mut::<T>()`.
    pub extensions: ExtensionMap,
    /// Per-client compositor (#474).
    ///
    /// `None` for tests that don't need compositor. Matches `EditingState.compositor`.
    compositor: Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
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
            session: Session::new(ClientId::new(1), home_mode.clone()), // #491: home_mode in shared
            mode_stack: ModeStack::new(home_mode),                      // Per-client state
            windows: WindowLayout::empty(),                             // Per-client state
            extensions: ExtensionMap::new(),                            // Per-client state
            compositor: None,                                           // Per-client (#474)
            kernel: Self::make_test_kernel(),
            executor: StubExecutor,
            changes: StateChanges::new(),
        }
    }

    /// Create a test runtime with a specific home mode.
    #[must_use]
    pub fn with_home_mode(mode: ModeId) -> Self {
        Self {
            session: Session::new(ClientId::new(1), mode.clone()), // #491: home_mode in shared
            mode_stack: ModeStack::new(mode),                      // Per-client state
            windows: WindowLayout::empty(),                        // Per-client state
            extensions: ExtensionMap::new(),                       // Per-client state
            compositor: None,                                      // Per-client (#474)
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
        test.windows.add(window); // Use self.windows, NOT self.session.windows

        // Set the active buffer (SSOT for session state)
        test.session.set_active_buffer(Some(buffer_id));

        test
    }

    /// Execute operations with a runtime, capturing changes automatically.
    ///
    /// This is the preferred way to use the test runtime. Changes are
    /// automatically captured when the callback returns.
    ///
    /// # Per-Client State (#471 Phase 0)
    ///
    /// Uses `SessionRuntime::new()` with per-client state held as
    /// **separate fields** in `TestSessionRuntime`. This matches production behavior
    /// where `EditingState` (per-client) is separate from `Session` (shared).
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

        // Phase #471 Phase 0: Use new() with per-client state from SEPARATE fields
        // (not from session, which would cause double mutable borrow)
        let mut runtime = SessionRuntime::new(
            &mut self.session,    // Shared infra (no conflict)
            &mut self.mode_stack, // Separate field (no conflict)
            &mut self.windows,    // Separate field (no conflict)
            &mut self.extensions, // Separate field (no conflict)
            &mut self.compositor, // Per-client compositor (#474)
            &self.kernel,
            &self.executor,
        );
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
    ///
    /// # Per-Client State (#471 Phase 0)
    ///
    /// Uses `SessionRuntime::new()` with per-client state from separate fields.
    pub fn runtime(&mut self) -> SessionRuntime<'_> {
        SessionRuntime::new(
            &mut self.session,
            &mut self.mode_stack,
            &mut self.windows,
            &mut self.extensions,
            &mut self.compositor,
            &self.kernel,
            &self.executor,
        )
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
        let current = self.mode_stack.current(); // Use separate field, NOT session.mode_stack
        assert_eq!(current, expected, "Expected mode {expected:?}, got {current:?}");
    }

    /// Assert the current mode name matches (ignores module).
    ///
    /// # Panics
    ///
    /// Panics if the mode name doesn't match.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn assert_mode_name(&self, expected_name: &str) {
        let current = self.mode_stack.current(); // Use separate field
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
        let depth = self.mode_stack.depth(); // Use separate field
        assert_eq!(depth, expected, "Expected mode depth {expected}, got {depth}");
    }

    /// Assert the cursor position for the active buffer.
    ///
    /// # Panics
    ///
    /// Panics if no active window or cursor position doesn't match.
    pub fn assert_cursor(&self, line: usize, column: usize) {
        let window = self
            .windows // Use separate field, NOT session.windows
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
        let count = self.windows.len(); // Use separate field
        assert_eq!(count, expected, "Expected {expected} windows, got {count}");
    }

    // === Getters for advanced assertions ===

    /// Get the current mode ID.
    #[must_use]
    pub fn current_mode(&self) -> &ModeId {
        self.mode_stack.current() // Use separate field
    }

    /// Get the active buffer ID, if any.
    #[must_use]
    pub const fn active_buffer(&self) -> Option<BufferId> {
        self.session.active_buffer()
    }

    /// Get the cursor position for the active buffer.
    #[must_use]
    pub fn cursor_position(&self) -> Option<Position> {
        self.windows // Use separate field
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

    /// Set the compositor for layout testing.
    ///
    /// Required for testing commands that need window navigation, splitting,
    /// or other compositor operations (e.g., window-ops commands).
    pub fn set_compositor(
        &mut self,
        compositor: Box<dyn reovim_driver_display::layout::RootCompositor>,
    ) {
        self.compositor = Some(compositor);
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandExecutor for StubExecutor {
    fn execute(
        &self,
        _cmd: &CommandId,
        _ctx: &CommandContext,
        _kernel: &KernelContext,
    ) -> Option<CommandResult> {
        Some(CommandResult::Success)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::api::{BufferApi, ModeApi},
    };

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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    /// Test cursor manipulation via per-client window state (#471).
    ///
    /// Cursor is now a per-WINDOW property, not per-buffer. Commands update
    /// cursor via `runtime.windows_mut().active_mut()?.cursor`.
    #[test]
    fn test_cursor_operations() {
        let mut test = TestSessionRuntime::with_buffer("hello\nworld");

        // Initial cursor should be at (0, 0)
        test.assert_cursor(0, 0);

        // Manually set cursor via window (simulating what a command would do)
        // Use test.windows (separate field), NOT test.session.windows
        test.windows.active_mut().unwrap().cursor = crate::CursorPosition::new(1, 3);

        // Verify cursor was updated
        test.assert_cursor(1, 3);
    }

    #[test]
    fn test_with_home_mode() {
        let custom_mode = ModeId::new(ModuleId::new("custom"), "special");
        let test = TestSessionRuntime::with_home_mode(custom_mode.clone());
        test.assert_mode(&custom_mode);
    }

    /// Test cursor position via per-client window state (#471).
    ///
    /// Post-#471: Cursor is per-WINDOW, not per-buffer. Access via
    /// `test.windows.active()?.cursor` (read) or
    /// `test.windows.active_mut()?.cursor = ...` (write).
    ///
    /// Commands should use `runtime.windows()` to access the
    /// client's window state.
    #[test]
    fn test_cursor_position_via_window() {
        let mut test = TestSessionRuntime::with_buffer("hello\nworld");
        let _buffer_id = test.active_buffer().expect("should have active buffer");

        // Initial position should be (0, 0)
        // Use test.windows (separate field), NOT test.session.windows
        let window = test.windows.active().expect("should have active window");
        assert_eq!((window.cursor.line, window.cursor.column), (0, 0));

        // Set new position via window
        test.windows.active_mut().unwrap().cursor = crate::CursorPosition::new(1, 3);

        // Verify new position
        let window = test.windows.active().expect("should have active window");
        assert_eq!((window.cursor.line, window.cursor.column), (1, 3));

        // Window without buffer still has cursor
        let mut layout = crate::WindowLayout::empty();
        layout.add(crate::Window::new()); // Empty window (no buffer)
        assert!(layout.active().is_some()); // Window exists
        assert_eq!(layout.active().unwrap().buffer_id, None); // But no buffer
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

    // NOTE: test_buffer_position_vs_cursor_position removed (#471)
    // There is now only ONE cursor location: per-client Window.cursor
    // The kernel Buffer no longer has a cursor field.

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

    // =========================================================================
    // TestSessionRuntime assertion tests
    // =========================================================================

    #[test]
    fn test_assert_mode() {
        let mode = ModeId::new(ModuleId::new("test"), "normal");
        let test = TestSessionRuntime::new();
        test.assert_mode(&mode); // Should not panic
    }

    #[test]
    fn test_assert_mode_name() {
        let test = TestSessionRuntime::new();
        test.assert_mode_name("normal"); // Should not panic
    }

    #[test]
    fn test_assert_mode_depth() {
        let test = TestSessionRuntime::new();
        test.assert_mode_depth(1); // Should not panic
    }

    #[test]
    fn test_assert_window_count_empty() {
        let test = TestSessionRuntime::new();
        test.assert_window_count(0); // No windows initially
    }

    #[test]
    fn test_assert_window_count_with_buffer() {
        let test = TestSessionRuntime::with_buffer("hello");
        test.assert_window_count(1);
    }

    #[test]
    fn test_assert_line_count() {
        let test = TestSessionRuntime::with_buffer("line1\nline2\nline3");
        test.assert_line_count(3);
    }

    #[test]
    fn test_assert_buffer_content() {
        let test = TestSessionRuntime::with_buffer("hello world");
        test.assert_buffer_content("hello world");
    }

    #[test]
    fn test_assert_cursor_default() {
        let test = TestSessionRuntime::with_buffer("hello");
        test.assert_cursor(0, 0);
    }

    #[test]
    fn test_current_mode_getter() {
        let test = TestSessionRuntime::new();
        assert_eq!(test.current_mode().name(), "normal");
    }

    #[test]
    fn test_active_buffer_getter() {
        let test = TestSessionRuntime::with_buffer("test");
        assert!(test.active_buffer().is_some());
    }

    #[test]
    fn test_active_buffer_none_when_no_buffer() {
        let test = TestSessionRuntime::new();
        assert!(test.active_buffer().is_none());
    }

    #[test]
    fn test_cursor_position_getter() {
        let test = TestSessionRuntime::with_buffer("hello");
        assert_eq!(test.cursor_position(), Some(Position::new(0, 0)));
    }

    #[test]
    fn test_cursor_position_none_when_no_window() {
        let test = TestSessionRuntime::new();
        assert!(test.cursor_position().is_none());
    }

    #[test]
    fn test_buffer_content_getter() {
        let test = TestSessionRuntime::with_buffer("hello world");
        assert_eq!(test.buffer_content(), Some("hello world".to_string()));
    }

    #[test]
    fn test_buffer_content_none_when_no_buffer() {
        let test = TestSessionRuntime::new();
        assert!(test.buffer_content().is_none());
    }

    #[test]
    fn test_kernel_getter() {
        let test = TestSessionRuntime::new();
        let _kernel = test.kernel(); // Should not panic
    }

    #[test]
    fn test_session_getter() {
        let test = TestSessionRuntime::new();
        let session = test.session();
        assert_eq!(session.id.as_usize(), 1);
    }

    #[test]
    fn test_changes_getter() {
        let test = TestSessionRuntime::new();
        let changes = test.changes();
        assert!(!changes.has_changes());
    }

    #[test]
    fn test_take_changes_resets() {
        let mut test = TestSessionRuntime::new();
        let insert_mode = ModeId::new(ModuleId::new("test"), "insert");

        test.with_runtime(|runtime| {
            runtime.push_mode(insert_mode, crate::TransitionContext::new());
        });

        // Changes should be recorded
        assert!(test.changes().mode_changed);

        // Take should clear
        let changes = test.take_changes();
        assert!(changes.mode_changed);
        assert!(!test.changes().has_changes());
    }

    #[test]
    fn test_runtime_method() {
        let mut test = TestSessionRuntime::new();
        let runtime = test.runtime();
        // Just verify it returns a valid runtime
        assert_eq!(runtime.current_mode().name(), "normal");
    }

    #[test]
    fn test_with_runtime_returns_value() {
        let mut test = TestSessionRuntime::with_buffer("hello");
        let line_count = test.with_runtime(|runtime| {
            let buf = runtime.active_buffer().unwrap();
            runtime.buffer_line_count(buf).unwrap()
        });
        assert_eq!(line_count, 1);
    }

    #[test]
    fn test_default_impl() {
        let test = TestSessionRuntime::default();
        test.assert_mode_name("normal");
        test.assert_mode_depth(1);
    }

    // =========================================================================
    // TestBufferManager tests
    // =========================================================================

    #[test]
    fn test_buffer_manager_create() {
        let test = TestSessionRuntime::new();
        // Creating a buffer via runtime should work
        let mut test = test;
        let buf_id = test.with_runtime(|runtime| {
            use crate::api::BufferApi;
            runtime.create_buffer(None, "test content")
        });

        let content = test.with_runtime(|runtime| {
            use crate::api::BufferApi;
            runtime.buffer_content(buf_id)
        });
        assert_eq!(content, Some("test content".to_string()));
    }

    #[test]
    fn test_buffer_manager_list() {
        let mut test = TestSessionRuntime::new();

        test.with_runtime(|runtime| {
            use crate::api::BufferApi;
            runtime.create_buffer(None, "buf1");
            runtime.create_buffer(None, "buf2");
        });

        // Kernel should have buffers
        let count = test.kernel().buffers.count();
        assert!(count >= 2);
    }

    #[test]
    fn test_with_home_mode_custom() {
        let custom = ModeId::new(ModuleId::new("custom"), "special-mode");
        let test = TestSessionRuntime::with_home_mode(custom.clone());
        test.assert_mode(&custom);
        test.assert_mode_name("special-mode");
    }
}
