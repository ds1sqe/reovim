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
// Char-Wait Infrastructure
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

/// State for commands waiting for a character argument.
///
/// When a find-char command (f/F/t/T) is executed, it returns `WaitingForChar`
/// and this state is set. The next key press provides the character argument
/// to complete the motion.
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
    pub char_wait: Option<CharWaitState>,

    /// Last find-char operation for `;` and `,` repeat commands.
    ///
    /// Updated each time a find-char motion successfully moves the cursor.
    pub last_find: Option<LastFind>,

    /// Search state for / and ? commands.
    ///
    /// Tracks the current pattern, direction, highlighting, and input mode.
    /// Pattern persists across buffer switches (like Vim).
    pub search: SearchState,
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
            last_find: None,
            search: SearchState::new(),
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
}
