//! Command driver for reovim - command execution framework.
//!
//! Linux equivalent: `drivers/block/` (block command interface)
//!
//! # Architecture
//!
//! This crate defines the command execution framework for reovim.
//! Commands implement [`Command`] for metadata and [`CommandHandler`] for execution.
//!
//! ```text
//! lib/drivers/command/      <-- Command framework (this crate)
//!        ^
//!        |  (modules implement commands)
//!        |
//! modules/                  <-- Policy: actual command implementations
//! ```
//!
//! # Components
//!
//! - [`Command`] - Self-describing command metadata
//! - [`CommandHandler`] - Command execution trait
//! - [`ArgSpec`] - Argument specification
//! - [`ArgKind`], [`ArgValue`] - Argument types
//! - [`CommandContext`] - Context carrying all command inputs
//! - [`CommandResult`] - Command execution result
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_command::{Command, CommandHandler, CommandContext, CommandResult, ArgSpec, ArgKind};
//! use reovim_kernel::api::v1::{CommandId, KernelContext, ModuleId};
//!
//! const MY_MODULE: ModuleId = ModuleId::new_const("my-module");
//!
//! pub struct CursorDown;
//!
//! impl Command for CursorDown {
//!     fn id(&self) -> CommandId {
//!         CommandId::new(MY_MODULE, "cursor-down")
//!     }
//!
//!     fn description(&self) -> &'static str {
//!         "Move cursor down"
//!     }
//!
//!     fn args(&self) -> Vec<ArgSpec> {
//!         vec![ArgSpec::optional("count", ArgKind::Count, "Number of lines")]
//!     }
//! }
//!
//! impl CommandHandler for CursorDown {
//!     fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
//!         let count = args.count().unwrap_or(1);
//!         // Move cursor down by count lines
//!         CommandResult::Success
//!     }
//! }
//! ```

use {
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{BufferId, CommandId, Edit, KernelContext, Position},
    std::{collections::HashMap, sync::Arc},
};

// ============================================================================
// Command Trait
// ============================================================================

/// Self-describing command metadata.
///
/// Commands implement this trait to provide metadata about themselves:
/// - Unique identifier ([`CommandId`])
/// - Human-readable description
/// - Argument specifications
/// - Command aliases (for ex commands like `:w`, `:write`)
///
/// # Design Philosophy
///
/// This trait separates command metadata from execution. The [`CommandHandler`]
/// trait handles actual execution. This allows querying command information
/// without executing.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_command::{Command, ArgSpec, ArgKind};
/// use reovim_kernel::api::v1::{CommandId, ModuleId};
///
/// struct DeleteLine;
///
/// impl Command for DeleteLine {
///     fn id(&self) -> CommandId {
///         CommandId::new(ModuleId::new("editor"), "delete-line")
///     }
///
///     fn description(&self) -> &'static str {
///         "Delete the current line"
///     }
///
///     fn args(&self) -> Vec<ArgSpec> {
///         vec![ArgSpec::optional("count", ArgKind::Count, "Number of lines")]
///     }
/// }
/// ```
pub trait Command: Send + Sync + 'static {
    /// Get the unique identifier for this command.
    fn id(&self) -> CommandId;

    /// Get a human-readable description of what this command does.
    fn description(&self) -> &'static str;

    /// Get the argument specifications for this command.
    ///
    /// Returns an empty vector if the command takes no arguments.
    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }

    /// Get command name aliases.
    ///
    /// These are used for ex commands (command-line mode). For example,
    /// `:w` and `:write` are aliases for the same command.
    fn names(&self) -> &[&'static str] {
        &[]
    }
}

// ============================================================================
// CommandHandler Trait
// ============================================================================

/// Command execution trait.
///
/// Commands that can be executed implement this trait in addition to [`Command`].
///
/// # Design Philosophy
///
/// Separating execution from metadata allows:
/// - Querying command info without execution capability
/// - Different execution strategies (sync, async, background)
/// - Testing command metadata independently
///
/// # Example
///
/// ```ignore
/// use reovim_driver_command::{Command, CommandHandler, CommandContext, CommandResult};
/// use reovim_kernel::api::v1::{CommandId, KernelContext, ModuleId};
///
/// struct HelloCommand;
///
/// impl Command for HelloCommand {
///     fn id(&self) -> CommandId {
///         CommandId::new(ModuleId::new("example"), "hello")
///     }
///     fn description(&self) -> &'static str { "Say hello" }
/// }
///
/// impl CommandHandler for HelloCommand {
///     fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
///         println!("Hello!");
///         CommandResult::Success
///     }
/// }
/// ```
pub trait CommandHandler: Command {
    /// Execute the command.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The kernel context providing access to buffers, windows, etc.
    /// * `args` - The command arguments parsed from user input
    ///
    /// # Returns
    ///
    /// A [`CommandResult`] indicating success, error, or special results like quit.
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult;
}

// ============================================================================
// Argument Types
// ============================================================================

/// Argument specification for self-describing commands.
///
/// Defines metadata about a command argument including its name, type,
/// description, and whether it's required.
#[derive(Debug, Clone)]
pub struct ArgSpec {
    /// The argument name (used in `CommandContext::get`).
    pub name: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// The kind of argument.
    pub kind: ArgKind,
    /// Whether this argument is required.
    pub required: bool,
}

impl ArgSpec {
    /// Create a required argument specification.
    #[must_use]
    pub const fn required(name: &'static str, kind: ArgKind, description: &'static str) -> Self {
        Self {
            name,
            description,
            kind,
            required: true,
        }
    }

    /// Create an optional argument specification.
    #[must_use]
    pub const fn optional(name: &'static str, kind: ArgKind, description: &'static str) -> Self {
        Self {
            name,
            description,
            kind,
            required: false,
        }
    }
}

/// The kind of argument a command accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    /// Numeric count (e.g., `3j` for "move down 3 lines").
    Count,
    /// Register name (e.g., `"a` for register 'a').
    Register,
    /// Motion command (e.g., `w` in `dw` for "delete word").
    Motion,
    /// Line range (e.g., `1,5` for "lines 1 through 5").
    Range,
    /// File path (e.g., `foo.txt` in `:w foo.txt`).
    FilePath,
    /// Generic string argument.
    String,
    /// Bang modifier (e.g., `!` in `:q!`).
    Bang,
    /// Buffer identifier (set by runner before command execution).
    BufferId,
}

/// Argument value parsed from user input.
#[derive(Debug, Clone)]
pub enum ArgValue {
    /// A numeric count.
    Count(usize),
    /// A register name.
    Register(char),
    /// A motion identifier (command ID as string for simplicity).
    Motion(String),
    /// A line range (start, end).
    Range(usize, usize),
    /// A file path.
    FilePath(String),
    /// A generic string.
    String(String),
    /// A bang modifier.
    Bang(bool),
    /// A buffer identifier (raw usize, converted to `BufferId` by helper).
    BufferId(usize),
}

// ============================================================================
// CommandContext
// ============================================================================

/// Context carrying all command inputs.
///
/// Provides typed access to arguments parsed from user input, plus
/// optional access to the virtual filesystem for file operations.
///
/// # Example
///
/// ```
/// use reovim_driver_command::{CommandContext, ArgValue};
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
}

impl std::fmt::Debug for CommandContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandContext")
            .field("args", &self.args)
            .field("vfs", &self.vfs.as_ref().map(|_| "<VfsDriver>"))
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
}

// ============================================================================
// Char-Wait Types (for find-char and replace-char commands)
// ============================================================================

/// Type of find-char operation.
///
/// Represents the four find-char motions in Vim.
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

/// Type of character-waiting operation.
///
/// Generalizes `FindType` to include operations beyond find-char motions.
/// Commands return this to indicate what operation is waiting for character input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharWaitOp {
    /// Find character forward, cursor on char (f)
    FindForward,
    /// Find character backward, cursor on char (F)
    FindBackward,
    /// Till character forward, cursor before char (t)
    TillForward,
    /// Till character backward, cursor after char (T)
    TillBackward,
    /// Replace character at cursor position (r{char})
    ReplaceChar,
}

impl CharWaitOp {
    /// Check if this is a find-char operation.
    #[must_use]
    pub const fn is_find_char(&self) -> bool {
        matches!(
            self,
            Self::FindForward | Self::FindBackward | Self::TillForward | Self::TillBackward
        )
    }

    /// Check if this is a replace-char operation.
    #[must_use]
    pub const fn is_replace_char(&self) -> bool {
        matches!(self, Self::ReplaceChar)
    }

    /// Convert to `FindType` if this is a find-char operation.
    #[must_use]
    pub const fn to_find_type(&self) -> Option<FindType> {
        match self {
            Self::FindForward => Some(FindType::FindForward),
            Self::FindBackward => Some(FindType::FindBackward),
            Self::TillForward => Some(FindType::TillForward),
            Self::TillBackward => Some(FindType::TillBackward),
            Self::ReplaceChar => None,
        }
    }
}

impl From<FindType> for CharWaitOp {
    fn from(find_type: FindType) -> Self {
        match find_type {
            FindType::FindForward => Self::FindForward,
            FindType::FindBackward => Self::FindBackward,
            FindType::TillForward => Self::TillForward,
            FindType::TillBackward => Self::TillBackward,
        }
    }
}

/// Context for a command waiting for character input.
///
/// Commands that need a character argument return this to indicate what
/// operation is pending. The runner uses this to set up pending-char state.
///
/// # Example
///
/// ```ignore
/// // Find-char-forward command (f)
/// fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
///     let pos = ctx.buffers.get(args.buffer_id().unwrap())
///         .unwrap()
///         .read()
///         .position();
///
///     CommandResult::WaitingForChar(CharWaitContext {
///         op_type: CharWaitOp::FindForward,
///         count: None,
///         start_position: Some(pos),
///     })
/// }
///
/// // Replace-char command (r)
/// fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
///     let count = args.count().unwrap_or(1);
///     CommandResult::WaitingForChar(CharWaitContext {
///         op_type: CharWaitOp::ReplaceChar,
///         count: Some(count),
///         start_position: None,
///     })
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharWaitContext {
    /// The type of character-waiting operation.
    pub op_type: CharWaitOp,
    /// Count for the operation (e.g., 3rx replaces 3 chars).
    pub count: Option<usize>,
    /// Starting cursor position (for find-char operator range calculation).
    ///
    /// Required for find-char operations, not needed for replace-char.
    pub start_position: Option<Position>,
}

impl CharWaitContext {
    /// Create a new char-wait context for find-char operations.
    #[must_use]
    pub const fn find_char(find_type: FindType, start_position: Position) -> Self {
        Self {
            op_type: match find_type {
                FindType::FindForward => CharWaitOp::FindForward,
                FindType::FindBackward => CharWaitOp::FindBackward,
                FindType::TillForward => CharWaitOp::TillForward,
                FindType::TillBackward => CharWaitOp::TillBackward,
            },
            count: None,
            start_position: Some(start_position),
        }
    }

    /// Create a new char-wait context for replace-char operation.
    #[must_use]
    pub const fn replace_char(count: usize) -> Self {
        Self {
            op_type: CharWaitOp::ReplaceChar,
            count: Some(count),
            start_position: None,
        }
    }

    /// DEPRECATED: Create a new char-wait context (backward compatibility).
    ///
    /// Use `find_char()` or `replace_char()` instead.
    #[must_use]
    pub const fn new(find_type: FindType, start_position: Position) -> Self {
        Self::find_char(find_type, start_position)
    }

    /// Get the `FindType` if this is a find-char operation.
    #[must_use]
    pub const fn find_type(&self) -> Option<FindType> {
        self.op_type.to_find_type()
    }
}

// ============================================================================
// Search Types (for search commands)
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

/// Search action intent returned by commands.
///
/// Commands return this to request search operations. The runner handles
/// the actual search execution, input mode management, and pattern storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAction {
    /// Enter search input mode (/ or ?)
    EnterSearchMode { direction: SearchDirection },
    /// Go to next match in the same direction (n)
    Next,
    /// Go to previous match / reverse direction (N)
    Previous,
    /// Search word under cursor (* or #)
    WordUnderCursor { direction: SearchDirection },
    /// Clear search highlighting (:noh)
    ClearHighlight,
}

// ============================================================================
// CommandResult
// ============================================================================

/// Result of command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    /// Command executed successfully.
    Success,
    /// Command failed with an error message.
    Error(String),
    /// Command requests editor to quit.
    Quit,
    /// Command requests editor to quit without saving.
    ForceQuit,
    /// Command requests an undo/redo action to be performed by the runner.
    ///
    /// This follows the callback pattern where commands declare WHAT they want
    /// (intent), and the runner decides HOW to execute it (policy).
    UndoAction(UndoAction),
    /// Command requests an undotree visualization action.
    ///
    /// This follows the same callback pattern as `UndoAction`, where commands
    /// declare intent and the runner handles execution.
    UndotreeAction(UndotreeAction),
    /// Command reports edits it made to a buffer.
    ///
    /// The runner records these edits in the undo registry for later
    /// undo/redo operations.
    EditAction(EditAction),
    /// Command needs a character argument before it can complete.
    ///
    /// Find-char commands (f, F, t, T) and replace-char command (r) return
    /// this to indicate they need the next keypress as a character argument.
    /// The runner sets pending-char state and waits for the character input.
    WaitingForChar(CharWaitContext),
    /// Repeat the last find-char motion in the same direction (;).
    ///
    /// The runner executes `last_find.repeat_motion()` if `last_find` is set.
    RepeatFindSame,
    /// Repeat the last find-char motion in the opposite direction (,).
    ///
    /// The runner executes `last_find.reverse_motion()` if `last_find` is set.
    RepeatFindReverse,
    /// Command requests a search action.
    ///
    /// Search commands (/, ?, n, N, *, #, :noh) return this to indicate
    /// what search operation should be performed. The runner handles
    /// input mode, pattern storage, and search execution.
    SearchAction(SearchAction),
    /// Repeat the last repeatable command (.).
    ///
    /// The runner replays the last repeatable command from `repeat_state`.
    /// This includes text-modifying commands and any accumulated insert text.
    RepeatAction,
}

/// Undo/redo action intent returned by commands.
///
/// Commands return this to request undo/redo operations. The runner
/// handles the actual undo tree manipulation, maintaining separation
/// of concerns between command (policy) and runner (mechanism).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoAction {
    /// Request to undo the specified number of changes.
    Undo { count: usize },
    /// Request to redo the specified number of changes.
    Redo { count: usize },
}

/// Undotree visualization action intent returned by commands.
///
/// Commands return this to request undotree operations. The runner
/// handles the actual panel creation, navigation, and tree traversal,
/// maintaining separation of concerns between command (policy) and
/// runner (mechanism).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndotreeAction {
    /// Toggle undotree panel for the specified buffer.
    Toggle { buffer_id: usize },
    /// Close undotree panel.
    Close,
    /// Navigate to a specific node in the undotree.
    GotoNode { node_index: usize },
    /// Go to the currently selected node.
    GotoSelected,
    /// Move selection up (toward parent).
    MoveUp,
    /// Move selection down (toward child).
    MoveDown,
}

/// Edit action intent returned by commands that modify buffer content.
///
/// Commands return this to report edits they made. The runner records
/// these edits in the undo registry for later undo/redo operations.
///
/// # Design Philosophy
///
/// This follows the callback pattern where commands declare WHAT they did
/// (the edit), and the runner decides HOW to handle it (record in undo tree).
///
/// # Example
///
/// ```ignore
/// fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
///     let buffer_id = args.buffer_id().unwrap();
///     let buffer = ctx.buffers.get(buffer_id).unwrap();
///
///     let cursor_before = buffer.position();
///     let edit = buffer.insert("hello");
///     let cursor_after = buffer.position();
///
///     CommandResult::EditAction(EditAction::new(
///         buffer_id, vec![edit], cursor_before, cursor_after
///     ))
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditAction {
    /// The buffer that was edited.
    pub buffer_id: BufferId,
    /// The edits that were made (in order applied).
    pub edits: Vec<Edit>,
    /// Cursor position before the edits were applied.
    pub cursor_before: Position,
    /// Cursor position after the edits were applied.
    pub cursor_after: Position,
}

impl EditAction {
    /// Create a new edit action.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec cannot be const-constructed
    pub fn new(
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Self {
        Self {
            buffer_id,
            edits,
            cursor_before,
            cursor_after,
        }
    }

    /// Create an edit action from a single edit.
    #[must_use]
    pub fn single(
        buffer_id: BufferId,
        edit: Edit,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Self {
        Self::new(buffer_id, vec![edit], cursor_before, cursor_after)
    }

    /// Check if this action has no edits (no-op).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty is not const stable
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }
}

impl CommandResult {
    /// Check if the result is success.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Check if the result is an error.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Check if the result requests quit.
    #[must_use]
    pub const fn is_quit(&self) -> bool {
        matches!(self, Self::Quit | Self::ForceQuit)
    }

    /// Check if the result is an undo/redo action.
    #[must_use]
    pub const fn is_undo_action(&self) -> bool {
        matches!(self, Self::UndoAction(_))
    }

    /// Check if the result is an undotree action.
    #[must_use]
    pub const fn is_undotree_action(&self) -> bool {
        matches!(self, Self::UndotreeAction(_))
    }
    /// Check if the result is an edit action.
    #[must_use]
    pub const fn is_edit_action(&self) -> bool {
        matches!(self, Self::EditAction(_))
    }

    /// Check if the result is waiting for a character argument.
    #[must_use]
    pub const fn is_waiting_for_char(&self) -> bool {
        matches!(self, Self::WaitingForChar(_))
    }

    /// Check if the result is a search action.
    #[must_use]
    pub const fn is_search_action(&self) -> bool {
        matches!(self, Self::SearchAction(_))
    }

    /// Create an error result.
    #[must_use]
    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }

    /// Create an edit action result.
    #[must_use]
    pub fn edit_action(
        buffer_id: BufferId,
        edit: Edit,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Self {
        Self::EditAction(EditAction::single(buffer_id, edit, cursor_before, cursor_after))
    }

    /// Create an edit action result from multiple edits.
    #[must_use]
    pub fn edit_actions(
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Self {
        Self::EditAction(EditAction::new(buffer_id, edits, cursor_before, cursor_after))
    }

    /// Create a waiting-for-char result for find-char operations.
    #[must_use]
    pub const fn waiting_for_char(find_type: FindType, start_position: Position) -> Self {
        Self::WaitingForChar(CharWaitContext::find_char(find_type, start_position))
    }

    /// Create a waiting-for-char result for replace-char operation.
    #[must_use]
    pub const fn waiting_for_replace_char(count: usize) -> Self {
        Self::WaitingForChar(CharWaitContext::replace_char(count))
    }

    /// Check if the result is a repeat action.
    #[must_use]
    pub const fn is_repeat_action(&self) -> bool {
        matches!(self, Self::RepeatAction)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    #[test]
    fn test_arg_spec_required() {
        let spec = ArgSpec::required("count", ArgKind::Count, "Number of times");
        assert_eq!(spec.name, "count");
        assert!(spec.required);
        assert_eq!(spec.kind, ArgKind::Count);
    }

    #[test]
    fn test_arg_spec_optional() {
        let spec = ArgSpec::optional("register", ArgKind::Register, "Target register");
        assert_eq!(spec.name, "register");
        assert!(!spec.required);
        assert_eq!(spec.kind, ArgKind::Register);
    }

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
    fn test_command_result_success() {
        let result = CommandResult::Success;
        assert!(result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
    }

    #[test]
    fn test_command_result_error() {
        let result = CommandResult::error("Something went wrong");
        assert!(!result.is_success());
        assert!(result.is_error());
        assert!(!result.is_quit());
    }

    #[test]
    fn test_command_result_quit() {
        assert!(CommandResult::Quit.is_quit());
        assert!(CommandResult::ForceQuit.is_quit());
    }

    #[test]
    fn test_command_result_undo_action() {
        let undo = CommandResult::UndoAction(UndoAction::Undo { count: 1 });
        assert!(undo.is_undo_action());
        assert!(!undo.is_success());
        assert!(!undo.is_error());
        assert!(!undo.is_quit());

        let redo = CommandResult::UndoAction(UndoAction::Redo { count: 3 });
        assert!(redo.is_undo_action());
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
    fn test_undo_action_variants() {
        let undo = UndoAction::Undo { count: 5 };
        let redo = UndoAction::Redo { count: 2 };

        assert_eq!(undo, UndoAction::Undo { count: 5 });
        assert_eq!(redo, UndoAction::Redo { count: 2 });
        assert_ne!(undo, redo);
    }

    #[test]
    fn test_undotree_action_toggle() {
        let action = UndotreeAction::Toggle { buffer_id: 42 };
        assert_eq!(action, UndotreeAction::Toggle { buffer_id: 42 });

        let result = CommandResult::UndotreeAction(action);
        assert!(result.is_undotree_action());
        assert!(!result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
        assert!(!result.is_undo_action());
    }

    #[test]
    fn test_undotree_action_close() {
        let action = UndotreeAction::Close;
        let result = CommandResult::UndotreeAction(action);
        assert!(result.is_undotree_action());
    }

    #[test]
    fn test_undotree_action_goto_node() {
        let action = UndotreeAction::GotoNode { node_index: 5 };
        assert_eq!(action, UndotreeAction::GotoNode { node_index: 5 });
    }

    #[test]
    fn test_undotree_action_navigation() {
        // Test all navigation variants
        let goto_selected = UndotreeAction::GotoSelected;
        let move_up = UndotreeAction::MoveUp;
        let move_down = UndotreeAction::MoveDown;

        // They should all be distinct
        assert_ne!(goto_selected, move_up);
        assert_ne!(move_up, move_down);
        assert_ne!(goto_selected, move_down);
    }

    #[test]
    fn test_undotree_action_all_variants() {
        // Verify all variants can be constructed and compared
        let variants = [
            UndotreeAction::Toggle { buffer_id: 0 },
            UndotreeAction::Close,
            UndotreeAction::GotoNode { node_index: 0 },
            UndotreeAction::GotoSelected,
            UndotreeAction::MoveUp,
            UndotreeAction::MoveDown,
        ];

        // Each variant wrapped in CommandResult should be an undotree action
        for action in variants {
            let result = CommandResult::UndotreeAction(action);
            assert!(result.is_undotree_action());
        }
    }

    #[test]
    fn test_command_trait_object_safety() {
        // Verify Command trait is object-safe
        fn _accepts_ref(_: &dyn Command) {}
        fn _accepts_box(_: Box<dyn Command>) {}
    }

    #[test]
    fn test_command_handler_trait_object_safety() {
        // Verify CommandHandler trait is object-safe
        fn _accepts_ref(_: &dyn CommandHandler) {}
        fn _accepts_box(_: Box<dyn CommandHandler>) {}
    }

    // Test implementation
    struct TestCommand;

    impl Command for TestCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "test-cmd")
        }

        fn description(&self) -> &'static str {
            "A test command"
        }

        fn args(&self) -> Vec<ArgSpec> {
            vec![ArgSpec::optional("count", ArgKind::Count, "Test count")]
        }

        fn names(&self) -> &[&'static str] {
            &["test", "t"]
        }
    }

    impl CommandHandler for TestCommand {
        fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
            if args.count().unwrap_or(0) > 100 {
                CommandResult::error("Count too large")
            } else {
                CommandResult::Success
            }
        }
    }

    #[test]
    fn test_command_implementation() {
        let cmd = TestCommand;
        assert_eq!(cmd.id().name(), "test-cmd");
        assert_eq!(cmd.description(), "A test command");
        assert_eq!(cmd.args().len(), 1);
        assert_eq!(cmd.names(), &["test", "t"]);
    }

    // EditAction tests (Phase 2)

    #[test]
    fn test_edit_action_new() {
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "hello");
        let before = Position::new(0, 0);
        let after = Position::new(0, 5);

        let action = EditAction::new(buffer_id, vec![edit], before, after);

        assert_eq!(action.buffer_id, buffer_id);
        assert_eq!(action.edits.len(), 1);
        assert_eq!(action.cursor_before, before);
        assert_eq!(action.cursor_after, after);
    }

    #[test]
    fn test_command_result_edit_action() {
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "hello");
        let before = Position::new(0, 0);
        let after = Position::new(0, 5);

        let result = CommandResult::edit_action(buffer_id, edit, before, after);

        assert!(result.is_edit_action());
        assert!(!result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
    }

    #[test]
    fn test_command_result_is_edit_action() {
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "x");

        let edit_result =
            CommandResult::edit_action(buffer_id, edit, Position::new(0, 0), Position::new(0, 1));
        let success_result = CommandResult::Success;
        let error_result = CommandResult::error("fail");

        assert!(edit_result.is_edit_action());
        assert!(!success_result.is_edit_action());
        assert!(!error_result.is_edit_action());
    }

    #[test]
    fn test_edit_action_empty_edits() {
        let buffer_id = BufferId::from_raw(1);
        let before = Position::new(0, 0);
        let after = Position::new(0, 0);

        // Empty edits vec is valid (no-op edit)
        let action = EditAction::new(buffer_id, vec![], before, after);

        assert!(action.is_empty());
        assert_eq!(action.edits.len(), 0);
    }

    #[test]
    fn test_edit_action_single() {
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "hello");
        let before = Position::new(0, 0);
        let after = Position::new(0, 5);

        let action = EditAction::single(buffer_id, edit.clone(), before, after);

        assert_eq!(action.edits.len(), 1);
        assert_eq!(action.edits[0], edit);
    }

    // CharWaitContext tests (Phase 7 - find-char infrastructure)

    #[test]
    fn test_find_type_variants() {
        // Verify all variants can be used
        assert_eq!(FindType::FindForward, FindType::FindForward);
        assert_eq!(FindType::FindBackward, FindType::FindBackward);
        assert_eq!(FindType::TillForward, FindType::TillForward);
        assert_eq!(FindType::TillBackward, FindType::TillBackward);
    }

    #[test]
    fn test_char_wait_context_new() {
        let ctx = CharWaitContext::new(FindType::FindForward, Position::new(1, 5));
        assert_eq!(ctx.find_type(), Some(FindType::FindForward));
        assert_eq!(ctx.start_position, Some(Position::new(1, 5)));
    }

    #[test]
    fn test_char_wait_context_find_char() {
        let ctx = CharWaitContext::find_char(FindType::TillBackward, Position::new(2, 3));
        assert_eq!(ctx.op_type, CharWaitOp::TillBackward);
        assert_eq!(ctx.find_type(), Some(FindType::TillBackward));
        assert_eq!(ctx.start_position, Some(Position::new(2, 3)));
        assert!(ctx.count.is_none());
    }

    #[test]
    fn test_char_wait_context_replace_char() {
        let ctx = CharWaitContext::replace_char(3);
        assert_eq!(ctx.op_type, CharWaitOp::ReplaceChar);
        assert_eq!(ctx.find_type(), None);
        assert!(ctx.start_position.is_none());
        assert_eq!(ctx.count, Some(3));
    }

    #[test]
    fn test_char_wait_op_is_find_char() {
        assert!(CharWaitOp::FindForward.is_find_char());
        assert!(CharWaitOp::FindBackward.is_find_char());
        assert!(CharWaitOp::TillForward.is_find_char());
        assert!(CharWaitOp::TillBackward.is_find_char());
        assert!(!CharWaitOp::ReplaceChar.is_find_char());
    }

    #[test]
    fn test_char_wait_op_is_replace_char() {
        assert!(!CharWaitOp::FindForward.is_replace_char());
        assert!(CharWaitOp::ReplaceChar.is_replace_char());
    }

    #[test]
    fn test_char_wait_op_to_find_type() {
        assert_eq!(CharWaitOp::FindForward.to_find_type(), Some(FindType::FindForward));
        assert_eq!(CharWaitOp::FindBackward.to_find_type(), Some(FindType::FindBackward));
        assert_eq!(CharWaitOp::TillForward.to_find_type(), Some(FindType::TillForward));
        assert_eq!(CharWaitOp::TillBackward.to_find_type(), Some(FindType::TillBackward));
        assert_eq!(CharWaitOp::ReplaceChar.to_find_type(), None);
    }

    #[test]
    fn test_char_wait_op_from_find_type() {
        let op: CharWaitOp = FindType::FindForward.into();
        assert_eq!(op, CharWaitOp::FindForward);

        let op: CharWaitOp = FindType::TillBackward.into();
        assert_eq!(op, CharWaitOp::TillBackward);
    }

    #[test]
    fn test_command_result_waiting_for_char() {
        let result = CommandResult::waiting_for_char(FindType::TillBackward, Position::new(0, 10));

        assert!(result.is_waiting_for_char());
        assert!(!result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
        assert!(!result.is_undo_action());
        assert!(!result.is_edit_action());
    }

    #[test]
    fn test_command_result_waiting_for_replace_char() {
        let result = CommandResult::waiting_for_replace_char(2);

        assert!(result.is_waiting_for_char());
        if let CommandResult::WaitingForChar(ctx) = result {
            assert_eq!(ctx.op_type, CharWaitOp::ReplaceChar);
            assert_eq!(ctx.count, Some(2));
        } else {
            panic!("Expected WaitingForChar");
        }
    }

    #[test]
    fn test_command_result_is_waiting_for_char() {
        let wait_result =
            CommandResult::waiting_for_char(FindType::FindForward, Position::new(0, 0));
        let success_result = CommandResult::Success;
        let error_result = CommandResult::error("fail");

        assert!(wait_result.is_waiting_for_char());
        assert!(!success_result.is_waiting_for_char());
        assert!(!error_result.is_waiting_for_char());
    }

    #[test]
    fn test_command_result_repeat_action() {
        let result = CommandResult::RepeatAction;
        assert!(result.is_repeat_action());
        assert!(!result.is_success());
        assert!(!result.is_error());
    }

    #[test]
    fn test_char_wait_context_equality() {
        let ctx1 = CharWaitContext::new(FindType::FindForward, Position::new(0, 5));
        let ctx2 = CharWaitContext::new(FindType::FindForward, Position::new(0, 5));
        let ctx3 = CharWaitContext::new(FindType::FindBackward, Position::new(0, 5));

        assert_eq!(ctx1, ctx2);
        assert_ne!(ctx1, ctx3);
    }

    #[test]
    fn test_find_type_equality() {
        assert_eq!(FindType::FindForward, FindType::FindForward);
        assert_ne!(FindType::FindForward, FindType::FindBackward);
        assert_ne!(FindType::TillForward, FindType::FindForward);
    }
}
