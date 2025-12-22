//! Core command trait definitions for extensible command system

use {
    crate::{
        buffer::Buffer,
        event_bus::DynEvent,
        modd::ModeState,
        screen::{NavigateDirection, SplitDirection},
    },
    std::{any::Any, fmt::Debug},
};

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
    ClipboardWrite {
        text: String,
        register: Option<char>,
    },
    /// Command needs Runtime access (deferred execution)
    ///
    /// DEPRECATED: Use `Deferred(Box<dyn DeferredActionHandler>)` for new code.
    /// This variant is kept for backward compatibility during plugin migration.
    DeferToRuntime(DeferredAction),
    /// Command needs Runtime access via trait-based handler
    ///
    /// This is the preferred way for plugins to defer actions to the runtime.
    /// The handler receives a `RuntimeContext` and can perform any runtime operation.
    Deferred(Box<dyn DeferredActionHandler>),
    /// Command emits an event to the event bus
    EmitEvent(DynEvent),
    /// Command failed with error message
    Error(String),
}

/// Trait for deferred action handlers
///
/// This trait allows plugins to define custom actions that require
/// runtime access. Instead of adding variants to `DeferredAction`,
/// plugins implement this trait.
///
/// # Example
///
/// ```ignore
/// struct MyPluginAction { /* ... */ }
///
/// impl DeferredActionHandler for MyPluginAction {
///     fn handle(&self, ctx: &mut dyn RuntimeContext) {
///         // Access runtime via context
///         ctx.with_state_mut::<MyState, _, _>(|state| {
///             // Modify state
///         });
///     }
///
///     fn name(&self) -> &'static str {
///         "my_plugin_action"
///     }
/// }
/// ```
pub trait DeferredActionHandler: Send + Sync {
    /// Execute the deferred action with runtime context
    fn handle(&self, ctx: &mut dyn crate::runtime::RuntimeContext);

    /// Name of this action for debugging
    fn name(&self) -> &'static str;
}

impl std::fmt::Debug for dyn DeferredActionHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeferredActionHandler({})", self.name())
    }
}

/// Actions that require Runtime-level access
///
/// For new features, prefer using:
/// - `CommandResult::Deferred(Box<dyn DeferredActionHandler>)` for complex handlers
/// - `CommandResult::EmitEvent(DynEvent)` for event-driven features
#[derive(Debug)]
pub enum DeferredAction {
    Paste {
        before: bool,
        register: Option<char>,
    },
    CommandLine(CommandLineAction),
    JumpOlder,
    JumpNewer,
    OperatorMotion(OperatorMotionAction),
    Window(WindowAction),
    Tab(TabAction),
    Buffer(BufferAction),
    File(FileAction),
}

#[derive(Debug)]
pub enum WindowAction {
    SplitHorizontal {
        filename: Option<String>,
    },
    SplitVertical {
        filename: Option<String>,
    },
    Close {
        force: bool,
    },
    CloseOthers,
    FocusDirection {
        direction: NavigateDirection,
    },
    MoveDirection {
        direction: NavigateDirection,
    },
    Resize {
        direction: SplitDirection,
        delta: i16,
    },
    Equalize,
    /// Focus window or create split if none exists
    FocusOrSplitLeft,
    FocusOrSplitDown,
    FocusOrSplitUp,
    FocusOrSplitRight,
}

#[derive(Debug)]
pub enum TabAction {
    New { filename: Option<String> },
    Close,
    Next,
    Prev,
    Goto { index: usize },
}

/// Buffer navigation actions
#[derive(Debug)]
pub enum BufferAction {
    /// Go to previous buffer
    Prev,
    /// Go to next buffer
    Next,
    /// Delete current buffer
    Delete { force: bool },
}

/// File operations that require Runtime access
///
/// Used by plugins (e.g., Explorer) to request file system operations
/// that need Runtime access for buffer management.
#[derive(Debug)]
pub enum FileAction {
    /// Open a file into a buffer
    Open { path: String },
    /// Create a new file or directory
    Create { path: String, is_dir: bool },
    /// Delete a file or directory
    Delete { path: String },
    /// Rename/move a file or directory
    Rename { from: String, to: String },
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
    /// Delete semantic text object (dif, dac, etc.) - uses treesitter
    DeleteSemanticTextObject {
        text_object: crate::textobject::SemanticTextObjectSpec,
    },
    /// Yank semantic text object (yif, yac, etc.) - uses treesitter
    YankSemanticTextObject {
        text_object: crate::textobject::SemanticTextObjectSpec,
    },
    /// Change semantic text object (cif, cac, etc.) - uses treesitter
    ChangeSemanticTextObject {
        text_object: crate::textobject::SemanticTextObjectSpec,
    },
    /// Delete word text object (diw, daw, diW, daW)
    DeleteWordTextObject {
        text_object: crate::textobject::WordTextObject,
    },
    /// Yank word text object (yiw, yaw, yiW, yaW)
    YankWordTextObject {
        text_object: crate::textobject::WordTextObject,
    },
    /// Change word text object (ciw, caw, ciW, caW)
    ChangeWordTextObject {
        text_object: crate::textobject::WordTextObject,
    },
    /// Change entire line (cc) - clears line content and enters insert mode
    ChangeLine,
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

    /// Whether this command modifies buffer text content
    ///
    /// Commands that insert, delete, or modify text return true here.
    /// This is used to trigger treesitter reparsing after buffer modifications.
    fn is_text_modifying(&self) -> bool {
        false
    }
}

impl Clone for Box<dyn CommandTrait> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
