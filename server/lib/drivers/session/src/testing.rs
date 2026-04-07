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
        ClientId, Jumplist, MarkBank, Session, WindowLayout,
        api::{CommandExecutor, CommandHandle, StateChanges},
        extension::ExtensionMap,
        runtime::SessionRuntime,
    },
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::{BufferId, CommandId, KernelContext, ModeId, ModeStack, ModuleId},
    reovim_provider_text::Buffer,
    reovim_types_text::{HistoryRing, Position, RegisterBank},
    std::sync::Arc,
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
    /// Session-wide shared extensions (#543, #740).
    ///
    /// Empty by default; tests that need to exercise commands which touch
    /// shared session state (e.g. `CodecSessionState`) should populate this
    /// map directly so `runtime.shared_ext_mut::<T>()` returns `Some(_)`.
    pub shared_extensions: ExtensionMap,
    /// Per-client compositor (#474).
    ///
    /// `None` for tests that don't need compositor. Matches `EditingState.compositor`.
    compositor: Option<Box<dyn reovim_driver_layout::RootCompositor>>,
    /// Per-client tab pages (#401).
    tabs: crate::TabPageSet,
    /// Per-client register storage (#515).
    registers: RegisterBank,
    /// Per-client clipboard history ring (#515).
    clipboard_history: HistoryRing,
    /// Per-client local marks (#515).
    local_marks: MarkBank,
    jumplist: Jumplist,
    /// Per-client active buffer (#471).
    active_buffer: Option<BufferId>,
    /// Per-client terminal dimensions (#471).
    terminal_size: (u16, u16),
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

// Test infrastructure — not production code.
#[cfg_attr(coverage_nightly, coverage(off))]
impl TestSessionRuntime {
    /// Create a `KernelContext` that uses a real buffer manager for testing.
    ///
    /// Also registers a `TextBufferRegistry` service so that
    /// `SessionRuntime::text_buffer()` resolves through the session-layer
    /// registry (#740).
    fn make_test_kernel() -> KernelContext {
        let ctx = reovim_kernel::testing::create_test_context();
        ctx.services
            .register(Arc::new(crate::TextBufferRegistry::new()));
        ctx.services
            .register(Arc::new(crate::ByteUndoRegistry::new()));
        ctx
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
            shared_extensions: ExtensionMap::new(),                     // Shared (#543, #740)
            compositor: None,                                           // Per-client (#474)
            tabs: crate::TabPageSet::new(),                             // Per-client (#401)
            registers: RegisterBank::new(),                             // Per-client (#515)
            clipboard_history: HistoryRing::new(),                      // Per-client (#515)
            local_marks: MarkBank::new(),                               // Per-client (#515)
            jumplist: Jumplist::new(),
            active_buffer: None,     // Per-client (#471)
            terminal_size: (80, 24), // Per-client (#471)
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
            shared_extensions: ExtensionMap::new(),                // Shared (#543, #740)
            compositor: None,                                      // Per-client (#474)
            tabs: crate::TabPageSet::new(),                        // Per-client (#401)
            registers: RegisterBank::new(),                        // Per-client (#515)
            clipboard_history: HistoryRing::new(),                 // Per-client (#515)
            local_marks: MarkBank::new(),                          // Per-client (#515)
            jumplist: Jumplist::new(),
            active_buffer: None,     // Per-client (#471)
            terminal_size: (80, 24), // Per-client (#471)
            kernel: Self::make_test_kernel(),
            executor: StubExecutor,
            changes: StateChanges::new(),
        }
    }

    /// Create a test runtime with a buffer containing the given content.
    #[must_use]
    pub fn with_buffer(content: &str) -> Self {
        let mut test = Self::new();

        // Create buffer with content, register in both kernel and text registry (#740).
        let buffer = Buffer::from_string(content);
        let arc = Arc::new(RwLock::new(buffer));
        if let Some(reg) = test.kernel.services.get::<crate::TextBufferRegistry>() {
            reg.register(arc.clone());
        }
        let buffer_id = test.kernel.buffers.register(arc);

        // Create a window displaying this buffer
        let mut window = crate::Window::new();
        window.buffer_id = Some(buffer_id);
        test.windows.add(window); // Use self.windows, NOT self.session.windows

        // Set the active buffer (per-client state)
        test.active_buffer = Some(buffer_id);

        test
    }

    /// Create a test runtime with buffer content and a specific home mode.
    ///
    /// Use for mode-sensitive tests (e.g., textobjects that behave differently
    /// in visual vs normal mode).
    #[must_use]
    pub fn with_buffer_and_mode(content: &str, mode: ModeId) -> Self {
        let mut test = Self::with_home_mode(mode);
        let buffer = Buffer::from_string(content);
        let arc = Arc::new(RwLock::new(buffer));
        if let Some(reg) = test.kernel.services.get::<crate::TextBufferRegistry>() {
            reg.register(arc.clone());
        }
        let buffer_id = test.kernel.buffers.register(arc);
        let mut window = crate::Window::new();
        window.buffer_id = Some(buffer_id);
        test.windows.add(window);
        test.active_buffer = Some(buffer_id);
        test
    }

    /// Create a test runtime with a pre-configured window and mode.
    ///
    /// Use when tests need specific cursor position or window configuration
    /// before executing a command.
    #[must_use]
    pub fn with_window(window: crate::Window, mode: ModeId) -> Self {
        let mut test = Self::with_home_mode(mode);
        test.active_buffer = window.buffer_id;
        test.windows.add(window);
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
        let client = crate::ClientContext {
            mode_stack: &mut self.mode_stack,
            windows: &mut self.windows,
            extensions: &mut self.extensions,
            compositor: &mut self.compositor,
            tabs: &mut self.tabs,
            registers: &mut self.registers,
            clipboard_history: &mut self.clipboard_history,
            local_marks: &mut self.local_marks,
            jumplist: &mut self.jumplist,
            active_buffer: &mut self.active_buffer,
            terminal_size: &mut self.terminal_size,
        };
        let mut runtime =
            SessionRuntime::new(&mut self.session, client, &self.kernel, &self.executor)
                .with_shared_extensions(&mut self.shared_extensions); // #740 Phase 0
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
        let client = crate::ClientContext {
            mode_stack: &mut self.mode_stack,
            windows: &mut self.windows,
            extensions: &mut self.extensions,
            compositor: &mut self.compositor,
            tabs: &mut self.tabs,
            registers: &mut self.registers,
            clipboard_history: &mut self.clipboard_history,
            local_marks: &mut self.local_marks,
            jumplist: &mut self.jumplist,
            active_buffer: &mut self.active_buffer,
            terminal_size: &mut self.terminal_size,
        };
        SessionRuntime::new(&mut self.session, client, &self.kernel, &self.executor)
            .with_shared_extensions(&mut self.shared_extensions) // #740 Phase 0
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
            .active_buffer
            .expect("No active buffer for content assertion");
        let buffer = self
            .text_buffer_arc(buffer_id)
            .expect("Buffer not found in TextBufferRegistry");
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
            .active_buffer
            .expect("No active buffer for line count assertion");
        let buffer = self
            .text_buffer_arc(buffer_id)
            .expect("Buffer not found in TextBufferRegistry");
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
        self.active_buffer
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
        self.active_buffer
            .and_then(|id| self.text_buffer_arc(id).map(|b| b.read().content()))
    }

    /// Get a text buffer arc from `TextBufferRegistry`.
    ///
    /// Test helper for text-specific access. The kernel's `BufferManager` only
    /// stores `dyn KernelBuffer` (byte-only); text methods require this path.
    fn text_buffer_arc(
        &self,
        id: BufferId,
    ) -> Option<std::sync::Arc<reovim_arch::sync::RwLock<dyn reovim_provider_text::BufferOps>>>
    {
        self.kernel
            .services
            .get::<crate::TextBufferRegistry>()
            .and_then(|reg| reg.get(id))
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
    pub fn set_compositor(&mut self, compositor: Box<dyn reovim_driver_layout::RootCompositor>) {
        self.compositor = Some(compositor);
    }
}

/// Stub command executor for testing.
///
/// Returns `None` (command not found) for all lookups.
pub struct StubExecutor;

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandExecutor for StubExecutor {
    fn get_handle(&self, _id: &CommandId) -> Option<Arc<dyn CommandHandle>> {
        None
    }
}
// ============================================================================
// Public test helpers for downstream modules (#740)
// ============================================================================

/// Create a `KernelContext` with `TextBufferRegistry` and `ByteUndoRegistry`
/// for testing.
///
/// Like `reovim_kernel::testing::create_test_context()` but also registers
/// a `TextBufferRegistry` so text access via `runtime.text_buffer()` works,
/// and a `ByteUndoRegistry` for byte-level undo recording.
#[must_use]
pub fn create_test_kernel() -> reovim_kernel::api::v1::KernelContext {
    let ctx = reovim_kernel::testing::create_test_context();
    ctx.services
        .register(Arc::new(crate::TextBufferRegistry::new()));
    ctx.services
        .register(Arc::new(crate::ByteUndoRegistry::new()));
    ctx
}

/// Create and register a buffer in both kernel and `TextBufferRegistry`.
///
/// Use with contexts created by [`create_test_kernel`]. Returns the buffer ID.
#[must_use]
pub fn dual_register_buffer(
    ctx: &reovim_kernel::api::v1::KernelContext,
    content: &str,
) -> reovim_kernel::api::v1::BufferId {
    let buffer = Buffer::from_string(content);
    let arc = Arc::new(RwLock::new(buffer));
    if let Some(reg) = ctx.services.get::<crate::TextBufferRegistry>() {
        reg.register(arc.clone());
    }
    ctx.buffers.register(arc)
}

#[cfg(test)]
#[path = "testing_tests.rs"]
mod tests;
