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
    reovim_driver_layout::{
        LayerId, NavigateDirection, OverlayConstraints, Rect, SplitDirection, WindowPlacement,
    },
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{
        BufferId, ByteEdit, CommandId, KernelContext, ModeId, OptionValue, TabId, WindowId,
        events::kernel::{
            CursorMoved, LayoutChangeKind, LayoutChanged, SplitDirection as KernelSplitDirection,
        },
    },
    reovim_provider_text::TextBufferRegistry,
    reovim_types_text::{Edit, Position, UndoResult},
};

use crate::{
    ByteUndoRegistry, Selection, Session, SessionExtension, Window,
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
    /// Shared session state (template compositor, shared uppercase marks, home mode).
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
    compositor: &'a mut Option<Box<dyn reovim_driver_layout::RootCompositor>>,
    /// Per-client tab pages (#401).
    tabs: &'a mut crate::TabPageSet,
    /// Per-client register storage (#515).
    ///
    /// Each client owns their own registers (unnamed, named a-z/A-Z).
    /// System clipboard (+, *) remains shared via `ClipboardProvider`.
    registers: &'a mut reovim_types_text::RegisterBank,
    /// Per-client clipboard history ring (#515).
    ///
    /// Tracks yank/delete history for numbered registers 0-9.
    clipboard_history: &'a mut reovim_types_text::HistoryRing,
    /// Per-client local marks (a-z, per-client special marks) (#515).
    local_marks: &'a mut crate::MarkBank,
    /// Per-client jump list for Ctrl-O / Ctrl-I navigation (#654).
    jumplist: &'a mut crate::Jumplist,
    /// Per-client active buffer (#471).
    ///
    /// Each client tracks which buffer they are viewing independently.
    active_buffer: &'a mut Option<reovim_kernel::api::v1::BufferId>,
    /// Kernel context (buffers, options, services).
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
    /// Cursor position snapshot for `CursorMoved` event emission (#664).
    ///
    /// Captured at construction time (pre-command cursor position).
    /// Updated after each `CursorMoved` emission for multi-move commands.
    cursor_snapshot: Option<(u32, u32)>,
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
    /// * `kernel` - Kernel context (buffers, options, services)
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
        // Snapshot cursor position before any command executes (#664).
        // Used as `from` in CursorMoved event emission.
        #[allow(clippy::cast_possible_truncation)]
        let cursor_snapshot = client
            .windows
            .active()
            .map(|w| (w.cursor.line as u32, w.cursor.column as u32));
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
            jumplist: client.jumplist,
            active_buffer: client.active_buffer,
            kernel,
            executor,
            screen,
            changes: StateChanges::new(),
            signals: Vec::new(),
            command_depth: 0,
            cursor_snapshot,
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
    /// * `kernel` - Kernel context (buffers, options, services)
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
        #[allow(clippy::cast_possible_truncation)]
        let cursor_snapshot = client
            .windows
            .active()
            .map(|w| (w.cursor.line as u32, w.cursor.column as u32));
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
            jumplist: client.jumplist,
            active_buffer: client.active_buffer,
            kernel,
            executor,
            screen,
            changes: StateChanges::new(),
            signals: Vec::new(),
            command_depth: 0,
            cursor_snapshot,
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

    /// Get a text buffer by ID from the session-layer text registry (#740).
    ///
    /// Resolves text buffers through `TextBufferRegistry`. The kernel's
    /// `BufferManager` only stores `dyn KernelBuffer` (byte-only); all
    /// text-specific access goes through this method.
    ///
    /// Use this instead of `kernel().buffers.get(id)` for text-specific access.
    #[must_use]
    pub fn text_buffer(
        &self,
        id: BufferId,
    ) -> Option<std::sync::Arc<reovim_arch::sync::RwLock<dyn reovim_provider_text::BufferOps>>>
    {
        if let Some(reg) = self.kernel.services.get::<TextBufferRegistry>()
            && let Some(buf) = reg.get(id)
        {
            return Some(buf);
        }
        None
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
        F: FnOnce(&dyn reovim_provider_text::BufferOps) -> R,
    {
        let buf_arc = self.text_buffer(buffer)?;
        let buf = buf_arc.read();
        Some(f(&*buf))
    }

    /// Read from a buffer as `&dyn TextGeometry`.
    ///
    /// Use this for motions, text objects, and any read-only text access.
    pub fn with_text_geometry<F, R>(&self, buffer: BufferId, f: F) -> Option<R>
    where
        F: FnOnce(&dyn reovim_types_text::TextGeometry) -> R,
    {
        let buf_arc = self.text_buffer(buffer)?;
        let buf = buf_arc.read();
        Some(f(buf.as_text_geometry()))
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
    pub const fn registers(&self) -> &reovim_types_text::RegisterBank {
        self.registers
    }

    /// Get per-client registers mutably (#515).
    #[allow(clippy::missing_const_for_fn)]
    pub fn registers_mut(&mut self) -> &mut reovim_types_text::RegisterBank {
        self.registers
    }

    /// Get per-client clipboard history (#515).
    #[must_use]
    pub const fn clipboard_history(&self) -> &reovim_types_text::HistoryRing {
        self.clipboard_history
    }

    /// Get per-client clipboard history mutably (#515).
    #[allow(clippy::missing_const_for_fn)]
    pub fn clipboard_history_mut(&mut self) -> &mut reovim_types_text::HistoryRing {
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
        &mut reovim_types_text::RegisterBank,
        &mut reovim_types_text::HistoryRing,
    ) {
        (self.kernel, self.registers, self.clipboard_history)
    }

    /// Get per-client local marks (#515).
    #[must_use]
    pub const fn local_marks(&self) -> &crate::MarkBank {
        self.local_marks
    }

    /// Get per-client local marks mutably (#515).
    #[allow(clippy::missing_const_for_fn)]
    pub fn local_marks_mut(&mut self) -> &mut crate::MarkBank {
        self.local_marks
    }

    /// Get session-shared uppercase/global marks (A-Z).
    #[must_use]
    pub const fn global_marks(&self) -> &crate::MarkBank {
        self.session.global_marks()
    }

    /// Get session-shared uppercase/global marks (A-Z) mutably.
    #[allow(clippy::missing_const_for_fn)]
    pub fn global_marks_mut(&mut self) -> &mut crate::MarkBank {
        self.session.global_marks_mut()
    }

    /// Get per-client jump list (#654).
    #[must_use]
    pub const fn jumplist(&self) -> &crate::Jumplist {
        self.jumplist
    }

    /// Get per-client jump list mutably (#654).
    #[allow(clippy::missing_const_for_fn)]
    pub fn jumplist_mut(&mut self) -> &mut crate::Jumplist {
        self.jumplist
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
        self.text_buffer(buffer)
            .and_then(|buf| buf.read().line(line).map(String::from))
    }

    fn buffer_line_count(&self, buffer: BufferId) -> Option<usize> {
        self.text_buffer(buffer).map(|buf| buf.read().line_count())
    }

    fn buffer_line_len(&self, buffer: BufferId, line: usize) -> Option<usize> {
        self.text_buffer(buffer)
            .and_then(|buf| buf.read().line_len(line))
    }

    #[allow(clippy::significant_drop_tightening)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn buffer_text_range(
        &self,
        buffer: BufferId,
        start: Position,
        end: Position,
    ) -> Option<String> {
        let buf_arc = self.text_buffer(buffer)?;
        let buf = buf_arc.read();

        // Use line-based extraction via BufferOps trait (works for both
        // Rope and VirtualBuffer through dyn dispatch).
        Some(extract_text_range(start, end, buf.line_count(), |idx| {
            buf.line(idx).map(std::borrow::Cow::into_owned)
        }))
    }

    fn buffer_content(&self, buffer: BufferId) -> Option<String> {
        self.text_buffer(buffer).map(|buf| buf.read().content())
    }

    fn buffer_file_path(&self, buffer: BufferId) -> Option<String> {
        self.text_buffer(buffer)
            .and_then(|buf| buf.read().file_path().map(String::from))
    }

    fn is_buffer_modified(&self, buffer: BufferId) -> Option<bool> {
        self.text_buffer(buffer).map(|buf| buf.read().is_modified())
    }

    fn set_buffer_modified(&mut self, buffer: BufferId, modified: bool) {
        if let Some(buf) = self.text_buffer(buffer) {
            buf.write().set_modified(modified);
        }
    }

    fn insert_text(&mut self, buffer: BufferId, pos: Position, text: &str) {
        if let Some(buf) = self.text_buffer(buffer) {
            // Get cursor from per-client active window (#471)
            // Note: cursor_after will be set by runner from CommandResult
            let cursor_before = self.windows().active().map_or_else(
                || Position::new(0, 0),
                |w| Position::new(w.cursor.line, w.cursor.column),
            );

            // Compute byte offset BEFORE mutation (#655)
            let byte_offset = buf.read().position_to_byte(pos);

            buf.write().insert_at(pos, text);

            // Record byte-level edit for undo and codec index notification (#740)
            let byte_edit = ByteEdit::insert(byte_offset, text.as_bytes());
            if let Some(reg) = self.kernel.services.get::<ByteUndoRegistry>() {
                reg.push(buffer, vec![byte_edit.clone()]);
            }
            self.changes.record_byte_edit(buffer, byte_edit);

            // For undo, use cursor_before as cursor_after too (runner will update actual cursor)
            let cursor_after = cursor_before;

            // Record edit for undo - use record_edit_mine for per-client undo (#471)
            let edit = Edit::Insert {
                position: pos,
                text: text.to_string(),
            };
            self.record_edit_mine(buffer, vec![edit], cursor_before, cursor_after);

            // Emit BufferModified event for subscribers (#655)
            #[allow(clippy::cast_possible_truncation)]
            {
                use reovim_kernel::api::v1::events::kernel::{BufferModified, Modification};
                let modification = Modification::Insert {
                    start: (pos.line as u32, pos.column as u32),
                    text: text.to_string(),
                    start_byte: byte_offset,
                };
                self.kernel.event_bus.emit(BufferModified {
                    buffer_id: buffer.as_usize() as u64,
                    modification: modification.clone(),
                });
                self.changes
                    .record_buffer_modified_with_edit(buffer, modification);
            }
        }
    }

    fn delete_range(&mut self, buffer: BufferId, start: Position, end: Position) {
        if let Some(buf) = self.text_buffer(buffer) {
            // Get cursor from per-client active window (#471)
            let cursor_before = self.windows().active().map_or_else(
                || Position::new(0, 0),
                |w| Position::new(w.cursor.line, w.cursor.column),
            );

            // Compute byte offset BEFORE mutation (#655)
            let byte_offset = buf.read().position_to_byte(start);

            let deleted_text = {
                let mut b = buf.write();
                b.delete_range(start, end)
            };

            // Record byte-level edit for undo and codec index notification (#740)
            if !deleted_text.is_empty() {
                let byte_edit = ByteEdit::delete(byte_offset, deleted_text.as_bytes());
                if let Some(reg) = self.kernel.services.get::<ByteUndoRegistry>() {
                    reg.push(buffer, vec![byte_edit.clone()]);
                }
                self.changes.record_byte_edit(buffer, byte_edit);
            }

            // For undo, use cursor_before as cursor_after too (runner will update actual cursor)
            let cursor_after = cursor_before;

            // Record edit for undo - use record_edit_mine for per-client undo (#471)
            if deleted_text.is_empty() {
                self.changes.record_buffer_modified(buffer);
            } else {
                let edit = Edit::Delete {
                    position: start,
                    text: deleted_text.clone(),
                };
                self.record_edit_mine(buffer, vec![edit], cursor_before, cursor_after);

                // Emit BufferModified event for subscribers (#440)
                #[allow(clippy::cast_possible_truncation)]
                {
                    use reovim_kernel::api::v1::events::kernel::{BufferModified, Modification};
                    let modification = Modification::Delete {
                        start: (start.line as u32, start.column as u32),
                        end: (end.line as u32, end.column as u32),
                        text: deleted_text,
                        start_byte: byte_offset,
                    };
                    self.kernel.event_bus.emit(BufferModified {
                        buffer_id: buffer.as_usize() as u64,
                        modification: modification.clone(),
                    });
                    self.changes
                        .record_buffer_modified_with_edit(buffer, modification);
                }
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn replace_content(&mut self, buffer: BufferId, content: &str) {
        if let Some(buf) = self.text_buffer(buffer) {
            let cursor_before = self.windows().active().map_or_else(
                || Position::new(0, 0),
                |w| Position::new(w.cursor.line, w.cursor.column),
            );

            let old_content = buf.read().content();
            buf.write().set_content(content);

            // Record byte-level edit for undo and codec index notification (#740)
            let byte_edit = ByteEdit::replace(0, old_content.as_bytes(), content.as_bytes());
            if let Some(reg) = self.kernel.services.get::<ByteUndoRegistry>() {
                reg.push(buffer, vec![byte_edit.clone()]);
            }
            self.changes.record_byte_edit(buffer, byte_edit);

            // Record for undo as a delete-all + insert-all
            let edits = vec![
                Edit::Delete {
                    position: Position::new(0, 0),
                    text: old_content,
                },
                Edit::Insert {
                    position: Position::new(0, 0),
                    text: content.to_string(),
                },
            ];
            self.record_edit_mine(buffer, edits, cursor_before, cursor_before);

            // Emit BufferModified for subscribers
            #[allow(clippy::cast_possible_truncation)]
            {
                use reovim_kernel::api::v1::events::kernel::{BufferModified, Modification};
                let modification = Modification::FullReplace;
                self.kernel.event_bus.emit(BufferModified {
                    buffer_id: buffer.as_usize() as u64,
                    modification: modification.clone(),
                });
                self.changes
                    .record_buffer_modified_with_edit(buffer, modification);
            }
        }
    }

    fn create_buffer(&mut self, name: Option<&str>, content: &str) -> BufferId {
        let mut buffer = reovim_provider_text::Buffer::from_string(content);
        if let Some(name) = name {
            buffer.set_file_path(Some(name.to_string()));
        }
        let arc = std::sync::Arc::new(reovim_arch::sync::RwLock::new(buffer));
        // Register in text buffer registry (session-layer text access, #740).
        if let Some(reg) = self.kernel.services.get::<TextBufferRegistry>() {
            reg.register(arc.clone());
        }
        let id = self.kernel.buffers.register(arc);
        self.changes.record_buffer_created(id);
        id
    }

    fn delete_buffer(&mut self, buffer: BufferId) -> Result<(), BufferError> {
        if self.kernel.buffers.count() <= 1 {
            return Err(BufferError::CannotDeleteLastBuffer);
        }
        if self.kernel.buffers.unregister(buffer).is_none() {
            return Err(BufferError::NotFound(buffer));
        }
        // Unregister from text buffer registry (#740).
        if let Some(reg) = self.kernel.services.get::<TextBufferRegistry>() {
            reg.unregister(buffer);
        }
        // Clean up byte undo log (#740).
        if let Some(reg) = self.kernel.services.get::<ByteUndoRegistry>() {
            reg.remove(buffer);
        }
        self.changes.record_buffer_deleted(buffer);
        Ok(())
    }

    fn rename_buffer(&mut self, buffer: BufferId, new_name: &str) {
        if let Some(buf) = self.text_buffer(buffer) {
            buf.write().set_file_path(Some(new_name.to_string()));
            self.changes
                .record_buffer_renamed(buffer, new_name.to_string());
        }
    }

    fn buffer_capabilities(
        &self,
        buffer: BufferId,
    ) -> Option<reovim_provider_text::BufferCapabilities> {
        self.text_buffer(buffer)
            .map(|buf| buf.read().buffer_capabilities())
    }

    fn buffer_write_to(
        &self,
        buffer: BufferId,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        self.text_buffer(buffer).map_or_else(
            || Err(std::io::Error::new(std::io::ErrorKind::NotFound, "buffer not found")),
            |buf| buf.read().write_to(writer),
        )
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
        if self.windows.remove(window) {
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
        if self.text_buffer(buffer).is_none() {
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

#[cfg_attr(coverage_nightly, coverage(off))]
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
        let Some(buf) = self.text_buffer(buffer) else {
            return;
        };
        let mut buf = buf.write();
        for edit in edits {
            match edit {
                Edit::Insert { position, text } => {
                    buf.insert_at(*position, text);
                }
                Edit::Delete { position, text } => {
                    let byte_start = buf.position_to_byte(*position);
                    let end = buf.byte_to_position(byte_start + text.len());
                    buf.delete_range(*position, end);
                }
            }
        }
    }
}

impl UndoApi for SessionRuntime<'_> {
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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
        // #664: Emit CursorMoved event for subscribers (illuminate, etc.).
        // `cursor_snapshot` holds the pre-command position (captured at construction)
        // or the post-previous-move position (updated after each emission).
        #[allow(clippy::cast_possible_truncation)]
        if let Some(window) = self.windows.active() {
            let to = (window.cursor.line as u32, window.cursor.column as u32);
            let from = self.cursor_snapshot.unwrap_or(to);
            self.kernel.event_bus.emit(CursorMoved {
                buffer_id: buffer.as_usize() as u64,
                from,
                to,
            });
            self.cursor_snapshot = Some(to);
        }

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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

        // Add new window to per-client WindowLayout inheriting cursor/viewport
        // from source. The compositor tracks geometry; WindowLayout tracks
        // buffer/cursor/selection. (#692)
        if let Some(source) = self.windows.get(from) {
            let win = Window::split_from(new_window, source);
            self.windows.add(win);
        }

        // Vim behavior: focus stays on original window after split.
        // Re-focus compositor back to original (split_tiled auto-focused new).
        if let Some(compositor) = self.compositor.as_mut()
            && let Some(active) = compositor.active_layer()
            && let Some(layer) = compositor.layer_compositor_mut(active)
        {
            layer.set_focus(from);
        }
        self.windows.set_active(from);

        self.changes.record_window_created(new_window);

        // Emit layout changed event
        self.emit_layout_event(LayoutChangeKind::Split {
            new_window: new_window.as_usize() as u64,
            direction: to_kernel_split_direction(direction),
        });

        Ok(new_window)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn close_current_window(&mut self) -> Result<WindowId, CompositorError> {
        use reovim_driver_layout::Zone;

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

        // Remove closed window from per-client WindowLayout
        self.windows.remove(current);
        self.windows.set_active(neighbor);

        self.changes.record_window_closed(current);

        // Emit layout changed event
        self.emit_layout_event(LayoutChangeKind::Close {
            closed_window: current.as_usize() as u64,
            new_focus: Some(neighbor.as_usize() as u64),
        });

        Ok(neighbor)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn close_others(&mut self) -> Result<(), CompositorError> {
        use reovim_driver_layout::Zone;

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
            self.windows.remove(*window);
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn focus(&mut self, window: WindowId) -> Result<(), CompositorError> {
        let compositor = self
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        // Get the previous focused window before changing focus
        let previous_focus = compositor.focused();

        // set_focus also activates the layer containing the window
        compositor.set_focus(window);

        // Sync per-client WindowLayout active window and active_buffer.
        // Without this, cursor notifications use the old window's buffer_id
        // and build_cursor_notification finds the wrong window.
        self.windows.set_active(window);
        if let Some(buffer_id) = self.windows.active().and_then(|w| w.buffer_id) {
            *self.active_buffer = Some(buffer_id);
        }

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

/// Convert a char-column index to a byte offset within a `&str`.
fn char_col_to_byte(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |(b, _)| b)
}

/// Extract text from a line range using a generic line accessor.
///
/// Used by `buffer_text_range` for both `Buffer` and `VirtualBuffer`.
fn extract_text_range(
    start: Position,
    end: Position,
    _line_count: usize,
    line_fn: impl Fn(usize) -> Option<String>,
) -> String {
    let mut result = String::new();

    if start.line == end.line {
        if let Some(line) = line_fn(start.line) {
            let char_len = line.chars().count();
            let start_col = start.column.min(char_len);
            let end_col = end.column.min(char_len);
            if start_col < end_col {
                let sb = char_col_to_byte(&line, start_col);
                let eb = char_col_to_byte(&line, end_col);
                result.push_str(&line[sb..eb]);
            }
        }
    } else {
        if let Some(line) = line_fn(start.line) {
            let char_len = line.chars().count();
            let start_col = start.column.min(char_len);
            let sb = char_col_to_byte(&line, start_col);
            result.push_str(&line[sb..]);
            result.push('\n');
        }
        for line_idx in (start.line + 1)..end.line {
            if let Some(line) = line_fn(line_idx) {
                result.push_str(&line);
                result.push('\n');
            }
        }
        if let Some(line) = line_fn(end.line) {
            let char_len = line.chars().count();
            let end_col = end.column.min(char_len);
            let eb = char_col_to_byte(&line, end_col);
            result.push_str(&line[..eb]);
        }
    }

    result
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
