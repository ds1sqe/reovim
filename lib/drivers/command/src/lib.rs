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
    reovim_kernel::api::v1::{BufferId, CommandId, KernelContext},
    std::collections::HashMap,
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
/// Provides typed access to arguments parsed from user input.
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
#[derive(Debug, Clone, Default)]
pub struct CommandContext {
    args: HashMap<&'static str, ArgValue>,
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

    /// Create an error result.
    #[must_use]
    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
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
    fn test_undo_action_variants() {
        let undo = UndoAction::Undo { count: 5 };
        let redo = UndoAction::Redo { count: 2 };

        assert_eq!(undo, UndoAction::Undo { count: 5 });
        assert_eq!(redo, UndoAction::Redo { count: 2 });
        assert_ne!(undo, redo);
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
}
