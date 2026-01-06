use std::path::PathBuf;

use {reovim_sys::event::KeyModifiers, tokio::sync::oneshot};

use crate::{
    bind::CommandRef,
    command::{CommandContext, traits::OperatorMotionAction},
    event_bus::DynEvent,
    highlight::{Highlight, HighlightGroup},
    modd::{ComponentId, ModeState},
    plugin::PluginId,
    rpc::RpcResponse,
    screen::{NavigateDirection, SplitDirection, window::SignColumnMode},
    syntax::SyntaxProvider,
    textobject::{SemanticTextObjectSpec, TextObject, WordTextObject},
};

pub enum InnerEvent {
    BufferEvent(BufferEvent),
    WindowEvent(WindowEvent),
    CommandEvent(CommandEvent),
    ModeChangeEvent(ModeState),
    PendingKeysEvent(String),
    HighlightEvent(HighlightEvent),
    SyntaxEvent(SyntaxEvent),
    /// Operator + motion action (d+motion, y+motion, c+motion)
    OperatorMotionEvent(OperatorMotionAction),
    /// Visual mode text object selection (viw, vi(, vif, etc.)
    VisualTextObjectEvent(VisualTextObjectAction),
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
    /// Text input events (delegated to text input target)
    TextInputEvent(TextInputEvent),
    /// Plugin-defined event (type-erased)
    ///
    /// This variant enables plugins to define their own event types
    /// that are dispatched through the event bus. During migration,
    /// this runs alongside the existing hardcoded event handlers.
    PluginEvent {
        /// Plugin that sent this event (for routing)
        plugin_id: PluginId,
        /// Type-erased event payload
        event: DynEvent,
    },
    /// Request to open a file (from explorer, commands, etc.)
    OpenFileRequest {
        /// Path to the file to open
        path: PathBuf,
    },
    /// Request to open a file at a specific position (for LSP navigation)
    OpenFileAtPositionRequest {
        /// Path to the file to open
        path: PathBuf,
        /// Line number (0-indexed)
        line: usize,
        /// Column number (0-indexed)
        column: usize,
    },
    /// Request to set a register's content
    SetRegister {
        /// Register name: None for unnamed, '+' for system clipboard, 'a'-'z' for named
        register: Option<char>,
        /// Text content to set
        text: String,
    },
    // =========================================================================
    // Settings/Option capability events
    // =========================================================================
    /// Set line number visibility
    SetLineNumbers {
        enabled: bool,
    },
    /// Set relative line number visibility
    SetRelativeLineNumbers {
        enabled: bool,
    },
    /// Set color theme
    SetTheme {
        name: String,
    },
    /// Set scrollbar visibility
    SetScrollbar {
        enabled: bool,
    },
    /// Set indent guide visibility
    SetIndentGuide {
        enabled: bool,
    },
    /// Set sign column configuration
    SetSignColumn {
        mode: SignColumnMode,
    },
    /// Move cursor to specific position (from plugins like range-finder, LSP)
    MoveCursor {
        buffer_id: usize,
        line: u32,
        column: u32,
    },
    /// Apply completion text to command line
    ApplyCmdlineCompletion {
        text: String,
        replace_start: usize,
    },
    /// Mouse event from terminal
    MouseEvent(MouseEvent),
}

/// Input events routed to the active focus target
#[derive(Debug, Clone, Copy)]
pub enum TextInputEvent {
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
    FocusPlugin {
        id: ComponentId,
    },
    /// Return focus to the editor (unfocus any plugin)
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

/// Syntax provider events
pub enum SyntaxEvent {
    /// Attach a syntax provider to a buffer
    Attach {
        buffer_id: usize,
        syntax: Box<dyn SyntaxProvider>,
    },
    /// Detach syntax provider from a buffer
    Detach { buffer_id: usize },
    /// Request a reparse (after buffer modification)
    Reparse { buffer_id: usize },
}

/// Command event to be processed by runtime
pub struct CommandEvent {
    pub command: CommandRef,
    pub context: CommandContext,
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

// =============================================================================
// Mouse Events
// =============================================================================

/// Mouse event from terminal input
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// Type of mouse action
    pub kind: MouseEventKind,
    /// Column (x) position in terminal coordinates
    pub column: u16,
    /// Row (y) position in terminal coordinates
    pub row: u16,
    /// Modifier keys held during the event
    pub modifiers: KeyModifiers,
}

/// Type of mouse action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventKind {
    /// Mouse button pressed down
    Down(MouseButton),
    /// Mouse button released
    Up(MouseButton),
    /// Mouse dragged while button held
    Drag(MouseButton),
    /// Scroll wheel up
    ScrollUp,
    /// Scroll wheel down
    ScrollDown,
    /// Scroll wheel left (horizontal)
    ScrollLeft,
    /// Scroll wheel right (horizontal)
    ScrollRight,
    /// Mouse moved (without button pressed)
    Moved,
}

/// Mouse button identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button (scroll wheel click)
    Middle,
}

impl From<reovim_sys::event::MouseEvent> for MouseEvent {
    fn from(event: reovim_sys::event::MouseEvent) -> Self {
        Self {
            kind: event.kind.into(),
            column: event.column,
            row: event.row,
            modifiers: event.modifiers,
        }
    }
}

impl From<reovim_sys::event::MouseEventKind> for MouseEventKind {
    fn from(kind: reovim_sys::event::MouseEventKind) -> Self {
        use reovim_sys::event::MouseEventKind as CT;
        match kind {
            CT::Down(btn) => Self::Down(btn.into()),
            CT::Up(btn) => Self::Up(btn.into()),
            CT::Drag(btn) => Self::Drag(btn.into()),
            CT::Moved => Self::Moved,
            CT::ScrollUp => Self::ScrollUp,
            CT::ScrollDown => Self::ScrollDown,
            CT::ScrollLeft => Self::ScrollLeft,
            CT::ScrollRight => Self::ScrollRight,
        }
    }
}

impl From<reovim_sys::event::MouseButton> for MouseButton {
    fn from(button: reovim_sys::event::MouseButton) -> Self {
        use reovim_sys::event::MouseButton as CT;
        match button {
            CT::Left => Self::Left,
            CT::Right => Self::Right,
            CT::Middle => Self::Middle,
        }
    }
}
