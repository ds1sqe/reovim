//! Core command trait definitions for extensible command system

use crate::buffer::Buffer;
use crate::modd::Mod;
use std::any::Any;
use std::fmt::Debug;

/// Execution context passed to commands
pub struct ExecutionContext<'a> {
    /// The buffer to operate on
    pub buffer: &'a mut Buffer,
    /// Repeat count (e.g., 5j moves down 5 lines)
    pub count: Option<usize>,
    /// ID of the buffer being operated on
    pub buffer_id: usize,
    /// ID of the window containing the buffer
    pub window_id: usize,
}

/// Result of command execution
#[derive(Debug)]
pub enum CommandResult {
    /// Command succeeded, no further action needed
    Success,
    /// Command succeeded, screen needs re-render
    NeedsRender,
    /// Command triggers a mode change
    ModeChange(Mod),
    /// Editor should quit
    Quit,
    /// Command produced text for clipboard (e.g., yank, delete)
    ClipboardWrite(String),
    /// Command needs Runtime access (deferred execution)
    DeferToRuntime(DeferredAction),
    /// Command failed with error message
    Error(String),
}

/// Actions that require Runtime-level access
#[derive(Debug)]
pub enum DeferredAction {
    /// Paste from clipboard
    Paste { before: bool },
    /// Command line operations
    CommandLine(CommandLineAction),
    /// Completion operations
    Completion(CompletionAction),
    /// Explorer operations
    Explorer(ExplorerAction),
}

/// Completion actions
#[derive(Debug)]
pub enum CompletionAction {
    /// Trigger completion at cursor
    Trigger,
    /// Select next completion item
    SelectNext,
    /// Select previous completion item
    SelectPrev,
    /// Confirm selected completion
    Confirm,
    /// Dismiss completion popup
    Dismiss,
}

/// Explorer mode actions that require runtime access
#[derive(Debug)]
pub enum ExplorerAction {
    /// Move cursor up
    CursorUp { count: usize },
    /// Move cursor down
    CursorDown { count: usize },
    /// Page up
    PageUp,
    /// Page down
    PageDown,
    /// Go to first item
    GotoFirst,
    /// Go to last item
    GotoLast,
    /// Toggle expand/collapse on current node
    ToggleNode,
    /// Open file or toggle directory
    OpenNode,
    /// Close parent directory
    CloseParent,
    /// Go to parent directory
    GoToParent,
    /// Refresh tree from filesystem
    Refresh,
    /// Toggle showing hidden files
    ToggleHidden,
    /// Close explorer (switch to editor)
    Close,
    /// Focus editor window
    FocusEditor,
    /// Toggle explorer visibility
    Toggle,
    /// Start creating a new file (enters input mode)
    CreateFile,
    /// Start creating a new directory (enters input mode)
    CreateDir,
    /// Start renaming current item (enters input mode)
    Rename,
    /// Delete current item (with confirmation)
    Delete,
    /// Start filtering (enters filter input mode)
    StartFilter,
    /// Clear the current filter
    ClearFilter,
    /// Confirm pending operation (create/rename/delete)
    ConfirmInput { input: String },
    /// Cancel pending operation
    CancelInput,
    /// Handle character input during input mode
    InputChar { c: char },
    /// Handle backspace during input mode
    InputBackspace,
}

/// Command line mode actions
#[derive(Debug)]
pub enum CommandLineAction {
    /// Insert a character into command line
    InsertChar(char),
    /// Delete character (backspace)
    Backspace,
    /// Execute the command line
    Execute,
    /// Cancel command line mode
    Cancel,
}

/// The core command trait - all commands must implement this
///
/// Commands are the fundamental unit of editor actions. They receive
/// an execution context containing the buffer and metadata, and return
/// a result indicating what action the runtime should take.
pub trait CommandTrait: Debug + Send + Sync {
    /// Unique identifier for this command (e.g., `cursor_up`, `enter_insert_mode`)
    fn name(&self) -> &'static str;

    /// Human-readable description for help/documentation
    fn description(&self) -> &'static str;

    /// Execute the command on the given buffer
    ///
    /// # Arguments
    /// * `ctx` - Execution context containing buffer and metadata
    ///
    /// # Returns
    /// Result indicating what action the runtime should take
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult;

    /// Clone into a boxed trait object
    ///
    /// This is required because `Clone` is not object-safe
    fn clone_box(&self) -> Box<dyn CommandTrait>;

    /// Downcast support for type inspection
    fn as_any(&self) -> &dyn Any;

    /// Optional: which modes this command is valid in (None = all modes)
    fn valid_modes(&self) -> Option<Vec<Mod>> {
        None
    }

    /// Optional: does this command support repeat count?
    fn supports_count(&self) -> bool {
        true
    }
}

impl Clone for Box<dyn CommandTrait> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
