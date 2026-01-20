//! `AppStateRuntime` - adapts `AppState` fields to the `SessionApiDyn` interface.
//!
//! This wrapper enables the event loop to use `resolve_with_session` by
//! providing the session API traits over individual `AppState` fields.
//!
//! # Design
//!
//! ```text
//! AppStateRuntime
//! ├── ModeStack (from AppState)
//! ├── WindowRegistry (from AppState)
//! ├── KernelContext (from AppState)
//! ├── CommandRegistry (for future command execution)
//! └── StateChanges (accumulated changes)
//! ```
//!
//! Note: Extensions are NOT borrowed by this runtime - they're passed separately
//! to `resolve_with_session` to work around Rust's borrow rules.
//!
//! # Why Not Use `SessionRuntime`?
//!
//! `SessionRuntime` from the session driver expects a `Session` type, but
//! the runner uses `AppState`. Rather than refactoring `AppState` into
//! `Session`, we create this adapter that implements the same traits.

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_session::{
        PopResult, TransitionContext,
        api::{
            BufferApi, BufferError, ChangeTracker, CommandApi, ModeApi, ModeError, Selection,
            SelectionMode, StateChanges, WindowApi, WindowError,
        },
    },
    reovim_kernel::api::v1::{
        BufferId, CommandId, KernelContext, ModeId, ModeStack, Position,
        SelectionMode as KernelSelectionMode, WindowId,
    },
};

use crate::server::{registry::CommandRegistry, window::WindowRegistry};

/// Runtime that adapts `AppState` fields to the session API traits.
///
/// This enables the event loop to use `resolve_with_session` while keeping
/// the existing `AppState` structure. Extensions are passed separately.
pub struct AppStateRuntime<'a> {
    /// Mode stack for mode transitions.
    mode_stack: &'a mut ModeStack,
    /// Window registry for window operations.
    windows: &'a mut WindowRegistry,
    /// Active buffer ID.
    active_buffer: &'a mut Option<BufferId>,
    /// Kernel context for buffer operations.
    kernel: &'a KernelContext,
    /// Command registry for looking up commands.
    command_registry: &'a CommandRegistry,
    /// Accumulated changes - runner takes at end.
    changes: StateChanges,
}

impl<'a> AppStateRuntime<'a> {
    /// Create a new runtime from individual `AppState` fields.
    ///
    /// Extensions are NOT borrowed here - pass them separately to `resolve_with_session`.
    pub fn new(
        mode_stack: &'a mut ModeStack,
        windows: &'a mut WindowRegistry,
        active_buffer: &'a mut Option<BufferId>,
        kernel: &'a KernelContext,
        command_registry: &'a CommandRegistry,
    ) -> Self {
        Self {
            mode_stack,
            windows,
            active_buffer,
            kernel,
            command_registry,
            changes: StateChanges::new(),
        }
    }

    /// Find the first window displaying a given buffer.
    fn find_window_for_buffer(
        &self,
        buffer_id: BufferId,
    ) -> Option<reovim_driver_display::WindowId> {
        self.windows.windows().find(|&win_id| {
            self.windows
                .get(win_id)
                .is_some_and(|state| state.buffer_id == Some(buffer_id))
        })
    }
}

// === ModeApi ===

impl ModeApi for AppStateRuntime<'_> {
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

impl BufferApi for AppStateRuntime<'_> {
    fn active_buffer(&self) -> Option<BufferId> {
        *self.active_buffer
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
        // Find window displaying this buffer
        let win_id = self.find_window_for_buffer(buffer)?;
        self.windows
            .get(win_id)
            .map(|state| Position::new(state.cursor.line, state.cursor.column))
    }

    fn buffer_position(&self, buffer: BufferId) -> Option<Position> {
        self.kernel
            .buffers
            .get(buffer)
            .map(|buf| buf.read().position())
    }

    fn set_buffer_position(&mut self, buffer: BufferId, pos: Position) {
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
        if let Some(win_id) = self.find_window_for_buffer(buffer)
            && let Some(state) = self.windows.get_mut(win_id)
        {
            state.cursor.line = pos.line;
            state.cursor.column = pos.column;
            self.changes.record_cursor_move(buffer);
        }
    }

    fn set_selection(&mut self, buffer: BufferId, sel: Option<Selection>) {
        if let Some(buf) = self.kernel.buffers.get(buffer) {
            let mut buf = buf.write();
            match sel {
                Some(selection) => {
                    let mode = match selection.mode {
                        SelectionMode::Character => KernelSelectionMode::Character,
                        SelectionMode::Line => KernelSelectionMode::Line,
                        SelectionMode::Block => KernelSelectionMode::Block,
                    };
                    buf.selection_mut().start(selection.start, mode);
                }
                None => {
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

impl WindowApi for AppStateRuntime<'_> {
    #[allow(clippy::cast_lossless)]
    fn active_window(&self) -> Option<WindowId> {
        // Convert display WindowId (usize) to kernel WindowId (u64)
        self.windows
            .active_window()
            .map(|id| WindowId::from_raw(id.0 as u64))
    }

    fn window_count(&self) -> usize {
        self.windows.window_count()
    }

    fn window_buffer(&self, window: WindowId) -> Option<BufferId> {
        #[allow(clippy::cast_possible_truncation)]
        let display_id = reovim_driver_display::WindowId(window.raw() as usize);
        self.windows.get(display_id).and_then(|w| w.buffer_id)
    }

    #[allow(clippy::cast_lossless)]
    fn create_window(&mut self, buffer: Option<BufferId>) -> WindowId {
        let display_id = self.windows.create_window(buffer);
        let kernel_id = WindowId::from_raw(display_id.0 as u64);
        self.changes.record_window_created(kernel_id);
        kernel_id
    }

    fn close_window(&mut self, window: WindowId) -> Result<(), WindowError> {
        if self.windows.window_count() <= 1 {
            return Err(WindowError::CannotCloseLastWindow);
        }
        #[allow(clippy::cast_possible_truncation)]
        let display_id = reovim_driver_display::WindowId(window.raw() as usize);
        if self.windows.close_window(display_id) {
            self.changes.record_window_closed(window);
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }

    fn focus_window(&mut self, window: WindowId) -> Result<(), WindowError> {
        #[allow(clippy::cast_possible_truncation)]
        let display_id = reovim_driver_display::WindowId(window.raw() as usize);
        if self.windows.get(display_id).is_some() {
            self.windows.set_active_window(display_id);
            self.changes.record_focus_change();
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }

    fn set_window_buffer(&mut self, window: WindowId, buffer: BufferId) -> Result<(), WindowError> {
        if self.kernel.buffers.get(buffer).is_none() {
            return Err(WindowError::BufferNotFound(buffer));
        }
        #[allow(clippy::cast_possible_truncation)]
        let display_id = reovim_driver_display::WindowId(window.raw() as usize);
        if let Some(state) = self.windows.get_mut(display_id) {
            state.buffer_id = Some(buffer);
            self.changes.window_changed = true;
            Ok(())
        } else {
            Err(WindowError::NotFound(window))
        }
    }
}

// === CommandApi ===

impl CommandApi for AppStateRuntime<'_> {
    fn execute_command(&mut self, cmd: CommandId, ctx: CommandContext) -> CommandResult {
        // NOTE: Full command execution requires mutable KernelContext.
        // For now, defer actual execution to the runner's handle_resolve_result.
        let _ = (&self.command_registry, cmd, ctx);
        CommandResult::Error(
            "command execution via AppStateRuntime not yet implemented".to_string(),
        )
    }
}

// === ChangeTracker ===

impl ChangeTracker for AppStateRuntime<'_> {
    fn take_changes(&mut self) -> StateChanges {
        std::mem::take(&mut self.changes)
    }

    fn record_cursor_move(&mut self, buffer: BufferId) {
        self.changes.record_cursor_move(buffer);
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_mode_2() -> ModeId {
        ModeId::with_discriminant(ModuleId::new("test"), "insert", 1)
    }

    #[test]
    fn test_mode_api() {
        let kernel = KernelContext::default();
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = WindowRegistry::new();
        let mut active_buffer: Option<BufferId> = None;
        let command_registry = CommandRegistry::new();

        let mut runtime = AppStateRuntime::new(
            &mut mode_stack,
            &mut windows,
            &mut active_buffer,
            &kernel,
            &command_registry,
        );

        // Check initial state
        assert_eq!(runtime.current_mode(), &test_mode());
        assert_eq!(runtime.mode_depth(), 1);

        // Push mode
        runtime.push_mode(test_mode_2(), TransitionContext::new());
        assert_eq!(runtime.current_mode(), &test_mode_2());
        assert_eq!(runtime.mode_depth(), 2);

        // Pop mode
        let result = runtime.pop_mode(None);
        assert!(result.is_ok());
        assert_eq!(runtime.current_mode(), &test_mode());

        // Check changes were recorded
        let changes = runtime.take_changes();
        assert!(changes.mode_changed);
    }

    #[test]
    fn test_change_tracking() {
        let kernel = KernelContext::default();
        let mut mode_stack = ModeStack::new(test_mode());
        let mut windows = WindowRegistry::new();
        let mut active_buffer: Option<BufferId> = None;
        let command_registry = CommandRegistry::new();

        let mut runtime = AppStateRuntime::new(
            &mut mode_stack,
            &mut windows,
            &mut active_buffer,
            &kernel,
            &command_registry,
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
}
