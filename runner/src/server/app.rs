//! Application state for the runner.
//!
//! Separates runtime concerns from kernel context. The kernel provides
//! core services (buffers, events, options); the runner tracks runtime
//! state like active buffer and mode stack.

use {
    crate::{UndoRegistry, server::window::WindowRegistry},
    reovim_arch::sync::RwLock,
    reovim_driver_display::WindowId,
    reovim_driver_input::{FallbackContext, KeySequence},
    reovim_kernel::api::v1::{
        Buffer, BufferId, Direction, Edit, KernelContext, ModeId, ModeStack, Motion, Position,
        SelectionMode,
    },
    std::sync::Arc,
};

// ============================================================================
// Pending Character Operation Infrastructure
// ============================================================================

/// Type of find-char operation.
///
/// Represents the four find-char motions in Vim:
/// - `f` - find forward, cursor on char
/// - `F` - find backward, cursor on char
/// - `t` - till forward, cursor before char
/// - `T` - till backward, cursor after char
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindType {
    /// Find character forward, cursor on char (f)
    FindForward,
    /// Find character backward, cursor on char (F)
    FindBackward,
    /// Till character forward, cursor before char (t)
    TillForward,
    /// Till character backward, cursor after char (T)
    TillBackward,
}

impl FindType {
    /// Convert to kernel Direction.
    #[must_use]
    pub const fn direction(self) -> Direction {
        match self {
            Self::FindForward | Self::TillForward => Direction::Forward,
            Self::FindBackward | Self::TillBackward => Direction::Backward,
        }
    }

    /// Whether this is a "till" motion (stops before/after the character).
    #[must_use]
    pub const fn is_till(self) -> bool {
        matches!(self, Self::TillForward | Self::TillBackward)
    }
}

/// Pending character operation.
///
/// Commands that need a single character argument return
/// `CommandResult::WaitingForChar` and the event loop stores
/// the operation here until the character is received.
///
/// This unified enum covers both:
/// - Find-char motions (f, F, t, T) - need a character to search for
/// - Replace-char operation (r{char}) - need a character to replace with
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingCharOp {
    /// Find character forward (f)
    FindForward {
        /// Starting position for operator range calculation.
        start: Position,
    },
    /// Find character backward (F)
    FindBackward {
        /// Starting position for operator range calculation.
        start: Position,
    },
    /// Till character forward (t)
    TillForward {
        /// Starting position for operator range calculation.
        start: Position,
    },
    /// Till character backward (T)
    TillBackward {
        /// Starting position for operator range calculation.
        start: Position,
    },
    /// Replace character (r{char})
    ReplaceChar {
        /// Count for replacement (e.g., 3rx replaces 3 chars with 'x').
        count: usize,
    },
}

impl PendingCharOp {
    /// Create a find-forward pending operation.
    #[must_use]
    pub const fn find_forward(start: Position) -> Self {
        Self::FindForward { start }
    }

    /// Create a find-backward pending operation.
    #[must_use]
    pub const fn find_backward(start: Position) -> Self {
        Self::FindBackward { start }
    }

    /// Create a till-forward pending operation.
    #[must_use]
    pub const fn till_forward(start: Position) -> Self {
        Self::TillForward { start }
    }

    /// Create a till-backward pending operation.
    #[must_use]
    pub const fn till_backward(start: Position) -> Self {
        Self::TillBackward { start }
    }

    /// Create a replace-char pending operation.
    #[must_use]
    pub const fn replace_char(count: usize) -> Self {
        Self::ReplaceChar { count }
    }

    /// Create from `FindType` and position (for find-char commands).
    #[must_use]
    pub const fn from_find_type(find_type: FindType, start: Position) -> Self {
        match find_type {
            FindType::FindForward => Self::FindForward { start },
            FindType::FindBackward => Self::FindBackward { start },
            FindType::TillForward => Self::TillForward { start },
            FindType::TillBackward => Self::TillBackward { start },
        }
    }

    /// Check if this is a find-char operation.
    #[must_use]
    pub const fn is_find_char(&self) -> bool {
        matches!(
            self,
            Self::FindForward { .. }
                | Self::FindBackward { .. }
                | Self::TillForward { .. }
                | Self::TillBackward { .. }
        )
    }

    /// Check if this is a replace-char operation.
    #[must_use]
    pub const fn is_replace_char(&self) -> bool {
        matches!(self, Self::ReplaceChar { .. })
    }

    /// Get the `FindType` if this is a find-char operation.
    #[must_use]
    pub const fn find_type(&self) -> Option<FindType> {
        match self {
            Self::FindForward { .. } => Some(FindType::FindForward),
            Self::FindBackward { .. } => Some(FindType::FindBackward),
            Self::TillForward { .. } => Some(FindType::TillForward),
            Self::TillBackward { .. } => Some(FindType::TillBackward),
            Self::ReplaceChar { .. } => None,
        }
    }

    /// Get the start position if this is a find-char operation.
    #[must_use]
    pub const fn start_position(&self) -> Option<Position> {
        match self {
            Self::FindForward { start }
            | Self::FindBackward { start }
            | Self::TillForward { start }
            | Self::TillBackward { start } => Some(*start),
            Self::ReplaceChar { .. } => None,
        }
    }
}

/// State for commands waiting for a character argument.
///
/// When a find-char command (f/F/t/T) is executed, it returns `WaitingForChar`
/// and this state is set. The next key press provides the character argument
/// to complete the motion.
///
/// DEPRECATED: Use `PendingCharOp` instead. Kept for backward compatibility
/// during transition.
#[derive(Debug, Clone)]
pub struct CharWaitState {
    /// The type of find operation.
    pub find_type: FindType,
    /// Starting position for the motion (for operator range calculation).
    pub start_position: Position,
}

impl CharWaitState {
    /// Create a new char-wait state.
    #[must_use]
    pub const fn new(find_type: FindType, start_position: Position) -> Self {
        Self {
            find_type,
            start_position,
        }
    }

    /// Convert to `PendingCharOp`.
    #[must_use]
    pub const fn to_pending_char_op(&self) -> PendingCharOp {
        PendingCharOp::from_find_type(self.find_type, self.start_position)
    }
}

impl From<CharWaitState> for PendingCharOp {
    fn from(state: CharWaitState) -> Self {
        state.to_pending_char_op()
    }
}

/// Record of the last find-char operation for `;` and `,` repeat.
///
/// Stores the character and find type so that `;` can repeat the same
/// find and `,` can repeat in the opposite direction.
#[derive(Debug, Clone, Copy)]
pub struct LastFind {
    /// The character that was searched for.
    pub char: char,
    /// The type of find operation.
    pub find_type: FindType,
}

impl LastFind {
    /// Create a new last-find record.
    #[must_use]
    pub const fn new(char: char, find_type: FindType) -> Self {
        Self { char, find_type }
    }

    /// Create the Motion for repeating in the same direction (`;`).
    #[must_use]
    pub const fn repeat_motion(self) -> Motion {
        Motion::FindChar {
            char: self.char,
            direction: self.find_type.direction(),
            till: self.find_type.is_till(),
        }
    }

    /// Create the Motion for repeating in opposite direction (`,`).
    #[must_use]
    pub const fn reverse_motion(self) -> Motion {
        Motion::FindChar {
            char: self.char,
            direction: self.find_type.direction().opposite(),
            till: self.find_type.is_till(),
        }
    }
}

// ============================================================================
// Search Infrastructure
// ============================================================================

/// Search direction for / and ? commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchDirection {
    /// Search forward from cursor (/)
    #[default]
    Forward,
    /// Search backward from cursor (?)
    Backward,
}

/// Search state for the session.
///
/// Tracks the current search pattern, direction, highlighting status,
/// and input mode for pattern entry.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Last search pattern (persists across buffers like Vim).
    pub pattern: Option<String>,
    /// Last search direction.
    pub direction: SearchDirection,
    /// Whether search highlighting is active.
    pub highlight_active: bool,
    /// Input buffer when in search mode (typing pattern).
    pub input_buffer: String,
    /// Whether we're in search input mode.
    pub input_active: bool,
    /// Direction for current input mode (forward for /, backward for ?).
    pub input_direction: SearchDirection,
}

impl SearchState {
    /// Create a new search state with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start search input mode.
    pub fn start_input(&mut self, direction: SearchDirection) {
        self.input_active = true;
        self.input_direction = direction;
        self.input_buffer.clear();
    }

    /// Cancel search input mode.
    pub fn cancel_input(&mut self) {
        self.input_active = false;
        self.input_buffer.clear();
    }

    /// Complete search input and return (pattern, direction).
    ///
    /// If input is empty, returns the last pattern (Vim behavior).
    /// Returns None if no pattern available.
    pub fn complete_input(&mut self) -> Option<(String, SearchDirection)> {
        if !self.input_active {
            return None;
        }

        self.input_active = false;

        if self.input_buffer.is_empty() {
            // Empty input uses last pattern
            return self
                .pattern
                .as_ref()
                .map(|p| (p.clone(), self.input_direction));
        }

        let pattern = std::mem::take(&mut self.input_buffer);
        let direction = self.input_direction;

        // Store for future use
        self.pattern = Some(pattern.clone());
        self.direction = direction;
        self.highlight_active = true;

        Some((pattern, direction))
    }

    /// Clear search highlighting.
    pub const fn clear_highlight(&mut self) {
        self.highlight_active = false;
    }
}

// ============================================================================
// Undo Transaction Batching Infrastructure
// ============================================================================

/// Pending edits for transaction batching.
///
/// Accumulates consecutive edits (typically character insertions in
/// insert mode) until a batch-breaking event occurs, then flushes
/// them as a single undo transaction.
///
/// # Design Philosophy
///
/// In Vim, typing "hello" in insert mode and pressing Escape creates
/// a single undo node - pressing `u` undoes all 5 characters at once.
/// This struct enables that behavior by collecting edits during insert
/// mode and committing them as a batch on mode exit or other break events.
///
/// # Batch Break Conditions
///
/// A batch is flushed when:
/// - Mode changes (e.g., Escape exits insert mode)
/// - A command is executed (e.g., Backspace, arrow keys)
/// - The buffer changes (different buffer ID)
///
/// # Example
///
/// ```ignore
/// // User types: ihello<Esc>
/// // Each 'h', 'e', 'l', 'l', 'o' calls accumulate_edit()
/// // <Esc> triggers flush_pending_edits()
/// // Result: single undo node containing all 5 edits
/// ```
#[derive(Debug, Default)]
pub struct PendingEditBatch {
    /// Buffer this batch applies to.
    ///
    /// When accumulating to a different buffer, the current batch
    /// is flushed before starting a new one.
    buffer_id: Option<BufferId>,

    /// Accumulated edits in order.
    ///
    /// Each edit is typically a single character insertion, but
    /// can be any edit operation.
    edits: Vec<Edit>,

    /// Cursor position before first edit in batch.
    ///
    /// Captured when the batch starts (first edit accumulated).
    /// Used as `cursor_before` when committing to undo tree.
    cursor_before: Option<Position>,

    /// Cursor position after most recent edit.
    ///
    /// Updated on each accumulate. Used as `cursor_after` when
    /// committing to undo tree.
    cursor_after: Option<Position>,
}

impl PendingEditBatch {
    /// Create a new empty pending edit batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the batch is empty (no pending edits).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Get the number of pending edits.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.edits.len()
    }

    /// Clear the batch, resetting all fields.
    pub fn clear(&mut self) {
        self.buffer_id = None;
        self.edits.clear();
        self.cursor_before = None;
        self.cursor_after = None;
    }
}

// ============================================================================
// Repeat Infrastructure
// ============================================================================

/// State for the repeat command (`.`).
///
/// Tracks the last repeatable command so that `.` can re-execute it.
/// Only certain commands are repeatable (text-modifying commands).
#[derive(Debug, Clone, Default)]
pub struct RepeatState {
    /// The last repeatable command ID.
    ///
    /// Stored as a string to avoid lifetime issues with `CommandId`.
    pub last_command: Option<String>,

    /// Text accumulated during insert mode.
    ///
    /// When insert mode is exited, this text is stored for `.` to replay.
    pub insert_text: String,

    /// Whether we're currently accumulating insert text.
    pub accumulating: bool,
}

impl RepeatState {
    /// Create a new empty repeat state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start accumulating insert mode text.
    pub fn start_accumulating(&mut self) {
        self.accumulating = true;
        self.insert_text.clear();
    }

    /// Add text to the insert accumulator.
    pub fn accumulate_insert(&mut self, text: &str) {
        if self.accumulating {
            self.insert_text.push_str(text);
        }
    }

    /// Stop accumulating and store the result.
    pub const fn stop_accumulating(&mut self) {
        self.accumulating = false;
    }

    /// Record a repeatable command.
    ///
    /// Only text-modifying commands should be recorded.
    pub fn record_command(&mut self, command_id: &str) {
        self.last_command = Some(command_id.to_string());
    }

    /// Clear the repeat state.
    pub fn clear(&mut self) {
        self.last_command = None;
        self.insert_text.clear();
        self.accumulating = false;
    }
}

// ============================================================================
// Block Insert Mode State
// ============================================================================

/// State for block insert mode (`I`/`A` in visual-block).
///
/// When active, text typed in insert mode is recorded and will be applied
/// to all lines in the block upon exiting insert mode.
///
/// # Block Insert Flow
///
/// 1. User selects text with visual-block mode (`<C-v>`)
/// 2. User presses `I` or `A` to start block insert
/// 3. This state is populated with block bounds
/// 4. User types text (shown at cursor on first line)
/// 5. User exits insert mode (Escape, Ctrl-C, etc.)
/// 6. Accumulated text is applied to all lines in the block
/// 7. State is cleared
#[derive(Debug, Clone, Default)]
pub struct BlockInsertState {
    /// Whether block insert is currently active.
    pub active: bool,
    /// First line of the block (0-indexed).
    pub start_line: usize,
    /// Last line of the block (0-indexed, inclusive).
    pub end_line: usize,
    /// Column where text will be inserted (0-indexed).
    ///
    /// For `I`, this is the left column of the block.
    /// For `A`, this is the right column + 1 (after the block).
    pub column: usize,
    /// Whether this is an append operation (`A`) vs insert (`I`).
    pub is_append: bool,
}

impl BlockInsertState {
    /// Create a new inactive block insert state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            start_line: 0,
            end_line: 0,
            column: 0,
            is_append: false,
        }
    }

    /// Start a block insert operation.
    #[allow(clippy::missing_const_for_fn)] // &mut self const fns not stable
    pub fn start(&mut self, start_line: usize, end_line: usize, column: usize, is_append: bool) {
        self.active = true;
        self.start_line = start_line;
        self.end_line = end_line;
        self.column = column;
        self.is_append = is_append;
    }

    /// Clear the block insert state.
    #[allow(clippy::missing_const_for_fn)] // &mut self const fns not stable
    pub fn clear(&mut self) {
        self.active = false;
        self.start_line = 0;
        self.end_line = 0;
        self.column = 0;
        self.is_append = false;
    }

    /// Check if block insert is currently active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Get the number of lines in the block.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        if self.active {
            self.end_line - self.start_line + 1
        } else {
            0
        }
    }
}

// ============================================================================
// Visual Mode Selection Infrastructure
// ============================================================================

/// Record of the last visual selection for `gv` (reselect) command.
///
/// When visual mode is exited, the selection boundaries are stored here
/// so that `gv` can restore the exact same selection.
#[derive(Debug, Clone)]
pub struct LastVisualSelection {
    /// The buffer where the selection was made.
    pub buffer_id: BufferId,
    /// The anchor position (where visual mode was entered).
    pub anchor: Position,
    /// The cursor position (where visual mode was exited).
    pub cursor: Position,
    /// The selection mode (Character, Line, or Block).
    pub mode: SelectionMode,
    /// The visual mode ID to restore (editor:visual, editor:visual-line, etc.).
    ///
    /// Storing the actual `ModeId` removes the need for hardcoded mode mapping
    /// from `SelectionMode` to `ModeId` in the runner.
    pub mode_id: ModeId,
}

impl LastVisualSelection {
    /// Create a new last visual selection record.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // ModeId cannot be const-constructed
    pub fn new(
        buffer_id: BufferId,
        anchor: Position,
        cursor: Position,
        mode: SelectionMode,
        mode_id: ModeId,
    ) -> Self {
        Self {
            buffer_id,
            anchor,
            cursor,
            mode,
            mode_id,
        }
    }
}

// ============================================================================
// Undotree Panel State
// ============================================================================

/// A rendered line for the undotree panel display.
#[derive(Debug, Clone)]
pub struct UndotreeRenderLine {
    /// The text content of this line.
    pub text: String,
    /// Whether this is the current node in the undo tree.
    pub is_current: bool,
    /// Whether this is the currently selected node.
    pub is_selected: bool,
}

/// A line in the diff preview display.
#[derive(Debug, Clone)]
pub struct DiffPreviewLine {
    /// The text content.
    pub text: String,
    /// Whether this is an insertion line.
    pub is_insert: bool,
    /// Whether this is a deletion line.
    pub is_delete: bool,
    /// Whether this is a header line.
    pub is_header: bool,
}

/// State for the undotree panel visualization.
///
/// Tracks the panel's visibility, associated window, source buffer,
/// and navigation state. This is a runtime concern (window manager level),
/// not kernel - the kernel provides the `UndoTree` data structure, while
/// the runner manages the panel UI.
#[derive(Debug, Clone, Default)]
pub struct UndotreeState {
    /// Whether the undotree panel is currently visible.
    panel_open: bool,
    /// Window ID of the panel when open.
    panel_window_id: Option<WindowId>,
    /// Buffer whose undo tree is being displayed.
    source_buffer_id: Option<BufferId>,
    /// Currently selected node index in the tree (for navigation).
    selected_node: usize,
    /// Scroll offset for rendering long trees.
    scroll_offset: usize,
    /// The window that was focused before the panel opened.
    ///
    /// Used to restore focus when the panel is closed.
    previous_window_id: Option<WindowId>,
    /// Rendered lines for display (cached).
    rendered_lines: Vec<UndotreeRenderLine>,
    /// Whether diff preview is currently active.
    preview_active: bool,
    /// Node index being previewed (may differ from selected).
    preview_node: Option<usize>,
    /// Cached diff lines for the previewed node.
    preview_diff_lines: Vec<DiffPreviewLine>,
}

impl UndotreeState {
    /// Create a new undotree state (panel closed by default).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            panel_open: false,
            panel_window_id: None,
            source_buffer_id: None,
            selected_node: 0,
            scroll_offset: 0,
            previous_window_id: None,
            rendered_lines: Vec::new(),
            preview_active: false,
            preview_node: None,
            preview_diff_lines: Vec::new(),
        }
    }

    /// Check if the panel is currently open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.panel_open
    }

    /// Get the panel's window ID if open.
    #[must_use]
    pub const fn panel_window_id(&self) -> Option<WindowId> {
        self.panel_window_id
    }

    /// Get the source buffer ID being displayed.
    #[must_use]
    pub const fn source_buffer_id(&self) -> Option<BufferId> {
        self.source_buffer_id
    }

    /// Get the currently selected node index.
    #[must_use]
    pub const fn selected_node(&self) -> usize {
        self.selected_node
    }

    /// Set the selected node index.
    pub const fn set_selected_node(&mut self, node: usize) {
        self.selected_node = node;
    }

    /// Get the scroll offset.
    #[must_use]
    pub const fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Set the scroll offset.
    pub const fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    /// Get the window that was focused before the panel opened.
    #[must_use]
    pub const fn previous_window_id(&self) -> Option<WindowId> {
        self.previous_window_id
    }

    /// Open the panel with the given window and source buffer.
    ///
    /// # Arguments
    ///
    /// * `window_id` - The window ID assigned to the panel
    /// * `buffer_id` - The buffer whose undo tree to display
    /// * `previous_window` - The window that was focused before opening
    pub const fn open(
        &mut self,
        window_id: WindowId,
        buffer_id: BufferId,
        previous_window: Option<WindowId>,
    ) {
        self.panel_open = true;
        self.panel_window_id = Some(window_id);
        self.source_buffer_id = Some(buffer_id);
        self.selected_node = 0; // Reset to root/current on open
        self.scroll_offset = 0;
        self.previous_window_id = previous_window;
    }

    /// Close the panel, returning the previous window ID for focus restoration.
    pub fn close(&mut self) -> Option<WindowId> {
        self.panel_open = false;
        self.panel_window_id = None;
        // Keep source_buffer_id for reference, but clear selection state
        self.selected_node = 0;
        self.scroll_offset = 0;
        self.rendered_lines.clear();
        // Clear preview state
        self.preview_active = false;
        self.preview_node = None;
        self.preview_diff_lines.clear();
        self.previous_window_id.take()
    }

    /// Update the rendered lines for display.
    pub fn set_rendered_lines(&mut self, lines: Vec<UndotreeRenderLine>) {
        self.rendered_lines = lines;
    }

    /// Get the rendered lines for display.
    #[must_use]
    pub fn rendered_lines(&self) -> &[UndotreeRenderLine] {
        &self.rendered_lines
    }

    /// Check if preview is currently active.
    #[must_use]
    pub const fn is_preview_active(&self) -> bool {
        self.preview_active
    }

    /// Get the node being previewed.
    #[must_use]
    pub const fn preview_node(&self) -> Option<usize> {
        self.preview_node
    }

    /// Set preview state for a node.
    pub fn set_preview(&mut self, node: usize, diff_lines: Vec<DiffPreviewLine>) {
        self.preview_active = true;
        self.preview_node = Some(node);
        self.preview_diff_lines = diff_lines;
    }

    /// Clear preview state.
    pub fn clear_preview(&mut self) {
        self.preview_active = false;
        self.preview_node = None;
        self.preview_diff_lines.clear();
    }

    /// Get the preview diff lines.
    #[must_use]
    pub fn preview_diff_lines(&self) -> &[DiffPreviewLine] {
        &self.preview_diff_lines
    }
}

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

    /// Block insert state for visual-block `I`/`A` commands.
    ///
    /// When block insert is active, text typed in insert mode is recorded
    /// and applied to all lines in the block when insert mode exits.
    pub block_insert: BlockInsertState,
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
            block_insert: BlockInsertState::new(),
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
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_visual_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("editor"), "visual")
    }

    fn test_visual_line_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("editor"), "visual-line")
    }

    fn test_visual_block_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("editor"), "visual-block")
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
        use reovim_kernel::api::v1::{Edit, Position};

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
        use reovim_kernel::api::v1::{Edit, Position};

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
    fn test_find_type_direction() {
        assert_eq!(FindType::FindForward.direction(), Direction::Forward);
        assert_eq!(FindType::FindBackward.direction(), Direction::Backward);
        assert_eq!(FindType::TillForward.direction(), Direction::Forward);
        assert_eq!(FindType::TillBackward.direction(), Direction::Backward);
    }

    #[test]
    fn test_find_type_is_till() {
        assert!(!FindType::FindForward.is_till());
        assert!(!FindType::FindBackward.is_till());
        assert!(FindType::TillForward.is_till());
        assert!(FindType::TillBackward.is_till());
    }

    #[test]
    fn test_char_wait_state_new() {
        let state = CharWaitState::new(FindType::FindForward, Position::new(0, 5));
        assert_eq!(state.find_type, FindType::FindForward);
        assert_eq!(state.start_position, Position::new(0, 5));
    }

    #[test]
    fn test_last_find_new() {
        let last = LastFind::new('x', FindType::FindForward);
        assert_eq!(last.char, 'x');
        assert_eq!(last.find_type, FindType::FindForward);
    }

    #[test]
    fn test_last_find_repeat_motion() {
        let last = LastFind::new('x', FindType::FindForward);
        let motion = last.repeat_motion();

        // Should create FindChar motion with same direction
        assert_eq!(
            motion,
            Motion::FindChar {
                char: 'x',
                direction: Direction::Forward,
                till: false,
            }
        );
    }

    #[test]
    fn test_last_find_reverse_motion() {
        let last = LastFind::new('x', FindType::FindForward);
        let motion = last.reverse_motion();

        // Should create FindChar motion with opposite direction
        assert_eq!(
            motion,
            Motion::FindChar {
                char: 'x',
                direction: Direction::Backward,
                till: false,
            }
        );
    }

    #[test]
    fn test_last_find_till_repeat_motion() {
        let last = LastFind::new('.', FindType::TillForward);
        let motion = last.repeat_motion();

        assert_eq!(
            motion,
            Motion::FindChar {
                char: '.',
                direction: Direction::Forward,
                till: true,
            }
        );
    }

    #[test]
    fn test_last_find_till_reverse_motion() {
        let last = LastFind::new('.', FindType::TillBackward);
        let motion = last.reverse_motion();

        // TillBackward reversed goes Forward
        assert_eq!(
            motion,
            Motion::FindChar {
                char: '.',
                direction: Direction::Forward,
                till: true,
            }
        );
    }

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
    // PendingCharOp Tests
    // ========================================================================

    #[test]
    fn test_pending_char_op_find_forward() {
        let op = PendingCharOp::find_forward(Position::new(0, 5));
        assert!(op.is_find_char());
        assert!(!op.is_replace_char());
        assert_eq!(op.find_type(), Some(FindType::FindForward));
        assert_eq!(op.start_position(), Some(Position::new(0, 5)));
    }

    #[test]
    fn test_pending_char_op_find_backward() {
        let op = PendingCharOp::find_backward(Position::new(1, 3));
        assert!(op.is_find_char());
        assert_eq!(op.find_type(), Some(FindType::FindBackward));
        assert_eq!(op.start_position(), Some(Position::new(1, 3)));
    }

    #[test]
    fn test_pending_char_op_till_forward() {
        let op = PendingCharOp::till_forward(Position::new(0, 0));
        assert!(op.is_find_char());
        assert_eq!(op.find_type(), Some(FindType::TillForward));
    }

    #[test]
    fn test_pending_char_op_till_backward() {
        let op = PendingCharOp::till_backward(Position::new(2, 10));
        assert!(op.is_find_char());
        assert_eq!(op.find_type(), Some(FindType::TillBackward));
    }

    #[test]
    fn test_pending_char_op_replace_char() {
        let op = PendingCharOp::replace_char(3);
        assert!(!op.is_find_char());
        assert!(op.is_replace_char());
        assert_eq!(op.find_type(), None);
        assert_eq!(op.start_position(), None);
    }

    #[test]
    fn test_pending_char_op_from_find_type() {
        let op = PendingCharOp::from_find_type(FindType::TillBackward, Position::new(5, 5));
        assert_eq!(op.find_type(), Some(FindType::TillBackward));
        assert_eq!(op.start_position(), Some(Position::new(5, 5)));
    }

    #[test]
    fn test_char_wait_state_to_pending_char_op() {
        let state = CharWaitState::new(FindType::FindForward, Position::new(0, 10));
        let op = state.to_pending_char_op();
        assert_eq!(op.find_type(), Some(FindType::FindForward));
        assert_eq!(op.start_position(), Some(Position::new(0, 10)));
    }

    #[test]
    fn test_char_wait_state_into_pending_char_op() {
        let state = CharWaitState::new(FindType::TillForward, Position::new(1, 2));
        let op: PendingCharOp = state.into();
        assert_eq!(op.find_type(), Some(FindType::TillForward));
    }

    // ========================================================================
    // RepeatState Tests
    // ========================================================================

    #[test]
    fn test_repeat_state_new() {
        let state = RepeatState::new();
        assert!(state.last_command.is_none());
        assert!(state.insert_text.is_empty());
        assert!(!state.accumulating);
    }

    #[test]
    fn test_repeat_state_record_command() {
        let mut state = RepeatState::new();
        state.record_command("delete-word");
        assert_eq!(state.last_command, Some("delete-word".to_string()));
    }

    #[test]
    fn test_repeat_state_accumulate_insert() {
        let mut state = RepeatState::new();
        state.start_accumulating();
        state.accumulate_insert("hello");
        state.accumulate_insert(" world");
        assert_eq!(state.insert_text, "hello world");
        state.stop_accumulating();
        assert!(!state.accumulating);
    }

    #[test]
    fn test_repeat_state_accumulate_only_when_active() {
        let mut state = RepeatState::new();
        // Should not accumulate when not started
        state.accumulate_insert("ignored");
        assert!(state.insert_text.is_empty());

        // Start accumulating
        state.start_accumulating();
        state.accumulate_insert("kept");
        assert_eq!(state.insert_text, "kept");
    }

    #[test]
    fn test_repeat_state_clear() {
        let mut state = RepeatState::new();
        state.record_command("change");
        state.start_accumulating();
        state.accumulate_insert("text");

        state.clear();
        assert!(state.last_command.is_none());
        assert!(state.insert_text.is_empty());
        assert!(!state.accumulating);
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
    // PendingEditBatch Tests
    // ========================================================================

    #[test]
    fn test_pending_edit_batch_new_is_empty() {
        let batch = PendingEditBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
        assert!(batch.buffer_id.is_none());
    }

    #[test]
    fn test_pending_edit_batch_clear() {
        let mut batch = PendingEditBatch::new();
        batch.buffer_id = Some(BufferId::from_raw(1));
        batch.edits.push(Edit::insert(Position::new(0, 0), "a"));
        batch.cursor_before = Some(Position::new(0, 0));
        batch.cursor_after = Some(Position::new(0, 1));

        assert!(!batch.is_empty());

        batch.clear();
        assert!(batch.is_empty());
        assert!(batch.buffer_id.is_none());
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
    // LastVisualSelection Tests
    // ========================================================================

    #[test]
    fn test_last_visual_selection_new() {
        let buffer_id = BufferId::from_raw(1);
        let anchor = Position::new(0, 5);
        let cursor = Position::new(2, 10);
        let mode = SelectionMode::Character;
        let mode_id = test_visual_mode_id();

        let selection = LastVisualSelection::new(buffer_id, anchor, cursor, mode, mode_id.clone());

        assert_eq!(selection.buffer_id, buffer_id);
        assert_eq!(selection.anchor, anchor);
        assert_eq!(selection.cursor, cursor);
        assert_eq!(selection.mode, mode);
        assert_eq!(selection.mode_id, mode_id);
    }

    #[test]
    fn test_last_visual_selection_line_mode() {
        let buffer_id = BufferId::from_raw(2);
        let selection = LastVisualSelection::new(
            buffer_id,
            Position::new(1, 0),
            Position::new(3, 0),
            SelectionMode::Line,
            test_visual_line_mode_id(),
        );

        assert_eq!(selection.mode, SelectionMode::Line);
    }

    #[test]
    fn test_last_visual_selection_block_mode() {
        let buffer_id = BufferId::from_raw(3);
        let selection = LastVisualSelection::new(
            buffer_id,
            Position::new(0, 0),
            Position::new(5, 10),
            SelectionMode::Block,
            test_visual_block_mode_id(),
        );

        assert_eq!(selection.mode, SelectionMode::Block);
    }

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
    fn test_undotree_state_default_closed() {
        let state = UndotreeState::new();

        assert!(!state.is_open());
        assert!(state.panel_window_id().is_none());
        assert!(state.source_buffer_id().is_none());
        assert_eq!(state.selected_node(), 0);
        assert_eq!(state.scroll_offset(), 0);
        assert!(state.previous_window_id().is_none());
    }

    #[test]
    fn test_undotree_state_open_close() {
        let mut state = UndotreeState::new();

        let window_id = WindowId::new(42);
        let buffer_id = BufferId::new();
        let prev_window = WindowId::new(1);

        // Open the panel
        state.open(window_id, buffer_id, Some(prev_window));

        assert!(state.is_open());
        assert_eq!(state.panel_window_id(), Some(window_id));
        assert_eq!(state.source_buffer_id(), Some(buffer_id));
        assert_eq!(state.selected_node(), 0); // Reset on open
        assert_eq!(state.previous_window_id(), Some(prev_window));

        // Close the panel
        let restored = state.close();

        assert!(!state.is_open());
        assert!(state.panel_window_id().is_none());
        // source_buffer_id is kept for reference
        assert!(state.source_buffer_id().is_some());
        assert_eq!(restored, Some(prev_window));
        assert!(state.previous_window_id().is_none()); // Taken during close
    }

    #[test]
    fn test_undotree_state_navigation() {
        let mut state = UndotreeState::new();

        // Set selection
        state.set_selected_node(5);
        assert_eq!(state.selected_node(), 5);

        state.set_selected_node(10);
        assert_eq!(state.selected_node(), 10);

        // Set scroll offset
        state.set_scroll_offset(3);
        assert_eq!(state.scroll_offset(), 3);
    }

    #[test]
    fn test_app_state_has_undotree_state() {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode_id());

        // AppState should have undotree_state initialized
        assert!(!app.undotree_state.is_open());
        assert!(app.undotree_state.panel_window_id().is_none());
    }

    #[test]
    fn test_undotree_state_open_without_previous_window() {
        let mut state = UndotreeState::new();

        let window_id = WindowId::new(42);
        let buffer_id = BufferId::new();

        // Open without previous window (first window scenario)
        state.open(window_id, buffer_id, None);

        assert!(state.is_open());
        assert!(state.previous_window_id().is_none());

        // Close should return None
        let restored = state.close();
        assert!(restored.is_none());
    }

    #[test]
    fn test_undotree_state_preview_initially_inactive() {
        let state = UndotreeState::new();

        assert!(!state.is_preview_active());
        assert!(state.preview_node().is_none());
        assert!(state.preview_diff_lines().is_empty());
    }

    #[test]
    fn test_undotree_state_set_preview_activates() {
        let mut state = UndotreeState::new();

        let diff_lines = vec![
            DiffPreviewLine {
                text: "+hello".to_string(),
                is_insert: true,
                is_delete: false,
                is_header: false,
            },
            DiffPreviewLine {
                text: "-world".to_string(),
                is_insert: false,
                is_delete: true,
                is_header: false,
            },
        ];

        state.set_preview(5, diff_lines);

        assert!(state.is_preview_active());
        assert_eq!(state.preview_node(), Some(5));
        assert_eq!(state.preview_diff_lines().len(), 2);
    }

    #[test]
    fn test_undotree_state_clear_preview_deactivates() {
        let mut state = UndotreeState::new();

        let diff_lines = vec![DiffPreviewLine {
            text: "+test".to_string(),
            is_insert: true,
            is_delete: false,
            is_header: false,
        }];

        state.set_preview(3, diff_lines);
        assert!(state.is_preview_active());

        state.clear_preview();

        assert!(!state.is_preview_active());
        assert!(state.preview_node().is_none());
        assert!(state.preview_diff_lines().is_empty());
    }

    #[test]
    fn test_undotree_state_close_clears_preview() {
        let mut state = UndotreeState::new();

        let window_id = WindowId::new(42);
        let buffer_id = BufferId::new();

        state.open(window_id, buffer_id, None);

        let diff_lines = vec![DiffPreviewLine {
            text: "@@header@@".to_string(),
            is_insert: false,
            is_delete: false,
            is_header: true,
        }];
        state.set_preview(2, diff_lines);

        assert!(state.is_preview_active());

        state.close();

        assert!(!state.is_preview_active());
        assert!(state.preview_node().is_none());
        assert!(state.preview_diff_lines().is_empty());
    }

    #[test]
    fn test_undotree_state_preview_node_tracking() {
        let mut state = UndotreeState::new();

        // Preview different nodes
        state.set_preview(1, vec![]);
        assert_eq!(state.preview_node(), Some(1));

        state.set_preview(7, vec![]);
        assert_eq!(state.preview_node(), Some(7));

        // Selection is independent of preview
        state.set_selected_node(3);
        assert_eq!(state.selected_node(), 3);
        assert_eq!(state.preview_node(), Some(7));
    }
}
