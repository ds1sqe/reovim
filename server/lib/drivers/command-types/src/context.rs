//! Command execution context.
//!
//! This module provides [`CommandContext`], which carries all inputs for command execution.

use {
    crate::args::ArgValue,
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{BufferId, Position},
    std::{collections::HashMap, sync::Arc},
};

/// Context carrying all command inputs.
///
/// Provides typed access to arguments parsed from user input, plus
/// optional access to the virtual filesystem for file operations.
///
/// # Example
///
/// ```
/// use reovim_driver_command_types::{CommandContext, ArgValue};
///
/// let mut ctx = CommandContext::new();
/// ctx.set("count", ArgValue::Count(5));
/// ctx.set("register", ArgValue::Register('a'));
///
/// assert_eq!(ctx.count(), Some(5));
/// assert_eq!(ctx.register(), Some('a'));
/// ```
#[derive(Clone, Default)]
pub struct CommandContext {
    args: HashMap<&'static str, ArgValue>,
    /// Optional VFS access for file operations.
    ///
    /// Set by the runner before dispatching commands that may need
    /// filesystem access (e.g., `:w`, `:e`).
    vfs: Option<Arc<dyn VfsDriver>>,
    /// Current mode name (e.g., "normal", "operator-pending").
    ///
    /// Set by the runner before dispatching commands. Commands can use
    /// this to adjust their behavior based on the current mode.
    mode_name: Option<String>,
}

impl std::fmt::Debug for CommandContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandContext")
            .field("args", &self.args)
            .field("vfs", &self.vfs.as_ref().map(|_| "<VfsDriver>"))
            .field("mode_name", &self.mode_name)
            .finish()
    }
}

impl CommandContext {
    /// Create a new empty command context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an argument value.
    pub fn set(&mut self, name: &'static str, value: ArgValue) {
        self.args.insert(name, value);
    }

    /// Get an argument value by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ArgValue> {
        self.args.get(name)
    }

    /// Get the count argument, if present.
    #[must_use]
    pub fn count(&self) -> Option<usize> {
        match self.args.get("count") {
            Some(ArgValue::Count(n)) => Some(*n),
            _ => None,
        }
    }

    /// Get the register argument, if present.
    #[must_use]
    pub fn register(&self) -> Option<char> {
        match self.args.get("register") {
            Some(ArgValue::Register(c)) => Some(*c),
            _ => None,
        }
    }

    /// Check if the bang modifier is set.
    #[must_use]
    pub fn has_bang(&self) -> bool {
        matches!(self.args.get("bang"), Some(ArgValue::Bang(true)))
    }

    /// Get a string argument by name.
    #[must_use]
    pub fn string(&self, name: &str) -> Option<&str> {
        match self.args.get(name) {
            Some(ArgValue::String(s) | ArgValue::FilePath(s) | ArgValue::Motion(s)) => Some(s),
            _ => None,
        }
    }

    /// Get a range argument, if present.
    #[must_use]
    pub fn range(&self) -> Option<(usize, usize)> {
        match self.args.get("range") {
            Some(ArgValue::Range(start, end)) => Some((*start, *end)),
            _ => None,
        }
    }

    /// Get the active buffer ID, if present.
    ///
    /// The buffer ID is set by the runner before command execution
    /// to indicate which buffer the command should operate on.
    #[must_use]
    pub fn buffer_id(&self) -> Option<BufferId> {
        match self.args.get("buffer_id") {
            Some(ArgValue::BufferId(id)) => Some(BufferId::from_raw(*id)),
            _ => None,
        }
    }

    /// Set the active buffer ID.
    ///
    /// Called by the runner before dispatching a command.
    pub fn set_buffer_id(&mut self, id: BufferId) {
        self.args
            .insert("buffer_id", ArgValue::BufferId(id.as_usize()));
    }

    /// Create a command context with VFS access.
    ///
    /// Builder method for creating a context with filesystem access.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ctx = CommandContext::new().with_vfs(session_state.vfs.clone());
    /// ```
    #[must_use]
    pub fn with_vfs(mut self, vfs: Arc<dyn VfsDriver>) -> Self {
        self.vfs = Some(vfs);
        self
    }

    /// Set the VFS for this context.
    ///
    /// Called by the runner before dispatching commands that need
    /// filesystem access.
    pub fn set_vfs(&mut self, vfs: Arc<dyn VfsDriver>) {
        self.vfs = Some(vfs);
    }

    /// Get the VFS driver, if available.
    ///
    /// Returns `None` if no VFS was set for this context. Commands
    /// that need filesystem access should check this and return an
    /// appropriate error if VFS is unavailable.
    #[must_use]
    pub fn vfs(&self) -> Option<&Arc<dyn VfsDriver>> {
        self.vfs.as_ref()
    }

    /// Set the current mode name.
    ///
    /// Called by the runner before dispatching commands. Commands can
    /// use `mode_name()` to adjust behavior based on the current mode.
    pub fn set_mode_name(&mut self, mode: impl Into<String>) {
        self.mode_name = Some(mode.into());
    }

    /// Get the current mode name.
    ///
    /// Returns the mode name (e.g., "normal", "operator-pending", "insert").
    /// Commands can use this to adjust behavior. For example, motion commands
    /// return `CommandResult::OperatorRange` in operator-pending mode instead
    /// of moving the cursor.
    #[must_use]
    pub fn mode_name(&self) -> Option<&str> {
        self.mode_name.as_deref()
    }

    /// Check if the current mode is operator-pending.
    ///
    /// Convenience method for motion commands to check if they should return
    /// a range instead of moving the cursor.
    #[must_use]
    pub fn is_operator_pending(&self) -> bool {
        self.mode_name
            .as_ref()
            .is_some_and(|m| m == "operator-pending")
    }

    /// Get a character argument by name.
    ///
    /// Used for commands that need a single character input, such as
    /// find-char (f/F/t/T) or replace (r).
    #[must_use]
    pub fn char(&self, name: &str) -> Option<char> {
        match self.args.get(name) {
            Some(ArgValue::Char(c)) => Some(*c),
            _ => None,
        }
    }

    /// Get the start position for operator ranges (Epic #415).
    ///
    /// Returns `(line, column)` of the range start position.
    #[must_use]
    pub fn range_start(&self) -> Option<(usize, usize)> {
        match self.args.get("range_start") {
            Some(ArgValue::Position(line, col)) => Some((*line, *col)),
            _ => None,
        }
    }

    /// Get the end position for operator ranges (Epic #415).
    ///
    /// Returns `(line, column)` of the range end position.
    #[must_use]
    pub fn range_end(&self) -> Option<(usize, usize)> {
        match self.args.get("range_end") {
            Some(ArgValue::Position(line, col)) => Some((*line, *col)),
            _ => None,
        }
    }

    /// Check if this is a linewise operation (Epic #415).
    ///
    /// When true, operators should expand the range to full lines.
    #[must_use]
    pub fn is_linewise(&self) -> bool {
        matches!(self.args.get("linewise"), Some(ArgValue::Bang(true)))
    }

    // === Per-Window State (Issue #471) ===
    //
    // Cursor and selection are per-WINDOW properties, not per-buffer.
    // A buffer can appear in multiple windows (`:split`), each with its own cursor.
    //
    // The runner sets these from the client's ACTIVE window before dispatching
    // commands. Commands should use these methods, NOT search for cursor position.

    /// Get the cursor position from the active window.
    ///
    /// This is the cursor position explicitly passed by the runner from the
    /// client's active window. Commands should use this instead of searching
    /// for cursor position through window lookups.
    ///
    /// Returns `None` if cursor was not set. Commands should fail explicitly
    /// rather than use a fallback position (Fail Loud Policy).
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
    ///     let Some(pos) = args.cursor_position() else {
    ///         return CommandResult::error("No cursor position");
    ///     };
    ///     // Use pos for operation...
    /// }
    /// ```
    #[must_use]
    pub fn cursor_position(&self) -> Option<Position> {
        match self.args.get("cursor") {
            Some(ArgValue::Position(line, col)) => Some(Position::new(*line, *col)),
            _ => None,
        }
    }

    /// Set the cursor position from the active window.
    ///
    /// Called by the runner before dispatching a command. The runner gets
    /// the cursor from the client's active window and passes it explicitly.
    ///
    /// # Why Explicit Passing
    ///
    /// A buffer can appear in multiple windows (`:split`). Using
    /// `cursor_position(buffer_id)` would find the FIRST window with that
    /// buffer, which may not be the ACTIVE window. Explicit passing ensures
    /// we always use the correct cursor position.
    pub fn set_cursor_position(&mut self, pos: Position) {
        self.args
            .insert("cursor", ArgValue::Position(pos.line, pos.column));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_context_new() {
        let ctx = CommandContext::new();
        assert!(ctx.count().is_none());
        assert!(ctx.register().is_none());
        assert!(!ctx.has_bang());
    }

    #[test]
    fn test_command_context_count() {
        let mut ctx = CommandContext::new();
        ctx.set("count", ArgValue::Count(5));
        assert_eq!(ctx.count(), Some(5));
    }

    #[test]
    fn test_command_context_register() {
        let mut ctx = CommandContext::new();
        ctx.set("register", ArgValue::Register('a'));
        assert_eq!(ctx.register(), Some('a'));
    }

    #[test]
    fn test_command_context_bang() {
        let mut ctx = CommandContext::new();
        assert!(!ctx.has_bang());

        ctx.set("bang", ArgValue::Bang(true));
        assert!(ctx.has_bang());
    }

    #[test]
    fn test_command_context_string() {
        let mut ctx = CommandContext::new();
        ctx.set("file", ArgValue::String("test.txt".into()));
        assert_eq!(ctx.string("file"), Some("test.txt"));
    }

    #[test]
    fn test_command_context_range() {
        let mut ctx = CommandContext::new();
        ctx.set("range", ArgValue::Range(1, 10));
        assert_eq!(ctx.range(), Some((1, 10)));
    }

    #[test]
    fn test_command_context_buffer_id() {
        let mut ctx = CommandContext::new();
        assert!(ctx.buffer_id().is_none());

        let id = BufferId::from_raw(42);
        ctx.set_buffer_id(id);
        assert_eq!(ctx.buffer_id(), Some(BufferId::from_raw(42)));
    }

    #[test]
    fn test_command_context_vfs_none_by_default() {
        let ctx = CommandContext::new();
        assert!(ctx.vfs().is_none());
    }

    #[test]
    fn test_command_context_set_vfs() {
        use reovim_driver_vfs::MockVfs;

        let mut ctx = CommandContext::new();
        let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
        ctx.set_vfs(vfs);
        assert!(ctx.vfs().is_some());
    }

    #[test]
    fn test_command_context_with_vfs() {
        use reovim_driver_vfs::MockVfs;

        let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
        let ctx = CommandContext::new().with_vfs(vfs);
        assert!(ctx.vfs().is_some());
    }

    #[test]
    fn test_command_context_debug_with_vfs() {
        use reovim_driver_vfs::MockVfs;

        let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
        let ctx = CommandContext::new().with_vfs(vfs);
        let debug_str = format!("{ctx:?}");
        assert!(debug_str.contains("<VfsDriver>"));
    }

    #[test]
    fn test_command_context_mode_name_none_by_default() {
        let ctx = CommandContext::new();
        assert!(ctx.mode_name().is_none());
    }

    #[test]
    fn test_command_context_set_mode_name() {
        let mut ctx = CommandContext::new();
        ctx.set_mode_name("operator-pending");
        assert_eq!(ctx.mode_name(), Some("operator-pending"));
    }

    #[test]
    fn test_command_context_is_operator_pending() {
        let mut ctx = CommandContext::new();
        assert!(!ctx.is_operator_pending());

        ctx.set_mode_name("operator-pending");
        assert!(ctx.is_operator_pending());

        ctx.set_mode_name("normal");
        assert!(!ctx.is_operator_pending());
    }

    #[test]
    fn test_command_context_cursor_position_none_by_default() {
        let ctx = CommandContext::new();
        assert!(ctx.cursor_position().is_none());
    }

    #[test]
    fn test_command_context_set_cursor_position() {
        use reovim_kernel::api::v1::Position;

        let mut ctx = CommandContext::new();
        let pos = Position::new(10, 5);
        ctx.set_cursor_position(pos);

        let result = ctx.cursor_position();
        assert!(result.is_some());
        let cursor = result.unwrap();
        assert_eq!(cursor.line, 10);
        assert_eq!(cursor.column, 5);
    }

    #[test]
    fn test_command_context_cursor_position_explicit_passing() {
        use reovim_kernel::api::v1::Position;

        // Simulating runner pattern: set cursor before command dispatch
        let mut ctx = CommandContext::new();

        // Runner sets cursor from active window
        ctx.set_cursor_position(Position::new(42, 17));

        // Command reads cursor - should get exactly what was set
        let pos = ctx.cursor_position().expect("cursor should be set");
        assert_eq!(pos.line, 42);
        assert_eq!(pos.column, 17);
    }
}
