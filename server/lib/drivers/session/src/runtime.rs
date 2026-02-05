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
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_display::{
        NavigateDirection, Rect, SplitDirection,
        layout::{LayerId, OverlayConstraints, WindowPlacement},
    },
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{
        BufferId, CommandId, Edit, KernelContext, ModeId, OptionValue, Position, UndoResult,
        WindowId,
        events::kernel::{LayoutChangeKind, LayoutChanged, SplitDirection as KernelSplitDirection},
    },
};

use crate::{
    Session, SessionExtension, Window,
    api::{
        BufferApi, BufferError, ChangeTracker, CommandApi, CommandExecutor, CompositorApi,
        CompositorError, ExtensionApi, ModeApi, ModeError, RegisterApi, RegisterContent,
        StateChanges, UndoApi, WindowApi, WindowError,
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
    /// Shared session state (compositor, `terminal_size`, `active_buffer`).
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
    /// Kernel context (buffers, registers, marks).
    kernel: &'a KernelContext,
    /// Command executor for looking up and running commands.
    executor: &'a dyn CommandExecutor,
    /// Cached screen size for compositor operations.
    screen: Rect,
    /// Accumulated changes - runner takes at end.
    changes: StateChanges,
}

impl<'a> SessionRuntime<'a> {
    /// Create a new runtime with per-client state (#471, #477).
    ///
    /// All per-client state is **required**. There are no fallbacks to shared
    /// session state. This enforces multi-client isolation at compile time.
    ///
    /// # Arguments
    ///
    /// * `session` - Shared session state (compositor, `terminal_size`, `active_buffer`)
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
    /// // Create runtime with per-client state
    /// let mut runtime = SessionRuntime::new(
    ///     &mut driver_session,
    ///     &mut editing_state.mode_stack,
    ///     &mut editing_state.windows,
    ///     &mut editing_state.extensions,
    ///     &kernel,
    ///     &executor,
    /// );
    ///
    /// // All operations use per-client state
    /// runtime.push_mode(insert_mode, ctx);     // Only affects this client
    /// runtime.windows().active();              // This client's active window
    /// runtime.ext_mut::<VimSessionState>();    // This client's vim state
    /// ```
    pub fn new(
        session: &'a mut Session,
        mode_stack: &'a mut reovim_kernel::api::v1::ModeStack,
        windows: &'a mut crate::WindowLayout,
        extensions: &'a mut crate::ExtensionMap,
        kernel: &'a KernelContext,
        executor: &'a dyn CommandExecutor,
    ) -> Self {
        let screen = {
            let (width, height) = session.terminal_size();
            Rect::new(0, 0, width, height)
        };
        Self {
            owner: None,
            session,
            mode_stack,
            windows,
            extensions,
            kernel,
            executor,
            screen,
            changes: StateChanges::new(),
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
    /// * `mode_stack` - Per-client mode stack
    /// * `windows` - Per-client window layout with cursors
    /// * `extensions` - Per-client module extensions
    /// * `kernel` - Kernel context (buffers, registers, marks)
    /// * `executor` - Command executor
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut runtime = SessionRuntime::with_owner(
    ///     client_id,
    ///     &mut driver_session,
    ///     &mut editing_state.mode_stack,
    ///     &mut editing_state.windows,
    ///     &mut editing_state.extensions,
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
    pub fn with_owner(
        owner: crate::ClientId,
        session: &'a mut Session,
        mode_stack: &'a mut reovim_kernel::api::v1::ModeStack,
        windows: &'a mut crate::WindowLayout,
        extensions: &'a mut crate::ExtensionMap,
        kernel: &'a KernelContext,
        executor: &'a dyn CommandExecutor,
    ) -> Self {
        let screen = {
            let (width, height) = session.terminal_size();
            Rect::new(0, 0, width, height)
        };
        Self {
            owner: Some(owner),
            session,
            mode_stack,
            windows,
            extensions,
            kernel,
            executor,
            screen,
            changes: StateChanges::new(),
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

    // Note: has_client_mode_stack(), has_client_windows(), has_client_extensions()
    // have been removed in #471 Phase 0. Per-client state is now REQUIRED,
    // so these methods would always return true.

    /// Check if compositor is available.
    #[must_use]
    pub fn has_compositor(&self) -> bool {
        self.session.shared.compositor.is_some()
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

            // Record edit for undo
            let edit = Edit::Insert {
                position: pos,
                text: text.to_string(),
            };
            self.record_edit(buffer, vec![edit], cursor_before, cursor_after);

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

            // Record edit for undo
            if !deleted_text.is_empty() {
                let edit = Edit::Delete {
                    position: start,
                    text: deleted_text.clone(),
                };
                self.record_edit(buffer, vec![edit], cursor_before, cursor_after);

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
}

// === RegisterApi ===

impl RegisterApi for SessionRuntime<'_> {
    fn get_register(&self, name: Option<char>) -> Option<RegisterContent> {
        match name {
            // System clipboard (+)
            Some('+') => {
                if let Some(registry) = self.kernel.services.get::<ClipboardProviderRegistry>()
                    && let Some(provider) = registry.get(&ClipboardKey::Default)
                    && let Ok(Some(text)) = provider.paste_from_clipboard()
                {
                    return Some(RegisterContent::characterwise(text));
                }
                None
            }

            // Selection clipboard (*)
            Some('*') => {
                if let Some(registry) = self.kernel.services.get::<ClipboardProviderRegistry>()
                    && let Some(provider) = registry.get(&ClipboardKey::Default)
                    && let Ok(Some(text)) = provider.paste_from_selection()
                {
                    return Some(RegisterContent::characterwise(text));
                }
                None
            }

            // Numbered registers (0-9) - yank history
            Some(n) if n.is_ascii_digit() => {
                if let Some(registry) = self.kernel.services.get::<ClipboardProviderRegistry>()
                    && let Some(provider) = registry.get(&ClipboardKey::Default)
                {
                    return provider.get_numbered(n);
                }
                None
            }

            // Named registers (a-z, A-Z) and unnamed - use kernel's RegisterBank
            _ => self.kernel.registers.read().get_by_name(name).cloned(),
        }
    }

    fn set_register(&mut self, name: Option<char>, content: RegisterContent) {
        match name {
            // System clipboard (+)
            Some('+') => {
                if let Some(registry) = self.kernel.services.get::<ClipboardProviderRegistry>()
                    && let Some(provider) = registry.get(&ClipboardKey::Default)
                {
                    let _ = provider.copy_to_clipboard(&content.text);
                    return;
                }
                // Fallback: store in kernel's registers
                self.kernel.registers.write().set_by_name(name, content);
            }

            // Selection clipboard (*)
            Some('*') => {
                if let Some(registry) = self.kernel.services.get::<ClipboardProviderRegistry>()
                    && let Some(provider) = registry.get(&ClipboardKey::Default)
                {
                    let _ = provider.copy_to_selection(&content.text);
                    return;
                }
                // Fallback: store in kernel's registers
                self.kernel.registers.write().set_by_name(name, content);
            }

            // Numbered registers (0-9) are read-only (populated by history)
            Some(n) if n.is_ascii_digit() => {
                // Ignore writes to numbered registers - they're managed by history
            }

            // Named registers (a-z, A-Z) and unnamed - use kernel's RegisterBank
            _ => {
                self.kernel.registers.write().set_by_name(name, content);
            }
        }
    }
}

// === UndoApi ===

impl UndoApi for SessionRuntime<'_> {
    fn undo(&mut self, buffer: BufferId) -> Option<UndoResult> {
        let undo_provider = self
            .kernel
            .services
            .get::<UndoProviderRegistry>()?
            .get(&UndoKey::Buffer)?;

        let result = undo_provider.undo(buffer)?;

        // Apply the inverse edits to the buffer
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            let mut buf = buf.write();
            for edit in &result.edits {
                match edit {
                    Edit::Insert { position, text } => {
                        buf.insert_at(*position, text);
                    }
                    Edit::Delete { position, text } => {
                        buf.delete_at(*position, text.chars().count());
                    }
                }
            }
            // NOTE: Don't set kernel buffer cursor - it no longer exists (#471)
        }

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

        // Apply the edits to the buffer
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            let mut buf = buf.write();
            for edit in &result.edits {
                match edit {
                    Edit::Insert { position, text } => {
                        buf.insert_at(*position, text);
                    }
                    Edit::Delete { position, text } => {
                        buf.delete_at(*position, text.chars().count());
                    }
                }
            }
            // NOTE: Don't set kernel buffer cursor - it no longer exists (#471)
        }

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
}

// === CommandApi ===

impl CommandApi for SessionRuntime<'_> {
    fn execute_command(&mut self, cmd: CommandId, ctx: CommandContext) -> CommandResult {
        // Execute command via the injected executor.
        //
        // KernelContext uses interior mutability (Arc<RwLock<...>>), so
        // &KernelContext is sufficient for command execution.
        self.executor
            .execute(&cmd, &ctx, self.kernel)
            .unwrap_or_else(|| CommandResult::Error(format!("command not found: {cmd:?}")))
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
}

// === ChangeTracker ===

impl ChangeTracker for SessionRuntime<'_> {
    fn take_changes(&mut self) -> StateChanges {
        std::mem::take(&mut self.changes)
    }

    fn record_cursor_move(&mut self, buffer: BufferId) {
        self.changes.record_cursor_move(buffer);
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
        self.session.shared.compositor.as_ref()?.focused()
    }

    fn compositor_window_count(&self) -> usize {
        self.session
            .shared
            .compositor
            .as_ref()
            .map_or(0, |c| c.window_count())
    }

    fn arrange(&self, screen: Rect) -> Vec<WindowPlacement> {
        self.session
            .shared
            .compositor
            .as_ref()
            .map_or_else(Vec::new, |c| c.composite(screen).placements)
    }

    fn active_layer(&self) -> Option<LayerId> {
        self.session.shared.compositor.as_ref()?.active_layer()
    }

    fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
        if let Some(compositor) = self.session.shared.compositor.as_mut() {
            compositor.set_screen(screen);
        }
    }

    // =========================================================================
    // Float Zone Operations (#398)
    // =========================================================================

    fn toggle_float(&mut self) -> Result<(), CompositorError> {
        let compositor = self
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
            .session
            .shared
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
}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::ClientId, reovim_kernel::api::v1::ModuleId};

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
            _kernel: &KernelContext,
        ) -> Option<CommandResult> {
            Some(CommandResult::Success)
        }
    }

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

        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
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

        // #491: Session no longer has mode_stack field - use home_mode() from shared
        let session_home_mode = session.shared.home_mode().clone();

        // Use a scope to release mutable borrow before checking session
        {
            // Create runtime with per-client state (#471 Phase 0)
            let mut runtime = SessionRuntime::new(
                &mut session,
                &mut client_mode_stack,
                &mut client_windows,
                &mut client_extensions,
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

        // Runtime created with new() has no owner
        {
            let runtime = SessionRuntime::new(
                &mut session,
                &mut client_stack,
                &mut client_windows,
                &mut client_extensions,
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
                &mut client_stack,
                &mut client_windows,
                &mut client_extensions,
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
        let mut client2_stack = ModeStack::new(test_mode());
        let mut client2_windows = crate::WindowLayout::empty();
        let mut client2_extensions = crate::ExtensionMap::new();

        // Client 1 enters insert mode (#471 Phase 0: use new())
        {
            let mut runtime1 = SessionRuntime::new(
                &mut session,
                &mut client1_stack,
                &mut client1_windows,
                &mut client1_extensions,
                &kernel,
                &executor,
            );
            runtime1.push_mode(test_mode_2(), TransitionContext::new());
        }

        // Client 2 stays in normal mode (#471 Phase 0: use new())
        {
            let runtime2 = SessionRuntime::new(
                &mut session,
                &mut client2_stack,
                &mut client2_windows,
                &mut client2_extensions,
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

        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
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

        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
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

        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
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
    // New tests will be added in Phase 7/8.

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
}
