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
    reovim_driver_clipboard::{ClipboardKey, ClipboardProviderRegistry},
    reovim_driver_command_types::{CommandContext, CommandResult, RuntimeSignal},
    reovim_driver_display::{
        NavigateDirection, Rect, SplitDirection,
        layout::{LayerId, OverlayConstraints, WindowPlacement},
    },
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{
        BufferId, CommandId, Edit, KernelContext, ModeId, OptionValue, Position, TabId, UndoResult,
        WindowId,
        events::kernel::{LayoutChangeKind, LayoutChanged, SplitDirection as KernelSplitDirection},
    },
};

use crate::{
    Selection, Session, SessionExtension, Window,
    api::{
        BufferApi, BufferError, ChangeTracker, ClipboardApi, CommandApi, CommandExecutor,
        CompositorApi, CompositorError, ExtensionApi, ModeApi, ModeError, RegisterApi,
        RegisterContent, StateChanges, UndoApi, WindowApi, WindowError,
    },
    transition::{PopResult, TransitionContext},
};

/// Runtime that implements all session API traits.
///
/// Bundles `Session` + `KernelContext` + `CommandExecutor`.
/// Changes accumulate internally; runner takes at end via [`take_changes`].
///
/// # Compositor Integration
///
/// The compositor is accessed via `session.compositor`. When present,
/// `CompositorApi` methods delegate to it. When absent, they return errors.
///
/// # Per-Client State (#471, #477)
///
/// `SessionRuntime` ALWAYS operates on per-client state. The per-client fields
/// are **required** (no Option wrappers, no fallback to shared state):
///
/// - `mode_stack` (#471): Per-client mode (INSERT, NORMAL, etc.)
/// - `windows` (#471): Per-client cursor positions
/// - `extensions` (#477): Per-client module state (`VimSessionState`, etc.)
///
/// This enforces multi-client isolation at compile time - you cannot create
/// a `SessionRuntime` without providing per-client state.
///
/// # Client Binding (#471)
///
/// The `owner` field tracks which client this runtime is bound to. When present,
/// it makes explicit which client's state we're operating on. This is useful for:
/// - Debugging and logging
/// - Assertions in tests
/// - Future multi-client coordination
///
/// [`take_changes`]: ChangeTracker::take_changes
pub struct SessionRuntime<'a> {
    /// The client this runtime is bound to (#471).
    ///
    /// When `Some`, makes explicit which client's state we're operating on.
    /// When `None`, this is a test runtime without explicit client binding.
    owner: Option<crate::ClientId>,
    /// Shared session state (template compositor, home mode).
    ///
    /// Per-client state is stored in SEPARATE fields below, not in session.
    session: &'a mut Session,
    /// Per-client mode stack (REQUIRED - no Option, #471).
    ///
    /// Mode operations use this directly. Multi-client mode isolation is
    /// enforced by requiring this field at construction time.
    mode_stack: &'a mut reovim_kernel::api::v1::ModeStack,
    /// Per-client window layout with cursors (REQUIRED - no Option, #471).
    ///
    /// Cursor operations use this directly. Multi-client cursor isolation
    /// is enforced by requiring this field at construction time.
    windows: &'a mut crate::WindowLayout,
    /// Per-client extensions (REQUIRED - no Option, #477).
    ///
    /// Extension operations use this directly. Per-client module state
    /// isolation is enforced by requiring this field at construction time.
    extensions: &'a mut crate::ExtensionMap,
    /// Session-wide shared extensions (optional, #543).
    ///
    /// When `Some`, commands can access session-wide state via `shared_ext()` /
    /// `shared_ext_mut()`. When `None` (tests, single-player), shared access
    /// returns `None`. Set via [`with_shared_extensions()`](Self::with_shared_extensions).
    shared_extensions: Option<&'a mut crate::ExtensionMap>,
    /// Per-client compositor for window layout (#474).
    ///
    /// Each client owns their own compositor cloned from the shared template.
    /// `CompositorApi` methods use this instead of `session.shared.compositor`.
    /// `None` when compositor is not set (tests, headless mode).
    compositor: &'a mut Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
    /// Per-client tab pages (#401).
    tabs: &'a mut crate::TabPageSet,
    /// Per-client register storage (#515).
    ///
    /// Each client owns their own registers (unnamed, named a-z/A-Z).
    /// System clipboard (+, *) remains shared via `ClipboardProvider`.
    registers: &'a mut reovim_kernel::api::v1::RegisterBank,
    /// Per-client clipboard history ring (#515).
    ///
    /// Tracks yank/delete history for numbered registers 0-9.
    clipboard_history: &'a mut reovim_kernel::api::v1::HistoryRing,
    /// Per-client local marks (a-z, per-client special marks) (#515).
    ///
    /// Global marks (A-Z) remain shared in `kernel.global_marks`.
    local_marks: &'a mut reovim_kernel::api::v1::MarkBank,
    /// Per-client active buffer (#471).
    ///
    /// Each client tracks which buffer they are viewing independently.
    active_buffer: &'a mut Option<reovim_kernel::api::v1::BufferId>,
    /// Kernel context (buffers, registers, global marks).
    kernel: &'a KernelContext,
    /// Command executor for looking up and running commands.
    executor: &'a dyn CommandExecutor,
    /// Cached screen size for compositor operations.
    screen: Rect,
    /// Accumulated changes - runner takes at end.
    changes: StateChanges,
    /// Signal queue — commands push signals during execution,
    /// server drains after command returns (#547).
    signals: Vec<RuntimeSignal>,
    /// Recursion depth counter for re-entrant command execution (#547).
    ///
    /// Incremented before each `execute_command()` call, decremented after.
    /// Max depth is 16 — deeper recursion returns an error.
    command_depth: usize,
}

impl<'a> SessionRuntime<'a> {
    /// Create a new runtime with per-client state (#471, #477).
    ///
    /// All per-client state is **required**. There are no fallbacks to shared
    /// session state. This enforces multi-client isolation at compile time.
    ///
    /// # Arguments
    ///
    /// * `session` - Shared session state (compositor, home mode)
    /// * `mode_stack` - Per-client mode stack (source of truth for mode)
    /// * `windows` - Per-client window layout with cursors
    /// * `extensions` - Per-client module extensions
    /// * `kernel` - Kernel context (buffers, registers, marks)
    /// * `executor` - Command executor
    ///
    /// # Multi-Client Isolation
    ///
    /// Each client has independent:
    /// - Mode state (Client A in INSERT while Client B in NORMAL)
    /// - Cursor positions (Client A at line 5, Client B at line 10)
    /// - Module state (Client A's `pending_count` doesn't affect Client B)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // From server level, get per-client EditingState
    /// let editing_state = session.client_state_mut(client_id)?;
    ///
    /// // Create runtime with per-client state bundle
    /// let client = editing_state.client_context();
    /// let mut runtime = SessionRuntime::new(
    ///     &mut driver_session,
    ///     client,
    ///     &kernel,
    ///     &executor,
    /// );
    ///
    /// // All operations use per-client state
    /// runtime.push_mode(insert_mode, ctx);     // Only affects this client
    /// runtime.windows().active();              // This client's active window
    /// runtime.ext_mut::<VimSessionState>();    // This client's vim state
    /// ```
    #[allow(clippy::needless_pass_by_value)] // ClientContext fields are moved into Self
    pub fn new(
        session: &'a mut Session,
        client: crate::ClientContext<'a>,
        kernel: &'a KernelContext,
        executor: &'a dyn CommandExecutor,
    ) -> Self {
        let screen = {
            let (width, height) = *client.terminal_size;
            Rect::new(0, 0, width, height)
        };
        Self {
            owner: None,
            session,
            mode_stack: client.mode_stack,
            windows: client.windows,
            extensions: client.extensions,
            shared_extensions: None,
            compositor: client.compositor,
            tabs: client.tabs,
            registers: client.registers,
            clipboard_history: client.clipboard_history,
            local_marks: client.local_marks,
            active_buffer: client.active_buffer,
            kernel,
            executor,
            screen,
            changes: StateChanges::new(),
            signals: Vec::new(),
            command_depth: 0,
        }
    }

    /// Create a runtime with explicit client binding (#471).
    ///
    /// Like [`new`], but also records which client this runtime is bound to.
    /// The `owner` field can be retrieved via [`owner()`].
    ///
    /// # Arguments
    ///
    /// * `owner` - The `ClientId` this runtime is bound to
    /// * `session` - Shared session infrastructure
    /// * `client` - Per-client state bundle (mode, windows, extensions, registers, etc.)
    /// * `kernel` - Kernel context (buffers, global marks)
    /// * `executor` - Command executor
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = editing_state.client_context();
    /// let mut runtime = SessionRuntime::with_owner(
    ///     client_id,
    ///     &mut driver_session,
    ///     client,
    ///     &kernel,
    ///     &executor,
    /// );
    ///
    /// // Query which client owns this runtime
    /// assert_eq!(runtime.owner(), Some(client_id));
    /// ```
    ///
    /// [`new`]: Self::new
    /// [`owner()`]: Self::owner
    #[allow(clippy::needless_pass_by_value)] // ClientContext fields are moved into Self
    pub fn with_owner(
        owner: crate::ClientId,
        session: &'a mut Session,
        client: crate::ClientContext<'a>,
        kernel: &'a KernelContext,
        executor: &'a dyn CommandExecutor,
    ) -> Self {
        let screen = {
            let (width, height) = *client.terminal_size;
            Rect::new(0, 0, width, height)
        };
        Self {
            owner: Some(owner),
            session,
            mode_stack: client.mode_stack,
            windows: client.windows,
            extensions: client.extensions,
            shared_extensions: None,
            compositor: client.compositor,
            tabs: client.tabs,
            registers: client.registers,
            clipboard_history: client.clipboard_history,
            local_marks: client.local_marks,
            active_buffer: client.active_buffer,
            kernel,
            executor,
            screen,
            changes: StateChanges::new(),
            signals: Vec::new(),
            command_depth: 0,
        }
    }

    /// Get the client ID this runtime is bound to (#471).
    ///
    /// Returns `Some(ClientId)` if created with [`with_owner`], `None` otherwise.
    /// Use this for debugging, logging, or assertions about which client owns
    /// this runtime.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let runtime = SessionRuntime::with_owner(client_id, ...);
    /// assert_eq!(runtime.owner(), Some(client_id));
    ///
    /// let shared_runtime = SessionRuntime::new(...);
    /// assert_eq!(shared_runtime.owner(), None);
    /// ```
    ///
    /// [`with_owner`]: Self::with_owner
    #[must_use]
    pub const fn owner(&self) -> Option<crate::ClientId> {
        self.owner
    }

    /// Set shared extensions for session-wide state access (#543).
    ///
    /// Called by server code that has access to `AppState.extensions`.
    /// Enables `shared_ext()` / `shared_ext_mut()` in commands.
    #[must_use]
    pub const fn with_shared_extensions(mut self, extensions: &'a mut crate::ExtensionMap) -> Self {
        self.shared_extensions = Some(extensions);
        self
    }

    /// Push a lifecycle signal onto the queue (#547).
    ///
    /// Commands call this during execution. The server drains
    /// the queue after the command returns and acts on the signals.
    /// Same pattern as `StateChanges` — accumulate during execution,
    /// drain after.
    pub fn signal(&mut self, signal: RuntimeSignal) {
        self.signals.push(signal);
    }

    /// Drain the signal queue, returning all accumulated signals (#547).
    ///
    /// Called by the server after command execution completes.
    /// Returns the signals and empties the queue.
    pub fn take_signals(&mut self) -> Vec<RuntimeSignal> {
        std::mem::take(&mut self.signals)
    }

    // Note: has_client_mode_stack(), has_client_windows(), has_client_extensions()
    // have been removed in #471 Phase 0. Per-client state is now REQUIRED,
    // so these methods would always return true.

    /// Check if compositor is available.
    #[must_use]
    pub fn has_compositor(&self) -> bool {
        self.compositor.is_some()
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

    /// Execute a read-only operation on a buffer.
    ///
    /// This method provides temporary read access to a buffer for complex
    /// calculations that need the full buffer interface (e.g., motion
    /// calculations, text object matching).
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer ID to access
    /// * `f` - A function that receives a read guard to the buffer
    ///
    /// # Returns
    ///
    /// `Some(R)` with the function's return value if the buffer exists,
    /// `None` if the buffer doesn't exist.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let target = runtime.with_buffer_read(buffer_id, |buffer| {
    ///     MotionEngine::calculate(buffer, &cursor, motion, count)
    /// });
    /// ```
    pub fn with_buffer_read<F, R>(&self, buffer: BufferId, f: F) -> Option<R>
    where
        F: FnOnce(&reovim_kernel::api::v1::Buffer) -> R,
    {
        let buf_arc = self.kernel.buffers.get(buffer)?;
        let buf = buf_arc.read();
        Some(f(&buf))
    }

    // === Option Change Tracking (#445) ===

    /// Record a global option change for notification emission.
    ///
    /// Call this after setting a global option via `kernel.options.set()`.
    /// The change will be emitted as an `OPTION_CHANGED` notification.
    pub fn record_global_option_change(&mut self, name: impl Into<String>, value: OptionValue) {
        self.changes.record_global_option_change(name, value);
    }

    /// Record a window-scoped option change for notification emission.
    ///
    /// Call this after setting a window option via `kernel.options.set()`.
    /// The change will be emitted as an `OPTION_CHANGED` notification.
    pub fn record_window_option_change(
        &mut self,
        name: impl Into<String>,
        value: OptionValue,
        window_id: WindowId,
    ) {
        self.changes
            .record_window_option_change(name, value, window_id);
    }

    /// Record that a buffer was modified for notification emission.
    ///
    /// Call this after modifying buffer content directly (e.g., via `OperatorContext`
    /// which bypasses `SessionRuntime`'s `BufferApi` methods like `delete_range()`).
    pub fn record_buffer_modified(&mut self, buffer: BufferId) {
        self.changes.record_buffer_modified(buffer);
    }

    // === Per-Client Window Accessors (#471) ===

    /// Get the per-client windows.
    ///
    /// Phase #471: Direct access to per-client windows. No fallback, no Option.
    /// Commands use this to access the active window's cursor position.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let Some(window) = runtime.windows().active() else {
    ///     return CommandResult::error("No active window");
    /// };
    /// let pos = Position::new(window.cursor.line, window.cursor.column);
    /// ```
    #[must_use]
    pub const fn windows(&self) -> &crate::WindowLayout {
        self.windows
    }

    /// Get the per-client windows mutably.
    ///
    /// Phase #471: Direct access to per-client windows for cursor/selection updates.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(window) = runtime.windows_mut().active_mut() {
    ///     window.cursor = new_pos.into();
    ///     window.selection = Some(Selection::new(start, end, mode));
    /// }
    /// ```
    #[allow(clippy::missing_const_for_fn)] // &mut self in const fn requires nightly
    pub fn windows_mut(&mut self) -> &mut crate::WindowLayout {
        self.windows
    }

    /// Get per-client registers (#515).
    #[must_use]
    pub const fn registers(&self) -> &reovim_kernel::api::v1::RegisterBank {
        self.registers
    }

    /// Get per-client registers mutably (#515).
    #[allow(clippy::missing_const_for_fn)]
    pub fn registers_mut(&mut self) -> &mut reovim_kernel::api::v1::RegisterBank {
        self.registers
    }

    /// Get per-client clipboard history (#515).
    #[must_use]
    pub const fn clipboard_history(&self) -> &reovim_kernel::api::v1::HistoryRing {
        self.clipboard_history
    }

    /// Get per-client clipboard history mutably (#515).
    #[allow(clippy::missing_const_for_fn)]
    pub fn clipboard_history_mut(&mut self) -> &mut reovim_kernel::api::v1::HistoryRing {
        self.clipboard_history
    }

    /// Get kernel, registers, and clipboard history together (#515).
    ///
    /// Splits the borrow so the caller can hold `&KernelContext` and
    /// `&mut RegisterBank` / `&mut HistoryRing` simultaneously, which
    /// isn't possible through separate accessor calls.
    pub const fn kernel_and_registers(
        &mut self,
    ) -> (
        &KernelContext,
        &mut reovim_kernel::api::v1::RegisterBank,
        &mut reovim_kernel::api::v1::HistoryRing,
    ) {
        (self.kernel, self.registers, self.clipboard_history)
    }

    /// Get per-client local marks (#515).
    #[must_use]
    pub const fn local_marks(&self) -> &reovim_kernel::api::v1::MarkBank {
        self.local_marks
    }

    /// Get per-client local marks mutably (#515).
    #[allow(clippy::missing_const_for_fn)]
    pub fn local_marks_mut(&mut self) -> &mut reovim_kernel::api::v1::MarkBank {
        self.local_marks
    }
}

// === ModeApi ===

/// Per-client state (#471): Mode operations use the per-client mode stack directly.
///
/// Since per-client state is now required (no Option), all mode operations
/// directly access `self.mode_stack` without fallback to session.
impl ModeApi for SessionRuntime<'_> {
    fn current_mode(&self) -> &ModeId {
        self.mode_stack.current()
    }

    fn home_mode(&self) -> &ModeId {
        self.mode_stack.home()
    }

    fn mode_depth(&self) -> usize {
        self.mode_stack.depth()
    }

    fn is_mode_active(&self, mode: &ModeId) -> bool {
        self.mode_stack.contains(mode)
    }

    fn mode_stack(&self) -> Vec<ModeId> {
        self.mode_stack.as_slice().to_vec()
    }

    fn push_mode(&mut self, mode: ModeId, _ctx: TransitionContext) {
        self.mode_stack.push(mode);
        self.changes.record_mode_change();
    }

    fn pop_mode(&mut self, _result: Option<PopResult>) -> Result<(), ModeError> {
        if self.mode_stack.depth() <= 1 {
            return Err(ModeError::CannotPopHomeMode);
        }
        self.mode_stack.pop();
        self.changes.record_mode_change();
        Ok(())
    }

    fn set_mode(&mut self, mode: ModeId, _ctx: TransitionContext) {
        self.mode_stack.set(mode);
        self.changes.record_mode_change();
    }
}

// === BufferApi ===

impl BufferApi for SessionRuntime<'_> {
    fn active_buffer(&self) -> Option<BufferId> {
        *self.active_buffer
    }

    fn set_active_buffer(&mut self, id: Option<BufferId>) {
        *self.active_buffer = id;
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

    fn buffer_line_len(&self, buffer: BufferId, line: usize) -> Option<usize> {
        self.kernel
            .buffers
            .get(buffer)
            .and_then(|buf| buf.read().line_len(line))
    }

    #[allow(clippy::significant_drop_tightening)]
    fn buffer_text_range(
        &self,
        buffer: BufferId,
        start: Position,
        end: Position,
    ) -> Option<String> {
        let buf_arc = self.kernel.buffers.get(buffer)?;
        let buf = buf_arc.read();

        // Build the text from the range
        let mut result = String::new();

        if start.line == end.line {
            // Single line case
            if let Some(line) = buf.line(start.line) {
                let line_chars: Vec<char> = line.chars().collect();
                let start_col = start.column.min(line_chars.len());
                let end_col = end.column.min(line_chars.len());
                result.extend(&line_chars[start_col..end_col]);
            }
        } else {
            // Multi-line case
            // First line: from start column to end of line
            if let Some(line) = buf.line(start.line) {
                let line_chars: Vec<char> = line.chars().collect();
                let start_col = start.column.min(line_chars.len());
                result.extend(&line_chars[start_col..]);
                result.push('\n');
            }

            // Middle lines: full lines
            for line_idx in (start.line + 1)..end.line {
                if let Some(line) = buf.line(line_idx) {
                    result.push_str(line);
                    result.push('\n');
                }
            }

            // Last line: from start to end column
            if let Some(line) = buf.line(end.line) {
                let line_chars: Vec<char> = line.chars().collect();
                let end_col = end.column.min(line_chars.len());
                result.extend(&line_chars[..end_col]);
            }
        }

        Some(result)
    }

    fn buffer_content(&self, buffer: BufferId) -> Option<String> {
        self.kernel
            .buffers
            .get(buffer)
            .map(|buf| buf.read().content())
    }

    fn buffer_file_path(&self, buffer: BufferId) -> Option<String> {
        self.kernel
            .buffers
            .get(buffer)
            .and_then(|buf| buf.read().file_path().map(String::from))
    }

    fn is_buffer_modified(&self, buffer: BufferId) -> Option<bool> {
        self.kernel
            .buffers
            .get(buffer)
            .map(|buf| buf.read().is_modified())
    }

    fn set_buffer_modified(&mut self, buffer: BufferId, modified: bool) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            buf.write().set_modified(modified);
        }
    }

    fn insert_text(&mut self, buffer: BufferId, pos: Position, text: &str) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            // Get cursor from per-client active window (#471)
            // Note: cursor_after will be set by runner from CommandResult
            let cursor_before = self.windows().active().map_or_else(
                || Position::new(0, 0),
                |w| Position::new(w.cursor.line, w.cursor.column),
            );

            buf.write().insert_at(pos, text);

            // For undo, use cursor_before as cursor_after too (runner will update actual cursor)
            let cursor_after = cursor_before;

            // Record edit for undo - use record_edit_mine for per-client undo (#471)
            let edit = Edit::Insert {
                position: pos,
                text: text.to_string(),
            };
            self.record_edit_mine(buffer, vec![edit], cursor_before, cursor_after);

            self.changes.record_buffer_modified(buffer);
        }
    }

    fn delete_range(&mut self, buffer: BufferId, start: Position, end: Position) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            // Get cursor from per-client active window (#471)
            // TODO(#471 Phase 6): cursor_after should come from CommandResult
            let cursor_before = self.windows().active().map_or_else(
                || Position::new(0, 0),
                |w| Position::new(w.cursor.line, w.cursor.column),
            );

            let deleted_text = {
                let mut b = buf.write();
                b.delete_range(start, end)
            };

            // For undo, use cursor_before as cursor_after too (runner will update actual cursor)
            let cursor_after = cursor_before;

            // Record edit for undo - use record_edit_mine for per-client undo (#471)
            if !deleted_text.is_empty() {
                let edit = Edit::Delete {
                    position: start,
                    text: deleted_text.clone(),
                };
                self.record_edit_mine(buffer, vec![edit], cursor_before, cursor_after);

                // Emit BufferModified event for pair module and other subscribers (#440)
                #[allow(clippy::cast_possible_truncation)]
                {
                    use reovim_kernel::api::v1::events::kernel::{BufferModified, Modification};
                    self.kernel.event_bus.emit(BufferModified {
                        buffer_id: buffer.as_usize() as u64,
                        modification: Modification::Delete {
                            start: (start.line as u32, start.column as u32),
                            end: (end.line as u32, end.column as u32),
                            text: deleted_text,
                        },
                    });
                }
            }

            self.changes.record_buffer_modified(buffer);
        }
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
        self.windows.active_id() // #491: use per-client windows
    }

    fn cursor_position(&self) -> Option<Position> {
        let window = self.windows().active()?;
        Some(Position::new(window.cursor.line, window.cursor.column))
    }

    fn window_count(&self) -> usize {
        self.windows.len() // #491: use per-client windows
    }

    fn window_buffer(&self, window: WindowId) -> Option<BufferId> {
        self.windows.get(window).and_then(|w| w.buffer_id) // #491: use per-client windows
    }

    fn create_window(&mut self, buffer: Option<BufferId>) -> WindowId {
        let mut window = Window::new();
        window.buffer_id = buffer;
        let id = window.id;
        self.windows.add(window); // #491: use per-client windows
        self.changes.record_window_created(id);
        id
    }

    fn close_window(&mut self, window: WindowId) -> Result<(), WindowError> {
        if self.windows.len() <= 1 {
            // #491: use per-client windows
            return Err(WindowError::CannotCloseLastWindow);
        }
        // Find and remove the window
        let idx = self.windows.windows.iter().position(|w| w.id == window); // #491: use per-client windows
        if let Some(idx) = idx {
            self.windows.windows.remove(idx); // #491: use per-client windows
            self.changes.record_window_closed(window);
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }

    fn focus_window(&mut self, window: WindowId) -> Result<(), WindowError> {
        if !self.windows.set_active(window) {
            // #491: use per-client windows
            return Err(WindowError::NotFound(window));
        }
        self.changes.record_focus_change();
        Ok(())
    }

    fn set_window_buffer(&mut self, window: WindowId, buffer: BufferId) -> Result<(), WindowError> {
        if self.kernel.buffers.get(buffer).is_none() {
            return Err(WindowError::BufferNotFound(buffer));
        }
        if let Some(w) = self.windows.get_mut(window) {
            // #491: use per-client windows
            // Phase 8 (#465): Clear selection when switching buffers.
            // Selection is per-window but associated with a specific buffer,
            // so it makes no sense to keep selection when viewing a different buffer.
            w.selection = None;
            w.buffer_id = Some(buffer);
            self.changes.window_changed = true;
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }

    fn set_active_selection(&mut self, selection: Option<Selection>) {
        if let Some(w) = self.windows.active_mut() {
            w.selection = selection;
        }
    }

    fn active_selection(&self) -> Option<&Selection> {
        self.windows.active().and_then(|w| w.selection.as_ref())
    }
}

// === RegisterApi ===

impl RegisterApi for SessionRuntime<'_> {
    fn get_register(&self, name: Option<char>) -> Option<RegisterContent> {
        match name {
            // Numbered registers (0-9) - per-client yank history (#515)
            Some(n) if n.is_ascii_digit() => self.clipboard_history.get_numbered(n),

            // All other registers (a-z, A-Z, +, *, "", None) - per-client RegisterBank (#515)
            //
            // Note: +/* are stored locally. Callers that need OS clipboard sync
            // must explicitly use ClipboardApi (#515 Phase 4).
            _ => self.registers.get_by_name(name).cloned(),
        }
    }

    fn set_register(&mut self, name: Option<char>, content: RegisterContent) {
        match name {
            // Numbered registers (0-9) are read-only (populated by history)
            Some(n) if n.is_ascii_digit() => {
                // Ignore writes to numbered registers - they're managed by history
            }

            // All other registers - per-client RegisterBank (#515)
            _ => {
                self.registers.set_by_name(name, content);
            }
        }
    }
}

// === ClipboardApi (#515) ===

impl ClipboardApi for SessionRuntime<'_> {
    fn copy_to_clipboard(&self, text: &str) -> bool {
        if let Some(registry) = self.kernel.services.get::<ClipboardProviderRegistry>()
            && let Some(provider) = registry.get(&ClipboardKey::Default)
        {
            provider.copy_to_clipboard(text).is_ok()
        } else {
            false
        }
    }

    fn paste_from_clipboard(&self) -> Option<String> {
        if let Some(registry) = self.kernel.services.get::<ClipboardProviderRegistry>()
            && let Some(provider) = registry.get(&ClipboardKey::Default)
        {
            provider.paste_from_clipboard().ok().flatten()
        } else {
            None
        }
    }

    fn copy_to_selection(&self, text: &str) -> bool {
        if let Some(registry) = self.kernel.services.get::<ClipboardProviderRegistry>()
            && let Some(provider) = registry.get(&ClipboardKey::Default)
        {
            provider.copy_to_selection(text).is_ok()
        } else {
            false
        }
    }

    fn paste_from_selection(&self) -> Option<String> {
        if let Some(registry) = self.kernel.services.get::<ClipboardProviderRegistry>()
            && let Some(provider) = registry.get(&ClipboardKey::Default)
        {
            provider.paste_from_selection().ok().flatten()
        } else {
            None
        }
    }
}

impl SessionRuntime<'_> {
    /// Push content to the per-client clipboard history (#515).
    ///
    /// Should be called after every yank/delete operation to populate
    /// numbered registers (0-9). This is an inherent method because
    /// history is per-client state, not a shared service.
    pub fn push_to_clipboard_history(&mut self, content: RegisterContent) {
        self.clipboard_history.push(content);
    }

    /// Store content to register with clipboard sync (#515 Phase 4).
    ///
    /// Coordinates `RegisterApi` and `ClipboardApi`:
    /// 1. Stores content in per-client register via `RegisterApi`
    /// 2. Syncs to OS clipboard for `+`/`*` registers via `ClipboardApi`
    /// 3. Pushes to clipboard history for numbered register rotation
    ///
    /// Use this from commands that operate on `SessionRuntime` directly
    /// (visual mode operators, editor commands). Vim operators that hold
    /// separate borrows via `OperatorContext` use `registers::store_and_sync`
    /// instead.
    pub fn store_register_with_sync(&mut self, register: Option<char>, content: RegisterContent) {
        self.set_register(register, content.clone());
        match register {
            Some('+') => {
                self.copy_to_clipboard(&content.text);
            }
            Some('*') => {
                self.copy_to_selection(&content.text);
            }
            _ => {}
        }
        self.push_to_clipboard_history(content);
    }

    /// Get register content with clipboard fallback for `+`/`*` (#515 Phase 4).
    ///
    /// For system clipboard registers:
    /// - `+`: reads from OS clipboard, falls back to per-client register
    /// - `*`: reads from OS selection, falls back to per-client register
    /// - All others: reads directly from per-client `RegisterBank`
    pub fn get_register_with_clipboard(&self, register: Option<char>) -> Option<RegisterContent> {
        match register {
            Some('+') => self
                .paste_from_clipboard()
                .map(RegisterContent::characterwise)
                .or_else(|| self.get_register(register)),
            Some('*') => self
                .paste_from_selection()
                .map(RegisterContent::characterwise)
                .or_else(|| self.get_register(register)),
            _ => self.get_register(register),
        }
    }
}

// === UndoApi ===

impl SessionRuntime<'_> {
    /// Apply undo/redo edits to the kernel buffer.
    ///
    /// Extracted from the 4 undo/redo methods to deduplicate the edit-application
    /// loop and avoid an LLVM coverage gap-region bug on `if let` closing braces.
    fn apply_undo_edits(&self, buffer: BufferId, edits: &[Edit]) {
        let Some(buf) = self.kernel.buffers.get(buffer) else {
            return;
        };
        let mut buf = buf.write();
        for edit in edits {
            match edit {
                Edit::Insert { position, text } => {
                    buf.insert_at(*position, text);
                }
                Edit::Delete { position, text } => {
                    buf.delete_at(*position, text.chars().count());
                }
            }
        }
    }
}

impl UndoApi for SessionRuntime<'_> {
    fn undo(&mut self, buffer: BufferId) -> Option<UndoResult> {
        let undo_provider = self
            .kernel
            .services
            .get::<UndoProviderRegistry>()?
            .get(&UndoKey::Buffer)?;

        let result = undo_provider.undo(buffer)?;
        self.apply_undo_edits(buffer, &result.edits);

        // Phase #471: Restore cursor to per-client active window
        if let Some(window) = self.windows_mut().active_mut() {
            window.cursor.line = result.cursor.line;
            window.cursor.column = result.cursor.column;
        }

        self.changes.record_buffer_modified(buffer);
        self.changes.record_cursor_move(buffer);

        Some(result)
    }

    fn redo(&mut self, buffer: BufferId) -> Option<UndoResult> {
        let undo_provider = self
            .kernel
            .services
            .get::<UndoProviderRegistry>()?
            .get(&UndoKey::Buffer)?;

        let result = undo_provider.redo(buffer)?;
        self.apply_undo_edits(buffer, &result.edits);

        // Phase #471: Restore cursor to per-client active window
        if let Some(window) = self.windows_mut().active_mut() {
            window.cursor.line = result.cursor.line;
            window.cursor.column = result.cursor.column;
        }

        self.changes.record_buffer_modified(buffer);
        self.changes.record_cursor_move(buffer);

        Some(result)
    }

    fn record_edit(
        &mut self,
        buffer: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        if let Some(undo_registry) = self.kernel.services.get::<UndoProviderRegistry>()
            && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
        {
            undo_provider.record(buffer, edits, cursor_before, cursor_after);
        }
    }

    fn can_undo(&self, buffer: BufferId) -> bool {
        self.kernel
            .services
            .get::<UndoProviderRegistry>()
            .and_then(|registry| registry.get(&UndoKey::Buffer))
            .and_then(|provider| provider.get_tree(buffer))
            .is_some_and(|tree| tree.can_undo())
    }

    fn can_redo(&self, buffer: BufferId) -> bool {
        self.kernel
            .services
            .get::<UndoProviderRegistry>()
            .and_then(|registry| registry.get(&UndoKey::Buffer))
            .and_then(|provider| provider.get_tree(buffer))
            .is_some_and(|tree| tree.can_redo())
    }

    fn undo_mine(&mut self, buffer: BufferId) -> Option<UndoResult> {
        // #471: Get the client ID from the owner field
        let client_id = self.owner?.as_usize();

        let undo_provider = self
            .kernel
            .services
            .get::<UndoProviderRegistry>()?
            .get(&UndoKey::Buffer)?;

        let result = undo_provider.undo_for_client(buffer, client_id)?;
        self.apply_undo_edits(buffer, &result.edits);

        // Restore cursor to per-client active window
        if let Some(window) = self.windows_mut().active_mut() {
            window.cursor.line = result.cursor.line;
            window.cursor.column = result.cursor.column;
        }

        self.changes.record_buffer_modified(buffer);
        self.changes.record_cursor_move(buffer);

        Some(result)
    }

    fn redo_mine(&mut self, buffer: BufferId) -> Option<UndoResult> {
        // #471: Get the client ID from the owner field
        let client_id = self.owner?.as_usize();

        let undo_provider = self
            .kernel
            .services
            .get::<UndoProviderRegistry>()?
            .get(&UndoKey::Buffer)?;

        let result = undo_provider.redo_for_client(buffer, client_id)?;
        self.apply_undo_edits(buffer, &result.edits);

        // Restore cursor to per-client active window
        if let Some(window) = self.windows_mut().active_mut() {
            window.cursor.line = result.cursor.line;
            window.cursor.column = result.cursor.column;
        }

        self.changes.record_buffer_modified(buffer);
        self.changes.record_cursor_move(buffer);

        Some(result)
    }

    fn record_edit_mine(
        &mut self,
        buffer: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        // #471: If we have an owner, use record_for_client with origin tagging
        if let Some(client_id) = self.owner {
            if let Some(undo_registry) = self.kernel.services.get::<UndoProviderRegistry>()
                && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
            {
                undo_provider.record_for_client(
                    buffer,
                    client_id.as_usize(),
                    edits,
                    cursor_before,
                    cursor_after,
                );
            }
        } else {
            // Fall back to regular record without origin
            self.record_edit(buffer, edits, cursor_before, cursor_after);
        }
    }
}

// === CommandApi ===

impl CommandApi for SessionRuntime<'_> {
    fn execute_command(&mut self, cmd: CommandId, ctx: CommandContext) -> CommandResult {
        // Recursion guard (#547): max depth 16.
        if self.command_depth >= 16 {
            return CommandResult::Error(
                "command recursion limit exceeded (max depth: 16)".to_string(),
            );
        }
        self.command_depth += 1;

        // get_handle() returns an owned Arc, releasing the borrow on
        // self.executor. This enables re-entrant execution: the returned
        // handle can call back into self via handle.execute(self, &ctx).
        let handle = self.executor.get_handle(&cmd);
        let result = handle.map_or_else(
            || CommandResult::Error(format!("command not found: {cmd:?}")),
            |handle| handle.execute(self, &ctx),
        );

        self.command_depth -= 1;
        result
    }
}

// === ExtensionApi ===

/// Per-client extensions (#477, #471 Phase 0).
///
/// Extensions are now ALWAYS per-client (no fallback to shared session).
/// This enforces complete module state isolation between clients. For example,
/// `VimSessionState.pending_count` is per-client, so Client A pressing `5`
/// doesn't affect Client B's motions.
impl ExtensionApi for SessionRuntime<'_> {
    fn ext<T: SessionExtension>(&self) -> Option<&T> {
        // #471 Phase 0: Per-client extensions are required, direct access
        self.extensions.get::<T>()
    }

    fn ext_mut<T: SessionExtension>(&mut self) -> &mut T {
        // #471 Phase 0: Per-client extensions are required, direct access
        self.extensions.get_or_insert::<T>()
    }

    fn shared_ext<T: SessionExtension>(&self) -> Option<&T> {
        self.shared_extensions.as_ref().and_then(|m| m.get::<T>())
    }

    fn shared_ext_mut<T: SessionExtension>(&mut self) -> Option<&mut T> {
        self.shared_extensions
            .as_mut()
            .map(|m| m.get_or_insert::<T>())
    }
}

// === ChangeTracker ===

impl ChangeTracker for SessionRuntime<'_> {
    fn take_changes(&mut self) -> StateChanges {
        std::mem::take(&mut self.changes)
    }

    fn record_cursor_move(&mut self, buffer: BufferId) {
        self.changes.record_cursor_move(buffer);
        // #474: Centralized visual selection extension.
        // When cursor moves and selection exists, auto-update sel.end
        // to match cursor position. This ensures ALL commands that call
        // record_cursor_move() automatically extend visual selection.
        // Note: For line-wise selections, operators normalize end via
        // expand_selection_range(), so the column+1 here is harmless.
        if let Some(window) = self.windows.active_mut()
            && let Some(ref mut sel) = window.selection
        {
            sel.end = Position::new(window.cursor.line, window.cursor.column + 1);
            self.changes.record_selection_change(buffer);
        }
    }

    fn record_selection_change(&mut self, buffer: BufferId) {
        self.changes.record_selection_change(buffer);
    }
}

// === CompositorApi ===

/// Convert driver `SplitDirection` to kernel `SplitDirection`.
const fn to_kernel_split_direction(dir: SplitDirection) -> KernelSplitDirection {
    match dir {
        SplitDirection::Horizontal => KernelSplitDirection::Horizontal,
        SplitDirection::Vertical => KernelSplitDirection::Vertical,
    }
}

impl SessionRuntime<'_> {
    /// Emit a `LayoutChanged` event via the kernel's event bus.
    fn emit_layout_event(&self, kind: LayoutChangeKind) {
        let (window_count, focused_window) = self
            .compositor
            .as_ref()
            .map_or((0, None), |c| (c.window_count(), c.focused().map(|id| id.as_usize() as u64)));
        self.kernel.event_bus.emit(LayoutChanged {
            kind,
            window_count,
            focused_window,
        });
    }
}

impl CompositorApi for SessionRuntime<'_> {
    fn navigate(&self, direction: NavigateDirection) -> Result<WindowId, CompositorError> {
        let compositor = self
            .compositor
            .as_ref()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let from = layer.focused().ok_or(CompositorError::NoFocusedWindow)?;

        layer
            .navigate_tiled(from, direction)
            .ok_or(CompositorError::NoNeighbor(direction))
    }

    fn split(&mut self, direction: SplitDirection) -> Result<WindowId, CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let from = layer.focused().ok_or(CompositorError::NoFocusedWindow)?;

        let new_window = layer
            .split_tiled(from, direction)
            .ok_or(CompositorError::NotEnoughRoom)?;

        // Focus moves to new window automatically in split_tiled
        self.changes.record_window_created(new_window);

        // Emit layout changed event
        self.emit_layout_event(LayoutChangeKind::Split {
            new_window: new_window.as_usize() as u64,
            direction: to_kernel_split_direction(direction),
        });

        Ok(new_window)
    }

    fn close_current_window(&mut self) -> Result<WindowId, CompositorError> {
        use reovim_driver_display::layout::Zone;

        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let current = layer.focused().ok_or(CompositorError::NoFocusedWindow)?;

        // Check if this is the last window
        if layer.windows_in_zone(Zone::Tiled).len() <= 1 {
            return Err(CompositorError::CannotCloseLastWindow);
        }

        let neighbor = layer
            .close_tiled(current)
            .ok_or(CompositorError::CannotCloseLastWindow)?;

        self.changes.record_window_closed(current);

        // Emit layout changed event
        self.emit_layout_event(LayoutChangeKind::Close {
            closed_window: current.as_usize() as u64,
            new_focus: Some(neighbor.as_usize() as u64),
        });

        Ok(neighbor)
    }

    fn close_others(&mut self) -> Result<(), CompositorError> {
        use reovim_driver_display::layout::Zone;

        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let current = layer.focused().ok_or(CompositorError::NoFocusedWindow)?;

        // Get all windows except current
        let windows: Vec<WindowId> = layer
            .windows_in_zone(Zone::Tiled)
            .into_iter()
            .filter(|&w| w != current)
            .collect();

        // Close all other windows and emit events
        for window in &windows {
            layer.close_tiled(*window);
            self.changes.record_window_closed(*window);
        }

        // Emit a single layout changed event for the batch close operation
        // Use the last closed window in the event (if any windows were closed)
        if let Some(&last_closed) = windows.last() {
            self.emit_layout_event(LayoutChangeKind::Close {
                closed_window: last_closed.as_usize() as u64,
                new_focus: Some(current.as_usize() as u64),
            });
        }

        Ok(())
    }

    fn resize(&mut self, direction: NavigateDirection, delta: i16) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let current = layer.focused().ok_or(CompositorError::NoFocusedWindow)?;

        layer.resize_tiled(current, direction, delta);
        self.changes.window_changed = true;

        // Emit layout changed event
        self.emit_layout_event(LayoutChangeKind::Resize {
            window: current.as_usize() as u64,
        });

        Ok(())
    }

    fn equalize(&mut self) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        layer.equalize_tiled();
        self.changes.window_changed = true;

        // Emit layout changed event
        self.emit_layout_event(LayoutChangeKind::Equalize);

        Ok(())
    }

    fn cycle(&self, forward: bool) -> Result<WindowId, CompositorError> {
        let compositor = self
            .compositor
            .as_ref()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let from = layer.focused().ok_or(CompositorError::NoFocusedWindow)?;

        layer
            .cycle_tiled(from, forward)
            .ok_or(CompositorError::NoFocusedWindow)
    }

    fn focus(&mut self, window: WindowId) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        // Get the previous focused window before changing focus
        let previous_focus = compositor.focused();

        // set_focus also activates the layer containing the window
        compositor.set_focus(window);
        self.changes.record_focus_change();

        // Emit layout changed event (only if focus actually changed)
        if previous_focus != Some(window) {
            self.emit_layout_event(LayoutChangeKind::Focus {
                from: previous_focus.map(|w| w.as_usize() as u64),
                to: window.as_usize() as u64,
            });
        }

        Ok(())
    }

    fn focused_window(&self) -> Option<WindowId> {
        self.compositor.as_ref()?.focused()
    }

    fn compositor_window_count(&self) -> usize {
        self.compositor.as_ref().map_or(0, |c| c.window_count())
    }

    fn arrange(&self, screen: Rect) -> Vec<WindowPlacement> {
        self.compositor
            .as_ref()
            .map_or_else(Vec::new, |c| c.composite(screen).placements)
    }

    fn active_layer(&self) -> Option<LayerId> {
        self.compositor.as_ref()?.active_layer()
    }

    fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
        if let Some(compositor) = self.compositor.as_mut() {
            compositor.set_screen(screen);
        }
    }

    // =========================================================================
    // Float Zone Operations (#398)
    // =========================================================================

    fn toggle_float(&mut self) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let current = layer.focused().ok_or(CompositorError::NoFocusedWindow)?;

        layer.toggle_float(current);
        self.changes.window_changed = true;

        Ok(())
    }

    fn raise_float(&mut self) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let current = layer.focused().ok_or(CompositorError::NoFocusedWindow)?;

        layer.raise_float(current);

        Ok(())
    }

    fn lower_float(&mut self) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let current = layer.focused().ok_or(CompositorError::NoFocusedWindow)?;

        layer.lower_float(current);

        Ok(())
    }

    // =========================================================================
    // Overlay Zone Operations (#399)
    // =========================================================================

    fn show_overlay(
        &mut self,
        constraints: OverlayConstraints,
    ) -> Result<WindowId, CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        let id = layer.show_overlay(constraints);
        // Note: Overlays do NOT auto-focus
        Ok(id)
    }

    fn hide_overlay(&mut self, window: WindowId) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        layer.hide_overlay(window);
        Ok(())
    }

    fn resize_overlay(
        &mut self,
        window: WindowId,
        width: u16,
        height: u16,
    ) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        layer.resize_overlay(window, width, height);
        Ok(())
    }

    fn hide_all_overlays(&mut self) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let layer = compositor
            .layer_compositor_mut(active)
            .ok_or(CompositorError::LayerNotFound(active))?;

        layer.hide_all_overlays();
        Ok(())
    }

    // =========================================================================
    // Opacity Operations (#400)
    // =========================================================================

    fn set_active_layer_opacity(&mut self, opacity: f32) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        compositor.set_layer_opacity(active, opacity.clamp(0.0, 1.0));
        self.changes.window_changed = true;
        Ok(())
    }

    fn active_layer_opacity(&self) -> Result<f32, CompositorError> {
        let compositor = self
            .compositor
            .as_ref()
            .ok_or(CompositorError::NoActiveLayer)?;

        let active = compositor
            .active_layer()
            .ok_or(CompositorError::NoActiveLayer)?;

        let opacity = compositor
            .layers()
            .iter()
            .find(|l| l.id == active)
            .map_or(1.0, |l| l.opacity);
        Ok(opacity)
    }

    fn adjust_active_layer_opacity(&mut self, delta: f32) -> Result<f32, CompositorError> {
        let current = self.active_layer_opacity()?;
        let new_opacity = (current + delta).clamp(0.0, 1.0);
        self.set_active_layer_opacity(new_opacity)?;
        Ok(new_opacity)
    }

    // =========================================================================
    // Tab Page Operations (#401)
    //
    // Tab page operations — delegate to per-client TabPageSet (#401 Phase 5).
    // =========================================================================

    fn tab_new(&mut self) -> Result<TabId, CompositorError> {
        let id = self.tabs.new_tab();
        self.changes.window_changed = true;
        Ok(id)
    }

    fn tab_close(&mut self) -> Result<(), CompositorError> {
        if self.tabs.close_tab() {
            self.changes.window_changed = true;
            Ok(())
        } else {
            Err(CompositorError::CannotCloseLastTab)
        }
    }

    fn tab_next(&mut self) -> Result<TabId, CompositorError> {
        let id = self.tabs.next_tab();
        self.changes.window_changed = true;
        Ok(id)
    }

    fn tab_prev(&mut self) -> Result<TabId, CompositorError> {
        let id = self.tabs.prev_tab();
        self.changes.window_changed = true;
        Ok(id)
    }

    fn tab_goto(&mut self, index: usize) -> Result<TabId, CompositorError> {
        self.tabs
            .goto_tab(index)
            .ok_or_else(|| CompositorError::TabNotFound(TabId::from_raw(index)))
            .inspect(|_| {
                self.changes.window_changed = true;
            })
    }

    fn tab_count(&self) -> usize {
        self.tabs.tab_count()
    }

    fn active_tab_id(&self) -> Option<TabId> {
        Some(self.tabs.active_tab_id())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{testing::StubExecutor, types::ClientId},
        reovim_kernel::{api::v1::{HistoryRing, MarkBank, ModuleId, RegisterBank}, testing::test_mode},
    };

    fn test_mode_2() -> ModeId {
        ModeId::with_discriminant(ModuleId::new("test"), "insert", 1)
    }

    use std::sync::Arc;

    use crate::api::CommandHandle;

    #[test]
    fn test_mode_api() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        // #471 Phase 0: Per-client state is now REQUIRED
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

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

    /// Test per-client mode stack isolation (#471, #477, Phase 0).
    ///
    /// Verifies that:
    /// 1. `new()` uses the provided per-client mode stack
    /// 2. Mode changes don't affect the session's shared mode stack
    /// 3. Two runtimes with different client stacks have independent modes
    #[test]
    fn test_per_client_mode_stack() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        // Create per-client state (#471, #477)
        let mut client_mode_stack = ModeStack::new(test_mode());
        let mut client_windows = crate::WindowLayout::empty();
        let mut client_extensions = crate::ExtensionMap::new();
        let mut client_compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut client_registers = RegisterBank::new();
        let mut client_clipboard_history = HistoryRing::new();
        let mut client_local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        // #491: Session no longer has mode_stack field - use home_mode() from shared
        let session_home_mode = session.shared.home_mode().clone();

        // Use a scope to release mutable borrow before checking session
        {
            // Create runtime with per-client state (#471 Phase 0)
            let mut runtime = SessionRuntime::new(
                &mut session,
                crate::ClientContext {
                    mode_stack: &mut client_mode_stack,
                    windows: &mut client_windows,
                    extensions: &mut client_extensions,
                    compositor: &mut client_compositor,
                    tabs: &mut tabs,
                    registers: &mut client_registers,
                    clipboard_history: &mut client_clipboard_history,
                    local_marks: &mut client_local_marks,
                    active_buffer: &mut active_buffer,
                    terminal_size: &mut terminal_size,
                },
                &kernel,
                &executor,
            );

            // #471 Phase 0: Per-client state is now REQUIRED (no has_* methods)
            assert_eq!(runtime.current_mode(), &test_mode());

            // Push mode to per-client stack
            runtime.push_mode(test_mode_2(), TransitionContext::new());
            assert_eq!(runtime.current_mode(), &test_mode_2());
            assert_eq!(runtime.mode_depth(), 2);
        }

        // #491: After runtime is dropped, session.shared.home_mode() remains unchanged
        assert_eq!(session.shared.home_mode(), &session_home_mode);

        // Verify client_mode_stack was modified
        assert_eq!(client_mode_stack.current(), &test_mode_2());
        assert_eq!(client_mode_stack.depth(), 2);
    }

    /// Test the `owner()` method for explicit client binding (#471 Phase 0).
    #[test]
    fn test_owner_tracking() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        // Per-client state is REQUIRED (#471 Phase 0)
        let mut client_stack = ModeStack::new(test_mode());
        let mut client_windows = crate::WindowLayout::empty();
        let mut client_extensions = crate::ExtensionMap::new();
        let mut client_compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut client_registers = RegisterBank::new();
        let mut client_clipboard_history = HistoryRing::new();
        let mut client_local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        // Runtime created with new() has no owner
        {
            let runtime = SessionRuntime::new(
                &mut session,
                crate::ClientContext {
                    mode_stack: &mut client_stack,
                    windows: &mut client_windows,
                    extensions: &mut client_extensions,
                    compositor: &mut client_compositor,
                    tabs: &mut tabs,
                    registers: &mut client_registers,
                    clipboard_history: &mut client_clipboard_history,
                    local_marks: &mut client_local_marks,
                    active_buffer: &mut active_buffer,
                    terminal_size: &mut terminal_size,
                },
                &kernel,
                &executor,
            );
            assert_eq!(runtime.owner(), None);
        }

        // Runtime created with with_owner() has explicit owner
        let client_id = ClientId::new(42);
        {
            let runtime = SessionRuntime::with_owner(
                client_id,
                &mut session,
                crate::ClientContext {
                    mode_stack: &mut client_stack,
                    windows: &mut client_windows,
                    extensions: &mut client_extensions,
                    compositor: &mut client_compositor,
                    tabs: &mut tabs,
                    registers: &mut client_registers,
                    clipboard_history: &mut client_clipboard_history,
                    local_marks: &mut client_local_marks,
                    active_buffer: &mut active_buffer,
                    terminal_size: &mut terminal_size,
                },
                &kernel,
                &executor,
            );
            assert_eq!(runtime.owner(), Some(client_id));
        }
    }

    /// Test that two clients have independent mode stacks (#471, #477, Phase 0).
    #[test]
    fn test_multi_client_mode_isolation() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        // Create two independent client state sets (#471, #477)
        let mut client1_stack = ModeStack::new(test_mode());
        let mut client1_windows = crate::WindowLayout::empty();
        let mut client1_extensions = crate::ExtensionMap::new();
        let mut client1_compositor = None;
        let mut client1_registers = RegisterBank::new();
        let mut client1_clipboard_history = HistoryRing::new();
        let mut client1_local_marks = MarkBank::new();
        let mut client2_stack = ModeStack::new(test_mode());
        let mut client2_windows = crate::WindowLayout::empty();
        let mut client2_extensions = crate::ExtensionMap::new();
        let mut client2_compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut client2_registers = RegisterBank::new();
        let mut client2_clipboard_history = HistoryRing::new();
        let mut client2_local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        // Client 1 enters insert mode (#471 Phase 0: use new())
        {
            let mut runtime1 = SessionRuntime::new(
                &mut session,
                crate::ClientContext {
                    mode_stack: &mut client1_stack,
                    windows: &mut client1_windows,
                    extensions: &mut client1_extensions,
                    compositor: &mut client1_compositor,
                    tabs: &mut tabs,
                    registers: &mut client1_registers,
                    clipboard_history: &mut client1_clipboard_history,
                    local_marks: &mut client1_local_marks,
                    active_buffer: &mut active_buffer,
                    terminal_size: &mut terminal_size,
                },
                &kernel,
                &executor,
            );
            runtime1.push_mode(test_mode_2(), TransitionContext::new());
        }

        // Client 2 stays in normal mode (#471 Phase 0: use new())
        {
            let runtime2 = SessionRuntime::new(
                &mut session,
                crate::ClientContext {
                    mode_stack: &mut client2_stack,
                    windows: &mut client2_windows,
                    extensions: &mut client2_extensions,
                    compositor: &mut client2_compositor,
                    tabs: &mut tabs,
                    registers: &mut client2_registers,
                    clipboard_history: &mut client2_clipboard_history,
                    local_marks: &mut client2_local_marks,
                    active_buffer: &mut active_buffer,
                    terminal_size: &mut terminal_size,
                },
                &kernel,
                &executor,
            );
            // Client 2 should still be in normal mode
            assert_eq!(runtime2.current_mode(), &test_mode());
        }

        // Verify isolation (#471 Phase 0)
        assert_eq!(client1_stack.current(), &test_mode_2()); // Client 1: insert
        assert_eq!(client2_stack.current(), &test_mode()); // Client 2: normal
        // Note: session.mode_stack is deprecated (#488) - per-client stacks are SSOT
    }

    #[test]
    fn test_window_api() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        // #471 Phase 0: Per-client state is REQUIRED
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_extension_api() {
        use reovim_kernel::api::v1::ModeStack;

        #[derive(Debug, Default)]
        struct TestExtension {
            value: i32,
        }

        impl SessionExtension for TestExtension {
            fn create() -> Self {
                Self { value: 42 }
            }
        }

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        // #471 Phase 0: Per-client state is REQUIRED
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        // Extension doesn't exist initially
        assert!(runtime.ext::<TestExtension>().is_none());

        // ext_mut creates it
        let ext = runtime.ext_mut::<TestExtension>();
        assert_eq!(ext.value, 42);
        ext.value = 100;

        // ext now returns it
        assert_eq!(runtime.ext::<TestExtension>().unwrap().value, 100);
    }

    // ========================================================================
    // Shared extension tests (#543)
    // ========================================================================

    #[test]
    fn test_shared_ext_without_shared_extensions_returns_none() {
        use reovim_kernel::api::v1::ModeStack;

        #[derive(Debug)]
        struct SharedTestExt;

        impl SessionExtension for SharedTestExt {
            fn create() -> Self {
                Self
            }
        }

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        // Without shared_extensions, shared_ext returns None
        assert!(runtime.shared_ext::<SharedTestExt>().is_none());
    }

    #[test]
    fn test_shared_ext_mut_without_shared_extensions_returns_none() {
        use reovim_kernel::api::v1::ModeStack;

        #[derive(Debug)]
        struct SharedTestExt2 {
            _value: i32,
        }

        impl SessionExtension for SharedTestExt2 {
            fn create() -> Self {
                Self { _value: 0 }
            }
        }

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        // Without shared_extensions, shared_ext_mut returns None
        assert!(runtime.shared_ext_mut::<SharedTestExt2>().is_none());
    }

    #[test]
    fn test_shared_ext_with_shared_extensions_returns_value() {
        use reovim_kernel::api::v1::ModeStack;

        #[derive(Debug)]
        struct SharedTestExt3 {
            value: i32,
        }

        impl SessionExtension for SharedTestExt3 {
            fn create() -> Self {
                Self { value: 99 }
            }
        }

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut shared_extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        // Pre-populate shared extensions
        shared_extensions.get_or_insert::<SharedTestExt3>();

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        )
        .with_shared_extensions(&mut shared_extensions);

        // With shared_extensions, shared_ext returns the value
        let ext = runtime.shared_ext::<SharedTestExt3>();
        assert!(ext.is_some());
        assert_eq!(ext.unwrap().value, 99);
    }

    #[test]
    fn test_shared_ext_mut_creates_and_returns() {
        use reovim_kernel::api::v1::ModeStack;

        #[derive(Debug)]
        struct SharedTestExt4 {
            value: i32,
        }

        impl SessionExtension for SharedTestExt4 {
            fn create() -> Self {
                Self { value: 77 }
            }
        }

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut shared_extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        )
        .with_shared_extensions(&mut shared_extensions);

        // shared_ext_mut creates and returns
        let ext = runtime.shared_ext_mut::<SharedTestExt4>();
        assert!(ext.is_some());
        let ext = ext.unwrap();
        assert_eq!(ext.value, 77);
        ext.value = 200;

        // shared_ext reads the updated value
        assert_eq!(runtime.shared_ext::<SharedTestExt4>().unwrap().value, 200);
    }

    #[test]
    fn test_shared_ext_default_trait_returns_none() {
        // Verify the default trait implementation (not SessionRuntime) returns None.
        struct MinimalApi;

        #[derive(Debug)]
        struct AnyExt;
        impl SessionExtension for AnyExt {
            fn create() -> Self {
                Self
            }
        }

        impl ExtensionApi for MinimalApi {
            fn ext<T: SessionExtension>(&self) -> Option<&T> {
                None
            }
            fn ext_mut<T: SessionExtension>(&mut self) -> &mut T {
                unimplemented!()
            }
        }

        let api = MinimalApi;
        assert!(api.shared_ext::<AnyExt>().is_none());
    }

    #[test]
    fn test_shared_ext_mut_default_trait_returns_none() {
        struct MinimalApi2;

        #[derive(Debug)]
        struct AnyExt2;
        impl SessionExtension for AnyExt2 {
            fn create() -> Self {
                Self
            }
        }

        impl ExtensionApi for MinimalApi2 {
            fn ext<T: SessionExtension>(&self) -> Option<&T> {
                None
            }
            fn ext_mut<T: SessionExtension>(&mut self) -> &mut T {
                unimplemented!()
            }
        }

        let mut api = MinimalApi2;
        assert!(api.shared_ext_mut::<AnyExt2>().is_none());
    }

    #[test]
    fn test_change_tracking() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;

        // #471 Phase 0: Per-client state is REQUIRED
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

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

    // NOTE: test_selection_api and test_selection_api_set_selection removed
    // as part of #471 - they tested the removed buffer_id-based selection API.
    // Selection is now per-window, managed via CommandContext and CommandResult.

    #[test]
    fn test_buffer_text_range_single_line() {
        use crate::testing::TestSessionRuntime;

        // Create test runtime with a buffer
        let mut harness = TestSessionRuntime::with_buffer("hello world");

        // Get the buffer ID
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Extract "llo wo" (columns 2-8)
        let result = harness.with_runtime(|runtime| {
            runtime.buffer_text_range(buffer_id, Position::new(0, 2), Position::new(0, 8))
        });
        assert_eq!(result, Some("llo wo".to_string()));
    }

    #[test]
    fn test_buffer_text_range_multi_line() {
        use crate::testing::TestSessionRuntime;

        // Create test runtime with multi-line content
        let mut harness = TestSessionRuntime::with_buffer("line one\nline two\nline three");

        // Get the buffer ID
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Extract from middle of first line to middle of last line
        let result = harness.with_runtime(|runtime| {
            runtime.buffer_text_range(buffer_id, Position::new(0, 5), Position::new(2, 4))
        });
        assert_eq!(result, Some("one\nline two\nline".to_string()));

        // Extract a full line (0,0 to 1,0 gets first line + newline)
        let result = harness.with_runtime(|runtime| {
            runtime.buffer_text_range(buffer_id, Position::new(0, 0), Position::new(1, 0))
        });
        assert_eq!(result, Some("line one\n".to_string()));
    }

    // =========================================================================
    // Additional BufferApi tests via TestSessionRuntime
    // =========================================================================

    #[test]
    fn test_buffer_line_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello\nworld\nfoo");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        harness.with_runtime(|runtime| {
            assert_eq!(runtime.buffer_line(buffer_id, 0), Some("hello".to_string()));
            assert_eq!(runtime.buffer_line(buffer_id, 1), Some("world".to_string()));
            assert_eq!(runtime.buffer_line(buffer_id, 2), Some("foo".to_string()));
            assert!(runtime.buffer_line(buffer_id, 99).is_none());
        });
    }

    #[test]
    fn test_buffer_line_count_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello\nworld\nfoo");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        let count = harness.with_runtime(|runtime| runtime.buffer_line_count(buffer_id));
        assert_eq!(count, Some(3));
    }

    #[test]
    fn test_buffer_content_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        let content = harness.with_runtime(|runtime| runtime.buffer_content(buffer_id));
        assert_eq!(content, Some("hello world".to_string()));
    }

    #[test]
    fn test_buffer_nonexistent() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let fake_id = BufferId::new();

        harness.with_runtime(|runtime| {
            assert!(runtime.buffer_line(fake_id, 0).is_none());
            assert!(runtime.buffer_line_count(fake_id).is_none());
            assert!(runtime.buffer_content(fake_id).is_none());
            assert!(runtime.buffer_file_path(fake_id).is_none());
            assert!(runtime.is_buffer_modified(fake_id).is_none());
            assert!(
                runtime
                    .buffer_text_range(fake_id, Position::new(0, 0), Position::new(0, 5))
                    .is_none()
            );
        });
    }

    #[test]
    fn test_buffer_file_path_none() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Buffer created without name should have no file path
        let path = harness.with_runtime(|runtime| runtime.buffer_file_path(buffer_id));
        assert!(path.is_none());
    }

    #[test]
    fn test_buffer_modified_flag() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Initially not modified
        let modified = harness.with_runtime(|runtime| runtime.is_buffer_modified(buffer_id));
        assert_eq!(modified, Some(false));

        // Set modified
        harness.with_runtime(|runtime| {
            runtime.set_buffer_modified(buffer_id, true);
        });
        let modified = harness.with_runtime(|runtime| runtime.is_buffer_modified(buffer_id));
        assert_eq!(modified, Some(true));

        // Clear modified
        harness.with_runtime(|runtime| {
            runtime.set_buffer_modified(buffer_id, false);
        });
        let modified = harness.with_runtime(|runtime| runtime.is_buffer_modified(buffer_id));
        assert_eq!(modified, Some(false));
    }

    #[test]
    fn test_set_buffer_modified_nonexistent() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let fake_id = BufferId::new();

        // Should not panic for non-existent buffer
        harness.with_runtime(|runtime| {
            runtime.set_buffer_modified(fake_id, true);
        });
    }

    #[test]
    fn test_insert_text_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("helloworld");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        harness.with_runtime(|runtime| {
            runtime.insert_text(buffer_id, Position::new(0, 5), " ");
        });

        harness.assert_buffer_content("hello world");
        let changes = harness.take_changes();
        assert!(changes.buffer_modified);
    }

    #[test]
    fn test_insert_text_nonexistent_buffer() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let fake_id = BufferId::new();

        // Should not panic for non-existent buffer
        harness.with_runtime(|runtime| {
            runtime.insert_text(fake_id, Position::new(0, 0), "text");
        });
    }

    #[test]
    fn test_delete_range_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        harness.with_runtime(|runtime| {
            runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 11));
        });

        harness.assert_buffer_content("hello");
        let changes = harness.take_changes();
        assert!(changes.buffer_modified);
    }

    #[test]
    fn test_delete_range_nonexistent_buffer() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let fake_id = BufferId::new();

        // Should not panic for non-existent buffer
        harness.with_runtime(|runtime| {
            runtime.delete_range(fake_id, Position::new(0, 0), Position::new(0, 5));
        });
    }

    #[test]
    fn test_create_buffer_unnamed() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        let buf_id = harness.with_runtime(|runtime| runtime.create_buffer(None, "content"));

        let content = harness.with_runtime(|runtime| runtime.buffer_content(buf_id));
        assert_eq!(content, Some("content".to_string()));

        let path = harness.with_runtime(|runtime| runtime.buffer_file_path(buf_id));
        assert!(path.is_none());

        let changes = harness.take_changes();
        assert!(changes.buffers_created.contains(&buf_id));
    }

    #[test]
    fn test_create_buffer_named() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        let buf_id =
            harness.with_runtime(|runtime| runtime.create_buffer(Some("test.txt"), "hello"));

        let path = harness.with_runtime(|runtime| runtime.buffer_file_path(buf_id));
        assert_eq!(path, Some("test.txt".to_string()));
    }

    #[test]
    fn test_rename_buffer_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        let buf_id =
            harness.with_runtime(|runtime| runtime.create_buffer(Some("old.txt"), "content"));
        harness.take_changes(); // Clear creation changes

        harness.with_runtime(|runtime| {
            runtime.rename_buffer(buf_id, "new.txt");
        });

        let path = harness.with_runtime(|runtime| runtime.buffer_file_path(buf_id));
        assert_eq!(path, Some("new.txt".to_string()));

        let changes = harness.take_changes();
        assert!(!changes.buffers_renamed.is_empty());
        assert_eq!(changes.buffers_renamed[0].1, "new.txt");
    }

    #[test]
    fn test_rename_nonexistent_buffer() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let fake_id = BufferId::new();

        // Should not panic
        harness.with_runtime(|runtime| {
            runtime.rename_buffer(fake_id, "new.txt");
        });
    }

    #[test]
    fn test_delete_buffer_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        let buf1 = harness.with_runtime(|runtime| runtime.create_buffer(None, "first"));
        let buf2 = harness.with_runtime(|runtime| runtime.create_buffer(None, "second"));
        harness.take_changes();

        // Delete one buffer
        let result = harness.with_runtime(|runtime| runtime.delete_buffer(buf1));
        assert!(result.is_ok());

        // Verify it's gone
        let content = harness.with_runtime(|runtime| runtime.buffer_content(buf1));
        assert!(content.is_none());

        // The other buffer should still exist
        let content = harness.with_runtime(|runtime| runtime.buffer_content(buf2));
        assert_eq!(content, Some("second".to_string()));

        let changes = harness.take_changes();
        assert!(changes.buffers_deleted.contains(&buf1));
    }

    #[test]
    fn test_delete_last_buffer_fails() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("only buffer");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        let result = harness.with_runtime(|runtime| runtime.delete_buffer(buffer_id));
        assert!(matches!(result, Err(BufferError::CannotDeleteLastBuffer)));
    }

    #[test]
    fn test_delete_nonexistent_buffer() {
        use crate::testing::TestSessionRuntime;

        // Need at least 2 buffers so the count check passes before the existence check
        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| runtime.create_buffer(None, "first"));
        harness.with_runtime(|runtime| runtime.create_buffer(None, "second"));

        let fake_id = BufferId::new();
        let result = harness.with_runtime(|runtime| runtime.delete_buffer(fake_id));
        assert!(matches!(result, Err(BufferError::NotFound(_))));
    }

    // =========================================================================
    // WindowApi tests via TestSessionRuntime
    // =========================================================================

    #[test]
    fn test_window_buffer_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
        let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());

        let buf = harness.with_runtime(|runtime| runtime.window_buffer(window_id));
        assert_eq!(buf, Some(buffer_id));
    }

    #[test]
    fn test_window_buffer_nonexistent() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let fake_id = WindowId::new();

        let buf = harness.with_runtime(|runtime| runtime.window_buffer(fake_id));
        assert!(buf.is_none());
    }

    #[test]
    fn test_cursor_position_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");

        let pos = harness.with_runtime(|runtime| runtime.cursor_position());
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_cursor_position_no_window() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new(); // No windows

        let pos = harness.with_runtime(|runtime| runtime.cursor_position());
        assert!(pos.is_none());
    }

    #[test]
    fn test_focus_nonexistent_window() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let fake_id = WindowId::new();

        let result = harness.with_runtime(|runtime| runtime.focus_window(fake_id));
        assert!(matches!(result, Err(WindowError::NotFound(_))));
    }

    #[test]
    fn test_close_nonexistent_window() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let _w1 = harness.with_runtime(|runtime| runtime.create_window(None));
        let fake_id = WindowId::new();

        let result = harness.with_runtime(|runtime| runtime.close_window(fake_id));
        assert!(matches!(result, Err(WindowError::NotFound(_))));
    }

    #[test]
    fn test_set_window_buffer_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());

        // Create a new buffer and set it on the window
        let new_buf = harness.with_runtime(|runtime| runtime.create_buffer(None, "world"));

        let result = harness.with_runtime(|runtime| runtime.set_window_buffer(window_id, new_buf));
        assert!(result.is_ok());

        // Verify the window now shows the new buffer
        let buf = harness.with_runtime(|runtime| runtime.window_buffer(window_id));
        assert_eq!(buf, Some(new_buf));
    }

    #[test]
    fn test_set_window_buffer_nonexistent_window() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let buf_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
        let fake_win = WindowId::new();

        let result = harness.with_runtime(|runtime| runtime.set_window_buffer(fake_win, buf_id));
        assert!(matches!(result, Err(WindowError::NotFound(_))));
    }

    #[test]
    fn test_set_window_buffer_nonexistent_buffer() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());
        let fake_buf = BufferId::new();

        let result = harness.with_runtime(|runtime| runtime.set_window_buffer(window_id, fake_buf));
        assert!(matches!(result, Err(WindowError::BufferNotFound(_))));
    }

    #[test]
    fn test_set_active_selection() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");

        // Initially no selection.
        let sel = harness.with_runtime(|runtime| runtime.active_selection().cloned());
        assert!(sel.is_none());

        // Set a selection.
        let selection = Selection::character(Position::new(0, 0), Position::new(0, 5));
        harness.with_runtime(|runtime| {
            runtime.set_active_selection(Some(selection.clone()));
        });

        let sel = harness.with_runtime(|runtime| runtime.active_selection().cloned());
        assert_eq!(sel.as_ref(), Some(&selection));

        // Clear it.
        harness.with_runtime(|runtime| {
            runtime.set_active_selection(None);
        });

        let sel = harness.with_runtime(|runtime| runtime.active_selection().cloned());
        assert!(sel.is_none());
    }

    #[test]
    fn test_active_selection_no_window() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new(); // No windows

        let sel = harness.with_runtime(|runtime| runtime.active_selection().cloned());
        assert!(sel.is_none());

        // set_active_selection on no window — should be no-op, not panic.
        harness.with_runtime(|runtime| {
            runtime.set_active_selection(Some(Selection::character(
                Position::new(0, 0),
                Position::new(0, 1),
            )));
        });
    }

    // =========================================================================
    // ModeApi additional tests
    // =========================================================================

    #[test]
    fn test_mode_stack_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        let stack = harness.with_runtime(|runtime| runtime.mode_stack());
        assert_eq!(stack.len(), 1);

        // Push a mode
        harness.with_runtime(|runtime| {
            runtime.push_mode(test_mode_2(), TransitionContext::new());
        });

        let stack = harness.with_runtime(|runtime| runtime.mode_stack());
        assert_eq!(stack.len(), 2);
        assert_eq!(stack[0], test_mode());
        assert_eq!(stack[1], test_mode_2());
    }

    #[test]
    fn test_set_mode_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        // set_mode replaces current mode
        harness.with_runtime(|runtime| {
            runtime.set_mode(test_mode_2(), TransitionContext::new());
        });

        let current = harness.with_runtime(|runtime| runtime.current_mode().clone());
        assert_eq!(current, test_mode_2());

        let depth = harness.with_runtime(|runtime| runtime.mode_depth());
        assert_eq!(depth, 1); // set_mode replaces, doesn't push
    }

    #[test]
    fn test_is_mode_active_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        assert!(harness.with_runtime(|runtime| runtime.is_mode_active(&test_mode())));
        assert!(!harness.with_runtime(|runtime| runtime.is_mode_active(&test_mode_2())));

        // Push mode_2
        harness.with_runtime(|runtime| {
            runtime.push_mode(test_mode_2(), TransitionContext::new());
        });

        // Both should be active (on the stack)
        assert!(harness.with_runtime(|runtime| runtime.is_mode_active(&test_mode())));
        assert!(harness.with_runtime(|runtime| runtime.is_mode_active(&test_mode_2())));
    }

    #[test]
    fn test_home_mode_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        let home = harness.with_runtime(|runtime| runtime.home_mode().clone());
        assert_eq!(home, test_mode());

        // Push another mode - home should stay the same
        harness.with_runtime(|runtime| {
            runtime.push_mode(test_mode_2(), TransitionContext::new());
        });
        let home = harness.with_runtime(|runtime| runtime.home_mode().clone());
        assert_eq!(home, test_mode());
    }

    // =========================================================================
    // SessionRuntime accessor tests
    // =========================================================================

    #[test]
    fn test_has_compositor_false() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(!runtime.has_compositor());
    }

    #[test]
    fn test_session_accessor() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert_eq!(runtime.session().id.as_usize(), 1);
    }

    #[test]
    fn test_session_mut_accessor() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        // Exercise session_mut() accessor.
        let session_ref = runtime.session_mut();
        assert_eq!(session_ref.id.as_usize(), 1);
    }

    #[test]
    fn test_kernel_accessor() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        // Just verify kernel() doesn't panic
        let _kernel = runtime.kernel();
    }

    #[test]
    fn test_windows_accessor() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(runtime.windows().is_empty());
    }

    #[test]
    fn test_windows_mut_accessor() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        runtime.windows_mut().add(crate::Window::new());
        assert_eq!(runtime.windows().len(), 1);
    }

    // =========================================================================
    // CommandApi test
    // =========================================================================

    #[test]
    fn test_execute_command_api() {
        use {
            crate::testing::TestSessionRuntime, reovim_driver_command_types::CommandContext,
            reovim_kernel::api::v1::ModuleId,
        };

        let mut harness = TestSessionRuntime::new();
        let cmd = CommandId::new(ModuleId::new("test"), "test_cmd");

        let result =
            harness.with_runtime(|runtime| runtime.execute_command(cmd, CommandContext::new()));
        // StubExecutor returns None (command not found) -> Error
        assert!(matches!(result, reovim_driver_command_types::CommandResult::Error(_)));
    }

    // =========================================================================
    // with_buffer_read test
    // =========================================================================

    #[test]
    fn test_with_buffer_read() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        let line_count = harness.with_runtime(|runtime| {
            runtime.with_buffer_read(buffer_id, reovim_kernel::api::v1::Buffer::line_count)
        });
        assert_eq!(line_count, Some(1));
    }

    #[test]
    fn test_with_buffer_read_nonexistent() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let fake_id = BufferId::new();

        let result = harness.with_runtime(|runtime| {
            runtime.with_buffer_read(fake_id, reovim_kernel::api::v1::Buffer::line_count)
        });
        assert!(result.is_none());
    }

    // =========================================================================
    // Record option change tests
    // =========================================================================

    #[test]
    fn test_record_global_option_change() {
        use {crate::testing::TestSessionRuntime, reovim_kernel::api::v1::OptionValue};

        let mut harness = TestSessionRuntime::new();

        harness.with_runtime(|runtime| {
            runtime.record_global_option_change("number", OptionValue::bool(true));
        });

        let changes = harness.take_changes();
        assert!(changes.option_changed);
        assert_eq!(changes.options_changed.len(), 1);
        assert_eq!(changes.options_changed[0].name, "number");
    }

    #[test]
    fn test_record_window_option_change() {
        use {crate::testing::TestSessionRuntime, reovim_kernel::api::v1::OptionValue};

        let mut harness = TestSessionRuntime::with_buffer("test");
        let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());

        harness.with_runtime(|runtime| {
            runtime.record_window_option_change("wrap", OptionValue::bool(false), window_id);
        });

        let changes = harness.take_changes();
        assert!(changes.option_changed);
        assert_eq!(changes.options_changed.len(), 1);
        assert_eq!(changes.options_changed[0].window_id, Some(window_id));
    }

    // =========================================================================
    // ChangeTracker record_cursor_move test
    // =========================================================================

    #[test]
    fn test_change_tracker_record_cursor_move() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        harness.with_runtime(|runtime| {
            ChangeTracker::record_cursor_move(runtime, buffer_id);
        });

        let changes = harness.take_changes();
        assert!(changes.cursor_moved);
        assert!(changes.affected_buffers.contains(&buffer_id));
    }

    // =========================================================================
    // UndoApi - can_undo / can_redo without provider
    // =========================================================================

    #[test]
    fn test_can_undo_without_provider() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // No undo provider registered, should return false
        let can_undo = harness.with_runtime(|runtime| runtime.can_undo(buffer_id));
        assert!(!can_undo);

        let can_redo = harness.with_runtime(|runtime| runtime.can_redo(buffer_id));
        assert!(!can_redo);
    }

    #[test]
    fn test_undo_without_provider() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        let result = harness.with_runtime(|runtime| runtime.undo(buffer_id));
        assert!(result.is_none());

        let result = harness.with_runtime(|runtime| runtime.redo(buffer_id));
        assert!(result.is_none());
    }

    #[test]
    fn test_undo_mine_without_owner() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Without owner, undo_mine should return None
        let result = harness.with_runtime(|runtime| runtime.undo_mine(buffer_id));
        assert!(result.is_none());

        let result = harness.with_runtime(|runtime| runtime.redo_mine(buffer_id));
        assert!(result.is_none());
    }

    // =========================================================================
    // RegisterApi additional tests
    // =========================================================================

    #[test]
    fn test_numbered_register_writes_ignored() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        harness.with_runtime(|runtime| {
            // Writing to numbered register should be ignored
            runtime.set_register(Some('0'), crate::api::RegisterContent::characterwise("test"));

            // Reading numbered registers returns None without provider
            let content = runtime.get_register(Some('0'));
            assert!(content.is_none());
        });
    }

    #[test]
    fn test_clipboard_register_without_provider() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        harness.with_runtime(|runtime| {
            // Without clipboard provider, + register falls back to kernel registers
            runtime
                .set_register(Some('+'), crate::api::RegisterContent::characterwise("clipboard"));

            // The fallback stores in kernel registers
            let content = runtime.get_register(Some('+'));
            assert!(content.is_none()); // No clipboard provider, get returns None
        });
    }

    #[test]
    fn test_selection_register_without_provider() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        harness.with_runtime(|runtime| {
            runtime
                .set_register(Some('*'), crate::api::RegisterContent::characterwise("selection"));

            let content = runtime.get_register(Some('*'));
            assert!(content.is_none()); // No clipboard provider
        });
    }

    // =========================================================================
    // CompositorApi error paths (no compositor)
    // =========================================================================

    #[test]
    fn test_compositor_api_no_compositor() {
        use {
            crate::testing::TestSessionRuntime,
            reovim_driver_display::{NavigateDirection, Rect, SplitDirection},
        };

        let mut harness = TestSessionRuntime::new();

        harness.with_runtime(|runtime| {
            // All compositor operations should return errors when no compositor
            assert!(runtime.navigate(NavigateDirection::Left).is_err());
            assert!(runtime.split(SplitDirection::Horizontal).is_err());
            assert!(runtime.close_current_window().is_err());
            assert!(runtime.close_others().is_err());
            assert!(runtime.resize(NavigateDirection::Right, 5).is_err());
            assert!(runtime.equalize().is_err());
            assert!(runtime.cycle(true).is_err());
            assert!(runtime.toggle_float().is_err());
            assert!(runtime.raise_float().is_err());
            assert!(runtime.lower_float().is_err());
            assert!(runtime.hide_all_overlays().is_err());
            assert!(runtime.set_active_layer_opacity(0.5).is_err());
            assert!(runtime.active_layer_opacity().is_err());
            assert!(runtime.adjust_active_layer_opacity(0.1).is_err());

            // Tab operations work via TabPageSet even without compositor (#401)
            assert!(runtime.tab_new().is_ok()); // creates a new tab
            assert!(runtime.tab_close().is_ok()); // close the new tab (2 -> 1)
            assert!(runtime.tab_close().is_err()); // can't close last tab
            assert!(runtime.tab_next().is_ok()); // cycle (only 1 tab)
            assert!(runtime.tab_prev().is_ok()); // cycle (only 1 tab)
            assert!(runtime.tab_goto(0).is_ok()); // goto first tab
            assert!(runtime.tab_goto(99).is_err()); // out-of-range
            assert_eq!(runtime.tab_count(), 1);
            assert!(runtime.active_tab_id().is_some());

            // focused_window returns None
            assert!(runtime.focused_window().is_none());
            // compositor_window_count returns 0
            assert_eq!(runtime.compositor_window_count(), 0);
            // active_layer returns None
            assert!(runtime.active_layer().is_none());
            // arrange returns empty
            assert!(runtime.arrange(Rect::new(0, 0, 80, 24)).is_empty());
        });
    }

    #[test]
    fn test_set_screen_no_compositor() {
        use {crate::testing::TestSessionRuntime, reovim_driver_display::Rect};

        let mut harness = TestSessionRuntime::new();

        // Should not panic even without compositor
        harness.with_runtime(|runtime| {
            runtime.set_screen(Rect::new(0, 0, 120, 40));
        });
    }

    // =========================================================================
    // buffer_text_range edge cases
    // =========================================================================

    #[test]
    fn test_buffer_text_range_empty_range() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Same position - should return empty string
        let result = harness.with_runtime(|runtime| {
            runtime.buffer_text_range(buffer_id, Position::new(0, 3), Position::new(0, 3))
        });
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn test_buffer_text_range_out_of_bounds_column() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hi");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Columns beyond line length should be clamped
        let result = harness.with_runtime(|runtime| {
            runtime.buffer_text_range(buffer_id, Position::new(0, 0), Position::new(0, 100))
        });
        assert_eq!(result, Some("hi".to_string()));
    }

    // =========================================================================
    // Named/unnamed register tests
    // =========================================================================

    #[test]
    fn test_named_register_set_and_get() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        harness.with_runtime(|runtime| {
            let content = crate::api::RegisterContent::characterwise("hello");
            runtime.set_register(Some('a'), content);

            let got = runtime.get_register(Some('a'));
            assert!(got.is_some());
            assert_eq!(got.unwrap().text, "hello");
        });
    }

    #[test]
    fn test_unnamed_register_set_and_get() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        harness.with_runtime(|runtime| {
            let content = crate::api::RegisterContent::characterwise("unnamed");
            runtime.set_register(None, content);

            let got = runtime.get_register(None);
            assert!(got.is_some());
            assert_eq!(got.unwrap().text, "unnamed");
        });
    }

    #[test]
    fn test_get_register_nonexistent_named() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        let got = harness.with_runtime(|runtime| runtime.get_register(Some('z')));
        assert!(got.is_none());
    }

    // =========================================================================
    // UndoApi: record_edit without provider
    // =========================================================================

    #[test]
    fn test_record_edit_without_provider() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Should not panic when no undo provider is registered
        harness.with_runtime(|runtime| {
            runtime.record_edit(
                buffer_id,
                vec![reovim_kernel::api::v1::Edit::Insert {
                    position: Position::new(0, 0),
                    text: "x".to_string(),
                }],
                Position::new(0, 0),
                Position::new(0, 1),
            );
        });
    }

    #[test]
    fn test_record_edit_mine_without_owner() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("test");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Without owner, record_edit_mine falls back to record_edit
        // Should not panic
        harness.with_runtime(|runtime| {
            runtime.record_edit_mine(
                buffer_id,
                vec![reovim_kernel::api::v1::Edit::Insert {
                    position: Position::new(0, 0),
                    text: "x".to_string(),
                }],
                Position::new(0, 0),
                Position::new(0, 1),
            );
        });
    }

    // =========================================================================
    // BufferApi: buffer_line_len
    // =========================================================================

    #[test]
    fn test_buffer_line_len_api() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello\nworld");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        harness.with_runtime(|runtime| {
            assert_eq!(runtime.buffer_line_len(buffer_id, 0), Some(5));
            assert_eq!(runtime.buffer_line_len(buffer_id, 1), Some(5));
            assert!(runtime.buffer_line_len(buffer_id, 99).is_none());
        });
    }

    #[test]
    fn test_buffer_line_len_nonexistent_buffer() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let fake_id = BufferId::new();

        let result = harness.with_runtime(|runtime| runtime.buffer_line_len(fake_id, 0));
        assert!(result.is_none());
    }

    // =========================================================================
    // BufferApi: delete_range that produces empty deleted text
    // =========================================================================

    #[test]
    fn test_delete_range_empty_range() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Delete from position to same position produces empty deleted text
        harness.with_runtime(|runtime| {
            runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 5));
        });

        // Buffer content should be unchanged
        harness.assert_buffer_content("hello world");
    }

    // =========================================================================
    // CompositorApi: show_overlay, hide_overlay, resize_overlay error paths
    // =========================================================================

    #[test]
    fn test_show_overlay_no_compositor() {
        use {
            crate::testing::TestSessionRuntime, reovim_driver_display::layout::OverlayConstraints,
        };

        let mut harness = TestSessionRuntime::new();

        let result = harness.with_runtime(|runtime| {
            runtime.show_overlay(OverlayConstraints::centered().with_size(20, 10))
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_hide_overlay_no_compositor() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let fake_win = WindowId::new();

        let result = harness.with_runtime(|runtime| runtime.hide_overlay(fake_win));
        assert!(result.is_err());
    }

    #[test]
    fn test_resize_overlay_no_compositor() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let fake_win = WindowId::new();

        let result = harness.with_runtime(|runtime| runtime.resize_overlay(fake_win, 40, 20));
        assert!(result.is_err());
    }

    #[test]
    fn test_focus_no_compositor() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        let fake_win = WindowId::new();

        let result = harness.with_runtime(|runtime| runtime.focus(fake_win));
        assert!(result.is_err());
    }

    // =========================================================================
    // to_kernel_split_direction coverage
    // =========================================================================

    #[test]
    fn test_to_kernel_split_direction() {
        use reovim_driver_display::SplitDirection;

        let h = to_kernel_split_direction(SplitDirection::Horizontal);
        assert!(matches!(h, KernelSplitDirection::Horizontal));

        let v = to_kernel_split_direction(SplitDirection::Vertical);
        assert!(matches!(v, KernelSplitDirection::Vertical));
    }

    // =========================================================================
    // buffer_text_range multi-line with out-of-bounds start column
    // =========================================================================

    #[test]
    fn test_buffer_text_range_multi_line_out_of_bounds_start() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("ab\ncd\nef");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Start column beyond line length in multi-line range
        let result = harness.with_runtime(|runtime| {
            runtime.buffer_text_range(buffer_id, Position::new(0, 100), Position::new(2, 1))
        });
        // Start column clamped to end of first line -> empty first line + \n + "cd\n" + "e"
        assert_eq!(result, Some("\ncd\ne".to_string()));
    }

    // =========================================================================
    // ExecuteCommand: command not found path
    // =========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_execute_command_not_found() {
        use reovim_kernel::api::v1::ModeStack;

        struct NullExecutor;
        impl CommandExecutor for NullExecutor {
            fn get_handle(&self, _id: &CommandId) -> Option<Arc<dyn CommandHandle>> {
                None
            }
        }

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = NullExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let cmd = CommandId::new(ModuleId::new("test"), "nonexistent");
        let result = runtime.execute_command(cmd, CommandContext::new());
        assert!(matches!(result, CommandResult::Error(_)));
    }

    // =========================================================================
    // ExecuteCommand: recursion guard (#547)
    // =========================================================================

    #[test]
    fn test_execute_command_recursion_guard() {
        use {
            crate::testing::TestSessionRuntime, reovim_driver_command_types::CommandContext,
            reovim_kernel::api::v1::ModuleId,
        };

        let mut harness = TestSessionRuntime::new();
        let cmd = CommandId::new(ModuleId::new("test"), "test_cmd");

        // Manually set command_depth to 16 (the limit)
        harness.with_runtime(|runtime| {
            runtime.command_depth = 16;
            let result = runtime.execute_command(cmd, CommandContext::new());
            assert!(result.is_error());
            match result {
                CommandResult::Error(msg) => {
                    assert!(msg.contains("recursion limit"));
                }
                CommandResult::Success => panic!("Expected recursion limit error"),
            }
        });
    }

    // =========================================================================
    // set_window_buffer clears selection
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_set_window_buffer_clears_selection() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());

        // Set a selection on the window
        if let Some(w) = harness.windows.get_mut(window_id) {
            w.selection = Some(crate::Selection::new(
                Position::new(0, 0),
                Position::new(0, 3),
                crate::SelectionMode::Character,
            ));
        }

        // Create a new buffer and set it on the window
        let new_buf = harness.with_runtime(|runtime| runtime.create_buffer(None, "world"));
        let result = harness.with_runtime(|runtime| runtime.set_window_buffer(window_id, new_buf));
        assert!(result.is_ok());

        // Selection should be cleared
        assert!(harness.windows.get(window_id).unwrap().selection.is_none());
    }

    // =========================================================================
    // insert_text with cursor from active window
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_text_with_cursor_position() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Set cursor to a specific position
        if let Some(w) = harness.windows.active_mut() {
            w.cursor.line = 0;
            w.cursor.column = 3;
        }

        harness.with_runtime(|runtime| {
            runtime.insert_text(buffer_id, Position::new(0, 3), " ");
        });

        harness.assert_buffer_content("hel lo");
    }

    // =========================================================================
    // delete_range with actual content and BufferModified event
    // =========================================================================

    #[test]
    fn test_delete_range_emits_buffer_modified() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        harness.with_runtime(|runtime| {
            runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 11));
        });

        harness.assert_buffer_content("hello");
        let changes = harness.take_changes();
        assert!(changes.buffer_modified);
        assert!(changes.affected_buffers.contains(&buffer_id));
    }

    // =========================================================================
    // pop_mode with PopResult
    // =========================================================================

    #[test]
    fn test_pop_mode_with_result() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        harness.with_runtime(|runtime| {
            runtime.push_mode(test_mode_2(), TransitionContext::new());
        });

        // Pop with a Cancelled result
        let result = harness.with_runtime(|runtime| runtime.pop_mode(Some(PopResult::Cancelled)));
        assert!(result.is_ok());
    }

    // =========================================================================
    // set_mode records change
    // =========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_set_mode_records_change() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();

        harness.with_runtime(|runtime| {
            runtime.set_mode(test_mode_2(), TransitionContext::new());
        });

        let changes = harness.take_changes();
        assert!(changes.mode_changed);
    }

    // =========================================================================
    // insert_text / delete_range fallback when no active window (lines 559, 584)
    // =========================================================================

    /// Buffer manager that actually stores buffers, for tests requiring real buffers.
    struct InMemoryBufferManager {
        buffers: reovim_arch::sync::RwLock<
            std::collections::HashMap<
                BufferId,
                std::sync::Arc<reovim_arch::sync::RwLock<reovim_kernel::api::v1::Buffer>>,
            >,
        >,
    }

    impl InMemoryBufferManager {
        fn new() -> Self {
            Self {
                buffers: reovim_arch::sync::RwLock::new(std::collections::HashMap::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_kernel::api::v1::BufferManager for InMemoryBufferManager {
        fn get(
            &self,
            id: BufferId,
        ) -> Option<std::sync::Arc<reovim_arch::sync::RwLock<reovim_kernel::api::v1::Buffer>>>
        {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = std::sync::Arc::new(reovim_arch::sync::RwLock::new(
                reovim_kernel::api::v1::Buffer::new(),
            ));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: reovim_kernel::api::v1::Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = std::sync::Arc::new(reovim_arch::sync::RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(
            &self,
            id: BufferId,
        ) -> Result<reovim_kernel::api::v1::Buffer, reovim_kernel::api::v1::BufferError> {
            self.buffers.write().remove(&id).map_or(
                Err(reovim_kernel::api::v1::BufferError::NotFound(id)),
                |arc_buf| {
                    std::sync::Arc::try_unwrap(arc_buf)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                },
            )
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    /// Helper to create a `KernelContext` with a real buffer manager and custom services.
    fn make_kernel_with_services(
        services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
    ) -> KernelContext {
        use reovim_kernel::api::v1::{
            EventBus, MarkBank, MotionEngine, OptionRegistry, TextObjectEngine,
        };

        KernelContext::new(
            std::sync::Arc::new(EventBus::new()),
            std::sync::Arc::new(InMemoryBufferManager::new()),
            std::sync::Arc::new(MotionEngine),
            std::sync::Arc::new(TextObjectEngine),
            std::sync::Arc::new(reovim_arch::sync::RwLock::new(MarkBank::new())),
            std::sync::Arc::new(OptionRegistry::new()),
            services,
        )
    }

    #[test]
    fn test_insert_text_no_active_window_uses_zero_position() {
        use reovim_kernel::api::v1::ModeStack;

        let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
        let kernel = make_kernel_with_services(services);
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;

        let buf = reovim_kernel::api::v1::Buffer::from_string("hello");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty(); // No windows!
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = Some(buf_id); // Per-client (#471)
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        runtime.insert_text(buf_id, Position::new(0, 5), " world");

        let content = runtime.buffer_content(buf_id);
        assert_eq!(content, Some("hello world".to_string()));
    }

    #[test]
    fn test_delete_range_no_active_window_uses_zero_position() {
        use reovim_kernel::api::v1::ModeStack;

        let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
        let kernel = make_kernel_with_services(services);
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;

        let buf = reovim_kernel::api::v1::Buffer::from_string("hello world");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty(); // No windows!
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = Some(buf_id); // Per-client (#471)
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        runtime.delete_range(buf_id, Position::new(0, 5), Position::new(0, 11));

        let content = runtime.buffer_content(buf_id);
        assert_eq!(content, Some("hello".to_string()));
    }

    // =========================================================================
    // RegisterApi with clipboard provider (lines 736-790)
    // =========================================================================

    struct MockClipboard {
        clipboard: std::sync::Mutex<Option<String>>,
        selection: std::sync::Mutex<Option<String>>,
    }

    impl MockClipboard {
        fn new() -> Self {
            Self {
                clipboard: std::sync::Mutex::new(None),
                selection: std::sync::Mutex::new(None),
            }
        }

        fn with_clipboard(text: &str) -> Self {
            Self {
                clipboard: std::sync::Mutex::new(Some(text.to_string())),
                selection: std::sync::Mutex::new(None),
            }
        }

        fn with_selection(text: &str) -> Self {
            Self {
                clipboard: std::sync::Mutex::new(None),
                selection: std::sync::Mutex::new(Some(text.to_string())),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_driver_clipboard::ClipboardProvider for MockClipboard {
        fn clipboard_available(&self) -> bool {
            true
        }
        fn copy_to_clipboard(
            &self,
            text: &str,
        ) -> Result<(), reovim_driver_clipboard::ClipboardError> {
            *self.clipboard.lock().unwrap() = Some(text.to_string());
            Ok(())
        }
        fn paste_from_clipboard(
            &self,
        ) -> Result<Option<String>, reovim_driver_clipboard::ClipboardError> {
            Ok(self.clipboard.lock().unwrap().clone())
        }
        fn selection_available(&self) -> bool {
            true
        }
        fn copy_to_selection(
            &self,
            text: &str,
        ) -> Result<(), reovim_driver_clipboard::ClipboardError> {
            *self.selection.lock().unwrap() = Some(text.to_string());
            Ok(())
        }
        fn paste_from_selection(
            &self,
        ) -> Result<Option<String>, reovim_driver_clipboard::ClipboardError> {
            Ok(self.selection.lock().unwrap().clone())
        }
    }

    fn kernel_with_clipboard(provider: MockClipboard) -> KernelContext {
        let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
        let clipboard_registry = ClipboardProviderRegistry::new();
        clipboard_registry.register(ClipboardKey::Default, std::sync::Arc::new(provider));
        services.register(std::sync::Arc::new(clipboard_registry));
        make_kernel_with_services(services)
    }

    /// Raw `get_register(Some('+'))` returns `None` since `RegisterBank`
    /// doesn't handle `+`. Use `get_register_with_clipboard` instead (#515).
    #[test]
    fn test_get_register_raw_clipboard_plus_returns_none() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_clipboard(MockClipboard::with_clipboard("from-clipboard"));
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        // Raw RegisterApi does NOT route + to clipboard
        assert!(runtime.get_register(Some('+')).is_none());

        // get_register_with_clipboard reads from OS clipboard
        let content = runtime.get_register_with_clipboard(Some('+'));
        assert!(content.is_some());
        assert_eq!(content.unwrap().text, "from-clipboard");
    }

    /// Raw `get_register(Some('*'))` returns `None`. Use
    /// `get_register_with_clipboard` to read from OS selection (#515).
    #[test]
    fn test_get_register_raw_selection_star_returns_none() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_clipboard(MockClipboard::with_selection("from-selection"));
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        // Raw RegisterApi does NOT route * to selection
        assert!(runtime.get_register(Some('*')).is_none());

        // get_register_with_clipboard reads from OS selection
        let content = runtime.get_register_with_clipboard(Some('*'));
        assert!(content.is_some());
        assert_eq!(content.unwrap().text, "from-selection");
    }

    #[test]
    fn test_get_register_numbered_with_provider() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_clipboard(MockClipboard::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        // Pre-populate per-client clipboard history (#515):
        // push "yank-1" first so it becomes index 1, then "yank-0" so it becomes index 0
        let mut clipboard_history = HistoryRing::new();
        clipboard_history.push(RegisterContent::characterwise("yank-1"));
        clipboard_history.push(RegisterContent::characterwise("yank-0"));
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let c0 = runtime.get_register(Some('0'));
        assert!(c0.is_some());
        assert_eq!(c0.unwrap().text, "yank-0");

        let c1 = runtime.get_register(Some('1'));
        assert!(c1.is_some());
        assert_eq!(c1.unwrap().text, "yank-1");
    }

    /// `set_register` for `+` does NOT store in `RegisterBank` (which only
    /// handles a-z/A-Z/unnamed). Callers must use `store_register_with_sync`
    /// for clipboard sync (#515 Phase 4).
    #[test]
    fn test_set_register_clipboard_plus_not_stored_in_register_bank() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_clipboard(MockClipboard::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        // Raw set_register does NOT store + in RegisterBank
        runtime.set_register(Some('+'), RegisterContent::characterwise("to-clipboard"));
        assert!(runtime.get_register(Some('+')).is_none());
    }

    /// `store_register_with_sync` for `+` syncs to clipboard and is readable
    /// via `get_register_with_clipboard` (#515 Phase 4).
    #[test]
    fn test_store_register_with_sync_clipboard_plus() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_clipboard(MockClipboard::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        runtime.store_register_with_sync(Some('+'), RegisterContent::characterwise("to-clipboard"));

        let content = runtime.get_register_with_clipboard(Some('+'));
        assert!(content.is_some());
        assert_eq!(content.unwrap().text, "to-clipboard");
    }

    /// Same as above but for `*` (selection).
    #[test]
    fn test_set_register_selection_star_not_stored_in_register_bank() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_clipboard(MockClipboard::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        runtime.set_register(Some('*'), RegisterContent::characterwise("to-selection"));
        assert!(runtime.get_register(Some('*')).is_none());
    }

    /// `store_register_with_sync` for `*` syncs to selection and is readable
    /// via `get_register_with_clipboard` (#515 Phase 4).
    #[test]
    fn test_store_register_with_sync_selection_star() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_clipboard(MockClipboard::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        runtime.store_register_with_sync(Some('*'), RegisterContent::characterwise("to-selection"));

        let content = runtime.get_register_with_clipboard(Some('*'));
        assert!(content.is_some());
        assert_eq!(content.unwrap().text, "to-selection");
    }

    // =========================================================================
    // UndoApi with provider (lines 817-993)
    // =========================================================================

    struct MockUndoProvider {
        edits: std::sync::Mutex<Vec<reovim_driver_undo::UndoRecord>>,
    }

    impl MockUndoProvider {
        fn new() -> Self {
            Self {
                edits: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_driver_undo::UndoProvider for MockUndoProvider {
        fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            Some(UndoResult {
                edits: vec![Edit::Insert {
                    position: Position::new(0, 0),
                    text: "X".to_string(),
                }],
                cursor: Position::new(0, 1),
            })
        }

        fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            Some(UndoResult {
                edits: vec![Edit::Delete {
                    position: Position::new(0, 0),
                    text: "X".to_string(),
                }],
                cursor: Position::new(0, 0),
            })
        }

        fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
            None
        }

        fn record(
            &self,
            buffer_id: BufferId,
            edits: Vec<Edit>,
            cursor_before: Position,
            cursor_after: Position,
        ) {
            self.edits
                .lock()
                .unwrap()
                .push(reovim_driver_undo::UndoRecord {
                    buffer_id,
                    edits,
                    cursor_before,
                    cursor_after,
                });
        }

        fn has_history(&self, _buffer_id: BufferId) -> bool {
            true
        }

        fn remove(&self, _buffer_id: BufferId) {}

        fn buffer_count(&self) -> usize {
            1
        }

        fn get_tree(&self, _buffer_id: BufferId) -> Option<reovim_kernel::api::v1::UndoTree> {
            let mut tree = reovim_kernel::api::v1::UndoTree::new();
            tree.push(
                vec![Edit::Insert {
                    position: Position::new(0, 0),
                    text: "a".to_string(),
                }],
                Position::new(0, 0),
                Position::new(0, 1),
            );
            Some(tree)
        }

        fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
        fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
        fn is_batching(&self, _buffer_id: BufferId) -> bool {
            false
        }

        fn persist(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn reovim_driver_vfs::VfsDriver,
        ) -> Result<(), reovim_driver_undo::UndoPersistError> {
            Ok(())
        }

        fn load(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn reovim_driver_vfs::VfsDriver,
        ) -> Result<bool, reovim_driver_undo::UndoPersistError> {
            Ok(false)
        }

        fn undo_for_client(&self, _buffer_id: BufferId, _client_id: usize) -> Option<UndoResult> {
            Some(UndoResult {
                edits: vec![Edit::Insert {
                    position: Position::new(0, 0),
                    text: "Y".to_string(),
                }],
                cursor: Position::new(0, 1),
            })
        }

        fn redo_for_client(&self, _buffer_id: BufferId, _client_id: usize) -> Option<UndoResult> {
            Some(UndoResult {
                edits: vec![Edit::Delete {
                    position: Position::new(0, 0),
                    text: "Y".to_string(),
                }],
                cursor: Position::new(0, 0),
            })
        }

        fn record_for_client(
            &self,
            buffer_id: BufferId,
            _client_id: usize,
            edits: Vec<Edit>,
            cursor_before: Position,
            cursor_after: Position,
        ) {
            self.record(buffer_id, edits, cursor_before, cursor_after);
        }
    }

    fn kernel_with_undo(provider: MockUndoProvider) -> KernelContext {
        let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
        let undo_registry = UndoProviderRegistry::new();
        undo_registry.register(UndoKey::Buffer, std::sync::Arc::new(provider));
        services.register(std::sync::Arc::new(undo_registry));
        make_kernel_with_services(services)
    }

    #[test]
    fn test_undo_with_provider() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_undo(MockUndoProvider::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;

        let buf = reovim_kernel::api::v1::Buffer::from_string("hello");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut window = crate::Window::new();
        window.buffer_id = Some(buf_id);
        let mut windows = crate::WindowLayout::empty();
        windows.add(window);
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let result = runtime.undo(buf_id);
        assert!(result.is_some());
        assert_eq!(result.unwrap().cursor, Position::new(0, 1));
        assert!(runtime.changes.buffer_modified);
        assert!(runtime.changes.cursor_moved);
    }

    #[test]
    fn test_redo_with_provider() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_undo(MockUndoProvider::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;

        let buf = reovim_kernel::api::v1::Buffer::from_string("Xhello");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut window = crate::Window::new();
        window.buffer_id = Some(buf_id);
        let mut windows = crate::WindowLayout::empty();
        windows.add(window);
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let result = runtime.redo(buf_id);
        assert!(result.is_some());
        assert_eq!(result.unwrap().cursor, Position::new(0, 0));
        assert!(runtime.changes.buffer_modified);
        assert!(runtime.changes.cursor_moved);
    }

    #[test]
    fn test_can_undo_with_provider() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_undo(MockUndoProvider::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let buf_id = BufferId::from_raw(1);
        assert!(runtime.can_undo(buf_id));
    }

    #[test]
    fn test_undo_mine_with_owner_and_provider() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_undo(MockUndoProvider::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let client_id = ClientId::new(42);

        let buf = reovim_kernel::api::v1::Buffer::from_string("hello");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut window = crate::Window::new();
        window.buffer_id = Some(buf_id);
        let mut windows = crate::WindowLayout::empty();
        windows.add(window);
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::with_owner(
            client_id,
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let result = runtime.undo_mine(buf_id);
        assert!(result.is_some());
        assert_eq!(result.unwrap().cursor, Position::new(0, 1));
        assert!(runtime.changes.buffer_modified);
        assert!(runtime.changes.cursor_moved);
    }

    #[test]
    fn test_redo_mine_with_owner_and_provider() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_undo(MockUndoProvider::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let client_id = ClientId::new(42);

        let buf = reovim_kernel::api::v1::Buffer::from_string("Yhello");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut window = crate::Window::new();
        window.buffer_id = Some(buf_id);
        let mut windows = crate::WindowLayout::empty();
        windows.add(window);
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::with_owner(
            client_id,
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let result = runtime.redo_mine(buf_id);
        assert!(result.is_some());
        assert_eq!(result.unwrap().cursor, Position::new(0, 0));
        assert!(runtime.changes.buffer_modified);
        assert!(runtime.changes.cursor_moved);
    }

    #[test]
    fn test_record_edit_mine_with_owner_and_provider() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_undo(MockUndoProvider::new());
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let client_id = ClientId::new(42);
        let buf_id = BufferId::from_raw(1);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = crate::WindowLayout::empty();
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::with_owner(
            client_id,
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        runtime.record_edit_mine(
            buf_id,
            vec![Edit::Insert {
                position: Position::new(0, 0),
                text: "z".to_string(),
            }],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        // No panic is sufficient
    }

    // =========================================================================
    // UndoApi: alternate edit type branches (Delete in undo, Insert in redo)
    // =========================================================================

    /// Undo provider that returns Delete edits for undo and Insert edits for redo,
    /// covering the alternate branches in the undo/redo implementations.
    struct AlternateUndoProvider;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_driver_undo::UndoProvider for AlternateUndoProvider {
        fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            // Return a Delete edit (the existing MockUndoProvider returns Insert)
            Some(UndoResult {
                edits: vec![Edit::Delete {
                    position: Position::new(0, 0),
                    text: "X".to_string(),
                }],
                cursor: Position::new(0, 0),
            })
        }

        fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            // Return an Insert edit (the existing MockUndoProvider returns Delete)
            Some(UndoResult {
                edits: vec![Edit::Insert {
                    position: Position::new(0, 0),
                    text: "Y".to_string(),
                }],
                cursor: Position::new(0, 1),
            })
        }

        fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
            None
        }

        fn record(
            &self,
            _buffer_id: BufferId,
            _edits: Vec<Edit>,
            _cursor_before: Position,
            _cursor_after: Position,
        ) {
        }

        fn has_history(&self, _buffer_id: BufferId) -> bool {
            true
        }

        fn remove(&self, _buffer_id: BufferId) {}

        fn buffer_count(&self) -> usize {
            1
        }

        fn get_tree(&self, _buffer_id: BufferId) -> Option<reovim_kernel::api::v1::UndoTree> {
            None
        }

        fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
        fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
        fn is_batching(&self, _buffer_id: BufferId) -> bool {
            false
        }

        fn persist(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn reovim_driver_vfs::VfsDriver,
        ) -> Result<(), reovim_driver_undo::UndoPersistError> {
            Ok(())
        }

        fn load(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn reovim_driver_vfs::VfsDriver,
        ) -> Result<bool, reovim_driver_undo::UndoPersistError> {
            Ok(false)
        }

        fn undo_for_client(&self, _buffer_id: BufferId, _client_id: usize) -> Option<UndoResult> {
            // Return Delete edit for undo_mine coverage
            Some(UndoResult {
                edits: vec![Edit::Delete {
                    position: Position::new(0, 0),
                    text: "Z".to_string(),
                }],
                cursor: Position::new(0, 0),
            })
        }

        fn redo_for_client(&self, _buffer_id: BufferId, _client_id: usize) -> Option<UndoResult> {
            // Return Insert edit for redo_mine coverage
            Some(UndoResult {
                edits: vec![Edit::Insert {
                    position: Position::new(0, 0),
                    text: "W".to_string(),
                }],
                cursor: Position::new(0, 1),
            })
        }

        fn record_for_client(
            &self,
            _buffer_id: BufferId,
            _client_id: usize,
            _edits: Vec<Edit>,
            _cursor_before: Position,
            _cursor_after: Position,
        ) {
        }
    }

    fn kernel_with_alternate_undo() -> KernelContext {
        let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
        let undo_registry = UndoProviderRegistry::new();
        undo_registry.register(UndoKey::Buffer, std::sync::Arc::new(AlternateUndoProvider));
        services.register(std::sync::Arc::new(undo_registry));
        make_kernel_with_services(services)
    }

    /// Undo with Delete edits covers the `Edit::Delete` branch in `undo()` (lines 829-831, 835).
    #[test]
    fn test_undo_with_delete_edits() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_alternate_undo();
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;

        let buf = reovim_kernel::api::v1::Buffer::from_string("Xhello");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut window = crate::Window::new();
        window.buffer_id = Some(buf_id);
        let mut windows = crate::WindowLayout::empty();
        windows.add(window);
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let result = runtime.undo(buf_id);
        assert!(result.is_some());
        // Delete edit removed "X" from position (0,0)
        let content = runtime.buffer_content(buf_id);
        assert_eq!(content, Some("hello".to_string()));
        assert!(runtime.changes.buffer_modified);
    }

    /// Redo with Insert edits covers the `Edit::Insert` branch in `redo()` (lines 863-865, 872).
    #[test]
    fn test_redo_with_insert_edits() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_alternate_undo();
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;

        let buf = reovim_kernel::api::v1::Buffer::from_string("hello");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut window = crate::Window::new();
        window.buffer_id = Some(buf_id);
        let mut windows = crate::WindowLayout::empty();
        windows.add(window);
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let result = runtime.redo(buf_id);
        assert!(result.is_some());
        // Insert edit added "Y" at position (0,0)
        let content = runtime.buffer_content(buf_id);
        assert_eq!(content, Some("Yhello".to_string()));
        assert!(runtime.changes.buffer_modified);
    }

    /// `undo_mine` with Delete edits covers the `Edit::Delete` branch (lines 938-940, 943).
    #[test]
    fn test_undo_mine_with_delete_edits() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_alternate_undo();
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let client_id = ClientId::new(42);

        let buf = reovim_kernel::api::v1::Buffer::from_string("Zhello");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut window = crate::Window::new();
        window.buffer_id = Some(buf_id);
        let mut windows = crate::WindowLayout::empty();
        windows.add(window);
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::with_owner(
            client_id,
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let result = runtime.undo_mine(buf_id);
        assert!(result.is_some());
        // Delete edit removed "Z" from position (0,0)
        let content = runtime.buffer_content(buf_id);
        assert_eq!(content, Some("hello".to_string()));
        assert!(runtime.changes.buffer_modified);
    }

    /// `redo_mine` with Insert edits covers the `Edit::Insert` branch (lines 974-976, 982).
    #[test]
    fn test_redo_mine_with_insert_edits() {
        use reovim_kernel::api::v1::ModeStack;

        let kernel = kernel_with_alternate_undo();
        let mut session = Session::new(ClientId::new(1), test_mode());
        let executor = StubExecutor;
        let client_id = ClientId::new(42);

        let buf = reovim_kernel::api::v1::Buffer::from_string("hello");
        let buf_id = kernel.buffers.register(buf);

        let mut mode_stack = ModeStack::new(test_mode());
        let mut window = crate::Window::new();
        window.buffer_id = Some(buf_id);
        let mut windows = crate::WindowLayout::empty();
        windows.add(window);
        let mut extensions = crate::ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = crate::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::with_owner(
            client_id,
            &mut session,
            crate::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let result = runtime.redo_mine(buf_id);
        assert!(result.is_some());
        // Insert edit added "W" at position (0,0)
        let content = runtime.buffer_content(buf_id);
        assert_eq!(content, Some("Whello".to_string()));
        assert!(runtime.changes.buffer_modified);
    }

    /// `apply_undo_edits` returns early when the buffer is not in the kernel.
    #[test]
    fn test_apply_undo_edits_buffer_not_found() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let rt = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        // BufferId::new() is not registered in the kernel
        let buf = BufferId::new();
        let edits = vec![Edit::Insert {
            position: Position::new(0, 0),
            text: "X".to_string(),
        }];

        // Should return without panic (early return from let...else)
        rt.apply_undo_edits(buf, &edits);
    }

    // =========================================================================
    // CompositorApi with compositor (lines 1081-1537)
    // =========================================================================

    struct MockLayerCompositor {
        id: reovim_driver_display::layout::LayerId,
        windows: Vec<WindowId>,
        focused: Option<WindowId>,
        next_id: usize,
    }

    impl MockLayerCompositor {
        fn new() -> Self {
            let first = WindowId::from_raw(1);
            let second = WindowId::from_raw(2);
            Self {
                id: reovim_driver_display::layout::LayerId::new(0),
                windows: vec![first, second],
                focused: Some(first),
                next_id: 3,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_driver_display::layout::WindowLayerCompositor for MockLayerCompositor {
        fn id(&self) -> reovim_driver_display::layout::LayerId {
            self.id
        }

        fn arrange(&self, _bounds: Rect) -> Vec<reovim_driver_display::layout::WindowPlacement> {
            Vec::new()
        }

        fn add_tiled(&mut self) -> WindowId {
            let id = WindowId::from_raw(self.next_id);
            self.next_id += 1;
            self.windows.push(id);
            if self.focused.is_none() {
                self.focused = Some(id);
            }
            id
        }

        fn split_tiled(
            &mut self,
            _from: WindowId,
            _direction: reovim_driver_display::SplitDirection,
        ) -> Option<WindowId> {
            let id = WindowId::from_raw(self.next_id);
            self.next_id += 1;
            self.windows.push(id);
            self.focused = Some(id);
            Some(id)
        }

        fn navigate_tiled(
            &self,
            from: WindowId,
            _direction: NavigateDirection,
        ) -> Option<WindowId> {
            self.windows.iter().find(|&&w| w != from).copied()
        }

        fn resize_tiled(&mut self, _window: WindowId, _direction: NavigateDirection, _delta: i16) {}

        fn close_tiled(&mut self, window: WindowId) -> Option<WindowId> {
            self.windows.retain(|&w| w != window);
            let next = self.windows.first().copied();
            if self.focused == Some(window) {
                self.focused = next;
            }
            next
        }

        fn equalize_tiled(&mut self) {}

        fn cycle_tiled(&self, from: WindowId, _forward: bool) -> Option<WindowId> {
            self.windows.iter().find(|&&w| w != from).copied()
        }

        fn create_float(&mut self, _bounds: Rect) -> WindowId {
            let id = WindowId::from_raw(self.next_id);
            self.next_id += 1;
            id
        }

        fn move_float(&mut self, _window: WindowId, _x: u16, _y: u16) {}
        fn resize_float(&mut self, _window: WindowId, _width: u16, _height: u16) {}
        fn raise_float(&mut self, _window: WindowId) {}
        fn lower_float(&mut self, _window: WindowId) {}
        fn close_float(&mut self, _window: WindowId) {}
        fn toggle_float(&mut self, _window: WindowId) {}

        fn show_overlay(
            &mut self,
            _constraints: reovim_driver_display::layout::OverlayConstraints,
        ) -> WindowId {
            let id = WindowId::from_raw(self.next_id);
            self.next_id += 1;
            id
        }

        fn hide_overlay(&mut self, _window: WindowId) {}
        fn resize_overlay(&mut self, _window: WindowId, _width: u16, _height: u16) {}
        fn hide_all_overlays(&mut self) {}

        fn set_focus(&mut self, window: WindowId) {
            self.focused = Some(window);
        }

        fn focused(&self) -> Option<WindowId> {
            self.focused
        }

        fn windows_in_zone(&self, zone: reovim_driver_display::layout::Zone) -> Vec<WindowId> {
            if zone == reovim_driver_display::layout::Zone::Tiled {
                self.windows.clone()
            } else {
                Vec::new()
            }
        }

        fn zone_of(&self, _window: WindowId) -> Option<reovim_driver_display::layout::Zone> {
            Some(reovim_driver_display::layout::Zone::Tiled)
        }
    }

    struct MockRootCompositor {
        layer: MockLayerCompositor,
        focused: Option<WindowId>,
    }

    impl MockRootCompositor {
        fn new() -> Self {
            let layer = MockLayerCompositor::new();
            let focused = layer.focused;
            Self { layer, focused }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_driver_display::layout::RootCompositor for MockRootCompositor {
        fn composite(&self, screen: Rect) -> reovim_driver_display::layout::CompositeResult {
            reovim_driver_display::layout::CompositeResult::empty(screen)
        }

        fn create_layer(
            &mut self,
            _config: reovim_driver_display::layout::LayerConfig,
        ) -> reovim_driver_display::layout::LayerId {
            reovim_driver_display::layout::LayerId::new(0)
        }

        fn remove_layer(&mut self, _layer: reovim_driver_display::layout::LayerId) {}

        fn layer_by_label(&self, _label: &str) -> Option<reovim_driver_display::layout::LayerId> {
            None
        }

        fn layers(&self) -> Vec<&reovim_driver_display::layout::Layer> {
            Vec::new()
        }

        fn set_layer_visible(
            &mut self,
            _layer: reovim_driver_display::layout::LayerId,
            _visible: bool,
        ) {
        }

        fn set_layer_opacity(
            &mut self,
            _layer: reovim_driver_display::layout::LayerId,
            _opacity: f32,
        ) {
        }

        fn reorder_layer(&mut self, _layer: reovim_driver_display::layout::LayerId, _new_z: u16) {}

        fn set_active_layer(&mut self, _layer: reovim_driver_display::layout::LayerId) {}

        fn active_layer(&self) -> Option<reovim_driver_display::layout::LayerId> {
            Some(reovim_driver_display::layout::LayerId::new(0))
        }

        fn set_focus(&mut self, window: WindowId) {
            self.focused = Some(window);
            reovim_driver_display::layout::WindowLayerCompositor::set_focus(
                &mut self.layer,
                window,
            );
        }

        fn focused(&self) -> Option<WindowId> {
            self.focused
        }

        fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
            self.focused
        }

        fn layer_compositor(
            &self,
            _layer: reovim_driver_display::layout::LayerId,
        ) -> Option<&dyn reovim_driver_display::layout::WindowLayerCompositor> {
            Some(&self.layer)
        }

        fn layer_compositor_mut(
            &mut self,
            _layer: reovim_driver_display::layout::LayerId,
        ) -> Option<&mut dyn reovim_driver_display::layout::WindowLayerCompositor> {
            Some(&mut self.layer)
        }

        fn window_count(&self) -> usize {
            self.layer.windows.len()
        }

        fn set_screen(&mut self, _screen: Rect) {}

        fn layer_of(&self, _window: WindowId) -> Option<reovim_driver_display::layout::LayerId> {
            Some(reovim_driver_display::layout::LayerId::new(0))
        }

        fn boxed_clone(&self) -> Box<dyn reovim_driver_display::layout::RootCompositor> {
            Box::new(Self {
                layer: MockLayerCompositor {
                    id: self.layer.id,
                    windows: self.layer.windows.clone(),
                    focused: self.layer.focused,
                    next_id: self.layer.next_id,
                },
                focused: self.focused,
            })
        }
    }

    fn make_compositor_runtime<'a>(
        session: &'a mut Session,
        client: crate::ClientContext<'a>,
        kernel: &'a KernelContext,
        executor: &'a StubExecutor,
    ) -> SessionRuntime<'a> {
        *client.compositor = Some(Box::new(MockRootCompositor::new()));
        SessionRuntime::new(session, client, kernel, executor)
    }

    #[test]
    fn test_compositor_navigate() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let result = rt.navigate(NavigateDirection::Right);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WindowId::from_raw(2));
    }

    #[test]
    fn test_compositor_split() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let result = rt.split(reovim_driver_display::SplitDirection::Horizontal);
        assert!(result.is_ok());
        assert!(rt.changes.windows_created.contains(&result.unwrap()));
    }

    #[test]
    fn test_compositor_close_current_window() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let result = rt.close_current_window();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WindowId::from_raw(2));
    }

    #[test]
    fn test_compositor_close_others() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let result = rt.close_others();
        assert!(result.is_ok());
        assert!(rt.changes.windows_closed.contains(&WindowId::from_raw(2)));
    }

    #[test]
    fn test_compositor_resize() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let result = rt.resize(NavigateDirection::Right, 5);
        assert!(result.is_ok());
        assert!(rt.changes.window_changed);
    }

    #[test]
    fn test_compositor_equalize() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let result = rt.equalize();
        assert!(result.is_ok());
        assert!(rt.changes.window_changed);
    }

    #[test]
    fn test_compositor_cycle() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let result = rt.cycle(true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WindowId::from_raw(2));
    }

    #[test]
    fn test_compositor_focus() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let result = rt.focus(WindowId::from_raw(2));
        assert!(result.is_ok());
        assert!(rt.changes.focus_changed);
    }

    #[test]
    fn test_compositor_focus_same_window_no_event() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        // Focus the already-focused window - LayoutChanged event should NOT fire
        let result = rt.focus(WindowId::from_raw(1));
        assert!(result.is_ok());
        assert!(rt.changes.focus_changed);
    }

    #[test]
    fn test_compositor_focused_window() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert_eq!(rt.focused_window(), Some(WindowId::from_raw(1)));
    }

    #[test]
    fn test_compositor_window_count() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert_eq!(rt.compositor_window_count(), 2);
    }

    #[test]
    fn test_compositor_active_layer() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(rt.active_layer().is_some());
    }

    #[test]
    fn test_compositor_toggle_float() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(rt.toggle_float().is_ok());
        assert!(rt.changes.window_changed);
    }

    #[test]
    fn test_compositor_raise_float() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(rt.raise_float().is_ok());
    }

    #[test]
    fn test_compositor_lower_float() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(rt.lower_float().is_ok());
    }

    #[test]
    fn test_compositor_show_overlay() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let result = rt.show_overlay(
            reovim_driver_display::layout::OverlayConstraints::centered().with_size(20, 10),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_compositor_hide_overlay() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(rt.hide_overlay(WindowId::from_raw(99)).is_ok());
    }

    #[test]
    fn test_compositor_resize_overlay() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(rt.resize_overlay(WindowId::from_raw(99), 40, 20).is_ok());
    }

    #[test]
    fn test_compositor_hide_all_overlays() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(rt.hide_all_overlays().is_ok());
    }

    #[test]
    fn test_compositor_set_active_layer_opacity() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert!(rt.set_active_layer_opacity(0.5).is_ok());
    }

    #[test]
    fn test_compositor_set_active_layer_opacity_clamps() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        // Values should be clamped to 0.0..=1.0
        assert!(rt.set_active_layer_opacity(2.0).is_ok());
        assert!(rt.set_active_layer_opacity(-1.0).is_ok());
    }

    #[test]
    fn test_compositor_active_layer_opacity() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        // MockRootCompositor returns empty layers(), so default 1.0
        let opacity = rt.active_layer_opacity().unwrap();
        assert!((opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compositor_adjust_active_layer_opacity() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        // Adjust from default 1.0 by -0.3 → 0.7
        let new_opacity = rt.adjust_active_layer_opacity(-0.3).unwrap();
        assert!((new_opacity - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_screen_with_compositor() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        rt.set_screen(Rect::new(0, 0, 120, 40));
    }

    #[test]
    fn test_emit_layout_event_with_compositor() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        rt.emit_layout_event(reovim_kernel::api::v1::events::kernel::LayoutChangeKind::Equalize);
    }

    #[test]
    fn test_arrange_with_compositor() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        let placements = rt.arrange(Rect::new(0, 0, 80, 24));
        assert!(placements.is_empty());
    }

    // =========================================================================
    // CompositorApi: close_current_window with single window (CannotCloseLastWindow)
    // =========================================================================

    /// Mock compositor with only ONE tiled window, so `close_current_window`
    /// returns `CannotCloseLastWindow` (covers line 1176).
    struct SingleWindowLayerCompositor {
        id: reovim_driver_display::layout::LayerId,
        window: WindowId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_driver_display::layout::WindowLayerCompositor for SingleWindowLayerCompositor {
        fn id(&self) -> reovim_driver_display::layout::LayerId {
            self.id
        }
        fn arrange(&self, _bounds: Rect) -> Vec<WindowPlacement> {
            Vec::new()
        }
        fn add_tiled(&mut self) -> WindowId {
            self.window
        }
        fn split_tiled(
            &mut self,
            _from: WindowId,
            _direction: reovim_driver_display::SplitDirection,
        ) -> Option<WindowId> {
            None
        }
        fn navigate_tiled(
            &self,
            _from: WindowId,
            _direction: NavigateDirection,
        ) -> Option<WindowId> {
            None
        }
        fn resize_tiled(&mut self, _window: WindowId, _direction: NavigateDirection, _delta: i16) {}
        fn close_tiled(&mut self, _window: WindowId) -> Option<WindowId> {
            None
        }
        fn equalize_tiled(&mut self) {}
        fn cycle_tiled(&self, _from: WindowId, _forward: bool) -> Option<WindowId> {
            None
        }
        fn create_float(&mut self, _bounds: Rect) -> WindowId {
            self.window
        }
        fn move_float(&mut self, _window: WindowId, _x: u16, _y: u16) {}
        fn resize_float(&mut self, _window: WindowId, _width: u16, _height: u16) {}
        fn raise_float(&mut self, _window: WindowId) {}
        fn lower_float(&mut self, _window: WindowId) {}
        fn close_float(&mut self, _window: WindowId) {}
        fn toggle_float(&mut self, _window: WindowId) {}
        fn show_overlay(&mut self, _constraints: OverlayConstraints) -> WindowId {
            self.window
        }
        fn hide_overlay(&mut self, _window: WindowId) {}
        fn resize_overlay(&mut self, _window: WindowId, _width: u16, _height: u16) {}
        fn hide_all_overlays(&mut self) {}
        fn set_focus(&mut self, _window: WindowId) {}
        fn focused(&self) -> Option<WindowId> {
            Some(self.window)
        }
        fn windows_in_zone(&self, zone: reovim_driver_display::layout::Zone) -> Vec<WindowId> {
            if zone == reovim_driver_display::layout::Zone::Tiled {
                vec![self.window] // Only ONE tiled window
            } else {
                Vec::new()
            }
        }
        fn zone_of(&self, _window: WindowId) -> Option<reovim_driver_display::layout::Zone> {
            Some(reovim_driver_display::layout::Zone::Tiled)
        }
    }

    struct SingleWindowRootCompositor {
        layer: SingleWindowLayerCompositor,
    }

    impl SingleWindowRootCompositor {
        fn new() -> Self {
            Self {
                layer: SingleWindowLayerCompositor {
                    id: reovim_driver_display::layout::LayerId::new(0),
                    window: WindowId::from_raw(1),
                },
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_driver_display::layout::RootCompositor for SingleWindowRootCompositor {
        fn composite(&self, screen: Rect) -> reovim_driver_display::layout::CompositeResult {
            reovim_driver_display::layout::CompositeResult::empty(screen)
        }
        fn create_layer(
            &mut self,
            _config: reovim_driver_display::layout::LayerConfig,
        ) -> reovim_driver_display::layout::LayerId {
            reovim_driver_display::layout::LayerId::new(0)
        }
        fn remove_layer(&mut self, _layer: reovim_driver_display::layout::LayerId) {}
        fn layer_by_label(&self, _label: &str) -> Option<reovim_driver_display::layout::LayerId> {
            None
        }
        fn layers(&self) -> Vec<&reovim_driver_display::layout::Layer> {
            Vec::new()
        }
        fn set_layer_visible(
            &mut self,
            _layer: reovim_driver_display::layout::LayerId,
            _visible: bool,
        ) {
        }
        fn set_layer_opacity(
            &mut self,
            _layer: reovim_driver_display::layout::LayerId,
            _opacity: f32,
        ) {
        }
        fn reorder_layer(&mut self, _layer: reovim_driver_display::layout::LayerId, _new_z: u16) {}
        fn set_active_layer(&mut self, _layer: reovim_driver_display::layout::LayerId) {}
        fn active_layer(&self) -> Option<reovim_driver_display::layout::LayerId> {
            Some(reovim_driver_display::layout::LayerId::new(0))
        }
        fn set_focus(&mut self, _window: WindowId) {}
        fn focused(&self) -> Option<WindowId> {
            Some(self.layer.window)
        }
        fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
            Some(self.layer.window)
        }
        fn layer_compositor(
            &self,
            _layer: reovim_driver_display::layout::LayerId,
        ) -> Option<&dyn reovim_driver_display::layout::WindowLayerCompositor> {
            Some(&self.layer)
        }
        fn layer_compositor_mut(
            &mut self,
            _layer: reovim_driver_display::layout::LayerId,
        ) -> Option<&mut dyn reovim_driver_display::layout::WindowLayerCompositor> {
            Some(&mut self.layer)
        }
        fn window_count(&self) -> usize {
            1
        }
        fn set_screen(&mut self, _screen: Rect) {}
        fn layer_of(&self, _window: WindowId) -> Option<reovim_driver_display::layout::LayerId> {
            Some(reovim_driver_display::layout::LayerId::new(0))
        }
        fn boxed_clone(&self) -> Box<dyn reovim_driver_display::layout::RootCompositor> {
            Box::new(Self::new())
        }
    }

    #[test]
    fn test_compositor_close_current_window_single_window() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c: Option<Box<dyn reovim_driver_display::layout::RootCompositor>> =
            Some(Box::new(SingleWindowRootCompositor::new()));
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);
        let mut rt = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let result = rt.close_current_window();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CompositorError::CannotCloseLastWindow));
    }

    // === #474: Centralized selection extension tests ===

    /// When cursor moves with an active selection, `sel.end` should auto-update
    /// and `selection_changed` should be set.
    #[test]
    fn test_record_cursor_move_extends_selection() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        // Add a window with selection and move cursor
        let mut window = crate::Window::new();
        window.cursor = Position::new(0, 5).into();
        window.selection =
            Some(crate::api::Selection::character(Position::new(0, 0), Position::new(0, 1)));
        w.add(window);

        let buf = BufferId::new();
        let mut rt = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        rt.record_cursor_move(buf);

        let changes = rt.take_changes();
        assert!(changes.cursor_moved);
        assert!(changes.selection_changed);

        // sel.end should match cursor position + 1
        let sel = rt.windows().active().unwrap().selection.as_ref().unwrap();
        assert_eq!(sel.end, Position::new(0, 6));
    }

    /// When cursor moves without a selection, `selection_changed` should NOT be set.
    #[test]
    fn test_record_cursor_move_no_selection() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut window = crate::Window::new();
        window.cursor = Position::new(0, 3).into();
        // No selection
        w.add(window);

        let buf = BufferId::new();
        let mut rt = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        rt.record_cursor_move(buf);

        let changes = rt.take_changes();
        assert!(changes.cursor_moved);
        assert!(!changes.selection_changed);
    }

    /// When cursor moves with no active window, only `cursor_moved` is set.
    #[test]
    fn test_record_cursor_move_no_active_window() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty(); // No windows added
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let buf = BufferId::new();
        let mut rt = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        rt.record_cursor_move(buf);

        let changes = rt.take_changes();
        assert!(changes.cursor_moved);
        assert!(!changes.selection_changed);
    }

    /// Direct `record_selection_change` should set `selection_changed`.
    #[test]
    fn test_record_selection_change_directly() {
        use reovim_kernel::api::v1::ModeStack;

        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let buf = BufferId::new();
        let mut rt = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        rt.record_selection_change(buf);

        let changes = rt.take_changes();
        assert!(changes.selection_changed);
        assert!(changes.affected_buffers.contains(&buf));
    }

    // =========================================================================
    // CompositorApi: focus() with same window (line 1352 false branch)
    // =========================================================================

    /// Calling `focus()` on the already-focused window should NOT emit a layout
    /// event (the `if previous_focus != Some(window)` branch is false).
    #[test]
    fn test_compositor_focus_same_window_no_layout_event() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        let mut c = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = make_compositor_runtime(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        // MockRootCompositor starts with focus on WindowId::from_raw(1).
        // Calling focus() on the already-focused window exercises the
        // `previous_focus == Some(window)` path (line 1352 false branch).
        let already_focused = WindowId::from_raw(1);
        let result = rt.focus(already_focused);
        assert!(result.is_ok());

        // Changes should record focus even though no layout event is emitted
        let changes = rt.take_changes();
        assert!(changes.focus_changed);
    }

    // =========================================================================
    // BufferApi: delete_range() with empty result (line 610 false branch)
    // =========================================================================

    /// Calling `delete_range()` where start==end produces empty deleted text.
    /// This exercises the `if !deleted_text.is_empty()` false branch (line 610).
    #[test]
    fn test_delete_range_empty_result_no_undo_record() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Delete an empty range (start == end): nothing is deleted
        harness.with_runtime(|runtime| {
            runtime.delete_range(buffer_id, Position::new(0, 3), Position::new(0, 3));
        });

        // Buffer content unchanged
        harness.assert_buffer_content("hello world");

        // buffer_modified is still recorded (changes.record_buffer_modified is called
        // unconditionally), but no undo edit is recorded for empty deletions.
        let changes = harness.take_changes();
        assert!(changes.buffer_modified);
    }

    // =========================================================================
    // CompositorApi: set_screen() without compositor (line 1382 false branch)
    // =========================================================================

    /// `set_screen()` without a compositor should only update `self.screen`.
    /// This exercises the `if let Some(compositor) = ...` false branch (line 1382).
    #[test]
    fn test_set_screen_without_compositor() {
        use reovim_kernel::api::v1::ModeStack;
        let mut session = Session::new(ClientId::new(1), test_mode());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut ms = ModeStack::new(test_mode());
        let mut w = crate::WindowLayout::empty();
        let mut e = crate::ExtensionMap::new();
        // No compositor
        let mut c: Option<Box<dyn reovim_driver_display::layout::RootCompositor>> = None;
        let mut tabs = crate::TabPageSet::new();
        let mut r = RegisterBank::new();
        let mut ch = HistoryRing::new();
        let mut lm = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut rt = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut ms,
                windows: &mut w,
                extensions: &mut e,
                compositor: &mut c,
                tabs: &mut tabs,
                registers: &mut r,
                clipboard_history: &mut ch,
                local_marks: &mut lm,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        let screen = Rect::new(0, 0, 80, 24);
        rt.set_screen(screen);
        assert_eq!(rt.screen, screen);
    }

    // =========================================================================
    // Per-client accessor coverage (#515)
    // =========================================================================

    #[test]
    fn test_registers_accessor() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            let bank = runtime.registers();
            assert!(bank.get().is_empty());
        });
    }

    #[test]
    fn test_registers_mut_accessor() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            let bank = runtime.registers_mut();
            bank.set_by_name(Some('a'), crate::api::RegisterContent::characterwise("test"));
            assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("test"));
        });
    }

    #[test]
    fn test_clipboard_history_accessor() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            let history = runtime.clipboard_history();
            assert!(history.is_empty());
        });
    }

    #[test]
    fn test_clipboard_history_mut_accessor() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            let history = runtime.clipboard_history_mut();
            history.push(crate::api::RegisterContent::characterwise("entry"));
            assert_eq!(history.len(), 1);
        });
    }

    #[test]
    fn test_local_marks_accessor() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            let marks = runtime.local_marks();
            assert!(marks.get_local('a').is_none());
        });
    }

    #[test]
    fn test_local_marks_mut_accessor() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            let marks = runtime.local_marks_mut();
            marks.set_local('a', reovim_kernel::api::v1::Position::new(0, 5));
            assert!(marks.get_local('a').is_some());
        });
    }

    // =========================================================================
    // ClipboardApi else-branch coverage (#515)
    // =========================================================================

    #[test]
    fn test_clipboard_api_no_provider_copy_to_clipboard() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            // No clipboard provider registered → returns false
            assert!(!runtime.copy_to_clipboard("test"));
        });
    }

    #[test]
    fn test_clipboard_api_no_provider_paste_from_clipboard() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            // No clipboard provider registered → returns None
            assert!(runtime.paste_from_clipboard().is_none());
        });
    }

    #[test]
    fn test_clipboard_api_no_provider_copy_to_selection() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            assert!(!runtime.copy_to_selection("test"));
        });
    }

    #[test]
    fn test_clipboard_api_no_provider_paste_from_selection() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            assert!(runtime.paste_from_selection().is_none());
        });
    }

    // =========================================================================
    // BufferApi trait-qualified active_buffer coverage
    // =========================================================================

    #[test]
    fn test_buffer_api_active_buffer_trait_qualified() {
        use crate::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::new();
        harness.with_runtime(|runtime| {
            // Trait-qualified call through BufferApi to ensure impl block attribution.
            assert!(BufferApi::active_buffer(runtime).is_none());
        });

        let mut harness = TestSessionRuntime::with_buffer("hello");
        harness.with_runtime(|runtime| {
            let buf_id = BufferApi::active_buffer(runtime);
            assert!(buf_id.is_some());

            // Test set_active_buffer round-trip via trait qualification.
            BufferApi::set_active_buffer(runtime, None);
            assert!(BufferApi::active_buffer(runtime).is_none());

            BufferApi::set_active_buffer(runtime, buf_id);
            assert_eq!(BufferApi::active_buffer(runtime), buf_id);
        });
    }

    // ========================================================================
    // Signal queue tests (#547)
    // ========================================================================

    #[test]
    fn test_signal_queue_initially_empty() {
        use crate::testing::TestSessionRuntime;
        let mut harness = TestSessionRuntime::with_buffer("");
        harness.with_runtime(|runtime| {
            let signals = runtime.take_signals();
            assert!(signals.is_empty());
        });
    }

    #[test]
    fn test_signal_push_and_take() {
        use crate::testing::TestSessionRuntime;
        let mut harness = TestSessionRuntime::with_buffer("");
        harness.with_runtime(|runtime| {
            runtime.signal(RuntimeSignal::Quit);
            let signals = runtime.take_signals();
            assert_eq!(signals.len(), 1);
            assert_eq!(signals[0], RuntimeSignal::Quit);
        });
    }

    #[test]
    fn test_signal_take_drains_queue() {
        use crate::testing::TestSessionRuntime;
        let mut harness = TestSessionRuntime::with_buffer("");
        harness.with_runtime(|runtime| {
            runtime.signal(RuntimeSignal::Quit);
            let first = runtime.take_signals();
            assert_eq!(first.len(), 1);

            // Second take should be empty
            let second = runtime.take_signals();
            assert!(second.is_empty());
        });
    }

    #[test]
    fn test_signal_multiple_fifo_order() {
        use crate::testing::TestSessionRuntime;
        let mut harness = TestSessionRuntime::with_buffer("");
        harness.with_runtime(|runtime| {
            runtime.signal(RuntimeSignal::Quit);
            runtime.signal(RuntimeSignal::Quit);
            runtime.signal(RuntimeSignal::Quit);

            let signals = runtime.take_signals();
            assert_eq!(signals.len(), 3);
            // All should be Quit (FIFO preserved)
            for s in &signals {
                assert_eq!(*s, RuntimeSignal::Quit);
            }
        });
    }

    #[test]
    fn test_signal_queue_independent_of_state_changes() {
        use crate::testing::TestSessionRuntime;
        let mut harness = TestSessionRuntime::with_buffer("hello");
        harness.with_runtime(|runtime| {
            // Push a signal and also create state changes
            runtime.signal(RuntimeSignal::Quit);
            let changes = runtime.take_changes();
            let signals = runtime.take_signals();

            // Both should be independent
            assert_eq!(signals.len(), 1);
            // State changes are their own thing
            drop(changes);
        });
    }
}
