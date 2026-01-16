//! Application state for the runner.
//!
//! Separates runtime concerns from kernel context. The kernel provides
//! core services (buffers, events, options); the runner tracks runtime
//! state like active buffer and mode stack.

use {
    crate::UndoRegistry,
    reovim_arch::sync::RwLock,
    reovim_driver_input::{FallbackContext, KeySequence},
    reovim_kernel::api::v1::{
        Buffer, BufferId, Direction, Edit, KernelContext, ModeId, ModeStack, Motion, Position,
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
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
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
}
