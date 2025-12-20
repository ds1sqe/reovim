use std::path::PathBuf;

use tokio::sync::oneshot;

use crate::{
    bind::CommandRef,
    command::{CommandContext, traits::OperatorMotionAction},
    completion::CompletionItem,
    highlight::{Highlight, HighlightGroup},
    leap::LeapDirection,
    modd::{ModeState, OperatorType},
    rpc::RpcResponse,
    screen::{NavigateDirection, SplitDirection},
    telescope::{PreviewContent, TelescopeItem},
    textobject::{SemanticTextObjectSpec, TextObject, WordTextObject},
    treesitter::BufferEdit,
};

pub enum InnerEvent {
    BufferEvent(BufferEvent),
    WindowEvent(WindowEvent),
    CommandEvent(CommandEvent),
    ModeChangeEvent(ModeState),
    PendingKeysEvent(String),
    HighlightEvent(HighlightEvent),
    CompletionEvent(CompletionEvent),
    ExplorerEvent(ExplorerEvent),
    /// Operator + motion action (d+motion, y+motion, c+motion)
    OperatorMotionEvent(OperatorMotionAction),
    /// Visual mode text object selection (viw, vi(, vif, etc.)
    VisualTextObjectEvent(VisualTextObjectAction),
    TelescopeEvent(TelescopeEvent),
    LeapEvent(LeapEvent),
    /// Treesitter-related events for incremental parsing
    TreesitterEvent(TreesitterEvent),
    /// Settings menu events
    SettingsMenuEvent(SettingsMenuEvent),
    RenderSignal,
    KillSignal,
    /// Terminal screen resize event
    ScreenResizeEvent {
        width: u16,
        height: u16,
    },
    /// RPC request from server mode
    RpcRequest {
        /// Request ID
        id: u64,
        /// Method name
        method: String,
        /// Method parameters
        params: serde_json::Value,
        /// Channel to send the response
        response_tx: oneshot::Sender<RpcResponse>,
    },
    /// Focus-related input events (delegated to active focus target)
    FocusInputEvent(FocusInputEvent),
    /// Generic focus input - dispatched to registered handler via enlist pattern
    FocusInput {
        /// Character to insert (None for delete-only)
        char: Option<char>,
        /// Whether to delete backward
        delete: bool,
        /// Whether to clear landing page (editor-specific flag)
        clear_landing: bool,
    },
}

/// Input events routed to the active focus target
#[derive(Debug, Clone, Copy)]
pub enum FocusInputEvent {
    /// Insert a character at the cursor position
    InsertChar(char),
    /// Delete the character before the cursor
    DeleteCharBackward,
}

/// Buffer-related events
pub enum BufferEvent {
    /// Set buffer content directly
    SetContent { buffer_id: usize, content: String },
    /// Load a file into a buffer
    LoadFile { buffer_id: usize, path: PathBuf },
    /// Create a new empty buffer
    Create { buffer_id: usize },
    /// Close a buffer
    Close { buffer_id: usize },
    /// Switch to a different buffer
    Switch { buffer_id: usize },
}

/// Window-related events
pub enum WindowEvent {
    // Explorer
    ToggleExplorer,
    FocusExplorer,
    FocusEditor,

    // Splits
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

    // Navigation
    FocusDirection {
        direction: NavigateDirection,
    },
    MoveWindow {
        direction: NavigateDirection,
    },

    // Resize
    Resize {
        direction: SplitDirection,
        delta: i16,
    },
    Equalize,

    // Tabs
    TabNew {
        filename: Option<String>,
    },
    TabClose,
    TabNext,
    TabPrev,
    TabGoto {
        index: usize,
    },
}

/// Explorer-related events
pub enum ExplorerEvent {
    /// Toggle explorer visibility
    Toggle,
    /// Open a file from the explorer
    OpenFile { path: PathBuf },
    /// Refresh the explorer tree
    Refresh,
    /// Set the explorer root directory
    SetRoot { path: PathBuf },
}

/// Highlight update events
pub enum HighlightEvent {
    /// Add highlights to a buffer
    Add {
        buffer_id: usize,
        highlights: Vec<Highlight>,
    },
    /// Clear a highlight group from a buffer
    ClearGroup {
        buffer_id: usize,
        group: HighlightGroup,
    },
    /// Clear all highlights from a buffer
    ClearAll { buffer_id: usize },
}

/// Command event to be processed by runtime
pub struct CommandEvent {
    pub command: CommandRef,
    pub context: CommandContext,
}

/// Completion-related events
pub enum CompletionEvent {
    /// Request completion at current position
    Trigger { buffer_id: usize },
    /// Update completion items (from async completion fetch)
    Update {
        items: Vec<CompletionItem>,
        prefix: String,
        start_col: u16,
        start_row: u16,
    },
    /// Select next completion item
    SelectNext,
    /// Select previous completion item
    SelectPrev,
    /// Confirm the selected completion
    Confirm,
    /// Dismiss the completion popup
    Dismiss,
    /// Update filter as user continues typing
    UpdateFilter { new_prefix: String },
}

/// Telescope-related events
pub enum TelescopeEvent {
    /// Open telescope with a specific picker
    Open { picker: String },
    /// Update the search query
    UpdateQuery { query: String },
    /// Update items from async fetch
    UpdateItems { items: Vec<TelescopeItem> },
    /// Select next item
    SelectNext,
    /// Select previous item
    SelectPrev,
    /// Page down
    PageDown,
    /// Page up
    PageUp,
    /// Confirm selection
    Confirm,
    /// Close telescope
    Close,
    /// Update preview content
    UpdatePreview { content: PreviewContent },
}

/// Leap motion events
pub enum LeapEvent {
    /// Start leap mode (from `s` or `S` in normal mode)
    Start {
        direction: LeapDirection,
        operator: Option<OperatorType>,
        count: Option<usize>,
    },
    /// First character entered
    FirstChar { char: char },
    /// Second character entered, find matches
    SecondChar { char: char },
    /// User pressed a label key to jump
    SelectLabel { label: String },
    /// Cancel leap mode (Escape)
    Cancel,
}

/// Treesitter-related events for incremental parsing
pub enum TreesitterEvent {
    /// Schedule a buffer for reparsing after an edit
    ScheduleReparse { buffer_id: usize },
    /// Perform incremental parse with edit information
    IncrementalParse { buffer_id: usize, edit: BufferEdit },
    /// Force a full reparse of a buffer
    FullReparse { buffer_id: usize },
}

/// Visual mode text object selection actions (viw, vi(, vif, etc.)
#[derive(Debug)]
pub enum VisualTextObjectAction {
    /// Select delimiter-based text object (vi(, va{, etc.)
    SelectDelimiter { text_object: TextObject },
    /// Select word text object (viw, vaw, viW, vaW)
    SelectWord { text_object: WordTextObject },
    /// Select semantic text object (vif, vac, etc.) - uses treesitter
    SelectSemantic { text_object: SemanticTextObjectSpec },
}

/// Settings menu events for the TUI settings panel
#[derive(Debug, Clone)]
pub enum SettingsMenuEvent {
    /// Open the settings menu
    Open,
    /// Close the settings menu
    Close,
    /// Navigate to next item
    SelectNext,
    /// Navigate to previous item
    SelectPrev,
    /// Toggle boolean setting (Space key)
    Toggle,
    /// Cycle to next choice option (l key)
    CycleNext,
    /// Cycle to previous choice option (h key)
    CyclePrev,
    /// Quick select a choice by index (1-9 keys)
    QuickSelect(u8),
    /// Increment number value (+ key)
    Increment,
    /// Decrement number value (- key)
    Decrement,
    /// Execute action item (Enter key)
    ExecuteAction,
    /// Input a character in text input mode
    InputChar(char),
    /// Delete character in text input mode (Backspace)
    InputBackspace,
    /// Confirm text input
    InputConfirm,
    /// Cancel text input (Escape in input mode)
    InputCancel,
}
