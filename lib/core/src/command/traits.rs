//! Core command trait definitions for extensible command system

use crate::buffer::Buffer;
use crate::modd::ModeState;
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
    ModeChange(ModeState),
    /// Editor should quit
    Quit,
    /// Command produced text for clipboard (e.g., yank, delete)
    /// `register` is the target register: None for unnamed, Some('a'-'z') for named, '+' for system
    ClipboardWrite { text: String, register: Option<char> },
    /// Command needs Runtime access (deferred execution)
    DeferToRuntime(DeferredAction),
    /// Command failed with error message
    Error(String),
}

/// Actions that require Runtime-level access
#[derive(Debug)]
pub enum DeferredAction {
    /// Paste from register
    /// `register` is the source register: None for unnamed, Some('a'-'z') for named, '+' for system
    Paste { before: bool, register: Option<char> },
    /// Command line operations
    CommandLine(CommandLineAction),
    /// Completion operations
    Completion(CompletionAction),
    /// Explorer operations
    Explorer(ExplorerAction),
    /// Telescope operations
    Telescope(TelescopeAction),
    /// Jump to older position (Ctrl-O)
    JumpOlder,
    /// Jump to newer position (Ctrl-I)
    JumpNewer,
    /// Execute operator with motion (e.g., dw, yj, c$)
    OperatorMotion(OperatorMotionAction),
}

/// Telescope fuzzy finder actions
#[derive(Debug)]
pub enum TelescopeAction {
    /// Open telescope with a specific picker
    Open { picker: String },
    /// Insert a character into the query
    InsertChar(char),
    /// Delete character from query (backspace)
    Backspace,
    /// Move cursor left in query
    CursorLeft,
    /// Move cursor right in query
    CursorRight,
    /// Select next item
    SelectNext,
    /// Select previous item
    SelectPrev,
    /// Page down
    PageDown,
    /// Page up
    PageUp,
    /// Go to first item
    GotoFirst,
    /// Go to last item
    GotoLast,
    /// Confirm selection
    Confirm,
    /// Close telescope
    Close,
    /// Enter insert mode (for typing query)
    EnterInsert,
    /// Enter normal mode (for j/k navigation)
    EnterNormal,
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

/// Operator + motion action (e.g., dw, yj, c$)
#[derive(Debug)]
pub enum OperatorMotionAction {
    /// Delete with motion (d + motion)
    Delete {
        motion: crate::motion::Motion,
        count: usize,
    },
    /// Yank with motion (y + motion)
    Yank {
        motion: crate::motion::Motion,
        count: usize,
    },
    /// Change with motion (c + motion)
    Change {
        motion: crate::motion::Motion,
        count: usize,
    },
    /// Delete text object (di(, da{, etc.)
    DeleteTextObject {
        text_object: crate::textobject::TextObject,
    },
    /// Yank text object (yi(, ya{, etc.)
    YankTextObject {
        text_object: crate::textobject::TextObject,
    },
    /// Change text object (ci(, ca{, etc.)
    ChangeTextObject {
        text_object: crate::textobject::TextObject,
    },
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
    fn valid_modes(&self) -> Option<Vec<ModeState>> {
        None
    }

    /// Optional: does this command support repeat count?
    fn supports_count(&self) -> bool {
        true
    }

    /// Whether this command is a "jump" that should be recorded in the jump list
    ///
    /// Commands like gg, G, search, etc. return true here so the cursor
    /// position before execution is recorded in the jump list.
    fn is_jump(&self) -> bool {
        false
    }
}

impl Clone for Box<dyn CommandTrait> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
