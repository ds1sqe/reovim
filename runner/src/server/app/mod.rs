//! Application state for the runner.
//!
//! Separates runtime concerns from kernel context. The kernel provides
//! core services (buffers, events, options); the runner tracks runtime
//! state like active buffer and mode stack.

mod char_ops;
mod cmdline;
mod repeat;
mod search;
mod undotree;
mod visual;

pub use {
    char_ops::{CharWaitState, FindType, LastFind, PendingCharOp},
    cmdline::CommandLineState,
    repeat::{InsertEntryType, MAX_INSERT_COUNT, PendingEditBatch, RepeatState},
    search::{SearchDirection, SearchState},
    undotree::{DiffPreviewLine, UndotreeRenderLine, UndotreeState},
    visual::LastVisualSelection,
};

// Re-export PendingOperator (defined in this file, not a submodule)

use {
    crate::{UndoRegistry, server::window::WindowRegistry},
    reovim_arch::sync::RwLock,
    reovim_driver_display::WindowId,
    reovim_driver_input::{FallbackContext, KeySequence},
    reovim_kernel::api::v1::{Buffer, BufferId, Edit, KernelContext, ModeId, ModeStack, Position},
    std::sync::Arc,
};

/// Application state combining kernel context with runtime state.
///
/// # Design Philosophy
///
/// `KernelContext` provides kernel services (buffers, event bus, etc.)
/// but intentionally does NOT track runtime state like "which buffer is active"
/// because that's a runtime/window manager concern.
///
/// `AppState` wraps `KernelContext` and adds runtime-specific state that
/// the event loop and commands need to operate.
///
/// # Example
///
/// ```ignore
/// use runner::AppState;
/// use reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId};
///
/// let kernel = KernelContext::default();
/// let initial_mode = ModeId::new(ModuleId::new("editor"), "normal");
/// let app = AppState::new(kernel, initial_mode);
/// ```
#[derive(Debug)]
pub struct AppState {
    /// Kernel context providing access to all kernel services.
    pub kernel: KernelContext,

    /// Current mode stack for vim-style mode handling.
    ///
    /// Supports mode stacking (e.g., Normal → Operator-Pending → back to Normal).
    pub mode_stack: ModeStack,

    /// Currently active buffer ID.
    ///
    /// This is a runtime concern, not kernel - the kernel just stores buffers,
    /// the runner decides which one is "active".
    pub active_buffer: Option<BufferId>,

    /// Pending key sequence for multi-key bindings.
    ///
    /// For sequences like `gg` or `<C-w>h`, keys accumulate here until
    /// they either match a binding, are a prefix, or don't match.
    pub pending_keys: KeySequence,

    /// Whether the application is running.
    pub running: bool,

    /// Terminal width in columns.
    ///
    /// Tracked at runtime for headless server mode. TUI clients send
    /// `editor/resize` to update this when their terminal is resized.
    /// Default: 80 (standard VT100 width).
    pub terminal_width: u16,

    /// Terminal height in rows.
    ///
    /// Tracked at runtime for headless server mode. TUI clients send
    /// `editor/resize` to update this when their terminal is resized.
    /// Default: 24 (standard VT100 height).
    pub terminal_height: u16,

    /// Per-buffer undo registry.
    ///
    /// Maintains separate undo trees for each buffer, enabling per-buffer
    /// undo/redo operations. Each buffer has its own isolated undo history.
    pub undo_registry: UndoRegistry,

    /// Character wait state for find-char commands (f, F, t, T).
    ///
    /// When Some, the next character input will be used as the argument
    /// to complete a find-char motion rather than being processed as
    /// a normal key event.
    ///
    /// DEPRECATED: Use `pending_char` instead. Kept for backward compatibility.
    pub char_wait: Option<CharWaitState>,

    /// Pending character operation.
    ///
    /// Unified field for all commands that need a character argument:
    /// - Find-char commands (f, F, t, T) - search for a character
    /// - Replace-char command (r) - replace with a character
    ///
    /// When Some, the next character input will be used as the argument
    /// to complete the pending operation.
    pub pending_char: Option<PendingCharOp>,

    /// Last find-char operation for `;` and `,` repeat commands.
    ///
    /// Updated each time a find-char motion successfully moves the cursor.
    pub last_find: Option<LastFind>,

    /// Search state for / and ? commands.
    ///
    /// Tracks the current pattern, direction, highlighting, and input mode.
    /// Pattern persists across buffer switches (like Vim).
    pub search: SearchState,

    /// Repeat state for the `.` command.
    ///
    /// Tracks the last repeatable command and any insert mode text.
    pub repeat_state: RepeatState,

    /// Pending edits for transaction batching.
    ///
    /// Accumulates consecutive edits (typically character insertions in
    /// insert mode) until a batch-breaking event occurs, then flushes
    /// them as a single undo transaction.
    ///
    /// # Batch Break Events
    ///
    /// - Mode change (e.g., Escape exits insert mode)
    /// - Any command execution (Backspace, arrow keys, etc.)
    /// - Buffer change (editing different buffer)
    pending_edits: PendingEditBatch,

    /// Last visual selection for the `gv` (reselect) command.
    ///
    /// When visual mode is exited, the selection is saved here so that
    /// `gv` can restore it. This allows re-selecting the last visual area.
    pub last_visual_selection: Option<LastVisualSelection>,

    /// Window registry for multi-window support.
    ///
    /// Tracks window state, layout, and focus. Each window has its own
    /// cursor position, allowing multiple views of the same buffer.
    pub windows: WindowRegistry,

    /// Undotree panel state.
    ///
    /// Tracks the undotree visualization panel: whether it's open,
    /// which window displays it, which buffer's tree is shown,
    /// and navigation state within the tree.
    pub undotree_state: UndotreeState,

    /// Command-line mode state for : commands.
    ///
    /// Tracks input buffer and whether command-line mode is active.
    /// Used for Ex-style commands like `:w`, `:q`, `:set`, etc.
    pub cmdline: CommandLineState,

    /// Pending operator waiting for a motion.
    ///
    /// When a user presses an operator key (d, y, c) in normal mode,
    /// the operator is stored here until a motion key is pressed to
    /// complete the operation. The motion provides the text range,
    /// then the operator is executed on that range.
    pub pending_operator: Option<PendingOperator>,
}

/// Information about a pending operator waiting for a motion.
///
/// In vim, operators like `d`, `y`, `c` wait for a motion to define
/// the text range they operate on. This struct captures the operator
/// and any modifiers (count, register) while waiting.
#[derive(Debug, Clone)]
pub struct PendingOperator {
    /// Operator ID: "delete", "yank", or "change".
    pub operator_id: &'static str,

    /// Count applied to the operator (e.g., `2dw` applies operator twice).
    pub count: usize,

    /// Target register for the operation.
    pub register: Option<char>,
}

impl PendingOperator {
    /// Create a new pending operator.
    #[must_use]
    pub const fn new(operator_id: &'static str) -> Self {
        Self {
            operator_id,
            count: 1,
            register: None,
        }
    }

    /// Set the count for this operator.
    #[must_use]
    pub const fn with_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// Set the register for this operator.
    #[must_use]
    pub const fn with_register(mut self, register: Option<char>) -> Self {
        self.register = register;
        self
    }
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Arguments
    ///
    /// * `kernel` - The kernel context providing core services
    /// * `initial_mode` - The mode to start in (typically Normal mode)
    #[must_use]
    pub fn new(kernel: KernelContext, initial_mode: ModeId) -> Self {
        Self {
            kernel,
            mode_stack: ModeStack::new(initial_mode),
            active_buffer: None,
            pending_keys: KeySequence::new(),
            running: true,
            terminal_width: 80,
            terminal_height: 24,
            undo_registry: UndoRegistry::new(),
            char_wait: None,
            pending_char: None,
            last_find: None,
            search: SearchState::new(),
            repeat_state: RepeatState::new(),
            pending_edits: PendingEditBatch::new(),
            last_visual_selection: None,
            windows: WindowRegistry::new(),
            undotree_state: UndotreeState::new(),
            pending_operator: None,
            cmdline: CommandLineState::new(),
        }
    }

    /// Get the current mode ID.
    #[must_use]
    pub fn current_mode(&self) -> &ModeId {
        self.mode_stack.current()
    }

    /// Request the application to quit.
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
    pub fn request_quit(&mut self) {
        self.running = false;
    }

    /// Request clients to detach (server continues running).
    ///
    /// Unlike `request_quit`, the server remains active and accepts new connections.
    /// Connected TUI clients receive a DETACH notification and disconnect gracefully.
    ///
    /// Note: This sets a flag; the actual notification is sent by the broadcaster.
    #[allow(clippy::missing_const_for_fn)]
    pub fn request_detach(&mut self) {
        // TODO: Set a detach_requested flag and have broadcaster send notifications.
        // For now, this is a placeholder that logs the intent.
        tracing::info!("Detach requested - clients will be notified");
    }

    /// Check if the application should continue running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Clear the pending key sequence.
    pub fn clear_pending_keys(&mut self) {
        self.pending_keys.clear();
    }

    // ========================================================================
    // Pending Character Operation Methods
    // ========================================================================

    /// Set a pending character operation.
    ///
    /// The next character input will be used to complete this operation.
    pub const fn set_pending_char(&mut self, op: PendingCharOp) {
        self.pending_char = Some(op);
        // Also set char_wait for backward compatibility
        if let Some(find_type) = op.find_type()
            && let Some(start) = op.start_position()
        {
            self.char_wait = Some(CharWaitState::new(find_type, start));
        }
    }

    /// Take the pending character operation, clearing it.
    ///
    /// Returns `Some(op)` if there was a pending operation, `None` otherwise.
    pub const fn take_pending_char(&mut self) -> Option<PendingCharOp> {
        // Clear char_wait for backward compatibility
        self.char_wait = None;
        self.pending_char.take()
    }

    /// Check if there is a pending character operation.
    #[must_use]
    pub const fn has_pending_char(&self) -> bool {
        self.pending_char.is_some()
    }

    /// Get a reference to the pending character operation, if any.
    #[must_use]
    pub const fn pending_char(&self) -> Option<&PendingCharOp> {
        self.pending_char.as_ref()
    }

    // ========================================================================
    // Pending Operator Methods (for operator-motion combinations)
    // ========================================================================

    /// Set a pending operator that's waiting for a motion.
    ///
    /// Called when an operator key (d, y, c) is pressed in normal mode.
    /// The operator waits for a motion to provide the text range.
    pub const fn set_pending_operator(&mut self, op: PendingOperator) {
        self.pending_operator = Some(op);
    }

    /// Take the pending operator, clearing it.
    ///
    /// Returns `Some(op)` if there was a pending operator, `None` otherwise.
    /// Called when a motion provides a range to complete the operation.
    pub const fn take_pending_operator(&mut self) -> Option<PendingOperator> {
        self.pending_operator.take()
    }

    /// Check if there is a pending operator.
    #[must_use]
    pub const fn has_pending_operator(&self) -> bool {
        self.pending_operator.is_some()
    }

    /// Get a reference to the pending operator, if any.
    #[must_use]
    pub const fn pending_operator(&self) -> Option<&PendingOperator> {
        self.pending_operator.as_ref()
    }

    // ========================================================================
    // Repeat State Methods
    // ========================================================================

    /// Record a command for repeat (`.`).
    ///
    /// Only text-modifying commands should be recorded.
    pub fn record_for_repeat(&mut self, command_id: &str) {
        self.repeat_state.record_command(command_id);
    }

    /// Start accumulating insert mode text for repeat.
    pub fn start_insert_accumulation(&mut self) {
        self.repeat_state.start_accumulating();
    }

    /// Add text to the insert mode accumulator.
    pub fn accumulate_insert_text(&mut self, text: &str) {
        self.repeat_state.accumulate_insert(text);
    }

    /// Stop accumulating insert mode text.
    pub const fn stop_insert_accumulation(&mut self) {
        self.repeat_state.stop_accumulating();
    }

    // ========================================================================
    // Window Management Methods
    // ========================================================================

    /// Get the currently active window ID.
    #[must_use]
    pub const fn active_window(&self) -> Option<WindowId> {
        self.windows.active_window()
    }

    /// Find the first window displaying a given buffer.
    #[must_use]
    pub fn window_for_buffer(&self, buffer_id: BufferId) -> Option<WindowId> {
        self.windows.windows().find(|&win_id| {
            self.windows
                .get(win_id)
                .is_some_and(|state| state.buffer_id == Some(buffer_id))
        })
    }

    /// Get the buffer displayed in a given window.
    #[must_use]
    pub fn buffer_for_window(&self, window_id: WindowId) -> Option<BufferId> {
        self.windows
            .get(window_id)
            .and_then(|state| state.buffer_id)
    }

    /// Ensure a window exists when setting the active buffer.
    ///
    /// For backward compatibility with single-window usage, this creates
    /// a window if none exist when a buffer is set as active.
    pub fn set_active_buffer_with_window(&mut self, buffer_id: BufferId) {
        // If no windows exist, create one
        if self.windows.is_empty() {
            let window_id = self.windows.create_window(Some(buffer_id));
            self.windows.set_active_window(window_id);
        } else if let Some(active) = self.windows.active_window() {
            // Update the active window's buffer
            if let Some(state) = self.windows.get_mut(active) {
                state.buffer_id = Some(buffer_id);
            }
        }
        self.active_buffer = Some(buffer_id);
    }

    // ========================================================================
    // Undo Transaction Batching Methods
    // ========================================================================

    /// Accumulate an edit for batched undo.
    ///
    /// Consecutive character insertions (or other edits) can be accumulated
    /// and later flushed as a single undo transaction. This enables Vim-like
    /// behavior where typing "hello" in insert mode creates a single undo node.
    ///
    /// If the buffer changes (different `buffer_id` from current batch), the
    /// existing batch is flushed before starting a new one.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer being edited
    /// * `edit` - The edit to accumulate
    /// * `cursor_before` - Cursor position before this edit
    /// * `cursor_after` - Cursor position after this edit
    ///
    /// # Example
    ///
    /// ```ignore
    /// // User types 'h' in insert mode
    /// app.accumulate_edit(buffer_id, edit, Position::new(0, 0), Position::new(0, 1));
    /// // User types 'i'
    /// app.accumulate_edit(buffer_id, edit, Position::new(0, 1), Position::new(0, 2));
    /// // User presses Escape - this triggers flush
    /// app.flush_pending_edits();
    /// // Result: single undo node with both edits
    /// ```
    pub fn accumulate_edit(
        &mut self,
        buffer_id: BufferId,
        edit: Edit,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        // Flush if buffer changed
        if self
            .pending_edits
            .buffer_id
            .is_some_and(|id| id != buffer_id)
        {
            self.flush_pending_edits();
        }

        // Start new batch if empty
        if self.pending_edits.edits.is_empty() {
            self.pending_edits.buffer_id = Some(buffer_id);
            self.pending_edits.cursor_before = Some(cursor_before);
        }

        self.pending_edits.edits.push(edit);
        self.pending_edits.cursor_after = Some(cursor_after);
    }

    /// Flush pending edits to undo registry as a single transaction.
    ///
    /// This commits all accumulated edits as a single undo node. If there are
    /// no pending edits, this is a no-op (safe to call multiple times).
    ///
    /// Called automatically on:
    /// - Mode change (e.g., exiting insert mode)
    /// - Before any command execution (Backspace, arrow keys, etc.)
    /// - Buffer change (via `accumulate_edit` when buffer differs)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Accumulated 5 character edits
    /// app.flush_pending_edits();
    /// // Now pressing 'u' undoes all 5 characters at once
    /// ```
    pub fn flush_pending_edits(&mut self) {
        if self.pending_edits.is_empty() {
            return;
        }

        let Some(buffer_id) = self.pending_edits.buffer_id else {
            return;
        };
        let Some(cursor_before) = self.pending_edits.cursor_before else {
            return;
        };
        let Some(cursor_after) = self.pending_edits.cursor_after else {
            return;
        };

        let edits = std::mem::take(&mut self.pending_edits.edits);
        self.undo_registry
            .record(buffer_id, edits, cursor_before, cursor_after);
        self.pending_edits.clear();
    }

    /// Check if there are pending edits waiting to be flushed.
    ///
    /// Useful for testing and debugging.
    #[must_use]
    pub const fn has_pending_edits(&self) -> bool {
        !self.pending_edits.is_empty()
    }

    /// Get the number of pending edits.
    ///
    /// Useful for testing and debugging.
    #[must_use]
    pub const fn pending_edit_count(&self) -> usize {
        self.pending_edits.len()
    }

    // ========================================================================
    // Visual Selection Methods
    // ========================================================================

    /// Save the current visual selection for later reselection with `gv`.
    ///
    /// Called when exiting visual mode to remember the selection boundaries.
    pub fn save_visual_selection(&mut self, selection: LastVisualSelection) {
        self.last_visual_selection = Some(selection);
    }

    /// Get the last visual selection, if any.
    #[must_use]
    pub const fn last_visual_selection(&self) -> Option<&LastVisualSelection> {
        self.last_visual_selection.as_ref()
    }

    /// Check if there is a saved visual selection.
    #[must_use]
    pub const fn has_last_visual_selection(&self) -> bool {
        self.last_visual_selection.is_some()
    }

    /// Clear the last visual selection.
    pub fn clear_last_visual_selection(&mut self) {
        self.last_visual_selection = None;
    }
}

impl FallbackContext for AppState {
    fn current_mode(&self) -> &ModeId {
        self.mode_stack.current()
    }

    fn active_buffer(&self) -> Option<BufferId> {
        self.active_buffer
    }

    fn get_buffer(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
        self.kernel.buffers.get(id)
    }

    fn record_edit(
        &mut self,
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        self.undo_registry
            .record(buffer_id, edits, cursor_before, cursor_after);
    }

    fn accumulate_edit(
        &mut self,
        buffer_id: BufferId,
        edit: Edit,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        // Delegate to the inherent method on AppState
        Self::accumulate_edit(self, buffer_id, edit, cursor_before, cursor_after);
    }

    fn flush_pending_edits(&mut self) {
        // Delegate to the inherent method on AppState
        Self::flush_pending_edits(self);
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{Direction, ModuleId, SelectionMode},
    };

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_visual_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("editor"), "visual")
    }

    fn test_visual_line_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("editor"), "visual-line")
    }

    #[test]
    fn test_app_state_new() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        assert!(app.is_running());
        assert!(app.active_buffer.is_none());
        assert!(app.pending_keys.is_empty());
        assert_eq!(app.current_mode().name(), "normal");
    }

    #[test]
    fn test_app_state_quit() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        assert!(app.is_running());
        app.request_quit();
        assert!(!app.is_running());
    }

    #[test]
    fn test_app_state_clear_pending() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Add some keys
        app.pending_keys
            .push(reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('g')));
        assert!(!app.pending_keys.is_empty());

        app.clear_pending_keys();
        assert!(app.pending_keys.is_empty());
    }

    #[test]
    fn test_app_state_default_terminal_size() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        // Default terminal size is 80x24 (VT100 standard)
        assert_eq!(app.terminal_width, 80);
        assert_eq!(app.terminal_height, 24);
    }

    #[test]
    fn test_app_state_terminal_size_access() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Verify we can read and write terminal dimensions
        app.terminal_width = 120;
        app.terminal_height = 40;

        assert_eq!(app.terminal_width, 120);
        assert_eq!(app.terminal_height, 40);
    }

    #[test]
    fn test_app_state_has_undo_registry() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        // AppState should initialize with empty UndoRegistry
        assert_eq!(app.undo_registry.buffer_count(), 0);
    }

    #[test]
    fn test_app_state_undo_registry_accessible() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Can call undo_registry.record()
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "hello");
        app.undo_registry
            .record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        assert!(app.undo_registry.has_history(buffer_id));
    }

    #[test]
    fn test_app_state_undo_registry_per_buffer() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let buffer1 = BufferId::from_raw(1);
        let buffer2 = BufferId::from_raw(2);

        // Record edit to buffer1
        let edit = Edit::insert(Position::new(0, 0), "hello");
        app.undo_registry
            .record(buffer1, vec![edit], Position::new(0, 0), Position::new(0, 5));

        // buffer1 has history, buffer2 does not (isolated)
        assert!(app.undo_registry.has_history(buffer1));
        assert!(!app.undo_registry.has_history(buffer2));
    }

    // ========================================================================
    // Char-Wait Infrastructure Tests
    // ========================================================================

    #[test]
    fn test_app_state_char_wait_initially_none() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        assert!(app.char_wait.is_none());
        assert!(app.last_find.is_none());
    }

    #[test]
    fn test_app_state_char_wait_set_and_clear() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Set char_wait
        app.char_wait = Some(CharWaitState::new(FindType::FindForward, Position::new(0, 0)));
        assert!(app.char_wait.is_some());

        // Clear char_wait
        app.char_wait = None;
        assert!(app.char_wait.is_none());
    }

    #[test]
    fn test_app_state_last_find_set() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Set last_find
        app.last_find = Some(LastFind::new('a', FindType::TillForward));

        let last = app.last_find.unwrap();
        assert_eq!(last.char, 'a');
        assert_eq!(last.find_type, FindType::TillForward);
    }

    // ========================================================================
    // AppState Pending Char Methods Tests
    // ========================================================================

    #[test]
    fn test_app_state_pending_char_initially_none() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        assert!(!app.has_pending_char());
        assert!(app.pending_char().is_none());
    }

    #[test]
    fn test_app_state_set_pending_char() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        app.set_pending_char(PendingCharOp::find_forward(Position::new(0, 0)));

        assert!(app.has_pending_char());
        assert!(app.pending_char().is_some());
        // Backward compatibility: char_wait should also be set
        assert!(app.char_wait.is_some());
    }

    #[test]
    fn test_app_state_take_pending_char() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        app.set_pending_char(PendingCharOp::till_backward(Position::new(1, 5)));

        let taken = app.take_pending_char();
        assert!(taken.is_some());
        assert!(!app.has_pending_char());
        // Backward compatibility: char_wait should also be cleared
        assert!(app.char_wait.is_none());
    }

    #[test]
    fn test_app_state_set_pending_char_replace() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Replace char doesn't set char_wait (no find_type)
        app.set_pending_char(PendingCharOp::replace_char(1));

        assert!(app.has_pending_char());
        // char_wait should NOT be set for replace operations
        assert!(app.char_wait.is_none());
    }

    // ========================================================================
    // AppState Repeat Methods Tests
    // ========================================================================

    #[test]
    fn test_app_state_repeat_state_initially_empty() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        assert!(app.repeat_state.last_command.is_none());
    }

    #[test]
    fn test_app_state_record_for_repeat() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        app.record_for_repeat("delete-line");
        assert_eq!(app.repeat_state.last_command, Some("delete-line".to_string()));
    }

    #[test]
    fn test_app_state_insert_accumulation() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        app.start_insert_accumulation();
        app.accumulate_insert_text("test");
        app.stop_insert_accumulation();

        assert_eq!(app.repeat_state.insert_text, "test");
        assert!(!app.repeat_state.accumulating);
    }

    // ========================================================================
    // AppState Undo Batching Methods Tests
    // ========================================================================

    #[test]
    fn test_app_state_pending_edits_initially_empty() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        assert!(!app.has_pending_edits());
        assert_eq!(app.pending_edit_count(), 0);
    }

    #[test]
    fn test_accumulate_single_edit() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "a");

        app.accumulate_edit(buffer_id, edit, Position::new(0, 0), Position::new(0, 1));

        assert!(app.has_pending_edits());
        assert_eq!(app.pending_edit_count(), 1);
    }

    #[test]
    fn test_accumulate_multiple_edits_same_buffer() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let buffer_id = BufferId::from_raw(1);

        // Accumulate "hello" (5 characters)
        app.accumulate_edit(
            buffer_id,
            Edit::insert(Position::new(0, 0), "h"),
            Position::new(0, 0),
            Position::new(0, 1),
        );
        app.accumulate_edit(
            buffer_id,
            Edit::insert(Position::new(0, 1), "e"),
            Position::new(0, 1),
            Position::new(0, 2),
        );
        app.accumulate_edit(
            buffer_id,
            Edit::insert(Position::new(0, 2), "l"),
            Position::new(0, 2),
            Position::new(0, 3),
        );
        app.accumulate_edit(
            buffer_id,
            Edit::insert(Position::new(0, 3), "l"),
            Position::new(0, 3),
            Position::new(0, 4),
        );
        app.accumulate_edit(
            buffer_id,
            Edit::insert(Position::new(0, 4), "o"),
            Position::new(0, 4),
            Position::new(0, 5),
        );

        assert!(app.has_pending_edits());
        assert_eq!(app.pending_edit_count(), 5);
    }

    #[test]
    fn test_accumulate_different_buffer_flushes() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let buffer1 = BufferId::from_raw(1);
        let buffer2 = BufferId::from_raw(2);

        // Accumulate to buffer1
        app.accumulate_edit(
            buffer1,
            Edit::insert(Position::new(0, 0), "a"),
            Position::new(0, 0),
            Position::new(0, 1),
        );
        app.accumulate_edit(
            buffer1,
            Edit::insert(Position::new(0, 1), "b"),
            Position::new(0, 1),
            Position::new(0, 2),
        );

        assert_eq!(app.pending_edit_count(), 2);

        // Accumulate to buffer2 - should flush buffer1 first
        app.accumulate_edit(
            buffer2,
            Edit::insert(Position::new(0, 0), "x"),
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // Now pending should only have 1 edit (for buffer2)
        // The previous 2 edits for buffer1 were flushed
        assert_eq!(app.pending_edit_count(), 1);

        // Buffer1 should have undo history from the flush
        assert!(app.undo_registry.has_history(buffer1));
    }

    #[test]
    fn test_flush_empty_batch_no_op() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let buffer_id = BufferId::from_raw(1);

        // Flush when empty should be safe no-op
        app.flush_pending_edits();

        // Should not create any undo history
        assert!(!app.undo_registry.has_history(buffer_id));
        assert!(!app.has_pending_edits());
    }

    #[test]
    fn test_flush_creates_single_undo_node() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let buffer_id = BufferId::from_raw(1);

        // Accumulate 3 edits
        app.accumulate_edit(
            buffer_id,
            Edit::insert(Position::new(0, 0), "a"),
            Position::new(0, 0),
            Position::new(0, 1),
        );
        app.accumulate_edit(
            buffer_id,
            Edit::insert(Position::new(0, 1), "b"),
            Position::new(0, 1),
            Position::new(0, 2),
        );
        app.accumulate_edit(
            buffer_id,
            Edit::insert(Position::new(0, 2), "c"),
            Position::new(0, 2),
            Position::new(0, 3),
        );

        assert_eq!(app.pending_edit_count(), 3);

        // Flush should create single undo node
        app.flush_pending_edits();

        assert!(!app.has_pending_edits());
        assert!(app.undo_registry.has_history(buffer_id));

        // Verify it's a single undo operation (one undo should undo all 3)
        let result = app.undo_registry.undo(buffer_id);
        assert!(result.is_some());
        let undo_result = result.unwrap();
        // The undo should contain 3 inverse edits (one for each accumulated edit)
        assert_eq!(undo_result.edits.len(), 3);
    }

    #[test]
    fn test_multiple_consecutive_flushes_safe() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Multiple flushes when empty should be safe
        app.flush_pending_edits();
        app.flush_pending_edits();
        app.flush_pending_edits();

        assert!(!app.has_pending_edits());
    }

    // ========================================================================
    // Visual Selection Tests
    // ========================================================================

    #[test]
    fn test_app_state_last_visual_selection_initially_none() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        assert!(!app.has_last_visual_selection());
        assert!(app.last_visual_selection().is_none());
    }

    #[test]
    fn test_app_state_save_visual_selection() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let selection = LastVisualSelection::new(
            BufferId::from_raw(1),
            Position::new(0, 0),
            Position::new(1, 5),
            SelectionMode::Character,
            test_visual_mode_id(),
        );

        app.save_visual_selection(selection);

        assert!(app.has_last_visual_selection());
        let saved = app.last_visual_selection().unwrap();
        assert_eq!(saved.anchor, Position::new(0, 0));
        assert_eq!(saved.cursor, Position::new(1, 5));
    }

    #[test]
    fn test_app_state_clear_last_visual_selection() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let selection = LastVisualSelection::new(
            BufferId::from_raw(1),
            Position::new(0, 0),
            Position::new(1, 5),
            SelectionMode::Character,
            test_visual_mode_id(),
        );

        app.save_visual_selection(selection);
        assert!(app.has_last_visual_selection());

        app.clear_last_visual_selection();
        assert!(!app.has_last_visual_selection());
    }

    #[test]
    fn test_app_state_save_visual_selection_overwrites() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let selection1 = LastVisualSelection::new(
            BufferId::from_raw(1),
            Position::new(0, 0),
            Position::new(1, 5),
            SelectionMode::Character,
            test_visual_mode_id(),
        );
        app.save_visual_selection(selection1);

        let selection2 = LastVisualSelection::new(
            BufferId::from_raw(2),
            Position::new(5, 0),
            Position::new(10, 20),
            SelectionMode::Line,
            test_visual_line_mode_id(),
        );
        app.save_visual_selection(selection2);

        // Should have the second selection
        let saved = app.last_visual_selection().unwrap();
        assert_eq!(saved.buffer_id, BufferId::from_raw(2));
        assert_eq!(saved.anchor, Position::new(5, 0));
        assert_eq!(saved.cursor, Position::new(10, 20));
        assert_eq!(saved.mode, SelectionMode::Line);
    }

    // ========================================================================
    // Window Management Tests
    // ========================================================================

    #[test]
    fn test_app_state_has_window_registry() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        // Initially no windows
        assert!(app.windows.is_empty());
        assert!(app.active_window().is_none());
    }

    #[test]
    fn test_app_state_active_window_initially_none() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        assert!(app.active_window().is_none());
        assert_eq!(app.windows.window_count(), 0);
    }

    #[test]
    fn test_app_state_create_window_on_buffer_set() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let buffer_id = BufferId::new();

        // Set active buffer with window creation
        app.set_active_buffer_with_window(buffer_id);

        // Should have created a window
        assert_eq!(app.windows.window_count(), 1);
        assert!(app.active_window().is_some());
        assert_eq!(app.active_buffer, Some(buffer_id));

        // The window should contain the buffer
        let win_id = app.active_window().unwrap();
        assert_eq!(app.buffer_for_window(win_id), Some(buffer_id));
    }

    #[test]
    fn test_app_state_backward_compat_single_buffer() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        // Old single-buffer workflow:
        // 1. Create buffer
        // 2. Set as active with window
        // 3. Edit buffer

        let buffer_id = BufferId::new();
        app.set_active_buffer_with_window(buffer_id);

        // Verify backward compatibility
        assert_eq!(app.active_buffer, Some(buffer_id));
        assert!(app.active_window().is_some());

        // Window should have the buffer
        let win = app.active_window().unwrap();
        assert_eq!(app.buffer_for_window(win), Some(buffer_id));

        // Set another buffer - should reuse existing window
        let buffer_id2 = BufferId::new();
        app.set_active_buffer_with_window(buffer_id2);

        // Still one window, but with new buffer
        assert_eq!(app.windows.window_count(), 1);
        assert_eq!(app.active_buffer, Some(buffer_id2));
        assert_eq!(app.buffer_for_window(win), Some(buffer_id2));
    }

    #[test]
    fn test_app_state_window_for_buffer() {
        let kernel = KernelContext::default();
        let mut app = AppState::new(kernel, test_mode_id());

        let buffer_id = BufferId::new();
        app.set_active_buffer_with_window(buffer_id);

        // Should find the window for this buffer
        let found = app.window_for_buffer(buffer_id);
        assert!(found.is_some());
        assert_eq!(found, app.active_window());

        // Should not find window for non-existent buffer
        let other_buffer = BufferId::new();
        assert!(app.window_for_buffer(other_buffer).is_none());
    }

    // ========================================================================
    // UndotreeState Tests
    // ========================================================================

    #[test]
    fn test_app_state_has_undotree_state() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        // AppState should have undotree_state initialized
        assert!(!app.undotree_state.is_open());
        assert!(app.undotree_state.panel_window_id().is_none());
    }

    // Type re-export tests to ensure flat re-exports work
    #[test]
    fn test_reexported_types_accessible() {
        // Verify types are accessible via the re-exports
        let _ = FindType::FindForward;
        let _ = SearchDirection::Forward;
        let _ = RepeatState::new();
        let _ = PendingEditBatch::new();
        let _ = UndotreeState::new();
    }

    #[test]
    fn test_find_type_direction_via_reexport() {
        assert_eq!(FindType::FindForward.direction(), Direction::Forward);
        assert_eq!(FindType::FindBackward.direction(), Direction::Backward);
    }
}
