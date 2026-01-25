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
        layout::{LayerId, WindowPlacement},
    },
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{
        BufferId, CommandId, Edit, KernelContext, ModeId, Position,
        SelectionMode as KernelSelectionMode, UndoResult, WindowId,
    },
};

use crate::{
    Session, SessionExtension, Window,
    api::{
        BufferApi, BufferError, ChangeTracker, CommandApi, CommandExecutor, CompositorApi,
        CompositorError, ExtensionApi, ModeApi, ModeError, RegisterApi, RegisterContent, Selection,
        SelectionMode, StateChanges, UndoApi, WindowApi, WindowError,
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
/// [`take_changes`]: ChangeTracker::take_changes
pub struct SessionRuntime<'a> {
    /// Per-session state (mode stack, windows, extensions, compositor).
    session: &'a mut Session,
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
    /// Create a new runtime.
    ///
    /// The compositor is accessed via `session.compositor`.
    /// Use `session.set_compositor()` before creating the runtime
    /// for full window management support.
    pub fn new(
        session: &'a mut Session,
        kernel: &'a KernelContext,
        executor: &'a dyn CommandExecutor,
    ) -> Self {
        let screen = {
            let (width, height) = session.terminal_size();
            Rect::new(0, 0, width, height)
        };
        Self {
            session,
            kernel,
            executor,
            screen,
            changes: StateChanges::new(),
        }
    }

    /// Check if compositor is available.
    #[must_use]
    pub fn has_compositor(&self) -> bool {
        self.session.compositor.is_some()
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
        // Get from window displaying this buffer (window cursor)
        self.session
            .windows
            .windows
            .iter()
            .find(|w| w.buffer_id == Some(buffer))
            .map(|w| Position::new(w.cursor.line, w.cursor.column))
    }

    fn buffer_position(&self, buffer: BufferId) -> Option<Position> {
        // Get from buffer's internal position (kernel)
        self.kernel
            .buffers
            .get(buffer)
            .map(|buf| buf.read().position())
    }

    fn set_buffer_position(&mut self, buffer: BufferId, pos: Position) {
        // Set buffer's internal position (kernel)
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            buf.write().set_position(pos);
        }
    }

    fn buffer_line_len(&self, buffer: BufferId, line: usize) -> Option<usize> {
        self.kernel
            .buffers
            .get(buffer)
            .and_then(|buf| buf.read().line_len(line))
    }

    #[allow(clippy::significant_drop_tightening)]
    fn selection(&self, buffer: BufferId) -> Option<Selection> {
        let buf_arc = self.kernel.buffers.get(buffer)?;
        let buf = buf_arc.read();

        let selection = buf.selection();
        if !selection.is_active() {
            return None;
        }

        let anchor = selection.anchor;
        let cursor = buf.position();

        // Normalize: start should be before end
        let (start, end) = if anchor <= cursor {
            (anchor, cursor)
        } else {
            (cursor, anchor)
        };

        // Convert kernel SelectionMode to API SelectionMode
        let mode = match selection.mode() {
            KernelSelectionMode::Character => SelectionMode::Character,
            KernelSelectionMode::Line => SelectionMode::Line,
            KernelSelectionMode::Block => SelectionMode::Block,
        };

        Some(Selection::new(start, end, mode))
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
            let cursor_before = {
                let b = buf.read();
                b.position()
            };

            buf.write().insert_at(pos, text);

            let cursor_after = {
                let b = buf.read();
                b.position()
            };

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
            let cursor_before = {
                let b = buf.read();
                b.position()
            };

            let deleted_text = {
                let mut b = buf.write();
                b.delete_range(start, end)
            };

            let cursor_after = {
                let b = buf.read();
                b.position()
            };

            // Record edit for undo
            if !deleted_text.is_empty() {
                let edit = Edit::Delete {
                    position: start,
                    text: deleted_text,
                };
                self.record_edit(buffer, vec![edit], cursor_before, cursor_after);
            }

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

    fn set_selection(&mut self, buffer: BufferId, sel: Option<Selection>) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            let mut buf = buf.write();
            match sel {
                Some(selection) => {
                    // Convert API SelectionMode to kernel SelectionMode
                    let mode = match selection.mode {
                        SelectionMode::Character => KernelSelectionMode::Character,
                        SelectionMode::Line => KernelSelectionMode::Line,
                        SelectionMode::Block => KernelSelectionMode::Block,
                    };
                    // Start selection at the start position with given mode
                    // The "end" is determined by cursor position
                    buf.selection_mut().start(selection.start, mode);
                }
                None => {
                    // Clear the selection
                    buf.selection_mut().clear();
                }
            }
        }
        self.changes.record_selection_change(buffer);
    }

    fn swap_selection_ends(&mut self, buffer: BufferId) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            let mut buf = buf.write();
            if buf.selection().is_active() {
                // Swap anchor and cursor
                let old_anchor = buf.selection().anchor;
                let cursor = buf.position();
                buf.selection_mut().anchor = cursor;
                buf.set_position(old_anchor);
            }
        }
        self.changes.record_selection_change(buffer);
    }

    fn set_selection_mode(&mut self, buffer: BufferId, mode: SelectionMode) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            let mut buf = buf.write();
            if buf.selection().is_active() {
                let kernel_mode = match mode {
                    SelectionMode::Character => KernelSelectionMode::Character,
                    SelectionMode::Line => KernelSelectionMode::Line,
                    SelectionMode::Block => KernelSelectionMode::Block,
                };
                buf.selection_mut().set_mode(kernel_mode);
            }
        }
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
            buf.set_position(result.cursor);
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
            buf.set_position(result.cursor);
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

    fn record_cursor_move(&mut self, buffer: BufferId) {
        self.changes.record_cursor_move(buffer);
    }
}

// === CompositorApi ===

impl CompositorApi for SessionRuntime<'_> {
    fn navigate(&self, direction: NavigateDirection) -> Result<WindowId, CompositorError> {
        let compositor = self
            .session
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
        Ok(new_window)
    }

    fn close_current_window(&mut self) -> Result<WindowId, CompositorError> {
        use reovim_driver_display::layout::Zone;

        let compositor = self
            .session
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
        Ok(neighbor)
    }

    fn close_others(&mut self) -> Result<(), CompositorError> {
        use reovim_driver_display::layout::Zone;

        let compositor = self
            .session
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

        // Close all other windows
        for window in windows {
            layer.close_tiled(window);
            self.changes.record_window_closed(window);
        }

        Ok(())
    }

    fn resize(&mut self, direction: NavigateDirection, delta: i16) -> Result<(), CompositorError> {
        let compositor = self
            .session
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
        Ok(())
    }

    fn equalize(&mut self) -> Result<(), CompositorError> {
        let compositor = self
            .session
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
        Ok(())
    }

    fn cycle(&self, forward: bool) -> Result<WindowId, CompositorError> {
        let compositor = self
            .session
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
            .compositor
            .as_mut()
            .ok_or(CompositorError::NoActiveLayer)?;

        // set_focus also activates the layer containing the window
        compositor.set_focus(window);
        self.changes.record_focus_change();
        Ok(())
    }

    fn focused_window(&self) -> Option<WindowId> {
        self.session.compositor.as_ref()?.focused()
    }

    fn compositor_window_count(&self) -> usize {
        self.session
            .compositor
            .as_ref()
            .map_or(0, |c| c.window_count())
    }

    fn arrange(&self, screen: Rect) -> Vec<WindowPlacement> {
        self.session
            .compositor
            .as_ref()
            .map_or_else(Vec::new, |c| c.composite(screen).placements)
    }

    fn active_layer(&self) -> Option<LayerId> {
        self.session.compositor.as_ref()?.active_layer()
    }

    fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
        if let Some(compositor) = self.session.compositor.as_mut() {
            compositor.set_screen(screen);
        }
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
            _kernel: &mut KernelContext,
        ) -> Option<CommandResult> {
            Some(CommandResult::Success)
        }
    }

    #[test]
    fn test_mode_api() {
        let mut session = Session::new(ClientId::new(1), test_mode());
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
        let mut session = Session::new(ClientId::new(1), test_mode());
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

        let mut session = Session::new(ClientId::new(1), test_mode());
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
        let mut session = Session::new(ClientId::new(1), test_mode());
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

    #[test]
    fn test_selection_api() {
        use crate::testing::TestSessionRuntime;

        // Create test runtime with a buffer
        let mut test = TestSessionRuntime::with_buffer("hello world");

        // Get the buffer ID
        let buffer_id = test.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Set buffer position to column 5 (at 'w')
        test.with_runtime(|runtime| {
            runtime.set_buffer_position(buffer_id, Position::new(0, 5));
        });

        // Selection is None initially
        let sel = test.with_runtime(|runtime| runtime.selection(buffer_id));
        assert!(sel.is_none());

        // Start a selection at position (0, 0) in character mode via kernel
        {
            let buf = test.kernel().buffers.get(buffer_id).unwrap();
            buf.write()
                .selection_mut()
                .start(Position::new(0, 0), KernelSelectionMode::Character);
        }

        // Now selection should be Some
        let sel = test.with_runtime(|runtime| runtime.selection(buffer_id));
        assert!(sel.is_some());
        let sel = sel.unwrap();
        assert_eq!(sel.start, Position::new(0, 0));
        assert_eq!(sel.end, Position::new(0, 5)); // cursor position
        assert_eq!(sel.mode, SelectionMode::Character);

        // Clear selection via set_selection
        test.with_runtime(|runtime| {
            runtime.set_selection(buffer_id, None);
        });

        // Selection should be None again
        let sel = test.with_runtime(|runtime| runtime.selection(buffer_id));
        assert!(sel.is_none());

        // Check changes were recorded
        assert!(test.changes().selection_changed);
    }

    #[test]
    #[allow(clippy::significant_drop_tightening)]
    fn test_selection_api_set_selection() {
        use crate::testing::TestSessionRuntime;

        // Create test runtime with a buffer
        let mut test = TestSessionRuntime::with_buffer("hello world");

        // Get the buffer ID
        let buffer_id = test.with_runtime(|runtime| runtime.active_buffer().unwrap());

        // Set selection via API
        test.with_runtime(|runtime| {
            let sel = Selection::line(Position::new(0, 2), Position::new(0, 8));
            runtime.set_selection(buffer_id, Some(sel));
        });

        // Verify kernel state was updated
        {
            let buf = test.kernel().buffers.get(buffer_id).unwrap();
            let buf = buf.read();
            let kernel_sel = buf.selection();
            assert!(kernel_sel.is_active());
            assert_eq!(kernel_sel.anchor, Position::new(0, 2));
            assert_eq!(kernel_sel.mode(), KernelSelectionMode::Line);
        }
    }

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
